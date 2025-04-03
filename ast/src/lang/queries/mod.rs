pub mod bash;
pub mod consts;
pub mod erb;
pub mod go;
pub mod haml;
pub mod java;
pub mod kotlin;
pub mod python;
mod rails_routes;
pub mod react;
pub mod ruby;
pub mod rust;
pub mod swift;
pub mod toml;
pub mod typescript;
pub mod svelte;
pub mod angular;

use crate::lang::asg::Operand;
use crate::lang::graph::Edge;
use crate::lang::graph_trait::Graph;
use crate::lang::{Function, Node, NodeData, NodeType};
use anyhow::Result;
use lsp::Language as LspLanguage;
use lsp::{CmdSender, Position};
use tree_sitter::{Node as TreeNode, Query, Tree};

// next: Java, PHP, Dart, C++

#[derive(Default, Debug)]
pub enum HandlerItemType {
    Collection,
    Member,
    ResourceMember,
    #[default]
    Namespace,
}

#[derive(Default, Debug)]
pub struct HandlerItem {
    pub item_type: HandlerItemType,
    pub name: String,
}

#[derive(Default, Debug)]
pub struct HandlerParams {
    pub actions_array: Option<String>,
    pub item: Option<HandlerItem>,
    pub parents: Vec<HandlerItem>, // nested resources OR namespaces in RoR
}

pub trait Stack {
    fn q(&self, q: &str, nt: &NodeType) -> Query;
    // use different parser for pkg files
    fn parse(&self, code: &str, nt: &NodeType) -> Result<Tree>;
    fn module_query(&self) -> Option<String> {
        None
    }
    fn lib_query(&self) -> Option<String> {
        None
    }
    fn is_lib_file(&self, file_name: &str) -> bool {
        // default: absolute path is library func
        file_name.starts_with("/")
    }
    fn is_component(&self, _func_name: &str) -> bool {
        false
    }
    // hack for now: imports are all concatenated into one section
    // so must be ONLY at the beginning of the file, with no other elements
    // only empty lines will be added between imports
    fn imports_query(&self) -> Option<String> {
        None
    }
    fn trait_query(&self) -> Option<String> {
        None
    }
    fn class_definition_query(&self) -> String;
    fn instance_definition_query(&self) -> Option<String> {
        None
    }
    fn function_definition_query(&self) -> String;
    fn test_query(&self) -> Option<String> {
        None
    }
    // fn method_definition_query(&self) -> Option<String>;
    fn function_call_query(&self) -> String;
    // this is optional if the one above captures both
    // fn method_call_query(&self) -> Option<String>;
    fn identifier_query(&self) -> String {
        format!("(identifier) @identifier")
    }
    fn type_identifier_node_name(&self) -> String {
        "type_identifier".to_string()
    }
    fn string_node_name(&self) -> String {
        "string".to_string()
    }
    // data model definitions
    fn data_model_query(&self) -> Option<String> {
        None
    }
    // data model CONTAINS edge within a function
    fn data_model_within_query(&self) -> Option<String> {
        None
    }
    fn data_model_path_filter(&self) -> Option<String> {
        None
    }
    fn use_data_model_within_finder(&self) -> bool {
        false
    }

    fn data_model_name(&self, dm_name: &str) -> String {
        dm_name.to_string()
    }

    // not used:
    // fn endpoint_handler_queries(&self) -> Vec<String> {
    //     Vec::new()
    // }
    fn endpoint_finders(&self) -> Vec<String> {
        Vec::new()
    }
    fn endpoint_group_find(&self) -> Option<String> {
        None
    }
    fn endpoint_path_filter(&self) -> Option<String> {
        None
    }
    fn request_finder(&self) -> Option<String> {
        None
    }
    fn is_test(&self, _func_name: &str, _func_file: &str) -> bool {
        false
    }
    fn is_test_file(&self, _filename: &str) -> bool {
        false
    }
    fn add_endpoint_verb(&self, _nd: &mut NodeData, _call: &Option<String>) {}
    fn update_endpoint_verb(&self, _nd: &mut NodeData, _call: &Option<String>) {}
    // this one should be the same for all langs?
    fn filter_tests(&self, funcs: Vec<Function>) -> (Vec<Function>, Vec<Function>) {
        let mut fs = Vec::new();
        let mut ts = Vec::new();
        for func in funcs {
            if self.is_test(&func.0.name, &func.0.file) {
                ts.push(func);
            } else {
                fs.push(func);
            }
        }
        (fs, ts)
    }
    fn e2e_test_id_finder_string(&self) -> Option<String> {
        None
    }
    fn use_handler_finder(&self) -> bool {
        false
    }

    fn integration_test_query(&self) -> Option<String> {
        None
    }
    fn use_integration_test_finder(&self) -> bool {
        false
    }

    fn is_router_file(&self, _file_name: &str, _code: &str) -> bool {
        false
    }
    fn page_query(&self) -> Option<String> {
        None
    }
    fn use_extra_page_finder(&self) -> bool {
        false
    }
    fn is_extra_page(&self, _file_name: &str) -> bool {
        false
    }
}

