#![forbid(unsafe_code)]

use source::SourceId;
use source_files_to_tokens::lexer::Lexer;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};
use syntax_trees::SyntaxTrees;
use tokens_to_syntax_trees::parser;

const DEPTH_FIXTURE_MARKERS: &[&str] = &[
    "fail/parser/nesting_exceeds_max_depth",
    "pass/parser/deep_nesting_within_limit",
    "pass/expressions/deep_paren_nesting",
];

#[derive(Default, Clone)]
struct MaxRecord {
    depth: usize,
    file: String,
    line: usize,
}

#[derive(Default)]
struct Aggregates {
    expr_tree: MaxRecord,
    type_ref: MaxRecord,
    binary_chain: MaxRecord,
    parse_failures: usize,
    parsed_files: usize,
}

fn is_depth_fixture(rel: &str) -> bool {
    DEPTH_FIXTURE_MARKERS.iter().any(|m| rel.contains(m))
}

fn line_number(source: &str, byte_offset: usize) -> usize {
    source[..byte_offset.min(source.len())]
        .bytes()
        .filter(|&b| b == b'\n')
        .count()
        + 1
}

enum DepthWork {
    Type(TypeReferenceHandle, usize),
    Expr(ExpressionHandle, usize),
}

fn type_children(trees: &SyntaxTrees, handle: TypeReferenceHandle, depth: usize) -> Vec<DepthWork> {
    let child = depth + 1;
    let node = trees.type_references.type_reference(handle);
    let mut out = Vec::new();
    match node {
        TypeReferenceNode::Reference { referee, .. } => out.push(DepthWork::Type(*referee, child)),
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            out.push(DepthWork::Type(*base_type, child));
            for constraint in trees.type_references.constraints(*constraints) {
                if let TypeConstraintNode::Domain(domain) = constraint {
                    for argument in trees.type_references.type_reference_handles(domain.arguments) {
                        out.push(DepthWork::Type(*argument, child));
                    }
                }
            }
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            out.push(DepthWork::Type(*element_type, child));
        }
        TypeReferenceNode::Slice { element_type } => out.push(DepthWork::Type(*element_type, child)),
        TypeReferenceNode::Generic { arguments, .. } => {
            for argument in trees.type_references.type_reference_handles(*arguments) {
                out.push(DepthWork::Type(*argument, child));
            }
        }
        TypeReferenceNode::ConstExpression(expression) => {
            out.push(DepthWork::Expr(*expression, child));
        }
        TypeReferenceNode::DynamicTrait { trait_arguments, .. } => {
            for argument in trees.type_references.type_reference_handles(*trait_arguments) {
                out.push(DepthWork::Type(*argument, child));
            }
        }
        TypeReferenceNode::Named(_) | TypeReferenceNode::Unit => {}
    }
    out
}

