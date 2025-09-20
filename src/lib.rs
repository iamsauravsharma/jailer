//! Crate for creating sandbox environments to perform actions in isolation.
//!
//! This crate provides two main types:
//! - [`Jailer`]: A simple sandbox that changes the current working directory
//!   into a temporary one. When dropped or closed, it restores the original
//!   directory and cleans up the temporary space.
//! - [`EnvJailer`]: Extends [`Jailer`] by also managing environment variables,
//!   allowing preservation of selected variables while clearing others on exit.
//!
//! Both types are thread-safe and ensure only one instance runs at a time via
//! a global mutex.

use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
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

/// [`Jailer`] struct which creates a jail environment.
///
/// [`Jailer`] creates a temporary directory and changes the current working
/// directory to it. On drop or manual close, it restores the original working
/// directory and deletes the temporary directory.
///
/// It uses a global mutex to ensure only one `Jailer` is active at a time
/// across threads.
pub struct Jailer {
    temp_directory: Option<TempDir>,
    original_directory: PathBuf,
    _lock: ArcMutexGuard<RawMutex, ()>,
    is_closed: bool,
}

impl Jailer {
    /// Create a new [`Jailer`].
    ///
    /// This will:
    /// - Lock globally to prevent concurrent instances.
    /// - Create a temporary directory.
    /// - Change the current directory to that temp dir.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The temporary directory cannot be created.
    /// - Changing the current directory fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jailer::Jailer;
    ///
    /// // Capture the original working directory
    /// let original_directory = std::env::current_dir().unwrap();
    ///
    /// let mut jailer = Jailer::new().unwrap();
    ///
    /// // We're now inside the jail
    /// let inside_jailer_directory = std::env::current_dir().unwrap();
    /// assert_ne!(inside_jailer_directory, original_directory);
    ///
    /// // Do some action in jailer...
    ///
    /// // Close the jail explicitly
    /// jailer.close().unwrap();
    ///
    /// // Back to original directory
    /// let after_jailer_directory = std::env::current_dir().unwrap();
    /// assert_eq!(after_jailer_directory, original_directory);
    /// assert_ne!(inside_jailer_directory, after_jailer_directory);
    /// ```
    pub fn new() -> Result<Self, std::io::Error> {
        let lock = initialize_or_get_mutex().lock_arc();
        let temp_dir = TempDir::new()?;
        let original_directory = std::env::current_dir()?;
        std::env::set_current_dir(&temp_dir)?;
        Ok(Self {
            temp_directory: Some(temp_dir),
            original_directory,
            _lock: lock,
            is_closed: false,
        })
    }

    /// Get a reference to the original directory
    ///
    /// This returns the directory that was active when the [`Jailer`] was
    /// created
    ///
    /// # Example
    ///
    /// ```rust
    /// use jailer::Jailer;
    ///
    /// let original_directory = std::env::current_dir().unwrap();
    /// let jailer = Jailer::new().unwrap();
    ///
    /// assert_eq!(jailer.original_directory(), &original_directory);
    /// jailer.close().unwrap();
    /// ```
    #[must_use]
    pub fn original_directory(&self) -> &PathBuf {
        &self.original_directory
    }

    /// Closes the [`Jailer`] and performs cleanup.
    ///
    /// This method:
    /// - Changes back to the original working directory.
    /// - Deletes the temporary directory.
    /// - Releases the global lock.
    ///
    /// It consumes `self`, so the jailer cannot be used afterward.
    ///
    /// # Errors
    ///
    /// Returns an error if changing the directory or deleting the temp dir
    /// fails.
    pub fn close(mut self) -> Result<(), std::io::Error> {
        std::env::set_current_dir(self.original_directory.as_path())?;
        if let Some(temp) = self.temp_directory.take() {
            temp.close()?;
        }
        self.is_closed = true;
        Ok(())
    }
}

