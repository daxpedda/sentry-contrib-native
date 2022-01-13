//! Sentry custom transport implementation.
//!
//! This can be used to send data to an upstream Sentry service in lieue of the
//! built-in transports provided by the sentry-native library itself.

#[cfg(doc)]
use crate::Event;
use crate::{ffi, Options, Ownership, Value};
use std::{
    mem::ManuallyDrop,
    os::raw::{c_char, c_int, c_void},
    process, slice, thread,
    time::Duration,
};
#[cfg(doc)]
use std::{process::abort, sync::Mutex};
pub use sys::SDK_USER_AGENT;
#[cfg(feature = "transport-custom")]
use ::{
    http::{HeaderMap, HeaderValue, Request as HttpRequest},
    std::{
        convert::{Infallible, TryFrom, TryInto},
        str::FromStr,
    },
    thiserror::Error,
    url::{ParseError, Url},
};

/// Sentry errors.
#[cfg(feature = "transport-custom")]
#[derive(Debug, Error, PartialEq)]
pub enum Error {
    /// Failed to parse DSN URL.
    #[error("failed to parse DSN URL")]
    UrlParse(#[from] ParseError),
    /// DSN doesn't have a http(s) scheme.
    #[error("DSN doesn't have a http(s) scheme")]
    Scheme,
    /// DSN has no username.
    #[error("DSN has no username")]
    Username,
    /// DSN has no project ID.
    #[error("DSN has no project ID")]
    ProjectId,
    /// DSN has no host.
    #[error("DSN has no host")]
    Host,
}

#[cfg(feature = "transport-custom")]
impl From<Infallible> for Error {
    fn from(from: Infallible) -> Self {
        match from {}
    }
}

/// The [`http::Request`] request your [`Transport`] is expected to send.
#[cfg(feature = "transport-custom")]
pub type Request = HttpRequest<Envelope>;

/// The MIME type for Sentry envelopes.
pub const ENVELOPE_MIME: &str = "application/x-sentry-envelope";
/// Version of the Sentry API we can communicate with, AFAICT this is just
/// hardcoded into sentry-native, so ... two can play at that game!
pub const API_VERSION: i8 = 7;

/// The return from [`Transport::shutdown`], which determines if we tell
/// the Sentry SDK if we were able to send all requests to the remote service
/// or not in the allotted time.
#[derive(Copy, Clone, Debug, Hash, Eq, Ord, PartialEq, PartialOrd)]
pub enum Shutdown {
    /// The custom transport was able to send all requests in the allotted time.
    Success,
    /// One or more requests could not be sent in the specified time frame.
    TimedOut,
}

impl Shutdown {
    /// Converts [`Shutdown`] into [`c_int`].
    const fn into_raw(self) -> c_int {
        match self {
            Self::Success => 0,
            Self::TimedOut => 1,
        }
    }
}

/// Trait used to define a custom transport that Sentry can use to send events
/// to a Sentry service.
///
/// # Examples
/// ```
/// # /*
/// #![cfg(feature = "transport-custom")]
///
/// # */
/// # fn main() -> anyhow::Result<()> {
/// # #[cfg(feature = "transport-custom")]
/// # {
/// # use sentry_contrib_native::{Dsn, Event, Options, RawEnvelope, test, Transport};
/// # use std::convert::TryInto;
/// # test::set_hook();
/// use reqwest::blocking::Client;
///
/// struct CustomTransport {
///     dsn: Dsn,
///     client: Client,
/// };
///
/// impl CustomTransport {
///     fn new(options: &Options) -> Result<Self, ()> {
///         Ok(CustomTransport {
///             dsn: options.dsn().and_then(|dsn| Dsn::new(dsn).ok()).ok_or(())?,
///             client: Client::new(),
///         })
///     }
/// }
///
/// impl Transport for CustomTransport {
///     fn send(&self, envelope: RawEnvelope) {
///         let dsn = self.dsn.clone();
///         let client = self.client.clone();
///
///         // in a correct implementation envelopes have to be sent in order for sessions to work
///         std::thread::spawn(move || {
///             let request = envelope
///                 .to_request(dsn)
///                 .map(|body| body.as_bytes().to_vec());
///             client
///                 .execute(request.try_into().unwrap())
///                 .expect("failed to send envelope")
///         });
///     }
/// }
///
/// let dsn = "https://public_key_1234@organization_1234.ingest.sentry.io/project_id_1234";
///
/// let mut options = Options::new();
/// options.set_dsn(dsn);
/// options.set_transport(CustomTransport::new);
/// # let shutdown = options.init()?;
/// # Event::new().capture();
/// # shutdown.shutdown();
/// # test::verify_panics();
/// # } Ok(()) }
/// ```
/// See the
/// [`transport-custom`](https://github.com/daxpedda/sentry-contrib-native/blob/master/examples/custom-transport.rs)
/// example for a more sophisticated implementation.
pub trait Transport: 'static + Send + Sync {
    /// Sends the specified envelope to a Sentry service.
    ///
    /// It is **required** to send envelopes in order for sessions to work
    /// correctly.
    ///
    /// It is **highly** recommended to not block in this method, but rather
    /// to enqueue the worker to another thread.
    fn send(&self, envelope: RawEnvelope);

