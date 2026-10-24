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
    modular_binary(trees, destination, source, BinaryOperator::Subtract)
}

pub(super) fn addition(
    trees: &mut SyntaxTrees,
    destination: ExpressionHandle,
    source: ExpressionHandle,
) -> ExpressionHandle {
    modular_binary(trees, destination, source, BinaryOperator::Add)
}

/// Which modular u32 update produced the arithmetic flags. Carry and signed
/// overflow follow that producer's rule. Parity, auxiliary carry, zero and
/// sign follow the wrapped result for both.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ArithmeticFlagProducer {
    Subtract,
    Add,
}

fn modular_binary(
    trees: &mut SyntaxTrees,
    destination: ExpressionHandle,
    source: ExpressionHandle,
    operator: BinaryOperator,
) -> ExpressionHandle {
    let left = cast(trees, destination, ArithmeticDomain::Wrapping);
    let right = cast(trees, source, ArithmeticDomain::Wrapping);
    let result = trees
        .expressions
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left,
            operator,
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
                .position(|name| !name.is_empty() && *name == identifier.as_str(trees))
                .map(|index| (handle, index))
        })
        .collect();
    for (handle, index) in names {
        let hidden = format!("asm#scratch#{first}#{}", NAMES[index]);
        if !registers[index].is_valid() {
            registers[index] = name(trees, &hidden);
            let type_reference = if [15, 16, 19, 20].contains(&index) {
                wide_type(trees)
            } else if index == ASM_ZERO_FLAG_SLOT {
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

// PUSHFQ observes the same arithmetic versions as label proofs, not incidental
// flags from realizing ordinary Omega expressions. The environment image is
// captured at each authored observation; SUB and ADD replace CF/PF/AF/ZF/SF/OF
// (mask 0x8d5). Pre-write operand snapshots also make `sub eax, eax` and
// `add eax, eax` independent of SSA order. ADD reuses the subtraction operand
// slots: each flag update reads the version written immediately before it.
pub(super) fn wide_type(trees: &mut SyntaxTrees) -> syntax_trees::types::TypeReferenceHandle {
    trees
        .type_references
        .insert(TypeReferenceNode::Named(Identifier::generated("u64")))
}
fn literal(trees: &mut SyntaxTrees, value: i64) -> ExpressionHandle {
    trees
        .expressions
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(value)))
}
fn wide_literal(trees: &mut SyntaxTrees, value: i64) -> ExpressionHandle {
    let value = literal(trees, value);
    let target_type = wide_type(trees);
    trees
        .expressions
        .insert(ExpressionNode::Cast(TableCastExpression {
            value,
            target_type,
            target_label: HandleSpan::empty(),
            domain: ArithmeticDomain::Exact,
            semantic_domain: HandleSpan::empty(),
            semantic_domain_arguments: HandleSpan::empty(),
            form: Default::default(),
        }))
}
fn binary(
    trees: &mut SyntaxTrees,
    left: ExpressionHandle,
    operator: BinaryOperator,
    right: ExpressionHandle,
) -> ExpressionHandle {
    trees
        .expressions
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left,
            operator,
            right,
        }))
}
fn write(trees: &mut SyntaxTrees, register: &str, value: ExpressionHandle) -> StatementHandle {
    let target = name(trees, register);
    trees
        .statements
        .insert(StatementNode::Assignment(TableAssignment { target, value }))
}
fn reg_binary(
    trees: &mut SyntaxTrees,
    left: &str,
    operator: BinaryOperator,
    right: &str,
) -> ExpressionHandle {
    let left = name(trees, left);
    let right = name(trees, right);
    binary(trees, left, operator, right)
}
pub(super) fn preserve_sub_operands(
    trees: &mut SyntaxTrees,
    destination: ExpressionHandle,
    source: ExpressionHandle,
) -> Vec<StatementHandle> {
    let destination = trees.expressions.expression(destination).clone();
    let destination = trees.expressions.insert(destination);
    let source = trees.expressions.expression(source).clone();
    let source = trees.expressions.insert(source);
    vec![
        write(trees, "asm#sub_left", destination),
        write(trees, "asm#sub_right", source),
    ]
}
fn wide_cast(trees: &mut SyntaxTrees, value: ExpressionHandle) -> ExpressionHandle {
    let target_type = wide_type(trees);
    trees
        .expressions
        .insert(ExpressionNode::Cast(TableCastExpression {
            value,
            target_type,
            target_label: HandleSpan::empty(),
            domain: ArithmeticDomain::Exact,
            semantic_domain: HandleSpan::empty(),
            semantic_domain_arguments: HandleSpan::empty(),
            form: Default::default(),
        }))
}
fn masked_shift(
    trees: &mut SyntaxTrees,
    value: ExpressionHandle,
    mask: i64,
    shift: i64,
) -> ExpressionHandle {
    let mask = literal(trees, mask);
    let value = binary(trees, value, BinaryOperator::BitwiseAnd, mask);
    let shift = literal(trees, shift);
    binary(trees, value, BinaryOperator::ShiftRight, shift)
}
pub(super) fn arithmetic_flags(
    trees: &mut SyntaxTrees,
    destination: ExpressionHandle,
    producer: ArithmeticFlagProducer,
) -> StatementHandle {
    use BinaryOperator::*;
    // Use width-total bit calculations, not a Boolean-to-integer admission.
    // Subtraction carry is the complement of bit 32 of lhs + 2^32 - rhs.
    // Addition carry is bit 32 of the unsigned sum. Widening the saved u32
    // inputs makes every intermediate representable and nonnegative.
    let cf = match producer {
        ArithmeticFlagProducer::Subtract => {
            let left = name(trees, "asm#sub_left");
            let left = wide_cast(trees, left);
            let right = name(trees, "asm#sub_right");
            let right = wide_cast(trees, right);
            let base = wide_literal(trees, 4294967296);
            let sum = binary(trees, left, Add, base);
            let difference = binary(trees, sum, Subtract, right);
            let count = wide_literal(trees, 32);
            let high = binary(trees, difference, ShiftRight, count);
            let one = wide_literal(trees, 1);
            binary(trees, high, BitwiseXor, one)
        }
        ArithmeticFlagProducer::Add => {
            let left = name(trees, "asm#sub_left");
            let left = wide_cast(trees, left);
            let right = name(trees, "asm#sub_right");
            let right = wide_cast(trees, right);
            let sum = binary(trees, left, Add, right);
            let count = wide_literal(trees, 32);
            binary(trees, sum, ShiftRight, count)
        }
    };
    // Signed overflow: subtraction uses (left XOR right) AND (left XOR result).
    // Addition uses (left XOR result) AND (right XOR result). Bit 31 moves to OF.
    let of = match producer {
        ArithmeticFlagProducer::Subtract => {
            let left = name(trees, "asm#sub_left");
            let difference = binary(trees, left, BitwiseXor, destination);
            let operands = reg_binary(trees, "asm#sub_left", BitwiseXor, "asm#sub_right");
            let overflow = binary(trees, operands, BitwiseAnd, difference);
            masked_shift(trees, overflow, 0x80000000, 20)
        }
        ArithmeticFlagProducer::Add => {
            let left = name(trees, "asm#sub_left");
            let left_difference = binary(trees, left, BitwiseXor, destination);
            let right = name(trees, "asm#sub_right");
            let right_difference = binary(trees, right, BitwiseXor, destination);
            let overflow = binary(trees, left_difference, BitwiseAnd, right_difference);
            masked_shift(trees, overflow, 0x80000000, 20)
        }
    };
    let operands = reg_binary(trees, "asm#sub_left", BitwiseXor, "asm#sub_right");
    let af = binary(trees, operands, BitwiseXor, destination);
    let af = masked_shift(trees, af, 16, 0);
    let sf = masked_shift(trees, destination, 0x80000000, 24);
    // For a nonzero u32, x | (-x mod 2^32) has bit 31 set. Toggle that
    // bit and project it to RFLAGS.ZF; no exact negation or overflow is owed.
    let zero = literal(trees, 0);
    let negated = subtraction(trees, zero, destination);
    let nonzero = binary(trees, destination, BitwiseOr, negated);
    let sign = literal(trees, 0x80000000);
    let inverted = binary(trees, nonzero, BitwiseXor, sign);
    let zf = masked_shift(trees, inverted, 0x80000000, 25);
    // Fold the low byte to a nibble and query the even-parity bit table.
    let four = literal(trees, 4);
    let upper = binary(trees, destination, ShiftRight, four);
    let folded = binary(trees, destination, BitwiseXor, upper);
    let fifteen = literal(trees, 15);
    let nibble = binary(trees, folded, BitwiseAnd, fifteen);
    let table = literal(trees, 0x9669);
    let table = cast(trees, table, ArithmeticDomain::Exact);
    let parity = binary(trees, table, ShiftRight, nibble);
    let mask = literal(trees, 1);
    let parity = binary(trees, parity, BitwiseAnd, mask);
    let count = literal(trees, 2);
    let pf = binary(trees, parity, ShiftLeft, count);
    let mut value = cf;
    for bits in [pf, af, zf, sf, of] {
        let bits = wide_cast(trees, bits);
        value = binary(trees, value, BitwiseOr, bits);
    }
    write(trees, "asm#flags", value)
}
/// Private bookkeeping has no architectural input and is initialized only
/// on body entry. A zero replacement mask preserves the first raw image.
pub(super) fn initialize_flags(trees: &mut SyntaxTrees) -> Vec<StatementHandle> {
    let zero = wide_literal(trees, 0);
    let flags = write(trees, "asm#flags", zero);
    let zero = wide_literal(trees, 0);
    vec![flags, write(trees, "asm#flags_mask", zero)]
}
pub(super) fn replace_arithmetic_flags(trees: &mut SyntaxTrees) -> StatementHandle {
    let mask = wide_literal(trees, 0x8d5);
    write(trees, "asm#flags_mask", mask)
}
pub(super) fn observe_environment(
    trees: &mut SyntaxTrees,
    raw: ExpressionHandle,
) -> StatementHandle {
    write(trees, "asm#flags_environment", raw)
}
pub(super) fn snapshot(trees: &mut SyntaxTrees) -> Vec<StatementHandle> {
    let all = trees.expressions.insert(ExpressionNode::Integer(
        IntegerLiteral::from_parts(
            false,
            numerics::literals::IntegerRadix::Hexadecimal,
            "ffffffffffffffff",
        )
        .expect("compiler-owned flag mask"),
    ));
    let target_type = wide_type(trees);
    let all = trees
        .expressions
        .insert(ExpressionNode::Cast(TableCastExpression {
            value: all,
            target_type,
            target_label: HandleSpan::empty(),
            domain: ArithmeticDomain::Exact,
            semantic_domain: HandleSpan::empty(),
            semantic_domain_arguments: HandleSpan::empty(),
            form: Default::default(),
        }));
    let replacement = name(trees, "asm#flags_mask");
    let mask = binary(trees, all, BinaryOperator::BitwiseXor, replacement);
    let environment = name(trees, "asm#flags_environment");
    let persistent = binary(trees, environment, BinaryOperator::BitwiseAnd, mask);
    let flags = name(trees, "asm#flags");
    let image = binary(trees, persistent, BinaryOperator::BitwiseOr, flags);
    let image = write(trees, "asm#flags_snapshot", image);
    // Capture before writing any authored place, and never replay its index or
    // projection while retaining the new arithmetic version.
    let snapshot = name(trees, "asm#flags_snapshot");
    let mask = wide_literal(trees, 0x8d5);
    let flags = binary(trees, snapshot, BinaryOperator::BitwiseAnd, mask);
    let flags = write(trees, "asm#flags", flags);
    let mask = replace_arithmetic_flags(trees);
    vec![image, flags, mask]
}
pub(super) fn observed_zero_flag(trees: &mut SyntaxTrees) -> StatementHandle {
    let flags = name(trees, "asm#flags");
    let mask = wide_literal(trees, 64);
    let value = binary(trees, flags, BinaryOperator::BitwiseAnd, mask);
    let zero = wide_literal(trees, 0);
    let value = binary(trees, value, BinaryOperator::NotEqual, zero);
    write(trees, "zf", value)
}
