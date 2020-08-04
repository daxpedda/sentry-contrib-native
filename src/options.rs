//! Sentry options implementation.

use crate::{
    before_send, logger, transport, BeforeSend, BeforeSendData, CPath, CToR, Error, Level, Message,
    RToC, Transport, TransportState, BEFORE_SEND, LOGGER,
};
#[cfg(doc)]
use crate::{set_user_consent, shutdown, Consent, Event};
use once_cell::sync::Lazy;
#[cfg(feature = "test")]
use std::env;
#[cfg(doc)]
use std::process::abort;
use std::{
    fmt::{Debug, Formatter, Result as FmtResult},
    mem,
    path::PathBuf,
    sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

/// Global lock for the following purposes:
/// - Prevent [`Options::init`] from being called twice.
/// - Fix some null-dereferncing bugs in `sentry-native` that can happen when
///   shutdown is called while other functions are still accessing options.
///   Hopefully this will be fixed upstream in the future, see
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
///
/// # Examples
/// ```
/// # use sentry_contrib_native::Options;
/// # fn main() -> anyhow::Result<()> {
/// let _shutdown = Options::new().init()?;
/// # Ok(()) }
/// ```
pub struct Options {
    /// Raw Sentry options.
    raw: Option<Ownership>,
    /// Storing a fake DSN to make documentation tests and examples work.
    #[cfg(feature = "test")]
    dsn: Option<String>,
    /// Storing [`Options::set_before_send`] data to save it globally on
    /// [`Options::init`] and properly deallocate it on [`shutdown`].
    before_send: Option<BeforeSendData>,
}

/// Represents the ownership status of [`Options`].
#[derive(Clone, Copy, Debug)]
pub enum Ownership {
    /// [`Options`] is owned.
    Owned(*mut sys::Options),
    /// [`Options`] is borrowed.
    Borrowed(*const sys::Options),
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
        let mut debug = fmt.debug_struct("Options");
        debug.field("raw", &self.raw);
        #[cfg(feature = "test")]
        debug.field("dsn", &self.dsn);
        debug
            .field(
                "before_send",
                if self.before_send.is_some() {
                    &"Some"
                } else {
                    &"None"
                },
            )
            .finish()
    }
}

