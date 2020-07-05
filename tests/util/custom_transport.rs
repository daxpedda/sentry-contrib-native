use anyhow::Result;
use futures_executor as executor;
use futures_util::{future::Map, FutureExt};
use reqwest::Client;
use sentry::{Dsn, Options, RawEnvelope, Transport as SentryTransport, TransportShutdown};
use sentry_contrib_native as sentry;
use std::{convert::TryInto, str::FromStr, time::Duration};
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    task::{JoinError, JoinHandle},
};

pub struct Transport {
    dsn: Dsn,
    receiver:
        Receiver<Map<JoinHandle<Result<()>>, fn(Result<Result<()>, JoinError>) -> Result<()>>>,
    sender: Sender<Map<JoinHandle<Result<()>>, fn(Result<Result<()>, JoinError>) -> Result<()>>>,
    client: Client,
}

impl Transport {
    pub fn new(options: &Options) -> Self {
        let dsn = Dsn::from_str(options.dsn().expect("no DSN found")).expect("invalid DSN");
        let (sender, receiver) = mpsc::channel(1024);
        let client = Client::new();

        Self {
            dsn,
            receiver,
            sender,
            client,
        }
    }
}

impl SentryTransport for Transport {
    fn send(&self, envelope: RawEnvelope) {
        let dsn = self.dsn.clone();
        let mut sender = self.sender.clone();
        let client = self.client.clone();

        tokio::spawn(async move {
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
        });
    }

    fn shutdown(mut self: Box<Self>, _timeout: Duration) -> TransportShutdown {
        executor::block_on(async {
            let mut ret = TransportShutdown::Success;

            while let Some(task) = self.receiver.recv().await {
                if let Err(error) = task.await {
                    eprintln!("{}", error);
                    ret = TransportShutdown::TimedOut;
                }
            }

            ret
        })
    }
}
