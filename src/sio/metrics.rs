use serde_json::value::Map;
use std::{collections::HashMap, fmt};

pub struct Metric {
  pub name:   String,
  pub mtype:  String,
  pub help:   String,
  pub labels: HashMap<&'static str, String>,
  pub value:  f64,
}
impl fmt::Debug for Metric {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}{:?} {} ({})", self.name, self.labels, self.value, self.mtype) }
}
impl Metric {
  pub fn new(name: String, mtype: String, help: String, labels: HashMap<&'static str, String>, value: f64) -> Metric {
    Metric { name,
             mtype,
             help,
             labels,
             value }
  }
}

/// Merge the States and Perf Metrics
pub fn get(inst: &Result<Map<String, serde_json::Value>, anyhow::Error>, stats: &Result<Map<String, serde_json::Value>, anyhow::Error>, labels: &Result<HashMap<&'static str, HashMap<String, HashMap<&'static str, String>>>, String>, rela: &Result<HashMap<&'static str, HashMap<String, HashMap<String, Vec<String>>>>, String>) -> Option<Vec<Metric>> {
  let mut metric_list: Vec<Metric> = Vec::new();

  let m = convert_metrics(&stats.as_ref().unwrap(), &labels.as_ref().unwrap());
  let s = convert_states(&inst.as_ref().unwrap(), &rela.as_ref().unwrap());

  if let Some(mut value) = m {
    metric_list.append(&mut value);
  }
  if let Some(mut value) = s {
    metric_list.append(&mut value);
  }

  if metric_list.is_empty() {
    None
  } else {
    Some(metric_list)
  }
}

