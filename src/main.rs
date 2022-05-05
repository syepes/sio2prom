mod sio;

use color_eyre::eyre::Result;
use std::{collections::HashMap, io::Write, path::Path, sync::Mutex, time::Duration};

#[macro_use]
extern crate log;
extern crate env_logger;

extern crate clap;
use clap::{Arg, ArgMatches, Command};

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate prometheus;
use prometheus::{GaugeVec, Histogram, HistogramOpts, IntCounterVec, IntGauge, Opts, Registry};

use warp::{Filter, Rejection, Reply};

lazy_static! {
  static ref REGISTRY: Registry = Registry::new();
  static ref HTTP_BODY_GAUGE: IntGauge = IntGauge::new("sio2prom_http_response_size_bytes", "The HTTP response sizes in bytes").expect("metric can be created");
  static ref HTTP_REQ_HISTOGRAM: Histogram = Histogram::with_opts(HistogramOpts::new("sio2prom_http_request_duration_seconds", "The HTTP request latencies in seconds")).expect("metric can be created");
  static ref UPDATE_HISTOGRAM: Histogram = Histogram::with_opts(HistogramOpts::new("sio2prom_update_duration_seconds", "The time in seconds it took to collect the stats")).expect("metric can be created");
  static ref METRIC_COUNTERS: Mutex<HashMap<String, IntCounterVec>> = Mutex::new(HashMap::new());
  static ref METRIC_GAUGES: Mutex<HashMap<String, GaugeVec>> = Mutex::new(HashMap::new());
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  color_eyre::install()?;

  let app =
    Command::new("").version(env!("CARGO_PKG_VERSION")).author(env!("CARGO_PKG_AUTHORS")).about(env!("CARGO_PKG_DESCRIPTION")).arg(Arg::new("interval").short('i').long("interval").env("INTERVAL").required(false).takes_value(true).default_value("60").help("Refresh interval in seconds")).arg(Arg::new("cfg_path").short('c').long("cfg_path").env("CFG_PATH").required(false).takes_value(true).default_value("cfg").help("Configuration path")).arg(Arg::new("ip").short('h').long("ip").env("IP").required(true).takes_value(true)).arg(Arg::new("auth_usr").short('u').long("auth_usr").env("AUTH_USR").required(true).takes_value(true)).arg(Arg::new("auth_pwd").short('p').long("auth_pwd").env("AUTH_PWD").requires("auth_usr").required(true).takes_value(true)).arg(Arg::new("v").short('v').multiple_occurrences(true).takes_value(false).required(false).help("Log verbosity (-v, -vv, -vvv...)")).get_matches();

  match app.occurrences_of("v") {
    0 => std::env::set_var("RUST_LOG", "error"),
    1 => std::env::set_var("RUST_LOG", "warn"),
    2 => std::env::set_var("RUST_LOG", "info"),
    3 => std::env::set_var("RUST_LOG", "debug"),
    4 => std::env::set_var("RUST_LOG", "trace"),
    _ => std::env::set_var("RUST_LOG", "trace"),
  }

  env_logger::Builder::from_default_env().format(|buf, record| writeln!(buf, "{} {} {}:{} [{}] - {}", chrono::Local::now().format("%Y-%m-%dT%H:%M:%S"), record.module_path().unwrap_or("unknown"), record.file().unwrap_or("unknown"), record.line().unwrap_or(0), record.level(), record.args())).init();

  if !Path::new(&app.value_of("cfg_path").unwrap()).exists() {
    error!("Config path not found: {}", app.value_of("cfg_path").unwrap());
    return Ok(());
  }

  register_metrics();
  let data_handle = tokio::task::spawn(data_collector(app));
  let metrics_route = warp::path!("metrics").and_then(metrics_handler);
  let warp_handle = warp::serve(metrics_route).run(([0, 0, 0, 0], 8080));

  info!("Started on port http://127.0.0.1:8080/metrics");
  let _ = tokio::join!(data_handle, warp_handle);
  Ok(())
}

