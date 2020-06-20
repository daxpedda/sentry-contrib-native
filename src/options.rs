//! Sentry options implementation.

use crate::{CPath, CToR, Error, RToC, Value};
use once_cell::sync::{Lazy, OnceCell};
#[cfg(feature = "test")]
use std::{env, ffi::CString};
use std::{
    fmt::{Debug, Formatter, Result as FmtResult},
    mem,
    mem::ManuallyDrop,
    os::raw::c_void,
    path::PathBuf,
    sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

/// Global lock for two purposes:
/// - Prevent [`Options::init`] from being called twice.
/// - Fix some use-after-free bugs in `sentry-native` that can happen when
///   shutdown is called while other functions are still accessing global
///   options. Hopefully this will be fixed upstream in the future, see
///   <https://github.com/getsentry/sentry-native/issues/280>.
static GLOBAL_LOCK: Lazy<RwLock<bool>> = Lazy::new(|| RwLock::new(false));

/// Convenience function to get a read lock on `GLOBAL_LOCK`.
pub fn global_read() -> RwLockReadGuard<'static, bool> {
    GLOBAL_LOCK.read().expect("global lock poisoned")
}

/// Convenience function to get a write lock on `GLOBAL_LOCK`.
pub fn global_write() -> RwLockWriteGuard<'static, bool> {
    GLOBAL_LOCK.write().expect("global lock poisoned")
}

/// The Sentry client options.
pub struct Options {
    /// Raw Sentry options.
    raw: Option<*mut sys::Options>,
    /// Storing a fake database path to make documentation tests and examples
    /// work without polluting the file system.
    #[cfg(feature = "test")]
    database_path: Option<CString>,
    /// Storing [`Options::set_before_send`] data to properly deallocate it
    /// later.
    before_send: Option<Box<Box<dyn BeforeSend>>>,
}

unsafe impl Send for Options {}
unsafe impl Sync for Options {}

impl Default for Options {
    fn default() -> Self {
        Self::new()
    }
}

impl Debug for Options {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> FmtResult {
        fmt.debug_struct("Options").field("raw", &self.raw).finish()
    }
}

impl Drop for Options {
    fn drop(&mut self) {
        if let Some(option) = self.raw.take() {
            unsafe { sys::options_free(option) };
        }
    }
}

impl PartialEq for Options {
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

impl Options {
    /// Crates new Sentry client options.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let options = Options::new();
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn new() -> Self {
        #[cfg_attr(not(feature = "test"), allow(unused_mut))]
        let mut options = Self {
            raw: Some(unsafe { sys::options_new() }),
            #[cfg(feature = "test")]
            database_path: None,
            before_send: None,
        };

        #[cfg(feature = "test")]
        {
            // will be set up properly for us inside those functions
            options.set_database_path(".sentry-native");
            options.set_handler_path("");
            options.set_dsn("");
        }

        options
    }

    /// Yields a pointer to [`sys::Options`], ownership is retained.
    fn as_ref(&self) -> *const sys::Options {
        self.raw.expect("use after free")
    }

    /// Yields a mutable pointer to [`sys::Options`], ownership is retained.
    fn as_mut(&mut self) -> *mut sys::Options {
        self.raw.expect("use after free")
    }

    /// Sets the before send callback.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{BeforeSend, Options, Value};
    /// # use std::error::Error;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.set_before_send(|value| {
    ///     // do something with the value and then return it
    ///     value
    /// });
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    pub fn set_before_send<B: Into<Box<B>> + BeforeSend>(&mut self, fun: B) {
        let fun = Box::into_raw(Box::<Box<dyn BeforeSend>>::new(fun.into()));
        self.before_send = Some(unsafe { Box::from_raw(fun) });

        unsafe { sys::options_set_before_send(self.as_mut(), Some(before_send), fun as _) }
    }

