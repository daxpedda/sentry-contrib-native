//! Sentry options implementation.

use crate::{
    ffi::{CPath, CToR},
    Error, SentryString, Value,
};
use once_cell::sync::{Lazy, OnceCell};
#[cfg(feature = "test")]
use std::env;
use std::{
    ffi::CString,
    fmt::{Debug, Formatter, Result as FmtResult},
    mem,
    os::raw::c_void,
    path::PathBuf,
    ptr,
    sync::RwLock,
};

/// Re-usable type to store function for [`Options::set_before_send`].
type BeforeSend = Box<dyn Fn(Value) -> Value + 'static + Send + Sync>;

/// Store function to use for [`Options::set_before_send`] globally because we
/// need to access it inside a `extern "C"` function.
static BEFORE_SEND: OnceCell<BeforeSend> = OnceCell::new();

/// Function to give [`Options::set_before_send`] which in turn calls user
/// defined one.
extern "C" fn before_send(
    event: sys::Value,
    _hint: *mut c_void,
    _closure: *mut c_void,
) -> sys::Value {
    if let Some(before_send) = BEFORE_SEND.get() {
        before_send(Value::from_raw(event)).take()
    } else {
        event
    }
}

/// Global lock for two purposes:
/// - Prevent [`Options::init`] from being called twice.
/// - Fix some use-after-free bugs in `sentry-native` that can happen when
///   shutdown is called while other functions are still accessing global
///   options. Hopefully this will be fixed upstream in the future, see
///   <https://github.com/getsentry/sentry-native/issues/280>.
pub static GLOBAL_LOCK: Lazy<RwLock<bool>> = Lazy::new(|| RwLock::new(false));

/// The Sentry client options.
pub struct Options {
    /// Raw Sentry options.
    raw: Option<*mut sys::Options>,
    /// Storing a fake database path to make documentation tests and examples
    /// work without polluting the file system.
    #[cfg(feature = "test")]
    database_path: Option<SentryString>,
    /// Store function for [`Options::set_before_send`] here temporarily. This
    /// way we can use [`OnceCell`] instead of a [`Mutex`](std::sync::Mutex).
    before_send: Option<BeforeSend>,
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

impl Eq for Options {}

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
    /// # use sentry_contrib_native::Options;
    /// # use std::error::Error;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.set_before_send(|value| {
    ///     // do something with the value and then return it
    ///     value
    /// });
    /// options.init()?;
    /// # Ok(()) }
    /// ```
    pub fn set_before_send<F: Fn(Value) -> Value + 'static + Send + Sync>(&mut self, fun: F) {
        self.before_send = Some(Box::new(fun));

