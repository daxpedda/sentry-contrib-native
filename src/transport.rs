//! Contains types for creating custom transports that the underlying
//! sentry-native library can use to send data to your upstream Sentry service
//! in lieue of the built-in transports provided by the sentry-native library
//! itself.

use crate::{ffi, Options, Ownership, Value};
use std::{
    mem::{self, ManuallyDrop},
    os::raw::{c_char, c_void},
    slice,
    time::Duration,
};
pub use sys::SDK_USER_AGENT;
#[cfg(feature = "custom-transport")]
use ::{
    http::{HeaderValue, Request as HttpRequest},
    std::{
        convert::{Infallible, TryFrom},
        str::FromStr,
    },
    thiserror::Error,
    url::{ParseError, Url},
};

/// Sentry errors.
#[cfg(feature = "custom-transport")]
#[cfg_attr(feature = "nightly", doc(cfg(feature = "custom-transport")))]
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
    ProjectID,
    /// DSN has no host.
    #[error("DSN has no host")]
    Host,
}

#[cfg(feature = "custom-transport")]
impl From<Infallible> for Error {
    fn from(from: Infallible) -> Self {
        match from {}
    }
}

/// The request your [`Transport`] is expected to send.
#[cfg(feature = "custom-transport")]
#[cfg_attr(feature = "nightly", doc(cfg(feature = "custom-transport")))]
pub type Request = HttpRequest<Envelope>;

/// The MIME type for Sentry envelopes.
pub const ENVELOPE_MIME: &str = "application/x-sentry-envelope";
/// Version of the Sentry API we can communicate with, AFAICT this is just
/// hardcoded into sentry-native, so...two can play at that game!
pub const API_VERSION: i8 = 7;

/// The return from [`Transport::shutdown`], which determines if we tell
/// the Sentry SDK if we were able to send all requests to the remote service
/// or not in the time allotted.
#[derive(Copy, Clone, Debug, Hash, Eq, Ord, PartialEq, PartialOrd)]
pub enum Shutdown {
    /// The custom transport was able to send all requests in the time
    /// specified.
    Success,
    /// One or more requests could not be sent in the specified time frame.
    TimedOut,
}

impl Shutdown {
    /// Converts [`Shutdown`] into [`bool`].
    fn into_raw(self) -> bool {
        match self {
            Self::Success => true,
            Self::TimedOut => false,
        }
    }
}

/// Trait used to define your own transport that Sentry can use to send events
/// to a Sentry service.
pub trait Transport: 'static + Send + Sync {
    /// Starts up the transport worker, with the options that were used to
    /// create the Sentry SDK.
    #[allow(unused_variables)]
    fn startup(&self, options: &Options) {}

    /// Sends the specified Envelope to a Sentry service.
    ///
    /// It is **highly** recommended to not block in this method, but rather
    /// to enqueue the worker to another thread.
    fn send(&self, envelope: RawEnvelope);

    /// Shuts down the transport worker. The worker should try to flush all
    /// of the pending requests to Sentry before shutdown. If the worker is
    /// successfully able to empty its queue and shutdown before the specified
    /// timeout duration, it should return [`Shutdown::Success`],
    /// otherwise it should return [`Shutdown::TimedOut`].
    #[allow(unused_variables)]
    fn shutdown(&self, timeout: Duration) -> Shutdown {
        Shutdown::Success
    }
}

impl<T: Fn(RawEnvelope) + 'static + Send + Sync> Transport for T {
    fn send(&self, envelope: RawEnvelope) {
        self(envelope)
    }
}

/// The function registered with [`sys::transport_new`] when the SDK wishes
/// to send an envelope to Sentry
pub extern "C" fn send(envelope: *mut sys::Envelope, state: *mut c_void) {
    let state = state as *mut Box<dyn Transport>;
    let state = ManuallyDrop::new(unsafe { Box::from_raw(state) });
    let envelope = RawEnvelope(envelope);

    ffi::catch(|| state.send(envelope))
}