    /// Shuts down the transport worker. The worker should try to flush all
    /// of the pending requests to Sentry before shutdown. If the worker is
    /// successfully able to empty its queue and shutdown before the specified
    /// timeout duration, it should return [`Shutdown::Success`],
    /// otherwise it should return [`Shutdown::TimedOut`].
    ///
    /// The default implementation will block the thread for `timeout` duration
    /// and always return [`Shutdown::TimedOut`], it has to be adjusted to
    /// work correctly.
    #[must_use]
    #[allow(clippy::boxed_local)]
    fn shutdown(self: Box<Self>, timeout: Duration) -> Shutdown {
        thread::sleep(timeout);
        Shutdown::TimedOut
    }
}

impl<T: Fn(RawEnvelope) + 'static + Send + Sync> Transport for T {
    fn send(&self, envelope: RawEnvelope) {
        self(envelope);
    }
}

/// Type used to store the startup function.
type Startup =
    Box<dyn (FnOnce(&Options) -> Result<Box<dyn Transport>, ()>) + 'static + Send + Sync>;

/// Internal state of the [`Transport`].
pub enum State {
    /// [`Transport`] is in the startup phase.
    Startup(Startup),
    /// [`Transport`] is in the sending phase.
    Send(Box<dyn Transport>),
}

/// Function to pass to [`sys::transport_set_startup_func`], which in turn calls
/// the user defined one.
///
/// `state` is mutably thread-safe, because this function is only called once
/// during [`Options::init`], which is blocked with our global [`Mutex`],
/// preventing [`Event::capture`] or [`shutdown`](crate::shutdown), the only
/// functions that interfere.
///
/// This function will catch any unwinding panics and [`abort`] if any occured.
pub extern "C" fn startup(options: *const sys::Options, state: *mut c_void) -> c_int {
    let options = Options::from_sys(Ownership::Borrowed(options));

    let state = unsafe { Box::from_raw(state.cast::<Option<State>>()) };
    let mut state = ManuallyDrop::new(state);

    if let Some(State::Startup(startup)) = state.take() {
        if let Ok(transport) = ffi::catch(|| startup(&options)) {
            state.replace(State::Send(transport));

            0
        } else {
            1
        }
    } else {
        process::abort();
    }
}

