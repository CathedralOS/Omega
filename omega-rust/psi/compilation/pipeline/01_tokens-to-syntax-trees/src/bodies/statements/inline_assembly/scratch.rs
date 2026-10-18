//! Body-local virtual register storage. Zero initialization reserves storage;
//! the CFG's separate definite-initialization judgment must prove every read.
//! Hidden names prevent communication between separately authored asm bodies.
use super::*;
use numerics::{arithmetic::ArithmeticDomain, literals::IntegerLiteral};
use syntax_trees::expression::{BinaryOperator, TableBinaryExpression, TableCastExpression};
use syntax_trees::statement::TableLocalData;
use syntax_trees::types::TypeReferenceNode;

pub(super) use language_core::inline_assembly::{
    ASM_SCRATCH_CLOBBERS as CLOBBERS, ASM_SCRATCH_NAMES as NAMES,
};
use language_core::inline_assembly::{ASM_SCRATCH_SLOT_COUNT, ASM_ZERO_FLAG_SLOT};

pub(super) fn name(trees: &mut SyntaxTrees, value: &str) -> ExpressionHandle {
    let member = trees
        .expressions
        .append_identifier_path_member(Identifier::generated(value));
    trees
        .expressions
        .insert(ExpressionNode::Name(HandleSpan::from_parts(member, 1)))
}

pub(super) fn unsigned_type(trees: &mut SyntaxTrees) -> syntax_trees::types::TypeReferenceHandle {
    trees
        .type_references
        .insert(TypeReferenceNode::Named(Identifier::generated("u32")))
}

fn cast(
    trees: &mut SyntaxTrees,
    value: ExpressionHandle,
    domain: ArithmeticDomain,
) -> ExpressionHandle {
    let target_type = unsigned_type(trees);
    trees
        .expressions
        .insert(ExpressionNode::Cast(TableCastExpression {
            value,
            target_type,
            target_label: HandleSpan::empty(),
            domain,
            semantic_domain: HandleSpan::empty(),
            semantic_domain_arguments: HandleSpan::empty(),
            form: Default::default(),
        }))
}

pub(super) fn subtraction(
    trees: &mut SyntaxTrees,
    destination: ExpressionHandle,
    source: ExpressionHandle,
) -> ExpressionHandle {
    let left = cast(trees, destination, ArithmeticDomain::Wrapping);
    let right = cast(trees, source, ArithmeticDomain::Wrapping);
    let result = trees
        .expressions
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left,
            operator: BinaryOperator::Subtract,
            right,
        }));
    cast(trees, result, ArithmeticDomain::Exact)
}

pub(super) fn zero_flag(trees: &mut SyntaxTrees, destination: ExpressionHandle) -> StatementHandle {
    let target = name(trees, "zf");
    let zero = trees
        .expressions
        .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
    let value = trees
        .expressions
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: destination,
            operator: BinaryOperator::Equal,
            right: zero,
        }));
    trees
        .statements
        .insert(StatementNode::Assignment(TableAssignment { target, value }))
}

pub(super) fn bind(
    trees: &mut SyntaxTrees,
    first: usize,
) -> (
    [ExpressionHandle; ASM_SCRATCH_SLOT_COUNT],
    Vec<StatementHandle>,
) {
    let mut registers = [ExpressionHandle::invalid(); ASM_SCRATCH_SLOT_COUNT];
    let mut bindings = Vec::new();
    let names: Vec<_> = trees
        .expressions
        .iter_expressions()
        .skip(first)
        .filter_map(|(handle, node)| {
            let ExpressionNode::Name(path) = node else {
                return None;
            };
            let [identifier] = trees.expressions.identifier_path_members(*path) else {
                return None;
            };
            NAMES
                .iter()
                .position(|name| !name.is_empty() && *name == identifier.as_str())
                .map(|index| (handle, index))
        })
        .collect();
    for (handle, index) in names {
        let hidden = format!("asm#scratch#{first}#{}", NAMES[index]);
        if !registers[index].is_valid() {
            registers[index] = name(trees, &hidden);
            let type_reference = if index == ASM_ZERO_FLAG_SLOT {
                trees
                    .type_references
                    .insert(TypeReferenceNode::Named(Identifier::generated("bool")))
            } else {
                unsigned_type(trees)
            };
            let initial_value = trees.expressions.insert(if index == ASM_ZERO_FLAG_SLOT {
                ExpressionNode::Boolean(false)
            } else {
                ExpressionNode::Integer(IntegerLiteral::zero())
            });
            bindings.push(
                trees
                    .statements
                    .insert(StatementNode::LocalData(TableLocalData {
                        role: Default::default(),
                        name: Identifier::generated(&hidden),
                        type_reference,
                        initial_value,
                        is_mutable: true,
                        relevance: Default::default(),
                    })),
            );
        }
        let ExpressionNode::Name(path) = trees.expressions.expression(registers[index]).clone()
        else {
            unreachable!()
        };
        trees
            .expressions
            .replace_expression(handle, ExpressionNode::Name(path));
    }
    (registers, bindings)
}
