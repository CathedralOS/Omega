//! Admission and carrier typing for the compiler-owned proof integer embedding.
//!
//! The source expression keeps its selected arithmetic meaning. Recognizing an
//! embedding neither executes that expression nor replaces it with mathematical
//! arithmetic; proof normalization must preserve the source denotation.

/// Membership sets take the integer hasher; none is iterated for output order.
type HashSet<T> = std::collections::HashSet<T, symbols::BuildSymbolHasher>;

use diagnostics::Diagnostic;
use numerics::bignum::BigInt;
use symbols::BuiltinFunction;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableCallExpression, UnaryOperator,
};
use typed_trees::proof_only::ProofOnlyClassification;
use typed_trees::proposition::{PropositionBody, PropositionFormula};
use typed_trees::statement::{StatementNode, StatementRootRole};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

use crate::expression_traversal::collect_expression_nodes;

mod calls;
pub use calls::ValidatedIntegerEmbeddingCall;
pub(crate) use calls::validate_integer_embedding_calls;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IntegerEmbedding {
    pub(crate) primitive: PrimitiveType,
    pub(crate) minimum: BigInt,
    pub(crate) maximum: Option<BigInt>,
}

pub(crate) fn validate_proof_embeddings(
    program: &dyn crate::CheckingInput,
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
    // These are transient sets of full generational handles, not semantic
    // identity. The vectors keep collection order; the hash sets keep
    // membership checks off the quadratic path for program-scale forests.
    let mut proof_nodes = Vec::new();
    let mut runtime_nodes = Vec::new();
    let mut proof_seen = HashSet::default();
    let mut runtime_seen = HashSet::default();
    for (_, fact) in typed_trees::references::program_proof_facts(program) {
        match fact {
            ProofFact::Expression(expression) => {
                collect_expression_nodes(program, *expression, &mut proof_nodes, &mut proof_seen);
            }
            ProofFact::Membership(membership) => {
                collect_expression_nodes(
                    program,
                    membership.value,
                    &mut proof_nodes,
                    &mut proof_seen,
                );
            }
            ProofFact::Proposition(application) => {
                for argument in program
                    .expression_table
                    .expression_handles(application.arguments)
                {
                    collect_expression_nodes(program, *argument, &mut proof_nodes, &mut proof_seen);
                }
            }
        }
    }
    for expression in crate::mathematical_applications::expression_nodes(program) {
        collect_expression_nodes(program, expression, &mut proof_nodes, &mut proof_seen);
    }
    for proposition in program.propositions() {
        if let PropositionBody::Transparent { proposition } = &proposition.body {
            match proposition {
                PropositionFormula::BooleanExpression(expression) => {
                    collect_expression_nodes(
                        program,
                        *expression,
                        &mut proof_nodes,
                        &mut proof_seen,
                    );
                }
                PropositionFormula::Application(application) => {
                    for argument in program
                        .expression_table
                        .expression_handles(application.arguments)
                    {
                        collect_expression_nodes(
                            program,
                            *argument,
                            &mut proof_nodes,
                            &mut proof_seen,
                        );
                    }
                }
            }
        }
    }
    for machine in program.machines() {
        if classification.is_proof_machine(program, machine) {
            for state in program.machine_states(machine) {
                for statement in program.statement_table.statements(state.statement_nodes) {
                    collect_statement_nodes(program, statement, &mut proof_nodes, &mut proof_seen);
                }
            }
        } else if let Some(expression) =
            crate::value_custody::content_projections::content_projection_body_expression(
                program, machine,
            )
        {
            collect_expression_nodes(program, expression, &mut proof_nodes, &mut proof_seen);
        } else {
            for state in program.machine_states(machine) {
                for statement in program.statement_table.statements(state.statement_nodes) {
                    if let Some(fact) = statement.as_assembly_fact() {
                        for expression in program.tables.proof_fact_expression_roots(fact.fact) {
                            collect_expression_nodes(
                                program,
                                expression,
                                &mut proof_nodes,
                                &mut proof_seen,
                            );
                        }
                    } else {
                        collect_statement_nodes(
                            program,
                            statement,
                            &mut runtime_nodes,
                            &mut runtime_seen,
                        );
                    }
                }
            }
        }
    }

    for (handle, node) in program.expression_table.iter_expressions() {
        let mathematical = match node {
            ExpressionNode::Name(path) => {
                program.symbols.builtin_type_member(path.symbol).is_some()
            }
            ExpressionNode::Call(call) => {
                crate::mathematical_applications::selected_definition(program, call).is_some()
            }
            _ => false,
        };
        if mathematical && (!proof_seen.contains(&handle) || runtime_seen.contains(&handle)) {
            diagnostics.push(Diagnostic::error("mathematical terms are proof-only and cannot be used as executable values or statements"));
        }
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
        } else if !proof_seen.contains(&handle) || runtime_seen.contains(&handle) {
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
                if let Some(call) = statement.as_call()
                    && program
                        .mathematical_definitions()
                        .iter()
                        .any(|definition| definition.symbol == call.target_symbol)
                {
                    diagnostics.push(Diagnostic::error("a mathematical definition denotes a proof term, not an executable statement call"));
                }
                if let Some(call) = statement.as_call()
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

pub(crate) fn is_exact_embed_call(
    program: &dyn crate::CheckingInput,
    call: &TableCallExpression,
) -> bool {
    !call.receiver.is_valid()
        && call.target.as_str() == BuiltinFunction::IntegerEmbed.name()
        && program
            .symbols
            .builtin_function_for_symbol(call.target_symbol)
            == Some(BuiltinFunction::IntegerEmbed)
}

pub(crate) fn integer_embedding(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<IntegerEmbedding> {
    let primitive = expression_primitive(program, expression)?;
    let (minimum, maximum) = if primitive == PrimitiveType::Addr {
        // Addr denotes the selected model, not the legacy hosted u64 payload.
        // Its upper range is related to the exact symbolic bound below.
        (BigInt::zero(), None)
    } else {
        let (minimum, maximum) = primitive_range(primitive)?;
        (minimum, Some(maximum))
    };
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
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<(PrimitiveType, ExpressionHandle)> {
    let call = program.expression_table.call_node(expression)?;
    if !is_exact_embed_call(program, call) || !has_embedding_shape(call) {
        return None;
    }
    let [argument] = program.expression_table.expression_handles(call.arguments) else {
        return None;
    };
    Some((integer_embedding(program, *argument)?.primitive, *argument))
}

pub(crate) fn machine_contains_integer_embedding(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> bool {
    let mut nodes = Vec::new();
    let mut seen = HashSet::default();
    for contract in program.machine_contracts(machine).iter().chain(
        program
            .machine_states(machine)
            .iter()
            .flat_map(|state| program.state_contracts(state)),
    ) {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            match fact {
                ProofFact::Expression(expression) => {
                    collect_expression_nodes(program, *expression, &mut nodes, &mut seen)
                }
                ProofFact::Membership(membership) => {
                    collect_expression_nodes(program, membership.value, &mut nodes, &mut seen)
                }
                ProofFact::Proposition(application) => {
                    for argument in program
                        .expression_table
                        .expression_handles(application.arguments)
                    {
                        collect_expression_nodes(program, *argument, &mut nodes, &mut seen);
                    }
                }
            }
        }
    }
    for state in program.machine_states(machine) {
        for statement in program.statement_table.statements(state.statement_nodes) {
            collect_statement_nodes(program, statement, &mut nodes, &mut seen);
        }
    }
    nodes.into_iter().any(|expression| {
        matches!(program.expression_table.expression(expression),
        ExpressionNode::Call(call) if is_exact_embed_call(program, call))
    })
}

pub(crate) fn proof_int_type_reference(
    program: &dyn crate::CheckingInput,
) -> Option<TypeReferenceHandle> {
    let symbol = program
        .symbols
        .builtin_type_symbol(symbols::BuiltinType::Int)?;
    program
        .type_reference_table
        .find_named_type_reference(symbol)
}

fn expression_primitive(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<PrimitiveType> {
    if let Some(type_reference) = expression_type_reference(program, expression) {
        return program.primitive_type_reference(type_reference);
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(literal) => Some(PrimitiveType::from_landed_integer(
            literal.landing()?.landed_type,
        )),
        ExpressionNode::Boolean(_) => Some(PrimitiveType::Bool),
        ExpressionNode::Binary(binary) => match binary.operator {
            BinaryOperator::CaseMembership => Some(PrimitiveType::Bool),
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

fn is_unlanded_integer(program: &dyn crate::CheckingInput, expression: ExpressionHandle) -> bool {
    matches!(program.expression_table.expression(expression),
        ExpressionNode::Integer(literal) if literal.landing().is_none())
}

/// The receiver a collection measure is read from.
///
/// Borrow spellings are peeled only for this classification. Embedding still
/// refuses to treat an arbitrary borrow as an integer payload; the measure
/// itself is the `u64` count, not the borrowed collection.
fn collection_measure_receiver(
    program: &dyn crate::CheckingInput,
    member: &typed_trees::expression::TableMemberExpression,
) -> Option<crate::collection_operations::MeasureReceiver> {
    let mut receiver = member.receiver;
    while let Some(borrow) = program.expression_table.borrow_expression(receiver) {
        receiver = borrow.target;
    }
    if let ExpressionNode::Indexed(indexed) = program.expression_table.expression(receiver)
        && program.expression_table.indexed_on_range(indexed)
    {
        return Some(crate::collection_operations::MeasureReceiver::Window);
    }
    expression_type_reference(program, receiver)
        .map(crate::collection_operations::MeasureReceiver::Declared)
}

/// Exact declared result types shared by proof-only coercion and embedding.
/// References are not erased here: embedding observes an integer payload,
/// never the address or referent of an arbitrary borrow.
pub(crate) fn expression_type_reference(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            crate::value_custody::expression_types::named_value_type_reference(program, path)
                .or_else(|| {
                    crate::proof_contracts::contract_results::type_reference(program, expression)
                })
        }
        ExpressionNode::Member(member) => {
            // A `self.<field>` receiver resolves to the owning machine's
            // symbol — not a value name — and its member binds the
            // machine-scoped field symbol, which is a distinct handle from the
            // attached data declaration's field row. `exact_self_field`
            // verifies that receiver identity and rejoins the field through
            // the attached definition's own symbols.
            if let ExpressionNode::Name(receiver) =
                program.expression_table.expression(member.receiver)
                && let Some(machine) = program.machine(receiver.symbol)
                && let Some(field) =
                    crate::value_custody::places::exact_self_field(program, machine, expression)
            {
                return Some(field.type_reference);
            }
            if let Some(receiver) = expression_type_reference(program, member.receiver)
                && let Some(data) =
                    crate::value_custody::places::data_definition_for_type(program, receiver)
            {
                return program
                    .data_field_named(data, member.member.as_str())
                    .map(|field| field.type_reference);
            }
            // Structural `.len` and `.capacity` are compiler-owned measures of
            // an array, slice, or range window. Lowering already retains the
            // builtin `u64` carrier for that measure; embedding has to recover
            // the same carrier. A record field that happens to be spelled
            // `len` is handled above and is not a measure.
            if let Some(measure_receiver) = collection_measure_receiver(program, member)
                && crate::collection_operations::collection_measure_member(
                    program,
                    member,
                    measure_receiver,
                )
                .is_some()
            {
                return crate::proof_contracts::bound_expression_meaning::builtin_type_reference(
                    program,
                    symbols::BuiltinTypeAtom::U64,
                );
            }
            crate::declarations::data_queries::data_field_type_by_symbol(
                program,
                member.member_symbol,
            )
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
                    typed_trees::state::state_by_symbol(states, call.target_symbol)
                }?;
                state.return_type.is_valid().then_some(state.return_type)
            })
        }
        ExpressionNode::Indexed(indexed) => {
            let collection = expression_type_reference(program, indexed.collection)?;
            let collection =
                crate::value_custody::places::unwrapped_type_reference(program, collection)?;
            crate::value_custody::places::collection_element_type(program, collection)
        }
        _ => None,
    }
}

pub(crate) fn primitive_range(primitive: PrimitiveType) -> Option<(BigInt, BigInt)> {
    primitive
        .integer_bounds()
        .map(|(minimum, maximum)| (BigInt::from_i128(minimum), BigInt::from_i128(maximum)))
}

fn collect_statement_nodes(
    program: &dyn crate::CheckingInput,
    statement: &StatementNode,
    nodes: &mut Vec<ExpressionHandle>,
    seen: &mut HashSet<ExpressionHandle>,
) {
    // Statement and expression argument spans have distinct arena owners.
    // Borrow the statement table's roots rather than interpreting its handles
    // in the expression table. Static root bindings supply no executable value.
    for (role, expression) in program.tables.expression_roots(statement) {
        if matches!(
            role,
            StatementRootRole::BindingReceiver | StatementRootRole::BindingImplementation
        ) {
            continue;
        }
        collect_expression_nodes(program, expression, nodes, seen);
    }
}
