mod sio;
use color_eyre::eyre::Result;
use std::{collections::HashMap, io::Write, path::Path, process::exit, time::Duration};
use tokio::sync::Mutex;

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
  static ref TOKIO_INSTRUMENTED_COUNT: IntGauge = IntGauge::new("sio2prom_tokio_instrumented_count", "The number of tasks instrumented").expect("metric can be created");
  static ref TOKIO_DROPPED_COUNT: IntGauge = IntGauge::new("sio2prom_tokio_dropped_count", "The number of tasks dropped").expect("metric can be created");
  static ref TOKIO_FIRST_POLL_COUNT: IntGauge = IntGauge::new("sio2prom_tokio_first_poll_count", "The number of tasks polled for the first time").expect("metric can be created");
  static ref TOKIO_TOTAL_IDLE_COUNT: IntGauge = IntGauge::new("sio2prom_tokio_total_idle_count", "The total number of times that tasks idled, waiting to be awoken").expect("metric can be created");
  static ref TOKIO_TOTAL_IDLE_DURATION: IntGauge = IntGauge::new("sio2prom_tokio_total_idle_duration_ms", "The total duration that tasks idled").expect("metric can be created");
  static ref TOKIO_TOTAL_SCHEDULED_COUNT: IntGauge = IntGauge::new("sio2prom_tokio_total_scheduled_count", "The total number of times that tasks were awoken (and then, presumably, scheduled for execution)").expect("metric can be created");
  static ref TOKIO_TOTAL_SCHEDULED_DURATION: IntGauge = IntGauge::new("sio2prom_tokio_total_scheduled_duration_ms", "The total duration that tasks spent waiting to be polled after awakening").expect("metric can be created");
  static ref TOKIO_TOTAL_POLL_COUNT: IntGauge = IntGauge::new("sio2prom_tokio_total_poll_count", "The total number of times that tasks were polled").expect("metric can be created");
  static ref TOKIO_TOTAL_POLL_DURATION: IntGauge = IntGauge::new("sio2prom_tokio_total_poll_duration_ms", "The total duration elapsed during polls").expect("metric can be created");
  static ref TOKIO_TOTAL_FAST_POLL_COUNT: IntGauge = IntGauge::new("sio2prom_tokio_total_fast_poll_count", "The total number of times that polling tasks completed swiftly").expect("metric can be created");
  static ref TOKIO_TOTAL_FAST_POLL_DURATION: IntGauge = IntGauge::new("sio2prom_tokio_total_fast_poll_duration_ms", "The total duration of fast polls").expect("metric can be created");
  static ref TOKIO_TOTAL_SLOW_POLL_COUNT: IntGauge = IntGauge::new("sio2prom_tokio_total_slow_poll_count", "The total number of times that polling tasks completed slowly").expect("metric can be created");
  static ref TOKIO_TOTAL_SLOW_POLL_DURATION: IntGauge = IntGauge::new("sio2prom_tokio_total_slow_poll_duration_ms", "The total duration of slow polls").expect("metric can be created");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error+Send+Sync>> {
  color_eyre::install()?;

  let app = Command::new("").version(env!("CARGO_PKG_VERSION"))
                            .author(env!("CARGO_PKG_AUTHORS"))
                            .about(env!("CARGO_PKG_DESCRIPTION"))
                            .arg(Arg::new("refresh").short('r').long("refresh").env("REFRESH").required(false).num_args(1).default_value("60").help("Refresh interval in seconds"))
                            .arg(Arg::new("cfg_path").short('c').long("cfg_path").env("CFG_PATH").required(false).num_args(1).default_value("cfg").help("Configuration path"))
                            .arg(Arg::new("port").long("port").env("PORT").required(false).num_args(1).default_value("8080").help("Metric listening port"))
                            .arg(Arg::new("ip").short('i').long("ip").env("IP").required(true).num_args(1).help("Gateway IP"))
                            .arg(Arg::new("auth_usr").short('u').long("auth_usr").env("AUTH_USR").required(true).num_args(1).help("Gateway Username"))
                            .arg(Arg::new("auth_pwd").short('p').long("auth_pwd").env("AUTH_PWD").requires("auth_usr").required(true).num_args(1).help("Gateway Password"))
                            .arg(Arg::new("v").short('v').action(clap::ArgAction::Count).required(false).help("Log verbosity (-v, -vv, -vvv...)"))
                            .get_matches();

  match app.get_one::<u8>("v").unwrap() {
    0 => std::env::set_var("RUST_LOG", "error"),
    1 => std::env::set_var("RUST_LOG", "warn"),
    2 => std::env::set_var("RUST_LOG", "info"),
    3 => std::env::set_var("RUST_LOG", "debug"),
    4 => std::env::set_var("RUST_LOG", "trace"),
    _ => std::env::set_var("RUST_LOG", "trace"),
  }

  env_logger::Builder::from_default_env().format(|buf, record| writeln!(buf, "{} {} {}:{} [{}] - {}", chrono::Local::now().format("%Y-%m-%dT%H:%M:%S"), record.module_path().unwrap_or("unknown"), record.file().unwrap_or("unknown"), record.line().unwrap_or(0), record.level(), record.args())).init();

  if !Path::new(&app.get_one::<String>("cfg_path").unwrap()).exists() {
    error!("Config path not found: {}", app.get_one::<String>("cfg_path").unwrap());
    exit(1);
  }

  let port = &app.get_one::<String>("port").and_then(|s| s.parse::<u16>().ok());
  if port.is_none() {
    error!("The specified port is not valid ({})", &app.get_one::<String>("port").unwrap());
    exit(1);
  }

  register_metrics();

  let monitor = tokio_metrics::TaskMonitor::new();
  let monitor_data = monitor.clone();
  let data_handle = tokio::task::spawn(async move {
    monitor_data.instrument(data_collector(app)).await;
  });

  let monitor_tokio = monitor.clone();
  let tokio_handle = tokio::spawn(async move {
    for metrics in monitor_tokio.intervals() {
      trace!("TaskMetrics: {:?}", metrics);
      TOKIO_INSTRUMENTED_COUNT.set(metrics.instrumented_count as i64);
      TOKIO_DROPPED_COUNT.set(metrics.dropped_count as i64);
      TOKIO_FIRST_POLL_COUNT.set(metrics.first_poll_count as i64);
      TOKIO_TOTAL_IDLE_COUNT.set(metrics.total_idled_count as i64);
      TOKIO_TOTAL_IDLE_DURATION.set(metrics.total_idle_duration.as_millis() as i64);
      TOKIO_TOTAL_SCHEDULED_COUNT.set(metrics.total_scheduled_count as i64);
      TOKIO_TOTAL_SCHEDULED_DURATION.set(metrics.total_scheduled_duration.as_millis() as i64);
      TOKIO_TOTAL_POLL_COUNT.set(metrics.total_poll_count as i64);
      TOKIO_TOTAL_POLL_DURATION.set(metrics.total_poll_duration.as_millis() as i64);
      TOKIO_TOTAL_FAST_POLL_COUNT.set(metrics.total_fast_poll_count as i64);
      TOKIO_TOTAL_FAST_POLL_DURATION.set(metrics.total_fast_poll_duration.as_millis() as i64);
      TOKIO_TOTAL_SLOW_POLL_COUNT.set(metrics.total_slow_poll_count as i64);
      TOKIO_TOTAL_SLOW_POLL_DURATION.set(metrics.total_slow_poll_duration.as_millis() as i64);
      tokio::time::sleep(std::time::Duration::from_millis(3000)).await;
    }
  });

  let metrics_route = warp::path!("metrics").and_then(metrics_handler);
  let warp_handle = warp::serve(metrics_route).run(([0, 0, 0, 0], port.unwrap()));

  info!("Started on port http://127.0.0.1:{}/metrics", port.unwrap());
  let _ = tokio::join!(tokio_handle, data_handle, warp_handle);
  Ok(())
}

