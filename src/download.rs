use owo_colors::OwoColorize;

use crate::workspace::{Course, UserId, Users};

#[derive(serde::Deserialize)]
pub struct Submission {
  pub user_id:     UserId,
  pub score:       Option<f32>,
  #[serde(default)]
  pub attachments: Vec<Attachment>,
}

#[derive(serde::Deserialize)]
pub struct Attachment {
  pub display_name: String,
  pub url:          String,
}

#[derive(serde::Deserialize, Debug)]
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

impl Course {
  pub fn fetch_users(&self) -> Users {
    let token = std::fs::read_to_string("../token.txt").unwrap().trim().to_string();

    let users: Vec<User> = ureq::get(format!(
      "https://wwu.instructure.com/api/v1/sections/{section}/users?per_page=100",
      section = self.settings.section
    ))
    .header("Authorization", &format!("Bearer {token}"))
    .header("Accept", "application/json")
    .call()
    .unwrap()
    .body_mut()
    .read_json()
    .unwrap();

    Users::from_vec(users)
  }

  pub fn download_submissions(&self, assignment: &str, dry_run: bool) {
    let assignment_id = self
      .settings
      .assignment
      .get(assignment)
      .unwrap_or_else(|| {
        eprintln!("error: assignment '{assignment}' not found in settings.toml");
        std::process::exit(1);
      })
      .id;

    let token = std::fs::read_to_string("../token.txt").unwrap().trim().to_string();

    let mut submissions: Vec<Submission> = ureq::get(format!(
      "https://wwu.instructure.com/api/v1/sections/{section}/assignments/{assignment_id}/submissions?per_page=100",
      section = self.settings.section,
    ))
    .header("Authorization", &format!("Bearer {token}"))
    .header("Accept", "application/json")
    .call()
    .unwrap()
    .body_mut()
    .read_json()
    .unwrap();

    println!("{} submission(s)", submissions.len());

    let users = self.users();

    submissions.sort_by_key(|s| &users.get(&s.user_id).unwrap().sortable_name);

    let directory = self.path.join(assignment);
    std::fs::create_dir_all(&directory).unwrap();

    for s in submissions {
      let user = &users[&s.user_id];

      if s.attachments.is_empty() {
        print!("{:<20}: <not submitted>", user.name)
      } else {
        let attachment = &s.attachments[0];
        print!(
          "{:<20}: {:<20}: {}",
          user.name,
          attachment.display_name,
          match s.score {
            Some(s) => format!("{s}"),
            None => "<not graded>".to_string(),
          }
        );

        let content = ureq::get(&attachment.url)
          .header("Authorization", &format!("Bearer {token}"))
          .call()
          .unwrap()
          .body_mut()
          .read_to_vec()
          .unwrap();

        let path =
          directory.join(format!("{}-{}", snakeify(&user.sortable_name), attachment.display_name));

        if !path.exists() {
          print!(": {:<40}: {}", path.file_name().unwrap().display(), "new".yellow());
        } else {
          let existing = std::fs::read(&path).unwrap();
          if existing != content {
            print!(": {:<40}: {}", path.file_name().unwrap().display(), "changed".yellow());
          } else {
            print!(": {:<40}: {}", path.file_name().unwrap().display(), "unchanged".green());
          }
        }

        if !dry_run {
          std::fs::write(&path, &content).unwrap();
        }
      }

      println!();
    }
  }
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
