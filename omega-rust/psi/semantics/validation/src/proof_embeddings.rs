//! Admission and carrier typing for the compiler-owned proof integer embedding.
//!
//! The source expression keeps its selected arithmetic meaning. Recognizing an
//! embedding neither executes that expression nor replaces it with mathematical
//! arithmetic; proof normalization must preserve the source denotation.

use diagnostics::Diagnostic;
use numerics::bignum::BigInt;
use numerics::literals::LandedIntegerType;
use symbols::BuiltinFunction;
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableCallExpression, UnaryOperator,
};
use typed_trees::proof_only::ProofOnlyClassification;
use typed_trees::proposition::{PropositionBody, PropositionFormula};
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

mod reserved_results;
pub(crate) use reserved_results::reserved_result_owner;

mod calls;
pub use calls::ValidatedIntegerEmbeddingCall;
pub(crate) use calls::validate_integer_embedding_calls;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IntegerEmbedding {
    pub(crate) primitive: PrimitiveType,
    pub(crate) minimum: BigInt,
    pub(crate) maximum: BigInt,
}

pub(crate) fn validate_proof_embeddings(
    program: &TypedTrees,
    classification: &ProofOnlyClassification,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines() {
        if machine.name.as_str().rsplit("::").next() == Some("embed") {
            diagnostics.push(Diagnostic::error(
                "`embed` is a compiler-owned proof term and cannot be declared or implemented as a machine",
            ));
        }
    }
    // This is a transient set of full generational handles, not semantic
    // identity. A linear arena-backed collection is sufficient for this gate.
    let mut proof_nodes = Vec::new();
    let mut runtime_nodes = Vec::new();
    for (_, fact) in program.proof_facts.iter() {
        match fact {
            ProofFact::Expression(expression) => {
                collect_expression_nodes(program, *expression, &mut proof_nodes);
            }
            ProofFact::Membership(membership) => {
                collect_expression_nodes(program, membership.value, &mut proof_nodes);
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
        if let PropositionBody::Transparent { proposition } = &proposition.body {
            match proposition {
                PropositionFormula::BooleanExpression(expression) => {
                    collect_expression_nodes(program, *expression, &mut proof_nodes);
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
    }
    for machine in program.machines() {
        if classification.is_proof_machine(program, machine) {
            for state in program.machine_states(machine) {
                for statement in program.statement_table.statements(state.statement_nodes) {
                    collect_statement_nodes(program, statement, &mut proof_nodes);
                }
            }
        } else if let Some(expression) =
            crate::content_projections::content_projection_body_expression(program, machine)
        {
            collect_expression_nodes(program, expression, &mut proof_nodes);
        } else {
            for state in program.machine_states(machine) {
                for statement in program.statement_table.statements(state.statement_nodes) {
                    if let StatementNode::AssemblyFact(fact) = statement {
                        collect_expression_nodes(program, fact.expression, &mut proof_nodes);
                    } else {
                        collect_statement_nodes(program, statement, &mut runtime_nodes);
                    }
                }
            }
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
        } else if !proof_nodes.contains(&handle) || runtime_nodes.contains(&handle) {
            diagnostics.push(Diagnostic::error(
                "compiler-owned `embed(value)` is proof-only and cannot be used in an executable value or statement",
            ));
        } else if !has_embedding_shape(call) {
            diagnostics.push(Diagnostic::error(
                "compiler-owned `embed` requires exactly one receiverless value argument and no static or evidence arguments",
            ));
        } else if program
            .expression_table
            .expression_handles(call.arguments)
            .first()
            .and_then(|argument| integer_embedding(program, *argument))
            .is_none()
        {
            diagnostics.push(Diagnostic::error(
                "compiler-owned `embed(value)` accepts only a fixed-width integer or address value",
            ));
        }
    }
    // Statement calls have a separate arena representation and may not bypass
    // the expression fence by explicitly discarding the proof result.
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let StatementNode::Call(call) = statement
                    && (program
                        .symbols
                        .builtin_function_for_symbol(call.target_symbol)
                        == Some(BuiltinFunction::IntegerEmbed)
                        || call.target.as_str().rsplit("::").next() == Some("embed"))
                {
                    diagnostics.push(Diagnostic::error(
                        "compiler-owned `embed(value)` is a proof value, not an executable statement call",
                    ));
                }
            }
        }
    }
}

fn has_embedding_shape(call: &TableCallExpression) -> bool {
    !call.receiver.is_valid()
        && call.arguments.len() == 1
        && call.machine_arguments.is_empty()
        && call.evidence_arguments.is_empty()
        && call.static_requirement_dispatch.is_none()
        && call.quotient_operation.is_none()
        && call.private_layout_operation.is_none()
}

pub(crate) fn is_exact_embed_call(program: &TypedTrees, call: &TableCallExpression) -> bool {
    !call.receiver.is_valid()
        && call.target.as_str() == BuiltinFunction::IntegerEmbed.name()
        && program
            .symbols
            .builtin_function_for_symbol(call.target_symbol)
            == Some(BuiltinFunction::IntegerEmbed)
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

/// Recover the closed embedding shape and its source carrier. This is a
/// structural query; proof placement and denotational eligibility still
/// require the ordinary validation and checked termination gates.
pub fn integer_embedding_argument(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<(PrimitiveType, ExpressionHandle)> {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return None;
    };
    if !is_exact_embed_call(program, call) || !has_embedding_shape(call) {
        return None;
    }
    let [argument] = program.expression_table.expression_handles(call.arguments) else {
        return None;
    };
    Some((integer_embedding(program, *argument)?.primitive, *argument))
}

pub(crate) fn machine_contains_integer_embedding(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> bool {
    let mut nodes = Vec::new();
    for contract in program.machine_contracts(machine).iter().chain(
        program
            .machine_states(machine)
            .iter()
            .flat_map(|state| program.state_contracts(state)),
    ) {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            match fact {
                ProofFact::Expression(expression) => {
                    collect_expression_nodes(program, *expression, &mut nodes)
                }
                ProofFact::Membership(membership) => {
                    collect_expression_nodes(program, membership.value, &mut nodes)
                }
                ProofFact::Proposition(application) => {
                    for argument in program
                        .expression_table
                        .expression_handles(application.arguments)
                    {
                        collect_expression_nodes(program, *argument, &mut nodes);
                    }
                }
            }
        }
    }
    for state in program.machine_states(machine) {
        for statement in program.statement_table.statements(state.statement_nodes) {
            collect_statement_nodes(program, statement, &mut nodes);
        }
    }
    nodes.into_iter().any(|expression| {
        matches!(program.expression_table.expression(expression),
        ExpressionNode::Call(call) if is_exact_embed_call(program, call))
    })
}

pub(crate) fn proof_int_type_reference(program: &TypedTrees) -> Option<TypeReferenceHandle> {
    let symbol = program
        .symbols
        .builtin_type_symbol(symbols::BuiltinType::Int)?;
    program
        .type_reference_table
        .find_named_type_reference(symbol)
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
        ExpressionNode::Boolean(_) => Some(PrimitiveType::Bool),
        ExpressionNode::Binary(binary) => match binary.operator {
            BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
            | BinaryOperator::And
            | BinaryOperator::Or => Some(PrimitiveType::Bool),
            BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight => {
                let count_is_integer = expression_primitive(program, binary.right)
                    .is_some_and(|primitive| primitive_range(primitive).is_some())
                    || is_unlanded_integer(program, binary.right);
                count_is_integer
                    .then(|| expression_primitive(program, binary.left))
                    .flatten()
            }
            BinaryOperator::Add
            | BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Modulo
            | BinaryOperator::BitwiseAnd
            | BinaryOperator::BitwiseOr
            | BinaryOperator::BitwiseXor => {
                let left = expression_primitive(program, binary.left);
                let right = expression_primitive(program, binary.right);
                match (left, right) {
                    (Some(left), Some(right)) if left == right => Some(left),
                    // An unlanded literal is contextually typed by the other
                    // operand; an unknown computed expression is never one.
                    (Some(primitive), None) if is_unlanded_integer(program, binary.right) => {
                        Some(primitive)
                    }
                    (None, Some(primitive)) if is_unlanded_integer(program, binary.left) => {
                        Some(primitive)
                    }
                    _ => None,
                }
            }
        },
        ExpressionNode::Unary(unary) => match unary.operator {
            UnaryOperator::LogicalNot => Some(PrimitiveType::Bool),
            UnaryOperator::BitwiseNot => expression_primitive(program, unary.operand),
        },
        _ => None,
    }
}

fn is_unlanded_integer(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    matches!(program.expression_table.expression(expression),
        ExpressionNode::Integer(literal) if literal.landing().is_none())
}

/// Exact declared result types shared by proof-only coercion and embedding.
/// References are not erased here: embedding observes an integer payload,
/// never the address or referent of an arbitrary borrow.
pub(crate) fn expression_type_reference(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            crate::expression_types::named_value_type_reference(program, path)
                .or_else(|| reserved_results::type_reference(program, expression))
        }
        ExpressionNode::Member(member) => {
            if let Some(receiver) = expression_type_reference(program, member.receiver)
                && let Some(data) = crate::places::data_definition_for_type(program, receiver)
            {
                return program.data_members(data).iter().find_map(
                    |data_member| match data_member {
                        typed_trees::data::DataMember::Field(field)
                            if field.name == member.member =>
                        {
                            Some(field.type_reference)
                        }
                        _ => None,
                    },
                );
            }
            program
                .data_definitions()
                .iter()
                .flat_map(|data| program.data_members(data))
                .find_map(|data_member| match data_member {
                    typed_trees::data::DataMember::Field(field)
                        if member.member_symbol.is_valid()
                            && field.symbol == member.member_symbol =>
                    {
                        Some(field.type_reference)
                    }
                    _ => None,
                })
        }
        ExpressionNode::Cast(cast) if !cast.form.is_recast() => Some(cast.target_type),
        ExpressionNode::Call(call) => {
            if is_exact_embed_call(program, call) && has_embedding_shape(call) {
                return proof_int_type_reference(program);
            }
            if !call.target_symbol.is_valid() {
                return None;
            }
            if let Some(operator) =
                typed_trees::operator::resolve_named_expression_call(program, call)
            {
                return operator
                    .return_type
                    .is_valid()
                    .then_some(operator.return_type);
            }
            program.machines().iter().find_map(|machine| {
                let states = program.machine_states(machine);
                let state = if machine.symbol == call.target_symbol {
                    states.first()
                } else {
                    states
                        .iter()
                        .find(|state| state.symbol == call.target_symbol)
                }?;
                state.return_type.is_valid().then_some(state.return_type)
            })
        }
        ExpressionNode::Indexed(indexed) => {
            let collection = expression_type_reference(program, indexed.collection)?;
            let collection = crate::places::unwrapped_type_reference(program, collection)?;
            match program.type_reference_table.type_reference(collection) {
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => Some(*element_type),
                _ => None,
            }
        }
        _ => None,
    }
}