    /// Sets the DSN.
    ///
    /// # Panics
    /// Panics if `dsn` contains any null bytes.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.set_dsn("yourdsn.com");
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    pub fn set_dsn<S: Into<String>>(&mut self, dsn: S) {
        #[cfg(feature = "test")]
        let dsn = {
            self.database_path = Some(dsn.into().into_cstring());
            env::var("SENTRY_DSN")
                .expect("tests require a valid `SENTRY_DSN` environment variable")
                .into_cstring()
        };
        #[cfg(not(feature = "test"))]
        let dsn = dsn.into().into_cstring();
        unsafe { sys::options_set_dsn(self.as_mut(), dsn.as_ptr()) };
    }

    /// Gets the DSN.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.set_dsn("yourdsn.com");
    /// assert_eq!(Some("yourdsn.com"), options.dsn());
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn dsn(&self) -> Option<&str> {
        #[cfg(feature = "test")]
        return self
            .database_path
            .as_ref()
            .map(|database_path| database_path.to_str().expect("invalid UTF-8"));
        #[cfg(not(feature = "test"))]
        unsafe {
            sys::options_get_dsn(self.as_ref()).as_str()
        }
    }

    /// Sets the sample rate, which should be a double between `0.0` and `1.0`.
    /// Sentry will randomly discard any event that is captured using
    /// [`Event`](crate::Event) when a sample rate < 1 is set.
    ///
    /// # Errors
    /// Fails with [`Error::SampleRateRange`] if `sample_rate` is smaller then
    /// `0.0` or bigger then `1.0`.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.set_sample_rate(0.5);
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    pub fn set_sample_rate(&mut self, sample_rate: f64) -> Result<(), Error> {
        if sample_rate >= 0. && sample_rate <= 1. {
            unsafe { sys::options_set_sample_rate(self.as_mut(), sample_rate) };

            Ok(())
        } else {
            Err(Error::SampleRateRange)
        }
    }

    /// Gets the sample rate.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.set_sample_rate(0.5);
    /// assert_eq!(0.5, options.sample_rate());
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn sample_rate(&self) -> f64 {
        unsafe { sys::options_get_sample_rate(self.as_ref()) }
    }

    /// Sets the release.
    ///
    /// # Panics
    /// Panics if `release` contains any null bytes.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.set_release("1.0");
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    pub fn set_release<S: Into<String>>(&mut self, release: S) {
        let release = release.into().into_cstring();
        unsafe { sys::options_set_release(self.as_mut(), release.as_ptr()) };
    }

    /// Gets the release.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.set_release("1.0");
    /// assert_eq!(Some("1.0"), options.release());
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn release(&self) -> Option<&str> {
        unsafe { sys::options_get_release(self.as_ref()).as_str() }
    }

    /// Sets the environment.
    ///
    /// # Panics
    /// Panics if `environment` contains any null bytes.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.set_environment("production");
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    pub fn set_environment<S: Into<String>>(&mut self, environment: S) {
        let environment = environment.into().into_cstring();
        unsafe { sys::options_set_environment(self.as_mut(), environment.as_ptr()) };
    }

    /// Gets the environment.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.set_environment("production");
    /// assert_eq!(Some("production"), options.environment());
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn environment(&self) -> Option<&str> {
        unsafe { sys::options_get_environment(self.as_ref()).as_str() }
    }

    /// Sets the distribution.
    ///
    /// # Panics
    /// Panics if `distribution` contains any null bytes.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.set_distribution("release-pgo");
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    pub fn set_distribution<S: Into<String>>(&mut self, distribution: S) {
        let dist = distribution.into().into_cstring();
        unsafe { sys::options_set_dist(self.as_mut(), dist.as_ptr()) };
    }

    /// Gets the distribution.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.set_distribution("release-pgo");
    /// assert_eq!(Some("release-pgo"), options.distribution());
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn distribution(&self) -> Option<&str> {
        unsafe { sys::options_get_dist(self.as_ref()).as_str() }
    }

    /// Configures the http proxy.
    ///
    /// # Panics
    /// Panics if `proxy` contains any null bytes.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.set_http_proxy("1.1.1.1");
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    pub fn set_http_proxy<S: Into<String>>(&mut self, proxy: S) {
        let proxy = proxy.into().into_cstring();
        unsafe { sys::options_set_http_proxy(self.as_mut(), proxy.as_ptr()) };
    }