impl Drop for Options {
    fn drop(&mut self) {
        if let Some(Ownership::Owned(options)) = self.raw.take() {
            unsafe { sys::options_free(options) }
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
    /// Creates new Sentry client options.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// let mut options = Options::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::from_sys(Ownership::Owned(unsafe { sys::options_new() }))
    }

    /// Creates new [`Options`] from a [`sys::Options`] wrapped in
    /// [`Ownership`].
    pub(crate) fn from_sys(options: Ownership) -> Self {
        #[cfg_attr(not(feature = "test"), allow(unused_mut))]
        let mut options = Self {
            raw: Some(options),
            #[cfg(feature = "test")]
            dsn: None,
            before_send: None,
        };

        #[cfg(feature = "test")]
        {
            if let Some(Ownership::Owned(_)) = options.raw {
                // will be set up properly for us inside those functions
                options.set_database_path(".sentry-native");
                options.set_handler_path("");
                options.set_dsn(
                    &env::var("SENTRY_DSN")
                        .expect("tests require a valid `SENTRY_DSN` environment variable"),
                )
            }
        }

        options
    }

    /// Yields a pointer to [`sys::Options`], ownership is retained.
    fn as_ref(&self) -> *const sys::Options {
        match self.raw.expect("use after free") {
            Ownership::Owned(options) => options,
            Ownership::Borrowed(options) => options,
        }
    }

    /// Yields a mutable pointer to [`sys::Options`], ownership is retained.
    fn as_mut(&mut self) -> *mut sys::Options {
        if let Ownership::Owned(options) = self.raw.expect("use after free") {
            options
        } else {
            unreachable!("can't mutably borrow `Options`")
        }
    }

    /// Sets a custom transport. This only affects events sent through
    /// [`Event::capture`], not the crash handler.
    ///
    /// The `startup` parameter is a function that serves as a one-time
    /// initialization event for your [`Transport`], it takes a
    /// [`&Options`](Options) and has to return a [`Transport`].
    ///
    /// # Notes
    /// Unwinding panics of functions in `transport` will be cought and
    /// [`abort`] will be called if any occured.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{Options, RawEnvelope};
    /// let mut options = Options::new();
    /// options.set_transport(|_| {
    ///     |envelope: RawEnvelope| println!("Event to be sent: {:?}", envelope.event())
    /// });
    /// ```
    /// See [`Transport`] for a more detailed documentation.
    pub fn set_transport<
        S: (FnOnce(&Self) -> T) + 'static + Send + Sync,
        T: Into<Box<T>> + Transport,
    >(
        &mut self,
        startup: S,
    ) {
        let startup = TransportState::Startup(Box::new(|options: &Self| startup(options).into()));
        let startup = Box::into_raw(Box::new(Some(startup)));

        unsafe {
            let transport = sys::transport_new(Some(transport::send));
            sys::transport_set_state(transport, startup.cast());
            sys::transport_set_startup_func(transport, Some(transport::startup));
            sys::transport_set_shutdown_func(transport, Some(transport::shutdown));
            sys::options_set_transport(self.as_mut(), transport)
        }
    }

    /// Sets a callback that is triggered before sending an event through
    /// [`Event::capture`].
    ///
    /// # Notes
    /// Unwinding panics of functions in `before_send` will be cought and
    /// [`abort`] will be called if any occured.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// let mut options = Options::new();
    /// options.set_before_send(|mut value| {
    ///     // do something with the value and then return it
    ///     value
    /// });
    /// ```
    pub fn set_before_send<B: Into<Box<B>> + BeforeSend>(&mut self, before_send: B) {
        let fun = Box::into_raw(Box::<Box<dyn BeforeSend>>::new(before_send.into()));
        self.before_send = Some(unsafe { Box::from_raw(fun) });

        unsafe {
            sys::options_set_before_send(self.as_mut(), Some(before_send::before_send), fun.cast())
        }
    }

    /// Sets the DSN.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// let mut options = Options::new();
    /// options.set_dsn("yourdsn.com");
    /// ```
    pub fn set_dsn<S: Into<String>>(&mut self, dsn: S) {
        #[cfg(feature = "test")]
        let dsn = {
            self.dsn = Some(dsn.into());
            env::var("SENTRY_DSN")
                .expect("tests require a valid `SENTRY_DSN` environment variable")
                .into_cstring()
        };
        #[cfg(not(feature = "test"))]
        let dsn = dsn.into().into_cstring();
        unsafe { sys::options_set_dsn(self.as_mut(), dsn.as_ptr()) }
    }

    /// Gets the DSN.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// let mut options = Options::new();
    /// options.set_dsn("yourdsn.com");
    ///
    /// assert_eq!(Some("yourdsn.com"), options.dsn());
    /// ```
    #[must_use]
    pub fn dsn(&self) -> Option<&str> {
        #[cfg(feature = "test")]
        if let Some(Ownership::Owned(_)) = self.raw {
            return self.dsn.as_deref();
        }

        unsafe { sys::options_get_dsn(self.as_ref()).as_str() }
    }

    /// Sets the sample rate, which should be a [`f64`] between `0.0` and `1.0`.
    /// Sentry will randomly discard any event that is captured using [`Event`]
    /// when a sample rate < 1.0 is set.
    ///
    /// # Errors
    /// Fails with [`Error::SampleRateRange`] if `sample_rate` is smaller than
    /// `0.0` or bigger than `1.0`.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// let mut options = Options::new();
    /// options.set_sample_rate(0.5);
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
    /// options.set_sample_rate(0.5)?;
    ///
    /// assert_eq!(0.5, options.sample_rate());
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
    /// let mut options = Options::new();
    /// options.set_release("1.0");
    /// ```
    pub fn set_release<S: Into<String>>(&mut self, release: S) {
        let release = release.into().into_cstring();
        unsafe { sys::options_set_release(self.as_mut(), release.as_ptr()) }
    }

    /// Gets the release.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// let mut options = Options::new();
    /// options.set_release("1.0");
    ///
    /// assert_eq!(Some("1.0"), options.release());
    /// ```
    #[must_use]
    pub fn release(&self) -> Option<&str> {
        unsafe { sys::options_get_release(self.as_ref()).as_str() }
    }

    /// Sets the environment.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// let mut options = Options::new();
    /// options.set_environment("production");
    /// ```
    pub fn set_environment<S: Into<String>>(&mut self, environment: S) {
        let environment = environment.into().into_cstring();
        unsafe { sys::options_set_environment(self.as_mut(), environment.as_ptr()) }
    }

    /// Gets the environment.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// let mut options = Options::new();
    /// options.set_environment("production");
    ///
    /// assert_eq!(Some("production"), options.environment());
    /// ```
    #[must_use]
    pub fn environment(&self) -> Option<&str> {
        unsafe { sys::options_get_environment(self.as_ref()).as_str() }
    }

    /// Sets the distribution.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// let mut options = Options::new();
    /// options.set_distribution("release-pgo");
    /// ```
    pub fn set_distribution<S: Into<String>>(&mut self, distribution: S) {
        let distribution = distribution.into().into_cstring();
        unsafe { sys::options_set_dist(self.as_mut(), distribution.as_ptr()) }
    }

    /// Gets the distribution.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// let mut options = Options::new();
    /// options.set_distribution("release-pgo");
    ///
    /// assert_eq!(Some("release-pgo"), options.distribution());
    /// ```
    #[must_use]
    pub fn distribution(&self) -> Option<&str> {
        unsafe { sys::options_get_dist(self.as_ref()).as_str() }
    }

    /// Configures the http proxy.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// let mut options = Options::new();
    /// options.set_http_proxy("1.1.1.1");
    /// ```
    pub fn set_http_proxy<S: Into<String>>(&mut self, proxy: S) {
        let proxy = proxy.into().into_cstring();
        unsafe { sys::options_set_http_proxy(self.as_mut(), proxy.as_ptr()) }
    }

    /// Returns the configured http proxy.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// let mut options = Options::new();
    /// options.set_http_proxy("1.1.1.1");
    ///
    /// assert_eq!(Some("1.1.1.1"), options.http_proxy());
    /// ```
    #[must_use]
    pub fn http_proxy(&self) -> Option<&str> {
        unsafe { sys::options_get_http_proxy(self.as_ref()).as_str() }
    }

    /// Configures the path to a file containing SSL certificates for
    /// verification.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// let mut options = Options::new();
    /// options.set_ca_certs("certs.pem");
    /// ```
    pub fn set_ca_certs<S: Into<String>>(&mut self, path: S) {
        let path = path.into().into_cstring();
        unsafe { sys::options_set_ca_certs(self.as_mut(), path.as_ptr()) }
    }

    /// Returns the configured path for CA certificates.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// let mut options = Options::new();
    /// options.set_ca_certs("certs.pem");
    ///
    /// assert_eq!(Some("certs.pem"), options.ca_certs());
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
    /// let mut options = Options::new();
    /// options.set_debug(true);
    /// ```
    pub fn set_debug(&mut self, debug: bool) {
        let debug = debug.into();
        unsafe { sys::options_set_debug(self.as_mut(), debug) }
    }

    /// Returns the current value of the debug flag.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// let mut options = Options::new();
    /// options.set_debug(true);
    ///
    /// assert!(options.debug());
    /// ```
    #[must_use]
    pub fn debug(&self) -> bool {
        match unsafe { sys::options_get_debug(self.as_ref()) } {
            0 => false,
            1 => true,
            error => unreachable!("{} couldn't be converted to a bool", error),
        }
    }

    /// Sets a callback that is used for logging purposes when
    /// [`Options::debug`] is `true`.
    ///
    /// # Notes
    /// Unwinding panics in `logger` will be cought and [`abort`]
    /// will be called if any occured.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{Level, Options};
    /// # use std::iter::FromIterator;
    /// let mut options = Options::new();
    /// options.set_debug(true);
    /// options.set_logger(|level, message| {
    ///     println!("[{}]: {}", level, message);
    /// });
    /// ```
    pub fn set_logger<L: Into<Box<L>> + Fn(Level, Message) + 'static + Send + Sync>(
        &mut self,
        logger: L,
    ) {
        *LOGGER.write().expect("failed to set `LOGGER`") = Some(logger.into());
        unsafe {
            sys::options_set_logger(self.as_mut(), Some(logger::logger), std::ptr::null_mut())
        }
    }

