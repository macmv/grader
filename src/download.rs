use std::{
  path::{Path, PathBuf},
  sync::{Arc, Mutex},
  thread::JoinHandle,
};

use owo_colors::OwoColorize;

use crate::{
  ui::Table,
  workspace::{Assignment, Course, UserId, Users},
};

#[derive(serde::Deserialize)]
pub struct Submission {
  pub user_id:     UserId,
  pub score:       Option<f32>,
  #[serde(default)]
  pub attachments: Vec<Attachment>,
}

#[derive(Clone, serde::Deserialize)]
pub struct Attachment {
  pub display_name: String,
  pub url:          String,
}

#[derive(Clone, serde::Deserialize, Debug)]
pub struct User {
  pub id:            UserId,
  pub name:          String,
  pub sortable_name: String,
}

pub fn list_sections(course: &str) {
  let token = std::fs::read_to_string("../token.txt").unwrap().trim().to_string();

  let res = ureq::get(format!("https://wwu.instructure.com/api/v1/courses/{course}/sections"))
    .header("Authorization", &format!("Bearer {token}"))
    .header("Accept", "application/json")
    .call()
    .unwrap()
    .body_mut()
    .read_to_string()
    .unwrap();

  println!("{res}");
}

impl Course<'_> {
  pub fn fetch_users(&self) -> Users {
    let users: Vec<User> = ureq::get(format!(
      "https://wwu.instructure.com/api/v1/sections/{section}/users?per_page=100",
      section = self.settings.section
    ))
    .header("Authorization", &format!("Bearer {}", self.workspace.token))
    .header("Accept", "application/json")
    .call()
    .unwrap()
    .body_mut()
    .read_json()
    .unwrap();

    Users::from_vec(users)
  }
}

impl Assignment<'_> {
  pub fn download_submissions(&self, dry_run: bool) {
    let mut submissions: Vec<Submission> = ureq::get(format!(
      "https://wwu.instructure.com/api/v1/sections/{section}/assignments/{assignment}/submissions?per_page=100",
      section = self.course.settings.section,
      assignment = self.settings.id,
    ))
    .header("Authorization", &format!("Bearer {}", self.course.workspace.token))
    .header("Accept", "application/json")
    .call()
    .unwrap()
    .body_mut()
    .read_json()
    .unwrap();

    let users = self.course.users();
    submissions.sort_by_key(|s| {
      let user = &users[&s.user_id];
      (user.name == "Test Student", &user.sortable_name)
    });

    std::fs::create_dir_all(&self.path).unwrap();

    let mut table = Table::new(&["Name", "Filename", "Score", "Status"]);
    for s in &submissions {
      let user = &users[&s.user_id];

      let score = match s.score {
        Some(s) => format!("{s}"),
        None => "<not graded>".to_string(),
      };
      if s.attachments.is_empty() {
        table.add_row(&[&user.name, "<not submitted>", &score, ""]);
      } else {
        table.add_row(&[
          &user.name,
          &self.attachment_filename(user, &s).unwrap_or_else(|e| e),
          &score,
          "...",
        ]);
      };
    }

    table.display();

    let table = Arc::new(Mutex::new(table));
    let mut handles = vec![];

    for (i, s) in submissions.iter().enumerate() {
      let Ok(attachments) = self.selected_attachments(s) else { continue };
      let user = &users[&s.user_id];

      // A row's status is the "most changed" status of any of its files.
      let status = Arc::new(Mutex::new(Status::Unchanged));

      for attachment in attachments {
        let table = table.clone();
        let status = status.clone();

        handles.push(self.spawn_download(
          self.submission_path(user, attachment),
          attachment.url.clone(),
          dry_run,
          move |file_status| {
            let mut status = status.lock().unwrap();
            *status = (*status).max(file_status);
            table.lock().unwrap().update_row(i, |row| row.cols[3] = status.label());
          },
        ));
      }
    }

    handles.into_iter().for_each(|h| h.join().unwrap());
  }

  fn submission_path(&self, user: &User, attachment: &Attachment) -> PathBuf {
    let student = snakeify(&user.sortable_name);
    if self.settings.download.separate_directories {
      self.path.join(student).join(&attachment.display_name)
    } else {
      self.path.join(format!("{student}-{}", attachment.display_name))
    }
  }

  fn submission_filename(&self, user: &User, attachment: &Attachment) -> String {
    let path = self.submission_path(user, attachment);
    path.strip_prefix(&self.path).unwrap_or(&path).to_string_lossy().to_string()
  }

  fn attachment_filename(&self, user: &User, s: &Submission) -> Result<String, String> {
    let names: Vec<_> = self
      .selected_attachments(s)?
      .into_iter()
      .map(|a| self.submission_filename(user, a))
      .collect();
    Ok(names.join(", "))
  }

  /// The attachments to download for a submission. With `separate_directories`,
  /// every attachment is downloaded (they get grouped per-student on the
  /// remote). Otherwise, exactly one attachment is picked.
  fn selected_attachments<'a>(&self, s: &'a Submission) -> Result<Vec<&'a Attachment>, String> {
    if self.settings.separate_directories {
      Ok(s.attachments.iter().collect())
    } else {
      Ok(vec![&s.attachments[self.find_attachment(s)?]])
    }
  }

  fn find_attachment(&self, s: &Submission) -> Result<usize, String> {
    let res = if let Some(filename) = &self.settings.filename {
      match s.attachments.iter().position(|a| filename_matches(&a.display_name, filename)) {
        Some(i) => Ok(i),
        None if s.attachments.len() == 1 => Ok(0),
        None => Err(format!("error: couldn't find \"{}\" in attachments", filename)),
      }
    } else {
      if s.attachments.len() != 1 {
        Err(String::from("error: multiple files submitted"))
      } else {
        Ok(0)
      }
    };

    res.map_err(|e| {
      format!("{e}: {:?}", s.attachments.iter().map(|a| &a.display_name).collect::<Vec<_>>())
    })
  }

  fn spawn_download(
    &self,
    path: PathBuf,
    url: String,
    dry_run: bool,
    on_complete: impl FnOnce(Status) + Send + 'static,
  ) -> JoinHandle<()> {
    let token = self.course.workspace.token.clone();

    std::thread::spawn(move || {
      let content = ureq::get(&url)
        .header("Authorization", &format!("Bearer {token}"))
        .call()
        .unwrap()
        .body_mut()
        .read_to_vec()
        .unwrap();

      on_complete(Status::of(&path, &content));

      if !dry_run {
        if let Some(parent) = path.parent() {
          std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, &content).unwrap();
      }
    })
  }
}

