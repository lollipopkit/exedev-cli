use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::{collections::BTreeMap, fs, path::Path};

const DEFAULT_IMAGE: &str = "ubuntu:22.04";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FleetFile {
    pub(crate) cluster: Cluster,
    #[serde(default)]
    pub(crate) defaults: Defaults,
    #[serde(default)]
    pub(crate) projects: BTreeMap<String, Project>,
    #[serde(default)]
    pub(crate) spare_pools: BTreeMap<String, SparePool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Cluster {
    pub(crate) name: String,
    pub(crate) control_plane: ControlPlane,
    pub(crate) worker_vm_budget: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ControlPlane {
    pub(crate) nodes: usize,
    pub(crate) vm_prefix: String,
    pub(crate) image: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct Defaults {
    pub(crate) network: Option<String>,
    pub(crate) kubernetes: Option<String>,
    pub(crate) isolated: Option<bool>,
    pub(crate) image: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Project {
    #[serde(default)]
    pub(crate) tasks: BTreeMap<String, TaskPool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TaskPool {
    pub(crate) nodes: usize,
    // Reserved for future workload manifest generation. VM allocation still
    // uses `nodes`; Kubernetes deployments own their replica counts today.
    #[allow(dead_code)]
    pub(crate) replicas: Option<usize>,
    pub(crate) vm_prefix: String,
    pub(crate) isolated: Option<bool>,
    pub(crate) image: Option<String>,
    #[serde(default)]
    pub(crate) labels: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SparePool {
    pub(crate) nodes: usize,
    pub(crate) vm_prefix: String,
    pub(crate) isolated: Option<bool>,
    pub(crate) image: Option<String>,
    #[serde(default)]
    pub(crate) labels: BTreeMap<String, String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NodeRole {
    ControlPlane,
    Worker,
    Spare,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NodeSpec {
    pub(crate) name: String,
    pub(crate) role: NodeRole,
    pub(crate) pool: String,
    pub(crate) image: String,
    pub(crate) labels: BTreeMap<String, String>,
    pub(crate) taint: Option<String>,
}

#[derive(Debug)]
pub(crate) struct FleetPlan {
    pub(crate) cluster_name: String,
    pub(crate) network: String,
    pub(crate) kubernetes: String,
    pub(crate) nodes: Vec<NodeSpec>,
}

impl FleetFile {
    pub(crate) fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("failed to read fleet file {}", path.display()))?;
        let fleet = serde_yaml::from_str::<Self>(&text)
            .with_context(|| format!("failed to parse fleet file {}", path.display()))?;
        fleet.validate()?;
        Ok(fleet)
    }

    pub(crate) fn validate(&self) -> Result<()> {
        if self.cluster.name.trim().is_empty() {
            bail!("cluster.name must not be empty");
        }
        if self.cluster.control_plane.nodes != 1 {
            bail!("only cluster.controlPlane.nodes: 1 is supported in v1");
        }
        if self.cluster.control_plane.vm_prefix.trim().is_empty() {
            bail!("cluster.controlPlane.vmPrefix must not be empty");
        }
        if self.network() != "tailscale" {
            bail!("only defaults.network: tailscale is supported in v1");
        }
        if self.kubernetes() != "k3s" {
            bail!("only defaults.kubernetes: k3s is supported in v1");
        }

        let worker_count = self.worker_node_count();
        if let Some(budget) = self.cluster.worker_vm_budget {
            if worker_count > budget {
                bail!("fleet requests {worker_count} worker VMs but workerVmBudget is {budget}");
            }
        }
        for (project_name, project) in &self.projects {
            for (task_name, task) in &project.tasks {
                if task.nodes == 0 {
                    bail!("projects.{project_name}.tasks.{task_name}.nodes must be greater than 0");
                }
                if task.vm_prefix.trim().is_empty() {
                    bail!("projects.{project_name}.tasks.{task_name}.vmPrefix must not be empty");
                }
            }
        }
        for (pool_name, pool) in &self.spare_pools {
            if pool.nodes == 0 {
                bail!("sparePools.{pool_name}.nodes must be greater than 0");
            }
            if pool.vm_prefix.trim().is_empty() {
                bail!("sparePools.{pool_name}.vmPrefix must not be empty");
            }
        }
        Ok(())
    }

    pub(crate) fn to_plan(&self) -> FleetPlan {
        let mut nodes = Vec::new();
        let default_image = self.default_image();
        let mut control_labels = BTreeMap::new();
        control_labels.insert("exedev.dev/role".into(), "control-plane".into());
        control_labels.insert("exedev.dev/pool".into(), "control-plane".into());
        nodes.push(NodeSpec {
            name: format!("{}-1", self.cluster.control_plane.vm_prefix),
            role: NodeRole::ControlPlane,
            pool: "control-plane".into(),
            image: self
                .cluster
                .control_plane
                .image
                .clone()
                .unwrap_or_else(|| default_image.clone()),
            labels: control_labels,
            taint: None,
        });

        for (project_name, project) in &self.projects {
            for (task_name, task) in &project.tasks {
                let pool = format!("{project_name}-{task_name}");
                for index in 1..=task.nodes {
                    let mut labels = task.labels.clone();
                    labels.insert("exedev.dev/project".into(), project_name.clone());
                    labels.insert("exedev.dev/task".into(), task_name.clone());
                    labels.insert("exedev.dev/pool".into(), pool.clone());
                    nodes.push(NodeSpec {
                        name: format!("{}-{index}", task.vm_prefix),
                        role: NodeRole::Worker,
                        pool: pool.clone(),
                        image: task.image.clone().unwrap_or_else(|| default_image.clone()),
                        labels,
                        taint: self
                            .is_isolated(task.isolated)
                            .then(|| format!("exedev.dev/pool={pool}:NoSchedule")),
                    });
                }
            }
        }

        for (pool_name, pool) in &self.spare_pools {
            for index in 1..=pool.nodes {
                let mut labels = pool.labels.clone();
                labels.insert("exedev.dev/pool".into(), pool_name.clone());
                nodes.push(NodeSpec {
                    name: format!("{}-{index}", pool.vm_prefix),
                    role: NodeRole::Spare,
                    pool: pool_name.clone(),
                    image: pool.image.clone().unwrap_or_else(|| default_image.clone()),
                    labels,
                    taint: self
                        .is_isolated(pool.isolated)
                        .then(|| format!("exedev.dev/pool={pool_name}:NoSchedule")),
                });
            }
        }

        FleetPlan {
            cluster_name: self.cluster.name.clone(),
            network: self.network(),
            kubernetes: self.kubernetes(),
            nodes,
        }
    }

    fn default_image(&self) -> String {
        self.defaults
            .image
            .clone()
            .unwrap_or_else(|| DEFAULT_IMAGE.into())
    }

    fn is_isolated(&self, value: Option<bool>) -> bool {
        value.unwrap_or(self.defaults.isolated.unwrap_or(false))
    }

    fn network(&self) -> String {
        self.defaults
            .network
            .clone()
            .unwrap_or_else(|| "tailscale".into())
    }

    fn kubernetes(&self) -> String {
        self.defaults
            .kubernetes
            .clone()
            .unwrap_or_else(|| "k3s".into())
    }

    fn worker_node_count(&self) -> usize {
        let task_nodes = self
            .projects
            .values()
            .flat_map(|project| project.tasks.values())
            .map(|task| task.nodes)
            .sum::<usize>();
        let spare_nodes = self
            .spare_pools
            .values()
            .map(|pool| pool.nodes)
            .sum::<usize>();
        task_nodes + spare_nodes
    }
}

impl FleetPlan {
    pub(crate) fn control_plane(&self) -> Option<&NodeSpec> {
        self.nodes
            .iter()
            .find(|node| node.role == NodeRole::ControlPlane)
    }

    pub(crate) fn bootstrap_nodes(&self, include_control_plane: bool) -> Vec<&NodeSpec> {
        self.nodes
            .iter()
            .filter(|node| include_control_plane || node.role != NodeRole::ControlPlane)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> FleetFile {
        serde_yaml::from_str(
            r#"
cluster:
  name: demo
  controlPlane:
    nodes: 1
    vmPrefix: cp
  workerVmBudget: 3
defaults:
  network: tailscale
  kubernetes: k3s
  isolated: true
projects:
  p1:
    tasks:
      a:
        nodes: 2
        replicas: 2
        vmPrefix: p1-a
sparePools:
  shared:
    nodes: 1
    vmPrefix: shared
    isolated: false
    labels:
      exedev.dev/purpose: shared
"#,
        )
        .unwrap()
    }

    #[test]
    fn expands_fleet_nodes_and_labels() {
        let plan = sample().to_plan();
        assert_eq!(plan.nodes.len(), 4);
        let worker = plan
            .nodes
            .iter()
            .find(|node| node.name == "p1-a-2")
            .unwrap();
        assert_eq!(worker.labels["exedev.dev/project"], "p1");
        assert_eq!(worker.labels["exedev.dev/task"], "a");
        assert_eq!(
            worker.taint.as_deref(),
            Some("exedev.dev/pool=p1-a:NoSchedule")
        );
        let shared = plan
            .nodes
            .iter()
            .find(|node| node.name == "shared-1")
            .unwrap();
        assert_eq!(shared.taint, None);
    }

    #[test]
    fn rejects_ha_control_plane() {
        let fleet: FleetFile = serde_yaml::from_str(
            r#"
cluster:
  name: demo
  controlPlane:
    nodes: 2
    vmPrefix: cp
"#,
        )
        .unwrap();
        assert!(fleet.validate().is_err());
    }
}