/// Function to pass to [`sys::transport_new`], which in turn calls the user
/// defined one.
///
/// This function will catch any unwinding panics and [`abort`] if any occured.
pub extern "C" fn send(envelope: *mut sys::Envelope, state: *mut c_void) {
    let envelope = RawEnvelope(envelope);

    let state = unsafe { Box::from_raw(state.cast::<Option<State>>()) };
    let state = ManuallyDrop::new(state);

    if let Some(State::Send(transport)) = state.as_ref() {
        ffi::catch(|| transport.send(envelope));
    } else {
        process::abort();
    }
}

/// Function to pass to [`sys::transport_set_shutdown_func`], which in turn
/// calls the user defined one.
///
/// `state` is ownership thread-safe, because this function is only called once
/// during [`shutdown`](crate::shutdown), which is blocked with our global
/// [`Mutex`], preventing [`Options::init`] or [`Event::capture`], the only
/// functions that interfere.
///
/// This function will catch any unwinding panics and [`abort`] if any occured.
pub extern "C" fn shutdown(timeout: u64, state: *mut c_void) -> c_int {
    let timeout = Duration::from_millis(timeout);
    let mut state = unsafe { Box::from_raw(state.cast::<Option<State>>()) };

    if let Some(State::Send(transport)) = state.take() {
        ffi::catch(|| transport.shutdown(timeout)).into_raw()
    } else {
        process::abort();
    }
}

/// Wrapper for the raw envelope that we should send to Sentry.
///
/// # Examples
/// ```
/// # #[cfg(feature = "transport-custom")]
/// # use sentry_contrib_native::{Dsn, Request};
/// # use sentry_contrib_native::{Envelope, RawEnvelope, Transport, Value};
/// struct CustomTransport {
///     #[cfg(feature = "transport-custom")]
///     dsn: Dsn,
/// };
///
/// impl Transport for CustomTransport {
///     fn send(&self, raw_envelope: RawEnvelope) {
///         // get the `Event` that is being sent
///         let event: Value = raw_envelope.event();
///         // serialize it, maybe move this to another thread to prevent blocking
///         let envelope: Envelope = raw_envelope.serialize();
///         // or convert it into a `Request` right away!
///         #[cfg(feature = "transport-custom")]
///         let request: Request = raw_envelope.to_request(self.dsn.clone());
///     }
/// }
/// ```
#[derive(Debug, Hash, Eq, Ord, PartialEq, PartialOrd)]
pub struct RawEnvelope(*mut sys::Envelope);

unsafe impl Send for RawEnvelope {}
unsafe impl Sync for RawEnvelope {}

impl Drop for RawEnvelope {
    fn drop(&mut self) {
        unsafe { sys::envelope_free(self.0) }
    }
}

impl RawEnvelope {
    /// Serialize a [`RawEnvelope`] into an [`Envelope`].
    #[must_use = "`RawEnvelope::serialize` only converts it to an `Envelope`, this doesn't do anything until it is sent"]
    pub fn serialize(&self) -> Envelope {
        let mut envelope_size = 0;
        let serialized_envelope = unsafe { sys::envelope_serialize(self.0, &mut envelope_size) };

        Envelope {
            data: serialized_envelope,
            len: envelope_size,
        }
    }

    /// Yields the event that is being sent in the form of a [`Value`].
    #[must_use]
    pub fn event(&self) -> Value {
        Value::from_raw_borrowed(unsafe { sys::envelope_get_event(self.0) })
    }

    /// Constructs a HTTP request for the provided [`RawEnvelope`] with a
    /// [`Dsn`].
    ///
    /// For more information see [`Envelope::into_request`].
    #[cfg(feature = "transport-custom")]
    #[must_use = "`Request` doesn't do anything until it is sent"]
    pub fn to_request(&self, dsn: Dsn) -> Request {
        self.serialize().into_request(dsn)
    }
}

