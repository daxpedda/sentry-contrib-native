# sentry-contrib-native-sys

[![Crates.io](https://img.shields.io/crates/v/sentry-contrib-native-sys.svg)](https://crates.io/crates/sentry-contrib-native-sys)
[![License](https://img.shields.io/crates/l/sentry-contrib-native)](https://github.com/daxpedda/sentry-contrib-native/blob/master/LICENSE)

**[Release](https://github.com/daxpedda/sentry-contrib-native/tree/release):**
[![Build](https://github.com/daxpedda/sentry-contrib-native/workflows/CI/badge.svg?branch=release)](https://github.com/daxpedda/sentry-contrib-native/actions?query=workflow%3ACI+branch%3Arelease)
[![Docs](https://docs.rs/sentry-contrib-native-sys/badge.svg)](https://docs.rs/sentry-contrib-native-sys)

**[Master](https://github.com/daxpedda/sentry-contrib-native):**
[![Build](https://github.com/daxpedda/sentry-contrib-native/workflows/CI/badge.svg?branch=master)](https://github.com/daxpedda/sentry-contrib-native/actions?query=workflow%3ACI+branch%3Amaster)
[![Docs](https://github.com/daxpedda/sentry-contrib-native/workflows/docs/badge.svg)](https://daxpedda.github.io/sentry-contrib-native/master/doc/sentry_contrib_native_sys)

## Table of contents

- [Description](#description)
- [Crate features](#crate-features)
- [License](#license)
  - [Attribution](#attribution)

## Description

**Unofficial** FFI bindings to the
[Sentry Native SDK](https://github.com/getsentry/sentry-native) for Rust. This
crate isn't intended to be used directly, use
[sentry-contrib-native](https://crates.io/crates/sentry-contrib-native) instead.

For more details see
[sentry-contrib-native's README](https://github.com/daxpedda/sentry-contrib-native/blob/master/README.md)

## Crate features

- **backend-default** - **Enabled by default**, will use Crashpad on MacOS and
  Windows, Breakpad on Linux and InProc for Android. See `SENTRY_BACKEND` at the
  [Sentry Native SDK](https://github.com/getsentry/sentry-native).
- **default-transport** - **Enabled by default**, will use WinHttp on Windows
  and Curl everywhere else as the default transport.
- **backend-crashpad** - Will use Crashpad. See `SENTRY_BACKEND` at the
  [Sentry Native SDK](https://github.com/getsentry/sentry-native).
- **backend-breakpad** - Will use Breakpad. See `SENTRY_BACKEND` at the
  [Sentry Native SDK](https://github.com/getsentry/sentry-native).
- **backend-inproc** - Will use InProc. See `SENTRY_BACKEND` at the
  [Sentry Native SDK](https://github.com/getsentry/sentry-native).
- **nightly** - Enables full documentation through
  [`feature(external_doc)`](https://doc.rust-lang.org/unstable-book/language-features/external-doc.html).

## License

Licensed under either of

- Apache License, Version 2.0
  ([LICENSE-APACHE](https://github.com/daxpedda/sentry-contrib-native/blob/master/sentry-contrib-native-sys/LICENSE-APACHE)
  or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license
  ([LICENSE-MIT](https://github.com/daxpedda/sentry-contrib-native/blob/master/sentry-contrib-native-sys/LICENSE-MIT)
  or http://opensource.org/licenses/MIT)

at your option.

### Attribution

Used documentation from
[Sentry Native SDK](https://github.com/getsentry/sentry-native):
[MIT](https://github.com/getsentry/sentry-native/blob/master/LICENSE)

See the
[ATTRIBUTION](https://github.com/daxpedda/sentry-contrib-native/blob/master/sentry-contrib-native-sys/ATTRIBUTION)
for more details.