    /// Returns the configured http proxy.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.set_http_proxy("1.1.1.1");
    /// assert_eq!(Some("1.1.1.1"), options.http_proxy());
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn http_proxy(&self) -> Option<&str> {
        unsafe { sys::options_get_http_proxy(self.as_ref()).as_str() }
    }

    /// Configures the path to a file containing ssl certificates for
    /// verification.
    ///
    /// # Panics
    /// Panics if `path` contains any null bytes.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.set_ca_certs("certs.pem");
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    pub fn set_ca_certs<S: Into<String>>(&mut self, path: S) {
        let path = path.into().into_cstring();
        unsafe { sys::options_set_ca_certs(self.as_mut(), path.as_ptr()) };
    }

    /// Returns the configured path for ca certificates.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.set_ca_certs("certs.pem");
    /// assert_eq!(Some("certs.pem"), options.ca_certs());
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn ca_certs(&self) -> Option<&str> {
        unsafe { sys::options_get_ca_certs(self.as_ref()).as_str() }
    }

    /// Enables or disables debug printing mode.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.set_debug(true);
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    pub fn set_debug(&mut self, debug: bool) {
        let debug = debug.into();
        unsafe { sys::options_set_debug(self.as_mut(), debug) };
    }

    /// Returns the current value of the debug flag.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.set_debug(true);
    /// assert!(options.debug());
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn debug(&self) -> bool {
        match unsafe { sys::options_get_debug(self.as_ref()) } {
            1 => true,
            _ => false,
        }
    }

    /// Enables or disabled user consent requirements for uploads.
    ///
    /// This disables uploads until the user has given the consent to the SDK.
    /// Consent itself is given with
    /// [`user_consent_give`](crate::user_consent_give) and
    /// [`user_consent_revoke`](crate::user_consent_revoke).
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.set_require_user_consent(true);
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    pub fn set_require_user_consent(&mut self, val: bool) {
        let val = val.into();
        unsafe { sys::options_set_require_user_consent(self.as_mut(), val) }
    }

    /// Returns true if user consent is required.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.set_require_user_consent(true);
    /// assert!(options.require_user_consent());
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn require_user_consent(&self) -> bool {
        match unsafe { sys::options_get_require_user_consent(self.as_ref()) } {
            1 => true,
            _ => false,
        }
    }

    /// Adds a new attachment to be sent along.
    ///
    /// # Panics
    /// - Panics if `name` contains any null bytes.
    /// - Panics if `path` contains any null bytes.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.add_attachment("your_attachment", "server.log");
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    pub fn add_attachment<S: Into<String>, P: Into<PathBuf>>(&mut self, name: S, path: P) {
        let name = name.into().into_cstring();
        let path = path.into().into_os_vec();

        #[cfg(windows)]
        unsafe {
            sys::options_add_attachmentw(self.as_mut(), name.as_ptr(), path.as_ptr())
        };
        #[cfg(not(windows))]
        unsafe {
            sys::options_add_attachment(self.as_mut(), name.as_ptr(), path.as_ptr())
        };
    }

