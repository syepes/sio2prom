//! SIO Metrics
//!
//! The ScaleIO `Metrics conversion`
//!

use std;

use std::collections::{HashMap, BTreeMap};
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::sync::{Arc, Mutex};

extern crate core;
extern crate serde;
extern crate serde_json;

use sio;

pub struct Metric {
    pub name: String,
    pub mtype: String,
    pub help: String,
    pub labels: HashMap<&'static str, String>,
    pub value: f64,
}
impl fmt::Debug for Metric {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}{:?} {} ({})", self.name, self.labels, self.value, self.mtype) }
}
impl Metric {
    pub fn new(name: String, mtype: String, help: String, labels: HashMap<&'static str, String>, value: f64) -> Metric {
        Metric { name: name,
                 mtype: mtype,
                 help: help,
                 labels: labels,
                 value: value, }
    }
}


/// Query ScaleIO instances and find their relationships
fn get_instances(sio: &Arc<Mutex<sio::client::Client>>) -> (BTreeMap<String, serde_json::Value>, HashMap<&'static str, HashMap<String, HashMap<String, Vec<String>>>>) {
    let instances = sio.lock().unwrap().instances();

    let mut relations: HashMap<&'static str, HashMap<String, HashMap<String, Vec<String>>>> = HashMap::new();
    relations.entry("childs").or_insert(HashMap::new());
    relations.entry("parents").or_insert(HashMap::new());

    // Get relations of all the elements
    for (key, value) in instances.iter() {
        if value.is_array() {
            for items in value.as_array().unwrap().iter() {
                let item_type = key.replace("List", "").to_string().replace('"', "").to_lowercase();
                let item_id = items.as_object().unwrap().get("id").unwrap().to_string().replace('"', "");
                let item_name = items.as_object().unwrap().get("name").unwrap().to_string().replace('"', "");
                trace!("Instance item type: {} / name: {} / id: {}", item_type, item_name, item_id);

                for links in items.find("links")
                                  .and_then(|v| v.as_array())
                                  .unwrap_or_else(|| {
                                      panic!("Failed to get 'links' from items");
                                  })
                                  .iter() {
                    let link = links.as_object().unwrap();
                    if !link.get("rel").unwrap().to_string().replace('"', "").starts_with("/api/parent") {
                        continue;
                    }

                    let parent_type_tmp: String = link.get("href").unwrap().to_string();
                    let parent_type: String = parent_type_tmp.split(':').nth(0).unwrap().split('/').last().unwrap().to_string().replace('"', "").to_lowercase();

                    let parent_id_tmp: String = link.get("href").unwrap().to_string().replace('"', "");
                    let parent_id: String = parent_id_tmp.split(':').last().unwrap().to_string().replace('"', "");

                    {
                        let mut a = relations.get_mut("childs").unwrap().entry(parent_id.clone()).or_insert(HashMap::new()).entry(item_type.clone()).or_insert(Vec::new());
                        a.push(item_id.clone());
                    }
                    {
                        let mut a = relations.get_mut("parents").unwrap().entry(item_id.clone()).or_insert(HashMap::new()).entry(parent_type.clone()).or_insert(Vec::new());
                        a.push(parent_id.clone());
                    }
                }
            }
        }
    }
    trace!("Found Instances: {:?}", instances.keys().map(|i| i.replace("List", "")).collect::<Vec<_>>());
    info!("Found Instance relationships Parent: {} / Child: {} relations", relations.get("parents").unwrap().len(), relations.get("childs").unwrap().len());
    (instances.clone(), relations)
}

