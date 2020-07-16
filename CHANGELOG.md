# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Added support for changing the backend.

### Changed

- Replaced `user_consent_give`, `user_consent_revoke` and `user_consent_reset`
  with `set_user_consent`.
- Renamed `user_consent_get` to `user_consent`.
- Renamed feature `default-transport` to `transport-default` and
  `custom-transport` to `transport-custom`.
- Improved naming of libraries in the documentation.
- Exclude some folders from the included Sentry Native SDK that are only
  relevant for testing from the Crates.io package. This not only reduces the
  size of the overall package, but also helps to avoid issues with Windows's
  maximum path length.

### Deprecated

### Removed

### Fixed

- Fixed thread-safety in almost all functions, they could crash the application
  otherwise or cause undefined behaviour.
- Fixed unnecessary include of the WinHttp library when the default transport is
  disabled.

### Security

## [0.1.0-rc] - 2020-07-06

### Added

- New `Map` trait that improves API of `Event::add_exception` and `set_context`.

### Changed

- Changed null-byte handling, `String`s are now cut off at the first null-byte
  position if any are found.
- Improved links to the documentation for the `master` branch.
- Improved general documentation.
- Update `vsprintf` to the new official version.
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
  https://github.com/daxpedda/sentry-contrib-native/compare/v0.1.0-rc...HEAD
[0.1.0-rc]:
  https://github.com/daxpedda/sentry-contrib-native/releases/tag/v0.1.0-rc
[0.1.0-alpha-2]:
  https://github.com/daxpedda/sentry-contrib-native/releases/tag/v0.1.0-alpha-2
[0.1.0-alpha]:
  https://github.com/daxpedda/sentry-contrib-native/releases/tag/v0.1.0-alpha
