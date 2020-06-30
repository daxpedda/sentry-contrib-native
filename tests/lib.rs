#![warn(
    clippy::all,
    clippy::nursery,
    clippy::missing_docs_in_private_items,
    clippy::pedantic,
    missing_docs
)]
// stable clippy seems to have an issue with await
#![allow(clippy::used_underscore_binding)]

mod test;

use anyhow::Result;
use libloading::{Library, Symbol};
use sentry::Event;
use sentry_contrib_native as sentry;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[tokio::test(threaded_scheduler)]
async fn event() -> Result<()> {
    fn lib_path() -> PathBuf {
        let mut path = PathBuf::from(env!("OUT_DIR"))
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .unwrap()
            .join("deps");

        #[cfg(windows)]
        {
            path = path.join("dylib");
        }
        #[cfg(not(windows))]
        {
            path = path.join("libdylib.so");
        }

        path
    }

    test::events(
        None,
        vec![(
            || {
                // collect libs first before we load a foreign one
                Event::new().capture();

                let lib = Library::new(lib_path()).unwrap();
                let func: Symbol<extern "C" fn() -> bool> = unsafe { lib.get(b"test\0") }.unwrap();
                assert_eq!(true, func());

                sentry::clear_modulecache();
                Event::new().capture()
            },
            |event| {
                assert_eq!("<unlabeled event>", event.title);
                assert_eq!("error", event.tags.get("level").unwrap());
                assert!(event.context.is_empty());
                assert_eq!("", event.message);
                assert_eq!(None, event.tags.get("logger"));
                let last_lib = event
                    .entries
                    .get("debugmeta")
                    .and_then(|v| v.get("images"))
                    .and_then(Value::as_array)
                    .and_then(|v| v.get(v.len() - 1))
                    .and_then(Value::as_object)
                    .unwrap();
                assert!(last_lib
                    .get("code_file")
                    .and_then(Value::as_str)
                    .unwrap()
                    .starts_with(lib_path().to_str().unwrap()));
            },
        )],
    )
    .await?;

    Ok(())
}
