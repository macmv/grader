use std::collections::HashMap;

#[derive(serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(dead_code)]
pub struct Settings {
  pub course:  u32,
  pub section: u32,

  /// Run the compile command on this machine instead of over ssh.
  #[serde(default)]
  pub local_compile: bool,

  pub assignment: HashMap<String, Assignment>,
}

#[derive(Clone, Default, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Download {
  /// Download into `<student>/<file>` instead of `<student>-<file>`.
  #[serde(default)]
  pub separate_directories: bool,
}

#[derive(Clone, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Assignment {
  pub id:       u32,
  #[serde(default)]
  pub compile:  String,
  pub filename: Option<String>,

  #[serde(default)]
  pub separate_directories: bool,

  #[serde(default)]
  pub download: Download,
}
