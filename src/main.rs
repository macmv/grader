use anyhow::{Context, bail};
use clap::Parser;
use std::{
  path::{Path, PathBuf},
  process::{Command, Stdio},
  thread,
};

#[derive(Parser)]
struct Args {
  /// The .c files to compile
  files: Vec<PathBuf>,
}

struct CompileResult {
  file:      PathBuf,
  stdout:    String,
  stderr:    String,
  exit_code: Option<i32>,
}

fn main() {
  let args = Args::parse();

  println!("=== compiling {} files ===", args.files.len());
  let handles: Vec<_> = args
    .files
    .into_iter()
    .map(|file| thread::spawn(move || compile(&file).map_err(|e| (file.clone(), e))))
    .collect();

  let mut failed = false;
  for handle in handles {
    match handle.join().unwrap() {
      Ok(result) => print_result(&result),
      Err((file, e)) => {
        println!("error compiling '{}': {e}", file.display());
        failed = true;
      }
    }
  }

  if failed {
    std::process::exit(1);
  }
}

fn print_result(result: &CompileResult) {
  println!("=== {} ===", result.file.file_name().unwrap().display());
  if !result.stdout.trim().is_empty() {
    print!("{}", result.stdout);
  }
  if !result.stderr.trim().is_empty() {
    print!("{}", result.stderr);
  }
  match result.exit_code {
    Some(0) => println!("Compilation successful."),
    Some(code) => println!("Compilation failed (exit code {}).", code),
    None => println!("Could not determine gcc exit code."),
  }
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
  Command::new("ssh")
    .arg("wwu")
    .arg(format!("mkdir -p ~/Desktop/ta/{parent}"))
    .output()
    .context("failed to create remote directory")?;

  let remote_path = format!("~/Desktop/ta/{}", path);
  let remote_build = format!("~/Desktop/ta/{}", path.strip_suffix(".c").unwrap());
  let gcc_flags = "-Wall -Wextra -pedantic";

  let status = Command::new("scp")
    .arg(file_str)
    .arg(&format!("wwu:{remote_path}"))
    .status()
    .context("failed to run scp")?;

  if !status.success() {
    bail!("scp failed with {}", status);
  }

  let output = Command::new("ssh")
    .arg("wwu")
    .arg(format!("gcc {remote_path} -o {remote_build} {gcc_flags} 2>&1; echo \"exit:$?\""))
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

  Ok(CompileResult { file: file.to_path_buf(), stdout, stderr, exit_code })
}
