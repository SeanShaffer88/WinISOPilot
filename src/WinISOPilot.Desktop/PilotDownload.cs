using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Threading;
using ImageGuide;
using Microsoft.Win32;

namespace WinISOPilot.Desktop;

public sealed partial class PilotWindow
{
    void DownloadDialog()
    {
        if (busy) return;
        var panel = new StackPanel();
        panel.Children.Add(Text("Get an official Windows 11 ISO", 24));
        panel.Children.Add(Muted("The download does not change your selected source. Choose Use downloaded ISO when it finishes; inspection is a separate step."));
        panel.Children.Add(Text("1. Get a Microsoft download link", 20));
        panel.Children.Add(Muted("Automatic: Fido 1.71, the Rufus link provider, runs unelevated through Windows PowerShell. This pin offers the latest release it knows: Windows 11 26H2 / build 26300.9457, x64 Home/Pro/Edu. This does not make build 26300 compatible with offline servicing."));
        var language = new ComboBox { ItemsSource = Engine.DownloadLanguages, SelectedIndex = 0, MinWidth = 240 };
        Register(language, "Download language", "Automatic ISO link language; this first slice offers English and English International"); panel.Children.Add(language);
        var state = Muted("No download link requested.");
        var provider = ActionButton("Get Microsoft download link", () => { }, true, "Get ISO link"); panel.Children.Add(provider);
        panel.Children.Add(Muted("If Windows blocks the provider or Microsoft rejects the request, use the alternative below. WinISOPilot does not change script policy or auto-update the provider."));
        var alternative = new StackPanel();
        alternative.Children.Add(ActionButton("Open Microsoft's Windows 11 download page", () =>
        {
            try { Process.Start(new ProcessStartInfo(Engine.MicrosoftIsoPage) { UseShellExecute = true }); }
            catch (Exception) { state.Text = "The browser could not open. Visit microsoft.com/software-download/windows11, choose the x64 ISO and language, then copy its generated download link."; }
        }));
        alternative.Children.Add(Muted("Choose the x64 disk-image download and language on Microsoft, then copy the generated ISO link. Links expire. Paste the complete link here:"));
        var manual = new TextBox { MaxLength = 16384, TextWrapping = TextWrapping.Wrap, MinHeight = 46 };
        Register(manual, "Microsoft ISO link", "Paste a generated Microsoft HTTPS ISO download link; query tokens are not logged"); alternative.Children.Add(manual);
        panel.Children.Add(new Expander { Header = "Alternative: use Microsoft's page and paste a link", Content = alternative });
        panel.Children.Add(Text("2. Save the ISO", 20));
        panel.Children.Add(Muted("Optional: paste the matching SHA-256 checksum from Microsoft. A computed hash alone does not prove publisher authenticity."));
        var checksum = new TextBox { MaxLength = 64 }; Register(checksum, "Expected ISO SHA-256", "Optional matching SHA-256 checksum from Microsoft"); panel.Children.Add(checksum);
        var download = ActionButton("Choose destination and download ISO", () => { }, true, "Download ISO"); panel.Children.Add(download);
        panel.Children.Add(Muted("4–32 GiB ISO, local NTFS folder, no overwrite. Pause stops reads at chunk boundaries; a long pause may time out. Interrupted transfers restart from the beginning. Keep partial data is the default after Cancel."));
        panel.Children.Add(state);
        var live = Text("", 15); panel.Children.Add(live);
        var controls = Row();
        var pauseDownload = ActionButton("Pause / resume", PauseJob); var cancelDownload = ActionButton("Cancel download / link request", CancelJob);
        controls.Children.Add(pauseDownload); controls.Children.Add(cancelDownload); panel.Children.Add(controls);
        var use = ActionButton("Use downloaded ISO", () => { }, true, "Use downloaded ISO"); use.IsEnabled = false; panel.Children.Add(use);
        var dialog = Dialog("Download Windows 11", panel, 920, 760);
        panel.Children.Add(DialogClose(dialog, "Close", true));
        Uri? generated = null; IsoDownloadResult? completed = null; bool working = false;
        var timer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(250) };
        void Refresh()
        {
            provider.IsEnabled = !working && !Engine.IsAdministrator; language.IsEnabled = manual.IsEnabled = checksum.IsEnabled = download.IsEnabled = !working;
            pauseDownload.IsEnabled = cancelDownload.IsEnabled = working && job != null && !critical;
            use.IsEnabled = !working && completed != null;
            live.Text = working ? JobDescription() : "";
        }
        timer.Tick += (_, _) => Refresh(); timer.Start();
        if (Engine.IsAdministrator) state.Text = "Automatic link generation is unavailable while elevated. Reopen without administrator privileges, or use Microsoft's page and paste the link.";
        provider.Click += async (_, _) =>
        {
            if (working || !Prepare()) return;
            working = true; generated = null; completed = null; BeginWork("", false); Refresh();
            try { generated = await Engine.GenerateMicrosoftIsoLink((string)language.SelectedItem, job!, ProgressFor(job!)); state.Text = "Microsoft link ready for " + language.SelectedItem + ". Choose a destination to start downloading. Link tokens remain private."; }
            catch (OperationCanceledException) { state.Text = "Link request canceled. No ISO download started."; }
            catch (Exception ex) { state.Text = ex.Message; }
            finally { EndWork(); working = false; Refresh(); }
        };
        manual.TextChanged += (_, _) => { generated = null; completed = null; Refresh(); };
        checksum.TextChanged += (_, _) => { if(completed!=null)state.Text="Checksum changed. Download again to compare the new checksum."; completed=null; Refresh(); };
        language.SelectionChanged += (_, _) => { generated = null; completed = null; state.Text = "Language changed. Request a new link before downloading."; Refresh(); };
        download.Click += async (_, _) =>
        {
            if (working) return;
            string link;
            try { link = (generated ?? Engine.ValidateMicrosoftIsoLink(manual.Text)).AbsoluteUri; }
            catch (Exception ex) { state.Text = ex.Message; return; }
            var picker = new SaveFileDialog { Title = "Save a NEW Windows ISO — existing files are never replaced", Filter = "Windows ISO (*.iso)|*.iso", FileName = "Windows11.iso", AddExtension = true, OverwritePrompt = true };
            if (picker.ShowDialog(dialog) != true || !Prepare()) return;
            working = true; completed = null; BeginWork("", false); Guid id = job!.RunId; bool canceled = false; Refresh();
            try
            {
                completed = await Engine.DownloadIso(link, picker.FileName, checksum.Text, job!, ProgressFor(job!));
                state.Text = $"Downloaded {completed.Bytes:N0} bytes.\n{completed.Path}\nSHA-256: {completed.Sha256}\n" +
                    (completed.TrustedChecksumMatched ? "The supplied checksum matched." : "Publisher checksum was not compared.") + " No installer ran. Choose Use downloaded ISO, then Inspect ISO.";
            }
            catch (OperationCanceledException) { canceled = true; state.Text = "Download canceled. No completed ISO was published. Partial data remains unless you choose guarded cleanup."; }
            catch (Exception ex) { state.Text = "Download did not complete. Partial data is retained. " + ex.Message; }
            finally { EndWork(); working = false; Refresh(); }
            if (canceled)
            {
                working=true; busy=true; critical=true; Refresh();
                try { await OfferDownloadCleanup(id, dialog); }
                catch(Exception) { state.Text="Partial data retained. Cleanup could not be completed safely."; }
                finally { working=false; busy=false; critical=false; Render(); Refresh(); }
            }
        };
        use.Click += (_, _) => { if (!working && completed != null) { string path = completed.Path; dialog.Close(); SelectSource(path); } };
        dialog.Closing += (_, e) => { if (working) { e.Cancel = true; job?.RequestCancel(); state.Text = "Cancellation requested. Wait for the active request to finish safely, then close."; } };
        dialog.Closed += (_, _) => timer.Stop(); Refresh(); dialog.ShowDialog();
    }

    async Task OfferDownloadCleanup(Guid id, Window owner)
    {
        var preview = await Task.Run(() => Engine.PreviewDownloadCleanup(id));
        var panel = new StackPanel(); panel.Children.Add(Text("Keep partial data is the default.", 22));
        panel.Children.Add(Muted($"{preview.Files.Count} unchanged partial file ({preview.TotalBytes:N0} bytes) is eligible.\n" + string.Join("\n", preview.Warnings)));
        var dialog = Dialog("Canceled ISO download", panel, 760, 420); dialog.Owner = owner;
        bool deleting=false;
        dialog.Closing+=(_,e)=>{if(deleting)e.Cancel=true;};
        var row = Row(); var keep=DialogClose(dialog, "Keep partial file (recommended)", true); row.Children.Add(keep);
        var remove = ActionButton("Delete this unchanged partial file", () => { });
        remove.Click += async (_, _) =>
        {
            deleting=true; keep.IsEnabled=remove.IsEnabled=false;
            try { var result=await Task.Run(()=>Engine.CleanupDownload(id)); ShowText("Partial download cleanup",$"Removed: {result.Removed.Count}\n"+string.Join("\n",result.Errors),dialog); }
            catch(Exception) { ShowText("Partial download cleanup","Partial data retained. Cleanup completion could not be confirmed.",dialog); }
            finally { deleting=false; dialog.Close(); }
        };
        remove.IsEnabled = preview.CanDelete; row.Children.Add(remove); panel.Children.Add(row); dialog.ShowDialog();
    }
}
