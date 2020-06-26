#![warn(
    clippy::all,
    //clippy::missing_docs_in_private_items,
    clippy::nursery,
    clippy::pedantic,
    missing_docs
)]
// stable clippy seems to have an issue with await
#![allow(clippy::used_underscore_binding)]

//!

use anyhow::{anyhow, bail, Result};
use parking_lot::{Condvar, Mutex};
use reqwest::Client;
use sentry::{
    http::Method, Dsn, Event, Options, RawEnvelope, Request, Transport as SentryTransport,
    TransportShutdown,
};
use sentry_contrib_native as sentry;
use std::{slice, str::FromStr, sync::Arc, time::Duration};
use tokio::{
    runtime::Handle,
    sync::mpsc::{self, Sender},
    task,
};

struct TransportState {
    sender: Sender<RawEnvelope>,
    shutdown: Arc<(Mutex<()>, Condvar)>,
}

async fn send_sentry_request(client: &Client, req: Request) -> Result<()> {
    let (parts, body) = req.into_parts();
    let uri = parts.uri.to_string();

    // Sentry should only give us POST requests to send
    if parts.method != Method::POST {
        bail!(
            "Sentry SDK is trying to send an unexpected request of '{}'",
            parts.method
        )
    }

    let rb = client.post(&uri);

    // we cheat so that we don't have to copy all of the bytes of the body
    // into a new buffer, but we have to fake that the slice is static, which
    // should be ok since we only need that buffer until the request is finished
    #[allow(unsafe_code)]
    let buffer = unsafe {
        let buf = body.as_bytes();
        slice::from_raw_parts::<'static, _>(buf.as_ptr(), buf.len())
    };

    let response = rb
        .headers(parts.headers)
        .body(buffer)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to send Sentry request: {}", e))?;

    response
        .error_for_status()
        .map_err(|e| anyhow!("Received error response from Sentry: {}", e))?;
    Ok(())
}

// we can implement our own transport for Sentry data so that we don't pull in
// C dependencies (COUGH OPENSSL COUGH) that we don't want
struct Transport {
    /// we don't currently use custom certs or proxies, so we can just use the
    /// same client that the rest of ark uses, if we do start doing that, we
    /// would need to build the client based on the options during startup()
    client: Client,
    inner: Option<TransportState>,
    rt: Handle,
}

impl Transport {
    const fn new(client: Client, rt: Handle) -> Self {
        Self {
            client,
            inner: None,
            rt,
        }
    }
}

impl SentryTransport for Transport {
    fn startup(&mut self, options: &Options) {
        if self.inner.is_some() {
            eprintln!("sentry transport has already been started!");
        } else {
            let (sender, mut receiver) = mpsc::channel::<RawEnvelope>(1024);
            let shutdown = Arc::new((Mutex::new(()), Condvar::new()));
            self.inner = Some(TransportState {
                sender,
                shutdown: shutdown.clone(),
            });
            let client = self.client.clone();
            let dsn = Dsn::from_str(options.dsn().expect("no DSN found")).expect("invalid DSN");

            self.rt.enter(|| {
                tokio::spawn(async move {
                    // dequeue and send events until we are asked to shut down
                    while let Some(envelope) = receiver.recv().await {
                        // convert the envelope into an HTTP request
                        let req = envelope.to_request(dsn.clone());

                        match send_sentry_request(&client, req).await {
                            Ok(_) => eprintln!("successfully sent sentry envelope"),
                            Err(err) => eprintln!("failed to send sentry envelope: {}", err),
                        }
                    }

                    // shutting down, signal the condition variable that we've
                    // finished sending everything, so that we can tell the
                    // SDK about whether we've sent it all before their timeout
                    let (lock, cvar) = &*shutdown;
                    let _shutdown_lock = lock.lock();
                    cvar.notify_one();
                });
            });
        }
    }

    fn send(&mut self, envelope: RawEnvelope) {
        if let Some(inner) = &self.inner {
            let mut sender = inner.sender.clone();
            self.rt.enter(|| {
                task::spawn(async move {
                    if let Err(err) = sender.send(envelope).await {
                        eprintln!("failed to send envelope to send queue: {}", err);
                    }
                });
            });
        }
    }

    fn shutdown(&mut self, timeout: Duration) -> TransportShutdown {
        // drop the sender so that the background thread will exit once
        // it has dequeued and processed all the envelopes we have enqueued
        match self.inner.take() {
            Some(inner) => {
                drop(inner.sender);

                // wait for the condition variable to notify that the thread has shutdown
                let (lock, cvar) = &*inner.shutdown;
                let mut shutdown = lock.lock();
                let result = cvar.wait_for(&mut shutdown, timeout);

                if result.timed_out() {
                    TransportShutdown::TimedOut
                } else {
                    TransportShutdown::Success
                }
            }
            None => TransportShutdown::Success,
        }
    }
}

fn main() -> Result<()> {
    let mut options = sentry::Options::new();

    // setting a DSN is absolutely required to use custom transports
    options.set_dsn("https://1234abcd@your.sentry.service.com/1234");

    // setup a runtime, if you're using tokio or some other async runtime to
    // send requests, you'll need to pass the handle to your transport so that
    // you can actually spawn tasks correctly, since the calls are going to
    // come on threads created by the C lib itself and won't have a runtime
    // installed
    let runtime = tokio::runtime::Builder::new()
        .threaded_scheduler()
        .enable_all()
        .thread_name("sentry-tokio")
        .build()
        .expect("failed to create tokio runtime");

    // in this case we are creating a client just for the transport, but in
    // a real app it is likely you would have this configured for other things
    // and just reuse it for Sentry
    // if you are using proxies or custom certs with Sentry, you could also
    // configure it here, or during startup, using the options you set
    let client = Client::new();

    // actually registers our custom transport so that the SDK will use that to
    // send requests to your Sentry service, rather than the built in transports
    // that come with the SDK
    options.set_transport(Transport::new(client, runtime.handle().clone()));

    let _shutdown = options.init().expect("failed to initialize Sentry");

    Event::new().capture();
    Event::new().capture();
    Event::new().capture();

    Ok(())
}
