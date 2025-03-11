//! Crate for creating sandbox environment and do some action within a sandbox
//! environment

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::ops::Deref;
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

/// [`Jailer`] struct which creates jail.
///
/// [`Jailer`] struct create temp dir and change current directory to temp
/// directory. When [`Jailer`] gets dropped than [`Jailer`] will automatically
/// delete temporary directory contents
pub struct Jailer {
    temp_directory: Option<TempDir>,
    directory: PathBuf,
    original_directory: PathBuf,
    _lock: ArcMutexGuard<RawMutex, ()>,
}

impl Jailer {
    /// Create new [`Jailer`]
    ///
    /// # Errors
    /// if new [`Jailer`] cannot be created
    ///
    /// ```rust
    /// use jailer::Jailer;
    ///
    /// let mut jailer = Jailer::new().unwrap();
    /// // do some action in jailer
    /// jailer.close().unwrap();
    /// ```
    pub fn new() -> Result<Self, std::io::Error> {
        let lock = initialize_or_get_mutex().lock_arc();
        let temp_dir = TempDir::new()?;
        let directory = temp_dir.path().canonicalize()?;
        let original_directory = std::env::current_dir()?;
        std::env::set_current_dir(&temp_dir)?;
        Ok(Self {
            temp_directory: Some(temp_dir),
            directory,
            original_directory,
            _lock: lock,
        })
    }

    /// Returns path of directory for jailer
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// Return path of original directory which was used before jailer directory
    ///
    /// ```rust
    /// use jailer::Jailer;
    ///
    /// let current_directory = std::env::current_dir().unwrap();
    /// let mut jailer = Jailer::new().unwrap();
    /// assert_eq!(jailer.original_directory(), &current_directory);
    /// jailer.close().unwrap();
    /// ```
    #[must_use]
    pub fn original_directory(&self) -> &Path {
        &self.original_directory
    }

    /// Closes a [`Jailer`]
    ///
    /// Although [`Jailer`] removes directory,
    /// It may not remove directory or change current directory which can still
    /// fails and error will be ignored. To detect and handle error due to
    /// change of directory or deletion of temp dir call close manually
    ///
    /// While closing/dropping. Current directory is changed to original
    /// directory and temp dir is closed
    ///
    /// # Errors
    /// When [`Jailer`] cannot be closed properly
    pub fn close(&mut self) -> Result<(), std::io::Error> {
        std::env::set_current_dir(self.original_directory.as_path())?;
        if let Some(temp) = self.temp_directory.take() {
            temp.close()?;
        }
        Ok(())
    }
}

impl Drop for Jailer {
    fn drop(&mut self) {
        std::env::set_current_dir(self.original_directory.as_path()).ok();
        if let Some(temp) = self.temp_directory.take() {
            temp.close().ok();
        }
    }
}

/// [`EnvJailer`] struct which creates jail. [`EnvJailer`] is build on top of
/// [`Jailer`] which also handles environment variable. It is different than
/// [`Jailer`] since environment variable set and unset operation is unsafe
///
/// [`EnvJailer`] reverts to original env variable when it is dropped or closed
pub struct EnvJailer {
    jailer: Jailer,
    original_env_vars_os: HashMap<OsString, OsString>,
    preserved_env_vars_os: HashMap<OsString, OsString>,
}

impl EnvJailer {
    /// Create new [`EnvJailer`]
    ///
    /// # Errors
    /// if new [`EnvJailer`] cannot be created
    ///
    /// ```rust
    /// use jailer::EnvJailer;
    ///
    /// let mut env_jailer = EnvJailer::new().unwrap();
    /// // do some action in jailer
    /// unsafe {
    ///     env_jailer.close().unwrap();
    /// }
    /// ```
    pub fn new() -> Result<Self, std::io::Error> {
        let original_env_vars_os = std::env::vars_os().collect();
        Ok(Self {
            jailer: Jailer::new()?,
            original_env_vars_os,
            preserved_env_vars_os: HashMap::new(),
        })
    }

