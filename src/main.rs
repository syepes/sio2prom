#![allow(unused_must_use)]
#![feature(plugin,proc_macro)]
#![cfg_attr(test, feature(plugin))]
#![cfg_attr(test, plugin(clippy))]

mod sio;

use serde_json::value::Map;
use std::{process, thread};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[macro_use]
extern crate log;
extern crate log4rs;

#[macro_use]
extern crate lazy_static;

extern crate serde;
extern crate serde_json;
extern crate serde_derive;

extern crate hyper;
use hyper::header::ContentType;
use hyper::mime::Mime;
use hyper::server::{Server, Request, Response};

#[macro_use]
extern crate prometheus;
use prometheus::{Opts, Collector, CounterVec, Gauge, GaugeVec, Histogram, TextEncoder, Encoder};



lazy_static! {
    static ref METRIC_COUNTERS: Mutex<HashMap<String, CounterVec>> = {
        Mutex::new(HashMap::new())
    };
    static ref METRIC_GAUGES: Mutex<HashMap<String, GaugeVec>> = {
        Mutex::new(HashMap::new())
    };

    static ref UPDATE_HISTOGRAM: Histogram = register_histogram!(
        histogram_opts!("sio2prom_update_duration_seconds",
                        "The time in seconds it took to collect the ScaleIO stats"
        )
    ).unwrap();

    static ref HTTP_BODY_GAUGE: Gauge = register_gauge!("sio2prom_http_response_size_bytes",
                                                        "The HTTP response sizes in bytes."
    ).unwrap();

    static ref HTTP_REQ_HISTOGRAM: Histogram = register_histogram!(
        histogram_opts!("sio2prom_http_request_duration_seconds",
                        "The HTTP request latencies in seconds."
        )
    ).unwrap();
}

fn read_cfg() -> Map<String, serde_json::Value> { sio::utils::read_json("cfg/sio2prom.json").unwrap_or_else(|| panic!("Failed to loading config")) }

fn start_exporter(ip: &str, port: u64) {
    let encoder = TextEncoder::new();
    let addr: &str = &format!("{}:{}", ip, port);
    info!("Starting exporter {:?}", addr);

    Server::http(addr)
        .expect("Could not start web server")
        .handle(move |_: Request, mut res: Response| {
            let metric_familys = prometheus::gather();
            let mut buffer = vec![];

            match encoder.encode(&metric_familys, &mut buffer) {
                Ok(_) => {
                    let timer = HTTP_REQ_HISTOGRAM.start_timer();

                    res.headers_mut().set(ContentType(encoder.format_type().parse::<Mime>().unwrap()));
                    if let Err(e) = res.send(&buffer) {
                        error!("Sending responce: {}", e);
                    }

                    timer.observe_duration();
                    HTTP_BODY_GAUGE.set(buffer.len() as f64);
                },
                Err(e) => error!("Encoder problem: {}", e),
            };
        })
        .expect("Could not spawn web server");
}


fn load_prom(metrics: &[sio::metrics::Metric]) {
    let mut counters = METRIC_COUNTERS.lock().expect("Failed to obtain metric counter lock");
    let mut gauges = METRIC_GAUGES.lock().expect("Failed to obtain metric gauge lock");

    for m in metrics {
        let labels: Vec<&str> = m.labels.iter().map(|v| *v.0).collect::<Vec<_>>();
        let opts = Opts::new(m.name.as_ref(), m.help.as_ref());

        trace!("Registering metric: {} {:?} ({})", m.name, labels, m.mtype);

        if m.mtype.to_lowercase() == "counter" {
            match register_counter_vec!(opts, &labels) {
                Err(e) => {
                    trace!("Register error: {} {:?} - {}", m.name, m.labels, e);
                },
                Ok(o) => {
                    counters.insert(m.name.clone().to_string(), o);
                },
            };
        } else if m.mtype.to_lowercase() == "gauge" {
            match register_gauge_vec!(opts, &labels) {
                Err(e) => {
                    trace!("Register error: {} {:?} - {}", m.name, m.labels, e);
                },
                Ok(o) => {
                    gauges.insert(m.name.clone().to_string(), o);
                },
            };
        } else {
            error!("Unknown metric type: {} {:?} ({})", m.name, labels, m.mtype);
        }

    }
    info!("Loaded metric Counters: {:?}", counters.keys().collect::<Vec<_>>());
    info!("Loaded metric Gauges: {:?}", gauges.keys().collect::<Vec<_>>());
}


