use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
pub(super) struct KubernetesNode {
    pub(super) ready: bool,
    pub(super) labels: BTreeMap<String, String>,
    pub(super) taints: BTreeSet<String>,
}

/// JSON keys that can hold a VM name, most specific first.
const VM_NAME_KEYS: [&str; 5] = ["name", "vm", "vmname", "vmName", "vm_name"];

pub(super) fn parse_vm_names(response: &str) -> Result<BTreeSet<String>> {
    let trimmed = response.trim();
    if trimmed.is_empty() {
        return Ok(BTreeSet::new());
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        let mut names = BTreeSet::new();
        collect_vm_names_from_json(&value, &mut names);
        if let Some(output) = value.get("output").and_then(Value::as_str) {
            // The wrapper carries either the serialized listing or a rendered
            // table, and is read whether or not the outer object also listed
            // something: skipping it when the outer had any entry would drop the
            // rest, and bootstrap would recreate VMs that already exist.
            if let Ok(inner) = serde_json::from_str::<Value>(output.trim()) {
                collect_vm_names_from_json(&inner, &mut names);
            } else if names.is_empty() {
                return Ok(parse_vm_names_from_text(output));
            }
        }
        // A response that parsed as JSON has already been searched. Handing its
        // serialized form to the text parser would take `{"vms":[]}` apart into
        // a VM named after the JSON itself; an empty list is simply empty.
        return Ok(names);
    }
    Ok(parse_vm_names_from_text(trimmed))
}

fn collect_vm_names_from_json(value: &Value, names: &mut BTreeSet<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                if let Some(name) = item.as_str() {
                    // A bare string carries no field saying it is a VM, so an
                    // array like ["error", "quota exceeded"] would otherwise
                    // become inventory and suppress creating the real VM.
                    if is_vm_name(name) {
                        names.insert(name.to_string());
                    }
                } else {
                    collect_vm_names_from_json(item, names);
                }
            }
        }
        Value::Object(object) => {
            for key in VM_NAME_KEYS {
                if let Some(name) = object.get(key).and_then(Value::as_str) {
                    names.insert(name.to_string());
                    break;
                }
            }
            // Naming a VM does not rule out carrying more of them: returning here
            // dropped every entry nested under an object that had both.
            for key in ["vms", "items", "data"] {
                if let Some(child) = object.get(key) {
                    collect_vm_names_from_json(child, names);
                }
            }
        }
        _ => {}
    }
}

/// Map VM name to the SSH destination reported by `exe.dev ls`.
///
/// exe.dev hostnames usually route SSH directly, but `ssh_dest` may carry a
/// username prefix (for example `vm+bloggy@exe.dev`) when they do not. VMs whose
/// destination cannot be read are left out, and the caller falls back to the
/// `<vm>.exe.xyz` hostname.
pub(super) fn parse_ssh_destinations(response: &str) -> BTreeMap<String, String> {
    let mut destinations = BTreeMap::new();
    let Ok(value) = serde_json::from_str::<Value>(response.trim()) else {
        return destinations;
    };
    collect_ssh_destinations(&value, &mut destinations);
    // Same wrapper `parse_vm_names` reads, and read on the same terms: when it
    // holds a rendered table there is nothing to find and the caller keeps the
    // hostname fallback; when it holds the serialized listing, its destinations
    // are authoritative, including for VMs the outer object did not mention.
    if let Some(output) = value.get("output").and_then(Value::as_str)
        && let Ok(inner) = serde_json::from_str::<Value>(output.trim())
    {
        collect_ssh_destinations(&inner, &mut destinations);
    }
    destinations
}

fn collect_ssh_destinations(value: &Value, destinations: &mut BTreeMap<String, String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_ssh_destinations(item, destinations);
            }
        }
        Value::Object(object) => {
            let name = VM_NAME_KEYS
                .iter()
                .find_map(|key| object.get(*key).and_then(Value::as_str));
            if let Some(name) = name
                && let Some(destination) = ssh_destination_from_object(object)
            {
                destinations.insert(name.to_string(), destination);
            }
            for key in ["vms", "items", "data"] {
                if let Some(child) = object.get(key) {
                    collect_ssh_destinations(child, destinations);
                }
            }
        }
        _ => {}
    }
}

fn ssh_destination_from_object(object: &serde_json::Map<String, Value>) -> Option<String> {
    let text = |key: &str| {
        object
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    };
    if let Some(dest) = text("ssh_dest").or_else(|| text("sshDest")) {
        return Some(dest.to_string());
    }
    let host = text("ssh_host").or_else(|| text("sshHost"))?;
    match text("ssh_user").or_else(|| text("sshUser")) {
        Some(user) => Some(format!("{user}@{host}")),
        None => Some(host.to_string()),
    }
}

pub(super) fn parse_vm_names_from_text(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(|line| line.split_whitespace().next())
        // Only the first column of the first row is a header. Dropping every row
        // whose name starts with "name" would lose a VM actually called
        // `nameserver`, and bootstrap would then try to create it again.
        .enumerate()
        .filter(|(index, name)| *index > 0 || !name.eq_ignore_ascii_case("name"))
        .map(|(_, name)| name)
        .filter(|name| is_vm_name(name))
        .map(str::to_string)
        .collect()
}

/// Whether a word from rendered output can be a VM name at all.
///
/// Not every non-JSON body is a table: `Error: quota exceeded` and
/// `VM vm-1 is unavailable` would otherwise contribute `Error:` and `VM` as VMs,
/// and a planned VM reported that way would look like it already exists. exe.dev
/// names are DNS labels, so anything else is prose rather than a row.
fn is_vm_name(word: &str) -> bool {
    !word.is_empty()
        && word.len() <= 63
        && word.starts_with(|ch: char| ch.is_ascii_lowercase() || ch.is_ascii_digit())
        && word
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
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
