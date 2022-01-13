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

- **transport-default** - **Enabled by default**, will use WinHttp on Windows
  and Curl everywhere else as the default transport.
- **backend-crashpad** - Will use Crashpad.
- **backend-breakpad** - Will use Breakpad.
- **backend-inproc** - Will use InProc.

By default the selected backend will be Crashpad for Linux, MacOS and Windows
and InProc for Android, even if no corresponding feature is active. See
[`SENTRY_BACKEND`](https://github.com/getsentry/sentry-native#compile-time-options)
for more information on backends.

## License

Licensed under either of

- Apache License, Version 2.0
  ([LICENSE-APACHE](https://github.com/daxpedda/sentry-contrib-native/blob/master/sentry-contrib-native-sys/LICENSE-APACHE)
  or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license
  ([LICENSE-MIT](https://github.com/daxpedda/sentry-contrib-native/blob/master/sentry-contrib-native-sys/LICENSE-MIT)
  or <http://opensource.org/licenses/MIT>)

at your option.

### Attribution

Used documentation from
[Sentry Native SDK](https://github.com/getsentry/sentry-native):
[MIT](https://github.com/getsentry/sentry-native/blob/master/LICENSE)

See the
[ATTRIBUTION](https://github.com/daxpedda/sentry-contrib-native/blob/master/sentry-contrib-native-sys/ATTRIBUTION)
for more details.
