//! Contains types for creating custom transports that the underlying sentry-native
//! library can use to send data to your upstream Sentry service in lieue of
//! the built-in transports provided by the sentry-native library itself

use std::{convert::TryFrom, os::raw::c_void};

/// The request your [`Transporter`] is expected to send.
pub type SentryRequest = http::Request<Envelope>;

/// From sentry.h, but only present as a preprocessor define :(
pub const USER_AGENT: &str = "sentry.native/0.3.2";
/// The MIME type for Sentry envelopes
pub const ENVELOPE_MIME: &str = "application/x-sentry-envelope";
/// Version of the Sentry API we can communicate with, AFAICT this is just
/// hardcoded into sentry-native, so...two can play at that game!
pub const API_VERSION: i8 = 7;

/// The return from [`TransportWorker::shutdown`], which determines if we tell
/// the Sentry SDK if we were able to send all requests to the remote service
/// or not in the time allotted
#[allow(clippy::module_name_repetitions)]
#[derive(Copy, Clone)]
pub enum TransportShutdown {
    /// The custom transport was able to send all requests in the time specified
    Success,
    /// One or more requests could be sent in the specified time frame
    TimedOut,
}

/// Trait used to define your own transport that Sentry can use to send events
/// to a Sentry service
#[allow(clippy::module_name_repetitions)]
pub trait TransportWorker {
    /// Starts up the transport worker, with the options that were used to
    /// create the Sentry SDK
    fn startup(&self, dsn: Dsn, debug: bool);

    /// Sends the specified Envelope to a Sentry service.
    ///
    /// It is **highly** recommended to not block in this method, but rather
    /// to enqueue the worker to another thread.
    fn send(&self, envelope: PostedEnvelope);

    /// Shuts down the transport worker. The worker should try to flush all
    /// of the pending requests to Sentry before shutdown. If the worker is
    /// successfully able to empty its queue and shutdown before the specified
    /// timeout duration, it should return [`WorkerShutdown::Success`].
    fn shutdown(&self, timeout: std::time::Duration) -> TransportShutdown;

    /// Constructs an HTTP request for the provided [`sys::Envelope`] with the
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
    /// clients will automatically set it for you in some cases, which should
    /// be fine.
    ///
    /// The `body` in the request is a [`Envelope`], which implements `AsRef<[u8]`
    /// to retrieve the actual bytes that should be sent as the body
    ///
    /// # Errors
    /// Can fail if the envelope can't be serialized, or there is an invalid
    /// header value
    fn convert_to_request(
        dsn: &Dsn,
        envelope: PostedEnvelope,
    ) -> Result<SentryRequest, &'static str>
    where
        Self: Sized,
    {
        let mut builder = http::Request::builder();

        {
            let headers = builder
                .headers_mut()
                .ok_or_else(|| "unable to mutate headers")?;
            headers.insert(
                "user-agent",
                USER_AGENT
                    .parse()
                    .map_err(|_| "failed to parse user agent")?,
            );
            headers.insert(
                "content-type",
                ENVELOPE_MIME
                    .parse()
                    .map_err(|_| "failed to parse MIME type")?,
            );
            headers.insert(
                "accept",
                "*/*".parse().map_err(|_| "failed to parse accept")?,
            );
        }

        builder = builder.method("POST");
        builder = dsn.build_req(builder);

        let envelope = Envelope::try_from(envelope)?;
        builder = builder.header("content-length", envelope.as_ref().len());

        builder
            .body(envelope)
            .map_err(|_| "failed to build HTTP request")
    }
}

/// Holds the state for your custom transport. The lifetime of this state is
/// handled by the underlying Sentry library, which is why you only get a `Box<>`
pub struct Transport {
    /// The inner transport that our state is attached to
    pub(crate) inner: *mut sys::Transport,
    /// The user's [`TransportWorker`] that's actually responsible for sendin
    /// requests to a remote Sentry service
    worker: Option<Box<dyn TransportWorker>>,
}

