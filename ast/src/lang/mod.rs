pub mod asg;
pub mod call_finder;
pub mod embedding;
pub mod graphs;
pub mod linker;
pub mod parse;
pub mod queries;

use asg::*;
use consts::*;
pub use graphs::*;
use lsp::{CmdSender, Language};
use queries::*;
use shared::{Context, Result};
use std::fmt;
use std::str::FromStr;
use streaming_iterator::{IntoStreamingIterator, StreamingIterator};
use tracing::trace;
use tree_sitter::{Node as TreeNode, Query, QueryCursor};

pub struct Lang {
    pub kind: Language,
    lang: Box<dyn Stack + Send + Sync + 'static>,
}

impl fmt::Display for Lang {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Lang Kind: {:?}", self.kind)
    }
    //test
}

// function, operand, requests within, data models within, trait operand, return types
pub type Function = (
    NodeData,
    Option<Operand>,
    Vec<NodeData>,
    Vec<Edge>,
    Option<Edge>,
    Vec<Edge>,
);
// Calls, args, external function (from library or std), call another Class
pub type FunctionCall = (Calls, Option<NodeData>, Option<NodeData>);

impl Lang {
    pub fn new_python() -> Self {
        Self {
            kind: Language::Python,
            lang: Box::new(python::Python::new()),
        }
    }
    pub fn new_go() -> Self {
        Self {
            kind: Language::Go,
            lang: Box::new(go::Go::new()),
        }
    }
    pub fn new_rust() -> Self {
        Self {
            kind: Language::Rust,
            lang: Box::new(rust::Rust::new()),
        }
    }
    pub fn new_react() -> Self {
        Self {
            kind: Language::React,
            lang: Box::new(react::ReactTs::new()),
        }
    }
    pub fn new_typescript() -> Self {
        Self {
            kind: Language::Typescript,
            lang: Box::new(typescript::TypeScript::new()),
        }
    }
    pub fn new_ruby() -> Self {
        Self {
            kind: Language::Ruby,
            lang: Box::new(ruby::Ruby::new()),
        }
    }
    pub fn new_kotlin() -> Self {
        Self {
            kind: Language::Kotlin,
            lang: Box::new(kotlin::Kotlin::new()),
        }
    }
    pub fn new_swift() -> Self {
        Self {
            kind: Language::Swift,
            lang: Box::new(swift::Swift::new()),
        }
    }
    pub fn new_java() -> Self {
        Self {
            kind: Language::Java,
            lang: Box::new(java::Java::new()),
        }
    }
    pub fn new_svelte() -> Self {
        Self {
            kind: Language::Svelte,
            lang: Box::new(svelte::Svelte::new()),
        }
    }
    pub fn new_angular() -> Self {
        Self {
            kind: Language::Angular,
            lang: Box::new(angular::Angular::new()),
        }
    }
    pub fn new_cpp() -> Self {
        Self {
            kind: Language::Cpp,
            lang: Box::new(cpp::Cpp::new()),
        }
    }
    pub fn lang(&self) -> &dyn Stack {
        self.lang.as_ref()
    }
    pub fn q(&self, q: &str, nt: &NodeType) -> Query {
        self.lang.q(q, nt)
    }
    pub fn get_libs<G: Graph>(&self, code: &str, file: &str) -> Result<Vec<NodeData>> {
        if let Some(qo) = self.lang.lib_query() {
            let qo = self.q(&qo, &NodeType::Library);
            Ok(self.collect::<G>(&qo, code, file, NodeType::Library)?)
        } else {
            Ok(Vec::new())
        }
    }
    pub fn get_classes<G: Graph>(&self, code: &str, file: &str) -> Result<Vec<NodeData>> {
        let qo = self.q(&self.lang.class_definition_query(), &NodeType::Class);
        Ok(self.collect::<G>(&qo, code, file, NodeType::Class)?)
    }
    pub fn get_traits<G: Graph>(&self, code: &str, file: &str) -> Result<Vec<NodeData>> {
        if let Some(qo) = self.lang.trait_query() {
            let qo = self.q(&qo, &NodeType::Trait);
            Ok(self.collect::<G>(&qo, code, file, NodeType::Trait)?)
        } else {
            Ok(Vec::new())
        }
    }
    pub fn get_imports<G: Graph>(&self, code: &str, file: &str) -> Result<Vec<NodeData>> {
        if let Some(qo) = self.lang.imports_query() {
            let qo = self.q(&qo, &NodeType::Import);
            Ok(self.collect::<G>(&qo, code, file, NodeType::Import)?)
        } else {
            Ok(Vec::new())
        }
    }
    pub fn get_vars<G: Graph>(&self, code: &str, file: &str) -> Result<Vec<NodeData>> {
        if let Some(qo) = self.lang.variables_query() {
            let qo = self.q(&qo, &NodeType::Var);
            Ok(self.collect::<G>(&qo, code, file, NodeType::Var)?)
        } else {
            Ok(Vec::new())
        }
    }
    pub fn get_pages<G: Graph>(
        &self,
        code: &str,
        file: &str,
        lsp_tx: &Option<CmdSender>,
        graph: &G,
    ) -> Result<Vec<(NodeData, Vec<Edge>)>> {
        if let Some(qo) = self.lang.page_query() {
            let qo = self.q(&qo, &NodeType::Page);
            Ok(self.collect_pages(&qo, code, file, lsp_tx, graph)?)
        } else {
            Ok(Vec::new())
        }
    }
    pub fn get_component_templates<G: Graph>(
        &self,
        code: &str,
        file: &str,
        _graph: &G,
    ) -> Result<Vec<Edge>> {
        if let Some(qo) = self.lang.component_template_query() {
            let qo = self.q(&qo, &NodeType::Class);
            let tree = self.lang.parse(&code, &NodeType::Class)?;
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&qo, tree.root_node(), code.as_bytes());

            let mut template_urls = Vec::new();
            let mut style_urls = Vec::new();
            let mut component = NodeData::in_file(file);

            let class_query = self.q(&self.lang.class_definition_query(), &NodeType::Class);
            let mut class_cursor = QueryCursor::new();
            let mut class_matches =
                class_cursor.matches(&class_query, tree.root_node(), code.as_bytes());

            if let Some(class_match) = class_matches.next() {
                for o in class_query.capture_names().iter() {
                    if let Some(ci) = class_query.capture_index_for_name(&o) {
                        let mut nodes = class_match.nodes_for_capture_index(ci);
                        if let Some(node) = nodes.next() {
                            if o == &CLASS_NAME {
                                component.name = node.utf8_text(code.as_bytes())?.to_string();
                            } else if o == &CLASS_DEFINITION {
                                component.start = node.start_position().row as usize;
                            }
                        }
                    }
                }
            }

            if component.name.is_empty() {
                return Ok(Vec::new());
            }

            while let Some(m) = matches.next() {
                let mut key = String::new();
                let mut value = String::new();

                for o in qo.capture_names().iter() {
                    if let Some(ci) = qo.capture_index_for_name(&o) {
                        let mut nodes = m.nodes_for_capture_index(ci);
                        if let Some(node) = nodes.next() {
                            let text = node.utf8_text(code.as_bytes())?.to_string();
                            if o == &TEMPLATE_KEY {
                                key = text;
                            } else if o == &TEMPLATE_VALUE {
                                value = text;
                            }
                        }
                    }
                }

                if !key.is_empty() && !value.is_empty() {
                    if key == "templateUrl" {
                        let template_url = parse::trim_quotes(&value);
                        template_urls.push(template_url.to_string());
                    } else if key == "styleUrls" {
                        if value.starts_with("[") && value.ends_with("]") {
                            let array_content = &value[1..value.len() - 1];
                            for style_url in array_content.split(",") {
                                let style_url = parse::trim_quotes(style_url.trim());
                                if !style_url.is_empty() {
                                    style_urls.push(style_url.to_string());
                                }
                            }
                        }
                    }
                }
            }

            let mut edges = Vec::new();

            for template_url in template_urls {
                let mut path = template_url;
                if path.starts_with("./") {
                    path = path[2..].to_string();
                }

                let dir = std::path::Path::new(file)
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();

                let full_path = if dir.is_empty() {
                    path.clone()
                } else {
                    format!("{}/{}", dir, path)
                };

                let template_name = std::path::Path::new(&path)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("template");

                let page = NodeData::name_file(template_name, &full_path);
                edges.push(Edge::render_from_class(&component, &page));
            }

            for style_url in style_urls {
                let mut path = style_url;
                if path.starts_with("./") {
                    path = path[2..].to_string();
                }

                let dir = std::path::Path::new(file)
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();

                let full_path = if dir.is_empty() {
                    path.clone()
                } else {
                    format!("{}/{}", dir, path)
                };

                let style_name = std::path::Path::new(&path)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("style");

                let page = NodeData::name_file(style_name, &full_path);
                edges.push(Edge::render_from_class(&component, &page));
            }

            return Ok(edges);
        }

