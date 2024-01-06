//! Crate for creating sandbox environment and do some action within a sandbox
//! environment

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
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

/// `Jailer` struct which creates jail. `Jailer` struct create temp dir and
/// change current directory to temp directory. As well as preserve all
/// environment variable.
///
/// When `Jailer` gets dropped than `Jailer` will automatically delete temporary
/// directory contents as well as revert all environment variables and change
/// current directory to original directory
pub struct Jailer {
    temp_directory: Option<TempDir>,
    directory: PathBuf,
    original_directory: PathBuf,
    envs_vars_os: HashMap<OsString, OsString>,
    _lock: ArcMutexGuard<RawMutex, ()>,
}

impl Jailer {
    /// Create new `Jailer`
    ///
    /// # Errors
    /// if new `Jailer` cannot be created
    pub fn new() -> Result<Self, std::io::Error> {
        let lock = initialize_or_get_mutex().lock_arc();
        let temp_dir = TempDir::new()?;
        let directory = temp_dir.path().canonicalize()?;
        let original_directory = std::env::current_dir()?;
        let envs_vars_os = std::env::vars_os().collect();
        std::env::set_current_dir(&temp_dir)?;
        Ok(Self {
            temp_directory: Some(temp_dir),
            directory,
            original_directory,
            envs_vars_os,
            _lock: lock,
        })
    }

    /// Returns path of directory for jailer
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// Return path of original directory which was used before jailer directory
    #[must_use]
    pub fn original_directory(&self) -> &Path {
        &self.original_directory
    }

    /// Set environment variable which will not be removed when `Jailer` gets
    /// dropped. Returns value which was already present on key if it exists
    /// otherwise returns `None`
    pub fn set_env<K, V>(&mut self, key: K, value: V) -> Option<OsString>
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        let old_value = self
            .envs_vars_os
            .insert(key.as_ref().to_os_string(), value.as_ref().to_os_string());
        std::env::set_var(key, value);
        old_value
    }

    /// Closes a `Jailer`
    ///
    /// Although `Jailer` removes directory and environment variable on drop,
    /// It may not remove directory or change current directory which can still
    /// fails and error will be ignored. To detect and handle error due to
    /// change of directory or deletion of temp dir call close manually
    ///
    /// # Errors
    /// When `Jailer` cannot be closed properly
    pub fn close(self) -> Result<(), std::io::Error> {
        let mut jailer = self;
        for key in std::env::vars_os().collect::<HashMap<_, _>>().keys() {
            std::env::remove_var(key);
        }
        for (key, value) in &jailer.envs_vars_os {
            std::env::set_var(key, value);
        }
        std::env::set_current_dir(jailer.original_directory.as_path())?;
        if let Some(temp) = jailer.temp_directory.take() {
            temp.close()?;
        };
        Ok(())
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
        std::env::set_current_dir(self.original_directory.as_path()).ok();
    }
}
