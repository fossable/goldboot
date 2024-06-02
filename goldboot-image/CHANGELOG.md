# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.3](https://github.com/fossable/goldboot/compare/goldboot-image-v0.0.2...goldboot-image-v0.0.3) - 2024-06-02

### Fixed
- convert_random_data test

### Other
- avoid unexpected qcow2 size
- attempt to use cross-rs image for release job
- continue liveusb command implementation

## [0.0.2](https://github.com/fossable/goldboot/compare/goldboot-image-v0.0.1...goldboot-image-v0.0.2) - 2024-05-12

### Other
- restore registry crate

## [0.0.1](https://github.com/fossable/goldboot/releases/tag/goldboot-image-v0.0.1) - 2024-03-17

### Fixed
- c_char cast on muslc
- unit tests

### Other
- initial ImageHandle request extractor
- bump MSRV
- improve archinstall config
- restore original gui functionality
- update dependencies
- attempt to format drive file properly
- prepare to generate session keys for ssh
- replace log with tracing
- replace simple_error -> anyhow
- reorganize crates
- move OVMF firmwares
- fix build errors in image crate
- reorganize crates
- Migrate goldboot-image crate into goldboot-core
- Reconnect SSH if a provisioner reboots
- Add new image write implementation
- Begin transition from qcow to custom image container
- Repair the build again
- Change formatter conventions
- Simplify image dependency
- Move templates back into core module
- Continue implementation
- Reorganize into rust workspaces
