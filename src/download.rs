use crate::workspace::Course;

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

    let res = ureq::get(format!(
      "https://wwu.instructure.com/api/v1/sections/{section}/assignments/{assignment_id}/submissions",
      section = self.settings.section,
    ))
    .header("Authorization", &format!("Bearer {token}"))
    .header("Accept", "application/json")
    .call()
    .unwrap()
    .body_mut()
    .read_to_string()
    .unwrap();

    println!("{res}");
  }
}
