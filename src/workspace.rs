use std::{cell::OnceCell, collections::HashMap, path::PathBuf};

use anyhow::Context;
use serde::Deserialize;

use crate::{download::User, settings::Settings};

pub struct Workspace {
  pub root: PathBuf,
}

pub struct Course {
  pub path:     PathBuf,
  pub settings: Settings,
  user_data:    OnceCell<Users>,
}

pub struct Users {
  users: HashMap<UserId, User>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(transparent)]
pub struct UserId(pub u32);

impl Course {
  pub fn users(&self) -> &HashMap<UserId, User> {
    &self.user_data.get_or_init(|| self.fetch_users()).users
  }
}

impl Workspace {
  pub fn new() -> Self { Workspace { root: PathBuf::from("/home/macmv/Desktop/school/wwu/ta") } }

  pub fn current_course(&self) -> anyhow::Result<Course> {
    let pwd = std::env::current_dir()?;
    let relative = pwd
      .strip_prefix(&self.root)
      .map_err(|_| anyhow::anyhow!("current directory is not inside the workspace"))?;
    let name = relative
      .components()
      .next()
      .ok_or_else(|| anyhow::anyhow!("current directory is the workspace root, not a course"))?;
    self.course(name.as_os_str().to_str().unwrap())
  }

  pub fn course(&self, name: &str) -> anyhow::Result<Course> {
    let valid = name.len() == 8
      && name[..4].chars().all(|c| c.is_ascii_uppercase())
      && name.as_bytes()[4] == b'-'
      && name[5..].chars().all(|c| c.is_ascii_digit());
    anyhow::ensure!(valid, "invalid course name '{name}': expected format like 'CSCI-101'");

    let path = self.root.join(name);
    let settings_str = std::fs::read_to_string(path.join("settings.toml"))
      .context("failed to read settings.toml. is this course setup?")?;
    let settings = toml::from_str(&settings_str)?;
    Ok(Course { path, settings, user_data: OnceCell::new() })
  }
}

impl Users {
  pub fn from_vec(users: Vec<User>) -> Self {
    Users { users: users.into_iter().map(|u| (u.id, u)).collect() }
  }
}
