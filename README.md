## ScaleIO / VxFlex / PowerFlex Prometheus Exporter

## Functionality

 Exposes all the selected [ScaleIO / VxFlex / PowerFlex](https://en.wikipedia.org/wiki/Dell_EMC_ScaleIO) statistics to a [Prometheus](https://prometheus.io/) endpoint

## Features

- 100% [Rust](http://rust-lang.org/)
- User definable statistics via `metric_query_selection.json` see [REST API Reference Guide](https://docs.delltechnologies.com/bundle/PF_REST_API_RG)
- [Prometheus](https://prometheus.io/) customizable metric naming via `metric_definition.json`

### Examples
<img src="https://raw.githubusercontent.com/syepes/sio2prom/master/grafana/sample_global.jpg" target="_blank" width="300">
<img src="https://raw.githubusercontent.com/syepes/sio2prom/master/grafana/sample_pool.jpg" target="_blank" width="300">
<img src="https://raw.githubusercontent.com/syepes/sio2prom/master/grafana/sample_cluster.jpg" target="_blank" width="300">
<img src="https://raw.githubusercontent.com/syepes/sio2prom/master/grafana/sample_sds.jpg" target="_blank" width="300">
<img src="https://raw.githubusercontent.com/syepes/sio2prom/master/grafana/sample_sdc.jpg" target="_blank" width="300">
<img src="https://raw.githubusercontent.com/syepes/sio2prom/master/grafana/sample_volume.jpg" target="_blank" width="300">

## Usage (Docker)

    # Locally copy the configuration files and set them up
    docker run -d --name sio2prom -h sio2prom -e IP=1.1.1.1 -e AUTH_USR=mon -p AUTH_PWD=mon syepes/sio2prom

## Usage (Built from src)

    git clone https://github.com/syepes/sio2prom.git && cd sio2prom
    cargo build --release (nightly)
    target/release/sio2prom --help

## Exposed labels

    System:           {clu_id="", clu_name=""}
    Sdr:              {clu_id="", clu_name="", sdr_id="", sdr_name=""}
    Sdc:              {clu_id="", clu_name="", sdc_id="", sdc_name=""}
    ProtectionDomain: {clu_id="", clu_name="", pdo_id="", pdo_name=""}
    Sds:              {clu_id="", clu_name="", pdo_id="", pdo_name="", sds_id="", sds_name=""}
    StoragePool:      {clu_id="", clu_name="", pdo_id="", pdo_name="", sto_id="", sto_name=""}
    Volume:           {clu_id="", clu_name="", pdo_id="", pdo_name="", sto_id="", sto_name="", vol_id="", vol_name=""}
    Device:           {clu_id="", clu_name="", pdo_id="", pdo_name="", sto_id="", sto_name="", sds_id="", sds_name="", dev_id="", dev_name="", dev_path=""}
