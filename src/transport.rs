//! Contains types for creating custom transports that the underlying sentry-native
//! library can use to send data to your upstream Sentry service in lieue of
//! the built-in transports provided by the sentry-native library itself

use std::{
    os::raw::c_void,
    sync::{mpsc, Arc, Condvar, Mutex},
};

/// The request your [`Transporter`] is expected to send.
pub type SentryRequest = http::Request<Envelope>;

/// Trait used to define your own transport that Sentry can use to send events
/// to a Sentry service
pub trait Transporter {
    /// Method called when Sentry wishes to send data to a Sentry service
    ///
    /// The request contains all of the necessary pieces of data
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
    /// If your [`Transporter`] is unable to send the request, it can return
    /// an error if it wishes. This error will be printed out to stderr if
    /// you have enabled debugging in [`Options`]
    fn send(&self, request: SentryRequest) -> Result<(), &'static str>;
}

impl<F> Transporter for F
where
    F: Fn(SentryRequest) -> Result<(), &'static str> + Send + Sync,
{
    fn send(&self, request: SentryRequest) -> Result<(), &'static str> {
        self(request)
    }
}

/// Wrapper around the raw pointer so that we can mark it as Send so that we
/// postpone any actual work to the worker thread
struct PostedEnvelope(*mut sys::Envelope);

unsafe impl Send for PostedEnvelope {}

/// Sentry internally uses a simple background thread worker to do the actual
/// work of sending requests for the provided transports, so we do as well
struct Worker {
    /// Sender we use to enqueue work to the background thread
    tx: mpsc::Sender<PostedEnvelope>,
    /// Condition variable we use to detect when the background thread has been
    /// shutdown
    shutdown: Arc<(Mutex<()>, Condvar)>,
    /// True if the user specified they want debug information from the SDK
    debug: bool,
}

impl Worker {
    /// Creates a new worker which spins up a thread to handle sending requests
    /// to Sentry
    fn new(debug: bool, dsn: Dsn, transporter: Box<dyn Transporter + Send + Sync>) -> Self {
        let (tx, rx) = mpsc::channel::<PostedEnvelope>();
        let shutdown = Arc::new((Mutex::new(()), Condvar::new()));
        let tshutdown = shutdown.clone();

        if debug {
            eprintln!("[sentry-contrib-native]: Starting up worker thread");
        }

        std::thread::spawn(move || {
            while let Ok(envelope) = rx.recv() {
                let envelope = envelope.0;

                // We don't protect against panics here, but maybe that should
                // be an option?
                match construct_request(envelope, &dsn) {
                    Ok(request) => {
                        if debug {
                            eprintln!(
                                "[sentry-contrib-native]: Sending envelope {} {:#?}",
                                request.uri(),
                                request.headers(),
                            );
                        }

                        let res = transporter.send(request);

                        if debug {
                            match res {
                                Ok(_) => {
                                    eprintln!("[sentry-contrib-native]: Successfully sent envelope")
                                }
                                Err(err) => eprintln!(
                                    "[sentry-contrib-native]: Failed to send envelope: {}",
                                    err
                                ),
                            }
                        }
                    }
                    Err(_) => unsafe { sys::envelope_free(envelope) },
                }
            }

            let (lock, cvar) = &*tshutdown;
            let _shutdown = lock.lock().unwrap();
            cvar.notify_one();
        });

        Self {
            debug,
            tx,
            shutdown,
        }
    }

    /// Enqueues an envelope to be sent via the user's [`Transporter`]
    fn enqueue(&self, envelope: *mut sys::Envelope) {
        self.tx
            .send(PostedEnvelope(envelope))
            .expect("failed to enqueue envelope");
    }

    /// Shuts down the worker and waits for it be shutdown up to the specified
    /// time. Returns `true` if the timeout is reached before the worker has
    /// been fully shutdown.
    fn shutdown(self, timeout: std::time::Duration) -> bool {
        if self.debug {
            eprintln!("[sentry-contrib-native]: Shutting down worker thread");
        }

        // Drop the sender so that the background thread will exit once
        // it has dequeued and processed all the envelopes we have enqueued
        drop(self.tx);

        // Wait for the condition variable to notify that the thread has shutdown
        let (lock, cvar) = &*self.shutdown;
        let shutdown = lock.lock().unwrap();
        let result = cvar.wait_timeout(shutdown, timeout).unwrap();

        result.1.timed_out()
    }
}