    /// Set environment variable which will be saved as preserved env this type
    /// of env will not be removed when [`Jailer`] gets dropped.
    ///
    /// # Safety
    ///  Setting environment variable is not safe operation see
    /// [`std::env::set_var`]
    ///
    /// ```rust
    /// use jailer::EnvJailer;
    ///
    /// let mut env_jailer = EnvJailer::new().unwrap();
    /// unsafe {
    ///     env_jailer.set_env("KEY", "VALUE");
    /// }
    /// assert_eq!(std::env::var("KEY"), Ok("VALUE".to_string()));
    /// unsafe {
    ///     std::env::set_var("KEY", "ANOTHER_VALUE");
    /// }
    /// assert_eq!(std::env::var("KEY"), Ok("ANOTHER_VALUE".to_string()));
    /// unsafe {
    ///     env_jailer.close().unwrap();
    /// }
    /// assert_eq!(std::env::var("KEY"), Ok("VALUE".to_string()));
    /// ```
    pub unsafe fn set_env<K, V>(&mut self, key: K, value: V)
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.preserved_env_vars_os
            .insert(key.as_ref().to_os_string(), value.as_ref().to_os_string());
        unsafe {
            std::env::set_var(key, value);
        }
    }

    /// Remove environment variable from preserved env
    ///
    /// This function do not remove current environment variable. To remove
    /// current environment variable you need to manually call
    /// [`std::env::remove_var`].
    /// ```rust
    /// use jailer::EnvJailer;
    ///
    /// let mut env_jailer = EnvJailer::new().unwrap();
    /// unsafe {
    ///     env_jailer.set_env("KEY", "VALUE");
    /// }
    /// assert_eq!(std::env::var("KEY"), Ok("VALUE".to_string()));
    /// env_jailer.remove_preserved_env("KEY");
    /// // value is not removed till now have to be removed manually
    /// assert_eq!(std::env::var("KEY"), Ok("VALUE".to_string()));
    /// unsafe {
    ///     env_jailer.close().unwrap();
    /// }
    /// assert!(std::env::var("KEY").is_err());
    /// ```
    pub fn remove_preserved_env<K>(&mut self, key: K)
    where
        K: AsRef<OsStr>,
    {
        self.preserved_env_vars_os.remove(key.as_ref());
    }

    /// Return hashmap of original env variables
    ///
    /// If any env is added by using set env than those env are not provided in
    /// this response use [`EnvJailer::preserved_env_vars_os`]
    #[must_use]
    pub fn original_env_vars_os(&self) -> &HashMap<OsString, OsString> {
        &self.original_env_vars_os
    }

    /// Return hashmap of preserved env variables
    #[must_use]
    pub fn preserved_env_vars_os(&self) -> &HashMap<OsString, OsString> {
        &self.preserved_env_vars_os
    }

    unsafe fn revert_env_vars(&self) {
        for key in std::env::vars_os().collect::<HashMap<_, _>>().keys() {
            unsafe {
                std::env::remove_var(key);
            }
        }
        for (key, value) in &self.original_env_vars_os {
            unsafe {
                std::env::set_var(key, value);
            }
        }
        for (key, value) in &self.preserved_env_vars_os {
            unsafe {
                std::env::set_var(key, value);
            }
        }
    }

    /// Closes a [`EnvJailer`]
    ///
    /// Although [`EnvJailer`] removes directory and environment variable on
    /// drop, It may not remove directory or change current directory which
    /// can still fails and error will be ignored. To detect and handle
    /// error due to change of directory or deletion of temp dir call close
    /// manually
    ///
    /// While closing/dropping, At first all current environment variables are
    /// removed than original env variables gets added at last preserved env
    /// variables gets added in those order before changing to original
    /// directory and closing temporary directory at last
    ///
    /// # Errors
    /// When [`EnvJailer`] cannot be closed properly
    ///
    ///
    /// # Safety
    ///  Close function calls [`std::env::remove_var`] and [`std::env::set_var`]
    /// function which are both unsafe operation
    pub unsafe fn close(&mut self) -> Result<(), std::io::Error> {
        unsafe { self.revert_env_vars() };
        self.jailer.close()?;
        Ok(())
    }
}

impl Deref for EnvJailer {
    type Target = Jailer;

    fn deref(&self) -> &Self::Target {
        &self.jailer
    }
}

impl Drop for EnvJailer {
    fn drop(&mut self) {
        unsafe {
            self.revert_env_vars();
        }
    }
}
