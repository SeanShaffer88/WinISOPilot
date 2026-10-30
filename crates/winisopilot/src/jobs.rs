//! Background jobs: run engine work off the UI thread.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;
use wip_core::inventory::Inventory;
use wip_engine::{BuildReport, EngineError, Event, Job, SourceInfo, Workspace};

pub enum Outcome {
    Inspected(SourceInfo),
    Prepared(Box<Workspace>, Box<Inventory>),
    Built(BuildReport),
    Discarded,
    LeftoverCleaned(String),
    Disks(Vec<wip_core::usb::Disk>),
    UsbWritten(wip_engine::UsbReport),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Inspect,
    Prepare,
    Build,
    Discard,
    Leftover,
    ListDisks,
    WriteUsb,
}

impl Kind {
    pub fn title(self) -> &'static str {
        match self {
            Kind::Inspect => "Reading the ISO",
            Kind::Prepare => "Preparing the edition",
            Kind::Build => "Building your Windows",
            Kind::Discard => "Discarding the open image",
            Kind::Leftover => "Cleaning up an earlier session",
            Kind::ListDisks => "Looking for USB drives",
            Kind::WriteUsb => "Writing the USB drive",
        }
    }

    /// Whether Cancel is offered. Discards always run to completion.
    pub fn cancellable(self) -> bool {
        matches!(self, Kind::Inspect | Kind::Prepare | Kind::Build | Kind::WriteUsb)
    }
}

pub struct Running {
    pub kind: Kind,
    rx: Receiver<Event>,
    cancel: Arc<AtomicBool>,
    handle: Option<JoinHandle<Result<Outcome, EngineError>>>,
    pub stage: (u32, u32, String),
    pub progress: Option<f32>,
    pub started: Instant,
    pub cancel_requested: bool,
}

impl Running {
    pub fn spawn<F>(kind: Kind, f: F) -> Running
    where
        F: FnOnce(&Job) -> Result<Outcome, EngineError> + Send + 'static,
    {
        let (tx, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let job = Job::new(tx, cancel.clone());
        let handle = std::thread::spawn(move || f(&job));
        Running {
            kind,
            rx,
            cancel,
            handle: Some(handle),
            stage: (0, 0, kind.title().to_string()),
            progress: None,
            started: Instant::now(),
            cancel_requested: false,
        }
    }

    pub fn request_cancel(&mut self) {
        self.cancel_requested = true;
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Drains events; returns log lines.
    pub fn poll(&mut self) -> Vec<String> {
        let mut lines = Vec::new();
        while let Ok(ev) = self.rx.try_recv() {
            match ev {
                Event::Stage(n, t, label) => {
                    self.stage = (n, t, label);
                    self.progress = None;
                }
                Event::Progress(p) => self.progress = p,
                Event::Log(l) => lines.push(l),
            }
        }
        lines
    }

    pub fn finished(&self) -> bool {
        self.handle.as_ref().is_none_or(|h| h.is_finished())
    }

    pub fn take_result(&mut self) -> Result<Outcome, EngineError> {
        match self.handle.take().map(|h| h.join()) {
            Some(Ok(r)) => r,
            Some(Err(_)) => Err(EngineError::new("WIP-E900", "The background task crashed. See the log for details.")),
            None => Err(EngineError::new("WIP-E901", "No result.")),
        }
    }
}