/// Holds the state for your custom transport. The lifetime of this state is
/// handled by the underlying Sentry library, which is why you only get a `Box<>`
pub struct Transport {
    /// The inner transport that our state is attached to
    pub(crate) inner: *mut sys::Transport,
    /// The user's [`Transporter`] implementation, moved to the background
    /// worker on startup
    user_impl: Option<Box<dyn Transporter + Send + Sync>>,
    /// Our background worker
    worker: Option<Worker>,
}

impl Transport {
    /// Creates a new Transport for Sentry using your provided [`Transporter`]
    /// implementation. It's required to by [`Send`] and [`Sync`] as requests
    /// can come at any time.
    #[must_use]
    pub fn new(transporter: Box<dyn Transporter + Send + Sync>) -> Box<Self> {
        let inner = unsafe { sys::transport_new(Some(Self::send_function)) };

        unsafe {
            let ret = Box::new(Self {
                inner,
                user_impl: Some(transporter),
                worker: None,
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
        if let Some(q) = &s.worker {
            q.enqueue(envelope);
        }
        s.into_raw();
    }

    /// The function registered with [`sys::transport_set_startup_hook`] to
    /// start our transport so that we can being sending requests to Sentry
    extern "C" fn startup(options: *const sys::Options, state: *mut c_void) {
        let mut s = Self::from_raw(state);

        if let Some(imp) = s.user_impl.take() {
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
                            s.worker = Some(Worker::new(debug, dsn, imp));
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
            Some(worker) => !worker.shutdown(std::time::Duration::from_millis(timeout)),
            None => true,
        };

        s.into_raw();

        sent_all
    }

    /// The function registered with [`sys::transport_set_free_func`] that
    /// actually frees
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

/// From sentry.h, but only present as a preprocessor define :(
const USER_AGENT: &str = "sentry.native/0.3.2";
/// The MIME type for Sentry envelopes
const ENVELOPE_MIME: &str = "application/x-sentry-envelope";
/// Version of the Sentry API we can communicate with, AFAICT this is just
/// hardcoded into sentry-native, so...two can play at that game!
const API_VERSION: i8 = 7;

/// The actual body which transports send to Sentry.
pub struct Envelope {
    /// The underlying opaque pointer. Freed once we are finished with the envelope.
    inner: *mut sys::Envelope,
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
        unsafe {
            sys::free(self.data as *mut _);
            sys::envelope_free(self.inner);
        }
    }
}

/// Contains the pieces we need to send requests based on the DSN the user
/// set on [`Options`]
struct Dsn {
    /// The auth header value
    auth: String,
    /// The full URI to send envelopes to
    uri: String,
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
        let url = url::Url::parse(s).map_err(|_| "failed to parse DSN url")?;

        // Do some basic checking that the DSN is remotely valid
        if !url.scheme().starts_with("http") {
            return Err("DSN doesn't have an http(s) scheme");
        }

        if url.username().is_empty() {
            return Err("DSN has no username");
        }

        if url.path().is_empty() || url.path() == "/" {
            return Err("DSN doesn't have a path");
        }

        match url.host_str() {
            Some(host) => {
                let auth = format!(
                    "Sentry sentry_key={}, sentry_version={}, sentry_client={}",
                    url.username(),
                    API_VERSION,
                    USER_AGENT
                );

                let uri = format!(
                    "{}://{}/api/{}/envelope/",
                    url.scheme(),
                    host,
                    &url.path()[1..]
                );

                Ok(Self { auth, uri })
            }
            None => Err("DSN doesn't have a host"),
        }
    }
}

/// Constructs an HTTP request for the provided [`sys::Envelope`] with the DSN
/// that was registered with the SDK
fn construct_request(
    envelope: *mut sys::Envelope,
    dsn: &Dsn,
) -> Result<http::Request<Envelope>, ()> {
    let mut builder = http::Request::builder();

    {
        let headers = builder.headers_mut().expect("unable to mutate headers");
        headers.insert("user-agent", USER_AGENT.parse().unwrap());
        headers.insert("content-type", ENVELOPE_MIME.parse().unwrap());
        headers.insert("accept", "*/*".parse().unwrap());
    }

    builder = builder.method("POST");
    builder = dsn.build_req(builder);

    // Get the DSN for the options, which informs us where to send the request, and what auth token to use
    let envelope = unsafe {
        let mut envelope_size = 0;
        let serialized_envelope = sys::envelope_serialize(envelope, &mut envelope_size);

        if envelope_size == 0 || serialized_envelope.is_null() {
            return Err(());
        }

        builder = builder.header("content-length", envelope_size);

        Envelope {
            inner: envelope,
            data: serialized_envelope,
            len: envelope_size,
        }
    };

    builder.body(envelope).map_err(|_| ())
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
