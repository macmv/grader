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

  pub fn download_submissions(&self, assignment: &str) {
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

    for s in submissions {
      let user = &users[&s.user_id];

      if s.attachments.is_empty() {
        println!("{:<20}: <not submitted>", user.name)
      } else {
        let attachment = &s.attachments[0];
        println!(
          "{:<20}: {:<20}: {}",
          user.name,
          attachment.display_name,
          match s.score {
            Some(s) => format!("{s}"),
            None => "<not graded>".to_string(),
          }
        );
      }
    }
  }
}