    /// Sets the path to the crashpad handler if the crashpad backend is used.
    ///
    /// The path defaults to the `crashpad_handler`/`crashpad_handler.exe`
    /// executable, depending on platform, which is expected to be present in
    /// the same directory as the app executable.
    ///
    /// It is recommended that library users set an explicit handler path,
    /// depending on the directory/executable structure of their app.
    ///
    /// # Panics
    /// Panics if `path` contains any null bytes.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.set_handler_path("crashpad_handler");
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    #[cfg_attr(
        all(feature = "test", any(windows, target_os = "macos")),
        allow(clippy::needless_pass_by_value)
    )]
    pub fn set_handler_path<P: Into<PathBuf>>(
        &mut self,
        #[cfg(not(all(feature = "test", any(windows, target_os = "macos"))))] path: P,
        #[cfg(all(feature = "test", any(windows, target_os = "macos")))] _path: P,
    ) {
        #[cfg(all(feature = "test", any(windows, target_os = "macos")))]
        let path = PathBuf::from(env!("CRASHPAD_HANDLER")).into_os_vec();
        #[cfg(not(all(feature = "test", any(windows, target_os = "macos"))))]
        let path = path.into().into_os_vec();

        #[cfg(windows)]
        unsafe {
            sys::options_set_handler_pathw(self.as_mut(), path.as_ptr())
        };
        #[cfg(not(windows))]
        unsafe {
            sys::options_set_handler_path(self.as_mut(), path.as_ptr())
        };
    }

    /// Sets the path to the Sentry database directory.
    ///
    /// Sentry will use this path to persist user consent, sessions, and other
    /// artifacts in case of a crash. This will also be used by the crashpad
    /// backend if it is configured.
    ///
    /// The path defaults to `.sentry-native` in the current working directory,
    /// will be created if it does not exist, and will be resolved to an
    /// absolute path inside of `sentry_init`.
    ///
    /// It is recommended that library users set an explicit absolute path,
    /// depending on their apps runtime directory.
    ///
    /// # Panics
    /// Panics if `path` contains any null bytes.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.set_database_path(".sentry-native2");
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    pub fn set_database_path<P: Into<PathBuf>>(&mut self, path: P) {
        #[cfg(feature = "test")]
        let path = PathBuf::from(env!("OUT_DIR"))
            .join(path.into())
            .into_os_vec();
        #[cfg(not(feature = "test"))]
        let path = path.into().into_os_vec();

        #[cfg(windows)]
        unsafe {
            sys::options_set_database_pathw(self.as_mut(), path.as_ptr())
        };
        #[cfg(not(windows))]
        unsafe {
            sys::options_set_database_path(self.as_mut(), path.as_ptr())
        };
    }

    /// Enables forwarding to the system crash reporter. Disabled by default.
    ///
    /// This setting only has an effect when using Crashpad on macOS. If
    /// enabled, Crashpad forwards crashes to the macOS system crash reporter.
    /// Depending on the crash, this may impact the crash time. Even if enabled,
    /// Crashpad may choose not to forward certain crashes.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.set_system_crash_reporter(true);
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    pub fn set_system_crash_reporter(&mut self, enabled: bool) {
        let enabled = enabled.into();
        unsafe { sys::options_set_system_crash_reporter_enabled(self.as_mut(), enabled) }
    }

    /// Initializes the Sentry SDK with the specified options. Make sure to
    /// capture the resulting [`Shutdown`], this makes sure to automatically
    /// call [`shutdown`](crate::shutdown) when it drops.
    ///
    /// # Errors
    /// Fails with [`Error::Init`] if Sentry couldn't initialize - should only
    /// occur in these situations:
    /// - Fails to create database directory.
    /// - Fails to lock database directory.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let options = Options::new();
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    pub fn init(mut self) -> Result<Shutdown, Error> {
        let mut lock = global_write();

        if *lock {
            panic!("already initialized before!")
        }

        match unsafe { sys::init(self.raw.unwrap()) } {
            0 => {
                *lock = true;
                // init has taken ownership now
                self.raw.take().expect("use after free");
                mem::drop(lock);

                // store `before_send` data so we can deallocate it later
                if let Some(before_send) = self.before_send.take() {
                    BEFORE_SEND
                        .set(Mutex::new(Some(before_send)))
                        .map_err(|_| ())
                        .expect("`BEFORE_SEND` was set once before");
                }

                Ok(Shutdown)
            }
            _ => Err(Error::Init(self)),
        }
    }
}

/// Automatically shuts down the Sentry client on drop.
///
/// # Examples
/// ```
/// # use anyhow::Result;
/// # use sentry_contrib_native::{Options, Shutdown};
/// fn main() -> Result<()> {
///     let options = Options::new();
///     let _shutdown: Shutdown = options.init()?;
///
///     // ...
///     // your application code
///     // ...
///
///     Ok(())
///     // Sentry client will automatically shutdown because `Shutdown` is leaving context
/// }
/// ```
#[derive(Debug, Hash, Eq, Ord, PartialEq, PartialOrd)]
pub struct Shutdown;

impl Drop for Shutdown {
    fn drop(&mut self) {
        crate::shutdown();
    }
}

