use clap::Parser;
use std::path::PathBuf;

use crate::workspace::Workspace;

mod compile;
mod download;
mod settings;
mod ui;
mod workspace;

#[derive(Parser)]
struct Args {
  #[clap(long, global = true)]
  course:     Option<String>,
  #[clap(long, global = true)]
  assignment: Option<String>,
  #[clap(long, global = true)]
  dry_run:    bool,

  #[clap(subcommand)]
  cmd: Cmd,
}

#[derive(clap::Subcommand)]
enum Cmd {
  Sections {
    course: String,
  },
  Download {},
  Compile {
    /// The .c files to compile
    files: Vec<PathBuf>,
  },
}

fn main() {
  let args = Args::parse();

  if let Cmd::Sections { course } = &args.cmd {
    download::list_sections(&course);
    return;
  }

  let workspace = Workspace::new();
  let course = args
    .course
    .as_deref()
    .map(|name| workspace.course(name))
    .unwrap_or_else(|| workspace.current_course())
    .unwrap_or_else(|e| {
      eprintln!("error: {e}");
      std::process::exit(1);
    });

  let assignment = args
    .assignment
    .as_deref()
    .map(|name| course.assignment(name))
    .unwrap_or_else(|| course.current_assignment())
    .unwrap_or_else(|e| {
      eprintln!("error: {e}");
      std::process::exit(1);
    });

  match args.cmd {
    Cmd::Download {} => {
      assignment.download_submissions(args.dry_run);
    }
    Cmd::Compile { files } => {
      let files = if files.is_empty() {
        assignment.path.read_dir().unwrap().map(|e| e.unwrap().path()).collect()
      } else {
        files
      };

      compile::compile_files(&files)
    }

    Cmd::Sections { .. } => unreachable!(),
  }
}
