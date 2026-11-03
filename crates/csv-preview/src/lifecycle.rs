// SPDX-License-Identifier: Apache-2.0

//! Library-owned thread lifetimes for the unload contract.
//!
//! A thread still running when Windows unloads the DLL would go on to execute
//! unmapped code. `DllCanUnloadNow` therefore refuses while any token from
//! [`LiveThread::enter`] is held: every thread this library spawns takes one,
//! released on normal exit and on unwind alike.

use std::sync::atomic::{AtomicU32, Ordering};

static LIVE_THREADS: AtomicU32 = AtomicU32::new(0);

/// RAII token held by a running library thread. Construct via
/// [`LiveThread::enter`] before scheduling work; it cannot be cloned. The
/// unit field exists only to make outside construction impossible.
#[allow(dead_code)]
pub struct LiveThread(());

impl LiveThread {
    /// Reserve before scheduling the thread; move ownership into its closure.
    pub fn enter() -> Self {
        LIVE_THREADS.fetch_add(1, Ordering::SeqCst);
        Self(())
    }
}

impl Drop for LiveThread {
    fn drop(&mut self) {
        LIVE_THREADS.fetch_sub(1, Ordering::SeqCst);
    }
}

// Reserve before handing work to the OS: a scheduled-but-not-started thread
// must prevent DllCanUnloadNow just like a running one.
fn counted<F, T>(work: F) -> impl FnOnce() -> T + Send + 'static
where
    F: FnOnce() -> T + Send + 'static,
{
    let alive = LiveThread::enter();
    move || {
        let _alive = alive;
        work()
    }
}
pub fn spawn<F, T>(
    builder: std::thread::Builder,
    work: F,
) -> std::io::Result<std::thread::JoinHandle<T>>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    builder.spawn(counted(work))
}
/// Number of library-owned threads currently running or scheduled.
pub fn live_threads() -> u32 {
    LIVE_THREADS.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_counts_until_it_is_dropped() {
        let token = LiveThread::enter();
        assert!(live_threads() >= 1);
        drop(token);
    }
}

#[cfg(test)]
mod queued_tests {
    use super::*;
    #[test]
    fn queued_work_already_prevents_unload_before_it_runs() {
        let work = counted(|| 42);
        assert!(
            live_threads() >= 1,
            "scheduled DLL work must already own a lifetime token"
        );
        assert_eq!(work(), 42);
    }
}
