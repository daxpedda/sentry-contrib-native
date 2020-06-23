use sentry_contrib_native as sentry;
use sentry::PostedEnvelope;
use parking_lot::{Mutex, Condvar};
use tokio::sync::mpsc;
use std::sync::Arc;

struct TransportState {
    tx: mpsc::Sender<PostedEnvelope>,
    shutdown: Arc<(Mutex<()>, Condvar)>,
}

async fn send_sentry_request(
    client: &reqwest::Client,
    req: sentry::SentryRequest,
) -> Result<(), String> {
    let (parts, body) = req.into_parts();
    let uri = parts.uri.to_string();

    // Sentry should only give us POST requests to send
    if parts.method != http::Method::POST {
        return Err(format!("Sentry SDK is trying to send an unexpected request of '{}'", parts.method));
    }
    
    let rb = client.post(&parts.uri.to_string());

    // We cheat so that we don't have to copy all of the bytes of the body
    // into a new buffer, but we have to fake that the slice is static, which
    // should be ok since we only need that buffer until the request is finished
    #[allow(unsafe_code)]
    let buffer = unsafe {
        let buf = body.as_ref();
        std::slice::from_raw_parts::<'static, u8>(buf.as_ptr(), buf.len())
    };

    let res = rb
        .headers(parts.headers)
        .body(reqwest::Body::from(buffer))
        .send()
        .await
        .map_err(|e| format!("Failed to send Sentry request: {}", e))?;

    res.error_for_status()
        .map_err(|e| format!("Received error response from Sentry: {}", e))?;
    Ok(())
}

// We can implement our own transport for Sentry data so that we don't pull in
// C dependencies (COUGH OPENSSL COUGH) that we don't want
struct ReqwestTransport {
    /// We don't currently use custom certs or proxies, so we can just use the
    /// same client that the rest of ark uses, if we do start doing that, we
    /// would need to build the client based on the options during startup()
    client: reqwest::Client,
    inner: Mutex<Option<TransportState>>,
    rt: tokio::runtime::Handle,
}

impl ReqwestTransport {
    fn new(client: reqwest::Client, rt: tokio::runtime::Handle) -> Self {
        Self {
            client,
            inner: Mutex::new(None),
            rt,
        }
    }
}

impl sentry::TransportWorker for ReqwestTransport {
    fn startup(&self, dsn: sentry::Dsn, _debug: bool) {
        let mut inner = self.inner.lock();
        match *inner {
            Some(_) => {
                eprintln!("sentry transport has already been started!");
            }
            None => {
                let (tx, mut rx) = mpsc::channel(1024);
                let shutdown = Arc::new((Mutex::new(()), Condvar::new()));
                let tshutdown = shutdown.clone();
                let client = self.client.clone();

                self.rt.enter(|| {
                    tokio::spawn(async move {
                        // Dequeue and send events until we are asked to shut down
                        while let Some(envelope) = rx.recv().await {
                            // Convert the envelope into an HTTP request
                            match Self::convert_to_request(&dsn, envelope) {
                                Ok(req) => match send_sentry_request(&client, req).await {
                                    Ok(_) => eprintln!("successfully sent sentry envelope"),
                                    Err(err) => {
                                        eprintln!("failed to send sentry envelope: {}", err)
                                    }
                                },
                                Err(err) => {
                                    eprintln!("failed to convert Sentry request: {}", err);
                                }
                            }
                        }

                        // Shutting down, signal the condition variable that we've
                        // finished sending everything, so that we can tell the
                        // SDK about whether we've sent it all before their timeout
                        let (lock, cvar) = &*tshutdown;
                        let _shutdown = lock.lock();
                        cvar.notify_one();
                    });
                });

                *inner = Some(TransportState { tx, shutdown });
            }
        }
    }

    fn send(&self, envelope: PostedEnvelope) {
        let inner = self.inner.lock();
        if let Some(inner) = &*inner {
            let mut tx = inner.tx.clone();
            self.rt.enter(|| {
                tokio::task::spawn(async move {
                    if let Err(err) = tx.send(envelope).await {
                        eprintln!("failed to send envelope to send queue: {}", err);
                    }
                });
            });
        }
    }

    fn shutdown(&self, timeout: std::time::Duration) -> sentry::TransportShutdown {
        // Drop the sender so that the background thread will exit once
        // it has dequeued and processed all the envelopes we have enqueued
        let inner = self.inner.lock().take();

        match inner {
            Some(inner) => {
                drop(inner.tx);

                // Wait for the condition variable to notify that the thread has shutdown
                let (lock, cvar) = &*inner.shutdown;
                let mut shutdown = lock.lock();
                let result = cvar.wait_for(&mut shutdown, timeout);

                if result.timed_out() {
                    sentry::TransportShutdown::TimedOut
                } else {
                    sentry::TransportShutdown::Success
                }
            }
            None => sentry::TransportShutdown::Success,
        }
    }
}

fn main() -> Result<(), String> {
    let mut options = sentry::Options::new();

    // Setting a DSN is absolutely required to use custom transports
    options.set_dsn("https://1234abcd@your.sentry.service.com/1234");

    // This debug flag is supplied to our custom transport if we want to eg
    // print debug information etc just as the underlying SDK does
    options.set_debug(true);

    // Setup a runtime, if you're using tokio or some other async runtime to
    // send requests, you'll need to pass the handle to your transport so that
    // you can actually spawn tasks correctly, since the calls are going to
    // come on threads created by the C lib itself and won't have a runtime
    // installed
    let runtime = tokio::runtime::Builder::new()
        .threaded_scheduler()
        .enable_all()
        .thread_name("sentry-tokio")
        .build()
        .map_err(|e| format!("Failed to create tokio runtime: {}", e))?;

    // In this case we are creating a client just for the transport, but in
    // a real app it is likely you would have this configured for other things
    // and just reuse it for Sentry. If you are using proxies are custom certs
    // with Sentry, you could also configure it here, or during startup, using
    // the options you set
    let client = reqwest::Client::new();

    // Actually registers our custom transport so that the SDK will use that to
    // send requests to your Sentry service, rather than the built in transports
    // that come with the SDK
    options.set_transport(sentry::Transport::new(Box::new(
        ReqwestTransport::new(client, runtime.handle().clone()),
    )));

    let _shutdown = options.init().map_err(|e| format!("Failed to initialize Sentry: {}", e))?;

    Ok(())
}