/// The function registered with [`sys::transport_set_startup_func`] to
/// start our transport so that we can being sending requests to Sentry
pub extern "C" fn startup(options: *const sys::Options, state: *mut c_void) {
    let state = state as *mut Box<dyn Transport>;
    let state = ManuallyDrop::new(unsafe { Box::from_raw(state) });
    let options = Options::from_sys(Ownership::Borrowed(options));

    ffi::catch(|| state.startup(&options))
}

/// The function registered with [`sys::transport_set_shutdown_func`] which
/// will attempt to flush all of the outstanding requests via the transport,
/// and shutdown the worker thread, before the specified timeout is reached
pub extern "C" fn shutdown(timeout: u64, state: *mut c_void) -> bool {
    let state = state as *mut Box<dyn Transport>;
    let state = ManuallyDrop::new(unsafe { Box::from_raw(state) });
    let timeout = Duration::from_millis(timeout);

    ffi::catch(|| state.shutdown(timeout)).into_raw()
}

/// The function registered with [`sys::transport_set_free_func`] that
/// actually frees our state
pub extern "C" fn free(state: *mut c_void) {
    ffi::catch(|| mem::drop(unsafe { Box::from_raw(state as *mut Box<dyn Transport>) }))
}

/// Wrapper for the raw Envelope that we should send to Sentry
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
    #[must_use]
    pub fn serialize(&self) -> Envelope {
        let mut envelope_size = 0;
        let serialized_envelope = unsafe { sys::envelope_serialize(self.0, &mut envelope_size) };

        Envelope {
            data: serialized_envelope,
            len: envelope_size,
        }
    }

    /// Constructs a HTTP request for the provided [`RawEnvelope`] with a
    /// [`Dsn`].
    ///
    /// For more information see [`Envelope::into_request`].
    #[cfg(feature = "custom-transport")]
    #[cfg_attr(feature = "nightly", doc(cfg(feature = "custom-transport")))]
    #[must_use]
    pub fn to_request(&self, dsn: Dsn) -> Request {
        self.serialize().into_request(dsn)
    }

    /// Yields the event that is being sent in the form of a [`Value`].
    #[must_use]
    pub fn event(&self) -> Value {
        Value::from_raw_borrowed(unsafe { sys::envelope_get_event(self.0) })
    }
}

/// The actual body which transports send to Sentry.
#[derive(Debug, Hash, Eq, Ord, PartialEq, PartialOrd)]
pub struct Envelope {
    /// The raw bytes of the serialized envelope, which is the actual data to
    /// send as the body of a request
    data: *const c_char,
    /// The length in bytes of the serialized data
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
    /// Get underlying data as `[u8]`.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.data as _, self.len) }
    }

    /// Get underlying data as an owned `Vec<u8>`.
    #[must_use]
    pub fn to_vec(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }

    /// Constructs a HTTP request for the provided [`sys::Envelope`] with the
    /// DSN that was registered with the SDK.
    ///
    /// The return value has all of the necessary pieces of data to create a
    /// HTTP request with the HTTP client of your choice:
    ///
    /// * The uri to send the request to
    /// * The headers that must be set
    /// * The body of the request
    ///
    /// The `content-length` header is already set for you, though some HTTP
    /// clients will automatically overwrite it, which should be fine.
    ///
    /// The `body` in the request is an [`Envelope`], which implements
    /// `AsRef<[u8]>` to retrieve the actual bytes that should be sent as the
    /// body.
    #[cfg(feature = "custom-transport")]
    #[cfg_attr(feature = "nightly", doc(cfg(feature = "custom-transport")))]
    #[must_use]
    pub fn into_request(self, dsn: Dsn) -> Request {
        HttpRequest::builder()
            .header("user-agent", HeaderValue::from_static(SDK_USER_AGENT))
            .header("content-type", HeaderValue::from_static(ENVELOPE_MIME))
            .header("accept", HeaderValue::from_static("*/*"))
            .method("POST")
            .header("x-sentry-auth", dsn.auth)
            .uri(dsn.url)
            .header("content-length", self.as_bytes().len())
            .body(self)
            .unwrap()
    }
}

