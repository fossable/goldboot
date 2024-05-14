# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.4](https://github.com/fossable/goldboot/compare/goldboot-v0.0.3...goldboot-v0.0.4) - 2024-05-14

### Fixed
- fetch latest ArchLinux ISO by default
- missing alpine packages (closes [#192](https://github.com/fossable/goldboot/pull/192))

### Other
- add --no-accel option to disable hardware acceleration check

## [0.0.3](https://github.com/fossable/goldboot/compare/goldboot-v0.0.2...goldboot-v0.0.3) - 2024-05-12

### Other
- enable nix os
- re-enable goldboot linux
- rename write subcommand -> deploy
- update readme
- continue windows 11 support
- begin windows 11 support
- Autounattend.xml handling on Windows
- refresh windows 10
- refresh alpine linux
- restore registry crate
- use openssl vendored

## [0.0.2](https://github.com/fossable/goldboot/compare/goldboot-v0.0.1...goldboot-v0.0.2) - 2024-03-17

### Fixed
- unit tests

### Other
- initial ImageHandle request extractor
- bump MSRV
- replace old http server with axum
- improve interactive init
- improve archinstall config
- initial integration of archinstall
- allow image id or path to locate images for write command
- restore original gui functionality
- move examples to separate repo
- re-enable windows
- update readme
- update readme
- revert rustls to avoid build errors
- enable bindgen for aws-lc-sys
- Cargo feature to enable bundled OVMF firmware
- dockerhub login
- switch vga driver for debian
- quit screen wait if nothing changes for 10 mins
- better log messages on exit
- try pacstrap with -K
- wait for pacman-init service to load keyring before install
- try to avoid blocking on pacstrap install
- update debian preseed
- merge registry into main crate
- allow default_source to use image arch
- update dependencies
- add initial Dockerfile
- update default arch linux mirror
- save to image library correctly
- bump sshdog to fix upload path issue
- fix ssh key generator
- convert SSH public key format
- fix join -> with_extension
- pass ssh keys via file instead of environment
- attempt to format drive file properly
- generate ssh key for session
- prepare to generate session keys for ssh
- update contributing
- update debian mold
- replace log with tracing
- reduce errors in gui feature
- fix remaining build errors
- new ImageElement struct
- continue cleanup
- move molds module (again)
- replace simple_error -> anyhow
- add enum dispatch for fabricators too
- use enum dispatch in image mold
- more renames
- continue cleanup
- reorganize crates (again)
- reorganize crates
- move OVMF firmwares
- fix build errors in image crate
- reorganize crates
- move template icons
- continue cleanup
- reorganize templates
- remove goldboot-graphics crate
- begin to reduce compile errors
- back to standard formatting
- Bump clap from 3.2.17 to 4.0.26 ([#78](https://github.com/fossable/goldboot/pull/78))
- Begin yaml conversion
- Consolidate generic provisioners
- begin to improve template structure
- continue with interactive prompts
- Extract command handlers into new modules
- Enable RUST_BACKTRACE and RUST_LOG for GBL build
- Fix remaining image test
- Remove vault section and add directory section to image format
- Fix one of the image tests
- Migrate core crate into CLI crate
