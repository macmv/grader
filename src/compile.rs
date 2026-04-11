use std::{
  fmt,
  path::{Path, PathBuf},
  process::{Command, Stdio},
  sync::{Arc, Mutex},
};

use anyhow::{Context, bail};
use owo_colors::OwoColorize;

use crate::ui::Table;

struct CompileResult {
  file:      PathBuf,
  stdout:    String,
  stderr:    String,
  exit_code: i32,
}

struct RemoteOutput {
  stdout:    String,
  stderr:    String,
  exit_code: i32,
}

enum Status {
  Success,
  Warning,
  Fail,
}
struct StatusPretty(Status);

pub fn compile_files(files: &[PathBuf]) {
  let mut table = Table::new(&["File", "Status"]);
  for f in files {
    table.add_row(&[f.file_name().unwrap().to_str().unwrap(), "..."]);
  }
  table.display();

  let table = Arc::new(Mutex::new(table));
  let mut handles = vec![];

  for (i, file) in files.iter().enumerate() {
    let file = file.clone();
    let table = table.clone();
    handles.push(std::thread::spawn(move || {
      let result = compile(&file);
      let status = match &result {
        Ok(r) => r.status_pretty().to_string(),
        Err(_) => "internal error".red().to_string(),
      };
      table.lock().unwrap().update_row(i, |row| row.cols[1] = status);
      result
    }));
  }

  let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

  for (file, result) in files.iter().zip(results) {
    match result {
      Err(e) => {
        eprintln!("{} compiling '{}': {e}", "error".red().bold(), file.display());
      }
      Ok(r) => print_result(&r),
    }
  }
}

impl CompileResult {
  fn status(&self) -> Status {
    if self.exit_code == 0 {
      if self.stdout.trim().is_empty() { Status::Success } else { Status::Warning }
    } else {
      Status::Fail
    }
  }

  fn status_pretty(&self) -> StatusPretty { StatusPretty(self.status()) }
}

impl fmt::Display for Status {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Status::Success => write!(f, "success"),
      Status::Warning => write!(f, "warning"),
      Status::Fail => write!(f, "fail"),
    }
  }
}

impl fmt::Display for StatusPretty {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self.0 {
      Status::Success => write!(f, "{}", self.0.green().bold()),
      Status::Warning => write!(f, "{}", self.0.yellow().bold()),
      Status::Fail => write!(f, "{}", self.0.red().bold()),
    }
  }
}

fn ssh(cmd: &str) -> anyhow::Result<RemoteOutput> {
  let output = Command::new("ssh")
    .args(["-T", "wwu", &format!("{cmd} 2>&1; echo \"exit:$?\"")])
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .output()
    .context("failed to run ssh")?;

  let raw_stdout = String::from_utf8_lossy(&output.stdout);
  let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

  let mut exit_code = None;
  let mut stdout = String::new();
  for line in raw_stdout.lines() {
    if let Some(code) = line.strip_prefix("exit:") {
      exit_code = code.parse::<i32>().ok();
    } else {
      stdout.push_str(line);
      stdout.push('\n');
    }
  }

  let exit_code = exit_code.context("could not determine remote exit code")?;
  Ok(RemoteOutput { stdout, stderr, exit_code })
}

fn compile(file: &Path) -> anyhow::Result<CompileResult> {
  let file = file.canonicalize()?;

  let file_str = file.to_str().context("file path is not valid utf-8")?;
  let path = file_str
    .strip_prefix("/home/macmv/Desktop/school/wwu/ta/")
    .context("file is not in the 'ta' directory")?;

  if !(path.chars().filter(|c| *c == '/').count() == 2 && path.ends_with(".c")) {
    bail!("invalid path: '{path}'\nshould have the format ta/<class>/<assignment>/<file>.c");
  }

  let parent = &path[..path.rfind('/').unwrap()];
  ssh(&format!("mkdir -p ~/Desktop/ta/{parent}")).context("failed to create remote directory")?;

  let remote_path = format!("~/Desktop/ta/{}", path);
  let remote_build = format!("~/Desktop/ta/{}", path.strip_suffix(".c").unwrap());
  let gcc_flags = "-Wall -Wextra -pedantic";

  let status = Command::new("scp")
    .arg(file_str)
    .arg(&format!("wwu:{remote_path}"))
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .status()
    .context("failed to run scp")?;

  if !status.success() {
    bail!("scp failed with {}", status);
  }

  let result =
    ssh(&format!("gcc {remote_path} -o {remote_build} {gcc_flags}")).context("gcc failed")?;

  Ok(CompileResult {
    file:      file.to_path_buf(),
    stdout:    result.stdout,
    stderr:    result.stderr,
    exit_code: result.exit_code,
  })
}

fn print_result(result: &CompileResult) {
  let name = result.file.file_name().unwrap().to_string_lossy();
  println!("{}", format_args!("== {name} ==").cyan().bold());

  if !result.stdout.trim().is_empty() {
    print!("{}", result.stdout);
  }
  if !result.stderr.trim().is_empty() {
    print!("{}", result.stderr);
  }

  print!("{}", result.status_pretty());
  if result.exit_code != 0 {
    println!(": exit code {}", result.exit_code.red().bold());
  }

  println!();
}
