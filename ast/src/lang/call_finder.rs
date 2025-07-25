use super::{graphs::Graph, *};

pub fn node_data_finder<G: Graph>(
    func_name: &str,
    graph: &G,
    current_file: &str,
    source_start: usize,
    source_node_type: NodeType,
) -> Option<NodeData> {
    func_target_file_finder(
        func_name,
        &None,
        graph,
        current_file,
        source_start,
        source_node_type,
    )
}

pub fn func_target_file_finder<G: Graph>(
    func_name: &str,
    _operand: &Option<String>,
    graph: &G,
    current_file: &str, // Add current file parameter
    source_start: usize,
    source_node_type: NodeType,
) -> Option<NodeData> {
    log_cmd(format!(
        "func_target_file_finder {:?} from file {:?}",
        func_name, current_file
    ));

    // First try: find only one function file
    if let Some(tf) = find_only_one_function_file(
        func_name,
        graph,
        source_start,
        current_file,
        source_node_type,
    ) {
        return Some(tf);
    }

    // Second try: find function with operand
    // if let Some(op) = operand {
    //     if let Some(tf) = find_function_with_operand(&op, func_name, graph) {
    //         return Some(tf);
    //     }
    // }

    // Third try: find in the same file
    if let Some(tf) = find_function_in_same_file(func_name, current_file, graph, source_start) {
        return Some(tf);
    }

    // Fourth try: find in the same directory
    if let Some(tf) = find_function_in_same_directory(func_name, current_file, graph, source_start)
    {
        return Some(tf);
    }

    None
}

fn find_only_one_function_file<G: Graph>(
    func_name: &str,
    graph: &G,
    source_start: usize,
    current_file: &str,
    source_node_type: NodeType,
) -> Option<NodeData> {
    let mut target_files_starts = Vec::new();
    let nodes = graph.find_nodes_by_name(NodeType::Function, func_name);
    if nodes.len() == 0 {
        log_cmd(format!("::: found zero {:?}", func_name));
        return None;
    }
    for node in nodes {
        let is_same = node.start == source_start && node.file == current_file;
        // NOT empty functions (interfaces)
        if !node.body.is_empty() {
            if !is_same || source_node_type != NodeType::Function {
                target_files_starts.push(node);
            }
        }
    }

    if target_files_starts.len() == 1 {
        return Some(target_files_starts[0].clone());
    }
    // TODO: disclude "mock"
    log_cmd(format!("::: found more than one {:?}", func_name));
    target_files_starts.retain(|x| !x.file.contains("mock"));
    if target_files_starts.len() == 1 {
        log_cmd(format!("::: discluded mocks for!!! {:?}", func_name));
        return Some(target_files_starts[0].clone());
    }
    None
}

fn _find_function_with_operand<G: Graph>(
    operand: &str,
    func_name: &str,
    graph: &G,
) -> Option<String> {
    let mut target_file = None;
    let mut instance = None;

    let operand_nodes = graph.find_nodes_by_name(NodeType::Instance, operand);
    for node in operand_nodes {
        instance = Some(node.clone());
        break;
    }
    if let Some(i) = instance {
        if let Some(dt) = &i.data_type {
            let function_nodes = graph.find_nodes_by_name(NodeType::Function, func_name);
            for node in function_nodes {
                if node.meta.get("operand") == Some(dt) {
                    target_file = Some(node.file.clone());
                    break;
                }
            }
        }
    }
    target_file
}

fn find_function_in_same_file<G: Graph>(
    func_name: &str,
    current_file: &str,
    graph: &G,
    source_start: usize,
) -> Option<NodeData> {
    let node =
        graph.find_node_by_name_and_file_end_with(NodeType::Function, func_name, current_file);
    if let Some(node) = node {
        // dont return like Label->label
        if node.name != func_name && node.name.to_lowercase() == func_name.to_lowercase() {
            return None;
        }
        if !node.body.is_empty() && node.file == current_file && node.start != source_start {
            log_cmd(format!(
                "::: found function in same file: {:?}",
                current_file
            ));
            return Some(node);
        }
    }

    None
}

fn find_function_in_same_directory<G: Graph>(
    func_name: &str,
    current_file: &str,
    graph: &G,
    source_start: usize,
) -> Option<NodeData> {
    let current_dir = std::path::Path::new(current_file)
        .parent()
        .and_then(|p| p.to_str())?;

    let nodes = graph.find_nodes_by_name(NodeType::Function, func_name);
    let mut same_dir_files = Vec::new();

    log_cmd(format!(
        "::: dir found {:?} nodes name: {:?} file: {:?} in dir: {:?}",
        nodes.len(),
        func_name,
        current_file,
        current_dir
    ));
    for node in &nodes {
        // dont return like Label->label
        if node.name != func_name && node.name.to_lowercase() == func_name.to_lowercase() {
            return None;
        }
        if !node.body.is_empty() {
            if let Some(node_dir) = std::path::Path::new(&node.file)
                .parent()
                .and_then(|p| p.to_str())
            {
                if node_dir == current_dir && !node.file.contains("mock") {
                    let is_same = node.start == source_start && node.file == current_file;
                    if !is_same {
                        log_cmd(format!(
                            "::: found function in same directory! file: {:?}",
                            current_file
                        ));
                        same_dir_files.push(node);
                    }
                }
            }
        }
    }

    if same_dir_files.len() == 1 {
        log_cmd(format!(
            "::: found function in same directory: {:?}",
            current_dir
        ));
        return Some(same_dir_files[0].clone());
    }

    None
}

fn log_cmd(cmd: String) {
    // if cmd.contains("src/components/designer/bitcoin/BitcoinDetails.tsx") {
    //     tracing::info!("{}", cmd);
    // }
    tracing::debug!("{}", cmd);
}

fn _find_function_files<G: Graph>(func_name: &str, graph: &G) -> Vec<String> {
    let mut target_files = Vec::new();
    let function_nodes = graph.find_nodes_by_name(NodeType::Function, func_name);
    for node in function_nodes {
        if !node.body.is_empty() {
            target_files.push(node.file.clone());
        }
    }
    target_files
}

fn _pick_target_file_from_graph<G: Graph>(target_name: &str, graph: &G) -> Option<String> {
    let mut target_file = None;
    let function_nodes = graph.find_nodes_by_name(NodeType::Function, target_name);
    for node in function_nodes {
        target_file = Some(node.file.clone());
        break;
    }

    target_file
}
