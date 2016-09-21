## ScaleIO Prometheus Exporter

## Functionality
 Exposes all the selected [ScaleIO](https://store.emc.com/ScaleIO/) statistics to a [Prometheus](https://prometheus.io/) endpoint

## Features
 - 100% [Rust](http://rust-lang.org/)
 - User selectable [ScaleIO](https://store.emc.com/ScaleIO/) statistics `metric_query_selection.json`
 - [Prometheus](https://prometheus.io/) customizable metric naming `metric_definition.json`
 - Runtime logging configuration `log4rs.toml`

## Usage
    git clone https://github.com/syepes/sio2prom.git && cd sio2prom
    cargo build --release (nightly)
    mkdir -p /opt/sio2prom/
    cp target/release/sio2prom /opt/sio2prom/
    cp -r cfg /opt/sio2prom/
    cd /opt/sio2prom/
    vi cfg/sio2prom.json (ScaleIO settings)
    ./sio2prom

## Notes
Beta version still needs some work on error handling