fn primitive_range(primitive: PrimitiveType) -> Option<(BigInt, BigInt)> {
    let (minimum, maximum) = match primitive {
        PrimitiveType::I8 => (i128::from(i8::MIN), i128::from(i8::MAX)),
        PrimitiveType::I16 => (i128::from(i16::MIN), i128::from(i16::MAX)),
        PrimitiveType::I32 => (i128::from(i32::MIN), i128::from(i32::MAX)),
        PrimitiveType::I64 => (i128::from(i64::MIN), i128::from(i64::MAX)),
        PrimitiveType::U8 => (0, i128::from(u8::MAX)),
        PrimitiveType::U16 => (0, i128::from(u16::MAX)),
        PrimitiveType::U32 => (0, i128::from(u32::MAX)),
        PrimitiveType::U64 | PrimitiveType::Addr => (0, i128::from(u64::MAX)),
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => return None,
    };
    Some((BigInt::from_i128(minimum), BigInt::from_i128(maximum)))
}

fn collect_statement_nodes(
    program: &TypedTrees,
    statement: &StatementNode,
    nodes: &mut Vec<ExpressionHandle>,
) {
    match statement {
        StatementNode::AssemblyFact(fact) => {
            collect_expression_nodes(program, fact.expression, nodes)
        }
        StatementNode::Assignment(assignment) => {
            collect_expression_nodes(program, assignment.target, nodes);
            collect_expression_nodes(program, assignment.value, nodes);
        }
        StatementNode::Call(call) => {
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_expression_nodes(program, *argument, nodes);
            }
        }
        StatementNode::Expression(expression) => {
            collect_expression_nodes(program, *expression, nodes)
        }
        StatementNode::LocalData(local) => {
            collect_expression_nodes(program, local.initial_value, nodes)
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                collect_expression_nodes(program, guard, nodes);
            }
            for target in [transition.target, transition.continuation] {
                match program.statement_table.transition_target(target) {
                    TransitionTargetNode::Value(expression) => {
                        collect_expression_nodes(program, *expression, nodes)
                    }
                    TransitionTargetNode::Named { arguments, .. } => {
                        for argument in program.expression_table.expression_handles(*arguments) {
                            collect_expression_nodes(program, *argument, nodes);
                        }
                    }
                    TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
                }
            }
        }
    }
}