pub trait StackGraphOperations {
    fn extra_page_finder<G>(&self, _file_name: &str, _graph: &G) -> Option<Edge>
    where
        G: Graph,
    {
        None
    }
    fn find_endpoint_parents<G>(
        &self,
        _node: TreeNode,
        _code: &str,
        _file: &str,
        _graph: &G,
    ) -> Result<Vec<HandlerItem>>
    where
        G: Graph,
    {
        Ok(Vec::new())
    }

    fn clean_graph<G>(&self, _graph: &mut G) -> bool
    where
        G: Graph,
    {
        false
    }
    fn integration_test_edge_finder<G>(
        &self,
        _nd: &NodeData,
        _graph: &G,
        _tt: NodeType,
    ) -> Option<Edge>
    where
        G: Graph,
    {
        None
    }
    fn handler_finder<G>(
        &self,
        endpoint: NodeData,
        graph: &G,
        _handler_params: HandlerParams,
    ) -> Vec<(NodeData, Option<Edge>)>
    where
        G: Graph,
    {
        if let Some(handler) = endpoint.meta.get("handler") {
            if let Some(nd) = graph.find_exact_func(handler, &endpoint.file) {
                let edge = Edge::handler(&endpoint, &nd);
                return vec![(endpoint, Some(edge))];
            }
        }
        Vec::new()
    }
    fn find_function_parent<G>(
        &self,
        _node: TreeNode,
        _code: &str,
        _file: &str,
        _func_name: &str,
        _graph: &G,
        _parent_type: Option<&str>,
    ) -> Result<Option<Operand>>
    where
        G: Graph,
    {
        Ok(None)
    }
    fn find_trait_operand<G>(
        &self,
        _pos: Position,
        _nd: &NodeData,
        _graph: &G,
        _lsp_tx: &Option<CmdSender>,
    ) -> Result<Option<Edge>>
    where
        G: Graph,
    {
        Ok(None)
    }
    fn data_model_within_finder<G>(&self, _dm: &NodeData, _graph: &G) -> Vec<Edge>
    where
        G: Graph,
    {
        Vec::new()
    }
}

pub fn treesitter_from_lsp_language(ll: LspLanguage) -> tree_sitter::Language {
    match ll {
        LspLanguage::Bash => tree_sitter_bash::LANGUAGE.into(),
        LspLanguage::Go => tree_sitter_go::LANGUAGE.into(),
        LspLanguage::Python => tree_sitter_python::LANGUAGE.into(),
        LspLanguage::Ruby => tree_sitter_ruby::LANGUAGE.into(),
        LspLanguage::Toml => tree_sitter_toml_ng::LANGUAGE.into(),
        LspLanguage::Kotlin => tree_sitter_kotlin_sg::LANGUAGE.into(),
        LspLanguage::Swift => tree_sitter_swift::LANGUAGE.into(),
        LspLanguage::Rust => tree_sitter_rust::LANGUAGE.into(),
        LspLanguage::Java => tree_sitter_java::LANGUAGE.into(),
        LspLanguage::Typescript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        LspLanguage::React => tree_sitter_typescript::LANGUAGE_TSX.into(),
        LspLanguage::Svelte => tree_sitter_svelte_ng::LANGUAGE.into(),
        LspLanguage::Angular => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        // _ => tree_sitter_bash::LANGUAGE.into(),
    }
}

impl HandlerItem {
    pub fn new_member(name: &str) -> Self {
        HandlerItem {
            item_type: HandlerItemType::Member,
            name: name.to_string(),
        }
    }
    pub fn new_collection(name: &str) -> Self {
        HandlerItem {
            item_type: HandlerItemType::Collection,
            name: name.to_string(),
        }
    }
    pub fn new_resource_member(name: &str) -> Self {
        HandlerItem {
            item_type: HandlerItemType::ResourceMember,
            name: name.to_string(),
        }
    }
}

use std::collections::BTreeMap;
pub fn filter_out_classes_without_methods<G>(graph: &mut G) -> bool
where
    G: Graph,
{
    let mut assumed_class: BTreeMap<String, bool> = BTreeMap::new();
    let mut actual_class: BTreeMap<String, bool> = BTreeMap::new();

    for node in graph.nodes() {
        match node {
            Node::Function(func) => {
                if let Some(operand) = func.meta.get("operand") {
                    actual_class.insert(operand.to_string(), true);
                }
            }
            Node::Class(class_data) => {
                assumed_class.insert(class_data.name.to_string(), false);
            }
            _ => {}
        }
    }

    for key in actual_class.keys() {
        if let Some(entry) = assumed_class.get_mut(key) {
            *entry = true
        }
    }

    let classes_to_remove: Vec<_> = assumed_class
        .iter()
        .filter(|(_, &has_methods)| !has_methods)
        .map(|(name, _)| name.clone())
        .collect();

    graph.remove_node_by_predicate(|node| {
        if let Node::Class(class_data) = node {
            classes_to_remove.contains(&class_data.name)
        } else {
            false
        }
    });

    true
}
