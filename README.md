# sentry-contrib-native

[![Crates.io](https://img.shields.io/crates/v/sentry-contrib-native.svg)](https://crates.io/crates/sentry-contrib-native)
[![Libraries.io](https://img.shields.io/librariesio/release/cargo/sentry-contrib-native.svg)](https://libraries.io/cargo/sentry-contrib-native)
[![Commits since](https://img.shields.io/github/commits-since/daxpedda/sentry-contrib-native/latest)](https://github.com/daxpedda/sentry-contrib-native/releases/latest)
[![License](https://img.shields.io/crates/l/sentry-contrib-native)](https://github.com/daxpedda/sentry-contrib-native/blob/master/LICENSE)
[![LoC](https://tokei.rs/b1/github/daxpedda/sentry-contrib-native)](https://github.com/daxpedda/sentry-contrib-native)

**[Release](https://github.com/daxpedda/sentry-contrib-native/tree/release):**
[![Build](https://github.com/daxpedda/sentry-contrib-native/workflows/CI/badge.svg?branch=release)](https://github.com/daxpedda/sentry-contrib-native/actions?query=workflow%3ACI+branch%3Arelease)
[![Docs](https://docs.rs/sentry-contrib-native/badge.svg)](https://docs.rs/sentry-contrib-native)

**[Master](https://github.com/daxpedda/sentry-contrib-native):**
[![Build](https://github.com/daxpedda/sentry-contrib-native/workflows/CI/badge.svg?branch=master)](https://github.com/daxpedda/sentry-contrib-native/actions?query=workflow%3ACI+branch%3Amaster)
[![Docs](https://github.com/daxpedda/sentry-contrib-native/workflows/docs/badge.svg)](https://daxpedda.github.io/sentry-contrib-native/master/doc/sentry_contrib_native)

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
  - [Attribution](#attribution)

## Description

**Unofficial** bindings to the
[Sentry Native SDK](https://github.com/getsentry/sentry-native) for Rust. See
the [Alternatives section](#alternatives) for details on the official Sentry SDK
for Rust.

This crates main purpose is to enable an application to send reports to Sentry
even if it crashes, which is currently not covered by the official Sentry SDK
for Rust.

## Branches

- **[release](https://github.com/daxpedda/sentry-contrib-native/tree/release)** -
  For releases only.
- **[master](https://github.com/daxpedda/sentry-contrib-native)** - For active
  development inluding PR's.

## Usage

```rust,should_panic
use sentry_contrib_native as sentry;
use sentry::{Event, Level, Options};
use std::ptr;

fn main() {
    // set up panic handler
    sentry::set_hook(None, None);
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

By default, on Linux, MacOS and Windows the Crashpad handler executable has to
be shipped with the application, for convenience the Crashpad handler executable
will be copied to Cargo's default binary output folder, so using `cargo run`
works without any additional setup or configuration.

If you need to export the Crashpad handler executable programmatically to a
specific output path, a "convenient" environment variable is provided to help
with that: `DEP_SENTRY_NATIVE_CRASHPAD_HANDLER`.

Here is an example `build.rs`.

```rust,no_run
use std::{env, fs, path::Path};

static OUTPUT_PATH: &str = "your/output/path";

fn main() {
    let target_os = env::var_os("CARGO_CFG_TARGET_OS").unwrap();

    let handler = env::var_os("DEP_SENTRY_NATIVE_CRASHPAD_HANDLER").unwrap();
    let executable = if target_os == "windows" {
        "crashpad_handler.exe"
    } else {
        "crashpad_handler"
    };

    fs::copy(handler, Path::new(OUTPUT_PATH).join(executable)).unwrap();
}
```

If you are using `panic = abort` make sure to let the panic handler call
`shutdown` to flush remaining transport before aborting the application.

```rust
use sentry_contrib_native as sentry;

std::panic::set_hook(Box::new(|_| sentry::shutdown()));
// or with the provided hook
sentry::set_hook(None, Some(Box::new(|_| sentry::shutdown())));
```

## Platform support

Currently the following systems are tested with CI:

- x86_64-unknown-linux-gnu
- x86_64-apple-darwin
- aarch64-apple-darwin (building only)
- x86_64-pc-windows-msvc

See the [CI itself](https://github.com/daxpedda/sentry-contrib-native/actions)
for more detailed information. See the
[Sentry Native SDK](https://github.com/getsentry/sentry-native#platform-and-feature-support)
for more platform and feature support details there, this crate doesn't do
anything fancy, so we mostly rely on `sentry-native` for support.

The default backend for Linux is changed from Breakpad to Crashpad.

The default transport for Android is changed from none to Curl.

The default behaviour of including the system shared zlib is disabled and
instead built from source.

Only the default backend is tested in the CI.

## Build

This crate relies on
[`sentry-contrib-native-sys`](https://crates.io/crates/sentry-contrib-native-sys)
which in turn builds
[Sentry's Native SDK](https://github.com/getsentry/sentry-native). This requires
[CMake](https://cmake.org).

Alternatively a path to a pre-built version of
[Sentry's Native SDK](https://github.com/getsentry/sentry-native) can be
provided with the `SENTRY_NATIVE_INSTALL` environment variable. If none is found
at that path,
[`sentry-contrib-native-sys`](https://crates.io/crates/sentry-contrib-native-sys)
will use that path as the build output.

Additionally, if the `transport-default` feature on Android, Linux and MacOS is
used, the development version of Curl is required. For example on Ubuntu you can
use the `libcurl4-openssl-dev` package.

See the
[Sentry Native SDK](https://github.com/getsentry/sentry-native#building-and-installation)
for more details.

## Crate features

- **transport-default** - **Enabled by default**, will use WinHttp on Windows
  and Curl everywhere else as the default transport.
- **backend-crashpad** - Will use Crashpad.
- **backend-breakpad** - Will use Breakpad.
- **backend-inproc** - Will use InProc.
- **transport-custom** - Adds helper types and methods to custom transport.

By default the selected backend will be Crashpad for Linux, MacOS and Windows
and InProc for Android, even if no corresponding feature is active. See
[`SENTRY_BACKEND`](https://github.com/getsentry/sentry-native#compile-time-options)
for more information on backends.

## Deployment

If the Crashpad backend is used, which is the default on Linux, MacOS or
Windows, the application has to be shipped together with the
`crashpad_handler(.exe)` executable. A way to programmatically export it using
`build.rs` is provided through the `DEP_SENTRY_NATIVE_CRASHPAD_HANDLER`.

See the [Usage section](#usage) for an example.

## Documentation

- For the bindings used:
  [official documentation](https://docs.sentry.io/platforms/native)
- For releases on [crates.io](https://crates.io):
  [![Docs](https://docs.rs/sentry-contrib-native/badge.svg)](https://docs.rs/sentry-contrib-native).
- For the master branch:
  [![Docs](https://github.com/daxpedda/sentry-contrib-native/workflows/docs/badge.svg)](https://daxpedda.github.io/sentry-contrib-native/master/doc/index.html).

## Tests

For correct testing the following has to be provided:

- `SENTRY_DSN` environment variable has to contain a valid Sentry DSN URL.
- `SENTRY_TOKEN` environment variable has to contain a valid Sentry API Token
  with read access to "Organization", "Project" and "Issue & Event".

Tests may easily exhaust large number of events and you may not want to expose a
Sentry API token, therefore it is recommended to run tests against a
[Sentry onpremise server](https://github.com/getsentry/onpremise), it is quiet
easy to set up.

The hidden cargo feature `test` is automatically activated when testing. It has
the following effects:

- Automatically sets the DSN to the `SENTRY_DSN` environment variable, no matter
  what is set through `Options::set_dsn`.
- Automatically sets the database path to the `OUT_DIR` environment variable, no
  matter what is set through `Options::set_database_path`.
- Automatically puts the crashpad handler path to the correct path, taking into
  account `SENTRY_NATIVE_INSTALL`, no matter what is set through
  `Options::set_handler_path`.

`cargo test`

## Alternatives

It's recommended to use Sentry's official SDK for Rust:
**[sentry](https://github.com/getsentry/sentry-rust)** -
[![Crates.io](https://img.shields.io/crates/v/sentry.svg)](https://crates.io/crates/sentry).

The official SDK provides a much better user experience and customizability.

In comparison the only upside this crate can provide is application crash
handling, the official SDK for Rust can only handle panics.

## Changelog

See the
[CHANGELOG](https://github.com/daxpedda/sentry-contrib-native/blob/master/CHANGELOG.md)
file for details.

## License

Licensed under either of

- Apache License, Version 2.0
  ([LICENSE-APACHE](https://github.com/daxpedda/sentry-contrib-native/blob/master/LICENSE-APACHE)
  or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license
  ([LICENSE-MIT](https://github.com/daxpedda/sentry-contrib-native/blob/master/LICENSE-MIT)
  or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

### Attribution

Used documentation from
[Sentry Native SDK](https://github.com/getsentry/sentry-native):
[MIT](https://github.com/getsentry/sentry-native/blob/master/LICENSE)

See the
[ATTRIBUTION](https://github.com/daxpedda/sentry-contrib-native/blob/master/ATTRIBUTION)
file for more details.
