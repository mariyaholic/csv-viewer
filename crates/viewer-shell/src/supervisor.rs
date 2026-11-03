// SPDX-License-Identifier: Apache-2.0
//! Desktop-only supervisor. Existing format worker wire protocols remain unchanged.
use crate::Table;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc::{sync_channel, Receiver, SyncSender, TryRecvError},
    Arc, Mutex,
};
use std::time::Duration;
pub type Inspect =
    fn(&mut File, &Path, &mut dyn Write, &mut dyn Read, &AtomicBool) -> Result<Table, String>;
#[derive(Clone, Copy)]
pub struct Adapter {
    pub title: &'static str,
    pub worker: &'static str,
    pub csv_spool: bool,
    pub inspect: Inspect,
}
struct OwnedChild {
    inner: Child,
    #[cfg(windows)]
    _job: windows_job::Job,
}
impl std::ops::Deref for OwnedChild {
    type Target = Child;
    fn deref(&self) -> &Child {
        &self.inner
    }
}
impl std::ops::DerefMut for OwnedChild {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.inner
    }
}
#[cfg(windows)]
mod windows_job {
    use std::{
        ffi::c_void,
        mem::{size_of, zeroed},
        os::windows::io::AsRawHandle,
        process::Child,
    };
    #[repr(C)]
    struct Basic {
        user: i64,
        kernel: i64,
        flags: u32,
        min: usize,
        max: usize,
        active: u32,
        affinity: usize,
        priority: u32,
        scheduling: u32,
    }
    #[repr(C)]
    struct Extended {
        basic: Basic,
        io: [u64; 6],
        process_memory: usize,
        job_memory: usize,
        peak_process: usize,
        peak_job: usize,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn CreateJobObjectW(attributes: *const c_void, name: *const u16) -> isize;
        fn SetInformationJobObject(job: isize, class: u32, data: *const c_void, length: u32)
            -> i32;
        fn AssignProcessToJobObject(job: isize, process: isize) -> i32;
        fn CloseHandle(handle: isize) -> i32;
    }
    pub struct Job(isize);
    impl Job {
        pub fn attach(child: &Child) -> Result<Self, String> {
            unsafe {
                let job = Self(CreateJobObjectW(std::ptr::null(), std::ptr::null()));
                if job.0 == 0 {
                    return Err("Cannot create worker ownership job".into());
                }
                let mut info: Extended = zeroed();
                info.basic.flags = 0x2000;
                if SetInformationJobObject(
                    job.0,
                    9,
                    (&info as *const Extended).cast(),
                    size_of::<Extended>() as u32,
                ) == 0
                    || AssignProcessToJobObject(job.0, child.as_raw_handle() as isize) == 0
                {
                    return Err("Cannot attach worker kill-on-close job; operation refused".into());
                }
                Ok(job)
            }
        }
    }
    impl Drop for Job {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}
static ACTIVE: AtomicUsize = AtomicUsize::new(0);
struct ActiveSlot;
impl Drop for ActiveSlot {
    fn drop(&mut self) {
        ACTIVE.fetch_sub(1, Ordering::SeqCst);
    }
}
pub const MAX_SOURCE_BYTES: u64 = 64 * 1024 * 1024;
#[derive(PartialEq, Eq)]
enum Phase {
    Running,
    Cancelled,
    Delivered,
}
enum Signal {
    Cancel,
    Finished,
}
pub struct Task {
    rx: Receiver<Result<Table, String>>,
    signal: SyncSender<Signal>,
    cancel: Arc<AtomicBool>,
    child: Arc<Mutex<Option<OwnedChild>>>,
    phase: Phase,
}
impl Task {
    pub fn start(adapter: Adapter, path: PathBuf, worker: PathBuf) -> Self {
        Self::with_deadline(adapter, path, worker, Duration::from_secs(120))
    }
    fn with_deadline(adapter: Adapter, path: PathBuf, worker: PathBuf, timeout: Duration) -> Self {
        Self::with_query(adapter, path, worker, timeout, None)
    }
    pub fn browse(adapter: Adapter, path: PathBuf, request: crate::browse::Request) -> Self {
        match std::env::current_exe() {
            Ok(worker) => Self::with_query(
                adapter,
                path,
                worker,
                Duration::from_secs(120),
                Some(request),
            ),
            Err(_) => Self::start(adapter, path, PathBuf::from("missing-desktop-worker")),
        }
    }
    fn with_query(
        adapter: Adapter,
        path: PathBuf,
        worker: PathBuf,
        timeout: Duration,
        request: Option<crate::browse::Request>,
    ) -> Self {
        let (tx, rx) = sync_channel(1);
        let (signal, deadline) = sync_channel(1);
        let finished = signal.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let child = Arc::new(Mutex::new(None::<OwnedChild>));
        let flag = cancel.clone();
        let process = child.clone();
        std::thread::spawn(move || {
            // At most two short-lived operations during cancellation/replacement.
            if ACTIVE.fetch_add(1, Ordering::SeqCst) >= 2 {
                ACTIVE.fetch_sub(1, Ordering::SeqCst);
                let _ = tx.send(Err(
                    "Previous operation is still closing; retry Open.".into()
                ));
                return;
            }
            let slot = ActiveSlot;
            let timer_flag = flag.clone();
            let timer_child = process.clone();
            std::thread::spawn(move || {
                if matches!(
                    deadline.recv_timeout(timeout),
                    Ok(Signal::Cancel) | Err(std::sync::mpsc::RecvTimeoutError::Timeout)
                ) {
                    timer_flag.store(true, Ordering::SeqCst);
                    if let Some(child) = timer_child.lock().unwrap().as_mut() {
                        let _ = child.kill();
                    }
                }
            });
            let spool = tempfile::Builder::new().prefix("viewer-desktop-").tempdir();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match &spool {
                Ok(dir) => run(
                    adapter,
                    &path,
                    &worker,
                    &flag,
                    &process,
                    dir.path(),
                    request.as_ref(),
                ),
                Err(e) => Err(format!("Cannot create private desktop spool: {e}")),
            }))
            .unwrap_or_else(|_| Err("Worker task panicked; operation failed safely.".into()));
            if let Some(mut c) = process.lock().unwrap().take() {
                let _ = c.kill();
                let _ = c.wait();
            }
            drop(spool);
            drop(slot);
            let _ = finished.try_send(Signal::Finished);
            let _ = tx.send(result);
        });
        Self {
            rx,
            signal,
            cancel,
            child,
            phase: Phase::Running,
        }
    }
    pub fn cancel(&mut self) {
        if self.phase != Phase::Running {
            return;
        }
        self.phase = Phase::Cancelled;
        self.cancel.store(true, Ordering::SeqCst);
        // The already-owned watchdog performs termination off the UI thread,
        // even if CreateProcess currently holds the publication mutex.
        let _ = self.signal.try_send(Signal::Cancel);
        if let Ok(mut child) = self.child.try_lock() {
            if let Some(child) = child.as_mut() {
                let _ = child.kill();
            }
        }
    }
    pub fn poll(&mut self) -> Option<Result<Table, String>> {
        if self.phase == Phase::Delivered {
            return None;
        }
        if self.phase == Phase::Cancelled {
            self.phase = Phase::Delivered;
            return Some(Err("Cancelled".into()));
        }
        // Delivery time is not execution time: modal UI can defer polling.
        let received = match self.rx.try_recv() {
            Ok(r) => Some(r),
            Err(TryRecvError::Disconnected) => Some(Err("Worker task stopped unexpectedly".into())),
            Err(TryRecvError::Empty) => None,
        };
        // The watchdog owns the execution deadline. A UI-local clock would
        // incorrectly include scheduling and post-cleanup delivery latency.
        if self.cancel.load(Ordering::SeqCst) {
            self.cancel();
            self.phase = Phase::Delivered;
            return Some(Err(
                "Operation timed out; worker terminated. Retry with a smaller local file.".into(),
            ));
        }
        if received.is_some() {
            self.phase = Phase::Delivered;
        }
        received
    }
    pub fn wait(mut self) -> Result<Table, String> {
        loop {
            if let Some(r) = self.poll() {
                return r;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for Task {
    fn drop(&mut self) {
        self.cancel();
    }
}
pub fn worker_path(adapter: Adapter) -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let name = if cfg!(windows) {
        format!("{}.exe", adapter.worker)
    } else {
        adapter.worker.into()
    };
    Ok(exe
        .parent()
        .ok_or("Executable directory unavailable")?
        .join(name))
}
fn run(
    adapter: Adapter,
    path: &Path,
    worker: &Path,
    cancel: &AtomicBool,
    child: &Mutex<Option<OwnedChild>>,
    spool: &Path,
    request: Option<&crate::browse::Request>,
) -> Result<Table, String> {
    let mut file = File::open(path).map_err(|e| format!("Cannot open file read-only: {e}"))?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err("Choose a regular file, not a directory or device".into());
    }
    if request.is_none() && meta.len() > MAX_SOURCE_BYTES {
        return Err("This first desktop slice accepts files up to 64 MiB; existing CLI/preview limits are unchanged.".into());
    }
    if !worker.is_file() {
        return Err(format!(
            "Missing companion worker: {}. Keep the GUI and its matching worker together.",
            worker.display()
        ));
    }
    let mut cmd = Command::new(worker);
    if let Some(request) = request {
        request.validate()?;
        if meta.len() > 16 * 1024 * 1024 * 1024 {
            return Err("Desktop file limit: 16 GiB".into());
        }
        if request
            .expected
            .as_ref()
            .is_some_and(|s| *s != crate::browse::Stamp::from_metadata(&meta))
        {
            return Err("Source changed; Reload before paging/searching".into());
        }
        cmd.arg("--browse-worker").arg(path);
    } else if adapter.csv_spool {
        cmd.arg("--spool-dir").arg(spool);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd.env("TMPDIR", spool)
        .env("TEMP", spool)
        .env("TMP", spool);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let (mut input, output) = {
        let mut guard = child.lock().unwrap();
        if cancel.load(Ordering::SeqCst) {
            return Err("Cancelled".into());
        }
        let mut process = cmd
            .spawn()
            .map_err(|e| format!("Cannot start matching worker: {e}"))?;
        let pipes = (
            process.stdin.take().ok_or("Worker stdin unavailable")?,
            process.stdout.take().ok_or("Worker stdout unavailable")?,
        );
        #[cfg(windows)]
        let job = match windows_job::Job::attach(&process) {
            Ok(j) => j,
            Err(e) => {
                let _ = process.kill();
                let _ = process.wait();
                return Err(e);
            }
        };
        *guard = Some(OwnedChild {
            inner: process,
            #[cfg(windows)]
            _job: job,
        });
        pipes
    };
    let mut reader = BufReader::new(output);
    let result = if let Some(request) = request {
        crate::browse::write(&mut input, request)?;
        let result: Result<Table, String> = crate::browse::read(&mut reader)?;
        let table = result?;
        if table
            .page
            .as_ref()
            .is_none_or(|p| p.stamp != crate::browse::Stamp::from_metadata(&meta))
        {
            return Err("Source changed before worker opened it; Reload".into());
        }
        Ok(table)
    } else {
        (adapter.inspect)(&mut file, spool, &mut input, &mut reader, cancel)
    };
    if cancel.load(Ordering::SeqCst) {
        return Err("Cancelled".into());
    }
    // Detect ordinary concurrent changes; this is not a cryptographic snapshot.
    let after = file.metadata().map_err(|e| e.to_string())?;
    if after.len() != meta.len() || after.modified().ok() != meta.modified().ok() {
        return Err("Source changed while reading; reload before using these results.".into());
    }
    result.map(Table::bounded)
}
pub fn copy_cancelled(
    reader: &mut dyn Read,
    writer: &mut dyn Write,
    cancel: &AtomicBool,
) -> Result<u64, String> {
    let mut buffer = [0u8; 65536];
    let mut bytes = 0;
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err("Cancelled".into());
        }
        let n = reader.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            return Ok(bytes);
        }
        bytes += n as u64;
        if bytes > MAX_SOURCE_BYTES {
            return Err("Source grew beyond the desktop transfer limit".into());
        }
        writer.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    static PROCESS_TESTS: Mutex<()> = Mutex::new(());
    fn queued_task() -> Task {
        let (tx, rx) = sync_channel(1);
        tx.send(Ok(Table {
            coverage: "finished in time".into(),
            ..Table::default()
        }))
        .unwrap();
        Task {
            rx,
            signal: sync_channel(1).0,
            cancel: Arc::new(AtomicBool::new(false)),
            child: Arc::new(Mutex::new(None)),
            phase: Phase::Running,
        }
    }
    #[test]
    fn completion_delivery_gap_is_not_a_new_execution_timeout() {
        let mut task = queued_task();
        let (tx, rx) = sync_channel(1);
        task.rx = rx;
        assert!(
            task.poll().is_none(),
            "a completed watchdog does not time out delivery latency"
        );
        tx.send(Ok(Table::default())).unwrap();
        assert!(task.poll().unwrap().is_ok());
    }
    #[test]
    fn completed_result_survives_delayed_ui_poll_after_modal_help() {
        let mut task = queued_task();
        assert_eq!(task.poll().unwrap().unwrap().coverage, "finished in time");
        assert!(task.poll().is_none());
    }
    #[test]
    fn explicit_cancel_invalidates_an_already_queued_success() {
        let mut task = queued_task();
        task.cancel();
        assert!(task.poll().unwrap().is_err());
        assert!(task.poll().is_none());
    }
    #[test]
    fn cancel_does_not_block_the_ui_on_worker_publication_mutex() {
        let mut task = queued_task();
        let child = task.child.clone();
        let guard = child.lock().unwrap();
        let (tx, rx) = sync_channel(1);
        let h = std::thread::spawn(move || {
            task.cancel();
            tx.send(()).unwrap();
        });
        let quick = rx.recv_timeout(Duration::from_millis(200)).is_ok();
        drop(guard);
        h.join().unwrap();
        assert!(
            quick,
            "Cancel must not wait for CreateProcess/child ownership publication"
        );
    }
    fn panicking_adapter(
        _: &mut File,
        _: &Path,
        _: &mut dyn Write,
        _: &mut dyn Read,
        _: &AtomicBool,
    ) -> Result<Table, String> {
        panic!("adapter failed during transfer");
    }
    #[test]
    fn adapter_panic_releases_operation_capacity_and_reaps_its_child() {
        let _serial = PROCESS_TESTS.lock().unwrap();
        let before = ACTIVE.load(Ordering::SeqCst);
        let f = tempfile::NamedTempFile::new().unwrap();
        let task = Task::start(
            Adapter {
                title: "test",
                worker: "shell",
                csv_spool: false,
                inspect: panicking_adapter,
            },
            f.path().into(),
            blocking_worker(),
        );
        let process = task.child.clone();
        assert!(task.wait().is_err());
        let leaked = process.lock().unwrap().is_some();
        // Explicit test cleanup on the broken baseline prevents an orphan shell.
        if let Some(mut c) = process.lock().unwrap().take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        assert!(!leaked, "a terminal task must not leave a child behind");
        assert_eq!(ACTIVE.load(Ordering::SeqCst), before);
    }

    fn wait_for_child(task: &Task) {
        let until = Instant::now() + Duration::from_secs(2);
        loop {
            if task.child.lock().unwrap().is_some() {
                return;
            }
            assert!(Instant::now() < until, "test child never started");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn blocked(
        _file: &mut File,
        _path: &Path,
        _input: &mut dyn Write,
        output: &mut dyn Read,
        _cancel: &AtomicBool,
    ) -> Result<Table, String> {
        let mut bytes = Vec::new();
        output
            .take(4096)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        Err("Worker closed".into())
    }
    fn blocking_worker() -> PathBuf {
        if cfg!(windows) {
            PathBuf::from(std::env::var_os("WINDIR").unwrap_or("C:\\windows".into()))
                .join("system32/cmd.exe")
        } else {
            PathBuf::from("/bin/sh")
        }
    }
    #[test]
    fn missing_worker_returns_actionable_error() {
        let _serial = PROCESS_TESTS.lock().unwrap();
        let f = tempfile::NamedTempFile::new().unwrap();
        let task = Task::start(
            Adapter {
                title: "test",
                worker: "missing",
                csv_spool: false,
                inspect: blocked,
            },
            f.path().into(),
            f.path().with_extension("missing"),
        );
        assert!(task
            .wait()
            .unwrap_err()
            .contains("Missing companion worker"));
    }
    #[test]
    fn independent_watchdog_breaks_blocked_worker_io_without_ui_polling() {
        let _serial = PROCESS_TESTS.lock().unwrap();
        let f = tempfile::NamedTempFile::new().unwrap();
        let mut task = Task::with_deadline(
            Adapter {
                title: "test",
                worker: "shell",
                csv_spool: false,
                inspect: blocked,
            },
            f.path().into(),
            blocking_worker(),
            Duration::from_millis(100),
        );
        std::thread::sleep(Duration::from_millis(300));
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(result) = task.poll() {
                assert!(result.is_err());
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    #[test]
    fn cancellation_breaks_blocked_worker_io() {
        let _serial = PROCESS_TESTS.lock().unwrap();
        let f = tempfile::NamedTempFile::new().unwrap();
        let mut task = Task::start(
            Adapter {
                title: "test",
                worker: "shell",
                csv_spool: false,
                inspect: blocked,
            },
            f.path().into(),
            blocking_worker(),
        );
        wait_for_child(&task);
        task.cancel();
        assert!(task.wait().is_err());
    }

    #[test]
    fn cancelled_copy_writes_nothing() {
        let mut out = Vec::new();
        let flag = AtomicBool::new(true);
        assert!(copy_cancelled(&mut &b"abc"[..], &mut out, &flag).is_err());
        assert!(out.is_empty());
    }
    #[test]
    fn copies_exact_bytes() {
        let mut out = Vec::new();
        assert_eq!(
            copy_cancelled(&mut &b"001,abc\n"[..], &mut out, &AtomicBool::new(false)).unwrap(),
            8
        );
        assert_eq!(out, b"001,abc\n");
    }
}
