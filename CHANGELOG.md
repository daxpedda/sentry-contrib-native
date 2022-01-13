# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

### Changed

- Updated `rmpv` to 1.0.0

### Deprecated

### Removed

### Fixed

- Fixed compilation on aarch64-unknown-linux-gnu

### Security

## [0.3.1] - 2021-04-23

### Changed

- Updated `sentry-native` to 0.4.9.

### Fixed

- Improved README.
- Fixed cross-compilation from x86_64-apple-darwin to aarch64-apple-darwin

## [0.3.0] - 2021-02-08

### Added

- Added `Options::set_max_breadcrumbs` and `Options::max_breadcrumbs`.

### Changed

- Changed `zlib` for Crashpad to always build from source.
- Updated `sentry-native` to 0.4.7.
- Changed `Error::ProjectID` to `Error::ProjectId`.

## [0.2.1] - 2021-01-21

### Added

- Added `reinstall_backend`.
- Enabled Breakpad support for MacOS.
- Official support for the `aarch64-apple-darwin` target was added, but
  currently untested in CI.

### Changed

- Updated `sentry-native` to 0.4.5.
- Removed minimum supported Windows SDK requirement.
- Removed internal global lock.

### Fixed

- Improved README.
- Fixed cross-compiling for MSVC with `crt-static`.

## [0.2.0] - 2021-01-19

### Added

- Added `modules_list`, `Options::set_transport_thread_name` and
  `Options::transport_thread_name`.
- Added error messages to `#[must_use]` cases when appropriate.

### Changed

- Updated `sentry-native` to 0.4.4.
- Updated `rand` to 0.8.
- Updated `tokio` to 1.
- Updated `reqwest` to 0.11.
- The minimum supported Windows SDK is version 1903 (10.0.18362.1) now.

### Removed

- Removed `Uuid::new`, as there is no use case for it.
- Removed `feature = "test"`, this is now an implementation detail and is
  automatically activated when `cargo test` is used.

### Fixed

- Fixed typos and improved general documentation.
- Fixed cross-compiling for MSVC with `crt-static`.
- Fix Android build.

## [0.1.0] - 2020-08-18

### Added

- Added support for changing the backend.
- Added support for Android.
- Added support for userdata for `Options::set_logger` through the `Logger`
  trait.
- Added `Options::set_auto_session_tracking` and
  `Options::auto_session_tracking`.
- Added missing documentation for `session_start` and `session_end`.

### Changed

- Replaced `user_consent_give`, `user_consent_revoke` and `user_consent_reset`
  with `set_user_consent`.
- Renamed `user_consent_get` to `user_consent`.
- Renamed feature `default-transport` to `transport-default` and
  `custom-transport` to `transport-custom`.
- Updated `sentry-native` to 0.4.0.
- Changed the default backend for Linux to Crashpad.
- Changed the default transport for Android to Curl.
- Changed `set_transport`'s `startup` argument to return `Result` and fail
  `Options::init` if `Err` is returned.

### Fixed

- Fixed thread-safety in almost all functions that could otherwise crash the
  application or cause undefined behaviour.
- Improved naming of libraries in the documentation.
- Exclude some folders from the included Sentry Native SDK that are only
  relevant for testing from the Crates.io package. This not only reduces the
  size of the overall package, but also helps to avoid issues with Windows's
  maximum path length.
- Improved README.
- Fixed unnecessary include of the WinHttp library when the default transport is
  disabled.
- Fixed `set_http_proxy` documentation to state that the full scheme is
  required.
- Fixed `Transport::send` documentation to state that envelopes have to be sent
  in order for sessions to work.

## [0.1.0-rc] - 2020-07-06

### Added

- New `Map` trait that improves API of `Event::add_exception` and `set_context`.

### Changed

- Changed null-byte handling, `String`s are now cut off at the first null-byte
  position if any are found.
- Improved links to the documentation for the `master` branch.
- Improved general documentation.
- Updated `vsprintf` to the new official version.
- Improved `custom-transport` example.

### Fixed

- Fixed `custom-transport` example which was crashing because of a
  use-after-free.
- Corrected `set_tag` and `remove_tag` examples.

## [0.1.0-alpha-2] - 2020-07-01

### Fixed

- Fixed some issues with the documentation.

## [0.1.0-alpha] - 2020-07-01

### Added

- Initial release.

[unreleased]:
  https://github.com/daxpedda/sentry-contrib-native/compare/0.3.1...HEAD
[0.3.1]: https://github.com/daxpedda/sentry-contrib-native/releases/tag/0.3.1
[0.3.0]: https://github.com/daxpedda/sentry-contrib-native/releases/tag/0.3.0
[0.2.1]: https://github.com/daxpedda/sentry-contrib-native/releases/tag/0.2.1
[0.2.0]: https://github.com/daxpedda/sentry-contrib-native/releases/tag/0.2.0
[0.1.0]: https://github.com/daxpedda/sentry-contrib-native/releases/tag/0.1.0
[0.1.0-rc]:
  https://github.com/daxpedda/sentry-contrib-native/releases/tag/0.1.0-rc
[0.1.0-alpha-2]:
  https://github.com/daxpedda/sentry-contrib-native/releases/tag/0.1.0-alpha-2
[0.1.0-alpha]:
  https://github.com/daxpedda/sentry-contrib-native/releases/tag/0.1.0-alpha