fn collect_expression_nodes(
    program: &TypedTrees,
    expression: ExpressionHandle,
    nodes: &mut Vec<ExpressionHandle>,
) {
    if !expression.is_valid() || nodes.contains(&expression) {
        return;
    }
    nodes.push(expression);
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

#[cfg(test)]
mod tests {
    use super::*;
    use numerics::arithmetic::ArithmeticDomain;
    use numerics::literals::{IntegerLanding, IntegerLiteral};
    use typed_trees::expression::{TableBinaryExpression, TableUnaryExpression};

    fn byte(program: &mut TypedTrees) -> ExpressionHandle {
        program.expression_table.insert(ExpressionNode::Integer(
            IntegerLiteral::from_value(3).with_landing(IntegerLanding {
                landed_type: LandedIntegerType::U8,
                domain: ArithmeticDomain::Wrapping,
            }),
        ))
    }

    #[test]
    fn comparisons_and_logical_operations_do_not_inherit_integer_carriers() {
        let mut program = TypedTrees::default();
        let value = byte(&mut program);
        for operator in [
            BinaryOperator::Equal,
            BinaryOperator::NotEqual,
            BinaryOperator::Less,
            BinaryOperator::LessOrEqual,
            BinaryOperator::Greater,
            BinaryOperator::GreaterOrEqual,
            BinaryOperator::And,
            BinaryOperator::Or,
        ] {
            let expression =
                program
                    .expression_table
                    .insert(ExpressionNode::Binary(TableBinaryExpression {
                        left: value,
                        operator,
                        right: value,
                    }));
            assert!(
                integer_embedding(&program, expression).is_none(),
                "{operator:?}"
            );
        }
        let expression =
            program
                .expression_table
                .insert(ExpressionNode::Unary(TableUnaryExpression {
                    operator: UnaryOperator::LogicalNot,
                    operand: value,
                }));
        assert!(integer_embedding(&program, expression).is_none());
    }

    #[test]
    fn integer_computations_retain_the_selected_source_carrier_range() {
        let mut program = TypedTrees::default();
        let value = byte(&mut program);
        for operator in [
            BinaryOperator::Add,
            BinaryOperator::Subtract,
            BinaryOperator::Multiply,
            BinaryOperator::BitwiseAnd,
            BinaryOperator::ShiftLeft,
        ] {
            let expression =
                program
                    .expression_table
                    .insert(ExpressionNode::Binary(TableBinaryExpression {
                        left: value,
                        operator,
                        right: value,
                    }));
            let embedding = integer_embedding(&program, expression).expect("integer result");
            assert_eq!(embedding.primitive, PrimitiveType::U8);
            assert_eq!(embedding.minimum, BigInt::zero());
            assert_eq!(embedding.maximum, BigInt::from_u64(255));
        }
        let expression =
            program
                .expression_table
                .insert(ExpressionNode::Unary(TableUnaryExpression {
                    operator: UnaryOperator::BitwiseNot,
                    operand: value,
                }));
        assert_eq!(
            integer_embedding(&program, expression).unwrap().primitive,
            PrimitiveType::U8
        );
    }

    #[test]
    fn all_integer_carrier_ranges_are_exact_and_nonintegers_reject() {
        assert_eq!(
            primitive_range(PrimitiveType::I64),
            Some((
                BigInt::from_i128(i128::from(i64::MIN)),
                BigInt::from_i128(i128::from(i64::MAX))
            ))
        );
        for primitive in [PrimitiveType::U64, PrimitiveType::Addr] {
            assert_eq!(
                primitive_range(primitive),
                Some((BigInt::zero(), BigInt::from_u64(u64::MAX)))
            );
        }
        for primitive in [PrimitiveType::Bool, PrimitiveType::F32, PrimitiveType::F64] {
            assert!(primitive_range(primitive).is_none());
        }
    }

    #[test]
    fn shift_embeddings_require_integer_counts_and_preserve_left_carriers() {
        let mut program = TypedTrees::default();
        let value = byte(&mut program);
        let boolean = program
            .expression_table
            .insert(ExpressionNode::Boolean(true));
        let float = program.expression_table.insert(ExpressionNode::Float(
            numerics::literals::FloatLiteral::from_f64(1.5),
        ));
        let signed_count = program.expression_table.insert(ExpressionNode::Integer(
            IntegerLiteral::from_value(-1).with_landing(IntegerLanding {
                landed_type: LandedIntegerType::I16,
                domain: ArithmeticDomain::Exact,
            }),
        ));
        let literal_count = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(1)));
        for operator in [BinaryOperator::ShiftLeft, BinaryOperator::ShiftRight] {
            for count in [boolean, float, signed_count, literal_count] {
                let expression = program.expression_table.insert(ExpressionNode::Binary(
                    TableBinaryExpression {
                        left: value,
                        operator,
                        right: count,
                    },
                ));
                let embedding = integer_embedding(&program, expression);
                if count == boolean || count == float {
                    assert!(
                        embedding.is_none(),
                        "{operator:?} must reject noninteger count"
                    );
                } else {
                    assert_eq!(
                        embedding.expect("integer count").primitive,
                        PrimitiveType::U8
                    );
                }
            }
        }
    }
}
