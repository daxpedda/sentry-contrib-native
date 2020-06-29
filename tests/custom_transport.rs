use parking_lot::{Condvar, Mutex};
use reqwest::Client;
use sentry::{Dsn, Options, RawEnvelope, Request, Transport as SentryTransport, TransportShutdown};
use sentry_contrib_native as sentry;
use std::{str::FromStr, sync::Arc, time::Duration};
use tokio::{
    sync::mpsc::{self, Sender},
    task,
};

async fn send_sentry_request(client: &Client, request: Request) {
    let (parts, body) = request.into_parts();
    let request = client.post(&parts.uri.to_string());

    let response = request
        .headers(parts.headers)
        .body(body.as_bytes().to_vec())
        .send()
        .await
        .expect("failed to send Sentry request");

    response
        .error_for_status()
        .expect("received error response from Sentry");
}

pub struct Transport {
    sender: Sender<RawEnvelope>,
    shutdown: Arc<(Mutex<()>, Condvar)>,
}

impl Transport {
    pub fn new(options: &Options) -> Self {
        let (sender, mut receiver) = mpsc::channel::<RawEnvelope>(1024);
        let shutdown = Arc::new((Mutex::new(()), Condvar::new()));
        let transport = Self {
            sender,
            shutdown: shutdown.clone(),
        };
        let dsn = Dsn::from_str(options.dsn().expect("no DSN found")).expect("invalid DSN");

        tokio::spawn(async move {
            let client = Client::new();

            // dequeue and send events until we are asked to shut down
            while let Some(envelope) = receiver.recv().await {
                let req = envelope.to_request(dsn.clone());
                send_sentry_request(&client, req).await;
            }

            let (lock, cvar) = &*shutdown;
            let _shutdown_lock = lock.lock();
            cvar.notify_one();
        });

        transport
    }
}

impl SentryTransport for Transport {
    fn send(&self, envelope: RawEnvelope) {
        let mut sender = self.sender.clone();
        task::spawn(async move {
            sender
                .send(envelope)
                .await
                .expect("failed to send envelope to send queue");
        });
    }

    fn shutdown(self: Box<Self>, timeout: Duration) -> TransportShutdown {
        drop(self.sender);

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
