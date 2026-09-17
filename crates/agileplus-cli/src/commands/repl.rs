//! Interactive REPL for AgilePlus projects.
//!
//! Provides a `agileplus repl` command that opens a `rusqlite::Connection`
//! to the configured project database and exposes a small read-only SQL
//! DSL over stdin/stdout. Useful for debugging, live experimentation, and
//! shell scripting. Mutations are intentionally rejected — funnel writes
//! through the regular CLI subcommands to keep audit trails consistent.
//!
//! Dot-commands:
//!   .tables              show discovered tables
//!   .schema <table>      show CREATE TABLE for a table
//!   .count <table>       row count for a table
//!   .help                list REPL commands
//!   .exit / Ctrl-D       quit
//!
//! Anything else is treated as a SQL statement; only SELECT / WITH / PRAGMA
//! / EXPLAIN are permitted (statements terminated by `;`).
//!
//! Traces to: FR-X01 (debugging), pillar L63 (Debug Tools)

use anyhow::{bail, Context, Result};
use colored::Colorize;
use rusqlite::Connection;
use std::io::{BufRead, Write};
use std::path::PathBuf;

/// Entry point invoked from `main.rs` for the `repl` subcommand.
pub fn run(db_path: PathBuf) -> Result<()> {
    let conn = open_conn(&db_path)?;
    println!(
        "{}",
        format!(
            "agileplus REPL — connected to {} (type .help, .exit to quit)",
            db_path.display()
        )
        .bright_cyan()
    );

    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let mut stdout = std::io::stdout();
    let mut buf = String::new();
    let mut accum = String::new();

    loop {
        let prompt = if accum.is_empty() {
            "agileplus> ".bright_green().to_string()
        } else {
            "       ...> ".bright_green().to_string()
        };
        write!(stdout, "{prompt}")?;
        stdout.flush()?;

        buf.clear();
        let n = input.read_line(&mut buf)?;
        if n == 0 {
            // EOF
            println!();
            break;
        }

        let line = buf.trim_end_matches(['\n', '\r']);
        if accum.is_empty() && line.is_empty() {
            continue;
        }
        accum.push_str(line);
        accum.push('\n');

        if accum.trim_start().starts_with('.') && !accum.contains('\n') {
            handle_dot_command(accum.trim(), &conn)?;
            accum.clear();
            continue;
        }

        if !statement_complete(&accum) {
            continue;
        }

        match dispatch(&accum, &conn) {
            Ok(()) => {}
            Err(e) => eprintln!("{} {e}", "error:".bright_red()),
        }
        accum.clear();
    }

    Ok(())
}

fn open_conn(db_path: &std::path::Path) -> Result<Connection> {
    let conn = Connection::open(db_path)
        .with_context(|| format!("failed to open database at {}", db_path.display()))?;
    conn.execute_batch(
        "PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000;",
    )?;
    Ok(conn)
}

/// Returns true if `s` ends a SQL statement (semicolon not inside quotes).
fn statement_complete(s: &str) -> bool {
    let mut in_single = false;
    let mut in_double = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut prev = '\0';
    for c in s.chars() {
        if in_line_comment {
            if c == '\n' {
                in_line_comment = false;
            }
            prev = c;
            continue;
        }
        if in_block_comment {
            if prev == '*' && c == '/' {
                in_block_comment = false;
            }
            prev = c;
            continue;
        }
        match c {
            '\'' if !in_double => {
                in_single = !in_single;
            }
            '"' if !in_single => {
                in_double = !in_double;
            }
            '-' if !in_single && !in_double && prev == '-' => {
                in_line_comment = true;
            }
            '/' if !in_single && !in_double && prev == '*' => {
                in_block_comment = true;
            }
            ';' if !in_single && !in_double => return true,
            _ => {}
        }
        prev = c;
    }
    false
}

fn handle_dot_command(line: &str, conn: &Connection) -> Result<()> {
    let mut parts = line.splitn(2, char::is_whitespace);
    let cmd = parts.next().unwrap_or("");
    let arg = parts.next().unwrap_or("").trim();
    match cmd {
        ".exit" | ".quit" => {
            println!("{}", "bye".bright_yellow());
            std::process::exit(0);
        }
        ".help" | ".h" | ".?" => print_help(),
        ".tables" => list_tables(conn)?,
        ".schema" => {
            if arg.is_empty() {
                bail!("usage: .schema <table>");
            }
            show_schema(conn, arg)?;
        }
        ".count" => {
            if arg.is_empty() {
                bail!("usage: .count <table>");
            }
            count_rows(conn, arg)?;
        }
        other => bail!("unknown command: {other} (try .help)"),
    }
    Ok(())
}

