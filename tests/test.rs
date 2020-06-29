use anyhow::Result;
use reqwest::{header::HeaderMap, Client};
use sentry_contrib_native::Uuid;
use serde_derive::Deserialize;
use serde_json::Value;
use std::{convert::TryInto, env, iter::FromIterator, time::Duration};
use url::Url;

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

    // clean what we don't need: username and path
    api_url.set_username("").expect("failed to clear username");
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

/// TODO
///
/// # Errors
/// TODO
pub async fn check(uuid: Uuid) -> Result<Event> {
    // build UUID
    let mut uuid = uuid.to_string();
    uuid.retain(|c| c != '-');

    // get API token set by the user
    let token = env::var("SENTRY_TOKEN")?;

    // build our HTTP client
    let headers = HeaderMap::from_iter(Some((
        "Authorization".try_into()?,
        format!("Bearer {}", token).try_into()?,
    )));
    let client = Client::builder().default_headers(headers).build()?;

    // build API URL
    let api_url = api_url(&client).await?;
    api_url.join(&format!("{}/", uuid))?;

    // build request
    let request = client.get(api_url.clone());

    // wait for the event to arrive at Sentry first!
    tokio::time::delay_for(Duration::from_secs(10)).await;

    // get that event!
    let events = request.send().await?.json::<Vec<Event>>().await?;
    let event = events.into_iter().next().expect("no event found");

    if event.message == "" {
        eprintln!("URL: {}", api_url);
        eprintln!(
            "JSON: {}",
            client.get(api_url).send().await?.json::<Value>().await?
        );
    }

    Ok(event)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub message: String,
}
