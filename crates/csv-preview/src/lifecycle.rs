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
/// [`LiveThread::enter`] inside the thread; it cannot be cloned or built
/// outside, so every token corresponds to exactly one thread lifetime. The
/// unit field exists only to make outside construction impossible.
#[allow(dead_code)]
pub struct LiveThread(());

impl LiveThread {
    /// Take at the very beginning of the thread body.
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

/// Number of library-owned threads currently running.
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
