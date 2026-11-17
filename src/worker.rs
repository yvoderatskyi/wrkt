use std::thread::ScopedJoinHandle;

use anyhow::{Result, anyhow};

pub fn joined<Outcome>(handle: ScopedJoinHandle<'_, Result<Outcome>>) -> Result<Outcome> {
    handle
        .join()
        .unwrap_or_else(|_| Err(anyhow!("the worker thread panicked")))
}