impl Drop for Jailer {
    fn drop(&mut self) {
        if !self.is_closed {
            std::env::set_current_dir(self.original_directory.as_path()).ok();
            if let Some(temp) = self.temp_directory.take() {
                temp.close().ok();
            }
        }
    }
}

/// [`EnvJailer`] struct which creates a jail environment with environment
/// variable management.
///
/// [`EnvJailer`] wraps [`Jailer`] and adds support for preserving specific
/// environment variables. On drop or close, it:
/// - Removes all environment variables not marked as preserved.
/// - Restores original values for preserved keys.
/// - Reverts to the original working directory and cleans up the temp dir.
///
/// # Safety
///
/// Environment variable manipulation via [`std::env::set_var`] and
/// [`std::env::remove_var`] is considered unsafe due to potential race
/// conditions in multi-threaded programs. Therefore, methods like
/// [`EnvJailer::close`] are marked as `unsafe`.
pub struct EnvJailer {
    jailer: Option<Jailer>,
    original_directory: PathBuf,
    original_env_vars_os: HashMap<OsString, OsString>,
    preserved_env_vars_os: HashSet<OsString>,
}

impl EnvJailer {
    /// Create a new [`EnvJailer`].
    ///
    /// This captures the current environment variables and working directory,
    /// then initializes a new [`Jailer`].
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying [`Jailer`] cannot be created.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jailer::EnvJailer;
    ///
    /// // Capture the original working directory
    /// let original_directory = std::env::current_dir().unwrap();
    ///
    /// let mut env_jailer = EnvJailer::new().unwrap();
    ///
    /// // Do some action in jailer...
    ///
    /// // Close the jail explicitly (unsafe due to env var changes)
    /// unsafe {
    ///     env_jailer.close().unwrap();
    /// }
    ///
    /// // Back to original directory
    /// assert_eq!(std::env::current_dir().unwrap(), original_directory);
    /// ```
    pub fn new() -> Result<Self, std::io::Error> {
        let original_env_vars_os = std::env::vars_os().collect();
        let jailer = Jailer::new()?;
        let original_dir = jailer.original_directory().clone();

        Ok(Self {
            jailer: Some(jailer),
            original_directory: original_dir,
            original_env_vars_os,
            preserved_env_vars_os: HashSet::new(),
        })
    }

    /// Get a reference to the original directory
    ///
    /// This returns the directory that was active when the [`EnvJailer`] was
    /// created
    ///
    /// # Example
    ///
    /// ```rust
    /// use jailer::EnvJailer;
    ///
    /// let original_directory = std::env::current_dir().unwrap();
    /// let env_jailer = EnvJailer::new().unwrap();
    /// assert_eq!(env_jailer.original_directory(), &original_directory);
    /// unsafe {
    ///     env_jailer.close().unwrap();
    /// }
    /// ```
    #[must_use]
    pub fn original_directory(&self) -> &PathBuf {
        &self.original_directory
    }

    /// Mark an environment variable as preserved.
    ///
    /// When the jailer is closed or dropped, this key will retain its current
    /// value instead of being removed or reset to the original value.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jailer::EnvJailer;
    ///
    /// unsafe {
    ///     std::env::set_var("KEY", "VALUE");
    ///     std::env::set_var("ANOTHER_KEY", "VALUE");
    /// }
    ///
    /// let mut env_jailer = EnvJailer::new().unwrap();
    ///
    /// assert_eq!(std::env::var("KEY"), Ok("VALUE".to_string()));
    ///
    /// unsafe {
    ///     std::env::set_var("KEY2", "VALUE2");
    /// }
    /// env_jailer.set_preserved_env("KEY");
    ///
    /// unsafe {
    ///     std::env::set_var("KEY", "VALUE2");
    ///     std::env::set_var("ANOTHER_KEY", "ANOTHER_VAL");
    /// }
    ///
    /// assert_eq!(std::env::var("KEY"), Ok("VALUE2".to_string()));
    /// assert_eq!(std::env::var("ANOTHER_KEY"), Ok("ANOTHER_VAL".to_string()));
    ///
    /// unsafe {
    ///     env_jailer.close().unwrap();
    /// }
    ///
    /// assert_eq!(std::env::var("KEY"), Ok("VALUE2".to_string()));
    /// assert_eq!(std::env::var("ANOTHER_KEY"), Ok("VALUE".to_string()));
    /// assert!(std::env::var("KEY2").is_err());
    /// ```
    pub fn set_preserved_env<K>(&mut self, key: K)
    where
        K: AsRef<OsStr>,
    {
        self.preserved_env_vars_os
            .insert(key.as_ref().to_os_string());
    }

