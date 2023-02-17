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

### Changed
- Instances log to target `euphoxide::live::<name>`

### Fixed
- `!uptime` minute count

## v0.3.0 - 2023-02-11

### Added
- `bot` feature
- `euphoxide::bot` module (enable the `bot` feature to use)
- `euphoxide::Emoji` for finding, replacing and removing colon-delimited emoji in text
- `euphoxide::api::Time::new`
- `euphoxide::nick::hue`
- `euphoxide::nick::mention`
- `euphoxide::nick::normalize`
- Debug logging using the `log` crate
- `testbot_instance` example using the new `euphoxide::bot::instance::Instance`
- VSCode project settings

### Changed
- `euphoxide::conn` module redesigned and rewritten (backwards-incompatible)
- `euphoxide::nick_hue` moved to `euphoxide::nick::hue_without_removing_emoji`
- Renamed `testbot` example to `testbot_manual`

### Removed
- `euphoxide::connect` (see `euphoxide::conn::Conn::connect`)
- `euphoxide::wrap` (see `euphoxide::conn::Conn::wrap`)

## v0.2.0 - 2022-12-10

### Added
- `euphoxide::connect`

### Changed
- Updated dependencies (backwards-incompatible)

## v0.1.0 - 2022-10-23

Initial release
