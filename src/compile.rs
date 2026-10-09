use std::{
  fmt,
  path::{Path, PathBuf},
  process::{Command, Stdio},
  sync::{Arc, Mutex},
};

use anyhow::{Context, bail};
use owo_colors::OwoColorize;

use crate::{ui::Table, workspace::Assignment};

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

/// The file's path relative to the assignment directory, so students can be
/// told apart.
fn label(assignment: &Assignment, file: &Path) -> String {
  file.strip_prefix(&assignment.path).unwrap_or(file).display().to_string()
}

pub fn compile_files(assignment: &Assignment, files: &[PathBuf]) {
  let mut table = Table::new(&["File", "Status"]);
  for f in files {
    table.add_row(&[&label(assignment, f), "..."]);
  }
  table.display();

  let table = Arc::new(Mutex::new(table));

  let results = std::thread::scope(|scope| {
    let mut handles = vec![];

    for (i, file) in files.iter().enumerate() {
      let file = file.clone();
      let table = table.clone();
      handles.push(scope.spawn(move || {
        let result = assignment.compile(&file);
        let status = match &result {
          Ok(r) => r.status_pretty().to_string(),
          Err(_) => "internal error".red().to_string(),
        };
        table.lock().unwrap().update_row(i, |row| row.cols[1] = status);
        result
      }));
    }

    handles.into_iter().map(|h| h.join().unwrap()).collect::<Vec<_>>()
  });

  for (file, result) in files.iter().zip(results) {
    match result {
      Err(e) => {
        eprintln!("{} compiling '{}': {e}", "error".red().bold(), file.display());
      }
      Ok(r) => print_result(&label(assignment, file), &r),
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

const GCC_FLAGS: &str = "-Wall -Wextra -pedantic -fdiagnostics-color=always";

fn shell_escape(s: &str) -> String { format!("'{}'", s.replace('\'', "'\\''")) }

impl Assignment<'_> {
  /// Picks the file to compile in a student's directory, the same way the
  /// downloader picks an attachment: the one matching the assignment's
  /// `filename`, or the only file there.
  pub fn find_submission_file(&self, dir: &Path) -> Result<PathBuf, String> {
    let mut files: Vec<(String, PathBuf)> = vec![];
    for entry in dir.read_dir().map_err(|e| e.to_string())? {
      let path = entry.map_err(|e| e.to_string())?.path();
      if path.is_file() {
        files.push((path.file_name().unwrap().to_string_lossy().into_owned(), path));
      }
    }
    files.sort();
    let names = || files.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>();

    if let Some(expected) = &self.settings.filename {
      match files.iter().position(|(n, _)| crate::download::filename_matches(n, expected)) {
        Some(i) => Ok(files.swap_remove(i).1),
        None if files.len() == 1 => Ok(files.remove(0).1),
        None => Err(format!("couldn't find \"{expected}\" in {:?}", names())),
      }
    } else if files.len() == 1 {
      Ok(files.remove(0).1)
    } else {
      Err(format!("multiple files: {:?}", names()))
    }
  }

  fn compile_command(&self, remote_path: &str) -> String {
    let quoted_path = shell_escape(remote_path);
    let mut cmd =
      self.settings.compile.replace("%REMOTE_PATH", &quoted_path).replace("%GCC_FLAGS", GCC_FLAGS);

    if cmd.contains("%REMOTE_OUT") {
      match remote_path.strip_suffix(".c") {
        Some(out_path) => cmd = cmd.replace("%REMOTE_OUT", &shell_escape(out_path)),
        None => cmd = cmd.replace("%REMOTE_OUT", ""),
      }
    }

    cmd
  }

  fn compile_local(&self, file: &Path) -> anyhow::Result<CompileResult> {
    let file = file.canonicalize()?;
    let file_str = file.to_str().context("file path is not valid utf-8")?;

    let cmd = self.compile_command(file_str);
    if cmd.is_empty() {
      return Ok(CompileResult {
        file,
        stdout: String::new(),
        stderr: String::new(),
        exit_code: 0,
      });
    }

    let output = Command::new("sh")
      .args(["-c", &format!("{cmd} 2>&1")])
      .current_dir(file.parent().context("file has no parent directory")?)
      .output()
      .context("failed to run compile command")?;

    Ok(CompileResult {
      file,
      stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
      stderr: String::new(),
      exit_code: output.status.code().unwrap_or(-1),
    })
  }

  fn compile(&self, file: &Path) -> anyhow::Result<CompileResult> {
    if self.course.settings.local_compile {
      return self.compile_local(file);
    }

    let file = file.canonicalize()?;

    let file_str = file.to_str().context("file path is not valid utf-8")?;
    let path = file_str
      .strip_prefix(&format!("{}/", self.course.workspace.root.to_str().unwrap()))
      .context("file is not in the 'ta' directory")?;

    // The remote layout mirrors the local one: `<assignment>/<student>/<file>`
    // with `download.separate_directories`, and `<assignment>/<file>`
    // otherwise.
    let (depth, format) = if self.settings.download.separate_directories {
      (3, "ta/<class>/<assignment>/<student>/<file>")
    } else {
      (2, "ta/<class>/<assignment>/<file>")
    };
    if path.chars().filter(|c| *c == '/').count() != depth {
      bail!("invalid path: '{path}'\nshould have the format {format}");
    }

    let parent = &path[..path.rfind('/').unwrap()];

    ssh(&format!("mkdir -p $HOME/Desktop/ta/{}", shell_escape(parent)))
      .context("failed to create remote directory")?;
    let remote_path = format!("/home/macnean2/Desktop/ta/{}", path);

    let out = Command::new("scp")
      .arg(file_str)
      .arg(&format!("wwu:{remote_path}"))
      .output()
      .context("failed to run scp")?;

    if !out.status.success() {
      let stderr = String::from_utf8_lossy(&out.stderr);
      bail!("scp failed:\n{stderr}");
    }

    let cmd = self.compile_command(&remote_path);
    // Always run from the directory containing the file.
    let remote_dir = &remote_path[..remote_path.rfind('/').unwrap()];
    let cmd =
      if cmd.is_empty() { cmd } else { format!("cd {} && {cmd}", shell_escape(remote_dir)) };
    if cmd.is_empty() {
      Ok(CompileResult {
        file:      file.to_path_buf(),
        stdout:    String::new(),
        stderr:    String::new(),
        exit_code: 0,
      })
    } else {
      let result = ssh(&cmd).context("gcc failed")?;

      Ok(CompileResult {
        file:      file.to_path_buf(),
        stdout:    result.stdout,
        stderr:    result.stderr,
        exit_code: result.exit_code,
      })
    }
  }
}

fn print_result(name: &str, result: &CompileResult) {
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
