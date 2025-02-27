# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

Procedure when bumping the version number:

1. Update dependencies in a separate commit
2. Set version number in `Cargo.toml`
3. Add new section in this changelog
4. Commit with message `Bump version to X.Y.Z`
5. Create tag named `vX.Y.Z`
6. Push `master` and the new tag

## Unreleased

## v0.6.1 - 2025-02-23

### Changed

- Updated set of emoji names

### Fixed

- Nick hue hashing algorithm in some edge cases

## v0.6.0 - 2025-02-21

### Added

- `api::Time::from_timestamp`
- `api::Time::as_timestamp`
- `bot::botrulez::full_help`
- `bot::botrulez::ping`
- `bot::botrulez::short_help`
- `bot::botrulez::uptime`
- `bot::botrulez::format_relative_time`

### Changed

- **(breaking)** Switched to `jiff` from `time`
- **(breaking)** `api::Time` contents are now an `i64`
- **(breaking)** Bumped `tokio-tungstenite` dependency from `0.18` to `0.24`. If
  this causes a panic while using euphoxide, consider following the steps
  mentioned in the [tokio-tungstenite README]. If I'm reading the [rustls docs]
  correctly, it is on the users of the libraries to set the required features.
- `bot::botrulez::format_duration` now no longer mentions "since" or "ago", but
  instead has a sign (`-`) if the duration is negative.

[tokio-tungstenite README]: https://github.com/snapview/tokio-tungstenite?tab=readme-ov-file#features
[rustls docs]: https://docs.rs/rustls/0.23.19/rustls/crypto/struct.CryptoProvider.html#using-the-per-process-default-cryptoprovider

### Removed

- `api::Time::new`

## v0.5.1 - 2024-05-20

### Added

- `Emoji::load_from_json`

### Changed

- Updated set of emoji names

## v0.5.0 - 2023-12-27

### Changed

- **(breaking)** `bot::instance::ServerConfig::default` now points to `euphoria.leet.nu`
- **(breaking)** Bumped `cookie` dependency from `0.17` to `0.18`
- **(breaking)** Bumped `tokio-tungstenite` dependency from `0.18` to `0.21`
- Updated set of emoji names
- Documentation now references `euphoria.leet.nu` instead of `euphoria.io`

## v0.4.0 - 2023-05-14

### Added

- `bot::botrulez::Uptime` now implements `bot::command::Command`
- `bot::command::parse_prefix_initiated`
- `bot::commands::Commands::fallthrough`
- `bot::commands::Commands::set_fallthrough`
- `conn::Error::ConnectionTimedOut`

### Changed

- **(breaking)** `bot::command::ClapCommand::execute` now returns a `Result<bool, E>` instead of a `Result<(), E>`
- **(breaking)** `bot::command::Command::execute` now returns a `Result<bool, E>` instead of a `Result<(), E>`
- **(breaking)** `bot::commands::Commands::handle_packet` now returns a `Result<bool, E>` instead of a `Result<(), E>`
- **(breaking)** `bot::instance::Snapshot` renamed to `ConnSnapshot`
- **(breaking)** `conn::Conn::connect` now returns `conn::Result`
- `bot::instance::Instance` now implements `Clone`

### Fixed

- **(breaking)** Deserializing empty events and replies by turning unit structs into empty structs
- `phone` and `mobile` emoji
- Instances getting stuck in "Connecting" state
- Euph errors always turning into `conn::Error`s

## v0.3.1 - 2023-02-26

### Added

- `bot::botrulez::FullHelp` now implements `bot::command::Command`
- `bot::botrulez::Ping` now implements `bot::command::Command`
- `bot::botrulez::ShortHelp` now implements `bot::command::Command`
- `bot::instances::Instances::is_from_known_instance`

### Changed

- Instances log to target `euphoxide::live::<name>`
- Instances stay connected if auth is required but no password is set

### Fixed

- `!uptime` minute count
- Instance reconnecting after encountering a 404 (it now stops and logs an error)
- Instance taking too long to stop when stopped during reconnect delay

## v0.3.0 - 2023-02-11

### Added

- `bot` feature
- `bot` module (enable the `bot` feature to use)
- `Emoji` for finding, replacing and removing colon-delimited emoji in text
- `api::Time::new`
- `nick::hue`
- `nick::mention`
- `nick::normalize`
- Debug logging using the `log` crate
- `testbot_instance` example using the new `bot::instance::Instance`
- VSCode project settings

### Changed

- **(breaking)** `conn` module redesigned and rewritten
- **(breaking)** `nick_hue` moved to `nick::hue_without_removing_emoji`
- Renamed `testbot` example to `testbot_manual`

### Removed

- **(breaking)** `connect` (see `conn::Conn::connect`)
- **(breaking)** `wrap` (see `conn::Conn::wrap`)

## v0.2.0 - 2022-12-10

### Added

- `connect`

### Changed

- **(breaking)** Updated dependencies

## v0.1.0 - 2022-10-23

Initial release
