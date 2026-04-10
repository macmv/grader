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

pub fn download_submissions(section: &str, assignment: &str) {
  let token = std::fs::read_to_string("../token.txt").unwrap().trim().to_string();

  let res = ureq::get(format!(
    "https://wwu.instructure.com/api/v1/sections/{section}/assignments/{assignment}/submissions"
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