impl From<RawEnvelope> for Envelope {
    fn from(value: RawEnvelope) -> Self {
        value.serialize()
    }
}

/// Contains the pieces we need to send requests based on the DSN the user
/// set on [`Options`]
#[cfg(feature = "custom-transport")]
#[cfg_attr(feature = "nightly", doc(cfg(feature = "custom-transport")))]
#[derive(Clone, Debug, Hash, Eq, Ord, PartialEq, PartialOrd)]
pub struct Dsn {
    /// The auth header value
    auth: String,
    /// The full URI to send envelopes to
    url: String,
}

#[cfg(feature = "custom-transport")]
#[cfg_attr(feature = "nightly", doc(cfg(feature = "custom-transport")))]
impl Dsn {
    /// Creates a new [`Dsn`] from a [`str`].
    ///
    /// # Errors
    /// Fails with [`Error::Transport`](crate::Error::Transport) if the DSN is
    /// invalid.
    pub fn new(dsn: &str) -> Result<Self, crate::Error> {
        // A sentry DSN contains the following components:
        // <https://<username>@<host>/<path>>
        // * username = public key
        // * host = obviously, the host, sentry.io in the case of the hosted service
        // * path = the project ID
        let dsn_url = Url::parse(dsn).map_err(Error::from)?;

        // Do some basic checking that the DSN is remotely valid
        if !dsn_url.scheme().starts_with("http") {
            return Err(Error::Scheme.into());
        }

        if dsn_url.username().is_empty() {
            return Err(Error::Username.into());
        }

        if dsn_url.path().is_empty() || dsn_url.path() == "/" {
            return Err(Error::ProjectID.into());
        }

        match dsn_url.host_str() {
            None => Err(Error::Host.into()),
            Some(host) => {
                let auth = format!(
                    "Sentry sentry_key={}, sentry_version={}, sentry_client={}",
                    dsn_url.username(),
                    API_VERSION,
                    SDK_USER_AGENT
                );

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

    /// The auth header value
    #[must_use]
    pub fn auth(&self) -> &str {
        &self.auth
    }

    /// The full URL to send envelopes to
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
}

#[cfg(feature = "custom-transport")]
impl FromStr for Dsn {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

#[cfg(feature = "custom-transport")]
impl TryFrom<&str> for Dsn {
    type Error = crate::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// [`Parts`] aquired from [`Dsn::into_parts`].
#[cfg(feature = "custom-transport")]
#[cfg_attr(feature = "nightly", doc(cfg(feature = "custom-transport")))]
#[derive(Clone, Debug, Hash, Eq, Ord, PartialEq, PartialOrd)]
pub struct Parts {
    /// The auth header value
    pub auth: String,
    /// The full URI to send envelopes to
    pub url: String,
}

#[cfg(all(test, feature = "custom-transport"))]
#[rusty_fork::test_fork(timeout_ms = 5000)]
fn dsn() -> anyhow::Result<()> {
    use crate::Event;

    struct Parser;

    impl Transport for Parser {
        fn send(&self, envelope: RawEnvelope) {
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
                let dsn = Dsn::new("http://a0b1c2d3e4f5678910abcdeffedcba12@192.168.1.1/0123456")
                    .unwrap();
                let request = envelope.to_request(dsn);

                assert_eq!(request.uri(), "http://192.168.1.1/api/0123456/envelope/");
                let headers = request.headers();
                assert_eq!(headers.get("x-sentry-auth").unwrap(), &format!("Sentry sentry_key=a0b1c2d3e4f5678910abcdeffedcba12, sentry_version={}, sentry_client={}", API_VERSION, SDK_USER_AGENT));
            }
        }
    }

    let mut options = Options::new();
    options.set_transport(Parser);
    let _shutdown = options.init();

    Event::new().capture();

    Ok(())
}