fn expr_children(trees: &SyntaxTrees, handle: ExpressionHandle, depth: usize) -> Vec<DepthWork> {
    let child = depth + 1;
    let node = trees.expressions.expression(handle);
    let mut out = Vec::new();
    match node {
        ExpressionNode::Binary(binary) => {
            out.push(DepthWork::Expr(binary.left, child));
            out.push(DepthWork::Expr(binary.right, child));
        }
        ExpressionNode::Unary(unary) => out.push(DepthWork::Expr(unary.operand, child)),
        ExpressionNode::Borrow(borrow) => out.push(DepthWork::Expr(borrow.target, child)),
        ExpressionNode::Cast(cast) => out.push(DepthWork::Expr(cast.value, child)),
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                out.push(DepthWork::Expr(call.receiver, child));
            }
            for argument in trees.expressions.expression_handles(call.arguments) {
                out.push(DepthWork::Expr(*argument, child));
            }
        }
        ExpressionNode::Indexed(indexed) => {
            out.push(DepthWork::Expr(indexed.collection, child));
            out.push(DepthWork::Expr(indexed.index, child));
        }
        ExpressionNode::Member(member) => out.push(DepthWork::Expr(member.receiver, child)),
        ExpressionNode::Range(range) => {
            out.push(DepthWork::Expr(range.start, child));
            out.push(DepthWork::Expr(range.end, child));
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in trees.expressions.struct_fields(literal.fields) {
                out.push(DepthWork::Expr(field.value, child));
            }
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in trees.expressions.expression_handles(*elements) {
                out.push(DepthWork::Expr(*element, child));
            }
        }
        ExpressionNode::Match(dispatch) => {
            if dispatch.subject.is_valid() {
                out.push(DepthWork::Expr(dispatch.subject, child));
            }
            for arm in trees.expressions.match_arms(dispatch.arms) {
                if arm.value.is_valid() {
                    out.push(DepthWork::Expr(arm.value, child));
                }
            }
        }
        ExpressionNode::Membership(m) => out.push(DepthWork::Expr(m.value, child)),
        ExpressionNode::Atomic(atomic) => {
            out.push(DepthWork::Expr(atomic.value, child));
            out.push(DepthWork::Expr(atomic.result, child));
        }
        ExpressionNode::TypeExpression(reference) | ExpressionNode::ZeroValue(reference) => {
            out.push(DepthWork::Type(*reference, child));
        }
        ExpressionNode::SavedSubject(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => {}
    }
    out
}

fn max_depth_from(seed: DepthWork, trees: &SyntaxTrees) -> usize {
    let mut stack = vec![seed];
    let mut on_path = HashSet::new();
    let mut best = 0usize;
    while let Some(work) = stack.pop() {
        let (key, children) = match work {
            DepthWork::Type(handle, depth) => {
                if !handle.is_valid() {
                    continue;
                }
                let key = (false, handle.arena_index());
                if !on_path.insert(key) {
                    continue;
                }
                best = best.max(depth);
                (key, type_children(trees, handle, depth))
            }
            DepthWork::Expr(handle, depth) => {
                if !handle.is_valid() {
                    continue;
                }
                let key = (true, handle.arena_index());
                if !on_path.insert(key) {
                    continue;
                }
                best = best.max(depth);
                (key, expr_children(trees, handle, depth))
            }
        };
        for child in children.into_iter().rev() {
            stack.push(child);
        }
        on_path.remove(&key);
    }
    best
}

fn binary_chain_length(trees: &SyntaxTrees, mut handle: ExpressionHandle) -> usize {
    let mut length = 0usize;
    while handle.is_valid() {
        let ExpressionNode::Binary(binary) = trees.expressions.expression(handle) else {
            break;
        };
        length += 1;
        handle = binary.left;
    }
    length
}

fn measure_trees(source: &str, trees: &SyntaxTrees) -> (usize, usize, usize, usize, usize) {
    let mut max_type = 0usize;
    let type_count = trees.type_references.type_reference_count();
    for i in 1..=type_count as u32 {
        let handle = TypeReferenceHandle::from_arena_index(i);
        if !handle.is_valid() {
            continue;
        }
        max_type = max_type.max(max_depth_from(DepthWork::Type(handle, 0), trees));
    }

    let mut max_expr_tree = 0usize;
    let mut max_bin = 0usize;
    let mut max_bin_line = 1usize;
    for (handle, _) in trees.expressions.iter_expressions() {
        max_expr_tree = max_expr_tree.max(max_depth_from(DepthWork::Expr(handle, 0), trees));
        let chain = binary_chain_length(trees, handle);
        if chain > max_bin {
            max_bin = chain;
            let span = trees.expressions.source_span(handle);
            max_bin_line = line_number(source, span.span.start);
        }
    }
    (max_expr_tree, max_type, max_bin, max_bin_line, trees.expressions.expression_count())
}