        unsafe { sys::options_set_before_send(self.as_mut(), Some(before_send), ptr::null_mut()) }
    }

    /// Sets the DSN.
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
    pub fn set_dsn<S: Into<SentryString>>(&mut self, dsn: S) {
        #[cfg(feature = "test")]
        let dsn: CString = {
            self.database_path = Some(dsn.into());
            SentryString::from(
                env::var("SENTRY_DSN")
                    .expect("tests require a valid `SENTRY_DSN` environment variable"),
            )
            .into()
        };
        #[cfg(not(feature = "test"))]
        let dsn: CString = dsn.into().into();
        unsafe { sys::options_set_dsn(self.as_mut(), dsn.as_ptr()) };
    }

    /// Gets the DSN.
    ///
    /// # Errors
    /// Fails with [`Error::StrUtf8`] if dsn contains invalid UTF-8.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.set_dsn("yourdsn.com");
    /// assert_eq!("yourdsn.com", options.dsn().unwrap().as_str()?);
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn dsn(&self) -> Option<SentryString> {
        #[cfg(feature = "test")]
        return self.database_path.clone();
        #[cfg(not(feature = "test"))]
        unsafe { sys::options_get_dsn(self.as_ref()) }
            .to_cstring()
            .map(SentryString::from_cstring)
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
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.set_release("1.0");
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    pub fn set_release<S: Into<SentryString>>(&mut self, release: S) {
        let release: CString = release.into().into();
        unsafe { sys::options_set_release(self.as_mut(), release.as_ptr()) };
    }

    /// Gets the release.
    ///
    /// # Errors
    /// Fails with [`Error::StrUtf8`] if release contains invalid UTF-8.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.set_release("1.0");
    /// assert_eq!(Ok("1.0"), options.release().unwrap().as_str());
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn release(&self) -> Option<SentryString> {
        unsafe { sys::options_get_release(self.as_ref()) }
            .to_cstring()
            .map(SentryString::from_cstring)
    }

    /// Sets the environment.
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
    pub fn set_environment<S: Into<SentryString>>(&mut self, environment: S) {
        let environment: CString = environment.into().into();
        unsafe { sys::options_set_environment(self.as_mut(), environment.as_ptr()) };
    }

    /// Gets the environment.
    ///
    /// # Errors
    /// Fails with [`Error::StrUtf8`] if environment contains invalid UTF-8.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.set_environment("production");
    /// assert_eq!(Ok("production"), options.environment().unwrap().as_str());
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn environment(&self) -> Option<SentryString> {
        unsafe { sys::options_get_environment(self.as_ref()) }
            .to_cstring()
            .map(SentryString::from_cstring)
    }

    /// Sets the distribution.
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
    pub fn set_distribution<S: Into<SentryString>>(&mut self, distribution: S) {
        let dist: CString = distribution.into().into();
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
    /// assert_eq!(Ok("release-pgo"), options.distribution().unwrap().as_str());
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn distribution(&self) -> Option<SentryString> {
        unsafe { sys::options_get_dist(self.as_ref()) }
            .to_cstring()
            .map(SentryString::from_cstring)
    }

    /// Configures the http proxy.
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
    pub fn set_http_proxy<S: Into<SentryString>>(&mut self, proxy: S) {
        let proxy: CString = proxy.into().into();
        unsafe { sys::options_set_http_proxy(self.as_mut(), proxy.as_ptr()) };
    }

    /// Returns the configured http proxy.
    ///
    /// # Errors
    /// Fails with [`Error::StrUtf8`] if the http proxy contains invalid UTF-8.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.set_http_proxy("1.1.1.1");
    /// assert_eq!(Ok("1.1.1.1"), options.http_proxy().unwrap().as_str());
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn http_proxy(&self) -> Option<SentryString> {
        unsafe { sys::options_get_http_proxy(self.as_ref()) }
            .to_cstring()
            .map(SentryString::from_cstring)
    }

    /// Configures the path to a file containing ssl certificates for
    /// verification.
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
    pub fn set_ca_certs<S: Into<SentryString>>(&mut self, path: S) {
        let path: CString = path.into().into();
        unsafe { sys::options_set_ca_certs(self.as_mut(), path.as_ptr()) };
    }

    /// Returns the configured path for ca certificates.
    ///
    /// # Errors
    /// Fails with [`Error::StrUtf8`] if the certificate path contains invalid
    /// UTF-8.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.set_ca_certs("certs.pem");
    /// assert_eq!(Ok("certs.pem"), options.ca_certs().unwrap().as_str());
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn ca_certs(&self) -> Option<SentryString> {
        unsafe { sys::options_get_ca_certs(self.as_ref()) }
            .to_cstring()
            .map(SentryString::from_cstring)
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
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut options = Options::new();
    /// options.add_attachment("your_attachment", "server.log");
    /// let _shutdown = options.init()?;
    /// # Ok(()) }
    /// ```
    pub fn add_attachment<S: Into<SentryString>, P: Into<PathBuf>>(&mut self, name: S, path: P) {
        let name: CString = name.into().into();
        let path = path.into().to_os_vec();

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
        let path = PathBuf::from(
            env::var_os("CRASHPAD_HANDLER").expect("failed to find crashpad handler"),
        )
        .to_os_vec();
        #[cfg(not(all(feature = "test", any(windows, target_os = "macos"))))]
        let path = path.into().to_os_vec();

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
        let path = PathBuf::from(env::var_os("OUT_DIR").unwrap())
            .join(path.into())
            .to_os_vec();
        #[cfg(not(feature = "test"))]
        let path = path.into().to_os_vec();

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
        let mut lock = GLOBAL_LOCK.write().expect("global lock poisoned");

        if *lock {
            panic!("already initialized before!")
        }

        match unsafe { sys::init(self.raw.unwrap()) } {
            0 => {
                *lock = true;
                // init has taken ownership now
                self.raw.take().expect("use after free");

                if let Some(before_send) = self.before_send.take() {
                    BEFORE_SEND
                        .set(before_send)
                        .ok()
                        .expect("`BEFORE_SEND` was filled once before");
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