impl Transport {
    /// Creates a new Transport for Sentry using your provided [`Transporter`]
    /// implementation. It's required to by [`Send`] and [`Sync`] as requests
    /// can come at any time.
    #[must_use]
    pub fn new(worker: Box<dyn TransportWorker>) -> Box<Self> {
        let inner = unsafe { sys::transport_new(Some(Self::send_function)) };

        unsafe {
            let ret = Box::new(Self {
                inner,
                worker: Some(worker),
            });

            let ptr = ret.into_raw();
            sys::transport_set_state(inner, ptr);
            sys::transport_set_startup_func(inner, Some(Self::startup));
            sys::transport_set_shutdown_func(inner, Some(Self::shutdown));
            sys::transport_set_free_func(inner, Some(Self::free));

            Self::from_raw(ptr)
        }
    }

    /// Convert ourselves into a state pointer, and prevents deallocating
    #[inline]
    fn into_raw(self: Box<Self>) -> *mut c_void {
        Box::into_raw(self) as *mut _
    }

    /// Convert a state pointer back into a Box
    #[inline]
    fn from_raw(state: *mut c_void) -> Box<Self> {
        unsafe { Box::from_raw(state as *mut _) }
    }

    /// The function registered with [`sys::transport_new`] when the SDK wishes
    /// to send an envelope to Sentry
    extern "C" fn send_function(envelope: *mut sys::Envelope, state: *mut c_void) {
        let s = Self::from_raw(state);
        let envelope = PostedEnvelope(envelope);

        if let Some(q) = &s.worker {
            q.send(envelope);
        }

        s.into_raw();
    }

    /// The function registered with [`sys::transport_set_startup_hook`] to
    /// start our transport so that we can being sending requests to Sentry
    extern "C" fn startup(options: *const sys::Options, state: *mut c_void) {
        let s = Self::from_raw(state);

        if let Some(imp) = &s.worker {
            unsafe {
                let dsn = sys::options_get_dsn(options);
                let debug = sys::options_get_debug(options) == 1;

                if dsn.is_null() {
                    if debug {
                        eprintln!("[sentry-contrib-native]: DSN is null");
                    }

                    s.into_raw();
                    return;
                }

                let dsn = std::ffi::CStr::from_ptr(dsn);

                match dsn.to_str() {
                    Ok(dsn_url) => match dsn_url.parse() {
                        Ok(dsn) => {
                            imp.startup(dsn, debug);
                        }
                        Err(err) => {
                            if debug {
                                eprintln!("[sentry-contrib-native]: Failed to parse DSN: {}", err);
                            }
                        }
                    },
                    Err(err) => {
                        if debug {
                            eprintln!(
                                "[sentry-contrib-native]: DSN url has invalid UTF-8: {}",
                                err
                            );
                        }
                    }
                }
            }
        }

        s.into_raw();
    }

    /// The function registered with [`sys::transport_set_shutdown_func`] which
    /// will attempt to flush all of the outstanding requests via the transport,
    /// and shutdown the worker thread, before the specified timeout is reached
    extern "C" fn shutdown(timeout: u64, state: *mut c_void) -> bool {
        let mut s = Self::from_raw(state);

        let sent_all = match s.worker.take() {
            Some(worker) => match worker.shutdown(std::time::Duration::from_millis(timeout)) {
                TransportShutdown::Success => true,
                TransportShutdown::TimedOut => false,
            },
            None => true,
        };

        s.into_raw();

        sent_all
    }

    /// The function registered with [`sys::transport_set_free_func`] that
    /// actually frees our state
    extern "C" fn free(state: *mut c_void) {
        let mut s = Self::from_raw(state);
        s.inner = std::ptr::null_mut();
    }
}

impl Drop for Transport {
    fn drop(&mut self) {
        unsafe {
            if !self.inner.is_null() {
                sys::transport_free(self.inner);
            }
        }
    }
}

/// Wrapper for the raw Envelope that we should send to Sentry
pub struct PostedEnvelope(pub *mut sys::Envelope);

unsafe impl Send for PostedEnvelope {}

impl Drop for PostedEnvelope {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                sys::envelope_free(self.0);
                self.0 = std::ptr::null_mut();
            }
        }
    }
}

/// The actual body which transports send to Sentry.
pub struct Envelope {
    /// The underlying opaque pointer. Freed once we are finished with the envelope.
    inner: PostedEnvelope,
    /// The raw bytes of the serialized envelope, which is the actual data to
    /// send as the body of a request
    data: *const std::os::raw::c_char,
    /// The length in bytes of the serialized data
    len: usize,
}