/// The actual body which transports send to Sentry.
///
/// # Examples
/// ```
/// # #[cfg(feature = "transport-custom")]
/// # use sentry_contrib_native::{Dsn, Request};
/// # use sentry_contrib_native::{Envelope, RawEnvelope, Transport, Value};
/// struct CustomTransport {
///     #[cfg(feature = "transport-custom")]
///     dsn: Dsn,
/// };
///
/// impl Transport for CustomTransport {
///     fn send(&self, raw_envelope: RawEnvelope) {
///         // serialize it, maybe move this to another thread to prevent blocking
///         let envelope: Envelope = raw_envelope.serialize();
///         // look at that body!
///         println!("{:?}", envelope.as_bytes());
///         // let's build the whole `Request`
///         #[cfg(feature = "transport-custom")]
///         let request: Request = envelope.into_request(self.dsn.clone());
///     }
/// }
/// ```
#[derive(Debug, Hash, Eq, Ord, PartialEq, PartialOrd)]
pub struct Envelope {
    /// The raw bytes of the serialized envelope, which is the actual data to
    /// send as the body of a request.
    data: *const c_char,
    /// The length in bytes of the serialized data.
    len: usize,
}

unsafe impl Send for Envelope {}
unsafe impl Sync for Envelope {}

impl Drop for Envelope {
    fn drop(&mut self) {
        unsafe { sys::free(self.data as _) }
    }
}

impl AsRef<[u8]> for Envelope {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl Envelope {
    /// Get underlying data as `&[u8]`.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.data.cast(), self.len) }
    }

    /// Constructs a HTTP request for the provided [`sys::Envelope`] with the
    /// DSN that was registered with the SDK.
    ///
    /// The return value has all of the necessary pieces of data to create a
    /// HTTP request with the HTTP client of your choice:
    ///
    /// * The URL to send the request to.
    /// * The headers that must be set.
    /// * The body of the request.
    ///
    /// The `content-length` header is already set for you, though some HTTP
    /// clients will automatically overwrite it, which should be fine.
    ///
    /// The `body` in the request is an [`Envelope`], which implements
    /// `AsRef<[u8]>` to retrieve the actual bytes that should be sent as the
    /// body.
    #[cfg(feature = "transport-custom")]
    #[must_use = "`Request` doesn't do anything until it is sent"]
    pub fn into_request(self, dsn: Dsn) -> Request {
        let mut request = HttpRequest::builder();
        *request.headers_mut().expect("failed to build headers") = dsn.to_headers();
        request
            .method("POST")
            .uri(dsn.url)
            .header("content-length", self.as_bytes().len())
            .body(self)
            .expect("failed to build request")
    }
}

/// Contains the pieces that are needed to build correct headers for a request
/// based on the given DSN.
///
/// # Examples
/// ```
/// # /*
/// #![cfg(feature = "transport-custom")]
///
/// # */
/// # fn main() -> anyhow::Result<()> {
/// # #[cfg(feature = "transport-custom")]
/// # {
/// # use sentry_contrib_native::{Dsn, Event, http::HeaderMap, Options, RawEnvelope, test, Transport};
///
/// struct CustomTransport {
///     dsn: Dsn,
/// };
///
/// impl CustomTransport {
///     fn new(options: &Options) -> Result<Self, ()> {
///         Ok(CustomTransport {
///             // we can also get the DSN here
///             dsn: options.dsn().and_then(|dsn| Dsn::new(dsn).ok()).ok_or(())?,
///         })
///     }
/// }
///
/// impl Transport for CustomTransport {
///     fn send(&self, envelope: RawEnvelope) {
///         // we need `Dsn` to build the `Request`!
///         envelope.to_request(self.dsn.clone());
///         // or build your own request with the help of a URL, `HeaderMap` and body.
///         let (url, headers, body): (&str, HeaderMap, &[u8]) = (self.dsn.url(), self.dsn.to_headers(), envelope.serialize().as_bytes());
///     }
/// }
///
/// let dsn = "https://public_key_1234@organization_1234.ingest.sentry.io/project_id_1234";
///
/// let mut options = Options::new();
/// options.set_dsn(dsn);
/// // we can take the `dsn` right here
/// let custom_transport = CustomTransport {
///     dsn: Dsn::new(dsn)?,
/// };
/// options.set_transport(move |_| Ok(custom_transport));
/// // this is also possible
/// options.set_transport(|options| Ok(CustomTransport {
///     dsn: options.dsn().and_then(|dsn| Dsn::new(dsn).ok()).ok_or(())?,
/// }));
/// // or use a method more directly
/// options.set_transport(CustomTransport::new);
/// # } Ok(()) }
/// ```
#[cfg(feature = "transport-custom")]
#[derive(Clone, Debug, Hash, Eq, Ord, PartialEq, PartialOrd)]
pub struct Dsn {
    /// The auth header value
    auth: String,
    /// The full URL to send envelopes to
    url: String,
}

