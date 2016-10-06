## ScaleIO Prometheus Exporter

## Functionality
 Exposes all the selected [ScaleIO](https://store.emc.com/ScaleIO/) statistics to a [Prometheus](https://prometheus.io/) endpoint

## Features
 - 100% [Rust](http://rust-lang.org/)
 - User selectable [ScaleIO](https://store.emc.com/ScaleIO/) statistics `metric_query_selection.json`
 - [Prometheus](https://prometheus.io/) customizable metric naming `metric_definition.json`
 - Runtime logging configuration `log4rs.toml`

### Examples
<img src="https://raw.githubusercontent.com/syepes/sio2prom/master/grafana/sample_global.jpg" width="300">
<img src="https://raw.githubusercontent.com/syepes/sio2prom/master/grafana/sample_pool.jpg" width="300">
<img src="https://raw.githubusercontent.com/syepes/sio2prom/master/grafana/sample_cluster.jpg" width="300">
<img src="https://raw.githubusercontent.com/syepes/sio2prom/master/grafana/sample_sds.jpg" width="300">
<img src="https://raw.githubusercontent.com/syepes/sio2prom/master/grafana/sample_sdc.jpg" width="300">
<img src="https://raw.githubusercontent.com/syepes/sio2prom/master/grafana/sample_volume.jpg" width="300">

## Usage
    git clone https://github.com/syepes/sio2prom.git && cd sio2prom
    cargo build --release (nightly)
    mkdir -p /opt/sio2prom/
    cp target/release/sio2prom /opt/sio2prom/
    cp -r cfg /opt/sio2prom/
    cd /opt/sio2prom/
    vi cfg/sio2prom.json (ScaleIO settings)
    ./sio2prom

## Exposed labels
    System:           {clu_id="", clu_name=""}
    Sdc:              {clu_id="", clu_name="", sdc_id="", sdc_name=""}
    ProtectionDomain: {clu_id="", clu_name="", pdo_id="", pdo_name=""}
    Sds:              {clu_id="", clu_name="", pdo_id="", pdo_name="", sds_id="", sds_name=""}
    StoragePool:      {clu_id="", clu_name="", pdo_id="", pdo_name="", sto_id="", sto_name=""}
    Volume:           {clu_id="", clu_name="", pdo_id="", pdo_name="", sto_id="", sto_name="", vol_id="", vol_name=""}
    Device:           {clu_id="", clu_name="", pdo_id="", pdo_name="", sto_id="", sto_name="", sds_id="", sds_name="", dev_id="", dev_name="", dev_path=""}

## Notes
Beta version still needs some work on error handling