impl Shutdown {
    /// Disable automatic shutdown.
    /// Call [`shutdown`](crate::shutdown) manually to force transports to flush
    /// out before the program exits.
    ///
    /// # Examples
    /// ```
    /// # use anyhow::Result;
    /// # use sentry_contrib_native::{Options, shutdown};
    /// fn main() -> Result<()> {
    ///     sentry_init()?;
    ///
    ///     // ...
    ///     // your application code
    ///     // ...
    ///
    ///     // call shutdown manually to make sure transports flush out
    ///     shutdown();
    ///     Ok(())
    /// }
    ///
    /// fn sentry_init() -> Result<()> {
    ///     let options = Options::new();
    ///     // call forget because we are leaving the context and we don't want to shut down the Sentry client yet
    ///     options.init()?.forget();
    ///     Ok(())
    /// }
    /// ```
    pub fn forget(self) {
        mem::forget(self)
    }

    /// Manually shutdown.
    ///
    /// # Examples
    /// ```
    /// # use anyhow::Result;
    /// # use sentry_contrib_native::Options;
    /// fn main() -> Result<()> {
    ///     let options = Options::new();
    ///     let shutdown = options.init()?;
    ///
    ///     // ...
    ///     // your application code
    ///     // ...
    ///
    ///     // call shutdown manually to make sure transports flush out
    ///     shutdown.shutdown();
    ///     Ok(())
    /// }
    /// ```
    pub fn shutdown(self) {
        mem::drop(self)
    }
}

/// Store [`Options::set_before_send`] data to properly allocate later.
#[allow(clippy::type_complexity)]
pub static BEFORE_SEND: OnceCell<Mutex<Option<Box<Box<dyn BeforeSend>>>>> = OnceCell::new();

/// Trait to help pass data to [`Options::set_before_send`].
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{BeforeSend, Options, Value};
/// # use std::error::Error;
/// # fn main() -> anyhow::Result<()> {
/// struct Filter {
///     filtered: usize,
/// };
///
/// impl BeforeSend for Filter {
///     fn before_send(&mut self, value: Value) -> Value {
///         self.filtered += 1;
///         // do something with the value and then return it
///         value
///     }
/// }
///
/// let mut options = Options::new();
/// options.set_before_send(Filter { filtered: 0 });
/// let _shutdown = options.init()?;
/// # Ok(()) }
/// ```
pub trait BeforeSend: 'static + Send + Sync {
    /// Before send callback.
    fn before_send(&mut self, value: Value) -> Value;
}

impl<T: Fn(Value) -> Value + 'static + Send + Sync> BeforeSend for T {
    fn before_send(&mut self, value: Value) -> Value {
        self(value)
    }
}

/// Function to give [`Options::set_before_send`] which in turn calls user
/// defined one.
extern "C" fn before_send(
    event: sys::Value,
    _hint: *mut c_void,
    closure: *mut c_void,
) -> sys::Value {
    let mut before_send =
        ManuallyDrop::new(unsafe { Box::<Box<dyn BeforeSend>>::from_raw(closure as *mut _) });
    before_send
        .before_send(unsafe { Value::from_raw(event) })
        .take()
}

#[cfg(test)]
mod test {
    use crate::{Options, Shutdown};
    use anyhow::{anyhow, Result};
    use rusty_fork::test_fork;
    use std::{sync::Arc, thread};

    #[test_fork]
    fn send() -> Result<()> {
        let mut options = Options::new();
        options.set_debug(true);

        let _shutdown =
            thread::spawn(move || -> Result<Shutdown> { options.init().map_err(Into::into) })
                .join()
                .unwrap()?;

        Ok(())
    }

    #[test_fork]
    fn sync() -> Result<()> {
        let mut options = Options::new();
        options.set_debug(true);
        let options = Arc::new(options);
        let mut handles = vec![];

        for _ in 0..100 {
            let options = Arc::clone(&options);
            let handle = thread::spawn(move || {
                println!("{}", options.debug());
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let _shutdown = thread::spawn(move || -> Result<Shutdown> {
            Arc::try_unwrap(options)
                .map_err(|_| anyhow!("failed to unwrap arc"))?
                .init()
                .map_err(Into::into)
        })
        .join()
        .unwrap()?;

        Ok(())
    }
}