    /// Remove an environment variable from the preserved list.
    ///
    /// Note: This does *not* remove the current environment variable.
    /// To remove it, call [`std::env::remove_var`] manually.
    ///
    /// # Example
    ///
    /// ```rust
    /// use jailer::EnvJailer;
    ///
    /// let mut env_jailer = EnvJailer::new().unwrap();
    ///
    /// unsafe {
    ///     std::env::set_var("KEY", "VALUE");
    /// }
    ///
    /// env_jailer.set_preserved_env("KEY");
    /// assert_eq!(std::env::var("KEY"), Ok("VALUE".to_string()));
    /// env_jailer.remove_preserved_env("KEY");
    ///
    /// unsafe {
    ///     env_jailer.close().unwrap();
    /// }
    ///
    /// assert!(std::env::var("KEY").is_err());
    /// ```
    pub fn remove_preserved_env<K>(&mut self, key: K)
    where
        K: AsRef<OsStr>,
    {
        self.preserved_env_vars_os.remove(key.as_ref());
    }

    /// Returns a reference to the map of original environment variables.
    ///
    /// These are the environment variables present when the [`EnvJailer`] was
    /// created. Any variables added during the session are not included
    /// here.
    #[must_use]
    pub fn original_env_vars_os(&self) -> &HashMap<OsString, OsString> {
        &self.original_env_vars_os
    }

    /// Returns a reference to the set of preserved environment variable names.
    #[must_use]
    pub fn preserved_env_vars_os(&self) -> &HashSet<OsString> {
        &self.preserved_env_vars_os
    }

    unsafe fn revert_env_vars(&self) {
        for key in std::env::vars_os().collect::<HashMap<_, _>>().keys() {
            if !self.preserved_env_vars_os.contains(key) {
                unsafe {
                    std::env::remove_var(key);
                }
            }
        }
        for (key, value) in &self.original_env_vars_os {
            if !self.preserved_env_vars_os.contains(key) {
                unsafe {
                    std::env::set_var(key, value);
                }
            }
        }
    }

    /// Closes the [`EnvJailer`] and performs cleanup.
    ///
    /// This method:
    /// - Reverts environment variables to their original state (except
    ///   preserved ones).
    /// - Closes the underlying [`Jailer`] (restoring directory and removing
    ///   temp dir).
    ///
    /// It consumes `self`, so the jailer cannot be used afterward.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying [`Jailer::close`] fails.
    ///
    /// # Safety
    ///
    /// This function calls [`std::env::remove_var`] and [`std::env::set_var`],
    /// which are unsafe due to possible data races in concurrent contexts.
    pub unsafe fn close(mut self) -> Result<(), std::io::Error> {
        unsafe {
            self.revert_env_vars();
        }
        if let Some(jailer) = self.jailer.take() {
            jailer.close()?;
        }
        Ok(())
    }
}

impl Drop for EnvJailer {
    fn drop(&mut self) {
        if self.jailer.is_some() {
            unsafe {
                self.revert_env_vars();
            }
        }
    }
}

