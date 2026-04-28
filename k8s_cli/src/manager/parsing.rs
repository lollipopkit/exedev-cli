use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
pub(super) struct KubernetesNode {
    pub(super) ready: bool,
    pub(super) labels: BTreeMap<String, String>,
    pub(super) taints: BTreeSet<String>,
}

pub(super) fn parse_vm_names(response: &str) -> Result<BTreeSet<String>> {
    let trimmed = response.trim();
    if trimmed.is_empty() {
        return Ok(BTreeSet::new());
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        let mut names = BTreeSet::new();
        collect_vm_names_from_json(&value, &mut names);
        if !names.is_empty() {
            return Ok(names);
        }
        if let Some(output) = value.get("output").and_then(Value::as_str) {
            return Ok(parse_vm_names_from_text(output));
        }
    }
    Ok(parse_vm_names_from_text(trimmed))
}

fn collect_vm_names_from_json(value: &Value, names: &mut BTreeSet<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                if let Some(name) = item.as_str() {
                    names.insert(name.to_string());
                } else {
                    collect_vm_names_from_json(item, names);
                }
            }
        }
        Value::Object(object) => {
            for key in ["name", "vm", "vmname", "vmName", "vm_name"] {
                if let Some(name) = object.get(key).and_then(Value::as_str) {
                    names.insert(name.to_string());
                    return;
                }
            }
            for key in ["vms", "items", "data"] {
                if let Some(child) = object.get(key) {
                    collect_vm_names_from_json(child, names);
                }
            }
        }
        _ => {}
    }
}

pub(super) fn parse_vm_names_from_text(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !line.to_ascii_lowercase().starts_with("name"))
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_string)
        .collect()
}

pub(super) fn parse_kubernetes_nodes(response: &str) -> Result<BTreeMap<String, KubernetesNode>> {
    let value = serde_json::from_str::<Value>(response).context("kubectl returned invalid JSON")?;
    let mut nodes = BTreeMap::new();
    let items = value
        .get("items")
        .and_then(Value::as_array)
        .context("kubectl nodes JSON did not contain items")?;
    for item in items {
        let name = item
            .pointer("/metadata/name")
            .and_then(Value::as_str)
            .context("node missing metadata.name")?
            .to_string();
        let labels = item
            .pointer("/metadata/labels")
            .and_then(Value::as_object)
            .map(|object| {
                object
                    .iter()
                    .filter_map(|(key, value)| {
                        value.as_str().map(|value| (key.clone(), value.to_string()))
                    })
                    .collect::<BTreeMap<_, _>>()
            })
            .unwrap_or_default();
        let ready = item
            .pointer("/status/conditions")
            .and_then(Value::as_array)
            .map(|conditions| {
                conditions.iter().any(|condition| {
                    condition.get("type").and_then(Value::as_str) == Some("Ready")
                        && condition.get("status").and_then(Value::as_str) == Some("True")
                })
            })
            .unwrap_or(false);
        let taints = item
            .pointer("/spec/taints")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|taint| {
                        let key = taint.get("key").and_then(Value::as_str)?;
                        let effect = taint.get("effect").and_then(Value::as_str)?;
                        let value = taint.get("value").and_then(Value::as_str).unwrap_or("");
                        Some(format!("{key}={value}:{effect}"))
                    })
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default();
        nodes.insert(
            name,
            KubernetesNode {
                ready,
                labels,
                taints,
            },
        );
    }
    Ok(nodes)
}
