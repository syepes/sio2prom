#Change Log
This project adheres to [Semantic Versioning](http://semver.org/).

This CHANGELOG follows the format listed at [Keep A Changelog](http://keepachangelog.com/)

## [Unreleased]
### Changed
- Second pass on handling errors correctly
- Remove unnecessary Arc/Mutex on metrics as they are already thread-safe
- Change Bandwidth metrics from Mb to Kb

## 0.1.1 - 2016-09-25
### Changed
- First pass on handling errors correctly

### Added
- `metric_query_selection.json` More metrics
- `metrics.rs` Added ProtectionDomain metrics

## 0.1.0 - 2016-09-21
### Added
- Initial release