#[cfg(feature = "transport-custom")]
impl Dsn {
    /// Creates a new [`Dsn`] from a [`str`].
    ///
    /// # Errors
    /// Fails with [`Error::Transport`](crate::Error::Transport) if the DSN is
    /// invalid.
    pub fn new(dsn: &str) -> Result<Self, crate::Error> {
        // a sentry DSN contains the following components:
        // <https://<username>@<host>/<path>>
        // * username = public key
        // * host = obviously, the host, sentry.io in the case of the hosted service
        // * path = the project ID
        let dsn_url = Url::parse(dsn).map_err(Error::from)?;

        // do some basic checking that the DSN is remotely valid
        if !dsn_url.scheme().starts_with("http") {
            return Err(Error::Scheme.into());
        }

        if dsn_url.username().is_empty() {
            return Err(Error::Username.into());
        }

        if dsn_url.path().is_empty() || dsn_url.path() == "/" {
            return Err(Error::ProjectId.into());
        }

        match dsn_url.host_str() {
            None => Err(Error::Host.into()),
            Some(host) => {
                let mut auth = format!(
                    "Sentry sentry_key={}, sentry_version={}, sentry_client={}",
                    dsn_url.username(),
                    API_VERSION,
                    SDK_USER_AGENT
                );

                if let Some(password) = dsn_url.password() {
                    auth.push_str(", sentry_secret=");
                    auth.push_str(password);
                }

                let host = dsn_url
                    .port()
                    .map_or_else(|| host.to_owned(), |port| format!("{}:{}", host, port));

                let url = format!(
                    "{}://{}/api/{}/envelope/",
                    dsn_url.scheme(),
                    host,
                    &dsn_url.path()[1..]
                );

                Ok(Self { auth, url })
            }
        }
    }

    /// The auth header value.
    #[must_use]
    pub fn auth(&self) -> &str {
        &self.auth
    }

    /// The full URL to send envelopes to.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Consume [`Dsn`] and return it's parts.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn into_parts(self) -> Parts {
        Parts {
            auth: self.auth,
            url: self.url,
        }
    }

    /// Yields a [`HeaderMap`] to build a correct HTTP request with this
    /// [`Dsn`].
    #[cfg(feature = "transport-custom")]
    #[must_use]
    pub fn to_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("user-agent", HeaderValue::from_static(SDK_USER_AGENT));
        headers.insert("content-type", HeaderValue::from_static(ENVELOPE_MIME));
        headers.insert("accept", HeaderValue::from_static("*/*"));
        headers.insert(
            "x-sentry-auth",
            (&self.auth)
                .try_into()
                .expect("failed to insert `x-sentry-auth`"),
        );
        headers
    }
}

