use std::{
    os::raw::c_void,
    sync::{mpsc, Arc, Condvar, Mutex},
};

/// Trait used to define your own transpor that Sentry can use to send events
/// to a Sentry service
pub trait Transporter {
    fn send(&self, request: http::Request<Envelope>);
}

impl<F> Transporter for F
where
    F: Fn(http::Request<Envelope>) + Send + Sync,
{
    fn send(&self, request: http::Request<Envelope>) {
        self(request)
    }
}

struct Worker {
    tx: mpsc::Sender<*mut sys::Envelope>,
}

impl Worker {
    fn new(transporter: Box<dyn Transporter + Send + Sync>) -> Self {
        let (tx, rx) = mpsc::channel();
        let shutdown = Arc::new((Mutex::new(false), Condvar::new()));
        let tshutdown = shutdown.clone();

        let handle = std::thread::spawn(move || {
            while let Ok(envelope) = rx.recv() {
                // We don't protect against panics here, but maybe that should
                // be an option?
                match construct_request(envelope) {
                    Ok(request) => {
                        transporter.send(request);
                    }
                    Err(_) => unsafe { sys::envelope_free(envelope) },
                }
            }

            let (lock, cvar) = &*tshutdown;
            let mut shutdown = lock.lock().unwrap();
            *shutdown = true;
            cvar.notify_one();
        });

        Self { tx, shutdown }
    }

    fn enqueue(&self, envelope: *mut sys::Envelope) {
        self.tx.send(envelope).expect("failed to enqueue envelope");
    }

    fn shutdown(self, timeout: std::time::Duration) -> bool {
        drop(self.tx);

        let (lock, cvar) = &*self.shutdown;
        let mut shutdown = lock.lock().unwrap();
        let result = cvar.wait_timeout(shutdown, timeout).unwrap();

        result.1.timed_out()
    }
}

pub struct Transport {
    inner: *mut sys::Transport,
    user_impl: Option<Box<dyn Transporter + Send + Sync>>,
    worker: Option<Worker>,
}

impl Transport {
    pub fn new(transporter: Box<dyn Transporter + Send + Sync>) -> Box<Self> {
        let inner = unsafe { sys::transport_new(Self::send_function) };

        let ret = Box::new(Self {
            inner,
            user_impl: transporter,
            worker: None,
        });

        unsafe {
            sys::transport_set_state(inner, ret.as_ref().as_ptr() as *mut _);
            sys::transport_set_startup_hook(inner, Self::startup);
            sys::transport_set_shutdown_hook(inner, Self::shutdown);
        }

        ret
    }

    pub(crate) fn into_raw(self: Box<Self>) -> *mut c_void {
        Box::into_raw(self) as *mut _
    }

    pub(crate) fn from_raw(state: *mut c_void) -> Box<Self> {
        unsafe { Box::from_raw(state as *mut _) }
    }

    fn send_function(envelope: *mut Envelope, state: *mut c_void) {
        let self = Self::from_raw(state);
        if let Some(q) = &self.queue {
            q.enqueue(envelope);
        }
        Self::into_raw(self);
    }

    fn startup(options: *const sys::Options, state: *mut c_void) {
        let self = Self::from_raw(state);

        match self.user_impl.take() {
            Some(imp) => self.worker = Some(Worker::new(imp)),
            None => {}
        }

        self.into_raw()
    }

    fn shutdown(timeout: u64, state: *mut c_void) -> bool {
        let self = Self::from_raw(state);

        let sent_all = match self.worker.take() {
            Some(worker) => !worker.shutdown(),
            None => true,
        };

        self.into_raw();

        sent_all
    }
}

impl Drop for Transport {
    fn drop(&mut self) {
        unsafe {
            sys::transport_free(self.inner);
        }
    }
}

/// From sentry.h, but only present as a preprocessor define :(
const USER_AGENT: &str = "sentry.native/0.3.2";
const ENVELOPE_MIME: &str = "application/x-sentry-envelope";
/// Version of the Sentry API we can communicate with, AFAICT this is just
/// hardcoded into sentry-native, so...two can play at that game!
const API_VERSION: i8 = 7;

/// The actual body which transports send to Sentry.
pub struct Envelope {
    inner: *mut sys::Envelope,
    data: *const std::os::raw::c_char,
    len: usize,
}

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

fn construct_request(envelope: *mut sys::Envelope) -> Result<http::Request<Envelope>, ()> {
    use std::ffi::CStr;

    let mut builder = http::Request::builder();

    {
        let headers = builder.headers_mut().expect("unable to mutate headers");
        headers.insert("user-agent", USER_AGENT.parse().unwrap());
        headers.insert("content-type", ENVELOPE_MIME.parse().unwrap());
        headers.insert("accept", "*/*".parse().unwrap());
    }

    builder = builder.method("POST");

    // Get the DSN for the options, which informs us where to send the request, and what auth token to use
    let envelope = unsafe {
        let opts = sys::get_options();

        if opts.is_null() {
            return Err(());
        }

        let dsn = sys::options_get_dsn(opts);

        if dsn.is_null() {
            return Err(());
        }

        let dsn = CStr::from_ptr(dsn);
        let dsn_url = dsn.to_str().map_err(|_| ())?;

        builder = from_dsn(builder, dsn_url)?;

        let mut envelope_size = 0;
        let serialized_envelope = sys::envelope_serialize(envelope, &mut envelope_size);

        if envelope_size == 0 || serialized_envelope.is_null() {
            return Err(());
        }

        builder = builder.header("content-length", envelope_size);

        Envelope {
            inner: serialized_envelope,
            len: envelope_size,
        }
    };

    builder.body(envelope).map_err(|_| ())
}

fn from_dsn(
    mut builder: http::request::Builder,
    dsn_url: &str,
) -> Result<http::request::Builder, ()> {
    // A sentry DSN contains the following components:
    // <https://<username>@<host>/<path>>
    // * username = public key
    // * host = obviously, the host, sentry.io in the case of the hosted service
    // * path = the project ID
    let url = url::Url::parse(dsn_url).map_err(|_| ())?;

    // Do some basic checking that the DSN is remotely valid
    if !url.scheme().starts_with("http")
        || url.username().is_empty()
        || !url.has_host()
        || url.path().is_empty()
        || url.path() == "/"
    {
        return Err(());
    }

    builder = builder.header(
        "x-sentry-auth",
        format!(
            "Sentry sentry_key={}, sentry_version={}, sentry_client={}",
            url.username(),
            API_VERSION,
            USER_AGENT
        ),
    );

    builder = builder.uri(format!(
        "{}://{}/api/{}/envelope/",
        url.scheme(),
        url.host_str().expect("DSN didn't contain a host"),
        &url.path()[1..]
    ));

    Ok(builder)
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
            builder = from_dsn(builder, hosted).expect("failed to parse hosted URL");

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
            builder = from_dsn(builder, private).expect("failed to parse private URL");

            assert_eq!(
                builder.uri_ref().unwrap(),
                "http://192.168.1.1/api/0123456/envelope/"
            );
            let headers = builder.headers_ref().unwrap();
            assert_eq!(headers.get("x-sentry-auth").unwrap(), &format!("Sentry sentry_key=a0b1c2d3e4f5678910abcdeffedcba12, sentry_version={}, sentry_client={}", API_VERSION, USER_AGENT));
        }
    }
}
