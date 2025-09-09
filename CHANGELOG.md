# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.9.0] - 2025-09-09

### Added
- **BroadcastChannels**: Introduced `ChannelSignal` for broadcasting messages to multiple clients.
- **Bidirectional Signals**: Added `BiDirectionalSignal` for real-time, two-way synchronization between client and server.

### Changed
- Switched internal storage to use `DashMap` for improved concurrency and performance.
- Switched to using Leptos native websockets for communication.
- Refactored and clarified feature flags and documentation in the README.
- Expanded README usage section to include both client and server-side code in a single example.

## [0.8.0] - 2025-09-06

### Changed
- Now support leptos 0.8.0


## [0.7.8] - 2024-03-25

### Changed
- Now support leptos 0.7.8
- Changed codee to 0.3


## [0.7.7] - 2024-03-02

### Changed
- Now support leptos 0.7.7

## [0.7.0-rc1] - 2024-11-16

### Changed
- Now support rc of leptos

### Fixed
- Fixed Issues with Reconnects

## [0.7.0-beta5] - 2024-09-28

### Changed
- Now support beta5 of leptos

### Fixed
- Fixed Issues with Hydration

## [0.7.0-beta4.1] - 2024-09-02

### Changed

- Use [leptos-use](https://leptos-use.rs/) instead of own client websocket implementation
