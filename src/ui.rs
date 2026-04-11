use std::io::Write;

use crossterm::{cursor, execute, terminal};
use owo_colors::OwoColorize;

pub struct Table {
  headers: Vec<String>,
  rows:    Vec<Row>,
  widths:  Vec<usize>,
}

pub struct Row {
  pub cols: Vec<String>,
}

impl Table {
  pub fn new(headers: &[&str]) -> Self {
    let widths = headers.iter().map(|h| h.len()).collect();
    Table { headers: headers.iter().map(|h| h.to_string()).collect(), rows: vec![], widths }
  }

  pub fn add_row(&mut self, row: &[&str]) -> usize {
    assert_eq!(
      row.len(),
      self.widths.len(),
      "row has {} columns, expected {}",
      row.len(),
      self.widths.len()
    );
    for (i, col) in row.iter().enumerate() {
      self.widths[i] = self.widths[i].max(col.len());
    }
    let index = self.rows.len();
    self.rows.push(Row { cols: row.iter().map(|c| c.to_string()).collect() });
    index
  }

  pub fn display(&self) {
    let mut stdout = std::io::stdout();
    self.print_headers(&mut stdout);
    writeln!(stdout).unwrap();
    for row in &self.rows {
      self.print_row(&mut stdout, row);
      writeln!(stdout).unwrap();
    }
    stdout.flush().unwrap();
  }

  pub fn update_row(&mut self, index: usize, f: impl FnOnce(&mut Row)) {
    f(&mut self.rows[index]);
    let row = &self.rows[index];
    assert_eq!(row.cols.len(), self.widths.len());

    let mut stdout = std::io::stdout();
    let rows_below = (self.rows.len() - index - 1) as u16;
    execute!(
      stdout,
      cursor::MoveUp(rows_below + 1),
      terminal::Clear(terminal::ClearType::CurrentLine)
    )
    .unwrap();
    self.print_row(&mut stdout, &row);
    execute!(stdout, cursor::MoveDown(rows_below + 1), cursor::MoveToColumn(0)).unwrap();
    stdout.flush().unwrap();
  }

  fn print_headers(&self, stdout: &mut std::io::Stdout) {
    for (i, header) in self.headers.iter().enumerate() {
      let width = self.widths[i];
      if i > 0 {
        write!(stdout, "  ").unwrap();
      }
      write!(stdout, "{:<width$}", header.bright_black()).unwrap();
    }
  }

  fn print_row(&self, stdout: &mut std::io::Stdout, row: &Row) {
    for (i, col) in row.cols.iter().enumerate() {
      let width = self.widths.get(i).copied().unwrap_or(0);
      if i > 0 {
        write!(stdout, "  ").unwrap();
      }
      write!(stdout, "{:<width$}", col).unwrap();
    }
  }
}
