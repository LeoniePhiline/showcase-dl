# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!-- next-header -->

## [Unreleased] <!-- release-date -->

### Fixed

- Keep rendering while waiting for child processes to shut down.

## [0.11.0] - 2023-10-21

### Added

- Support YouTube video and livestream player URLs as arguments.
- Add support for events without event hash.

### Fixed

- Use `clip_to_play` rather than `streamable_clip` when extracting event video URLs.

### Changed

- Update dependencies.

## [0.10.0] - 2023-09-19

### Added

- Add experimental support for Vimeo events.
  Requesting exit (Q, Esc, Ctrl-C) will now wait 5 seconds before quitting,
  to allow the downloader to mux event live streams before terminating.
  During these 5 seconds the UI freezes. In the future it will be made to keep rendering.

### Changed

- Refactor existing functionality into modules for future maintainability.

## [0.9.0] - 2023-09-14

### Added

- Handle child process exit, either terminating by signal, or exiting with non-zero status.
  Prematurely exited downloader child processes will mark the corresponding download as 'Failed'.
  Auto-retry can be added later.

### Changed

- Allow pre-TLS 1.3 connections for the time being
- Update dependencies.

## [0.8.1] - 2023-09-12

### Added

- Support Vimeo links as input. Vimeo showcase and Vimeo player URLs can now be passed as downloadable URL.
  If the target is referer restricted, use the `--referer` command line option, passing the embedding page's URL.
  Closes [#19](https://github.com/LeoniePhiline/showcase-dl/issues/19).

### Changed

- Store `downloader` and `downloader_options`  in `State` rather than pulling them all the way through the call tree.

## [0.8.0] - 2023-09-12

### Added

- Document `--downloader` command line option.

### Changed

- Update dependencies.
- Adhere to new clippy lints.
- Update showcase clips detection to latest player changes.

## [0.7.0] - 2023-07-07

### Added

- Pass all command line options after a double dash (`--`) straight to the downloader.
  This allows for [detailed configuration](https://github.com/yt-dlp/yt-dlp#general-options) of `yt-dlp`.
- Release terminal before printing error and panic stack traces.
- Add `reqwest` and `hyper` to credentials.

### Changed

- Rename command line flag `--bin` to `--downloader` to match `downloader_options`.
- Clarify logging options in [`README.md`](https://github.com/LeoniePhiline/showcase-dl/blob/main/README.md).
- Rename log file to `showcase-dl.log`.
- Minor code clean-up.
- Update locked dependencies.

### Removed

- Remove built-in `mp3` and `opus` audio extraction.
  The former behavior can be imitated by appending `yt-dlp` audio extraction options to the command line.
  E.g.: `showcase-dl <URL> -- --extract-audio --audio-format "opus/mp3" --keep-video`

### Fixed

- Print failing command in spawn error message.

## [0.6.1] - 2023-06-04

### Fixed

- Set an arbitrary user agent string to circumvent Vimeo crawler player, which does not fill the `<title />` HTMl tag correctly.
  ([#9](https://github.com/LeoniePhiline/showcase-dl/issues/9))
- Decode HTML entities in video titles.

### Changed

- Demote content dumps into 'trace' log level.
- Lift successful title match into 'info' log level.
- Use `%` / `Display` rather than `?` / `Debug` to render readable content dumps.

## [0.6.0] - 2023-06-04

### Added 

- Enable tracing [`EnvFilter`](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html).
  ([#2](https://github.com/LeoniePhiline/showcase-dl/issues/2))
- Add `cargo-release` configuration.

### Changed

- Migrate `yt-dlp` from `--referer "<URL>"` to new style `--add-header "Referer:<URL>"`
- Migrate from unmaintained `fdehau/tui` to `tui-rs-revival/ratatui`.
  Thanks [joshka](https://github.com/joshka)!
  ([#4](https://github.com/LeoniePhiline/showcase-dl/issues/4), [#5](https://github.com/LeoniePhiline/showcase-dl/pull/5))
  See also <https://github.com/fdehau/tui-rs/issues/654>
- Sort `use` groups and `mod` in a standardized fashion: ([#6](https://github.com/LeoniePhiline/showcase-dl/issues/6))
  - `use std::...`
  - `use <external>::...`
  - `use` internal
    - Relative without `self::` for submodules
    - Relative with `super::...` where in the same logical group;
      e.g. `ui/layout` uses `super::style`, as both are tightly coupled
    - Absolute with `crate::...`
  - `mod ...`
- Switch from `lazy_static` to `once_cell` until `std::sync::LazyLock` is released.
  ([#7](https://github.com/LeoniePhiline/showcase-dl/issues/7))
- Swallow `futures::future::Aborted` explicitly. ([#8](https://github.com/LeoniePhiline/showcase-dl/issues/8))

### Fixed

- Change `maybe_join` to propagate future output result. (#[3](https://github.com/LeoniePhiline/showcase-dl/issues/3))

### Removed

_(none)_

## [0.5.2] - 2023-05-03

### Changed

- Update transitive dependencies.


## [0.5.1] - 2023-02-10

### Fixed

- Progress detail extraction failed in rare cases.

## [0.5.0] - 2023-02-10

### Added

- Tracing logs are now written to `vimeo-showcase.log` and can be `tail`ed for live viewing.
- Custom patched versions of `yt-dlp` or `youtube-dl` can be used via the new `--bin` option.
- Add `CHANGELOG.md`, following [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

### Fixed

- Downloader errors are now reported with error log level.

## [0.4.0] - 2023-01-21

### Fixed

- Download progress is now correctly parsed again, after `yt-dlp` changed to a tabular format.

## [0.3.2] - 2023-01-21

### Fixed

- Command line help is now wrapped by `clap`.

### Changed

- Change command line arguments definitions to use `clap` 4 attribute macros.
- Follow `clippy` auto-deref lints.

## [0.3.1] - 2023-01-21

### Added

- Extract `mp3` and `opus` audio with `ffmeg`.

### Fixed

- Regex failed to match valid embeds.

### Changed

- Update dependencies.
- Upgrade `clap` from `3.x` to `4.1.1`.

## [0.3.0] - 2022-09-12

### Added

- Implement terminal user interface.
- Add `README.md`

### Changed

- Spawn tasks to make use of multi-threaded runtime.
- Implement graceful shutdown.

## [0.2.0] - 2022-09-07

### Added

- Implement progress tracking in shared state, preparing for terminal UI.
- Use lazy_static! for compile-once regular expressions.

## [0.1.0] - 2022-09-05

### Added

- Initial implementation.

<!-- next-url -->
[Unreleased]: https://github.com/LeoniePhiline/showcase-dl/compare/v0.11.0...HEAD
[0.11.0]: https://github.com/LeoniePhiline/showcase-dl/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/LeoniePhiline/showcase-dl/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/LeoniePhiline/showcase-dl/compare/v0.8.1...v0.9.0
[0.8.1]: https://github.com/LeoniePhiline/showcase-dl/compare/v0.8.0...v0.8.1
[0.8.0]: https://github.com/LeoniePhiline/showcase-dl/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/LeoniePhiline/showcase-dl/compare/v0.6.1...v0.7.0
[0.6.1]: https://github.com/LeoniePhiline/showcase-dl/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/LeoniePhiline/showcase-dl/compare/0.5.2...v0.6.0
[0.5.2]: https://github.com/LeoniePhiline/showcase-dl/compare/0.5.1...0.5.2
[0.5.1]: https://github.com/LeoniePhiline/showcase-dl/compare/0.5.0...0.5.1
[0.5.0]: https://github.com/LeoniePhiline/showcase-dl/compare/0.4.0...0.5.0
[0.4.0]: https://github.com/LeoniePhiline/showcase-dl/compare/0.3.2...0.4.0
[0.3.2]: https://github.com/LeoniePhiline/showcase-dl/compare/0.3.1...0.3.2
[0.3.1]: https://github.com/LeoniePhiline/showcase-dl/compare/0.3.0...0.3.1
[0.3.0]: https://github.com/LeoniePhiline/showcase-dl/compare/0.2.0...0.3.0
[0.2.0]: https://github.com/LeoniePhiline/showcase-dl/compare/0.1.0...0.2.0
[0.1.0]: https://github.com/LeoniePhiline/showcase-dl/releases/tag/0.1.0

