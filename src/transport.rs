//! Contains types for creating custom transports that the underlying
//! sentry-native library can use to send data to your upstream Sentry service
//! in lieue of the built-in transports provided by the sentry-native library
//! itself.

use crate::{Options, Ownership};
use http::{HeaderValue, Request};
use std::{
    mem::{self, ManuallyDrop},
    os::raw::c_void,
    time::Duration,
};
use sys::SDK_USER_AGENT;

/// The request your [`TransportWorker`] is expected to send.
pub type SentryRequest = Request<Envelope>;

/// The MIME type for Sentry envelopes.
pub const ENVELOPE_MIME: &str = "application/x-sentry-envelope";
/// Version of the Sentry API we can communicate with, AFAICT this is just
/// hardcoded into sentry-native, so...two can play at that game!
pub const API_VERSION: i8 = 7;

/// The return from [`TransportWorker::shutdown`], which determines if we tell
/// the Sentry SDK if we were able to send all requests to the remote service
/// or not in the time allotted.
#[allow(clippy::module_name_repetitions)]
#[derive(Copy, Clone)]
pub enum TransportShutdown {
    /// The custom transport was able to send all requests in the time
    /// specified.
    Success,
    /// One or more requests could not be sent in the specified time frame.
    TimedOut,
}

/// Trait used to define your own transport that Sentry can use to send events
/// to a Sentry service.
pub trait Transport {
    /// Starts up the transport worker, with the options that were used to
    /// create the Sentry SDK.
    fn startup(&mut self, dsn: &Options);

    /// Sends the specified Envelope to a Sentry service.
    ///
    /// It is **highly** recommended to not block in this method, but rather
    /// to enqueue the worker to another thread.
    fn send(&mut self, envelope: PostedEnvelope);

    /// Shuts down the transport worker. The worker should try to flush all
    /// of the pending requests to Sentry before shutdown. If the worker is
    /// successfully able to empty its queue and shutdown before the specified
    /// timeout duration, it should return [`WorkerShutdown::Success`],
    /// otherwise it should return [`WorkerShutdown::TimedOut`].
    fn shutdown(&mut self, timeout: Duration) -> TransportShutdown;

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
    ///
    /// # Errors
    /// Can fail if the envelope can't be serialized, or there is an invalid
    /// header value.
    #[must_use]
    fn convert_to_request(dsn: &Dsn, envelope: PostedEnvelope) -> SentryRequest
    where
        Self: Sized,
    {
        let mut builder = Request::builder()
            .header("user-agent", HeaderValue::from_static(SDK_USER_AGENT))
            .header("content-type", HeaderValue::from_static(ENVELOPE_MIME))
            .header("accept", HeaderValue::from_static("*/*"))
            .method("POST");
        builder = dsn.build_req(builder);

        let envelope = Envelope::from(envelope);
        builder = builder.header("content-length", envelope.as_ref().len());

        builder.body(envelope).unwrap()
    }
}

/// The function registered with [`sys::transport_new`] when the SDK wishes
/// to send an envelope to Sentry
pub extern "C" fn send(envelope: *mut sys::Envelope, state: *mut c_void) {
    let state = state as *mut Box<dyn Transport>;
    let mut state = ManuallyDrop::new(unsafe { Box::from_raw(state) });
    let envelope = PostedEnvelope(envelope);

    state.send(envelope);
}

/// The function registered with [`sys::transport_set_startup_func`] to
/// start our transport so that we can being sending requests to Sentry
pub extern "C" fn startup(options: *const sys::Options, state: *mut c_void) {
    let state = state as *mut Box<dyn Transport>;
    let mut state = ManuallyDrop::new(unsafe { Box::from_raw(state) });
    let options = Options::from_sys(Ownership::Borrowed(options));

    state.startup(&options);
}

/// The function registered with [`sys::transport_set_shutdown_func`] which
/// will attempt to flush all of the outstanding requests via the transport,
/// and shutdown the worker thread, before the specified timeout is reached
pub extern "C" fn shutdown(timeout: u64, state: *mut c_void) -> bool {
    let state = state as *mut Box<dyn Transport>;
    let mut state = ManuallyDrop::new(unsafe { Box::from_raw(state) });
    let timeout = Duration::from_millis(timeout);

    match state.shutdown(timeout) {
        TransportShutdown::Success => true,
        TransportShutdown::TimedOut => false,
    }
}

/// The function registered with [`sys::transport_set_free_func`] that
/// actually frees our state
pub extern "C" fn free(state: *mut c_void) {
    mem::drop(unsafe { Box::from_raw(state as *mut Box<dyn Transport>) });
}

/// Wrapper for the raw Envelope that we should send to Sentry
pub struct PostedEnvelope(*mut sys::Envelope);

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
            }
        }
    }
}

impl From<PostedEnvelope> for Envelope {
    fn from(pe: PostedEnvelope) -> Self {
        unsafe {
            let mut envelope_size = 0;
            let serialized_envelope = sys::envelope_serialize(pe.0, &mut envelope_size);

            Self {
                data: serialized_envelope,
                len: envelope_size,
            }
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
                    SDK_USER_AGENT
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
            assert_eq!(headers.get("x-sentry-auth").unwrap(), &format!("Sentry sentry_key=a0b1c2d3e4f5678910abcdeffedcba12, sentry_version={}, sentry_client={}", API_VERSION, SDK_USER_AGENT));
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
            assert_eq!(headers.get("x-sentry-auth").unwrap(), &format!("Sentry sentry_key=a0b1c2d3e4f5678910abcdeffedcba12, sentry_version={}, sentry_client={}", API_VERSION, SDK_USER_AGENT));
        }
    }
}
