use std::collections::HashMap;

#[derive(serde::Deserialize)]
#[allow(dead_code)]
pub struct Settings {
  pub course:  u32,
  pub section: u32,

  pub assignment: HashMap<String, Assignment>,
}

#[derive(Clone, serde::Deserialize)]
pub struct Assignment {
  pub id:       u32,
  pub compile:  String,
  pub filename: Option<String>,
}