fn register_metrics() {
  REGISTRY.register(Box::new(HTTP_BODY_GAUGE.clone())).expect("collector can be registered");
  REGISTRY.register(Box::new(HTTP_REQ_HISTOGRAM.clone())).expect("collector can be registered");
  REGISTRY.register(Box::new(UPDATE_HISTOGRAM.clone())).expect("collector can be registered");
  REGISTRY.register(Box::new(TOKIO_INSTRUMENTED_COUNT.clone())).expect("collector can be registered");
  REGISTRY.register(Box::new(TOKIO_DROPPED_COUNT.clone())).expect("collector can be registered");
  REGISTRY.register(Box::new(TOKIO_FIRST_POLL_COUNT.clone())).expect("collector can be registered");
  REGISTRY.register(Box::new(TOKIO_TOTAL_IDLE_COUNT.clone())).expect("collector can be registered");
  REGISTRY.register(Box::new(TOKIO_TOTAL_IDLE_DURATION.clone())).expect("collector can be registered");
  REGISTRY.register(Box::new(TOKIO_TOTAL_SCHEDULED_COUNT.clone())).expect("collector can be registered");
  REGISTRY.register(Box::new(TOKIO_TOTAL_SCHEDULED_DURATION.clone())).expect("collector can be registered");
  REGISTRY.register(Box::new(TOKIO_TOTAL_POLL_COUNT.clone())).expect("collector can be registered");
  REGISTRY.register(Box::new(TOKIO_TOTAL_POLL_DURATION.clone())).expect("collector can be registered");
  REGISTRY.register(Box::new(TOKIO_TOTAL_FAST_POLL_COUNT.clone())).expect("collector can be registered");
  REGISTRY.register(Box::new(TOKIO_TOTAL_FAST_POLL_DURATION.clone())).expect("collector can be registered");
  REGISTRY.register(Box::new(TOKIO_TOTAL_SLOW_POLL_COUNT.clone())).expect("collector can be registered");
  REGISTRY.register(Box::new(TOKIO_TOTAL_SLOW_POLL_DURATION.clone())).expect("collector can be registered");
}

