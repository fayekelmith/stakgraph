use crate::lang::graph::Node;
use crate::lang::graph_trait::Graph;
use crate::lang::Lang;
use crate::repo::Repo;
use anyhow::Context;
use lsp::Language as LspLanguage;
use std::collections::HashMap;
use std::result::Result;
use tracing::info;
pub struct FrontendTester<G>
where
    G: Graph,
{
    graph: G,
    lang: Lang,
    repo: Option<String>,
}

pub struct FrontendArtefact<'a> {
    pub components: Vec<&'a str>,
    pub request: Vec<(&'a str, &'a str)>,
    pub pages: Vec<&'a str>,
    pub data_model: &'a str,
    pub contains_pages_and_components: Vec<&'a LspLanguage>,
}

impl FrontendArtefact<'_> {
    pub fn default() -> FrontendArtefact<'static> {
        FrontendArtefact {
            components: vec!["NewPerson", "People"],
            request: vec![("GET", "/people"), ("POST", "/person")],
            pages: vec!["/new-person", "/people"],
            data_model: "Person",
            contains_pages_and_components: vec![&LspLanguage::React],
        }
    }
}
impl<G> FrontendTester<G>
where
    G: Graph,
{
    pub async fn new(lang: Lang, repo: Option<String>) -> Result<Self, anyhow::Error> {
        let language_name = lang.kind.clone();
        let language_in_repository = Lang::from_language(language_name.clone());

        let repo_path = repo.clone().unwrap_or_else(|| language_name.to_string());
        let repository = Repo::new(
            &format!("src/testing/{}", repo_path),
            language_in_repository,
            false,
            Vec::new(),
            Vec::new(),
        )?;

        Ok(Self {
            graph: repository
                .build_graph()
                .await
                .with_context(|| format!("Failed to build graph for {}", repo_path))?,
            lang,
            repo: Some(repo_path),
        })
    }

    pub fn test_frontend(&self) -> Result<(), anyhow::Error> {
        let artefact = FrontendArtefact::default();

        info!(
            "\n\nTesting frontend for {} at src/testing/{}\n\n",
            self.lang.kind.to_string().to_uppercase(),
            self.repo.as_ref().unwrap()
        );

        self.test_language()?;
        self.test_package_file()?;
        self.test_data_model(artefact.data_model)?;
        if artefact
            .contains_pages_and_components
            .contains(&&self.lang.kind)
        {
            self.test_components(artefact.components)?;
            self.test_pages(artefact.pages)?;
        }
        self.test_requests(artefact.request)?;

        Ok(())
    }

    fn test_language(&self) -> Result<(), anyhow::Error> {
        let language_nodes = self.graph.find_languages();

        let language_node = language_nodes
            .iter()
            .find(|node| node.into_data().name == self.lang.kind.to_string())
            .unwrap();

        assert_eq!(
            language_node.into_data().name,
            self.lang.kind.to_string(),
            "Language node name mismatch"
        );

        Ok(())
    }

    fn test_package_file(&self) -> Result<(), anyhow::Error> {
        let package_file_name = self.lang.kind.pkg_file();

        let file_nodes = self
            .graph
            .nodes()
            .iter()
            .filter(|node| matches!(node, Node::File(_)))
            .collect::<Vec<_>>();

        let package_files: Vec<_> = file_nodes
            .iter()
            .filter(|node| {
                let file_data = node.into_data();
                file_data.name.contains(&package_file_name)
            })
            .collect();

        assert!(
            !package_files.is_empty(),
            "No package file found matching {}",
            package_file_name
        );

        info!("✓ Found package file {}", package_file_name);

        Ok(())
    }

    fn test_data_model(&self, data_model: &str) -> Result<(), anyhow::Error> {
        let data_model_nodes = self
            .graph
            .find_data_model_by(|node| node.name.contains(data_model));

        info!("✓ Found data model {}", data_model);

        assert!(
            !data_model_nodes.is_none(),
            "No data model found matching {}",
            data_model
        );

        Ok(())
    }

    fn test_components(&self, expected_components: Vec<&str>) -> Result<(), anyhow::Error> {
        self.verify_nodes(
            expected_components,
            |node| matches!(node, Node::Function(_)),
            |component, name| component.contains(name),
            "component",
        )
    }

    fn test_pages(&self, expected_pages: Vec<&str>) -> Result<(), anyhow::Error> {
        self.verify_nodes(
            expected_pages,
            |node| matches!(node, Node::Page(_)),
            |page, name| page.contains(name),
            "page",
        )
    }

    fn test_requests(&self, expected_requests: Vec<(&str, &str)>) -> Result<(), anyhow::Error> {
        let requests = self
            .graph
            .nodes()
            .iter()
            .filter(|node| matches!(node, Node::Request(_)))
            .collect::<Vec<_>>();

        let mut found_requests: HashMap<(String, String), bool> = expected_requests
            .iter()
            .map(|request| ((request.0.to_string(), request.1.to_string()), false))
            .collect();

        for request in &requests {
            let request_data = request.into_data();

            let verb = match request_data.meta.get("verb") {
                Some(verb_value) => verb_value.to_uppercase(),
                None => "".to_string(),
            };

            let url = &request_data.name;

            for ((expected_verb, expected_endpoint), found) in found_requests.iter_mut() {
                if verb == expected_verb.to_uppercase() && url.contains(expected_endpoint) {
                    *found = true;
                    info!("✓ Found request: {} {} ({})", verb, expected_endpoint, url);
                }
            }
        }

        for ((verb, endpoint), found) in found_requests.iter() {
            assert!(*found, "Request {} {} not found in graph", verb, endpoint);
        }

        Ok(())
    }

    fn verify_nodes<T, F>(
        &self,
        expected_items: Vec<T>,
        filter_fn: F,
        match_fn: impl Fn(&T, &String) -> bool,
        item_type: &str,
    ) -> Result<(), anyhow::Error>
    where
        T: std::fmt::Display + std::cmp::Eq + std::hash::Hash,
        F: Fn(&&Node) -> bool,
    {
        let nodes = self
            .graph
            .nodes()
            .iter()
            .filter(filter_fn)
            .collect::<Vec<_>>();

        let mut found_map: HashMap<T, bool> = expected_items
            .into_iter()
            .map(|item| (item, false))
            .collect();

        for node in nodes {
            let name = node.into_data().name.to_string();
            for (item, found) in found_map.iter_mut() {
                if match_fn(item, &name) {
                    *found = true;
                    info!("✓ Found {} {}", item_type, name);
                }
            }
        }

        for (item, found) in found_map.iter() {
            assert!(*found, "{} {} not found in graph", item_type, item);
        }

        Ok(())
    }
}
