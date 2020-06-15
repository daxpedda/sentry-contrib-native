# sentry-contrib-native

[![Crates.io](https://img.shields.io/crates/v/sentry-contrib-native.svg)](https://crates.io/crates/sentry-contrib-native)
[![Libraries.io](https://img.shields.io/librariesio/release/cargo/sentry-contrib-native.svg)](https://libraries.io/cargo/sentry-contrib-native)
[![Commits since](https://img.shields.io/github/commits-since/daxpedda/sentry-contrib-native/latest)](https://github.com/daxpedda/sentry-contrib-native/releases/latest)
[![Resolution](https://isitmaintained.com/badge/resolution/daxpedda/sentry-contrib-native.svg)](http://isitmaintained.com/project/daxpedda/sentry-contrib-native)
[![Issues](https://isitmaintained.com/badge/open/daxpedda/sentry-contrib-native.svg)](http://isitmaintained.com/project/daxpedda/sentry-contrib-native)
[![License](https://img.shields.io/crates/l/sentry-contrib-native)](https://github.com/daxpedda/sentry-contrib-native/blob/master/LICENSE)
[![LoC](https://tokei.rs/b1/github/daxpedda/sentry-contrib-native)](https://github.com/daxpedda/sentry-contrib-native)

**[Release](https://github.com/daxpedda/sentry-contrib-native/tree/release):**
[![Build](https://github.com/daxpedda/sentry-contrib-native/workflows/CI/badge.svg?branch=release)](https://github.com/daxpedda/sentry-contrib-native/actions?query=workflow%3ACI+branch%3Arelease)
[![Docs](https://docs.rs/sentry-contrib-native/badge.svg)](https://docs.rs/sentry-contrib-native)

**[Master](https://github.com/daxpedda/sentry-contrib-native):**
[![Build](https://github.com/daxpedda/sentry-contrib-native/workflows/CI/badge.svg?branch=master)](https://github.com/daxpedda/sentry-contrib-native/actions?query=workflow%3ACI+branch%3Amaster)
[![Docs](https://github.com/daxpedda/sentry-contrib-native/workflows/docs/badge.svg)](https://daxpedda.github.io/sentry-contrib-native/master/doc/index.html)

## Table of contents

- [Description](#description)
- [Branches](#branches)
- [Usage](#usage)
- [Build](#build)
- [Crate features](#crate-features)
- [Deployment](#deployment)
- [Documentation](#documentation)
- [Tests](#tests)
- [Alternatives](#alternatives)
- [Changelog](#changelog)
- [License](#license)
- [Contribution](#contribution)

## Description

**Unofficial** bindings to the [Sentry Native SDK](https://github.com/getsentry/sentry-native) for Rust.
See the [Alternatives section](#alternatives) for details on the official Sentry SDK for Rust.

This crates main purpose is to enable an application to send reports to Sentry even if it crashes, which is currently not covered by the official Sentry SDK for Rust.

## Branches

- **[release](https://github.com/daxpedda/sentry-contrib-native/tree/release)** - For releases only.
- **[master](https://github.com/daxpedda/sentry-contrib-native)** - For active development inluding PR's.

## Usage

```rust,should_panic
use sentry_contrib_native as sentry;
use sentry::{Event, Level, Options};
use std::ptr;

fn main() {
    // set up panic handler
    sentry::set_hook();
    // start Sentry
    let mut options = Options::new();
    options.set_dsn("your-sentry-dsn.com");
    let _shutdown = options.init().expect("failed to initialize Sentry");

    // send an event to Sentry
    Event::new_message(Level::Debug, None, "test");

    // this code triggers a crash, but it will still be reported to Sentry
    unsafe { *ptr::null_mut() = true; }

    // Sentry receives an event with an attached stacktrace and message
    panic!("application should have crashed at this point");
}
```

On MacOS and Windows the Crashpad handler executable has to be shipped with your application, a "convenient" environment variable is provided to help with that: `DEP_SENTRY_NATIVE_HANDLER`.

Here is an example `build.rs`.

```rust,no_run
use std::{env, fs, path::Path};

static OUTPUT_PATH: &str = "your/output/path";

fn main() {
    let target_os = env::var_os("CARGO_CFG_TARGET_OS").unwrap();

    if target_os == "macos" || target_os == "windows" {
        let handler = env::var_os("DEP_SENTRY_NATIVE_HANDLER").unwrap();
        let executable = if target_os == "macos" {
            "crashpad_handler"
        } else if target_os == "windows" {
            "crashpad_handler.exe"
        } else {
            unreachable!()
        };

        fs::copy(handler, Path::new(OUTPUT_PATH).join(executable)).unwrap();
    }
}
```

## Build

This crate relies on [`sentry-contrib-native-sys`](https://crates.io/crates/sentry-contrib-native-sys) which in turn builds [Sentry's Native SDK](https://github.com/getsentry/sentry-native) for you. This requires [CMake](https://cmake.org) or alternatively you can provide a pre-installed version with the `SENTRY_NATIVE_INSTALL` environment variable.

Additionally on any non-Windows platform the development version of `curl` is required.

See [`sentry-contrib-native-sys`](https://crates.io/crates/sentry-contrib-native-sys) for more details.

## Crate features

- **test** - Corrects testing for documentation tests.
- **nightly** - Enables full documentation through [`feature(external_doc)`](https://doc.rust-lang.org/unstable-book/language-features/external-doc.html).

## Deployment

When deploying your binary for MacOS or Windows, you have to ship it together with the `crashpad_handler(.exe)` executable. A way to programmatically export it using `build.rs` is provided through the `DEP_SENTRY_NATIVE_HANDLER`.

See the [Usage section](#usage) for an example.

## Documentation

- For the bindings used: [official documentation](https://docs.sentry.io/platforms/native)
- For releases on [crates.io](https://crates.io): [![Docs](https://docs.rs/sentry-contrib-native/badge.svg)](https://docs.rs/sentry-contrib-native).
- For the master branch: [![Docs](https://github.com/daxpedda/sentry-contrib-native/workflows/docs/badge.svg)](https://daxpedda.github.io/sentry-contrib-native/master/doc/index.html).

Currently, nightly is needed for full documentation: `cargo doc --features nightly`

If you are not using nightly, use `cargo doc` as usual.

## Tests

For correct testing the following has to be provided:

- `feature = "test"` has to be enabled.
- `SENTRY_DSN` environment variable has to contain a valid sentry URL.

`cargo test --features test`

## Alternatives

It's recommended to use Sentry's official SDK for rust: **[sentry](https://github.com/getsentry/sentry-rust)** - [![Crates.io](https://img.shields.io/crates/v/sentry.svg)](https://crates.io/crates/sentry).

The official SDK provides a much better user experience and customizability.

In comparison the only upside this crate can provide is application crash handling, the official SDK for rust can only handle panics.

## Changelog

See the [CHANGELOG](https://github.com/daxpedda/sentry-contrib-native/blob/master/CHANGELOG.md) file for details.

## License

Licensed under either of

- Apache License, Version 2.0
  ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license
  ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
