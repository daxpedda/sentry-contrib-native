#![warn(
    clippy::cargo,
    clippy::missing_docs_in_private_items,
    clippy::nursery,
    clippy::pedantic,
    missing_docs
)]
#![cfg(crashpad)]

mod util;

use anyhow::Result;
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::fs;

#[tokio::test(flavor = "multi_thread")]
async fn crash() -> Result<()> {
    util::external_events_success(vec![("crash".into(), |event| {
        // options
        {
            let release = event.release.unwrap();

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
        }

        // breadcrumb
        {
            let breadcrumb = event
                .entries
                .get("breadcrumbs")
                .and_then(|v| v.get("values"))
                .and_then(Value::as_array)
                .and_then(|v| v.get(0))
                .and_then(Value::as_object)
                .unwrap();
            assert_eq!(
                Some("test type"),
                breadcrumb.get("type").and_then(Value::as_str)
            );
            assert_eq!(
                Some("test message"),
                breadcrumb.get("message").and_then(Value::as_str)
            );
        }

        // dylib
        {
            let libs = event
                .entries
                .get("debugmeta")
                .and_then(|v| v.get("images"))
                .and_then(Value::as_array)
                .unwrap();
            assert!(libs
                .iter()
                .any(|v| v.get("code_file").and_then(Value::as_str).unwrap()
                    == dylib::location().to_str().unwrap()));
        }

        // tag
        assert_eq!("test", event.tags.get("test-tag").unwrap());

        // extra
        assert_eq!("test", event.context.get("test tag").unwrap());

        // context
        {
            let context = event.contexts.get("test context").unwrap();
            assert_eq!("os", context.r#type);
            assert_eq!("Redox", context.get("name").unwrap());
        }

        // transcation
        assert_eq!("test transaction", event.tags.get("transaction").unwrap());

        // level
        assert_eq!("fatal", event.tags.get("level").unwrap());
    })])
    .await
}
