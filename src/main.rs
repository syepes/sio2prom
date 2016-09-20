#![allow(unused_imports,dead_code,unused_variables,unused_must_use,unused_features)]
#![feature(custom_derive, plugin, question_mark,question_mark_carrier)]
#![plugin(serde_macros)]

mod sio;

use std::collections::{HashMap, BTreeMap};
use std::fs::File;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[macro_use]
extern crate log;
extern crate log4rs;

#[macro_use]
extern crate lazy_static;

extern crate serde;
extern crate serde_json;

extern crate hyper;
use hyper::header::{Authorization, Basic, Headers, ContentType};
use hyper::mime::Mime;
use hyper::server::{Server, Request, Response};

#[macro_use]
extern crate prometheus;
use prometheus::{Opts, Collector, Registry, Counter, CounterVec, Gauge, GaugeVec, Histogram, TextEncoder, Encoder};



lazy_static! {
    static ref REPO_METRICS_COUNTER: Mutex<HashMap<String, Arc<Mutex<CounterVec>>>> = {
        println!("Init: LOAD_METRICS - COUNTER");
        Mutex::new(HashMap::new())
    };
    static ref REPO_METRICS_GAUGE: Mutex<HashMap<String, Arc<Mutex<GaugeVec>>>> = {
        println!("Init: LOAD_METRICS - GAUGE");
        Mutex::new(HashMap::new())
    };

    static ref UPDATE_HISTOGRAM: Histogram = register_histogram!(
        histogram_opts!("sio2prom_update_duration_seconds",
                        "The HTTP request latencies in seconds."
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


/// Read json file using `serde_json`
fn read_json(file: &str) -> Option<BTreeMap<String, serde_json::Value>> {
    match File::open(file) {
        Err(e) => panic!("Failed to open file: {}, {:?}", file, e.kind()),
        Ok(mut f) => {
            let mut content: String = String::new();
            f.read_to_string(&mut content).ok().expect("Error reading file");
            let j: serde_json::Value = serde_json::from_str::<serde_json::Value>(&mut content).expect(&format!("Can't deserialize json file {}", file));
            Some(j.as_object().unwrap().clone())
        },
    }
}






fn parse_stats(stats: &BTreeMap<String, serde_json::Value>) {
    println!("-- parse_stats --");

    for (key, value) in stats.iter() {
        if key != "Volume" && key != "System" && key != "ProtectionDomain" {
            continue;
        }
        if value.is_object() {
            println!("{}:", key);
            for (k, v) in value.as_object().unwrap().iter() {
                println!("\t{}: {}", k, v);
            }
        }
    }
}




fn start_exporter(ip: &str, port: &str) {
    let encoder = TextEncoder::new();
    let addr: &str = &format!("{}:{}", ip, port);
    println!("Starting Exporter @{:?}", addr);

    Server::http(addr)
        .unwrap()
        .handle(move |_: Request, mut res: Response| {
            let timer = HTTP_REQ_HISTOGRAM.start_timer();

            let metric_familys = prometheus::gather();
            let mut buffer = vec![];
            encoder.encode(&metric_familys, &mut buffer).expect("Encoder problem");
            res.headers_mut().set(ContentType(encoder.format_type().parse::<Mime>().unwrap()));
            res.send(&buffer).unwrap();

            timer.observe_duration();
            HTTP_BODY_GAUGE.set(buffer.len() as f64);
        })
        .unwrap();
}




// -> Result<(), io::Error>
fn load_prom(metrics: &Vec<sio::metrics::Metric>) {
    let mut repo_counter = REPO_METRICS_COUNTER.lock().unwrap();
    let mut repo_gauge = REPO_METRICS_GAUGE.lock().unwrap();

    for m in metrics {
        // Labels need to be sorted by value https://github.com/pingcap/rust-prometheus/blob/master/src/vec.rs#L78-L80
        let mut labels_sort = m.labels.iter().collect::<Vec<_>>();
        labels_sort.sort_by(|v1, v2| v1.1.cmp(v2.1));
        let labels: Vec<&str> = labels_sort.iter().map(|v| v.0.clone()).collect::<Vec<_>>();

        let opts = Opts::new(m.name.clone(), m.help.clone());

        // println!("Regestering metric: {:?} {:?} ({})", m.name,labels, m.mtype);

        if m.mtype.to_lowercase() == "counter" {
            match register_counter_vec!(opts, &labels) {
                // Err(e) => {  println!("Register error: {}{:?} - {}", m.name.clone(), m.labels, e); },
                Err(_) => {},
                Ok(o) => {
                    repo_counter.insert(m.name.clone().to_string(), Arc::new(Mutex::new(o)));
                },
            };
        } else if m.mtype.to_lowercase() == "gauge" {
            match register_gauge_vec!(opts, &labels) {
                // Err(e) => {  println!("Register error: {}{:?} - {}", m.name.clone(), m.labels, e); },
                Err(_) => {},
                Ok(o) => {
                    repo_gauge.insert(m.name.clone().to_string(), Arc::new(Mutex::new(o)));
                },
            };
        } else {
            println!("Unknown metric type: {:?} {:?} ({})", m.name, labels, m.mtype);
        }

    }
    println!("Loaded REPO_METRICS_COUNTER: {:?}", repo_counter.keys().collect::<Vec<_>>());
    println!("Loaded REPO_METRICS_GAUGE: {:?}", repo_gauge.keys().collect::<Vec<_>>());
}


fn updata_metrics(metrics: &Vec<sio::metrics::Metric>) {
    let repo_counter = REPO_METRICS_COUNTER.lock().unwrap();
    let repo_gauge = REPO_METRICS_GAUGE.lock().unwrap();

    for m in metrics {
        if !repo_counter.contains_key(&m.name) && !repo_gauge.contains_key(&m.name) {
            println!("Metric {} not found in repo {}", m.name, m.mtype);
            continue;
        }

        let mut labels: HashMap<&str, &str> = HashMap::new();
        for (k, v) in m.labels.iter() {
            labels.insert(k, &v);
        }

        if m.mtype.to_lowercase() == "counter" {
            let c = repo_counter.get(&m.name).unwrap().lock().unwrap();

            // println!("Updateing Metric: {:?}", c.collect());

            let metric = c.get_metric_with(&labels).unwrap();
            metric.inc_by(m.value as f64);

        } else if m.mtype.to_lowercase() == "gauge" {
            let g = repo_gauge.get(&m.name).unwrap().lock().unwrap();

            // println!("Updateing Metric: {:?}", g.collect());

            let metric = g.get_metric_with(&labels).unwrap();
            metric.set(m.value as f64);
        } else {

        }
    }
}

fn scheduler(sio: &Arc<Mutex<sio::client::Client>>, interval: Duration) -> Option<thread::JoinHandle<()>> {
    if interval == Duration::from_secs(0) {
        return None;
    }
    let sio_clone = sio.clone();
    Some(thread::spawn(move || {
        loop {
            let timer = UPDATE_HISTOGRAM.start_timer();

            println!("Start: Update Metrics");
            let metrics = sio::metrics::get_metrics(&sio_clone);
            updata_metrics(&metrics);

            timer.observe_duration();
            thread::sleep(interval);
        }
    }))
}


fn main() {
    let sio: Arc<Mutex<sio::client::Client>> = sio::client::Client::new("localhost", "admin", "admin");
    println!("connect: {:?}", sio.lock().unwrap().connect());
    // println!("stats: {:?}", sio.stats());
    // println!("instances: {:?}", sio.instances());

    let metrics = sio::metrics::get_metrics(&sio);
    // println!("instances: {:?}", sio.instances());

    // println!("metrics.size: {}", metrics.len());
    // for m in metrics { println!("{:?}", m); }

    load_prom(&metrics);

    // updata_metrics(&metrics);
    scheduler(&sio, Duration::from_millis(30000 as u64));
    start_exporter("0.0.0.0", "9898");
}
