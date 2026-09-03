//! Exact admission for the compiler-owned `embed(value)` proof term.
//!
//! `embed` is not a machine call. Its builtin symbol is only syntax-level
//! identity; this pass proves that every occurrence has the closed unary
//! fixed-integer/address shape and lives in a proof fact, transparent
//! proposition, or canonical content-projection proof body. Later proof
//! normalization may then erase the call wrapper without granting execution,
//! provider, or selection authority.

use std::collections::HashSet;

use psi_diagnostics::Diagnostic;
use psi_numerics::bignum::BigInt;
use psi_numerics::literals::LandedIntegerType;
use psi_symbols::BuiltinFunction;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::domain::ProofFact;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use psi_typed_trees::proposition::{PropositionBody, PropositionFormula};
use psi_typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IntegerEmbedding {
    pub(crate) primitive: PrimitiveType,
    pub(crate) minimum: BigInt,
    pub(crate) maximum: BigInt,
}

pub(crate) fn validate_proof_embeddings(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    let mut proof_nodes = HashSet::new();
    for (_, fact) in program.proof_facts.iter() {
        match fact {
            ProofFact::Expression(expression) => {
                collect_expression_nodes(program, *expression, &mut proof_nodes)
            }
            ProofFact::Membership(membership) => {
                collect_expression_nodes(program, membership.value, &mut proof_nodes)
            }
            ProofFact::Proposition(application) => {
                for argument in program
                    .expression_table
                    .expression_handles(application.arguments)
                {
                    collect_expression_nodes(program, *argument, &mut proof_nodes);
                }
            }
        }
    }
    for proposition in program.propositions() {
        let PropositionBody::Transparent { proposition } = &proposition.body else {
            continue;
        };
        match proposition {
            PropositionFormula::BooleanExpression(expression) => {
                collect_expression_nodes(program, *expression, &mut proof_nodes)
            }
            PropositionFormula::Application(application) => {
                for argument in program
                    .expression_table
                    .expression_handles(application.arguments)
                {
                    collect_expression_nodes(program, *argument, &mut proof_nodes);
                }
            }
        }
    }
    for machine in program.machines() {
        if let Some(expression) =
            crate::content_projections::content_projection_body_expression(program, machine)
        {
            collect_expression_nodes(program, expression, &mut proof_nodes);
        }
    }

    for (handle, node) in program.expression_table.iter_expressions() {
        let ExpressionNode::Call(call) = node else {
            continue;
        };
        let exact = is_exact_embed_call(program, call);
        let names_embed = call.target.as_str().rsplit("::").next() == Some("embed");
        if !exact && !names_embed {
            continue;
        }
        if !exact {
            diagnostics.push(Diagnostic::error(
                "`embed` is a compiler-owned proof term; an authored, package-qualified, or same-spelled call cannot replace it",
            ));
            continue;
        }
        if !proof_nodes.contains(&(handle.arena_index(), handle.generation())) {
            diagnostics.push(Diagnostic::error(
                "compiler-owned `embed(value)` is proof-only and cannot be used in an executable value or statement",
            ));
            continue;
        }
        if call.receiver.is_valid()
            || call.arguments.len() != 1
            || !call.machine_arguments.is_empty()
            || !call.evidence_arguments.is_empty()
            || call.quotient_operation.is_some()
            || call.private_layout_operation.is_some()
        {
            diagnostics.push(Diagnostic::error(
                "compiler-owned `embed` requires exactly one receiverless value argument and no static or evidence arguments",
            ));
            continue;
        }
        let [argument] = program.expression_table.expression_handles(call.arguments) else {
            unreachable!("argument count checked above");
        };
        if integer_embedding(program, *argument).is_none() {
            diagnostics.push(Diagnostic::error(
                "compiler-owned `embed(value)` accepts only a fixed-width integer or address value",
            ));
        }
    }
}

pub(crate) fn is_exact_embed_call(program: &TypedTrees, call: &TableCallExpression) -> bool {
    !call.receiver.is_valid()
        && call.target.as_str() == BuiltinFunction::ContentEmbed.name()
        && program
            .symbols
            .builtin_function_for_symbol(call.target_symbol)
            == Some(BuiltinFunction::ContentEmbed)
}