    /// Enables or disabled user consent requirements for uploads.
    ///
    /// This disables uploads until the user has given the consent to the SDK.
    /// Consent itself is given with [`set_user_consent`] and [`Consent::Given`]
    /// or revoked with [`Consent::Revoked`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// let mut options = Options::new();
    /// options.set_require_user_consent(true);
    /// ```
    pub fn set_require_user_consent(&mut self, val: bool) {
        let val = val.into();
        unsafe { sys::options_set_require_user_consent(self.as_mut(), val) }
    }

    /// Returns `true` if user consent is required.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// let mut options = Options::new();
    /// options.set_require_user_consent(true);
    ///
    /// assert!(options.require_user_consent());
    /// ```
    #[must_use]
    pub fn require_user_consent(&self) -> bool {
        match unsafe { sys::options_get_require_user_consent(self.as_ref()) } {
            0 => false,
            1 => true,
            error => unreachable!("{} couldn't be converted to a bool", error),
        }
    }

    /// Enables or disables on-device symbolication of stack traces.
    ///
    /// This feature can have a performance impact, and is enabled by default on
    /// Android. It is usually only needed when it is not possible to provide
    /// debug information files for system libraries which are needed for
    /// serverside symbolication.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// let mut options = Options::new();
    /// options.set_symbolize_stacktraces(true);
    /// ```
    pub fn set_symbolize_stacktraces(&mut self, val: bool) {
        let val = val.into();
        unsafe { sys::options_set_symbolize_stacktraces(self.as_mut(), val) }
    }

