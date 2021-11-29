//! SIO Utils

use serde_json::value::Map;
use std::{fs::File, io::Read};

extern crate serde;
extern crate serde_json;

/// Read json file using `serde_json`
pub fn read_json(file: &str) -> Option<Map<String, serde_json::Value>> {
  match File::open(file) {
    Err(e) => panic!("Failed to open file: {}, {:?}", file, e.kind()),
    Ok(mut f) => {
      let mut content: String = String::new();
      f.read_to_string(&mut content).expect("Error reading file");
      let j: serde_json::Value = serde_json::from_str::<serde_json::Value>(&content).unwrap_or_else(|_| panic!("Can't deserialize json file {}", file));
      Some(j.as_object().unwrap().clone())
    },
  }
}
