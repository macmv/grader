use std::{cell::OnceCell, collections::HashMap, path::PathBuf};

use anyhow::Context;
use serde::Deserialize;

use crate::{download::User, settings::Settings};

pub struct Workspace {
  pub token: String,
  pub root:  PathBuf,
}

pub struct Course<'a> {
  pub workspace: &'a Workspace,

  pub path:     PathBuf,
  pub settings: Settings,
  user_data:    OnceCell<Users>,
}

pub struct Assignment<'a> {
  pub course: &'a Course<'a>,

  pub path: PathBuf,
  pub id:   u32,
}

pub struct Users {
  users: HashMap<UserId, User>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(transparent)]
pub struct UserId(pub u32);

impl Course<'_> {
  pub fn users(&self) -> &HashMap<UserId, User> {
    &self.user_data.get_or_init(|| self.fetch_users()).users
  }

  pub fn current_assignment(&self) -> anyhow::Result<Assignment<'_>> {
    let pwd = std::env::current_dir()?;
    let relative = pwd
      .strip_prefix(&self.path)
      .map_err(|_| anyhow::anyhow!("current directory is not inside this course"))?;
    let name = relative
      .components()
      .next()
      .ok_or_else(|| anyhow::anyhow!("current directory is the course root, not an assignment"))?;
    self.assignment(name.as_os_str().to_str().unwrap())
  }

  pub fn assignment(&self, name: &str) -> anyhow::Result<Assignment<'_>> {
    let id = self
      .settings
      .assignment
      .get(name)
      .ok_or_else(|| anyhow::anyhow!("assignment '{name}' not found in settings.toml"))?
      .id;

    Ok(Assignment { course: self, path: self.path.join(name), id })
  }
}

impl Workspace {
  pub fn new() -> Self {
    let root = PathBuf::from("/home/macmv/Desktop/school/wwu/ta");
    let token = std::fs::read_to_string(root.join("token.txt")).unwrap().trim().to_string();

    Workspace { root, token }
  }

  pub fn current_course(&self) -> anyhow::Result<Course<'_>> {
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

  pub fn course(&self, name: &str) -> anyhow::Result<Course<'_>> {
    let valid = name.len() == 8
      && name[..4].chars().all(|c| c.is_ascii_uppercase())
      && name.as_bytes()[4] == b'-'
      && name[5..].chars().all(|c| c.is_ascii_digit());
    anyhow::ensure!(valid, "invalid course name '{name}': expected format like 'CSCI-101'");

    let path = self.root.join(name);
    let settings_str = std::fs::read_to_string(path.join("settings.toml"))
      .context("failed to read settings.toml. is this course setup?")?;
    let settings = toml::from_str(&settings_str)?;
    Ok(Course { workspace: self, path, settings, user_data: OnceCell::new() })
  }
}

impl Users {
  pub fn from_vec(users: Vec<User>) -> Self {
    Users { users: users.into_iter().map(|u| (u.id, u)).collect() }
  }
}
