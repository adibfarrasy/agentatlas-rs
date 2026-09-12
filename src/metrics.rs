use tree_sitter::Node;

/// Cyclomatic complexity: cx = 1 + decision points in a function/method body.
/// The decision node kinds mirror ripwire's isDecisionType for the Go + Java grammars.
pub fn complexity_of(body: Node) -> u32 {
    let mut decisions = 0;
    walk(body, &mut decisions);
    1 + decisions
}

fn is_decision(kind: &str) -> bool {
    matches!(
        kind,
        // shared / Go / Java
        "if_statement" | "if_expression" | "for_statement" | "for_range_loop" | "while_statement"
            | "while_expression" | "do_statement" | "case_statement" | "expression_case"
            | "catch_clause" | "except_clause" | "conditional_expression" | "ternary_expression"
            // Java switch/catch spellings
            | "switch_expression" | "switch_expression_arm"
    )
}

fn walk(node: Node, decisions: &mut u32) {
    if is_decision(node.kind()) {
        *decisions += 1;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk(child, decisions);
    }
}
