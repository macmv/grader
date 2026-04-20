use std::sync::{Arc, Mutex};

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
      if s.attachments.is_empty() {
        continue;
      }

      let Ok(attachment) = self.find_attachment(s) else { continue };
      let user = users[&s.user_id].clone();
      let attachment = s.attachments[attachment].clone();

      let token = self.course.workspace.token.clone();
      let table = table.clone();
      let path =
        self.path.join(format!("{}-{}", snakeify(&user.sortable_name), attachment.display_name));

      handles.push(std::thread::spawn(move || {
        let content = ureq::get(&attachment.url)
          .header("Authorization", &format!("Bearer {token}"))
          .call()
          .unwrap()
          .body_mut()
          .read_to_vec()
          .unwrap();

        let status = if !path.exists() {
          "new".yellow().to_string()
        } else {
          let existing = std::fs::read(&path).unwrap();
          if existing != content {
            "changed".yellow().to_string()
          } else {
            "unchanged".green().to_string()
          }
        };

        table.lock().unwrap().update_row(i, |row| row.cols[3] = status);

        if !dry_run {
          std::fs::write(&path, &content).unwrap();
        }
      }));
    }

    handles.into_iter().for_each(|h| h.join().unwrap());
  }

  fn submission_filename(&self, user: &User, attachment: &Attachment) -> String {
    let path =
      self.path.join(format!("{}-{}", snakeify(&user.sortable_name), attachment.display_name));
    path.file_name().unwrap().to_string_lossy().to_string()
  }

  fn attachment_filename(&self, user: &User, s: &Submission) -> Result<String, String> {
    let i = self.find_attachment(s)?;
    Ok(self.submission_filename(user, &s.attachments[i]))
  }

  fn find_attachment(&self, s: &Submission) -> Result<usize, String> {
    let res = if let Some(filename) = &self.settings.filename {
      s.attachments
        .iter()
        .position(|a| filename_matches(&a.display_name, filename))
        .ok_or_else(|| format!("error: couldn't find \"{}\" in attachments", filename))
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
}

// Matches 'foo.c' against 'foo.c', 'foo-1.c', 'foo-2.c', etc.
// Also works without extensions: 'foo' matches 'foo', 'foo-1', etc.
fn filename_matches(display_name: &str, expected: &str) -> bool {
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
