use crate::lang::graphs::neo4j_graph::Neo4jGraph;
use crate::lang::graphs::{BTreeMapGraph, Graph};
use crate::repo::{check_revs_files, Repo};
use anyhow::Result;
use serde_json;
use std::fs::File;
use std::io::{BufWriter, Write};
use tracing::info;

pub struct GraphOps {
    pub graph: Neo4jGraph,
}

impl GraphOps {
    pub fn new() -> Self {
        Self {
            graph: Neo4jGraph::default(),
        }
    }

    pub fn connect(&mut self) -> Result<()> {
        futures::executor::block_on(self.graph.connect())
    }

    pub fn clear(&mut self) -> Result<(u32, u32)> {
        self.graph.clear();
        let (nodes, edges) = self.graph.get_graph_size();
        info!("Graph cleared - Nodes: {}, Edges: {}", nodes, edges);
        Ok((nodes, edges))
    }

    pub fn update_incremental(
        &mut self,
        repo_url: &str,
        repo_path: &str,
        current_hash: &str,
        stored_hash: &str,
    ) -> Result<(u32, u32)> {
        let revs = vec![stored_hash.to_string(), current_hash.to_string()];
        if let Some(modified_files) = check_revs_files(repo_path, revs.clone()) {
            info!(
                "Processing {} changed files between commits",
                modified_files.len()
            );

            if !modified_files.is_empty() {
                let mut all_incoming_edges = Vec::new();
                for file in &modified_files {
                    all_incoming_edges.extend(self.graph.get_incoming_edges_for_file(file));
                    self.graph.remove_nodes_by_file(file)?;
                }

                let file_repos = futures::executor::block_on(Repo::new_multi_detect(
                    repo_path,
                    Some(repo_url.to_string()),
                    modified_files.clone(),
                    Vec::new(),
                ))?;

                for repo in &file_repos.0 {
                    let file_graph =
                        futures::executor::block_on(repo.build_graph_inner::<Neo4jGraph>())?;
                    let (nodes_before, edges_before) = self.graph.get_graph_size();
                    self.graph.extend_graph(file_graph);

                    for (edge, _target_data) in &all_incoming_edges {
                        let source_exists = self
                            .graph
                            .find_nodes_by_name(
                                edge.source.node_type.clone(),
                                &edge.source.node_data.name,
                            )
                            .iter()
                            .any(|n| n.file == edge.source.node_data.file);
                        let target_exists = self
                            .graph
                            .find_nodes_by_name(
                                edge.target.node_type.clone(),
                                &edge.target.node_data.name,
                            )
                            .iter()
                            .any(|n| n.file == edge.target.node_data.file);
                        if source_exists && target_exists {
                            self.graph.add_edge(edge.clone());
                        }
                    }

                    let (nodes_after, edges_after) = self.graph.get_graph_size();
                    info!(
                        "Updated files: added {} nodes and {} edges",
                        nodes_after - nodes_before,
                        edges_after - edges_before
                    );
                }
            }
        }
        self.graph.update_repository_hash(repo_url, current_hash)?;
        Ok(self.graph.get_graph_size())
    }

    pub fn update_full(
        &mut self,
        repo_url: &str,
        repo_path: &str,
        current_hash: &str,
    ) -> Result<(u32, u32)> {
        let repos = futures::executor::block_on(Repo::new_multi_detect(
            repo_path,
            Some(repo_url.to_string()),
            Vec::new(),
            Vec::new(),
        ))?;

        let temp_graph = futures::executor::block_on(repos.build_graphs_inner::<Neo4jGraph>())?;
        self.graph.extend_graph(temp_graph);

        self.graph.update_repository_hash(repo_url, current_hash)?;
        Ok(self.graph.get_graph_size())
    }

    /// Export a BTreeMapGraph to JSONL format
    /// This is stage 1 of our two-stage async process
    pub async fn export_btreemap_to_jsonl<P: AsRef<std::path::Path>>(
        &self,
        graph: &BTreeMapGraph,
        path: P,
    ) -> Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        // Export nodes
        for (_, node) in &graph.nodes {
            let json_line = serde_json::to_string(node)?;
            writeln!(writer, "{}", json_line)?;
        }

        // Export edges
        for edge in graph.to_array_graph_edges() {
            let json_line = serde_json::to_string(&edge)?;
            writeln!(writer, "{}", json_line)?;
        }

        writer.flush()?;
        info!(
            "Exported BTreeMapGraph to JSONL with {} nodes and {} edges",
            graph.nodes.len(),
            graph.edges.len()
        );
        Ok(())
    }

    /// Import JSONL data to Neo4j database
    /// This is stage 2 of our two-stage async process
    pub async fn import_jsonl_to_neo4j<P: AsRef<std::path::Path>>(
        &mut self,
        path: P,
    ) -> Result<()> {
        use std::io::{BufRead, BufReader};

        let file = File::open(path)?;
        let reader = BufReader::new(file);

        let mut node_count = 0;
        let mut edge_count = 0;

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            // Try to parse as Node first, then as Edge
            if let Ok(node) = serde_json::from_str::<crate::lang::graphs::Node>(&line) {
                self.graph.add_node(node.node_type, node.node_data).await;
                node_count += 1;
            } else if let Ok(edge) = serde_json::from_str::<crate::lang::graphs::Edge>(&line) {
                self.graph.add_edge(edge).await;
                edge_count += 1;
            }
        }

        info!(
            "Imported {} nodes and {} edges to Neo4j",
            node_count, edge_count
        );
        Ok(())
    }
}
