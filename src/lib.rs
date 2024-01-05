//! Crate for creating sandbox environment and do some action within a sandbox
//! environment

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use parking_lot::lock_api::ArcMutexGuard;
use parking_lot::{Mutex, RawMutex};
use tempfile::TempDir;

/// Static mutex which is wrapped around once lock to be thread safe
static MUTEX: OnceLock<Arc<Mutex<()>>> = OnceLock::new();

/// Initialize or get mutex
fn initialize_or_get_mutex<'a>() -> &'a Arc<Mutex<()>> {
    MUTEX.get_or_init(|| Arc::new(Mutex::new(())))
}

/// Main struct for jailer which holds information related to jail
pub struct Jailer {
    _temp_directory: TempDir,
    current_directory: PathBuf,
    envs_vars_os: HashMap<OsString, OsString>,
    _lock: ArcMutexGuard<RawMutex, ()>,
}

impl Jailer {
    /// Create new jailer
    ///
    /// # Errors
    /// if new jailer cannot be created
    pub fn new() -> Result<Self, std::io::Error> {
        let lock = initialize_or_get_mutex().lock_arc();
        let temp_dir = TempDir::new()?;
        let current_directory = std::env::current_dir()?;
        let vars = std::env::vars_os().collect();
        std::env::set_current_dir(&temp_dir)?;
        Ok(Self {
            _temp_directory: temp_dir,
            current_directory,
            envs_vars_os: vars,
            _lock: lock,
        })
    }
}

impl Drop for Jailer {
    fn drop(&mut self) {
        for key in std::env::vars_os().collect::<HashMap<_, _>>().keys() {
            std::env::remove_var(key);
        }
        for (key, value) in &self.envs_vars_os {
            std::env::set_var(key, value);
        }
        std::env::set_current_dir(self.current_directory.as_path()).ok();
    }
}