/// How a downloaded file compares to what is already on disk. Ordered by how
/// much attention it deserves.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Status {
  Unchanged,
  Changed,
  New,
}

impl Status {
  fn of(path: &Path, content: &[u8]) -> Status {
    if !path.exists() {
      Status::New
    } else if std::fs::read(path).unwrap() != content {
      Status::Changed
    } else {
      Status::Unchanged
    }
  }

  fn label(self) -> String {
    match self {
      Status::New => "new".yellow().to_string(),
      Status::Changed => "changed".yellow().to_string(),
      Status::Unchanged => "unchanged".green().to_string(),
    }
  }
}

// Matches 'foo.c' against 'foo.c', 'foo-1.c', 'foo-2.c', etc.
// Also works without extensions: 'foo' matches 'foo', 'foo-1', etc.
pub fn filename_matches(display_name: &str, expected: &str) -> bool {
  let display = display_name.to_lowercase();
  let exp = expected.to_lowercase();

  let (exp_stem, exp_ext) = match exp.rfind('.') {
    Some(i) => (&exp[..i], Some(&exp[i..])),
    None => (exp.as_str(), None),
  };

  let (display_stem, display_ext) = match display.rfind('.') {
    Some(i) => (&display[..i], Some(&display[i..])),
    None => (display.as_str(), None),
  };

  if display_ext != exp_ext {
    return false;
  }

  if display_stem == exp_stem {
    return true;
  }

  if let Some(rest) = display_stem.strip_prefix(exp_stem) {
    let bytes = rest.as_bytes();
    if bytes.first() == Some(&b'-')
      && bytes[1..].iter().all(|b| b.is_ascii_digit())
      && bytes.len() > 1
    {
      return true;
    }
  }

  false
}

fn snakeify(name: &str) -> String {
  let mut result = String::new();
  for c in name.chars() {
    if c.is_ascii_alphanumeric() {
      result.push(c.to_ascii_lowercase());
    } else if c == ' ' {
      result.push('-');
    }
  }
  result
}