    /// Returns `true` if on-device symbolication of stack traces is enabled.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// let mut options = Options::new();
    /// options.set_symbolize_stacktraces(true);
    ///
    /// assert!(options.symbolize_stacktraces());
    /// ```
    #[must_use]
    pub fn symbolize_stacktraces(&self) -> bool {
        match unsafe { sys::options_get_symbolize_stacktraces(self.as_ref()) } {
            0 => false,
            1 => true,
            error => unreachable!("{} couldn't be converted to a bool", error),
        }
    }

    /// Adds a new attachment to be sent along.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// let mut options = Options::new();
    /// options.add_attachment("server.log");
    /// ```
    pub fn add_attachment<P: Into<PathBuf>>(&mut self, path: P) {
        let path = path.into().into_os_vec();

        #[cfg(windows)]
        unsafe {
            sys::options_add_attachmentw(self.as_mut(), path.as_ptr())
        };
        #[cfg(not(windows))]
        unsafe {
            sys::options_add_attachment(self.as_mut(), path.as_ptr())
        }
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
    /// let mut options = Options::new();
    /// options.set_handler_path("crashpad_handler");
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
        }
    }

    /// Sets the path to the Sentry database directory.
    ///
    /// Sentry will use this path to persist user consent, sessions, and other
    /// artifacts in case of a crash. This will also be used by the crashpad
    /// backend if it is configured.
    ///
    /// The path defaults to `.sentry-native` in the current working directory,
    /// will be created if it does not exist, and will be resolved to an
    /// absolute path inside of [`Options::init`].
    ///
    /// It is recommended that library users set an explicit absolute path,
    /// depending on their apps runtime directory.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// let mut options = Options::new();
    /// options.set_database_path(".sentry-native");
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
    /// let mut options = Options::new();
    /// options.set_system_crash_reporter(true);
    /// ```
    pub fn set_system_crash_reporter(&mut self, enabled: bool) {
        let enabled = enabled.into();
        unsafe { sys::options_set_system_crash_reporter_enabled(self.as_mut(), enabled) }
    }

    /// Initializes the Sentry SDK with the specified options. Make sure to
    /// capture the resulting [`Shutdown`], this makes sure to automatically
    /// call [`shutdown`] when it drops.
    ///
    /// # Errors
    /// Fails with [`Error::Init`] if Sentry couldn't initialize - should only
    /// occur in these situations:
    /// - Fails to create database directory.
    /// - Fails to lock database directory.
    ///
    /// # Panics
    /// Panics if `init` was already called once before.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Options;
    /// # fn main() -> anyhow::Result<()> {
    /// let _shutdown = Options::new().init()?;
    /// # Ok(()) }
    /// ```
    pub fn init(mut self) -> Result<Shutdown, Error> {
        let mut lock = global_write();

        // make sure we aren't initializing Sentry twice
        if *lock {
            panic!("already initialized Sentry once")
        }

        // disolve `Options`, `sys::init` is going to take ownership now
        let options = if let Ownership::Owned(options) = self.raw.take().expect("use after free") {
            options
        } else {
            unreachable!("can't mutably borrow `Options`")
        };

        match unsafe { sys::init(options) } {
            0 => {
                *lock = true;

                // store `before_send` data so we can deallocate it later
                if let Some(before_send) = self.before_send.take() {
                    BEFORE_SEND
                        .set(Mutex::new(Some(before_send)))
                        .map_err(|_| ())
                        .expect("`BEFORE_SEND` was set once before");
                }

                drop(lock);

                Ok(Shutdown)
            }
            _ => Err(Error::Init),
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
#[derive(Clone, Debug, Hash, Eq, Ord, PartialEq, PartialOrd)]
pub struct Shutdown;

impl Drop for Shutdown {
    fn drop(&mut self) {
        crate::shutdown();
    }
}

impl Shutdown {
    /// Disable automatic shutdown. Call [`shutdown`] manually to force
    /// transports to flush out before the program exits.
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
        drop(self)
    }
}

#[test]
fn options() -> anyhow::Result<()> {
    use crate::{RawEnvelope, Value};

    struct CustomTransport;

    impl CustomTransport {
        const fn new(_: &Options) -> Self {
            Self
        }
    }

    impl Transport for CustomTransport {
        fn send(&self, _: RawEnvelope) {}
    }

    struct Filter;

    impl BeforeSend for Filter {
        fn before_send(&self, value: Value) -> Value {
            value
        }
    }

    let mut options = Options::new();

    options.set_transport(|_| |_| {});
    options.set_transport(CustomTransport::new);

    options.set_before_send(|value| value);
    options.set_before_send(Filter);

    options.set_dsn("yourdsn.com");
    assert_eq!(Some("yourdsn.com"), options.dsn());

    let sample_rate = 0.5;
    options.set_sample_rate(sample_rate)?;
    #[allow(clippy::float_cmp)]
    {
        assert_eq!(sample_rate, options.sample_rate());
    }

    options.set_release("1.0");
    assert_eq!(Some("1.0"), options.release());

    options.set_environment("production");
    assert_eq!(Some("production"), options.environment());

    options.set_distribution("release-pgo");
    assert_eq!(Some("release-pgo"), options.distribution());

    options.set_http_proxy("1.1.1.1");
    assert_eq!(Some("1.1.1.1"), options.http_proxy());

    options.set_ca_certs("certs.pem");
    assert_eq!(Some("certs.pem"), options.ca_certs());

    options.set_debug(true);
    assert!(options.debug());

    options.set_logger(|_, _| ());

    options.set_require_user_consent(true);
    assert!(options.require_user_consent());

    options.set_symbolize_stacktraces(true);
    assert!(options.symbolize_stacktraces());

    options.add_attachment("server.log");

    options.set_handler_path("crashpad_handler");

    options.set_database_path(".sentry-native");

    options.set_system_crash_reporter(true);

    Ok(())
}

#[cfg(test)]
#[rusty_fork::test_fork(timeout_ms = 60000)]
#[should_panic]
fn options_fail() -> anyhow::Result<()> {
    Options::new().init()?;
    Options::new().init()?;

    Ok(())
}

#[cfg(test)]
#[rusty_fork::test_fork(timeout_ms = 60000)]
fn threaded_stress() -> anyhow::Result<()> {
    use crate::test;
    use std::{convert::TryFrom, sync::Arc, thread};

    #[allow(clippy::type_complexity)]
    fn spawns(tests: Vec<fn(Arc<RwLock<Options>>, usize)>) -> Options {
        let options = Arc::new(RwLock::new(Options::new()));

        let mut spawns = Vec::with_capacity(tests.len());
        for test in tests {
            let options = Arc::clone(&options);

            let handle = thread::spawn(move || {
                let mut handles = Vec::with_capacity(100);

                for index in 0..100 {
                    let options = Arc::clone(&options);

                    handles.push(thread::spawn(move || test(options, index)))
                }

                handles
            });
            spawns.push(handle)
        }

        for spawn in spawns {
            for handle in spawn.join().unwrap() {
                handle.join().unwrap()
            }
        }

        Arc::try_unwrap(options).unwrap().into_inner().unwrap()
    }

    test::set_hook();

    let options = spawns(vec![
        |options, _| options.write().unwrap().set_before_send(|value| value),
        |options, index| options.write().unwrap().set_dsn(index.to_string()),
        |options, _| println!("{:?}", options.read().unwrap().dsn()),
        |options, index| {
            let sample_rate = f64::from(u32::try_from(index).unwrap()) / 100.;
            options
                .write()
                .unwrap()
                .set_sample_rate(sample_rate)
                .unwrap()
        },
        |options, _| println!("{:?}", options.read().unwrap().sample_rate()),
        |options, index| options.write().unwrap().set_release(index.to_string()),
        |options, _| println!("{:?}", options.read().unwrap().release()),
        |options, index| options.write().unwrap().set_environment(index.to_string()),
        |options, _| println!("{:?}", options.read().unwrap().environment()),
        |options, index| options.write().unwrap().set_distribution(index.to_string()),
        |options, _| println!("{:?}", options.read().unwrap().distribution()),
        |options, index| options.write().unwrap().set_http_proxy(index.to_string()),
        |options, _| println!("{:?}", options.read().unwrap().http_proxy()),
        |options, index| options.write().unwrap().set_ca_certs(index.to_string()),
        |options, _| println!("{:?}", options.read().unwrap().ca_certs()),
        |options, index| {
            options.write().unwrap().set_debug(match index % 2 {
                0 => false,
                1 => true,
                _ => unreachable!(),
            })
        },
        |options, _| println!("{:?}", options.read().unwrap().debug()),
        |options, index| {
            options
                .write()
                .unwrap()
                .set_require_user_consent(match index % 2 {
                    0 => false,
                    1 => true,
                    _ => unreachable!(),
                })
        },
        |options, _| println!("{:?}", options.write().unwrap().require_user_consent()),
        |options, index| {
            options
                .write()
                .unwrap()
                .set_symbolize_stacktraces(match index % 2 {
                    0 => false,
                    1 => true,
                    _ => unreachable!(),
                })
        },
        |options, _| println!("{:?}", options.read().unwrap().symbolize_stacktraces()),
        |options, index| options.write().unwrap().add_attachment(index.to_string()),
        |options, index| options.write().unwrap().set_handler_path(index.to_string()),
        |options, index| {
            options
                .write()
                .unwrap()
                .set_database_path(index.to_string())
        },
        |options, index| {
            options
                .write()
                .unwrap()
                .set_system_crash_reporter(match index % 2 {
                    0 => false,
                    1 => true,
                    _ => unreachable!(),
                })
        },
    ]);

    options.init()?.shutdown();

    test::verify_panics();

    Ok(())
}

#[cfg(test)]
#[rusty_fork::test_fork(timeout_ms = 60000)]
fn sync() -> anyhow::Result<()> {
    use crate::test;
    use anyhow::{anyhow, Result};
    use std::{sync::Arc, thread};

    test::set_hook();

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

    thread::spawn(move || -> Result<Shutdown> {
        Arc::try_unwrap(options)
            .map_err(|_| anyhow!("failed to unwrap arc"))?
            .init()
            .map_err(Into::into)
    })
    .join()
    .unwrap()?
    .shutdown();

    test::verify_panics();

    Ok(())
}