pub(crate) fn integer_embedding(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<IntegerEmbedding> {
    let primitive = expression_primitive(program, expression)?;
    let (minimum, maximum) = primitive_range(primitive)?;
    Some(IntegerEmbedding {
        primitive,
        minimum,
        maximum,
    })
}

pub(crate) fn proof_int_type_reference(program: &TypedTrees) -> Option<TypeReferenceHandle> {
    let symbol = program
        .symbols
        .builtin_type_symbol(psi_symbols::BuiltinType::Int)?;
    (1..=program.type_reference_table.type_reference_count())
        .map(|index| TypeReferenceHandle::from_arena_index(index as u32))
        .find(|type_reference| {
            matches!(
                program.type_reference_table.type_reference(*type_reference),
                TypeReferenceNode::Named { symbol: candidate, .. } if *candidate == symbol
            )
        })
}

fn expression_primitive(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<PrimitiveType> {
    if let Some(type_reference) = expression_type_reference(program, expression) {
        return program.primitive_type_reference(type_reference);
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(literal) => Some(match literal.landing()?.landed_type {
            LandedIntegerType::I8 => PrimitiveType::I8,
            LandedIntegerType::I16 => PrimitiveType::I16,
            LandedIntegerType::I32 => PrimitiveType::I32,
            LandedIntegerType::I64 => PrimitiveType::I64,
            LandedIntegerType::U8 => PrimitiveType::U8,
            LandedIntegerType::U16 => PrimitiveType::U16,
            LandedIntegerType::U32 => PrimitiveType::U32,
            LandedIntegerType::U64 => PrimitiveType::U64,
            LandedIntegerType::Addr => PrimitiveType::Addr,
        }),
        ExpressionNode::Cast(cast) => (!cast.form.is_recast())
            .then(|| program.primitive_type_reference(cast.target_type))
            .flatten(),
        ExpressionNode::Binary(binary) => {
            let left = expression_primitive(program, binary.left);
            let right = expression_primitive(program, binary.right);
            match (left, right) {
                (Some(left), Some(right)) if left == right => Some(left),
                (Some(primitive), None) | (None, Some(primitive)) => Some(primitive),
                _ => None,
            }
        }
        ExpressionNode::Atomic(_)
        | ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Borrow(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Indexed(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::String(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::Unary(_)
        | ExpressionNode::ZeroValue(_) => None,
    }
}

fn expression_type_reference(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            crate::expression_types::named_value_type_reference(program, path)
        }
        ExpressionNode::Member(member) => {
            let by_symbol = program
                .data_definitions()
                .iter()
                .flat_map(|definition| program.data_members(definition))
                .find_map(|data_member| match data_member {
                    psi_typed_trees::data::DataMember::Field(field)
                        if member.member_symbol.is_valid()
                            && field.symbol == member.member_symbol =>
                    {
                        Some(field.type_reference)
                    }
                    _ => None,
                });
            by_symbol.or_else(|| {
                let receiver = expression_type_reference(program, member.receiver)?;
                let data = crate::places::data_definition_for_type(program, receiver)?;
                program
                    .data_members(data)
                    .iter()
                    .find_map(|data_member| match data_member {
                        psi_typed_trees::data::DataMember::Field(field)
                            if field.name.as_str() == member.member.as_str() =>
                        {
                            Some(field.type_reference)
                        }
                        _ => None,
                    })
            })
        }
        ExpressionNode::Cast(cast) if !cast.form.is_recast() => Some(cast.target_type),
        ExpressionNode::Call(call) => program
            .machines()
            .iter()
            .flat_map(|machine| program.machine_states(machine))
            .find(|state| state.symbol == call.target_symbol)
            .map(|state| state.return_type),
        ExpressionNode::Indexed(indexed) => {
            let collection = expression_type_reference(program, indexed.collection)?;
            let collection = crate::places::unwrapped_type_reference(program, collection)?;
            match program.type_reference_table.type_reference(collection) {
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => Some(*element_type),
                _ => None,
            }
        }
        ExpressionNode::Atomic(atomic) => expression_type_reference(program, atomic.value),
        ExpressionNode::Borrow(_)
        | ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Binary(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::String(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::Unary(_)
        | ExpressionNode::ZeroValue(_) => None,
    }
}

fn primitive_range(primitive: PrimitiveType) -> Option<(BigInt, BigInt)> {
    let range = match primitive {
        PrimitiveType::I8 => (i128::from(i8::MIN), i128::from(i8::MAX)),
        PrimitiveType::I16 => (i128::from(i16::MIN), i128::from(i16::MAX)),
        PrimitiveType::I32 => (i128::from(i32::MIN), i128::from(i32::MAX)),
        PrimitiveType::I64 => (i128::from(i64::MIN), i128::from(i64::MAX)),
        PrimitiveType::U8 => (0, i128::from(u8::MAX)),
        PrimitiveType::U16 => (0, i128::from(u16::MAX)),
        PrimitiveType::U32 => (0, i128::from(u32::MAX)),
        PrimitiveType::U64 | PrimitiveType::Addr => {
            return Some((BigInt::zero(), BigInt::from_u64(u64::MAX)));
        }
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => return None,
    };
    Some((BigInt::from_i128(range.0), BigInt::from_i128(range.1)))
}

fn collect_expression_nodes(
    program: &TypedTrees,
    expression: ExpressionHandle,
    nodes: &mut HashSet<(u32, u32)>,
) {
    if !expression.is_valid() || !nodes.insert((expression.arena_index(), expression.generation()))
    {
        return;
    }
    let mut recurse = |child| collect_expression_nodes(program, child, nodes);
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            recurse(atomic.value);
            recurse(atomic.result);
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in program.expression_table.expression_handles(*elements) {
                recurse(*element);
            }
        }
        ExpressionNode::Binary(binary) => {
            recurse(binary.left);
            recurse(binary.right);
        }
        ExpressionNode::Borrow(inner) => recurse(inner.target),
        ExpressionNode::Call(call) => {
            recurse(call.receiver);
            for argument in program.expression_table.expression_handles(call.arguments) {
                recurse(*argument);
            }
        }
        ExpressionNode::Cast(cast) => recurse(cast.value),
        ExpressionNode::Indexed(indexed) => {
            recurse(indexed.collection);
            recurse(indexed.index);
        }
        ExpressionNode::Member(member) => recurse(member.receiver),
        ExpressionNode::Range(range) => {
            recurse(range.start);
            recurse(range.end);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                recurse(field.value);
            }
        }
        ExpressionNode::Unary(unary) => recurse(unary.operand),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}
