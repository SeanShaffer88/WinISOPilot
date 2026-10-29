using System.Diagnostics;
using System.Runtime.CompilerServices;
namespace ImageGuide;
public sealed record JobProgress(string Phase,string Message,long? Completed=null,long? Total=null,bool IsPaused=false,bool CancellationPending=false);
public sealed class JobControl:IDisposable
{
    public Guid RunId {get;}=Guid.NewGuid();
    readonly object sync=new();bool pause,cancel,paused,disposed;
    public bool PauseRequested {get{lock(sync)return pause;}}
    public bool CancellationRequested {get{lock(sync)return cancel;}}
    public bool IsPaused {get{lock(sync)return paused;}}
    public void RequestPause(){lock(sync){if(!disposed&&!cancel)pause=true;}}
    public void Resume(){lock(sync){pause=false;Monitor.PulseAll(sync);}}
    public void RequestCancel(){lock(sync){cancel=true;pause=false;Monitor.PulseAll(sync);}}
    public void Checkpoint(string phase,IProgress<JobProgress>? progress=null,long? completed=null,long? total=null,bool throttleWorking=false)
    {
        while(true){
            lock(sync){if(cancel){paused=false;Publish(progress,new(phase,"Cancellation acknowledged at a safe checkpoint.",completed,total,false,true));throw new OperationCanceledException("Inspection canceled at a safe checkpoint.");}if(!pause){paused=false;break;}paused=true;}
            Publish(progress,new(phase,"Paused at a safe checkpoint. Resume or cancel to continue.",completed,total,true,false));
            lock(sync){if(pause&&!cancel)Monitor.Wait(sync);}
        }
        Publish(progress,new(phase,"Working",completed,total),throttleWorking);
    }
    sealed class ProgressState{public bool Published;public string? Phase;public bool Paused,Canceled;public long Timestamp;}
    static readonly ConditionalWeakTable<IProgress<JobProgress>,ProgressState> progressStates=new();
    public static void Publish(IProgress<JobProgress>? progress,JobProgress value,bool repeated=false)
    {
        if(progress==null)return;var state=progressStates.GetValue(progress,_=>new ProgressState());
        lock(state){long now=Stopwatch.GetTimestamp();bool transition=!state.Published||state.Phase!=value.Phase||state.Paused!=value.IsPaused||state.Canceled!=value.CancellationPending;
            if(repeated&&!transition&&now-state.Timestamp<Stopwatch.Frequency/10)return;
            state.Published=true;state.Phase=value.Phase;state.Paused=value.IsPaused;state.Canceled=value.CancellationPending;state.Timestamp=now;
        }
        // Never invoke consumer code while holding the throttle gate.
        try{progress.Report(value);}catch(Exception){/* UI notification failures must never interrupt native lifetime or integrity work. */}
    }
    public void FinalCheckpoint(string phase,IProgress<JobProgress>? progress=null)
    {
        bool canceled;lock(sync){pause=false;paused=false;canceled=cancel;Monitor.PulseAll(sync);}
        if(canceled){Publish(progress,new(phase,"Cancellation acknowledged after final integrity verification.",CancellationPending:true));throw new OperationCanceledException("Inspection canceled after final integrity verification.");}
        Publish(progress,new(phase,"Finalizing verified results; this phase does not pause."));
    }
    public void Dispose(){lock(sync){if(disposed)return;disposed=true;cancel=true;pause=false;Monitor.PulseAll(sync);}}
}
