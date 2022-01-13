#[cfg(feature = "transport-custom")]
pub mod custom_transport;
pub mod event;

use anyhow::{anyhow, bail, Error, Result};
#[cfg(feature = "transport-custom")]
use custom_transport::Transport;
use event::{Attachment, Event, MinEvent};
use futures_util::{future, FutureExt};
use reqwest::{header::HeaderMap, Client, StatusCode};
use sentry::{Options, Uuid};
use sentry_contrib_native as sentry;
use serde_json::Value;
use std::{
    collections::HashMap,
    convert::TryInto,
    env,
    iter::FromIterator,
    panic::{self, AssertUnwindSafe},
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{io::AsyncWriteExt, process::Command, time};
use url::Url;

/// Number of tries to wait for Sentry to process an event. Sentry.io sometimes
/// takes really long to process those.
#[allow(dead_code)]
const NUM_OF_TRIES_SUCCESS: u32 = 20;
/// Time between tries.
#[allow(dead_code)]
const TIME_BETWEEN_TRIES_SUCCESS: Duration = Duration::from_secs(30);
/// [`NUM_OF_TRIES_SUCCESS`] for failure.
#[allow(dead_code)]
const NUM_OF_TRIES_FAILURE: u32 = 1;
/// [`TIME_BETWEEN_TRIES_SUCCESS`] for failure.
#[allow(dead_code)]
const TIME_BETWEEN_TRIES_FAILURE: Duration = Duration::from_secs(60);

/// Interface to store URL to Sentry's Web API and easily generate specific
/// endpoint URLs.
#[derive(Clone)]
struct ApiUrl {
    base: Url,
    organization_slug: String,
    project_slug: String,
}

impl ApiUrl {
    /// Converts `SENTRY_DSN` environment variable to proper API URL to Sentry's
    /// Web API.
    async fn new(client: &Client) -> Result<Self> {
        // build url to Sentry API
        let mut api_url = Url::parse(&env::var("SENTRY_DSN")?)?;
        // get the project ID before we drop it
        let project_id = api_url
            .path_segments()
            .and_then(|mut path| path.next())
            .expect("no projet ID found")
            .to_owned();

        // if we are connecting to the official "sentry.io" server, remove the
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
        let base = api_url.join("api/")?.join("0/")?;

        // extract organization and project slug
        let (organization_slug, project_slug) = {
            // ask the Sentry API to give us a list of all projects, they also contain
            // organization slugs
            let response = client
                .get(base.join("projects/")?)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;

            // extract them!
            Self::slugs(&response, &project_id).expect("couldn't get project or organization slug")
        };

        Ok(Self {
            base,
            organization_slug,
            project_slug,
        })
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

    /// Get event by UUID endpoint.
    fn event(&self, uuid: Uuid) -> Result<Url> {
        self.base
            .join("projects/")?
            .join(&format!("{}/", self.organization_slug))?
            .join(&format!("{}/", self.project_slug))?
            .join("events/")?
            .join(&format!("{}/", uuid.to_plain()))
            .map_err(Into::into)
    }

    /// Get attachments of an event by UUID endpoint.
    fn attachments(&self, uuid: Uuid) -> Result<Url> {
        self.event(uuid)?.join("attachments/").map_err(Into::into)
    }

    /// Get all issues that have the given user ID.
    fn issues(&self, user_id: &str) -> Result<Url> {
        let mut url = self
            .base
            .join("projects/")?
            .join(&format!("{}/", self.organization_slug))?
            .join(&format!("{}/", self.project_slug))?
            .join("issues/")?;
        url.query_pairs_mut()
            .append_pair("query", &format!("user.id:{}", user_id));
        url.query_pairs_mut().append_pair("statsPeriod", "24h");

        Ok(url)
    }

    /// Get all events of an issue with the given ID.
    fn events(&self, issue: &str) -> Result<Url> {
        self.base
            .join("issues/")?
            .join(&format!("{}/", issue))?
            .join("events/")
            .map_err(Into::into)
    }
}

/// Initialize [`Client`] with defaults and build [`ApiUrl`].
async fn init() -> Result<(Client, ApiUrl)> {
    // get API token set by the user
    let token = env::var("SENTRY_TOKEN")?;

    // build our HTTP client
    let headers = HeaderMap::from_iter(Some((
        "Authorization".try_into()?,
        format!("Bearer {}", token).try_into()?,
    )));
    let client = Client::builder().default_headers(headers).build()?;

    // build API URL by querying Sentry service for organization and project slug
    let api_url = ApiUrl::new(&client).await?;

    Ok((client, api_url))
}

/// Query the Web API with the given endpoint.
async fn query(
    client: &Client,
    api_url: Url,
    num_of_tries: u32,
    time_between_tries: Duration,
) -> Result<Option<Value>> {
    // we want to keep retrying until the event arrives at Sentry
    for _ in 0..num_of_tries {
        // build request
        let request = client.get(api_url.clone());

        // wait for the event to arrive at Sentry first!
        time::sleep(time_between_tries).await;

        // get that event!
        match request.send().await?.error_for_status() {
            Ok(response) => return response.json().await.map_err(Into::into),
            Err(error) => {
                if let Some(error) = error.status() {
                    match error {
                        StatusCode::NOT_FOUND | StatusCode::TOO_MANY_REQUESTS => continue,
                        _ => bail!(error),
                    }
                }

                bail!(error)
            }
        };
    }

    Ok(None)
}

#[allow(clippy::type_complexity, dead_code)]
/// Query events with the given [`Uuid`] and run given checks on them.
pub async fn events_success(
    option: Option<fn(&mut Options)>,
    events: Vec<(fn() -> Uuid, fn(Event))>,
) -> Result<()> {
    let events = events
        .into_iter()
        .map(|(event, check)| (event, move |event: Option<Event>| check(event.unwrap())))
        .collect();

    events_internal(
        option,
        events,
        NUM_OF_TRIES_SUCCESS,
        TIME_BETWEEN_TRIES_SUCCESS,
    )
    .await
}

#[allow(dead_code)]
/// Query events with the given [`Uuid`] and make sure they never arrived.
pub async fn events_failure(
    option: Option<fn(&mut Options)>,
    events: Vec<fn() -> Uuid>,
) -> Result<()> {
    let events = events
        .into_iter()
        .map(|event| (event, move |event: Option<Event>| assert!(event.is_none())))
        .collect();

    events_internal(
        option,
        events,
        NUM_OF_TRIES_FAILURE,
        TIME_BETWEEN_TRIES_FAILURE,
    )
    .await
}

/// Query events with the given [`Uuid`] and run given checks on them.
async fn events_internal(
    option: Option<fn(&mut Options)>,
    events: Vec<(fn() -> Uuid, impl Fn(Option<Event>) + 'static + Send)>,
    num_of_tries: u32,
    time_between_tries: Duration,
) -> Result<()> {
    // build the Sentry client
    let mut options = Options::new();
    options.set_debug(true);
    options.set_logger(|level, message| eprintln!("[{}]: {}", level, message));
    #[cfg(feature = "transport-custom")]
    options.set_transport(Transport::new);

    // apply custom configuration
    if let Some(option) = option {
        option(&mut options);
    }

    // start the Sentry client!
    let _shutdown = options.init()?;

    let mut uuids = Vec::new();
    let mut checks = Vec::new();

    // send all events
    for (event, check) in events {
        uuids.push(event());
        checks.push(check);
    }

    // initialize HTTP client
    let (client, api_url) = init().await?;

    // store tokio tasks
    let mut tasks = Vec::new();

    for (uuid, check) in uuids.into_iter().zip(checks) {
        let client = client.clone();
        let api_url = api_url.clone();

        tasks.push(
            tokio::spawn(async move {
                // get event from the Sentry service
                let response =
                    event(&client, api_url, uuid, num_of_tries, time_between_tries).await?;
                let event = response.clone();

                // run our checks against it
                panic::catch_unwind(AssertUnwindSafe(|| check(event))).map_err(|error| {
                    // if there was a response and the check failed dump that information in the CI
                    // if there was no response than we timed out
                    response.map_or_else(
                        || eprintln!("[Timeout]: {}", uuid),
                        |event| eprintln!("Event: {:?}", event),
                    );

                    // return that error
                    if let Ok(error) = error.downcast::<Error>() {
                        *error
                    } else {
                        anyhow!("unknown error")
                    }
                })
            })
            .map(|result| result?),
        );
    }

    // poll all tasks
    future::try_join_all(tasks).await?;

    Ok(())
}

/// Query event from Sentry service.
async fn event(
    client: &Client,
    api_url: ApiUrl,
    uuid: Uuid,
    num_of_tries: u32,
    time_between_tries: Duration,
) -> Result<Option<Event>> {
    if let Some(response) = query(
        client,
        api_url.event(uuid)?,
        num_of_tries,
        time_between_tries,
    )
    .await?
    {
        let mut event: Event = serde_json::from_value(response)?;

        if let Some(attachments) =
            query(client, api_url.attachments(uuid)?, 1, Duration::default()).await?
        {
            let mut map = HashMap::new();

            for attachment in serde_json::from_value::<Vec<Attachment>>(attachments)? {
                map.insert(attachment.name.clone(), attachment);
            }

            event.attachments = map;
            return Ok(Some(event));
        }
    }

    Ok(None)
}

#[allow(dead_code)]
pub async fn external_events_success(events: Vec<(String, fn(Event))>) -> Result<()> {
    let events = events
        .into_iter()
        .map(|(event, check)| (event, move |event: Option<Event>| check(event.unwrap())))
        .collect();

    external_events_internal(events, NUM_OF_TRIES_SUCCESS, TIME_BETWEEN_TRIES_SUCCESS).await
}

#[allow(dead_code)]
pub async fn external_events_failure(events: Vec<String>) -> Result<()> {
    let events = events
        .into_iter()
        .map(|event| (event, move |event: Option<Event>| assert!(event.is_none())))
        .collect();

    external_events_internal(events, NUM_OF_TRIES_FAILURE, TIME_BETWEEN_TRIES_FAILURE).await
}

/// Run external example in a process, feed it a user id and search for it
/// through Web API.
async fn external_events_internal(
    events: Vec<(String, impl Fn(Option<Event>) + 'static + Send)>,
    num_of_tries: u32,
    time_between_tries: Duration,
) -> Result<()> {
    let (client, api_url) = init().await?;

    // build path to example
    let example_path = PathBuf::from(env!("OUT_DIR"))
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .unwrap()
        .join("examples");

    // store check workers
    let mut tasks = Vec::new();

    for (example, check) in events {
        let client = client.clone();
        let api_url = api_url.clone();

        #[cfg(not(target_os = "windows"))]
        let example = example_path.join(example);
        #[cfg(target_os = "windows")]
        let example = example_path.join(format!("{}.exe", example));

        tasks.push(
            tokio::spawn(async move {
                // build user ID
                let id: [u8; 16] = rand::random();
                let user_id = hex::encode(id);
                let mut child = Command::new(example)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .spawn()
                    .expect("make sure to build the example first!");
                child.stdin.as_mut().unwrap().write_all(&id).await?;

                assert!(!child.wait().await?.success());

                // get event from the Sentry service
                let event = event_by_user(
                    &client,
                    api_url,
                    user_id.clone(),
                    num_of_tries,
                    time_between_tries,
                )
                .await?;

                // run our checks against it
                panic::catch_unwind(AssertUnwindSafe(|| check(event.clone()))).map_err(|error| {
                    // if there was a response and the check failed dump that information in the CI
                    // if there was no response than we timed out
                    event.map_or_else(
                        || eprintln!("[Timeout]: {}", user_id),
                        |event| eprintln!("Event: {:?}", event),
                    );

                    if let Ok(error) = error.downcast::<Error>() {
                        *error
                    } else {
                        anyhow!("unknown error")
                    }
                })
            })
            .map(|result| result?),
        );
    }

    // poll all tasks
    future::try_join_all(tasks).await?;

    Ok(())
}

/// Query event by user ID.
#[allow(dead_code)]
async fn event_by_user(
    client: &Client,
    api_url: ApiUrl,
    user_id: String,
    num_of_tries: u32,
    time_between_tries: Duration,
) -> Result<Option<Event>> {
    let mut issues = None;

    // timeout check is here because we also need to check if the response array
    // contains anything
    for _ in 0..num_of_tries {
        if let Some(Value::Array(value)) =
            query(client, api_url.issues(&user_id)?, 1, time_between_tries).await?
        {
            if value.is_empty() {
                continue;
            }

            issues = Some(value);
            break;
        }
    }

    let issues = match issues {
        None => return Ok(None),
        Some(issues) => issues,
    };

    // there should be only one issue with that user ID
    let issue = issues[0]
        .as_object()
        .unwrap()
        .get("id")
        .unwrap()
        .as_str()
        .unwrap();

    // get the event
    let events: Vec<MinEvent> = serde_json::from_value(
        query(
            client,
            api_url.events(issue)?,
            NUM_OF_TRIES_SUCCESS,
            TIME_BETWEEN_TRIES_SUCCESS,
        )
        .await?
        .unwrap(),
    )?;

    // search for the event that has the user ID
    for event in events {
        if let Some(user) = event.user {
            if let Some(id) = user.id {
                if id == user_id {
                    let uuid: [u8; 16] = hex::decode(event.event_id)?.as_slice().try_into()?;
                    let uuid = Uuid::from(uuid);
                    // we didn't get the whole event, just a minified version, query for the full
                    // one
                    return self::event(client, api_url.clone(), uuid, 1, Duration::default())
                        .await;
                }
            }
        }
    }

    Ok(None)
}
