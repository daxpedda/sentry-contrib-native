#![warn(
    clippy::cargo,
    clippy::missing_docs_in_private_items,
    clippy::nursery,
    clippy::pedantic,
    missing_docs
)]

mod util;

use anyhow::Result;
use sentry::Event;
use sentry_contrib_native as sentry;
use sha1::{Digest, Sha1};
use std::fs;

#[tokio::test(flavor = "multi_thread")]
async fn options() -> Result<()> {
    util::events_success(
        Some(|options| {
            options.set_release("1.0");
            options.set_environment("production");
            options.set_distribution("release-pgo");
            options.set_ca_certs("tests/res/getsentry.pem");
            options.add_attachment("tests/res/attachment.txt");
        }),
        vec![(
            || Event::new().capture(),
            |event| {
                let release = event.release.unwrap();

                assert_eq!("<unlabeled event>", event.title);
                assert_eq!("1.0", event.tags.get("release").unwrap());
                assert_eq!("1.0", release.short_version.unwrap());
                assert_eq!("1.0", release.version.unwrap());
                assert_eq!("1.0", release.version_info.as_ref().unwrap().description);
                assert_eq!(
                    "1.0",
                    release.version_info.unwrap().version.get("raw").unwrap()
                );

                assert_eq!("production", event.tags.get("environment").unwrap());

                assert_eq!("release-pgo", event.dist.unwrap());
                assert_eq!("release-pgo", event.tags.get("dist").unwrap());

                let attachment = event.attachments.get("attachment.txt").unwrap();
                let content = fs::read_to_string("tests/res/attachment.txt").unwrap();
                let hash = hex::encode(Sha1::digest(content.as_bytes()));
                assert_eq!("attachment.txt", attachment.name);
                assert_eq!(hash, attachment.sha1);
                assert_eq!(content.len(), attachment.size);
            },
        )],
    )
    .await?;

    Ok(())
}