/// Generate Prometheus.io labels from ScaleIO instances and relations
fn get_labels(instances: &BTreeMap<String, serde_json::Value>, relations: &HashMap<&'static str, HashMap<String, HashMap<String, Vec<String>>>>)
              -> HashMap<&'static str, HashMap<String, HashMap<&'static str, String>>> {
    let mut labels: HashMap<&'static str, HashMap<String, HashMap<&'static str, String>>> = HashMap::new();
    let clu_name = instances.get("System").unwrap().as_object().unwrap().get("name").unwrap().to_string().replace('"', "");
    let clu_id = instances.get("System").unwrap().as_object().unwrap().get("id").unwrap().to_string().replace('"', "");

    // System
    {
        let mut label: HashMap<&'static str, String> = HashMap::new();
        label.entry("clu_name").or_insert(clu_name.to_string());
        label.entry("clu_id").or_insert(clu_id.to_string());

        labels.entry("System").or_insert(HashMap::new()).entry("System".to_string()).or_insert(label);
    }
    // Sdc
    for sdc in instances.get("sdcList").and_then(|v| v.as_array()).unwrap_or_else(|| panic!("Failed to get 'sdcList' from instances")) {
        for sdc in sdc.as_object().iter() {
            let mut label: HashMap<&'static str, String> = HashMap::new();
            let sdc_name = sdc.get("name").unwrap().to_string().replace('"', "");
            let sdc_id = sdc.get("id").unwrap().to_string().replace('"', "");

            label.entry("clu_name").or_insert(clu_name.to_string());
            label.entry("clu_id").or_insert(clu_id.to_string());
            label.entry("sdc_name").or_insert(sdc_name);
            label.entry("sdc_id").or_insert(sdc_id.to_string());

            labels.entry("sdc").or_insert(HashMap::new()).entry(sdc_id).or_insert(label);
        }
    }
    // ProtectionDomain
    for pd in instances.get("protectionDomainList").and_then(|v| v.as_array()).unwrap_or_else(|| panic!("Failed to get 'protectionDomainList' from instances")) {
        for pdo in pd.as_object().iter() {
            let mut label: HashMap<&'static str, String> = HashMap::new();
            let pdo_name = pdo.get("name").unwrap().to_string().replace('"', "");
            let pdo_id = pdo.get("id").unwrap().to_string().replace('"', "");

            label.entry("clu_name").or_insert(clu_name.to_string());
            label.entry("clu_id").or_insert(clu_id.to_string());
            label.entry("pdo_name").or_insert(pdo_name.to_string());
            label.entry("pdo_id").or_insert(pdo_id.to_string());

            labels.entry("protectiondomain").or_insert(HashMap::new()).entry(pdo_id).or_insert(label);
        }
    }
    // StoragePool
    for spl in instances.get("storagePoolList").and_then(|v| v.as_array()).unwrap_or_else(|| panic!("Failed to get 'storagePoolList' from instances")) {
        let mut parent: HashMap<&'static str, String> = HashMap::new();

        for sp in spl.as_object().iter() {
            let mut label: HashMap<&'static str, String> = HashMap::new();
            let sp_name = sp.get("name").unwrap().to_string().replace('"', "");
            let sp_id = sp.get("id").unwrap().to_string().replace('"', "");

            for pd in instances.get("protectionDomainList").unwrap().as_array().unwrap().iter() {
                for pdo in pd.as_object().iter() {
                    if relations["parents"].get(&(sp_id)).unwrap().get("protectiondomain").unwrap().contains(&(pdo.get("id").unwrap().to_string().replace('"', ""))) {
                        parent.entry("name").or_insert(pdo.get("name").unwrap().to_string().replace('"', ""));
                        parent.entry("id").or_insert(pdo.get("id").unwrap().to_string().replace('"', ""));
                        break;
                    }
                }
            }

            label.entry("clu_name").or_insert(clu_name.to_string());
            label.entry("clu_id").or_insert(clu_id.to_string());
            label.entry("sto_name").or_insert(sp_name);
            label.entry("sto_id").or_insert(sp_id.to_string());
            label.entry("pdo_name").or_insert(parent.get("name").unwrap().to_string());
            label.entry("pdo_id").or_insert(parent.get("id").unwrap().to_string());

            labels.entry("storagepool").or_insert(HashMap::new()).entry(sp_id).or_insert(label);
        }
    }
    // Sds
    for sdsl in instances.get("sdsList").and_then(|v| v.as_array()).unwrap_or_else(|| panic!("Failed to get 'sdsList' from instances")) {
        let mut parent: HashMap<&'static str, String> = HashMap::new();

        for sds in sdsl.as_object().iter() {
            let mut label: HashMap<&'static str, String> = HashMap::new();
            let sds_name = sds.get("name").unwrap().to_string().replace('"', "");
            let sds_id = sds.get("id").unwrap().to_string().replace('"', "");

            for pd in instances.get("protectionDomainList").unwrap().as_array().unwrap().iter() {
                for pdo in pd.as_object().iter() {
                    if relations["parents"].get(&(sds_id)).unwrap().get("protectiondomain").unwrap().contains(&(pdo.get("id").unwrap().to_string().replace('"', ""))) {
                        parent.entry("name").or_insert(pdo.get("name").unwrap().to_string().replace('"', ""));
                        parent.entry("id").or_insert(pdo.get("id").unwrap().to_string().replace('"', ""));
                        break;
                    }
                }
            }

            label.entry("clu_name").or_insert(clu_name.to_string());
            label.entry("clu_id").or_insert(clu_id.to_string());
            label.entry("sds_name").or_insert(sds_name);
            label.entry("sds_id").or_insert(sds_id.to_string());
            label.entry("pdo_name").or_insert(parent.get("name").unwrap().to_string());
            label.entry("pdo_id").or_insert(parent.get("id").unwrap().to_string());

            labels.entry("sds").or_insert(HashMap::new()).entry(sds_id).or_insert(label);
        }
    }
    // Volumes
    for vl in instances.get("volumeList").and_then(|v| v.as_array()).unwrap_or_else(|| panic!("Failed to get 'volumeList' from instances")) {
        let mut parent_sto: HashMap<&'static str, String> = HashMap::new();
        let mut parent_pdo: HashMap<&'static str, String> = HashMap::new();

        for vol in vl.as_object().iter() {
            let mut label: HashMap<&'static str, String> = HashMap::new();
            let vol_name = vol.get("name").unwrap().to_string().replace('"', "");
            let vol_id = vol.get("id").unwrap().to_string().replace('"', "");

            for sp in instances.get("storagePoolList").unwrap().as_array().unwrap().iter() {
                for sto in sp.as_object().iter() {
                    if relations["parents"].get(&(vol_id)).unwrap().get("storagepool").unwrap().contains(&(sto.get("id").unwrap().to_string().replace('"', ""))) {
                        parent_sto.entry("name").or_insert(sto.get("name").unwrap().to_string().replace('"', ""));
                        parent_sto.entry("id").or_insert(sto.get("id").unwrap().to_string().replace('"', ""));
                        break;
                    }
                }
            }
            for pd in instances.get("protectionDomainList").unwrap().as_array().unwrap().iter() {
                for pdo in pd.as_object().iter() {
                    if relations["parents"]
                        .get(&(parent_sto.get("id").unwrap().to_string().replace('"', "")))
                        .unwrap()
                        .get("protectiondomain")
                        .unwrap()
                        .contains(&(pdo.get("id").unwrap().to_string().replace('"', ""))) {
                        parent_pdo.entry("name").or_insert(pdo.get("name").unwrap().to_string().replace('"', ""));
                        parent_pdo.entry("id").or_insert(pdo.get("id").unwrap().to_string().replace('"', ""));
                        break;
                    }
                }
            }

            label.entry("clu_name").or_insert(clu_name.to_string());
            label.entry("clu_id").or_insert(clu_id.to_string());
            label.entry("vol_name").or_insert(vol_name);
            label.entry("vol_id").or_insert(vol_id.to_string());
            label.entry("sto_name").or_insert(parent_sto.get("name").unwrap().to_string());
            label.entry("sto_id").or_insert(parent_sto.get("id").unwrap().to_string());
            label.entry("pdo_name").or_insert(parent_pdo.get("name").unwrap().to_string());
            label.entry("pdo_id").or_insert(parent_pdo.get("id").unwrap().to_string());

            labels.entry("volume").or_insert(HashMap::new()).entry(vol_id).or_insert(label);
        }
    }
    // Devices
    for dl in instances.get("deviceList").and_then(|v| v.as_array()).unwrap_or_else(|| panic!("Failed to get 'deviceList' from instances")).iter() {
        let mut parent_sds: HashMap<&'static str, String> = HashMap::new();
        let mut parent_sto: HashMap<&'static str, String> = HashMap::new();
        let mut parent_pdo: HashMap<&'static str, String> = HashMap::new();

        for dev in dl.as_object().iter() {
            let mut label: HashMap<&'static str, String> = HashMap::new();
            let dev_name = dev.get("name").unwrap().to_string().replace('"', "");
            let dev_id = dev.get("id").unwrap().to_string().replace('"', "");
            let dev_path = dev.get("deviceCurrentPathName").unwrap().to_string().replace("/dev/", "").replace('"', "");

            for sdsl in instances.get("sdsList").unwrap().as_array().unwrap().iter() {
                for sds in sdsl.as_object().iter() {
                    if relations["parents"].get(&(dev_id)).unwrap().get("sds").unwrap().contains(&(sds.get("id").unwrap().to_string().replace('"', ""))) {
                        parent_sds.entry("name").or_insert(sds.get("name").unwrap().to_string().replace('"', ""));
                        parent_sds.entry("id").or_insert(sds.get("id").unwrap().to_string().replace('"', ""));
                        break;
                    }
                }
            }
            for sp in instances.get("storagePoolList").unwrap().as_array().unwrap().iter() {
                for sto in sp.as_object().iter() {
                    if relations["parents"].get(&(dev_id)).unwrap().get("storagepool").unwrap().contains(&(sto.get("id").unwrap().to_string().replace('"', ""))) {
                        parent_sto.entry("name").or_insert(sto.get("name").unwrap().to_string().replace('"', ""));
                        parent_sto.entry("id").or_insert(sto.get("id").unwrap().to_string().replace('"', ""));
                        break;
                    }
                }
            }
            for pd in instances.get("protectionDomainList").unwrap().as_array().unwrap().iter() {
                for pdo in pd.as_object().iter() {
                    if relations["parents"]
                        .get(&(parent_sto.get("id").unwrap().to_string().replace('"', "")))
                        .unwrap()
                        .get("protectiondomain")
                        .unwrap()
                        .contains(&(pdo.get("id").unwrap().to_string().replace('"', ""))) {
                        parent_pdo.entry("name").or_insert(pdo.get("name").unwrap().to_string().replace('"', ""));
                        parent_pdo.entry("id").or_insert(pdo.get("id").unwrap().to_string().replace('"', ""));
                        break;
                    }
                }
            }

            label.entry("clu_name").or_insert(clu_name.to_string());
            label.entry("clu_id").or_insert(clu_id.to_string());
            label.entry("dev_name").or_insert(dev_name);
            label.entry("dev_id").or_insert(dev_id.to_string());
            label.entry("dev_path").or_insert(dev_path);
            label.entry("sds_name").or_insert(parent_sds.get("name").unwrap().to_string());
            label.entry("sds_id").or_insert(parent_sds.get("id").unwrap().to_string());
            label.entry("sto_name").or_insert(parent_sto.get("name").unwrap().to_string());
            label.entry("sto_id").or_insert(parent_sto.get("id").unwrap().to_string());
            label.entry("pdo_name").or_insert(parent_pdo.get("name").unwrap().to_string());
            label.entry("pdo_id").or_insert(parent_pdo.get("id").unwrap().to_string());

            labels.entry("device").or_insert(HashMap::new()).entry(dev_id).or_insert(label);
        }
    }
    labels
}

/// Build the final metric definition that should be used to create and update the metrics
fn convert_metrics(stats: &BTreeMap<String, serde_json::Value>, labels: &HashMap<&'static str, HashMap<String, HashMap<&'static str, String>>>) -> Vec<Metric> {
    let mdef = read_json("cfg/metric_definition.json").unwrap_or_else(|| panic!("Failed to loading metric_definition"));
    debug!("Loaded metric defenitions: {:?}", mdef.keys().collect::<Vec<_>>());

    let mut metric_list: Vec<Metric> = Vec::new();

    for (instance_type, metrics) in stats.iter() {
        if instance_type == "System" {
            if metrics.is_object() {
                let stype: &str = &instance_type.replace('"', "");

                for (m, v) in metrics.as_object().unwrap().iter() {
                    if mdef.contains_key(m) {
                        let m_labels = labels.get(stype).and_then(|l| l.get(stype)).unwrap_or_else(|| panic!("Failed to get 'labels' from {}", stype));

                        if m.ends_with("Bwc") && v.is_object() {
                            let m_io_name = format!("{}_{}_iops", stype, mdef.get(m).unwrap().as_object().unwrap().get("name").unwrap()).replace('"', "").to_lowercase();
                            let m_io_type = mdef.get(m).unwrap().as_object().unwrap().get("type").unwrap().to_string().replace('"', "").to_lowercase();
                            let m_io_help = mdef.get(m).unwrap().as_object().unwrap().get("help").unwrap().to_string().replace('"', "");
                            let m_io_value: f64 = iops_calc(v.as_object().unwrap().get("numOccured").unwrap().to_string().parse::<i32>().unwrap(),
                                                            v.as_object().unwrap().get("numSeconds").unwrap().to_string().parse::<i32>().unwrap());
                            let metric_io: Metric = Metric::new(m_io_name, m_io_type, m_io_help, m_labels.clone(), m_io_value);
                            metric_list.push(metric_io);

                            let m_bw_name = format!("{}_{}_mbs", stype, mdef.get(m).unwrap().as_object().unwrap().get("name").unwrap()).replace('"', "").to_lowercase();
                            let m_bw_type = mdef.get(m).unwrap().as_object().unwrap().get("type").unwrap().to_string().replace('"', "").to_lowercase();
                            let m_bw_help = mdef.get(m).unwrap().as_object().unwrap().get("help").unwrap().to_string().replace('"', "");
                            let m_bw_value: f64 = bw_calc(v.as_object().unwrap().get("totalWeightInKb").unwrap().to_string().parse::<i32>().unwrap(),
                                                          v.as_object().unwrap().get("numSeconds").unwrap().to_string().parse::<i32>().unwrap());
                            let metric_bw: Metric = Metric::new(m_bw_name, m_bw_type, m_bw_help, m_labels.clone(), m_bw_value);
                            metric_list.push(metric_bw);

                        } else {
                            let m_name = format!("{}_{}", stype, mdef.get(m).unwrap().as_object().unwrap().get("name").unwrap()).replace('"', "").to_lowercase();
                            let m_type = mdef.get(m).unwrap().as_object().unwrap().get("type").unwrap().to_string().replace('"', "").to_lowercase();
                            let m_help = mdef.get(m).unwrap().as_object().unwrap().get("help").unwrap().to_string().replace('"', "");
                            let m_value: f64 = v.as_f64().expect("Invalid metric value");

                            let metric_bw: Metric = Metric::new(m_name, m_type, m_help, m_labels.clone(), m_value);
                            metric_list.push(metric_bw);
                        }
                    } else {
                        error!("Metric: {} ({}) not found in the metric definition", m, stype);
                        continue;
                    }
                }
            }
        } else {
            if metrics.is_object() {
                for (id, v) in metrics.as_object().unwrap().iter() {
                    let stype: &str = &instance_type.replace('"', "").to_lowercase();

                    for (m, v) in v.as_object().unwrap().iter() {
                        if mdef.contains_key(m) {
                            let m_labels = labels.get(stype).and_then(|l| l.get(id)).unwrap_or_else(|| panic!("Failed to get 'labels' from {} -> {}", stype, id));

                            if m.ends_with("Bwc") && v.is_object() {
                                let m_io_name = format!("{}_{}_iops", stype, mdef.get(m).unwrap().as_object().unwrap().get("name").unwrap()).replace('"', "").to_lowercase();
                                let m_io_type = mdef.get(m).unwrap().as_object().unwrap().get("type").unwrap().to_string().replace('"', "").to_lowercase();
                                let m_io_help = mdef.get(m).unwrap().as_object().unwrap().get("help").unwrap().to_string().replace('"', "");
                                let m_io_value: f64 = iops_calc(v.as_object().unwrap().get("numOccured").unwrap().to_string().parse::<i32>().unwrap(),
                                                                v.as_object().unwrap().get("numSeconds").unwrap().to_string().parse::<i32>().unwrap());
                                let metric_io: Metric = Metric::new(m_io_name, m_io_type, m_io_help, m_labels.clone(), m_io_value);
                                metric_list.push(metric_io);

                                let m_bw_name = format!("{}_{}_mbs", stype, mdef.get(m).unwrap().as_object().unwrap().get("name").unwrap()).replace('"', "").to_lowercase();
                                let m_bw_type = mdef.get(m).unwrap().as_object().unwrap().get("type").unwrap().to_string().replace('"', "").to_lowercase();
                                let m_bw_help = mdef.get(m).unwrap().as_object().unwrap().get("help").unwrap().to_string().replace('"', "");
                                let m_bw_value: f64 = bw_calc(v.as_object().unwrap().get("totalWeightInKb").unwrap().to_string().parse::<i32>().unwrap(),
                                                              v.as_object().unwrap().get("numSeconds").unwrap().to_string().parse::<i32>().unwrap());
                                let metric_bw: Metric = Metric::new(m_bw_name, m_bw_type, m_bw_help, m_labels.clone(), m_bw_value);
                                metric_list.push(metric_bw);

                            } else {
                                let m_name = format!("{}_{}", stype, mdef.get(m).unwrap().as_object().unwrap().get("name").unwrap()).replace('"', "").to_lowercase();
                                let m_type = mdef.get(m).unwrap().as_object().unwrap().get("type").unwrap().to_string().replace('"', "").to_lowercase();
                                let m_help = mdef.get(m).unwrap().as_object().unwrap().get("help").unwrap().to_string().replace('"', "");
                                let m_value: f64 = v.as_f64().expect("Invalid metric value");

                                let metric: Metric = Metric::new(m_name, m_type, m_help, m_labels.clone(), m_value);
                                metric_list.push(metric);
                            }
                        } else {
                            error!("Metric: {} ({}) not found in the metric definition", m, stype);
                            continue;
                        }
                    }
                }
            }
        }
    }
    metric_list
}

/// Calculate IOPS from the *Bwc metrics
/// https://github.com/andrewjwhite/ScaleIO_RestAPI_Python_Examples/blob/master/ScaleIO_cluster_stats_example.py#L92-L108
fn iops_calc(occur: i32, secs: i32) -> f64 {
    if occur == 0 {
        0.0 as f64
    } else {
        (occur / secs) as f64
    }
}

/// Calculate Bandwidth MB/s from the *Bwc metrics
fn bw_calc(occur: i32, secs: i32) -> f64 {
    if occur == 0 {
        0.0 as f64
    } else {
        ((occur / secs) / 1024) as f64
    }
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

pub fn get_metrics(sio: &Arc<Mutex<sio::client::Client>>) -> Vec<Metric> {
    let (inst, rela) = get_instances(&sio);

    let labels = get_labels(&inst, &rela);
    debug!("Loaded labels for instances: {:?}", labels.keys().collect::<Vec<_>>());

    let ststs = sio.lock().unwrap().stats();
    debug!("Loaded ststs for instances: {:?}", ststs.keys().collect::<Vec<_>>());

    convert_metrics(&ststs, &labels)
}
