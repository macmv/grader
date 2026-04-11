use std::collections::HashMap;

#[derive(serde::Deserialize)]
pub struct Settings {
  pub course:  u32,
  pub section: u32,

  pub assignment: HashMap<String, Assignment>,
}

#[derive(serde::Deserialize)]
pub struct Assignment {
  pub id: u32,
}