        Ok(Vec::new())
    }
    pub fn get_identifier_for_node(&self, node: TreeNode, code: &str) -> Result<Option<String>> {
        let query = self.q(&self.lang.identifier_query(), &NodeType::Function);
        let ident = Self::get_identifier_for_query(query, node, code)?;
        Ok(ident)
    }
    pub fn get_identifier_for_query(
        query: Query,
        node: TreeNode,
        code: &str,
    ) -> Result<Option<String>> {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, node, code.as_bytes());
        let first = matches.next();
        if first.is_none() {
            return Ok(None);
        }
        let mut cs = first.unwrap().captures.iter().into_streaming_iter_ref();
        let name_node = cs.next().context("no name_node")?;
        let name = name_node.node.utf8_text(code.as_bytes())?;
        Ok(Some(name.to_string()))
    }
    // returns (Vec<Function>, Vec<TestRecord>)
    pub fn get_functions_and_tests<G: Graph>(
        &self,
        code: &str,
        file: &str,
        graph: &G,
        lsp_tx: &Option<CmdSender>,
    ) -> Result<(Vec<Function>, Vec<TestRecord>)> {
        let qo = self.q(&self.lang.function_definition_query(), &NodeType::Function);
        let funcs1 = self.collect_functions(&qo, code, file, graph, lsp_tx)?;
        let (funcs, filtered_tests) = self.lang.filter_tests(funcs1);
        let mut tests: Vec<TestRecord> = Vec::new();
        for t in filtered_tests.iter() {
            let nd = t.0.clone();
            let kind = match nd.meta.get("test_kind").map(|s| s.as_str()) {
                Some("integration") => NodeType::IntegrationTest,
                Some("e2e") => NodeType::E2eTest,
                _ => NodeType::UnitTest,
            };
              //TODO: Add edge relationships with other nodes
            tests.push(TestRecord::new(nd, kind, None));
        }
        if let Some(tq) = self.lang.test_query() {
            let qo2 = self.q(&tq, &NodeType::UnitTest);
            let more_tests = self.collect_tests(&qo2, code, file)?;
            for mt in more_tests {
                let nd = mt.0.clone();
                let kind = match nd.meta.get("test_kind").map(|s| s.as_str()) {
                    Some("integration") => NodeType::IntegrationTest,
                    Some("e2e") => NodeType::E2eTest,
                    _ => NodeType::UnitTest,
                };
                //TODO: Add edge relationships with other nodes
                tests.push(TestRecord::new(nd, kind, None));
            }
        }
        if let Ok(int_tests) = self.collect_integration_tests::<G>(code, file, graph) {
            for (nd, tt, edge) in int_tests {
                tests.push(TestRecord::new(nd, tt, edge));
            }
        }
        if let Ok(e2e_tests) = self.collect_e2e_tests(code, file) {
            for nd in e2e_tests {
                tests.push(TestRecord::new(nd, NodeType::E2eTest, None));
            }
        }
        Ok((funcs, tests))
    }
    pub fn get_query_opt<G: Graph>(
        &self,
        q: Option<String>,
        code: &str,
        file: &str,
        fmtr: NodeType,
    ) -> Result<Vec<NodeData>> {
        if let Some(qo) = q {
            let insts = self.collect::<G>(&self.q(&qo, &fmtr), code, file, fmtr)?;
            Ok(insts)
        } else {
            Ok(Vec::new())
        }
    }
    // returns (Vec<CallsFromFunctions>, Vec<CallsFromTests>, Vec<IntegrationTests>, Vec<ExtraCalls>)
    pub async fn get_function_calls<G: Graph>(
        &self,
        code: &str,
        file: &str,
        graph: &G,
        lsp_tx: &Option<CmdSender>,
    ) -> Result<(Vec<FunctionCall>, Vec<FunctionCall>, Vec<Edge>, Vec<Edge>)> {
        trace!("get_function_calls");
        let tree = self.lang.parse(&code, &NodeType::Function)?;
        // get each function
        let qo1 = self.q(&self.lang.function_definition_query(), &NodeType::Function);
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&qo1, tree.root_node(), code.as_bytes());
        // calls from functions, calls from tests, integration tests
        let mut res = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        // get each function call within that function
        while let Some(m) = matches.next() {
            // FIXME can we only pass in the node code here? Need to sum line nums
            trace!("add_calls_for_function");
            let mut caller_name = "".to_string();
            Self::loop_captures(&qo1, &m, code, |body, node, o| {
                if o == FUNCTION_NAME {
                    caller_name = body;
                } else if o == FUNCTION_DEFINITION {
                    let caller_start = node.start_position().row as usize;
                    // NOTE this should always be the last one
                    let q2 = self.q(&self.lang.function_call_query(), &NodeType::Function);
                    let calls = self.collect_calls_in_function(
                        &q2,
                        code,
                        file,
                        node,
                        &caller_name,
                        caller_start,
                        graph,
                        lsp_tx,
                    )?;
                    self.add_calls_inside(&mut res, &caller_name, file, calls);
                    if self.lang.is_test(&caller_name, file) {
                        let int_calls = self.collect_integration_test_calls(
                            code,
                            file,
                            node,
                            &caller_name,
                            graph,
                            lsp_tx,
                        )?;
                        res.2.extend(int_calls);
                    }
                    for eq in self.lang.extra_calls_queries() {
                        let qex = self.q(&eq, &NodeType::Function);
                        let extras = self.collect_extras_in_function(
                            &qex,
                            code,
                            file,
                            node,
                            &caller_name,
                            caller_start,
                            graph,
                            lsp_tx,
                        )?;
                        res.3.extend(extras);
                    }
                }
                Ok(())
            })?;
        }

        if let Some(tq) = self.lang.test_query() {
            let q_tests = self.q(&tq, &NodeType::UnitTest);
            let tree_tests = self.lang.parse(&code, &NodeType::UnitTest)?;
            let mut cursor_tests = QueryCursor::new();
            let mut test_matches =
                cursor_tests.matches(&q_tests, tree_tests.root_node(), code.as_bytes());

            while let Some(tm) = test_matches.next() {
                let mut caller_name = String::new();
                Self::loop_captures(&q_tests, &tm, code, |body, node, o| {
                    if o == FUNCTION_NAME {
                        caller_name = body;
                    } else if o == FUNCTION_DEFINITION {
                        let caller_start = node.start_position().row as usize;
                        let q2 = self.q(&self.lang.function_call_query(), &NodeType::Function);
                        let calls = self.collect_calls_in_function(
                            &q2,
                            code,
                            file,
                            node,
                            &caller_name,
                            caller_start,
                            graph,
                            lsp_tx,
                        )?;
                        self.add_calls_inside(&mut res, &caller_name, file, calls);
                        // link test to endpoint: integration tests
                        if let Some(rq) = self.lang.request_finder() {
                            let rq_q = self.q(&rq, &NodeType::Request);
                            let mut cursor_r = QueryCursor::new();
                            let mut matches_r = cursor_r.matches(&rq_q, node, code.as_bytes());
                            while let Some(mr) = matches_r.next() {
                                if let Ok(reqs) =
                                    self.format_endpoint::<G>(&mr, code, file, &rq_q, None, &None)
                                {
                                    for (req_node, _edge_opt) in reqs {
                                        if req_node.name.is_empty() {
                                            continue;
                                        }
                                        let mut path = req_node.name.clone();
                                        if let Some(pos) = path.find("://") {
                                            if let Some(start) = path[pos + 3..].find('/') {
                                                path = path[pos + 3 + start..].to_string();
                                            }
                                        }
                                        if let Some(q) = path.find('?') {
                                            path = path[..q].to_string();
                                        }
                                        if let Some(h) = path.find('#') {
                                            path = path[..h].to_string();
                                        }
                                        if path.len() > 1 && path.ends_with('/') {
                                            path.pop();
                                        }

                                        let verb = req_node
                                            .meta
                                            .get("verb")
                                            .cloned()
                                            .unwrap_or_else(|| "GET".to_string());

                                        let mut endpoints = graph.find_resource_nodes(
                                            NodeType::Endpoint,
                                            &verb,
                                            &path,
                                        );
                                        if endpoints.is_empty() {
                                            endpoints =
                                                graph.find_nodes_by_name(NodeType::Endpoint, &path);
                                            if !endpoints.is_empty() {
                                                endpoints
                                                    .retain(|e| e.meta.get("verb") == Some(&verb));
                                            }
                                            if endpoints.is_empty() {
                                                let mut eps = graph.find_nodes_by_name(
                                                    NodeType::Endpoint,
                                                    &req_node.name,
                                                );
                                                eps.retain(|e| e.meta.get("verb") == Some(&verb));
                                                endpoints = eps;
                                            }
                                        }
                                        for ep in endpoints {
                                            let source =
                                                NodeKeys::new(&caller_name, file, caller_start);
                                            let edge = Edge::new(
                                                EdgeType::Calls,
                                                NodeRef::from(source, NodeType::UnitTest),
                                                NodeRef::from(ep.into(), NodeType::Endpoint),
                                            );
                                            res.2.push(edge);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Ok(())
                })?;
            }
        }
        Ok(res)
    }
    fn add_calls_inside(
        &self,
        res: &mut (Vec<FunctionCall>, Vec<FunctionCall>, Vec<Edge>, Vec<Edge>),
        caller_name: &str,
        caller_file: &str,
        calls: Vec<FunctionCall>,
    ) {
        if self.lang.is_test(&caller_name, caller_file) {
            res.1.extend_from_slice(&calls);
        } else {
            res.0.extend_from_slice(&calls);
        }
    }
}

impl Lang {
    pub fn from_language(l: Language) -> Lang {
        match l {
            Language::Rust => Lang::new_rust(),
            Language::Python => Lang::new_python(),
            Language::Go => Lang::new_go(),
            Language::Typescript => Lang::new_typescript(),
            Language::React => Lang::new_react(),
            Language::Ruby => Lang::new_ruby(),
            Language::Bash => unimplemented!(),
            Language::Toml => unimplemented!(),
            Language::Kotlin => Lang::new_kotlin(),
            Language::Swift => Lang::new_swift(),
            Language::Java => Lang::new_java(),
            Language::Svelte => Lang::new_svelte(),
            Language::Angular => Lang::new_angular(),
            Language::Cpp => Lang::new_cpp(),
        }
    }
}
impl FromStr for Lang {
    type Err = shared::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "tsx" | "jsx" => Ok(Lang::new_react()),
            _ => {
                let ss = Language::from_str(s)?;
                Ok(Lang::from_language(ss))
            }
        }
    }
}

pub fn vecy(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| s.to_string()).collect()
}

pub fn query_to_ident(query: Query, node: TreeNode, code: &str) -> Result<Option<String>> {
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, node, code.as_bytes());
    let first = matches.next();
    if first.is_none() {
        return Ok(None);
    }
    let mut cs = first.unwrap().captures.iter().into_streaming_iter_ref();
    let name_node = cs.next().context("no name_node")?;
    let name = name_node.node.utf8_text(code.as_bytes())?;
    Ok(Some(name.to_string()))
}
