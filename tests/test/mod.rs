#[cfg(feature = "custom-transport")]
pub mod custom_transport;
mod event;

use anyhow::{anyhow, bail, Result};
#[cfg(feature = "custom-transport")]
use custom_transport::Transport;
use event::{Event, Response};
use reqwest::{header::HeaderMap, Client};
use sentry::{Options, Uuid};
use sentry_contrib_native as sentry;
use serde_json::Value;
use std::{
    convert::TryInto,
    env,
    iter::FromIterator,
    panic::{self, AssertUnwindSafe},
    time::Duration,
};
use url::Url;

/// Number of tries to wait for Sentry to process an event. Sentry.io sometimes
/// takes really long to process those.
const NUM_OF_TRIES: i32 = 20;
/// Time between tries.
const TIME_BETWEEN_TRIES: Duration = Duration::from_secs(30);

/// Converts `SENTRY_DSN` environment variable to proper URL to Sentry API.
async fn api_url(client: &Client) -> Result<Url> {
    // build url to Sentry API
    let mut api_url = Url::parse(&env::var("SENTRY_DSN")?)?;
    // get the project ID before we drop it
    let project_id = api_url
        .path_segments()
        .and_then(|mut path| path.next())
        .expect("no projet ID found")
        .to_owned();

    // if we are connection to the official "sentry.io" server, remove the
    // "o1234.ingest." part
    if let Some(domain) = api_url.domain() {
        if domain.ends_with(".ingest.sentry.io") {
            api_url.set_host(Some("sentry.io"))?;
        }
    }

    // clean what we don't need: username, password and path
    api_url.set_username("").expect("failed to clear username");
    api_url
        .set_password(None)
        .expect("failed to clear username");
    api_url
        .path_segments_mut()
        .expect("failed to clear path")
        .clear();
    // add what we do need: "/api/0/projects/"
    let api_url = api_url.join("api/")?.join("0/")?.join("projects/")?;

    // extract organization and project slug
    let (organization_slug, project_slug) = {
        // ask the Sentry API to give us a list of all projects, they also contain
        // organization slugs
        let response = client.get(api_url.clone()).send().await?.json().await?;

        // extract them!
        slugs(&response, &project_id).expect("couldn't get project or organization slug")
    };

    // put everything together:
    // "/api/0/projects/{organization_slug}/{project_slug}/events/"
    Ok(api_url
        .join(&format!("{}/", organization_slug))?
        .join(&format!("{}/", project_slug))?
        .join("events/")?)
}

/// Extracts organization and project slug from JSON response.
fn slugs(response: &Value, id: &str) -> Option<(String, String)> {
    for project in response.as_array()? {
        let project = project.as_object()?;

        if project.get("id")?.as_str().unwrap() == id {
            return Some((
                project
                    .get("organization")?
                    .as_object()?
                    .get("slug")?
                    .as_str()?
                    .to_owned(),
                project.get("slug")?.as_str()?.to_owned(),
            ));
        }
    }

    None
}

/// Query event from Sentry service.
pub async fn event(client: Client, api_url: Url, uuid: Uuid) -> Result<(Event, Value)> {
    // build UUID
    let mut uuid = uuid.to_string();
    uuid.retain(|c| c != '-');

    // build API URL
    let api_url = api_url.join(&format!("{}/", uuid))?;

    let mut response = Value::Null;

    // we want to keep retrying until the message arrives at Sentry
    for _ in 0..NUM_OF_TRIES {
        // build request
        let request = client.get(api_url.clone());

        // wait for the event to arrive at Sentry first!
        tokio::time::delay_for(TIME_BETWEEN_TRIES).await;

        // get that event!
        response = request.send().await?.json::<Value>().await?;
        let event: Response = serde_json::from_value(response.clone())?;

        match event {
            Response::Event(event) => return Ok((event, response)),
            Response::NotFound { detail } => {
                if detail != "Event not found" {
                    bail!("unknown message")
                }
            }
        }
    }

    Err(anyhow!("[UUID]: {}\n[Timeout]: {}", uuid, response))
}

#[allow(clippy::type_complexity)]
pub async fn events(events: Vec<(fn() -> Uuid, fn(Event))>) -> Result<()> {
    let mut options = Options::new();
    options.set_debug(true);
    options.set_logger(|level, message| eprintln!("[{}]: {}", level, message));
    #[cfg(feature = "custom-transport")]
    options.set_transport(Transport::new);
    let _shutdown = options.init()?;

    let mut uuids = Vec::new();
    let mut checks = Vec::new();

    // send all events
    for (event, check) in events {
        uuids.push(event());
        checks.push(check);
    }

    // get API token set by the user
    let token = env::var("SENTRY_TOKEN")?;

    // build our HTTP client
    let headers = HeaderMap::from_iter(Some((
        "Authorization".try_into()?,
        format!("Bearer {}", token).try_into()?,
    )));
    let client = Client::builder().default_headers(headers).build()?;

    // build API URL by querying Sentry service for organization and project slug
    let api_url = api_url(&client).await?;

    // store check workers
    let mut tasks = Vec::new();

    for uuid in uuids {
        let client = client.clone();
        let api_url = api_url.clone();

        tasks.push(tokio::spawn(
            async move { event(client, api_url, uuid).await },
        ));
    }

    for (task, check) in tasks.into_iter().zip(checks) {
        let (event, response) = task.await??;

        if panic::catch_unwind(AssertUnwindSafe(|| check(event.clone()))).is_err() {
            bail!("Failed:\nEvent: {:?}\nJson: {}", event, response)
        }
    }

    Ok(())
}
