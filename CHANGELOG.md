# Change Log

This project adheres to [Semantic Versioning](http://semver.org/).

This CHANGELOG follows the format listed at [Keep A Changelog](http://keepachangelog.com/)

## [Unreleased]

## 0.2.13 - 2022-06-03

- Fix crash when missing or none values are found
- CI Update

## 0.2.12 - 2022-05-05

- Added new parameter (--port) to change the default network port 8080
- Check / print the API version
- Update dependencies

## 0.2.11 - 2022-05-05

- Fix paramater parcing (clap)
- Update dependencies

## 0.2.10 - 2022-03-02

- Update dependencies

## 0.2.9 - 2021-02-02

- Update dependencies

## 0.2.8 - 2021-01-01

- Update dependencies

## 0.2.6 - 2021-11-29

- Added metric `volume_size_in_kb` (sizeInKb)
- Added label `vol_type (volumeType)` to all volume metrics

## 0.2.5 - 2021-08-31

- Added option for specifying the configuration path (folder) `--cfg_path`

## 0.2.4 - 2021-06-30

- Fix sds maintenance metric name (maintenanceState -> sds_state_maintenance)

## 0.2.3 - 2021-06-24

- Added more sds and device states

## 0.2.2 - 2021-06-11

- rename metric dev_state to device_state

## 0.2.1 - 2021-06-11

- Fix auth token expiration

## 0.2.0 - 2021-06-07

- Complete revamp and updated all dependencies
- Added State metrics for sds, sdc and device (<https://github.com/syepes/sio2prom/blob/master/src/sio/metrics.rs#L46>)
- Removed file configuration (sio2prom.json), it's now all done with params or environment vars
- Removed log file (log4rs.toml), it's now Stdout and managed with -v
- New docker builds for linux/amd64, linux/arm/v7 and linux/arm64 (<https://hub.docker.com/r/syepes/sio2prom/tags>)
- Tested with the last versions of PowerFlex (<=3.5)

## 0.1.3 - 2017-04-23

- If cluster name (clu_name) has not been configured use the id (clu_id) as cluster name.

## 0.1.2 - 2016-10-06

### Added

- Clippy compliant
- Grafana templates

### Changed

- Second+Third pass on handling errors correctly
- Remove unnecessary Arc/Mutex on metrics as they are already thread-safe
- Change Bandwidth metrics from Mb to Kb
- Label sorting is no longer needed: <https://github.com/pingcap/rust-prometheus/pull/73>
- Update log4rs settings

## 0.1.1 - 2016-09-25

### Changed

- First pass on handling errors correctly

### Added

- `metric_query_selection.json` More metrics
- `metrics.rs` Added ProtectionDomain metrics

## 0.1.0 - 2016-09-21

### Added

- Initial release