fn updata_metrics(metrics: &[sio::metrics::Metric]) {
    let counters = METRIC_COUNTERS.lock().expect("Failed to obtain metric counter lock");
    let gauges = METRIC_GAUGES.lock().expect("Failed to obtain metric gauge lock");

    for m in metrics {
        let mut labels: HashMap<&str, &str> = HashMap::new();
        for (k, v) in &m.labels {
            labels.insert(k, v);
        }

        if m.mtype.to_lowercase() == "counter" {
            let c = match counters.get(&m.name) {
                None => {
                    error!("The metric {} ({}) was not found as registered", m.name, m.mtype);
                    continue;
                },
                Some(c) => c,
            };

            trace!("Updateing Metric: {:?}", c.collect());

            let metric = match c.get_metric_with(&labels) {
                Err(e) => {
                    error!("The metric {} {:?} ({}) was not found in MetricFamily - {}", m.name, labels, m.mtype, e);
                    continue;
                },
                Ok(m) => m,
            };

            metric.inc_by(m.value as f64);

        } else if m.mtype.to_lowercase() == "gauge" {
            let g = match gauges.get(&m.name) {
                None => {
                    error!("The metric {} ({}) was not found as registered", m.name, m.mtype);
                    continue;
                },
                Some(g) => g,
            };

            trace!("Updateing Metric: {:?}", g.collect());

            let metric = match g.get_metric_with(&labels) {
                Err(e) => {
                    error!("The metric {} {:?} ({}) was not found in MetricFamily - {}", m.name, labels, m.mtype, e);
                    continue;
                },
                Ok(m) => m,
            };

            metric.set(m.value as f64);

        } else {
            error!("Unknown metric type: {} {:?} ({})", m.name, labels, m.mtype);
        }
    }
}


fn scheduler(sio: &Arc<Mutex<sio::client::Client>>, interval: Duration) -> Option<thread::JoinHandle<()>> {
    if interval == Duration::from_secs(0) {
        return None;
    }
    let sio_clone = sio.clone();
    Some(thread::Builder::new()
        .name("scheduler".to_string())
        .spawn(move || {
            loop {
                info!("Starting scheduled metric update");

                match sio::metrics::metrics(&sio_clone) {
                    None => error!("Skipping scheduled metric update"),
                    Some(m) => {
                        let timer = UPDATE_HISTOGRAM.start_timer();
                        updata_metrics(&m);
                        timer.observe_duration();
                    },
                }

                thread::sleep(interval);
            }
        })
        .expect("Could not spawn scheduler"))
}


fn main() {
    log4rs::init_file("cfg/log4rs.toml", Default::default()).expect("Failed to initialize logger");

    let cfg = read_cfg();
    let sio_host = cfg.get("sio").and_then(|o| o.as_object().and_then(|j| j.get("host")).map(|s| s.to_string().replace('"', ""))).expect("Missing sio_host");
    let sio_user = cfg.get("sio").and_then(|o| o.as_object().and_then(|j| j.get("user")).map(|s| s.to_string().replace('"', ""))).expect("Missing sio_user");
    let sio_pass = cfg.get("sio").and_then(|o| o.as_object().and_then(|j| j.get("pass")).map(|s| s.to_string().replace('"', ""))).expect("Missing sio_pass");
    let sio_update = cfg.get("sio").and_then(|o| o.as_object().and_then(|j| j.get("metric_update")).and_then(|s| s.as_u64())).expect("Missing metric_update");
    let prom_listen_ip = cfg.get("prom").and_then(|o| o.as_object().and_then(|j| j.get("listen_ip")).map(|s| s.to_string().replace('"', ""))).expect("Missing listen_ip");
    let prom_listen_port = cfg.get("prom").and_then(|o| o.as_object().and_then(|j| j.get("listen_port")).and_then(|s| s.as_u64())).expect("Missing listen_port");

    let sio = sio::client::Client::new(sio_host, sio_user, sio_pass);

    match sio::metrics::metrics(&sio) {
        None => {
            process::exit(1);
        },
        Some(m) => load_prom(&m),
    }
    scheduler(&sio, Duration::from_secs(sio_update));

    start_exporter(prom_listen_ip.as_str(), prom_listen_port);
}
