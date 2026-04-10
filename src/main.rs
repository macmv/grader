use anyhow::{Context, bail};
use clap::Parser;
use owo_colors::OwoColorize;
use std::{
  path::{Path, PathBuf},
  process::{Command, Stdio},
  thread,
};

mod download;

#[derive(Parser)]
struct Args {
  #[clap(subcommand)]
  cmd: Cmd,
}

#[derive(clap::Subcommand)]
enum Cmd {
  List {
    course: String,
  },
  Download {
    section:    String,
    assignment: String,
  },
  Compile {
    /// The .c files to compile
    files: Vec<PathBuf>,
  },
}

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

fn main() {
  let args = Args::parse();

  match args.cmd {
    Cmd::List { course } => download::list_sections(&course),
    Cmd::Download { section, assignment } => download::download_submissions(&section, &assignment),
    Cmd::Compile { files } => compile_files(&files),
  }
}

fn compile_files(files: &[PathBuf]) {
  println!("{}", format_args!("compiling {} file(s)...", files.len()).dimmed());
  let handles: Vec<_> = files
    .into_iter()
    .map(|file| {
      let file = file.clone();
      thread::spawn(move || compile(&file).map_err(|e| (file.clone(), e)))
    })
    .collect();

  let mut failed = false;
  for handle in handles {
    match handle.join().unwrap() {
      Ok(result) => print_result(&result),
      Err((file, e)) => {
        println!("{} compiling '{}': {e}", "error".red().bold(), file.display());
        failed = true;
      }
    }
  }

  if failed {
    std::process::exit(1);
  }
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

  if result.exit_code == 0 {
    println!("{}", "compilation successful".green().bold());
  } else {
    println!(
      "{}",
      format_args!("compilation failed (exit code {})", result.exit_code).red().bold()
    );
  }

  println!();
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