fn print_help() {
    println!(
        "{}",
        "agileplus REPL — read-only SQL explorer over the project DB".bright_cyan()
    );
    println!();
    println!("  .tables              list tables");
    println!("  .schema <table>      print CREATE TABLE for <table>");
    println!("  .count <table>       row count for <table>");
    println!("  .help                this message");
    println!("  .exit                quit");
    println!();
    println!("Anything else is run as SQL. Read-only — only SELECT / WITH /");
    println!("PRAGMA / EXPLAIN are permitted. Use the regular CLI subcommands to mutate.");
}

fn list_tables(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    for r in rows {
        println!("{}", r?);
    }
    Ok(())
}

fn show_schema(conn: &Connection, table: &str) -> Result<()> {
    if !is_safe_identifier(table) {
        bail!("invalid table name: {table}");
    }
    let sql = format!("SELECT sql FROM sqlite_schema WHERE type='table' AND name = '{table}'");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query([])?;
    if let Some(r) = rows.next()? {
        let sql: String = r.get(0)?;
        println!("{sql}");
    } else {
        bail!("no such table: {table}");
    }
    Ok(())
}

fn count_rows(conn: &Connection, table: &str) -> Result<()> {
    if !is_safe_identifier(table) {
        bail!("invalid table name: {table}");
    }
    let sql = format!("SELECT COUNT(*) FROM \"{table}\"");
    let n: i64 = conn.query_row(&sql, [], |r| r.get(0))?;
    println!("{n}");
    Ok(())
}

/// Validates that an identifier matches `[A-Za-z_][A-Za-z0-9_]*`.
fn is_safe_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn dispatch(sql: &str, conn: &Connection) -> Result<()> {
    let trimmed = sql.trim().trim_end_matches(';').trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    if !is_read_only(trimmed) {
        bail!(
            "mutating statements are not permitted in REPL — use `agileplus <subcommand>` instead"
        );
    }
    execute_query(trimmed, conn)
}

fn is_read_only(sql: &str) -> bool {
    let stripped = strip_leading_comments(sql);
    let first = stripped
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    matches!(first.as_str(), "SELECT" | "WITH" | "PRAGMA" | "EXPLAIN")
}

fn strip_leading_comments(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len());
    let mut s = sql;
    loop {
        let t = s.trim_start();
        if let Some(rest) = t.strip_prefix("--") {
            if let Some(nl) = rest.find('\n') {
                s = &rest[nl + 1..];
                continue;
            } else {
                break;
            }
        }
        if let Some(rest) = t.strip_prefix("/*") {
            if let Some(end) = rest.find("*/") {
                s = &rest[end + 2..];
                continue;
            } else {
                out.push_str(t);
                break;
            }
        }
        out.push_str(t);
        break;
    }
    out
}

fn execute_query(sql: &str, conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare(sql)?;
    let column_count = stmt.column_count();
    let columns: Vec<String> = (0..column_count)
        .map(|i| stmt.column_name(i).unwrap_or("?").to_string())
        .collect();

    let mut rows = stmt.query([])?;
    let mut count = 0usize;
    while let Some(row) = rows.next()? {
        if count == 0 && !columns.is_empty() {
            println!("{}", columns.join(" | ").bright_white().bold());
        }
        let mut parts = Vec::with_capacity(column_count);
        for i in 0..column_count {
            let v: rusqlite::types::Value = row.get(i)?;
            parts.push(format_value(&v));
        }
        println!("{}", parts.join(" | "));
        count += 1;
        if count >= 1000 {
            println!("{}", "... (truncated at 1000 rows)".bright_yellow());
            break;
        }
    }
    if count == 0 {
        println!("{}", "(no rows)".bright_black());
    } else {
        println!(
            "{}",
            format!("({count} row{})", if count == 1 { "" } else { "s" }).bright_black()
        );
    }
    Ok(())
}

fn format_value(v: &rusqlite::types::Value) -> String {
    use rusqlite::types::Value;
    match v {
        Value::Null => "NULL".bright_black().to_string(),
        Value::Integer(i) => i.to_string(),
        Value::Real(f) => format!("{f}"),
        Value::Text(t) => t.clone(),
        Value::Blob(b) => format!("<blob {} bytes>", b.len()),
    }
}