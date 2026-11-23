---cargo
[package]
edition = "2024"
[dependencies]
omega-lexer = { path = "../../../omega-rust-v2/layers/00_bytes-to-tokens" }
omega-parser = { path = "../../../omega-rust-v2/layers/01_tokens-to-syntax-trees" }
---
use omega_parser::{NodeKind, TreeEvent};
use std::{env, fs};

#[derive(Debug, Default)]
struct Count {
    roots: usize,
    states: usize,
    path_members: usize,
    typed_parameters: usize,
    type_nodes: usize,
    statements: usize,
    expressions: usize,
    complete_units: usize,
    split_units: usize,
    refused_fragments: usize,
}

impl Count {
    fn open_node(&mut self, kind: NodeKind, parents: &[NodeKind]) {
        use NodeKind::*;
        if parents == [File] && matches!(kind, Use | Machine | Data) {
            self.roots += 1;
            if kind == Machine {
                self.path_members += 1;
            }
        }
        if parents.get(1) != Some(&Machine) {
            return;
        }
        if kind == Param && parents.ends_with(&[Machine, Params]) {
            self.typed_parameters += 1;
            self.type_nodes += 1;
        }
        // Each ordinary wrapper owns a type row in addition to the parameter's base.
        if matches!(kind, ReferenceType | ArrayType | SliceType | QualifiedType)
            && parents.windows(3).any(|p| p == [Machine, Params, Param])
            && !parents
                .iter()
                .any(|n| matches!(n, TypeArgs | MachineType | FunctionType))
        {
            self.type_nodes += 1;
        }
        let Some(body) = parents.iter().rposition(|&n| n == Body) else {
            return;
        };
        let inside = &parents[body + 1..];
        if kind == Body && inside.last() == Some(&Machine) {
            self.states += 1;
        }
        if matches!(kind, Let | Assign) || (kind == Arm && inside.last() == Some(&Transition)) {
            self.statements += 1;
        }
        // Count ordinary expression nodes in bodies. Omit patterns, nested
        // signatures and type-level expressions even when these contain operators.
        if !inside
            .iter()
            .any(|n| matches!(n, Let | Assign | Transition | Tail | ExpressionStatement))
            || inside.iter().any(|n| {
                matches!(
                    n,
                    Machine
                        | ArrayType
                        | TypeArgs
                        | ValuePattern
                        | FieldPattern
                        | CaseField
                        | MathLet
                )
            })
        {
            return;
        }
        if matches!(kind, Binary | Unary | Index | Cast) {
            self.expressions += 1;
        }
        if matches!(inside.last(), Some(Binary | Unary | Index))
            && matches!(kind, PathExpression | Literal)
        {
            self.expressions += 1;
            if kind == PathExpression {
                self.path_members += 1;
            }
        }
    }
}

// A refused tree contributes zero rows.
fn count_tree(source: &[u8], count: &mut Count) -> bool {
    let tokens = omega_lexer::lex(source).expect("unit must lex");
    let Ok(tree) = omega_parser::parse_file(source, &tokens) else {
        return false;
    };
    let mut indices = tree.get_meaningful_token_indices().iter();
    let mut stack = Vec::new();
    for event in tree.get_events() {
        match *event {
            TreeEvent::Open(kind) => {
                count.open_node(kind, &stack);
                stack.push(kind);
            }
            TreeEvent::Close => {
                stack.pop().unwrap();
            }
            TreeEvent::Token => {
                let span = tokens[*indices.next().unwrap() as usize].get_span();
                let spelling = &source[span.get_start_offset()..span.get_end_offset()];
                if stack == [NodeKind::File, NodeKind::Use, NodeKind::Path]
                    && !matches!(spelling, b"\\" | b"::")
                {
                    count.path_members += 1;
                }
            }
        }
    }
    true
}

// A syntax error elsewhere in a unit need not discard its complete declarations.
// Split only on top-level delimiters selected from real tokens, so braces inside
// quotes and comments never open or close a fragment.
fn count_unit(source: &[u8], count: &mut Count) {
    if count_tree(source, count) {
        count.complete_units += 1;
        return;
    }
    count.split_units += 1;
    let mut depth = 0i32;
    let mut start = 0;
    for token in omega_lexer::lex(source).unwrap() {
        let span = token.get_span();
        let end = span.get_end_offset();
        let spelling = &source[span.get_start_offset()..end];
        match spelling {
            b"(" | b"[" | b"{" => depth += 1,
            b")" | b"]" | b"}" => depth -= 1,
            _ => {}
        }
        assert!(depth >= 0, "unbalanced unit");
        if depth == 0 && matches!(spelling, b";" | b"}") {
            if !count_tree(&source[start..end], count) {
                count.refused_fragments += 1;
            }
            start = end;
        }
    }
    assert_eq!(depth, 0, "unbalanced unit");
    if start < source.len() && !count_tree(&source[start..], count) {
        count.refused_fragments += 1;
    }
}

// Read the path-bearing product written by source/application/construction/construct.py.
fn main() {
    let input = fs::read(env::args_os().nth(1).expect("provide product.bin")).unwrap();
    let word = |i: usize| u32::from_le_bytes(input[4 * i..4 * i + 4].try_into().unwrap()) as usize;
    assert_eq!(
        (word(0), word(1)),
        (0, 1),
        "expected path-bearing construction input"
    );
    let (packages, edges, units) = (word(2), word(3), word(4));
    assert!(packages <= 64 && edges <= 256 && units <= 512);
    let first = 9 + 3 * packages + 4 * edges;
    let arena = &input[4 * (first + 5 * units)..];
    assert_eq!(arena.len(), word(8));
    let mut count = Count::default();
    for i in 0..units {
        let row = first + 5 * i;
        count_unit(&arena[word(row + 1)..word(row + 2)], &mut count);
    }
    println!("{count:#?}");
}
