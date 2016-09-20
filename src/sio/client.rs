//! SIO Client
//!
//! The ScaleIO `Client`
//!

use std;
use std::cell::RefCell;
use std::collections::{HashMap, BTreeMap};
use std::fs::File;
use std::io::Read;
use std::sync::{Arc, Mutex};

extern crate core;
extern crate serde;
extern crate serde_json;

extern crate hyper_insecure_https_connector;
extern crate hyper;
use hyper::client;
use hyper::header::{Authorization, Basic, Headers, ContentType, UserAgent};
use hyper::status::StatusCode;


pub struct Client {
    gw: &'static str,
    user: &'static str,
    pass: &'static str,
    token: RefCell<Option<String>>,
}

impl Client {
    pub fn new(gw: &'static str, user: &'static str, pass: &'static str) -> Arc<Mutex<Client>> {
        Arc::new(Mutex::new(Client { gw: gw,
                                     user: user,
                                     pass: pass,
                                     token: RefCell::new(None), }))
    }

    pub fn connect(&self) -> Result<(), String> {
        let url = format!("https://{}/api/login", &self.gw);
        let mut buf = String::new();
        let mut headers = Headers::new();
        headers.set(Authorization(Basic { username: self.user.to_string(),
                                          password: Some(self.pass.to_string()), }));
        headers.set(UserAgent("sio2prom".to_string()));
        let client = client::Client::with_connector(hyper_insecure_https_connector::insecure_https_connector());

        let mut response = client.get(&url).headers(headers).send().unwrap_or_else(|e| {
            panic!("Failed to get {}; error is {}", url, e);
        });

        if response.status != StatusCode::Ok {
            panic!("Failed to get {}; error RC {}", url, response.status);
        }

        response.read_to_string(&mut buf).unwrap_or_else(|e| {
            panic!("Failed to read data: {}", e);
        });

        if buf.replace('"', "").to_string().is_empty() {
            Err(format!("Could not get Auth Token from {}", url))
        } else {
            *self.token.borrow_mut() = Some(buf.replace('"', ""));
            println!("Token: {}", self.token.borrow().clone().unwrap());
            Ok(())
        }
    }

    pub fn stats(&self) -> BTreeMap<String, serde_json::Value> {
        let url = format!("https://{}/api/instances/querySelectedStatistics", &self.gw);
        let mut buf = String::new();
        let mut headers = Headers::new();
        headers.set(ContentType::json());
        headers.set(Authorization(Basic { username: self.user.to_string(),
                                          password: Some(self.token.borrow().clone().unwrap()), }));
        headers.set(UserAgent("sio2prom".to_string()));

        let client = client::Client::with_connector(hyper_insecure_https_connector::insecure_https_connector());
        let query: String =
            Client::read_json("cfg/metric_query_selection.json").map(|q| serde_json::to_string(&q)).expect("Could not load the query (querySelectedStatistics)").unwrap();

        let mut response = client.post(&url).headers(headers).body(&query).send().unwrap_or_else(|e| {
            panic!("Failed to get stats {} error is {}", url, e);
        });

        if response.status == StatusCode::Unauthorized {
            println!("Requesting an new token");
            self.connect();

        } else if response.status != StatusCode::Ok {
            panic!("Failed to get {}; error RC {}", url, response.status);
        }

        response.read_to_string(&mut buf).unwrap_or_else(|e| {
            panic!("Failed to read data: {}", e);
        });

        let json: serde_json::Value = serde_json::from_str(&buf).unwrap_or_else(|e| {
            panic!("Failed to parse json; error is {}", e);
        });

        let data: &BTreeMap<String, serde_json::Value> = json.as_object().unwrap_or_else(|| {
            panic!("Failed to convert json to object");
        });

        data.clone()
    }

    pub fn instances(&self) -> BTreeMap<String, serde_json::Value> {
        let url = format!("https://{}/api/instances/", &self.gw);
        let mut buf = String::new();
        let mut headers = Headers::new();
        headers.set(ContentType::json());
        headers.set(Authorization(Basic { username: self.user.to_string(),
                                          password: Some(self.token.borrow().clone().unwrap()), }));
        headers.set(UserAgent("sio2prom".to_string()));

        let client = client::Client::with_connector(hyper_insecure_https_connector::insecure_https_connector());

        let mut response = client.get(&url).headers(headers).send().unwrap_or_else(|e| {
            panic!("Failed to get instances {}; error is {}", url, e);
        });

        if response.status == StatusCode::Unauthorized {
            println!("Requesting an new token");
            self.connect();
        } else if response.status != StatusCode::Ok {
            panic!("Failed to get {}; error RC {}", url, response.status);
        }

        response.read_to_string(&mut buf).unwrap_or_else(|e| {
            panic!("Failed to read data: {}", e);
        });

        let json: serde_json::Value = serde_json::from_str(&buf).unwrap_or_else(|e| {
            panic!("Failed to parse json; error is {}", e);
        });

        let data: &BTreeMap<String, serde_json::Value> = json.as_object().unwrap_or_else(|| {
            panic!("Failed to convert json to object");
        });

        data.clone()
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
}