/// Run a closure inside a [`Jailer`] environment.
///
/// This function creates a [`Jailer`], runs the provided closure, and ensures
/// the jail is closed afterward.
///
/// # Errors
///
/// Returns any error from creating or closing the [`Jailer`], or from the
/// closure itself.
///
/// # Example
///
/// ```rust
/// use jailer::with_jailer;
///
/// with_jailer(|_jailer| {
///     // Do something in the jailed directory
///     Ok(())
/// })
/// .unwrap();
/// ```
pub fn with_jailer<F, T>(f: F) -> Result<T, Box<dyn std::error::Error>>
where
    F: FnOnce(&Jailer) -> Result<T, Box<dyn std::error::Error>>,
{
    let jailer = Jailer::new()?;
    let result = f(&jailer);
    jailer.close()?;
    result
}

/// Run a closure inside a [`Jailer`] environment asynchronously.
///
/// This function creates a [`Jailer`], runs the provided async closure, and
/// ensures the jail is closed afterward.
///
/// # Errors
///
/// Returns any error from creating or closing the [`Jailer`], or from the
/// closure itself.
///
/// # Example
///
/// ```rust
/// use jailer::with_jailer_async;
/// # tokio_test::block_on(async {
/// with_jailer_async(|_jailer| {
///     async {
///         // Do something in the jailed directory
///         Ok(())
///     }
/// })
/// .await
/// .unwrap();
/// # })
/// ```
pub async fn with_jailer_async<F, T, Fut>(f: F) -> Result<T, Box<dyn std::error::Error>>
where
    F: FnOnce(&Jailer) -> Fut,
    Fut: Future<Output = Result<T, Box<dyn std::error::Error>>>,
{
    let jailer = Jailer::new()?;
    let result = f(&jailer).await;
    jailer.close()?;
    result
}

/// Run a closure inside a [`EnvJailer`] environment.
///
/// This function creates a [`EnvJailer`], runs the provided closure, and
/// ensures the jail is closed afterward.
///
/// # Errors
///
/// Returns any error from creating or closing the [`EnvJailer`], or from the
/// closure itself.
///
/// # Example
///
/// ```rust
/// use jailer::with_env_jailer;
/// unsafe {
///     with_env_jailer(|_env_jailer| {
///         // Do something in the jailed directory
///         Ok(())
///     })
///     .unwrap();
/// }
/// ```
///
/// # Safety
///
/// This function calls [`std::env::remove_var`] and [`std::env::set_var`],
/// which are unsafe due to possible data races in concurrent contexts.
pub unsafe fn with_env_jailer<F, T>(f: F) -> Result<T, Box<dyn std::error::Error>>
where
    F: FnOnce(&Jailer) -> Result<T, Box<dyn std::error::Error>>,
{
    let jailer = Jailer::new()?;
    let result = f(&jailer);
    jailer.close()?;
    result
}

/// Run a closure inside a [`EnvJailer`] environment asynchronously.
///
/// This function creates a [`EnvJailer`], runs the provided async closure, and
/// ensures the jail is closed afterward.
///
/// # Errors
///
/// Returns any error from creating or closing the [`EnvJailer`], or from the
/// closure itself.
///
/// # Example
///
/// ```rust
/// use jailer::with_env_jailer_async;
/// # tokio_test::block_on(async {
/// unsafe {
///     with_env_jailer_async(|_env_jailer| {
///         async {
///             // Do something in the jailed directory
///             Ok(())
///         }
///     })
///     .await
///     .unwrap();
/// }
/// # })
/// ```
///
/// # Safety
///
/// This function calls [`std::env::remove_var`] and [`std::env::set_var`],
/// which are unsafe due to possible data races in concurrent contexts.
pub async unsafe fn with_env_jailer_async<F, Fut, T>(f: F) -> Result<T, Box<dyn std::error::Error>>
where
    F: FnOnce(&Jailer) -> Fut,
    Fut: Future<Output = Result<T, Box<dyn std::error::Error>>>,
{
    let jailer = Jailer::new()?;
    let result = f(&jailer).await;
    jailer.close()?;
    result
}
