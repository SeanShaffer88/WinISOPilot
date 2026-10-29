namespace ImageGuide;
public sealed record MountReadiness(bool Executable,string[] Reasons)
{
    public static MountReadiness Evaluate(bool elevated,bool knownProfile,bool ownedCopy,bool expansionKnown)
    {
        var reasons=new List<string>{"Mount worker not implemented or validated; no mount command may execute."};if(!elevated)reasons.Add("An elevated token is required.");if(!knownProfile)reasons.Add("No documented servicing profile for this image and tool combination.");if(!ownedCopy)reasons.Add("A verified app-owned copy is required; source containers cannot be mounted writable.");if(!expansionKnown)reasons.Add("Expanded image storage is unknown; scratch capacity cannot be approved.");return new(false,reasons.ToArray());
    }
}
public sealed record ScratchAssessment(string Path,long AvailableBytes,long? RequiredBytes,bool Approved,string Reason);
public static class ScratchPreflight
{
    public static long? RequiredBytes(long sourceBytes,long? expandedBytes)
    {
        if(sourceBytes<0||expandedBytes<0)throw new ArgumentOutOfRangeException(nameof(sourceBytes));return expandedBytes==null?null:checked(3*sourceBytes+expandedBytes.Value+1024L*1024*1024);
    }
    public static ScratchAssessment Inspect(string destination,long sourceBytes,long? expandedBytes)
    {
        if(!System.IO.Path.IsPathFullyQualified(destination)||destination.StartsWith(@"\\",StringComparison.Ordinal))throw new InvalidDataException("Scratch must be a new absolute local directory.");destination=System.IO.Path.GetFullPath(destination);if(destination[2..].Contains(':'))throw new InvalidDataException("Scratch path contains alternate data stream syntax.");Engine.CheckPath(destination);if(Directory.Exists(destination)||File.Exists(destination))throw new IOException("Scratch destination must not already exist.");string? parent=System.IO.Path.GetDirectoryName(destination);if(parent==null||!Directory.Exists(parent))throw new IOException("Scratch parent must exist.");var drive=new DriveInfo(System.IO.Path.GetPathRoot(destination)!);if(!drive.IsReady||drive.DriveType is not(DriveType.Fixed or DriveType.Ram)||!drive.DriveFormat.Equals("NTFS",StringComparison.OrdinalIgnoreCase))throw new IOException("Scratch requires a ready local NTFS volume.");long? required=RequiredBytes(sourceBytes,expandedBytes);return new(destination,drive.AvailableFreeSpace,required,false,required==null?"Expanded image size unknown; servicing blocked.":drive.AvailableFreeSpace<required?"Insufficient capacity; servicing blocked.":"Capacity estimate met; mount worker still unavailable. No directory was created.");
    }
}