#[cfg(feature = "transport-custom")]
impl FromStr for Dsn {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

#[cfg(feature = "transport-custom")]
impl TryFrom<&str> for Dsn {
    type Error = crate::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// [`Parts`] aquired from [`Dsn::into_parts`].
#[cfg(feature = "transport-custom")]
#[derive(Clone, Debug, Hash, Eq, Ord, PartialEq, PartialOrd)]
pub struct Parts {
    /// The auth header value
    pub auth: String,
    /// The full URL to send envelopes to
    pub url: String,
}

#[cfg(all(test, feature = "transport-custom"))]
#[rusty_fork::fork_test(timeout_ms = 60000)]
fn transport() -> anyhow::Result<()> {
    use crate::Event;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    struct CustomTransport {
        dsn: Dsn,
    }

    impl CustomTransport {
        fn new(dsn: Dsn, options: &Options) -> Self {
            assert!(!STARTUP.swap(true, Ordering::SeqCst));
            assert_eq!(dsn, Dsn::new(options.dsn().unwrap()).unwrap());

            Self { dsn }
        }
    }

    impl Transport for CustomTransport {
        fn send(&self, envelope: RawEnvelope) {
            SEND.fetch_add(1, Ordering::SeqCst);

            let _event = envelope.event();
            let request_1 = envelope.to_request(self.dsn.clone());

            let envelope = envelope.serialize();
            let request_2 = envelope.into_request(self.dsn.clone());

            assert_eq!(request_1.uri(), request_2.uri());
            assert_eq!(request_1.headers(), request_2.headers());
            assert_eq!(request_1.body().as_bytes(), request_2.body().as_bytes());
        }

        fn shutdown(self: Box<Self>, _: Duration) -> Shutdown {
            assert!(!SHUTDOWN.swap(true, Ordering::SeqCst));
            Shutdown::Success
        }
    }

    static STARTUP: AtomicBool = AtomicBool::new(false);
    static SEND: AtomicUsize = AtomicUsize::new(0);
    static SHUTDOWN: AtomicBool = AtomicBool::new(false);

    let mut options = Options::new();
    let dsn = Dsn::new(options.dsn().unwrap())?;
    let _event = dsn.to_headers();
    options.set_transport(|options| Ok(CustomTransport::new(dsn, options)));
    let shutdown = options.init()?;

    Event::new().capture();
    Event::new().capture();
    Event::new().capture();

    shutdown.shutdown();

    assert!(STARTUP.load(Ordering::SeqCst));
    assert_eq!(3, SEND.load(Ordering::SeqCst));
    assert!(SHUTDOWN.load(Ordering::SeqCst));

    Ok(())
}

#[cfg(all(test, feature = "transport-custom"))]
#[rusty_fork::fork_test(timeout_ms = 60000)]
fn dsn() {
    use crate::Event;

    #[allow(clippy::needless_pass_by_value)]
    fn send(envelope: RawEnvelope) {
        {
            let dsn = Dsn::new(
                "https://a0b1c2d3e4f5678910abcdeffedcba12@o209016.ingest.sentry.io/0123456",
            )
            .unwrap();
            let request = envelope.to_request(dsn);

            assert_eq!(
                request.uri(),
                "https://o209016.ingest.sentry.io/api/0123456/envelope/"
            );
            let headers = request.headers();
            assert_eq!(headers.get("x-sentry-auth").unwrap(), &format!("Sentry sentry_key=a0b1c2d3e4f5678910abcdeffedcba12, sentry_version={}, sentry_client={}", API_VERSION, SDK_USER_AGENT));
        }

        {
            let dsn = Dsn::new("http://a0b1c2d3e4f5678910abcdeffedcba12@192.168.1.1:9000/0123456")
                .unwrap();
            let request = envelope.to_request(dsn);

            assert_eq!(
                request.uri(),
                "http://192.168.1.1:9000/api/0123456/envelope/"
            );
            let headers = request.headers();
            assert_eq!(headers.get("x-sentry-auth").unwrap(), &format!("Sentry sentry_key=a0b1c2d3e4f5678910abcdeffedcba12, sentry_version={}, sentry_client={}", API_VERSION, SDK_USER_AGENT));
        }
    }

    let mut options = Options::new();
    options.set_transport(|_| Ok(send));
    let _shutdown = options.init();

    Event::new().capture();
}
