#![warn(
    clippy::all,
    clippy::missing_docs_in_private_items,
    clippy::nursery,
    clippy::pedantic,
    missing_docs
)]

//! Example how to use [`reqwest`] with the feature `transport-custom`.

use anyhow::{anyhow, Result};
use parking_lot::{Condvar, Mutex};
use reqwest::Client;
use sentry::{
    Dsn, Event, Options, RawEnvelope, Request, Transport as SentryTransport, TransportShutdown,
};
use sentry_contrib_native as sentry;
use std::{convert::TryInto, str::FromStr, sync::Arc, time::Duration};
use tokio::{
    sync::mpsc::{self, Sender},
    task,
};

/// Send the request.
async fn send_sentry_request(client: &Client, request: Request) -> Result<()> {
    let request = request.map(|body| body.as_bytes().to_vec());

    let response = client
        .execute(request.try_into()?)
        .await
        .map_err(|e| anyhow!("Failed to send Sentry request: {}", e))?;

    response
        .error_for_status()
        .map_err(|e| anyhow!("Received error response from Sentry: {}", e))?;

    Ok(())
}

/// We can implement our own transport for Sentry data so that we don't pull in
/// C dependencies (COUGH OPENSSL COUGH) that we don't want.
struct Transport {
    /// Client.
    client: Client,
    /// MPSC {`Sender`].
    sender: Sender<RawEnvelope>,
    /// Shutdown helpers.
    shutdown: Arc<(Mutex<()>, Condvar)>,
}

impl Transport {
    /// Create a new [`Transport`].
    fn new(client: Client, options: &Options) -> Self {
        let (sender, mut receiver) = mpsc::channel::<RawEnvelope>(1024);
        let shutdown = Arc::new((Mutex::new(()), Condvar::new()));
        let transport = Self {
            client,
            sender,
            shutdown: shutdown.clone(),
        };
        let client = transport.client.clone();
        let dsn = Dsn::from_str(options.dsn().expect("no DSN found")).expect("invalid DSN");

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

        transport
    }
}

impl SentryTransport for Transport {
    fn send(&self, envelope: RawEnvelope) {
        let sender = self.sender.clone();
        task::spawn(async move {
            if let Err(err) = sender.send(envelope).await {
                eprintln!("failed to send envelope to send queue: {}", err);
            }
        });
    }

    fn shutdown(self: Box<Self>, timeout: Duration) -> TransportShutdown {
        // drop the sender so that the background thread will exit once
        // it has dequeued and processed all the envelopes we have enqueued
        drop(self.sender);

        // wait for the condition variable to notify that the thread has shutdown
        let (lock, cvar) = &*self.shutdown;
        let mut shutdown = lock.lock();
        let result = cvar.wait_for(&mut shutdown, timeout);

        if result.timed_out() {
            TransportShutdown::TimedOut
        } else {
            TransportShutdown::Success
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut options = sentry::Options::new();

    // setting a DSN is absolutely required to use custom transports
    options.set_dsn("https://1234abcd@your.sentry.service.com/1234");

    // in this case we are creating a client just for the transport, but in
    // a real app it is likely you would have this configured for other things
    // and just reuse it for Sentry
    // if you are using proxies or custom certs with Sentry, you could also
    // configure it here, or during startup, using the options you set
    let client = Client::new();

    // actually registers our custom transport so that the SDK will use that to
    // send requests to your Sentry service, rather than the built in transports
    // that come with the SDK
    {
        let client = client.clone();
        options.set_transport(move |options| Ok(Transport::new(client, options)));
    }

    let _shutdown = options.init().expect("failed to initialize Sentry");

    Event::new().capture();
    Event::new().capture();
    Event::new().capture();

    // it's possible to use the same `Client` for something else
    let _builder = client.post("example.com");

    Ok(())
}
