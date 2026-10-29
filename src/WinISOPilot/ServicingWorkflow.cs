using System.Text.Json;

namespace ImageGuide;
public static partial class Engine
{
    static void ValidateServicingDestination(string output, long mediaBytes)
    {
        CheckPath(output);
        var drive = new DriveInfo(Path.GetPathRoot(Path.GetFullPath(output))!);
        if (drive.DriveType != DriveType.Fixed || drive.DriveFormat != "NTFS") throw new IOException("Offline servicing requires a local fixed NTFS drive.");
        // Expanded component stores can greatly exceed compressed media. Conservative first-slice reserve.
        long reserve = checked(mediaBytes * 4 + 60L * 1024 * 1024 * 1024);
        if (drive.AvailableFreeSpace < reserve) throw new IOException("Offline servicing needs four times the ISO size plus 60 GB free for private WIM, mounted files, scratch, output and safety margin.");
    }
    internal static long ReadExpandedBytes(string xml, int selectedIndex)
    {
        if(xml.Length>4*1024*1024)throw new InvalidDataException("Expanded-size metadata exceeds bound.");
        var settings=new System.Xml.XmlReaderSettings{DtdProcessing=System.Xml.DtdProcessing.Prohibit,XmlResolver=null,MaxCharactersInDocument=4*1024*1024};
        using var reader=System.Xml.XmlReader.Create(new StringReader(xml),settings);
        var document=System.Xml.Linq.XDocument.Load(reader);
        var images=document.Root?.Elements("IMAGE").Where(x=>(string?)x.Attribute("INDEX")==selectedIndex.ToString(System.Globalization.CultureInfo.InvariantCulture)).ToArray()??[];
        if(images.Length!=1||images[0].Elements("TOTALBYTES").Count()!=1||!long.TryParse(images[0].Element("TOTALBYTES")?.Value,System.Globalization.NumberStyles.None,System.Globalization.CultureInfo.InvariantCulture,out long bytes)||bytes<=0||bytes>1024L*1024*1024*1024)
            throw new InvalidDataException("Selected-image expanded size is unknown or invalid; mounting is blocked.");
        return bytes;
    }
    static void RequireExpandedSpace(long expanded, string output, long mediaBytes)
    {
        ValidateServicingDestination(output,mediaBytes);
        if(expanded<=0||expanded>1024L*1024*1024*1024)throw new InvalidDataException("Trusted expanded size is unknown; mounting is blocked.");
        long required=checked(mediaBytes*4+Math.Max(60L*1024*1024*1024,checked(expanded*2+10L*1024*1024*1024)));
        if(new DriveInfo(Path.GetPathRoot(Path.GetFullPath(output))!).AvailableFreeSpace<required)throw new IOException("Not enough free space for the observed expanded image, scratch, copies and safety margin. Need "+required+" bytes free.");
    }
    public static Task<ServicingInventory> ReadServicingApps(Inspection inspection, int selectedIndex, string output, JobControl? control = null, IProgress<JobProgress>? progress = null)
    {
        inspection=FreezeInspection(inspection);
        return Task.Run(() =>
    {
        using var gate = new ServicingLease();
        return ReadServicingAppsCore(inspection, selectedIndex, output, control, progress).GetAwaiter().GetResult();
    });
    }
    static async Task<ServicingInventory> ReadServicingAppsCore(Inspection inspection, int selectedIndex, string output, JobControl? control, IProgress<JobProgress>? progress)
    {
        using var owned = control == null ? new JobControl() : null; control ??= owned!;
        if(inspection.SchemaVersion!=1||inspection.Cleanup!="HandleReleased"||!System.Text.RegularExpressions.Regex.IsMatch(inspection.SourceSha256??"","^[A-Fa-f0-9]{64}$"))throw new InvalidDataException("A completed inspection with verified source identity is required.");
        var edition = inspection.Editions.SingleOrDefault(x => x.Index == selectedIndex) ?? throw new InvalidDataException("Choose an observed edition.");
        var tool = RequireServicing(edition);
        string source = ValidateSource(inspection.Source);
        using var sourceLease = new FileStream(source, FileMode.Open, FileAccess.Read, FileShare.Read);
        string before = HashControlled(source, control, progress, "Checking original ISO");
        if (before != inspection.SourceSha256) throw new InvalidDataException("Original ISO changed. Inspect it again.");
        ValidateServicingDestination(output, sourceLease.Length);
        string root = NewFolder(output);
        using var directoryLease = CleanupNative.LeaseDirectory(root);
        string rootIdentity = CleanupNative.DirectoryIdentity(root);
        void Write(string relative, string text)
        {
            CheckPath(root);
            if (CleanupNative.DirectoryIdentity(root) != rootIdentity) throw new IOException("Inventory directory identity changed.");
            using var stream = new FileStream(Path.Combine(root, relative), FileMode.CreateNew, FileAccess.Write, FileShare.None);
            using var writer = new StreamWriter(stream); writer.Write(text);
        }
        OfflineSession? session = null; NativeIso? original = null; ServicingInventory? result = null; Exception? failure = null;
        try
        {
            {
                var iso = original = new NativeIso(source);
                iso.AttachOrReuse(); iso.VerifyAssociation();
                string payload = ValidateMediaLayout(iso.DriveRoot!);
                if (!payload.EndsWith(".wim", StringComparison.OrdinalIgnoreCase)) throw new InvalidDataException("This first servicing slice requires install.wim. ESD/SWM conversion is not supported.");
                string metadata = Path.Combine(root, "metadata"); Directory.CreateDirectory(metadata);
                long expanded=0;SameEditions(inspection.Editions, BuildReadMetadata(payload, metadata,xml=>expanded=ReadExpandedBytes(xml,selectedIndex)));
                RequireExpandedSpace(expanded,root,sourceLease.Length);
                string copy = Path.Combine(root, "inventory.wim");
                using (var input = new FileStream(payload, FileMode.Open, FileAccess.Read, FileShare.Read))
                using (var target = new FileStream(copy, FileMode.CreateNew, FileAccess.Write, FileShare.None))
                {
                    var buffer = new byte[1024 * 1024]; long copied = 0; int count;
                    while ((count = input.Read(buffer)) != 0) { control.Checkpoint("Copying private WIM for inventory", progress, copied, input.Length, true); target.Write(buffer, 0, count); copied += count; }
                    target.Flush(true);
                }
                iso.VerifyAssociation();
                string wimHash = HashControlled(copy, control, progress, "Checking private inventory WIM");
                session = new OfflineSession(root, copy, selectedIndex, tool, control, progress, Write,spaceCheck:()=>RequireExpandedSpace(expanded,root,sourceLease.Length));
                await session.Mount(true);
                var apps = await session.Apps();
                await session.Finish(false);
                result = new(Guid.NewGuid(), before, selectedIndex, edition.Version, tool.Path, tool.Version, Hash(tool.Path), wimHash, apps);
            }
        }
        catch (Exception ex) { failure = ex; }
        finally
        {
            if (session != null) try { await session.Release(); } catch (Exception ex) { failure = new IOException("Inventory mount cleanup uncertain; retain files.", ex); }
            if (original != null) try { original.Dispose(); if(original.Disposition!="HandleReleased") throw new IOException("ISO handle release unconfirmed."); } catch(Exception ex) { RequireRestart("Inventory source handle release uncertain."); failure = new IOException("Inventory source cleanup uncertain.", ex); }
            try { if (HashControlled(source, null, progress, "Verifying source identity", false) != before) { RequireRestart("Original changed during app inventory."); throw new IOException("Original ISO identity changed."); } }
            catch (Exception ex) { failure = ex; RequireRestart("Original verification could not complete."); }
        }
        if (failure == null) try { control.FinalCheckpoint("Finalizing app inventory", progress); } catch (Exception ex) { failure = ex; }
        if (failure != null)
        {
            Write("servicing-inventory-failure.json", JsonSerializer.Serialize(new { Status = failure is OperationCanceledException ? "Canceled" : "Failed", Error = failure.Message, RestartRequired, Cleanup = "Keep all files; native-generated files have no deletion authority." }, Json));
            if (failure is OperationCanceledException) throw new OperationCanceledException("App inventory canceled safely; original verified. Files retained.", failure);
            throw new IOException("App inventory failed. " + failure.Message, failure);
        }
        Write("servicing-inventory.json", JsonSerializer.Serialize(result, Json));
        RegisterServicingInventory(result!);
        return result!;
    }
    static void RegisterServicingInventory(ServicingInventory inventory)
    {
        lock(inventoryGate)servicingInventories.Add(inventory.Id,inventory with{Apps=inventory.Apps.ToArray()});
    }
    static ServicingInventory RequireInventory(Inspection inspection, Edition edition, ServicingChoices choices, Tool tool)
    {
        ServicingInventory inventory;
        lock (inventoryGate) inventory = servicingInventories.GetValueOrDefault(choices.InventoryId) ?? throw new InvalidDataException("Read apps in this app session before customization. Imported reports cannot authorize servicing.");
        if (inventory.SourceSha256 != inspection.SourceSha256 || inventory.EditionIndex != edition.Index || inventory.EditionVersion != edition.Version || inventory.ToolPath != tool.Path || inventory.ToolVersion != tool.Version || inventory.ToolSha256 != Hash(tool.Path)) throw new InvalidDataException("App inventory or servicing tool changed. Read apps again.");
        ValidateServicingChoices(inventory, choices); return inventory;
    }
    internal static void VerifyAppsAfter(ProvisionedApp[] before, ProvisionedApp[] after, string[] remove)
    {
        var expected = before.Select(x => x.PackageName).Except(remove, StringComparer.Ordinal).Order(StringComparer.Ordinal);
        if (!after.Select(x => x.PackageName).Order(StringComparer.Ordinal).SequenceEqual(expected, StringComparer.Ordinal)) throw new IOException("Post-change app inventory does not match the requested removals; no ISO will be published.");
    }
    static async Task<ServicingEvidence> ServiceBuildCopy(BuildRun run, string wim, Edition edition, Inspection inspection, ServicingChoices choices, long expandedBytes, long mediaBytes, JobControl control, IProgress<JobProgress>? progress)
    {
        var tool = RequireServicing(edition);
        var inventory = RequireInventory(inspection, edition, choices, tool);
        string before = HashControlled(wim, control, progress, "Checking WIM before servicing");
        if (before != inventory.WimSha256) throw new InvalidDataException("Copied WIM no longer matches app inventory. Read apps again.");
        // DISM will modify this file externally. Permanently remove its current-session delete proof.
        run.Created.Remove(wim); run.ExpectedHash.Remove(wim);
        var session = new OfflineSession(run.Root, wim, edition.Index, tool, control, progress, (name, text) => BuildWrite(run, name, text), inventory.ToolSha256,spaceCheck:()=>RequireExpandedSpace(expandedBytes,run.Root,mediaBytes));
        Exception? failure = null;
        try
        {
            await session.Mount(false);
            session.CheckPendingActions();
            var observed = await session.Apps();
            if (!observed.Select(x => x.PackageName).SequenceEqual(inventory.Apps.Select(x => x.PackageName), StringComparer.Ordinal)) throw new InvalidDataException("Provisioned app inventory changed before mutation.");
            string[] remove = ValidateServicingChoices(inventory, choices);
            foreach (string package in remove) await session.Remove(package);
            if (choices.ComponentCleanup) await session.Cleanup();
            var after = await session.Apps();
            VerifyAppsAfter(observed, after, remove);
            await session.Health();
            BuildWrite(run, "servicing-before-apps.json", JsonSerializer.Serialize(observed, Json));
            BuildWrite(run, "servicing-after-apps.json", JsonSerializer.Serialize(after, Json));
            await session.Finish(true);
        }
        catch (Exception ex) { failure = ex; }
        finally { try { await session.Release(); } catch (Exception ex) { failure = new IOException("Servicing teardown uncertain. Keep all files; restart and recover the exact owned mount manually.", ex); } }
        if (failure != null) { if (failure is OperationCanceledException) throw failure; throw new IOException("Offline servicing failed; no ISO published. " + failure.Message, failure); }
        control.Checkpoint("Customized WIM committed; checking copy", progress);
        var evidence = new ServicingEvidence(choices.RemovePackages.ToArray(), choices.ComponentCleanup, before, HashControlled(wim, control, progress, "Hashing customized WIM"));
        BuildWrite(run, "servicing-result.json", JsonSerializer.Serialize(evidence, Json));
        return evidence;
    }
}