/// Build the metrics from the states
fn convert_states(instances: &Map<String, serde_json::Value>, relations: &HashMap<&'static str, HashMap<String, HashMap<String, Vec<String>>>>) -> Option<Vec<Metric>> {
  let default_val = vec![serde_json::Value::Null];
  let mut metric_list: Vec<Metric> = Vec::new();

  let clu_id = instances.get("System").and_then(|o| o.as_object().and_then(|j| j.get("id")).map(|s| s.to_string().replace('"', ""))).expect("clu_id Not found");
  let clu_name = match instances.get("System").and_then(|o| o.as_object().and_then(|j| j.get("name")).map(|s| s.to_string().replace('"', ""))) {
    None => {
      warn!("clu_name Not found using clu_id as name");
      clu_id.to_string()
    },
    Some(s) => s,
  };

  // Sdc
  for sdc in instances.get("sdcList").and_then(|v| v.as_array()).unwrap_or_else(|| {
                                                                  error!("Failed to get 'sdcList' from instances");
                                                                  &default_val
                                                                })
  {
    for sdc in sdc.as_object().iter() {
      let sdc_name = sdc.get("name").map(|s| s.to_string().replace('"', "")).expect("sdc_name Not found");
      let sdc_id = sdc.get("id").map(|s| s.to_string().replace('"', "")).expect("sdc_id Not found");
      let sdc_state_mdm_connection = match sdc.get("mdmConnectionState").map(|s| s.to_string().replace('"', "")) {
        Some(s) => {
          match s.as_str() {
            "Connected" => Some(0.0),
            "Disconnected" => Some(1.0),
            _ => {
              warn!("Unknown mdmConnectionState: {:?}", s);
              None
            },
          }
        },
        None => None,
      };

      if let Some(value) = sdc_state_mdm_connection {
        let mut label: HashMap<&'static str, String> = HashMap::new();
        label.entry("clu_name").or_insert_with(|| clu_name.to_string());
        label.entry("clu_id").or_insert_with(|| clu_id.to_string());
        label.entry("sdc_name").or_insert_with(|| sdc_name);
        label.entry("sdc_id").or_insert_with(|| sdc_id.to_string());

        let state: Metric = Metric::new("sdc_state_mdm_connection".to_string(), "gauge".to_string(), "mdmConnectionState: Connected=0.0 or Disconnected=1.0".to_string(), label.clone(), value);
        metric_list.push(state);
      }
    }
  }

  // Sds
  for sds in instances.get("sdsList").and_then(|v| v.as_array()).unwrap_or_else(|| {
                                                                  error!("Failed to get 'sdsList' from instances");
                                                                  &default_val
                                                                })
  {
    let mut parent: HashMap<&'static str, String> = HashMap::new();

    for sds in sds.as_object().iter() {
      let sds_name = sds.get("name").map(|s| s.to_string().replace('"', "")).expect("sds_name Not found");
      let sds_id = sds.get("id").map(|s| s.to_string().replace('"', "")).expect("sds_id Not found");
      let sds_state = match sds.get("sdsState").map(|s| s.to_string().replace('"', "")) {
        Some(s) => {
          match s.as_str() {
            "Normal" => Some(0.0),
            "RemovePending" => Some(1.0),
            _ => {
              warn!("Unknown sdsState: {:?}", s);
              None
            },
          }
        },
        None => None,
      };

      let sds_state_mdm_connection = match sds.get("mdmConnectionState").map(|s| s.to_string().replace('"', "")) {
        Some(s) => {
          match s.as_str() {
            "Connected" => Some(0.0),
            "Disconnected" => Some(1.0),
            _ => {
              warn!("Unknown mdmConnectionState: {:?}", s);
              None
            },
          }
        },
        None => None,
      };

      for pd in instances["protectionDomainList"].as_array().unwrap().iter() {
        for pdo in pd.as_object().iter() {
          if relations["parents"][&(sds_id)]["protectiondomain"].contains(&(pdo["id"].to_string().replace('"', ""))) {
            parent.entry("name").or_insert_with(|| pdo["name"].to_string().replace('"', ""));
            parent.entry("id").or_insert_with(|| pdo["id"].to_string().replace('"', ""));
            break;
          }
        }
      }

      if let Some(value) = sds_state {
        let mut label: HashMap<&'static str, String> = HashMap::new();
        label.entry("clu_name").or_insert_with(|| clu_name.to_string());
        label.entry("clu_id").or_insert_with(|| clu_id.to_string());
        label.entry("sds_name").or_insert_with(|| sds_name.clone());
        label.entry("sds_id").or_insert_with(|| sds_id.to_string());
        label.entry("pdo_name").or_insert_with(|| parent["name"].to_string());
        label.entry("pdo_id").or_insert_with(|| parent["id"].to_string());

        let state: Metric = Metric::new("sds_state".to_string(), "gauge".to_string(), "sdsState: Normal=0.0 or RemovePending=1.0".to_string(), label.clone(), value);
        metric_list.push(state);
      }
      if let Some(value) = sds_state_mdm_connection {
        let mut label: HashMap<&'static str, String> = HashMap::new();
        label.entry("clu_name").or_insert_with(|| clu_name.to_string());
        label.entry("clu_id").or_insert_with(|| clu_id.to_string());
        label.entry("sds_name").or_insert_with(|| sds_name);
        label.entry("sds_id").or_insert_with(|| sds_id.to_string());
        label.entry("pdo_name").or_insert_with(|| parent["name"].to_string());
        label.entry("pdo_id").or_insert_with(|| parent["id"].to_string());

        let state: Metric = Metric::new("sds_state_mdm_connection".to_string(), "gauge".to_string(), "mdmConnectionState: Connected=0.0 or Disconnected=1.0".to_string(), label.clone(), value);
        metric_list.push(state);
      }
    }
  }

  // Devices
  for dl in instances.get("deviceList")
                     .and_then(|v| v.as_array())
                     .unwrap_or_else(|| {
                       error!("Failed to get 'deviceList' from instances");
                       &default_val
                     })
                     .iter()
  {
    let mut parent_sds: HashMap<&'static str, String> = HashMap::new();
    let mut parent_sto: HashMap<&'static str, String> = HashMap::new();
    let mut parent_pdo: HashMap<&'static str, String> = HashMap::new();

    for dev in dl.as_object().iter() {
      let mut label: HashMap<&'static str, String> = HashMap::new();

      let dev_name = dev.get("name").map(|s| s.to_string().replace('"', "")).expect("dev_name Not found");
      let dev_id = dev.get("id").map(|s| s.to_string().replace('"', "")).expect("dev_id Not found");
      let dev_path = dev.get("deviceCurrentPathName").map(|s| s.to_string().replace("/dev/", "").replace('"', "")).expect("dev_path Not found");
      let dev_state = match dev.get("deviceState").map(|s| s.to_string().replace('"', "")) {
        Some(s) => {
          match s.as_str() {
            "Normal" | "NormalTesting" => Some(0.0),
            "DeviceInit" => Some(1.0),
            "DeviceRecovery" => Some(2.0),
            "InitialTest" => Some(3.0),
            "InitialTestDone" => Some(4.0),
            "RemovePending" => Some(5.0),
            _ => {
              warn!("Unknown deviceState: {:?}", s);
              None
            },
          }
        },
        None => None,
      };

      if let Some(value) = dev_state {
        label.entry("clu_name").or_insert_with(|| clu_name.to_string());
        label.entry("clu_id").or_insert_with(|| clu_id.to_string());
        label.entry("dev_name").or_insert_with(|| dev_name);
        label.entry("dev_id").or_insert_with(|| dev_id.to_string());
        label.entry("dev_path").or_insert_with(|| dev_path);

        for sdsl in instances["sdsList"].as_array().unwrap().iter() {
          for sds in sdsl.as_object().iter() {
            if relations["parents"][&(dev_id)]["sds"].contains(&(sds["id"].to_string().replace('"', ""))) {
              parent_sds.entry("name").or_insert_with(|| sds["name"].to_string().replace('"', ""));
              parent_sds.entry("id").or_insert_with(|| sds["id"].to_string().replace('"', ""));
              break;
            }
          }
        }
        for sp in instances["storagePoolList"].as_array().unwrap().iter() {
          for sto in sp.as_object().iter() {
            if relations["parents"][&(dev_id)]["storagepool"].contains(&(sto["id"].to_string().replace('"', ""))) {
              parent_sto.entry("name").or_insert_with(|| sto["name"].to_string().replace('"', ""));
              parent_sto.entry("id").or_insert_with(|| sto["id"].to_string().replace('"', ""));
              break;
            }
          }
        }
        for pd in instances["protectionDomainList"].as_array().unwrap().iter() {
          for pdo in pd.as_object().iter() {
            if relations["parents"][&(parent_sto["id"].to_string().replace('"', ""))]["protectiondomain"].contains(&(pdo["id"].to_string().replace('"', ""))) {
              parent_pdo.entry("name").or_insert_with(|| pdo["name"].to_string().replace('"', ""));
              parent_pdo.entry("id").or_insert_with(|| pdo["id"].to_string().replace('"', ""));
              break;
            }
          }
        }

        match parent_sds.get("name") {
          None => {
            error!("Failed to get 'name' from parent_sds");
            continue;
          },
          Some(o) => label.entry("sds_name").or_insert_with(|| o.to_string()),
        };
        match parent_sds.get("id") {
          None => {
            error!("Failed to get 'id' from parent_sds");
            continue;
          },
          Some(o) => label.entry("sds_id").or_insert_with(|| o.to_string()),
        };

        match parent_sto.get("name") {
          None => {
            error!("Failed to get 'name' from parent_sto");
            continue;
          },
          Some(o) => label.entry("sto_name").or_insert_with(|| o.to_string()),
        };
        match parent_sto.get("id") {
          None => {
            error!("Failed to get 'id' from parent_sto");
            continue;
          },
          Some(o) => label.entry("sto_id").or_insert_with(|| o.to_string()),
        };

        match parent_pdo.get("name") {
          None => {
            error!("Failed to get 'name' from parent_pdo");
            continue;
          },
          Some(o) => label.entry("pdo_name").or_insert_with(|| o.to_string()),
        };
        match parent_pdo.get("id") {
          None => {
            error!("Failed to get 'id' from parent_pdo");
            continue;
          },
          Some(o) => label.entry("pdo_id").or_insert_with(|| o.to_string()),
        };

        let state: Metric = Metric::new("device_state".to_string(), "gauge".to_string(), "deviceState: Normal,NormalTesting=0.0 or DeviceInit=1.0 or DeviceRecovery=2.0 or InitialTest=3.0 or InitialTestDone=4.0 or RemovePending=5.0".to_string(), label.clone(), value);
        metric_list.push(state);
      }
    }
  }

  if metric_list.is_empty() {
    None
  } else {
    Some(metric_list)
  }
}

