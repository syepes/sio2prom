use anyhow::{anyhow, Result};
use reqwest::StatusCode;
use serde_json::{value::Map, Value};
use std::{cell::RefCell, collections::HashMap, time::Duration};

#[derive(Debug, Default)]
pub struct ClientInfo<'a> {
  pub cfg_path: Option<&'a str>,
  pub ip:       Option<&'a str>,
  pub auth_usr: Option<&'a str>,
  pub auth_pwd: Option<&'a str>,
  token:        RefCell<Option<String>>,
}

impl<'a> ClientInfo<'a> {
  pub fn new(cfg_path: Option<&'a str>, ip: Option<&'a str>, auth_usr: Option<&'a str>, auth_pwd: Option<&'a str>) -> ClientInfo<'a> {
    ClientInfo { cfg_path,
                 ip,
                 auth_usr,
                 auth_pwd,
                 token: RefCell::new(None) }
  }

  async fn auth(&mut self) {
    trace!("auth");
    if self.token.borrow().is_none() {
      if let Ok(c) = reqwest::Client::builder().user_agent(env!("CARGO_PKG_NAME")).danger_accept_invalid_certs(true).timeout(Duration::from_secs(10)).connection_verbose(true).build() {
        if !self.auth_usr.unwrap().is_empty() && !self.auth_pwd.unwrap().is_empty() && self.token.borrow().is_none() {
          let req_url = format!("https://{ip}/api/login", ip = self.ip.unwrap());
          trace!("Auth on {:?} with {:?}/{:?}", req_url.clone(), self.auth_usr.unwrap().to_string(), self.auth_pwd.unwrap().to_string());

          let req = c.get(req_url).basic_auth(self.auth_usr.unwrap(), Some(self.auth_pwd.unwrap()));
          match req.send().await {
            Ok(r) => {
              trace!("resp:{:#?}", r);
              match r.status() {
                StatusCode::OK => {
                  match r.json::<serde_json::Value>().await {
                    Ok(t) => {
                      *self.token.borrow_mut() = Some(t.to_string().replace('"', ""));
                    },
                    _ => {
                      *self.token.borrow_mut() = None;
                    },
                  }
                },
                StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                  *self.token.borrow_mut() = None;
                  let msg: String = match r.json::<serde_json::Value>().await {
                    Ok(Value::Object(m)) => m.get("message").map(|m| m.to_string().replace('"', "")).unwrap_or_else(|| "unknown".to_string()),
                    _ => "unknown".to_string(),
                  };
                  error!("Auth failed: {:?}", msg);
                },
                _ => {
                  *self.token.borrow_mut() = None;
                  let msg: String = match r.json::<serde_json::Value>().await {
                    Ok(Value::Object(m)) => m.get("message").map(|m| m.to_string().replace('"', "")).unwrap_or_else(|| "unknown".to_string()),
                    _ => "unknown".to_string(),
                  };
                  error!("Unknown auth request error: {:?}", msg);
                },
              };
            },
            Err(e) => {
              *self.token.borrow_mut() = None;
              error!("Auth request error: {:?}", e.to_string());
            },
          }
        } else {
          *self.token.borrow_mut() = None;
          error!("Auth missing credentials");
        }
      }
    }

    debug!("Token:{:#?}", *self.token.borrow_mut());
  }

  pub async fn version(&mut self) -> Result<(), anyhow::Error> {
    trace!("version");
    self.auth().await;

    if let Ok(c) = reqwest::Client::builder().user_agent(env!("CARGO_PKG_NAME")).danger_accept_invalid_certs(true).timeout(Duration::from_secs(15)).connection_verbose(true).build() {
      if !self.auth_usr.unwrap().is_empty() && self.token.borrow().is_some() {
        let req_url = format!("https://{ip}/api/version", ip = self.ip.unwrap());
        let t = self.token.borrow().as_ref().unwrap().clone();
        trace!("Auth on {:?} with {:?}/{:?}", req_url.clone(), self.auth_usr, t);

        let req = c.get(req_url).basic_auth(self.auth_usr.unwrap(), Some(t));
        match req.send().await {
          Ok(r) => {
            trace!("resp:{:#?}", r);
            match r.status() {
              StatusCode::OK => {
                match r.text().await {
                  Ok(t) => {
                    info!("API Version: {}", t.replace('"', ""));
                    Ok(())
                  },
                  _ => Err(anyhow!("Failed to detect API version")),
                }
              },
              StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                *self.token.borrow_mut() = None;
                Err(anyhow!("Auth failed"))
              },
              _ => {
                let msg: String = match r.json::<serde_json::Value>().await {
                  Ok(Value::Object(m)) => m.get("message").map(|m| m.to_string().replace('"', "")).unwrap_or_else(|| "unknown".to_string()),
                  _ => "unknown".to_string(),
                };
                Err(anyhow!("Unknown instance request error: {:?}", msg))
              },
            }
          },
          Err(e) => Err(anyhow!("Instance request error: {:?}", e.to_string())),
        }
      } else {
        Err(anyhow!("Missing auth token"))
      }
    } else {
      Err(anyhow!("Cant build client"))
    }
  }

  async fn instances(&mut self) -> Result<Map<String, serde_json::Value>, anyhow::Error> {
    trace!("instances");
    if let Ok(c) = reqwest::Client::builder().user_agent(env!("CARGO_PKG_NAME")).danger_accept_invalid_certs(true).timeout(Duration::from_secs(15)).connection_verbose(true).build() {
      if !self.auth_usr.unwrap().is_empty() && self.token.borrow().is_some() {
        let req_url = format!("https://{ip}/api/instances", ip = self.ip.unwrap());
        let t = self.token.borrow().as_ref().unwrap().clone();
        trace!("Auth on {:?} with {:?}/{:?}", req_url.clone(), self.auth_usr, t);

        let req = c.get(req_url).basic_auth(self.auth_usr.unwrap(), Some(t));
        match req.send().await {
          Ok(r) => {
            trace!("resp:{:#?}", r);
            match r.status() {
              StatusCode::OK => {
                match r.json::<serde_json::Value>().await {
                  Ok(t) => {
                    trace!("data: {:#?}", t);
                    return Ok(t.as_object().unwrap().clone());
                  },
                  _ => Err(anyhow!("Failed to parse json")),
                }
              },
              StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                *self.token.borrow_mut() = None;
                Err(anyhow!("Auth failed"))
              },
              _ => {
                let msg: String = match r.json::<serde_json::Value>().await {
                  Ok(Value::Object(m)) => m.get("message").map(|m| m.to_string().replace('"', "")).unwrap_or_else(|| "unknown".to_string()),
                  _ => "unknown".to_string(),
                };
                error!("Unknown instance request error: {:?}", msg);
                Err(anyhow!("Unknown instance request error: {:?}", msg))
              },
            }
          },
          Err(e) => {
            error!("Instance request error: {:?}", e.to_string());
            Err(anyhow!("Instance request error: {:?}", e.to_string()))
          },
        }
      } else {
        Err(anyhow!("Missing auth token"))
      }
    } else {
      Err(anyhow!("Cant build client"))
    }
  }

  async fn stats(&mut self) -> Result<Map<String, serde_json::Value>, anyhow::Error> {
    trace!("stats");
    if let Ok(c) = reqwest::Client::builder().user_agent(env!("CARGO_PKG_NAME")).danger_accept_invalid_certs(true).timeout(Duration::from_secs(15)).connection_verbose(true).build() {
      if !self.auth_usr.unwrap().is_empty() && self.token.borrow().is_some() {
        let req_url = format!("https://{ip}/api/instances/querySelectedStatistics", ip = self.ip.unwrap());
        let t = self.token.borrow().as_ref().unwrap().clone();
        trace!("Auth on {:?} with {:?}/{:?}", req_url.clone(), self.auth_usr, t);

        let path = format!("{}{}", self.cfg_path.unwrap(), "/metric_query_selection.json");
        let query = super::utils::read_json(&path).expect("Could not load the query (querySelectedStatistics)");
        trace!("query: {:#?}", query);

        let req = c.post(req_url).basic_auth(self.auth_usr.unwrap(), Some(t));
        match req.json(&query).send().await {
          Ok(r) => {
            trace!("resp:{:#?}", r);
            match r.status() {
              StatusCode::OK => {
                match r.json::<serde_json::Value>().await {
                  Ok(t) => {
                    trace!("data: {:#?}", t);
                    Ok(t.as_object().unwrap().clone())
                  },
                  _ => Err(anyhow!("Failed to parse json")),
                }
              },
              StatusCode::BAD_REQUEST => {
                let msg: String = match r.json::<serde_json::Value>().await {
                  Ok(Value::Object(m)) => m.get("message").map(|m| m.to_string().replace('"', "")).unwrap_or_else(|| "unknown".to_string()),
                  _ => "unknown".to_string(),
                };

                error!("request failed incorrect stats query, verify the file (metric_query_selection.json) definitions: {:#?}", msg);
                Err(anyhow!("request failed incorrect stats query: {:?}", msg))
              },
              StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(anyhow!("Auth failed")),
              _ => {
                let msg: String = match r.json::<serde_json::Value>().await {
                  Ok(Value::Object(m)) => m.get("message").map(|m| m.to_string().replace('"', "")).unwrap_or_else(|| "unknown".to_string()),
                  _ => "unknown".to_string(),
                };
                error!("Unknown stats request error: {:?}", msg);
                Err(anyhow!("Unknown stats request error: {:?}", msg))
              },
            }
          },
          Err(e) => {
            error!("Stats request error: {:?}", e.to_string());
            Err(anyhow!("Stats request error: {:?}", e.to_string()))
          },
        }
      } else {
        Err(anyhow!("Missing auth token"))
      }
    } else {
      Err(anyhow!("Cant build client"))
    }
  }

  /// Query `ScaleIO` instances and find their relationships
  fn relations(&mut self, instances: &Map<String, serde_json::Value>) -> Result<HashMap<&'static str, HashMap<String, HashMap<String, Vec<String>>>>, String> {
    trace!("relations");
    let mut relations: HashMap<&'static str, HashMap<String, HashMap<String, Vec<String>>>> = HashMap::new();
    relations.entry("childs").or_insert_with(HashMap::new);
    relations.entry("parents").or_insert_with(HashMap::new);

    // Get relations of all the elements
    for (key, value) in instances.iter() {
      if value.is_array() {
        for items in value.as_array().unwrap().iter() {
          let item_type: String = key.replace("List", "").to_string().replace('"', "").to_lowercase();
          let item_id: String = items.as_object().and_then(|o| o.get("id").map(|s| s.to_string().replace('"', ""))).expect("item_id Not found");
          let item_name: String = items.as_object().and_then(|o| o.get("name").map(|s| s.to_string().replace('"', ""))).unwrap_or_else(|| "unknown_item".to_string());
          trace!("Instance item type: {} / name: {} / id: {}", item_type, item_name, item_id);

          let items_links = match items.get("links").and_then(|v| v.as_array()) {
            None => {
              error!("Cound not find links for instance item type: {} / name: {} / id: {}", item_type, item_name, item_id);
              continue;
            },
            Some(l) => l,
          };

          for links in items_links.iter() {
            let link = links.as_object().unwrap();
            if !link["rel"].to_string().replace('"', "").starts_with("/api/parent") {
              continue;
            }

            let parent_type: String = link["href"].to_string().split(':').next().unwrap().split('/').last().unwrap().to_string().replace('"', "").to_lowercase();
            let parent_id: String = link["href"].to_string().split(':').last().unwrap().to_string().replace('"', "");

            {
              let a = relations.get_mut("childs").unwrap().entry(parent_id.clone()).or_insert_with(HashMap::new).entry(item_type.clone()).or_insert_with(Vec::new);
              a.push(item_id.clone());
            }
            {
              let a = relations.get_mut("parents").unwrap().entry(item_id.clone()).or_insert_with(HashMap::new).entry(parent_type.clone()).or_insert_with(Vec::new);
              a.push(parent_id.clone());
            }
          }
        }
      }
    }

    if relations["parents"].is_empty() || relations["childs"].is_empty() {
      error!("Found Instance relationships Parent: {} / Child: {} relations", relations["parents"].len(), relations["childs"].len());
      Err("Instance relationships not found".to_string())
    } else {
      info!("Found Instance relationships Parent: {} / Child: {} relations", relations["parents"].len(), relations["childs"].len());
      Ok(relations)
    }
  }

  /// Generate Prometheus.io labels from `ScaleIO` instances and relations
  fn labels(&mut self, instances: &Map<String, serde_json::Value>, relations: &HashMap<&'static str, HashMap<String, HashMap<String, Vec<String>>>>) -> Result<HashMap<&'static str, HashMap<String, HashMap<&'static str, String>>>, String> {
    trace!("labels");
    let default_val = vec![serde_json::Value::Null];
    let mut labels: HashMap<&'static str, HashMap<String, HashMap<&'static str, String>>> = HashMap::new();
    let clu_id = instances.get("System").and_then(|o| o.as_object().and_then(|j| j.get("id")).map(|s| s.to_string().replace('"', ""))).expect("clu_id Not found");
    let clu_name = match instances.get("System").and_then(|o| o.as_object().and_then(|j| j.get("name")).map(|s| s.to_string().replace('"', ""))) {
      None => {
        warn!("clu_name Not found using clu_id as name");
        None
      },
      Some(s) => Some(s),
    };

    let clu_name = if let Some(id) = clu_name { id } else { clu_name.unwrap() };

    // System
    {
      let mut label: HashMap<&'static str, String> = HashMap::new();
      label.entry("clu_name").or_insert_with(|| clu_name.to_string());
      label.entry("clu_id").or_insert_with(|| clu_id.to_string());

      labels.entry("System").or_insert_with(HashMap::new).entry("System".to_string()).or_insert_with(|| label);
    }
    // Sdr
    for sdr in instances.get("sdrList").and_then(|v| v.as_array()).unwrap_or_else(|| {
                                                                    warn!("Failed to get 'sdrList' from instances");
                                                                    &default_val
                                                                  })
    {
      for sdr in sdr.as_object().iter() {
        let mut label: HashMap<&'static str, String> = HashMap::new();
        let sdr_name = sdr.get("name").map(|s| s.to_string().replace('"', "")).expect("sdr_name Not found");
        let sdr_id = sdr.get("id").map(|s| s.to_string().replace('"', "")).expect("sdr_id Not found");

        label.entry("clu_name").or_insert_with(|| clu_name.to_string());
        label.entry("clu_id").or_insert_with(|| clu_id.to_string());
        label.entry("sdr_name").or_insert_with(|| sdr_name);
        label.entry("sdr_id").or_insert_with(|| sdr_id.to_string());

        labels.entry("sdr").or_insert_with(HashMap::new).entry(sdr_id).or_insert_with(|| label);
      }
    }
    // Sdc
    for sdc in instances.get("sdcList").and_then(|v| v.as_array()).unwrap_or_else(|| {
                                                                    error!("Failed to get 'sdcList' from instances");
                                                                    &default_val
                                                                  })
    {
      for sdc in sdc.as_object().iter() {
        let mut label: HashMap<&'static str, String> = HashMap::new();
        let sdc_name = sdc.get("name").map(|s| s.to_string().replace('"', "")).expect("sdc_name Not found");
        let sdc_id = sdc.get("id").map(|s| s.to_string().replace('"', "")).expect("sdc_id Not found");

        label.entry("clu_name").or_insert_with(|| clu_name.to_string());
        label.entry("clu_id").or_insert_with(|| clu_id.to_string());
        label.entry("sdc_name").or_insert_with(|| sdc_name);
        label.entry("sdc_id").or_insert_with(|| sdc_id.to_string());

        labels.entry("sdc").or_insert_with(HashMap::new).entry(sdc_id).or_insert_with(|| label);
      }
    }
    // ProtectionDomain
    for pd in instances.get("protectionDomainList").and_then(|v| v.as_array()).unwrap_or_else(|| {
                                                                                error!("Failed to get 'protectionDomainList' from instances");
                                                                                &default_val
                                                                              })
    {
      for pdo in pd.as_object().iter() {
        let mut label: HashMap<&'static str, String> = HashMap::new();
        let pdo_name = pdo.get("name").map(|s| s.to_string().replace('"', "")).expect("pdo_name Not found");
        let pdo_id = pdo.get("id").map(|s| s.to_string().replace('"', "")).expect("pdo_id Not found");

        label.entry("clu_name").or_insert_with(|| clu_name.to_string());
        label.entry("clu_id").or_insert_with(|| clu_id.to_string());
        label.entry("pdo_name").or_insert_with(|| pdo_name.to_string());
        label.entry("pdo_id").or_insert_with(|| pdo_id.to_string());

        labels.entry("protectiondomain").or_insert_with(HashMap::new).entry(pdo_id).or_insert_with(|| label);
      }
    }
    // StoragePool
    for spl in instances.get("storagePoolList").and_then(|v| v.as_array()).unwrap_or_else(|| {
                                                                            error!("Failed to get 'storagePoolList' from instances");
                                                                            &default_val
                                                                          })
    {
      let mut parent: HashMap<&'static str, String> = HashMap::new();

      for sp in spl.as_object().iter() {
        let mut label: HashMap<&'static str, String> = HashMap::new();
        let sp_name = sp.get("name").map(|s| s.to_string().replace('"', "")).expect("sp_name Not found");
        let sp_id = sp.get("id").map(|s| s.to_string().replace('"', "")).expect("sp_id Not found");

        for pd in instances["protectionDomainList"].as_array().unwrap().iter() {
          for pdo in pd.as_object().iter() {
            if relations["parents"][&sp_id]["protectiondomain"].contains(&pdo["id"].to_string().replace('"', "")) {
              parent.entry("name").or_insert_with(|| pdo["name"].to_string().replace('"', ""));
              parent.entry("id").or_insert_with(|| pdo["id"].to_string().replace('"', ""));
              break;
            }
          }
        }

        label.entry("clu_name").or_insert_with(|| clu_name.to_string());
        label.entry("clu_id").or_insert_with(|| clu_id.to_string());
        label.entry("sto_name").or_insert_with(|| sp_name);
        label.entry("sto_id").or_insert_with(|| sp_id.to_string());
        label.entry("pdo_name").or_insert_with(|| parent["name"].to_string());
        label.entry("pdo_id").or_insert_with(|| parent["id"].to_string());

        labels.entry("storagepool").or_insert_with(HashMap::new).entry(sp_id).or_insert_with(|| label);
      }
    }
    // Sds
    for sdsl in instances.get("sdsList").and_then(|v| v.as_array()).unwrap_or_else(|| {
                                                                     error!("Failed to get 'sdsList' from instances");
                                                                     &default_val
                                                                   })
    {
      let mut parent: HashMap<&'static str, String> = HashMap::new();

      for sds in sdsl.as_object().iter() {
        let mut label: HashMap<&'static str, String> = HashMap::new();
        let sds_name = sds.get("name").map(|s| s.to_string().replace('"', "")).expect("sds_name Not found");
        let sds_id = sds.get("id").map(|s| s.to_string().replace('"', "")).expect("sds_id Not found");

        for pd in instances["protectionDomainList"].as_array().unwrap().iter() {
          for pdo in pd.as_object().iter() {
            if relations["parents"][&sds_id]["protectiondomain"].contains(&pdo["id"].to_string().replace('"', "")) {
              parent.entry("name").or_insert_with(|| pdo["name"].to_string().replace('"', ""));
              parent.entry("id").or_insert_with(|| pdo["id"].to_string().replace('"', ""));
              break;
            }
          }
        }

        label.entry("clu_name").or_insert_with(|| clu_name.to_string());
        label.entry("clu_id").or_insert_with(|| clu_id.to_string());
        label.entry("sds_name").or_insert_with(|| sds_name);
        label.entry("sds_id").or_insert_with(|| sds_id.to_string());
        label.entry("pdo_name").or_insert_with(|| parent["name"].to_string());
        label.entry("pdo_id").or_insert_with(|| parent["id"].to_string());

        labels.entry("sds").or_insert_with(HashMap::new).entry(sds_id).or_insert_with(|| label);
      }
    }
    // Volumes
    for vl in instances.get("volumeList").and_then(|v| v.as_array()).unwrap_or_else(|| {
                                                                      error!("Failed to get 'volumeList' from instances");
                                                                      &default_val
                                                                    })
    {
      let mut parent_sto: HashMap<&'static str, String> = HashMap::new();
      let mut parent_pdo: HashMap<&'static str, String> = HashMap::new();

      for vol in vl.as_object().iter() {
        let mut label: HashMap<&'static str, String> = HashMap::new();
        let vol_name = vol.get("name").map(|s| s.to_string().replace('"', "")).expect("vol_name Not found");
        let vol_id = vol.get("id").map(|s| s.to_string().replace('"', "")).expect("vol_id Not found");
        let vol_type = vol.get("volumeType").map(|s| s.to_string().replace('"', "")).expect("vol_type Not found");

        for sp in instances["storagePoolList"].as_array().unwrap().iter() {
          for sto in sp.as_object().iter() {
            if relations["parents"][&vol_id]["storagepool"].contains(&sto["id"].to_string().replace('"', "")) {
              parent_sto.entry("name").or_insert_with(|| sto["name"].to_string().replace('"', ""));
              parent_sto.entry("id").or_insert_with(|| sto["id"].to_string().replace('"', ""));
              break;
            }
          }
        }
        for pd in instances["protectionDomainList"].as_array().unwrap().iter() {
          for pdo in pd.as_object().iter() {
            if relations["parents"][&parent_sto["id"].to_string().replace('"', "")]["protectiondomain"].contains(&pdo["id"].to_string().replace('"', "")) {
              parent_pdo.entry("name").or_insert_with(|| pdo["name"].to_string().replace('"', ""));
              parent_pdo.entry("id").or_insert_with(|| pdo["id"].to_string().replace('"', ""));
              break;
            }
          }
        }

        label.entry("clu_name").or_insert_with(|| clu_name.to_string());
        label.entry("clu_id").or_insert_with(|| clu_id.to_string());
        label.entry("vol_name").or_insert_with(|| vol_name);
        label.entry("vol_id").or_insert_with(|| vol_id.to_string());
        label.entry("vol_type").or_insert_with(|| vol_type.to_string());
        label.entry("sto_name").or_insert_with(|| parent_sto["name"].to_string());
        label.entry("sto_id").or_insert_with(|| parent_sto["id"].to_string());
        label.entry("pdo_name").or_insert_with(|| parent_pdo["name"].to_string());
        label.entry("pdo_id").or_insert_with(|| parent_pdo["id"].to_string());

        labels.entry("volume").or_insert_with(HashMap::new).entry(vol_id).or_insert_with(|| label);
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

        for sdsl in instances["sdsList"].as_array().unwrap().iter() {
          for sds in sdsl.as_object().iter() {
            if relations["parents"][&dev_id]["sds"].contains(&sds["id"].to_string().replace('"', "")) {
              parent_sds.entry("name").or_insert_with(|| sds["name"].to_string().replace('"', ""));
              parent_sds.entry("id").or_insert_with(|| sds["id"].to_string().replace('"', ""));
              break;
            }
          }
        }
        for sp in instances["storagePoolList"].as_array().unwrap().iter() {
          for sto in sp.as_object().iter() {
            if relations["parents"][&dev_id]["storagepool"].contains(&sto["id"].to_string().replace('"', "")) {
              parent_sto.entry("name").or_insert_with(|| sto["name"].to_string().replace('"', ""));
              parent_sto.entry("id").or_insert_with(|| sto["id"].to_string().replace('"', ""));
              break;
            }
          }
        }
        for pd in instances["protectionDomainList"].as_array().unwrap().iter() {
          for pdo in pd.as_object().iter() {
            if relations["parents"][&parent_sto["id"].to_string().replace('"', "")]["protectiondomain"].contains(&pdo["id"].to_string().replace('"', "")) {
              parent_pdo.entry("name").or_insert_with(|| pdo["name"].to_string().replace('"', ""));
              parent_pdo.entry("id").or_insert_with(|| pdo["id"].to_string().replace('"', ""));
              break;
            }
          }
        }

        label.entry("clu_name").or_insert_with(|| clu_name.to_string());
        label.entry("clu_id").or_insert_with(|| clu_id.to_string());
        label.entry("dev_name").or_insert_with(|| dev_name);
        label.entry("dev_id").or_insert_with(|| dev_id.to_string());
        label.entry("dev_path").or_insert_with(|| dev_path);

        match parent_sds.get("name") {
          None => {
            error!("Failed to get 'name' from parent_sds");
            continue;
          },
          Some(o) => {
            label.entry("sds_name").or_insert_with(|| o.to_string());
          },
        }

        match parent_sds.get("id") {
          None => {
            error!("Failed to get 'id' from parent_sds");
            continue;
          },
          Some(o) => {
            label.entry("sds_id").or_insert_with(|| o.to_string());
          },
        }

        match parent_sto.get("name") {
          None => {
            error!("Failed to get 'name' from parent_sto");
            continue;
          },
          Some(o) => {
            label.entry("sto_name").or_insert_with(|| o.to_string());
          },
        }
        match parent_sto.get("id") {
          None => {
            error!("Failed to get 'id' from parent_sto");
            continue;
          },
          Some(o) => {
            label.entry("sto_id").or_insert_with(|| o.to_string());
          },
        }

        match parent_pdo.get("name") {
          None => {
            error!("Failed to get 'name' from parent_pdo");
            continue;
          },
          Some(o) => {
            label.entry("pdo_name").or_insert_with(|| o.to_string());
          },
        }
        match parent_pdo.get("id") {
          None => {
            error!("Failed to get 'id' from parent_pdo");
            continue;
          },
          Some(o) => {
            label.entry("pdo_id").or_insert_with(|| o.to_string());
          },
        }

        labels.entry("device").or_insert_with(HashMap::new).entry(dev_id).or_insert_with(|| label);
      }
    }

    if labels.is_empty() {
      error!("Could not generate labels");
      Err("Could not generate labels".to_string())
    } else {
      Ok(labels)
    }
  }

  // pub fn metrics(&mut self) -> Option<Vec<Metric>> {
  pub async fn metrics(&mut self) -> Option<Vec<super::metrics::Metric>> {
    self.auth().await;

    let inst = self.instances().await;
    if inst.is_err() {
      return None;
    }
    info!("Loaded instances: {:?}", &inst.as_ref().unwrap().keys().collect::<Vec<_>>());

    let rela = self.relations(inst.as_ref().unwrap());
    if rela.is_err() {
      return None;
    }

    let labels = self.labels(inst.as_ref().unwrap(), rela.as_ref().unwrap());
    if labels.is_err() {
      return None;
    }
    info!("Loaded labels: {:?}", &labels.as_ref().unwrap().keys().collect::<Vec<_>>());

    let stats = self.stats().await;
    if stats.is_err() {
      return None;
    }
    info!("Loaded stats: {:?}", stats.as_ref().unwrap().keys().collect::<Vec<_>>());

    super::metrics::get(self.cfg_path, &inst, &stats, &labels, &rela)
  }
}