async fn data_collector(app: ArgMatches) {
  let refresh = app.get_one::<String>("refresh").unwrap().parse::<u64>().unwrap_or(60);
  let mut collect_interval = tokio::time::interval(Duration::from_secs(refresh));

  let mut sio = sio::client::ClientInfo::new(app.get_one::<String>("cfg_path").map(|s| s.as_str()), app.get_one::<String>("ip").map(|s| s.as_str()), app.get_one::<String>("auth_usr").map(|s| s.as_str()), app.get_one::<String>("auth_pwd").map(|s| s.as_str()));
  if sio.version().await.is_err() {
    exit(1);
  }

  loop {
    let metrics = sio.metrics().await;
    if let Some(m) = metrics {
      let timer = UPDATE_HISTOGRAM.start_timer();
      unreg_metrics(&m).await;
      load_metrics(&m).await;
      update_metrics(&m).await;
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
    eprintln!("could not encode metrics: {e}");
  };
  let mut res = match String::from_utf8(buffer.clone()) {
    Ok(v) => v,
    Err(e) => {
      eprintln!("metrics could not be from_utf8'd: {e}");
      String::default()
    },
  };
  buffer.clear();

  let mut buffer = Vec::new();
  if let Err(e) = encoder.encode(&prometheus::gather(), &mut buffer) {
    eprintln!("could not encode prometheus metrics: {e}");
  };
  let res_custom = match String::from_utf8(buffer.clone()) {
    Ok(v) => v,
    Err(e) => {
      eprintln!("prometheus metrics could not be from_utf8'd: {e}");
      String::default()
    },
  };
  buffer.clear();

  res.push_str(&res_custom);
  timer.observe_duration();
  HTTP_BODY_GAUGE.set(res.len() as i64);
  Ok(res)
}

async fn unreg_metrics(metrics: &[sio::metrics::Metric]) {
  let counters = METRIC_COUNTERS.lock().await;
  let gauges = METRIC_GAUGES.lock().await;

  info!("UnRegistering series: {:?}", metrics.len());
  for m in metrics {
    trace!("UnRegistering metric: {} ({})", m.name, m.mtype);
    if m.mtype.to_lowercase() == "counter" {
      let c = counters.get(&m.name);
      let c = match c {
        None => continue,
        Some(c) => c,
      };
      c.reset();
    } else if m.mtype.to_lowercase() == "gauge" {
      let g = gauges.get(&m.name);
      let g = match g {
        None => continue,
        Some(g) => g,
      };
      g.reset();
    }
  }
}

async fn load_metrics(metrics: &[sio::metrics::Metric]) {
  let mut counters = METRIC_COUNTERS.lock().await;
  let mut gauges = METRIC_GAUGES.lock().await;

  info!("Loaded series: {:?}", metrics.len());
  for m in metrics {
    let labels: Vec<&str> = m.labels.iter().map(|v| *v.0).collect::<Vec<_>>();
    let opts = Opts::new(m.name.to_string(), m.help.to_string());
    trace!("Registering metric: {} {:?} ({})", m.name, labels, m.mtype);

    if m.mtype.to_lowercase() == "counter" {
      match register_int_counter_vec!(opts, &labels) {
        Err(_) => continue,
        Ok(o) => {
          counters.insert(m.name.clone().to_string(), o);
        },
      };
    } else if m.mtype.to_lowercase() == "gauge" {
      match register_gauge_vec!(opts, &labels) {
        Err(_) => continue,
        Ok(o) => {
          gauges.insert(m.name.clone().to_string(), o);
        },
      };
    } else {
      error!("Unknown metric type: {} {:?} ({})", m.name, labels, m.mtype);
    }
  }
  info!("Loaded Counters: {:?}", counters.keys().count());
  info!("Loaded Gauges: {:?}", gauges.keys().count());
}

async fn update_metrics(metrics: &[sio::metrics::Metric]) {
  info!("Update metrics");

  let counters = METRIC_COUNTERS.lock().await;
  let gauges = METRIC_GAUGES.lock().await;

  for m in metrics {
    let mut labels: HashMap<&str, &str> = HashMap::new();
    for (k, v) in &m.labels {
      labels.insert(k, v);
    }

    if m.mtype.to_lowercase() == "counter" {
      let c = counters.get(&m.name);
      let c = match c {
        None => {
          error!("The metric {} ({}) was not found as registered", m.name, m.mtype);
          continue;
        },
        Some(c) => c,
      };

      let metric = c.get_metric_with(&labels);
      let metric = match metric {
        Err(e) => {
          error!("The metric {} {:?} ({}) was not found in MetricFamily - {}", m.name, labels, m.mtype, e);
          continue;
        },
        Ok(m) => m,
      };

      metric.inc_by(m.value as u64);
    } else if m.mtype.to_lowercase() == "gauge" {
      let g = gauges.get(&m.name);
      let g = match g {
        None => {
          error!("The metric {} ({}) was not found as registered", m.name, m.mtype);
          continue;
        },
        Some(g) => g,
      };

      let metric = g.get_metric_with(&labels);
      let metric = match metric {
        Err(e) => {
          error!("The metric {} {:?} ({}) was not found in MetricFamily - {}", m.name, labels, m.mtype, e);
          continue;
        },
        Ok(m) => m,
      };

      metric.set(m.value);
    } else {
      error!("Unknown metric type: {} {:?} ({})", m.name, labels, m.mtype);
    }
  }
  info!("Updated Counters: {:?}", counters.keys().count());
  info!("Updated Gauges: {:?}", gauges.keys().count());
}