unsafe impl Send for Envelope {}

impl AsRef<[u8]> for Envelope {
    fn as_ref(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.data as *const _, self.len) }
    }
}

impl Drop for Envelope {
    fn drop(&mut self) {
        if !self.data.is_null() {
            unsafe {
                sys::free(self.data as *mut _);
                self.data = std::ptr::null();
                drop(&mut self.inner);
            }
        }
    }
}

impl TryFrom<PostedEnvelope> for Envelope {
    type Error = &'static str;

    fn try_from(pe: PostedEnvelope) -> Result<Self, Self::Error> {
        unsafe {
            let mut envelope_size = 0;
            let serialized_envelope = sys::envelope_serialize(pe.0, &mut envelope_size);

            // I assume the serialization can fail
            if envelope_size == 0 || serialized_envelope.is_null() {
                return Err("failed to serialize envelope");
            }

            Ok(Self {
                inner: pe,
                data: serialized_envelope,
                len: envelope_size,
            })
        }
    }
}

/// Contains the pieces we need to send requests based on the DSN the user
/// set on [`Options`]
pub struct Dsn {
    /// The auth header value
    pub auth: String,
    /// The full URI to send envelopes to
    pub uri: String,
}

impl Dsn {
    /// Adds the URI and auth header to the request
    #[inline]
    fn build_req(&self, mut rb: http::request::Builder) -> http::request::Builder {
        rb = rb.header("x-sentry-auth", &self.auth);
        rb.uri(&self.uri)
    }
}

impl std::str::FromStr for Dsn {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // A sentry DSN contains the following components:
        // <https://<username>@<host>/<path>>
        // * username = public key
        // * host = obviously, the host, sentry.io in the case of the hosted service
        // * path = the project ID
        let dsn_url = url::Url::parse(s).map_err(|_| "failed to parse DSN url")?;

        // Do some basic checking that the DSN is remotely valid
        if !dsn_url.scheme().starts_with("http") {
            return Err("DSN doesn't have an http(s) scheme");
        }

        if dsn_url.username().is_empty() {
            return Err("DSN has no username");
        }

        if dsn_url.path().is_empty() || dsn_url.path() == "/" {
            return Err("DSN doesn't have a path");
        }

        match dsn_url.host_str() {
            Some(host) => {
                let auth = format!(
                    "Sentry sentry_key={}, sentry_version={}, sentry_client={}",
                    dsn_url.username(),
                    API_VERSION,
                    USER_AGENT
                );

                let uri = format!(
                    "{}://{}/api/{}/envelope/",
                    dsn_url.scheme(),
                    host,
                    &dsn_url.path()[1..]
                );

                Ok(Self { auth, uri })
            }
            None => Err("DSN doesn't have a host"),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parses_dsn() {
        {
            let hosted =
                "https://a0b1c2d3e4f5678910abcdeffedcba12@o209016.ingest.sentry.io/0123456";

            let mut builder = http::Request::builder();
            let dsn: Dsn = hosted.parse().expect("failed to parse hosted DSN");
            builder = dsn.build_req(builder);

            assert_eq!(
                builder.uri_ref().unwrap(),
                "https://o209016.ingest.sentry.io/api/0123456/envelope/"
            );
            let headers = builder.headers_ref().unwrap();
            assert_eq!(headers.get("x-sentry-auth").unwrap(), &format!("Sentry sentry_key=a0b1c2d3e4f5678910abcdeffedcba12, sentry_version={}, sentry_client={}", API_VERSION, USER_AGENT));
        }

        {
            let private = "http://a0b1c2d3e4f5678910abcdeffedcba12@192.168.1.1/0123456";

            let mut builder = http::Request::builder();
            let dsn: Dsn = private.parse().expect("failed to parse hosted DSN");
            builder = dsn.build_req(builder);

            assert_eq!(
                builder.uri_ref().unwrap(),
                "http://192.168.1.1/api/0123456/envelope/"
            );
            let headers = builder.headers_ref().unwrap();
            assert_eq!(headers.get("x-sentry-auth").unwrap(), &format!("Sentry sentry_key=a0b1c2d3e4f5678910abcdeffedcba12, sentry_version={}, sentry_client={}", API_VERSION, USER_AGENT));
        }
    }
}