/// Build the final metric definition that should be used to create and update the metrics
fn convert_metrics(stats: &Map<String, serde_json::Value>, labels: &HashMap<&'static str, HashMap<String, HashMap<&'static str, String>>>) -> Option<Vec<Metric>> {
  let mdef = super::utils::read_json("cfg/metric_definition.json").unwrap_or_else(|| panic!("Failed to loading metric definition"));
  debug!("Loaded metric defenitions: {:?}", mdef.keys().collect::<Vec<_>>());

  let mut metric_list: Vec<Metric> = Vec::new();

  for (instance_type, metrics) in stats.iter() {
    if instance_type == "System" {
      if metrics.is_object() {
        let stype: &str = &instance_type.replace('"', "");

        for (m, v) in metrics.as_object().unwrap().iter() {
          if mdef.contains_key(m) {
            let m_labels = match labels.get(stype).and_then(|l| l.get(stype)) {
              None => {
                error!("Failed to get 'labels' from {}", stype);
                continue;
              },
              Some(l) => l,
            };

            if m.ends_with("Bwc") && v.is_object() {
              let m_io_name = format!("{}_{}_iops", stype, mdef[m].as_object().unwrap()["name"]).replace('"', "").to_lowercase();
              let m_io_type = mdef[m].as_object().unwrap()["type"].to_string().replace('"', "").to_lowercase();
              let m_io_help = mdef[m].as_object().unwrap()["help"].to_string().replace('"', "");
              let m_io_value: f64 = iops_calc(v.as_object().unwrap()["numOccured"].to_string().parse::<i32>().unwrap(), v.as_object().unwrap()["numSeconds"].to_string().parse::<i32>().unwrap());
              let metric_io: Metric = Metric::new(m_io_name, m_io_type, m_io_help, m_labels.clone(), m_io_value);
              metric_list.push(metric_io);

              let m_bw_name = format!("{}_{}_kb", stype, mdef[m].as_object().unwrap()["name"]).replace('"', "").to_lowercase();
              let m_bw_type = mdef[m].as_object().unwrap()["type"].to_string().replace('"', "").to_lowercase();
              let m_bw_help = mdef[m].as_object().unwrap()["help"].to_string().replace('"', "");
              let m_bw_value: f64 = bw_calc(v.as_object().unwrap()["totalWeightInKb"].to_string().parse::<i32>().unwrap(), v.as_object().unwrap()["numSeconds"].to_string().parse::<i32>().unwrap());
              let metric_bw: Metric = Metric::new(m_bw_name, m_bw_type, m_bw_help, m_labels.clone(), m_bw_value);
              metric_list.push(metric_bw);
            } else {
              let m_name = format!("{}_{}", stype, mdef[m].as_object().unwrap()["name"]).replace('"', "").to_lowercase();
              let m_type = mdef[m].as_object().unwrap()["type"].to_string().replace('"', "").to_lowercase();
              let m_help = mdef[m].as_object().unwrap()["help"].to_string().replace('"', "");
              let m_value: f64 = v.as_f64().expect("Invalid metric value");

              let metric_bw: Metric = Metric::new(m_name, m_type, m_help, m_labels.clone(), m_value);
              metric_list.push(metric_bw);
            }
          } else {
            error!("Metric: {} ({}) not found in (metric_definition.json)", m, stype);
            continue;
          }
        }
      }
    } else if metrics.is_object() {
      for (id, v) in metrics.as_object().unwrap().iter() {
        let stype: &str = &instance_type.replace('"', "").to_lowercase();

        for (m, v) in v.as_object().unwrap().iter() {
          if mdef.contains_key(m) {
            let m_labels = match labels.get(stype).and_then(|l| l.get(id)) {
              None => {
                warn!("Failed to get 'labels' from {} -> {}", stype, id);
                continue;
              },
              Some(l) => l,
            };

            if m.ends_with("Bwc") && v.is_object() {
              let m_io_name = format!("{}_{}_iops", stype, mdef[m].as_object().unwrap()["name"]).replace('"', "").to_lowercase();
              let m_io_type = mdef[m].as_object().unwrap()["type"].to_string().replace('"', "").to_lowercase();
              let m_io_help = mdef[m].as_object().unwrap()["help"].to_string().replace('"', "");
              let m_io_value: f64 = iops_calc(v.as_object().unwrap()["numOccured"].to_string().parse::<i32>().unwrap(), v.as_object().unwrap()["numSeconds"].to_string().parse::<i32>().unwrap());
              let metric_io: Metric = Metric::new(m_io_name, m_io_type, m_io_help, m_labels.clone(), m_io_value);
              metric_list.push(metric_io);

              let m_bw_name = format!("{}_{}_kb", stype, mdef[m].as_object().unwrap()["name"]).replace('"', "").to_lowercase();
              let m_bw_type = mdef[m].as_object().unwrap()["type"].to_string().replace('"', "").to_lowercase();
              let m_bw_help = mdef[m].as_object().unwrap()["help"].to_string().replace('"', "");
              let m_bw_value: f64 = bw_calc(v.as_object().unwrap()["totalWeightInKb"].to_string().parse::<i32>().unwrap(), v.as_object().unwrap()["numSeconds"].to_string().parse::<i32>().unwrap());
              let metric_bw: Metric = Metric::new(m_bw_name, m_bw_type, m_bw_help, m_labels.clone(), m_bw_value);
              metric_list.push(metric_bw);
            } else {
              let m_name = format!("{}_{}", stype, mdef[m].as_object().unwrap()["name"]).replace('"', "").to_lowercase();
              let m_type = mdef[m].as_object().unwrap()["type"].to_string().replace('"', "").to_lowercase();
              let m_help = mdef[m].as_object().unwrap()["help"].to_string().replace('"', "");
              let m_value: f64 = v.as_f64().expect("Invalid metric value");

              let metric: Metric = Metric::new(m_name, m_type, m_help, m_labels.clone(), m_value);
              metric_list.push(metric);
            }
          } else {
            error!("Metric: {} ({}) not found in (metric_definition.json)", m, stype);
            continue;
          }
        }
      }
    }
  }

  if metric_list.is_empty() {
    None
  } else {
    Some(metric_list)
  }
}

/// Calculate IOPS from the *Bwc metrics
/// `https://github.com/andrewjwhite/ScaleIO_RestAPI_Python_Examples/blob/master/ScaleIO_cluster_stats_example.py#L92-L108`
fn iops_calc(occur: i32, secs: i32) -> f64 {
  if occur == 0 {
    0.0_f64
  } else {
    (occur / secs) as f64
  }
}

/// Calculate Bandwidth Kb/s from the *Bwc metrics
fn bw_calc(occur: i32, secs: i32) -> f64 {
  if occur == 0 {
    0.0_f64
  } else {
    (occur / secs) as f64
  }
}
