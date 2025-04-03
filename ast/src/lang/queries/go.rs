use super::super::*;
use super::consts::*;
use anyhow::{Context, Result};
use lsp::{Cmd as LspCmd, CmdSender, Position, Res as LspRes};
use tree_sitter::{Language, Parser, Query, Tree};
#[derive(Clone, Debug)]
pub struct Go(Language);

impl Go {
    pub fn new() -> Self {
        Go(tree_sitter_go::LANGUAGE.into())
    }
}

impl Stack for Go {
    fn q(&self, q: &str, nt: &NodeType) -> Query {
        if matches!(nt, NodeType::Library) {
            Query::new(&tree_sitter_bash::LANGUAGE.into(), q).unwrap()
        } else {
            Query::new(&self.0, q).unwrap()
        }
    }
    fn parse(&self, code: &str, nt: &NodeType) -> Result<Tree> {
        let mut parser = Parser::new();
        if matches!(nt, NodeType::Library) {
            parser.set_language(&tree_sitter_bash::LANGUAGE.into())?;
        } else {
            parser.set_language(&self.0)?;
        }
        Ok(parser.parse(code, None).context("failed to parse")?)
    }
    fn lib_query(&self) -> Option<String> {
        Some(format!(
            r#"(command
  name: (command_name) @require (#eq? @require "require")
  (subshell
    (command
      name: (command_name) @{LIBRARY_NAME}
      argument: (word) @{LIBRARY_VERSION}
    )
  )
) @{LIBRARY}"#
        ))
    }
    fn module_query(&self) -> Option<String> {
        Some(format!("(package_identifier) @{MODULE_NAME}"))
    }
    fn imports_query(&self) -> Option<String> {
        Some(format!(
            "(source_file
    (import_declaration)+ @{IMPORTS}
)"
        ))
    }
    fn trait_query(&self) -> Option<String> {
        Some(format!(
            r#"(type_declaration
    (type_spec
        name: (type_identifier) @{TRAIT_NAME}
        type: (interface_type)
    )
) @{TRAIT}"#
        ))
    }
    // FIXME for go this just gets every struct. Filter them out later. If class has no methods, delete it.
    fn class_definition_query(&self) -> String {
        format!(
            "(type_spec
    name: (type_identifier) @{CLASS_NAME}
) @{CLASS_DEFINITION}"
        )
    }
    fn instance_definition_query(&self) -> Option<String> {
        Some(format!(
            "(source_file
    (var_declaration
    	(var_spec
        	name: (identifier) @{INSTANCE_NAME}
            type: (type_identifier) @{CLASS_NAME}
        )
    ) @{INSTANCE}
)"
        ))
    }
    fn function_definition_query(&self) -> String {
        let return_type = format!(r#"result: (_)? @{RETURN_TYPES}"#);
        format!(
            "[
    (function_declaration
        name: (identifier) @{FUNCTION_NAME}
        parameters: (parameter_list) @{ARGUMENTS}
        {return_type}
    )
    (method_declaration
        receiver: (parameter_list
            (parameter_declaration
                name: (identifier) @{PARENT_NAME}
                type: [
                    (type_identifier)
                    (pointer_type) (type_identifier)
                ] @{PARENT_TYPE}
            )
        )
        name: (field_identifier) @{FUNCTION_NAME}
        parameters: (parameter_list) @{ARGUMENTS}
        {return_type}
    )
] @{FUNCTION_DEFINITION}"
        )
    }
    fn function_call_query(&self) -> String {
        format!(
            "(call_expression
    function: [
    	(identifier) @{FUNCTION_NAME}
        (selector_expression
            operand: [
                (identifier) @{OPERAND}
            	(selector_expression) @{OPERAND}
                (call_expression)
            ]
            field: (field_identifier) @{FUNCTION_NAME}
        )
    ]
    arguments: (argument_list) @{ARGUMENTS}
) @{FUNCTION_CALL}"
        )
    }
    //     fn endpoint_handler_queries(&self) -> Vec<String> {
    //         let q1 = r#"("func"
    //     parameters: (parameter_list
    //         (parameter_declaration
    //             type: (qualified_type) @res (#eq? @res "http.ResponseWriter")
    //         )
    //         (parameter_declaration
    //             type: (pointer_type) @req (#eq? @req "*http.Request")
    //         )
    //     )
    // )"#;
    //         vec![q1.to_string()]
    //     }
    fn endpoint_finders(&self) -> Vec<String> {
        vec![format!(
            r#"(call_expression
    function: (selector_expression
        operand: (identifier)
        field: (field_identifier) @{ENDPOINT_VERB} (#match? @{ENDPOINT_VERB} "^Get$|^Post$|^Put$|^Delete$")
    )
    arguments: (argument_list
        (interpreted_string_literal) @{ENDPOINT}
        [
            (selector_expression
                field: (field_identifier) @{HANDLER}
            )
            (identifier) @{HANDLER}
        ]
    )
) @{ROUTE}"#
        )]
    }
    fn endpoint_group_find(&self) -> Option<String> {
        Some(format!(
            r#"(call_expression
            function: (selector_expression
                operand: (identifier)
                field: (field_identifier) @{ENDPOINT_VERB} (#match? @{ENDPOINT_VERB} "Mount")
            )
            arguments: (argument_list
                (interpreted_string_literal) @{ENDPOINT}
                (call_expression
                    function: (identifier) @{ENDPOINT_GROUP}
                )
            )
        ) @{ROUTE}"#
        ))
    }

    //     fn data_model_query(&self) -> Option<String> {
    //         Some(format!(
    //             "(type_declaration
    //     (type_spec
    //     	name: (type_identifier) @{STRUCT_NAME}
    //         type: (struct_type
    //         	(field_declaration_list
    //             	(field_declaration
    //                 	tag: (_)
    //                 )
    //             )
    //         )
    //     )
    // ) @{STRUCT}"
    //         ))
    //     }
    fn data_model_query(&self) -> Option<String> {
        Some(format!(
            "(type_declaration
    (type_spec
        name: (type_identifier) @{STRUCT_NAME}
        type: (_)
    )
) @{STRUCT}"
        ))
    }
    fn data_model_within_query(&self) -> Option<String> {
        // the surrounding () is required to match the match work
        let type_finder = format!(
            r#"(
    (type_identifier) @{STRUCT_NAME} (#match? @{STRUCT_NAME} "^[A-Z].*")
)"#
        );
        Some(type_finder)
    }
    fn is_test(&self, func_name: &str, _func_file: &str) -> bool {
        func_name.starts_with("Test")
    }
    fn is_test_file(&self, filename: &str) -> bool {
        filename.ends_with("_test.go")
    }
    fn integration_test_query(&self) -> Option<String> {
        Some(format!(
            r#"(call_expression
    function: (selector_expression) @hff (#eq? @hff "http.HandlerFunc")
    arguments: (argument_list
        (selector_expression
            operand: (identifier)
            field: (field_identifier) @{HANDLER}
        )
    )
)"#
        ))
    }
    // in Go a Class is really just a struct
    fn clean_graph(&self, nodes: &mut Vec<Node>) -> bool {
        filter_out_classes_without_methods(nodes)
    }
    fn find_function_parent(
        &self,
        _node: TreeNode,
        _code: &str,
        file: &str,
        func_name: &str,
        nodes: &[Node],
        parent_type: Option<&str>,
    ) -> Result<Option<Operand>> {
        if parent_type.is_none() {
            return Ok(None);
        }
        let parent_type = parent_type.unwrap();
        let parent_class = nodes.iter().find(
            |n| matches!(n, Node::Class(c) if c.name.contains(parent_type) && c.file == file),
        );
        Ok(match parent_class {
            Some(class) => Some(Operand {
                source: NodeKeys::new(&class.into_data().name, &class.into_data().file),
                target: NodeKeys::new(func_name, file),
            }),
            None => None,
        })
    }
    fn find_trait_operand(
        &self,
        pos: Position,
        nd: &NodeData,
        nodes: &[Node],
        lsp_tx: &Option<CmdSender>,
    ) -> Result<Option<Edge>> {
        if let Some(lsp) = lsp_tx {
            let res = LspCmd::GotoImplementations(pos.clone()).send(&lsp)?;
            if let LspRes::GotoImplementations(Some(imp)) = res {
                let tr = nodes.iter().find(|n| {
                    matches!(n, Node::Trait(t) if
                        t.start as u32 <= imp.line &&
                        t.end as u32 >= imp.line
                    )
                });

                if let Some(tr) = tr {
                    let edge = Edge::trait_operand(&tr.into_data(), nd);
                    return Ok(Some(edge));
                }
            }
        }
        Ok(None)
    }
}