fn register_metrics() {
  REGISTRY.register(Box::new(HTTP_BODY_GAUGE.clone())).expect("collector can be registered");
  REGISTRY.register(Box::new(HTTP_REQ_HISTOGRAM.clone())).expect("collector can be registered");
  REGISTRY.register(Box::new(UPDATE_HISTOGRAM.clone())).expect("collector can be registered");
}

async fn data_collector(app: ArgMatches) {
  let interval = app.value_of("interval").unwrap().parse::<u64>().unwrap_or(60);
  let mut collect_interval = tokio::time::interval(Duration::from_secs(interval));

  let mut sio = sio::client::ClientInfo::new(app.value_of("cfg_path"), app.value_of("ip"), app.value_of("auth_usr"), app.value_of("auth_pwd"));

  loop {
    let metrics = sio.metrics().await;
    if let Some(m) = metrics {
      let timer = UPDATE_HISTOGRAM.start_timer();
      load_metrics(&m);
      update_metrics(&m);
      timer.observe_duration();
    }

    collect_interval.tick().await;
  }
}

async fn metrics_handler() -> Result<impl Reply, Rejection> {
  let timer = HTTP_REQ_HISTOGRAM.start_timer();
  use prometheus::Encoder;
  let encoder = prometheus::TextEncoder::new();

  let mut buffer = Vec::new();
  if let Err(e) = encoder.encode(&REGISTRY.gather(), &mut buffer) {
    eprintln!("could not encode metrics: {}", e);
  };
  let mut res = match String::from_utf8(buffer.clone()) {
    Ok(v) => v,
    Err(e) => {
      eprintln!("metrics could not be from_utf8'd: {}", e);
      String::default()
    },
  };
  buffer.clear();

  let mut buffer = Vec::new();
  if let Err(e) = encoder.encode(&prometheus::gather(), &mut buffer) {
    eprintln!("could not encode prometheus metrics: {}", e);
  };
  let res_custom = match String::from_utf8(buffer.clone()) {
    Ok(v) => v,
    Err(e) => {
      eprintln!("prometheus metrics could not be from_utf8'd: {}", e);
      String::default()
    },
  };
  buffer.clear();

  res.push_str(&res_custom);
  timer.observe_duration();
  HTTP_BODY_GAUGE.set(res.len() as i64);
  Ok(res)
}

fn load_metrics(metrics: &[sio::metrics::Metric]) {
  let mut counters = METRIC_COUNTERS.lock().expect("Failed to obtain metric counter lock");
  let mut gauges = METRIC_GAUGES.lock().expect("Failed to obtain metric gauge lock");

  for m in metrics {
    let labels: Vec<&str> = m.labels.iter().map(|v| *v.0).collect::<Vec<_>>();
    let opts = Opts::new(m.name.to_string(), m.help.to_string());

    trace!("Registering metric: {} {:?} ({})", m.name, labels, m.mtype);

    if m.mtype.to_lowercase() == "counter" {
      match register_int_counter_vec!(opts, &labels) {
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
  info!("Loaded Counters: {:?}", counters.keys().collect::<Vec<_>>());
  info!("Loaded Gauges: {:?}", gauges.keys().collect::<Vec<_>>());
}

fn update_metrics(metrics: &[sio::metrics::Metric]) {
  info!("Update metrics");

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

      let metric = match c.get_metric_with(&labels) {
        Err(e) => {
          error!("The metric {} {:?} ({}) was not found in MetricFamily - {}", m.name, labels, m.mtype, e);
          continue;
        },
        Ok(m) => m,
      };

      metric.inc_by(m.value as u64);
    } else if m.mtype.to_lowercase() == "gauge" {
      let g = match gauges.get(&m.name) {
        None => {
          error!("The metric {} ({}) was not found as registered", m.name, m.mtype);
          continue;
        },
        Some(g) => g,
      };

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