fn parse_file(source: &str) -> Option<SyntaxTrees> {
    let source_id = SourceId::default();
    let tokens = Lexer::new(source).tokenize().ok()?;
    let mut trees = SyntaxTrees::new(source_id);
    parser::parse(&mut trees, source_id, &tokens).ok()?;
    Some(trees)
}

fn update_max(record: &mut MaxRecord, depth: usize, rel: &str, line: usize) {
    if depth > record.depth {
        record.depth = depth;
        record.file = rel.to_owned();
        record.line = line;
    }
}

fn main() -> std::io::Result<()> {
    let repo = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/agent/repos/omega"));
    let roots = [
        repo.join("tests/omega"),
        repo.join("samples"),
        repo.join("source/library"),
        repo.join("source/psi"),
    ];

    let mut all = Aggregates::default();
    let mut excl = Aggregates::default();

    for root in roots {
        if !root.is_dir() {
            continue;
        }
        for entry in walkdir::WalkDir::new(&root)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|x| x == "omg"))
        {
            let path = entry.path();
            let rel = path
                .strip_prefix(&repo)
                .unwrap_or(path)
                .to_string_lossy()
                .into_owned();
            let source = match fs::read_to_string(path) {
                Ok(text) => text,
                Err(_) => continue,
            };
            let Some(trees) = parse_file(&source) else {
                all.parse_failures += 1;
                excl.parse_failures += 1;
                continue;
            };
            all.parsed_files += 1;
            let (expr_d, type_d, bin_d, bin_line, _) = measure_trees(&source, &trees);

            let apply = |agg: &mut Aggregates| {
                update_max(&mut agg.expr_tree, expr_d, &rel, 1);
                update_max(&mut agg.type_ref, type_d, &rel, 1);
                if bin_d > agg.binary_chain.depth {
                    agg.binary_chain.depth = bin_d;
                    agg.binary_chain.file = rel.clone();
                    agg.binary_chain.line = bin_line;
                }
            };
            apply(&mut all);
            if !is_depth_fixture(&rel) {
                excl.parsed_files += 1;
                apply(&mut excl);
            }
        }
    }

    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    writeln!(
        lock,
        "{{\n  \"engine\": \"tokens-to-syntax-trees (Psi 01)\",\n  \
         \"parsed_files\": {},\n  \"parse_failures\": {},\n  \
         \"deep_paren_nesting_comment\": \"Deep parenthesis nesting within the compile path's recursion budget.\",\n  \
         \"fixture_paths\": {:?},\n  \
         \"all\": {{\n    \"expression_tree_depth\": {{\"depth\": {}, \"file\": {:?}, \"line\": {}}},\n    \
         \"type_reference_nesting\": {{\"depth\": {}, \"file\": {:?}, \"line\": {}}},\n    \
         \"binary_operator_chain\": {{\"depth\": {}, \"file\": {:?}, \"line\": {}}}\n  }},\n  \
         \"excluding_depth_fixtures\": {{\n    \"expression_tree_depth\": {{\"depth\": {}, \"file\": {:?}, \"line\": {}}},\n    \
         \"type_reference_nesting\": {{\"depth\": {}, \"file\": {:?}, \"line\": {}}},\n    \
         \"binary_operator_chain\": {{\"depth\": {}, \"file\": {:?}, \"line\": {}}}\n  }}\n}}",
        all.parsed_files,
        all.parse_failures,
        DEPTH_FIXTURE_MARKERS,
        all.expr_tree.depth,
        all.expr_tree.file,
        all.expr_tree.line,
        all.type_ref.depth,
        all.type_ref.file,
        all.type_ref.line,
        all.binary_chain.depth,
        all.binary_chain.file,
        all.binary_chain.line,
        excl.expr_tree.depth,
        excl.expr_tree.file,
        excl.expr_tree.line,
        excl.type_ref.depth,
        excl.type_ref.file,
        excl.type_ref.line,
        excl.binary_chain.depth,
        excl.binary_chain.file,
        excl.binary_chain.line,
    )?;
    Ok(())
}