/*

fn endpoint_finder(&self) -> Option<String> {
        Some(format!(
            r#"(call_expression
    function: (member_expression
        object: (identifier)
        property: (property_identifier) @{ENDPOINT_VERB} (#match? @{ENDPOINT_VERB} "Get|Post|Put|Delete")
    )
    arguments: (arguments
        (string
            (string_fragment) @{ENDPOINT}
        )
        [
            (member_expression
                property: (property_identifier) @{HANDLER}
            )
            (identifier) @{HANDLER}
        ]
    )
) @{ROUTE}"#
        ))
    }

*/

/*

(function_definition
    name: (identifier) @method-name
    parameters: (parameters) @parameters)
    body: (block) @body


(class_definition
    body: (block
        (function_definition
            name: (identifier) @method-name
            parameters: (parameters) @parameters))
    @method-definition)
*/

/*

package something

import (
    "fmt"
    "testing"
)

type Thing struct {}

func (thing Thing) Init() {}

func (thing Thing) Method(arg string) {
    val := a_function(arg)
}

func (thing Thing) Method2(arg string) {
    thing.Method("hi")
}

func a_function(a: string) {
    return "return value " + a
}

func TestThing(t *testing.T) {
    thing := Thing{}
    ret := thing.Method("hi")
    if ret != "return value hi" {
        panic("bad return value"
    }
}

*/

/*

fn function_definition_query(&self) -> Query {
    self.q("
    (function_declaration
        name: (identifier) @function-name
        parameters: (parameter_list
            (parameter_declaration
                name: (identifier) @arg-name
                type: (type_identifier) @arg-type))*
    ) @function-definition
    ")
}
fn method_definition_query(&self) -> Option<Query> {
    Some(self.q("
    (method_declaration
        receiver: (parameter_list
            (parameter_declaration
                name: (identifier) @parent-name
                type: (type_identifier) @parent-type)
        )
        name: (field_identifier) @method-name
        parameters: (parameter_list
            (parameter_declaration
                name: (identifier) @arg-name
                type: (type_identifier) @arg-type))*
    ) @method-definition
    "))
}

*/

/*

(function_declaration
            name: (identifier) @function-name
            parameters: (parameter_list
                (parameter_declaration
                    name: (identifier) @arg-name
                    type: (type_identifier) @arg-type))*
        ) @function-definition

*/
