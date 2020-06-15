use crate::{
    ffi::{CPath, CToR},
    Error, Level, SentryString, Value,
};
use once_cell::sync::Lazy;
#[cfg(feature = "test")]
use std::{env, path::PathBuf};
use std::{
    ffi::CString,
    mem,
    os::raw::c_void,
    path::Path,
    ptr,
    sync::{Mutex, RwLock},
};

type EventFunction = fn(Value) -> Value;

static EVENT_FUNCTION: Lazy<Mutex<Option<EventFunction>>> = Lazy::new(|| Mutex::new(None));

extern "C" fn event_function(
    event: sys::Value,
    _hint: *mut c_void,
    _closure: *mut c_void,
) -> sys::Value {
    if let Ok(event_function) = EVENT_FUNCTION.lock() {
        if let Some(event_function) = &*event_function {
            let event_function = event_function(Value::from_raw(event));
            return event_function.take();
        }
    }

    event
}

pub static GLOBAL_LOCK: Lazy<RwLock<bool>> = Lazy::new(|| RwLock::new(false));

/// The Sentry client options.
#[derive(Debug)]
pub struct Options(
    Option<*mut sys::Options>,
    #[cfg(feature = "test")] Option<SentryString>,
);

unsafe impl Send for Options {}
unsafe impl Sync for Options {}

impl Default for Options {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Options {
    fn drop(&mut self) {
        if let Some(option) = self.0.take() {
            unsafe { sys::options_free(option) };
        }
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
        let mut options = Self(
            Some(unsafe { sys::options_new() }),
            #[cfg(feature = "test")]
            None,
        );

        #[cfg(feature = "test")]
        {
            // will be set up properly for us inside those functions
            options.set_database_path(".sentry-native");
            options.set_handler_path("");
            options.set_dsn("");
        }

        options
    }

    fn as_ref(&self) -> *const sys::Options {
        self.0.expect("use after free")
    }

    fn as_mut(&mut self) -> *mut sys::Options {
        self.0.expect("use after free")
    }

    fn take(mut self) -> *mut sys::Options {
        self.0.take().expect("use after free")
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
    ///     // do something with the value
    ///     value
    /// });
    /// options.init()?;
    /// # Ok(()) }
    /// ```
    pub fn set_before_send(&mut self, fun: EventFunction) {
        {
            let mut event_function = EVENT_FUNCTION.lock().expect("`Mutex` poisoned somehow");
            *event_function = Some(fun);
        }

        unsafe {
            sys::options_set_before_send(self.as_mut(), Some(event_function), ptr::null_mut())
        };
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
            self.1 = Some(dsn.into());
            SentryString::from(
                &env::var("SENTRY_DSN")
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
        return self.1.clone();
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
    pub fn add_attachment<S: Into<SentryString>, P: AsRef<Path>>(&mut self, name: S, path: P) {
        let name: CString = name.into().into();
        let path = path.as_ref().to_os_vec();

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
    pub fn set_handler_path<P: AsRef<Path>>(
        &mut self,
        #[cfg(not(feature = "test"))] path: P,
        #[cfg(feature = "test")] _path: P,
    ) {
        #[cfg(feature = "test")]
        let path: &dyn AsRef<Path> = &PathBuf::from(env::var_os("HANDLER").unwrap());
        let path = path.as_ref().to_os_vec();

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
    pub fn set_database_path<P: AsRef<Path>>(&mut self, path: P) {
        #[cfg(feature = "test")]
        let path: &dyn AsRef<Path> = &PathBuf::from(env::var_os("OUT_DIR").unwrap()).join(path);
        let path = path.as_ref().to_os_vec();

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

    /// Initializes the Sentry SDK with the specified options.
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
    pub fn init(self) -> Result<Shutdown, Error> {
        let options = self.take();

        {
            let mut lock = GLOBAL_LOCK.write().expect("global lock poisoned");

            if *lock {
                panic!("already initialized before!")
            }

            match unsafe { sys::init(options) } {
                0 => {
                    *lock = true;
                    mem::drop(lock);
                    // workaround: send/set any form of data to make sure the error appears on
                    // Sentry
                    crate::set_level(Level::Error);

                    Ok(Shutdown)
                }
                _ => Err(Error::Init),
            }
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
