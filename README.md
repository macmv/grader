# grader

A CLI tool for WWU TAs to download and compile student submissions from Canvas.

## Setup

The workspace root is any directory containing a `ta.toml` file (contents don't matter -- it's just a marker). Place a `token.txt` file next to `ta.toml`, and include a Canvas API token. Ask the helpdesk about creating a token.

Courses live as subdirectories of the workspace root, named like `CSCI-101`. Each course needs a `settings.toml`:

```toml
course  = 12345   # Canvas course ID
section = 67890   # Canvas section ID

# run `compile` on this machine instead of over ssh (default: false)
local-compile = true

[assignment.hw1]
id      = 11111   # Canvas assignment ID
compile = "gcc %GCC_FLAGS %REMOTE_PATH -o %REMOTE_BUILD"

[assignment.hw2]
id       = 22222
compile  = "gcc %GCC_FLAGS %REMOTE_PATH -o %REMOTE_BUILD"

# expected filename - only matters if multiple files are uploaded
filename = "main.c"

# place each student's file in its own subdirectory
separate-directories = true

[assignment.hw2.download]
# download into <student>/<file> instead of <student>-<file>
separate-directories = true
```

Compile string placeholders:
- `%REMOTE_PATH` -- absolute remote path to the uploaded file
- `%REMOTE_BUILD` -- same as `%REMOTE_PATH` with `.c` stripped (output binary path)
- `%GCC_FLAGS` -- expands to `-Wall -Wextra -pedantic -fdiagnostics-color=always`

If `local-compile = true`, the command instead runs locally (in the file's directory), `%REMOTE_PATH`/`%REMOTE_BUILD` expand to local paths, and no ssh/scp is needed.

Otherwise, the `compile` command runs over SSH to a host named `wwu` and copies files with `scp`, so `~/.ssh/config` must have a `wwu` entry pointing at the school server.

## Commands

All commands infer the course and assignment from the current directory. Use `--course` and `--assignment` to override.

```
grader download [--dry-run]
```
Downloads all submissions for the current assignment into `<course>/<assignment>/`. Files are named `<student>-<filename>`. Normally one file is picked per student; with `separate-directories`, every attachment is downloaded.

```
grader compile [files...]
```
Uploads each file to the remote server and compiles it. With no arguments, compiles every file in the assignment directory. Shows a live status table, then prints compiler output for each file. Additionally, this expects a `wwu` alias to be setup in `~/.ssh/config`, that should look like so:
```
Host wwu
  HostName linux.cs.wwu.edu
  User <username>
  IdentityFile ~/.ssh/id_ed25519
  Port 922
```

When `separate-directories` is set, each student's file is placed under `<assignment>/<student>/` on the remote (student name is derived by splitting the local filename at the second `-`).

## Example layout

```
ta.toml
token.txt
CSCI-101/
  settings.toml
  hw1/
    doe_john-main.c
    smith-jones_jane-main.c
  hw2/
    ...
```
