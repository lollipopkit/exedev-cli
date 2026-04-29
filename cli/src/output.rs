use anyhow::{Context, Result, bail};
use comfy_table::{Attribute, Cell, Color, Table, presets::UTF8_FULL};
use exedev_core::terminal;
use serde_json::Value;
use std::collections::BTreeSet;

pub(crate) fn print_response(response: &str, json: bool) -> Result<()> {
    let trimmed = response.trim();
    if trimmed.is_empty() {
        return Ok(());
    }

    let parsed = serde_json::from_str::<Value>(trimmed).ok();
    if json {
        if let Some(value) = parsed {
            println!("{}", serde_json::to_string_pretty(&value)?);
        } else {
            println!("{trimmed}");
        }
        return Ok(());
    }

    if let Some(value) = parsed {
        print_human_json(&value)?;
    } else {
        print_plain_text(trimmed);
    }
    Ok(())
}

fn print_human_json(value: &Value) -> Result<()> {
    if let Some(output) = value.get("output").and_then(Value::as_str) {
        if !output.trim().is_empty() {
            print_plain_text(output.trim_end());
            return Ok(());
        }
    }
    if let Some(error) = value.get("error").and_then(Value::as_str) {
        if !error.trim().is_empty() {
            bail!(error.to_string());
        }
    }

    match value {
        Value::Array(items) if items.iter().all(Value::is_object) => print_table(items),
        Value::Object(_) => {
            println!("{}", terminal::label(serde_json::to_string_pretty(value)?));
            Ok(())
        }
        _ => {
            println!("{}", terminal::label(value));
            Ok(())
        }
    }
}

fn print_table(items: &[Value]) -> Result<()> {
    if items.is_empty() {
        println!("{}", terminal::muted("[]"));
        return Ok(());
    }

    let mut columns = BTreeSet::new();
    for item in items {
        if let Some(object) = item.as_object() {
            for key in object.keys() {
                columns.insert(key.clone());
            }
        }
    }

    let columns = columns.into_iter().collect::<Vec<_>>();
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(columns.iter().map(|column| styled_header_cell(column)));
    for item in items {
        let object = item
            .as_object()
            .context("table output only supports object arrays")?;
        let row = columns.iter().map(|column| {
            let text = object
                .get(column)
                .map(format_json_cell)
                .unwrap_or_else(String::new);
            styled_value_cell(column, text)
        });
        table.add_row(row);
    }
    println!("{table}");
    Ok(())
}

fn format_json_cell(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        _ => serde_json::to_string(value).unwrap_or_else(|_| value.to_string()),
    }
}

fn print_plain_text(text: &str) {
    for line in text.lines() {
        if line.starts_with("error:") || line.starts_with("Error:") {
            eprintln!("{}", terminal::error(line));
        } else if line.starts_with("warning:") || line.starts_with("Warning:") {
            println!("{}", terminal::warn(line));
        } else {
            println!("{line}");
        }
    }
}

fn styled_header_cell(column: &str) -> Cell {
    let cell = Cell::new(column);
    if terminal::stdout_color_enabled() {
        cell.fg(Color::Cyan).add_attribute(Attribute::Bold)
    } else {
        cell
    }
}

fn styled_value_cell(column: &str, text: String) -> Cell {
    let cell = Cell::new(text.clone());
    if !terminal::stdout_color_enabled() {
        return cell;
    }
    if is_name_column(column) {
        return cell.fg(Color::Magenta);
    }
    if is_status_column(column) {
        return match text.as_str() {
            "running" | "ready" | "active" | "ok" | "success" | "present" => cell.fg(Color::Green),
            "stopped" | "missing" | "inactive" | "pending" | "unknown" => cell.fg(Color::Yellow),
            "failed" | "error" | "unavailable" => cell.fg(Color::Red),
            _ => cell,
        };
    }
    cell
}

fn is_name_column(column: &str) -> bool {
    matches!(
        column,
        "name" | "vm_name" | "vmName" | "ssh_dest" | "sshDest"
    )
}

fn is_status_column(column: &str) -> bool {
    matches!(column, "status" | "state" | "ready")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_table_json() {
        print_response(r#"[{"name":"a","status":"running"}]"#, false).unwrap();
    }
}
