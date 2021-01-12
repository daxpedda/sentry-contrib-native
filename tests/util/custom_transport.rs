use anyhow::Result;
use futures_executor as executor;
use futures_util::{future::Map, FutureExt};
use reqwest::Client;
use sentry::{Dsn, Options, RawEnvelope, Transport as SentryTransport, TransportShutdown};
use sentry_contrib_native as sentry;
use std::{convert::TryInto, process, time::Duration};
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    task::{JoinError, JoinHandle},
};

type Payload = Map<JoinHandle<Result<()>>, fn(Result<Result<()>, JoinError>) -> Result<()>>;

pub struct Transport {
    dsn: Dsn,
    receiver: Receiver<Payload>,
    sender: Sender<Payload>,
    client: Client,
}

impl Transport {
    pub fn new(options: &Options) -> Result<Self, ()> {
        let dsn = options.dsn().and_then(|dsn| Dsn::new(dsn).ok()).ok_or(())?;
        let (sender, receiver) = mpsc::channel(1024);
        let client = Client::new();

        Ok(Self {
            dsn,
            receiver,
            sender,
            client,
        })
    }
}

impl SentryTransport for Transport {
    fn send(&self, envelope: RawEnvelope) {
        let dsn = self.dsn.clone();
        let sender = self.sender.clone();
        let client = self.client.clone();

        if let Err(error) = executor::block_on(async move {
            sender
                .send(
                    tokio::spawn(async move {
                        let request = envelope
                            .to_request(dsn.clone())
                            .map(|body| body.as_bytes().to_vec());
                        client
                            .execute(request.try_into().unwrap())
                            .await?
                            .error_for_status()?;

                        Ok(())
                    })
                    .map(|result| result?),
                )
                .await
        }) {
            eprintln!("{}", error);
            process::abort();
        }
    }

    fn shutdown(mut self: Box<Self>, _timeout: Duration) -> TransportShutdown {
        executor::block_on(async {
            let mut ret = TransportShutdown::Success;

            while let Some(Some(task)) = self.receiver.recv().now_or_never() {
                if let Err(error) = task.await {
                    eprintln!("{}", error);
                    ret = TransportShutdown::TimedOut;
                }
            }

            ret
        })
    }
}
