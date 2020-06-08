# sentry-native

[![Crates.io](https://img.shields.io/crates/v/sentry-native.svg)](https://crates.io/crates/sentry-native)
[![Libraries.io](https://img.shields.io/librariesio/release/cargo/sentry-native.svg)](https://libraries.io/cargo/sentry-native)
[![Commits since](https://img.shields.io/github/commits-since/daxpedda/sentry-native/latest)](https://github.com/daxpedda/sentry-native/releases/latest)
[![Resolution](https://isitmaintained.com/badge/resolution/daxpedda/sentry-native.svg)](http://isitmaintained.com/project/daxpedda/sentry-native)
[![Issues](https://isitmaintained.com/badge/open/daxpedda/sentry-native.svg)](http://isitmaintained.com/project/daxpedda/sentry-native)
[![License](https://img.shields.io/crates/l/sentry-native)](https://github.com/daxpedda/sentry-native/blob/master/LICENSE)
[![LoC](https://tokei.rs/b1/github/daxpedda/sentry-native)](https://github.com/daxpedda/sentry-native)

**[Release](https://github.com/daxpedda/sentry-native/tree/release):**
[![Build](https://github.com/daxpedda/sentry-native/workflows/CI/badge.svg?branch=release)](https://github.com/daxpedda/sentry-native/actions?query=workflow%3ACI+branch%3Arelease)
[![Docs](https://docs.rs/sentry-native/badge.svg)](https://docs.rs/sentry-native)

**[Master](https://github.com/daxpedda/sentry-native):**
[![Build](https://github.com/daxpedda/sentry-native/workflows/CI/badge.svg?branch=master)](https://github.com/daxpedda/sentry-native/actions?query=workflow%3ACI+branch%3Amaster)
[![Docs](https://github.com/daxpedda/sentry-native/workflows/docs/badge.svg)](https://daxpedda.github.io/sentry-native/master/doc/index.html)

**Unofficial** bindings to the [Sentry Native SDK](https://github.com/getsentry/sentry-native) for Rust.

See the [Alternatives section](#alternatives) for details.

## Branches

- **[release](https://github.com/daxpedda/sentry-native/tree/release)** - For releases only.
- **[master](https://github.com/daxpedda/sentry-native)** - For active development inluding PR's.

## Usage

```rust,should_panic
use sentry_native::Options;

fn main() {
    let mut options = Options::new();
    options.set_dsn("your-sentry-dsn.com");
    let _shutdown = options.init().expect("failed to initialize sentry");

    // this code triggers a segfault
    unsafe { *(0 as *mut u32) = 42; }
}
```

## Crate features

- **test** - Corrects testing for documentation tests.
- **nightly** - Enables full documentation through [`feature(external_doc)`](https://doc.rust-lang.org/unstable-book/language-features/external-doc.html).

## Documentation

- For releases on [crates.io](https://crates.io): [![Docs](https://docs.rs/sentry-native/badge.svg)](https://docs.rs/sentry-native).
- For the master branch: [![Docs](https://github.com/daxpedda/sentry-native/workflows/docs/badge.svg)](https://daxpedda.github.io/sentry-native/master/doc/index.html).

Currently, nightly is needed for full documentation: `cargo doc --features nightly`

If you are not using nightly, use `cargo doc` as usual.

## Tests

For correct testing the following has to be provided:

- `feature = "test` has to be enabled.
- `SENTRY_DSN` environment variable has to contain a valid sentry URL.

`cargo test --features test`

## CI

This crate is checked daily by CI to make sure that it builds successfully with the newest versions of rust stable, beta and nightly.

## Alternatives

I recommend using Sentry's official SDK for rust: **[sentry](https://github.com/getsentry/sentry-rust)** - [![Crates.io](https://img.shields.io/crates/v/sentry.svg)](https://crates.io/crates/sentry).

The official SDK provides a much better user experience and customizability.

In comparison the only upside this crate can provide is application crash handling, the official SDK for rust can only handle panics.

## Changelog

See the [CHANGELOG](https://github.com/daxpedda/sentry-native/blob/master/CHANGELOG.md) file for details

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
