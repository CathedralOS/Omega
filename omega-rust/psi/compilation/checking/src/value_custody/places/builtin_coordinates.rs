//! Builtin meaning required before interpreting place selectors as storage geometry.

use super::{declared_indexed_projection_type, declared_place_type_raw};
use language_core::OperatorSpelling;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use typed_trees::machine::Machine;
use typed_trees::state::State;

/// Recognize collection metadata through the receiver's structural type, not
/// through a same-spelled nominal field or accessor.
pub fn collection_length_receiver(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let receiver = program
        .expression_table
        .authored_length_receiver(expression)?;
    let receiver_type = declared_place_type_raw(program, machine, state, receiver)?;
    let receiver_type = super::unwrapped_type_reference(program, receiver_type)?;
    matches!(
        program.type_reference_table.type_reference(receiver_type),
        typed_trees::types::TypeReferenceNode::Slice { .. }
            | typed_trees::types::TypeReferenceNode::FixedArray { .. }
    )
    .then_some(receiver)
}

/// The constant extent a `.len` observation reads when the receiver's
/// declared shape is statically countable: a fixed array names its declared
/// length, and a borrow or reference spelled around such a place loans the
/// same extent through its shell. A runtime-length slice keeps the
/// observation symbolic — callers then discharge the bound through the
/// carried-fact channels as before.
pub fn declared_collection_extent(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    receiver: ExpressionHandle,
) -> Option<usize> {
    // `declared_place_type_raw` already reads through `&`/`&mut` spellings to
    // the loaned place's declared type, so `(&mut control).len` on a fixed
    // field resolves the same extent `control.len` does.
    let declared = declared_place_type_raw(program, machine, state, receiver)?;
    fixed_array_type_length(program, declared)
}

/// A type establishes exact length only for always-full arrays. Preserve
/// constrained shells until their carrier classification is known: bounded
/// byte owners carry a live length independently of their storage capacity.
/// Expression and canonical-place observations share this rule with bounds
/// checking, so projecting or borrowing a buffer cannot turn capacity into
/// evidence that any bytes are present.
pub fn fixed_array_type_length(
    program: &dyn crate::CheckingInput,
    mut reference: typed_trees::types::TypeReferenceHandle,
) -> Option<usize> {
    loop {
        if crate::bounded_byte_buffer_capacity(program, reference).is_some() {
            return None;
        }
        match program.type_reference_table.type_reference(reference) {
            typed_trees::types::TypeReferenceNode::FixedArray {
                length: typed_trees::types::FixedArrayLength::Literal(extent),
                ..
            } => return Some(*extent),
            typed_trees::types::TypeReferenceNode::Reference { referee, .. }
            | typed_trees::types::TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => reference = *referee,
            _ => return None,
        }
    }
}

/// Resolve the indexed geometry of a readable subslice occurrence. This
/// query grants no access: each consumer retains the authored borrow and
/// checks its access against the requested view and source loan. Write-only
/// borrowing cannot supply the length observation required by narrowing.
pub fn subslice_expression(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let operation = match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(borrow)
            if matches!(
                borrow.access,
                language_core::ReferenceAccess::Shared | language_core::ReferenceAccess::Mutable
            ) =>
        {
            borrow.target
        }
        ExpressionNode::Borrow(_) => return None,
        _ => expression,
    };
    let indexed = program.expression_table.indexed_node(operation)?;
    matches!(
        program.expression_table.expression(indexed.index),
        ExpressionNode::Range(_)
    )
    .then_some(operation)
}

/// Recognize builtin subslicing without granting bounds, source
/// custody, or permission to use the resulting view.
pub fn has_builtin_subslice_meaning(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> bool {
    let Some(indexed) = program.expression_table.indexed_node(expression) else {
        return false;
    };
    if program.expression_table.range_node(indexed.index).is_none() {
        return false;
    }
    indexed_operation_has_builtin_meaning(
        program,
        machine,
        state,
        expression,
        declared_place_type_raw(program, machine, state, indexed.collection),
    )
}

/// A uniquely selected range's exact declared guarantee supplies its extent.
/// This grants no relation to the input storage or independent bounds proof.
pub fn range_operator_establishes_endpoint_extent(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> bool {
    let Some(indexed) = program.expression_table.indexed_node(expression) else {
        return false;
    };
    let Some(range) = program.expression_table.range_node(indexed.index) else {
        return false;
    };
    let Some(collection) = declared_place_type_raw(program, machine, state, indexed.collection)
    else {
        return false;
    };
    if !reference_has_builtin_collection_geometry(program, collection) {
        return false;
    }
    let operands = [
        Some(collection),
        declared_place_type_raw(program, machine, state, range.start),
        declared_place_type_raw(program, machine, state, range.end),
    ];
    let candidates = typed_trees::operator::resolve_indexed_spelling_for_operands(
        program,
        OperatorSpelling::Range,
        &operands,
        program.expression_table.source_span(expression),
    );
    let [candidate] = candidates.as_slice() else {
        return false;
    };
    // A unique source candidate is insufficient when the applied trait scope
    // supplies another range meaning. Its result law cannot license this one.
    if typed_trees::operator::selected_trait_operator_meaning_exists(
        program.typed_program(),
        machine.symbol,
        OperatorSpelling::Range,
        &operands,
    ) {
        return false;
    }
    operator_ensures_endpoint_extent(program, candidate.operator)
}

/// The actuals of a completed call retain their ordinary positional mapping.
/// A source-owned result equation describes length, never backing identity.
pub fn ensured_range_call_endpoints(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<(ExpressionHandle, ExpressionHandle)> {
    let call = program.expression_table.call_node(expression)?;
    let arguments = program.expression_table.expression_handles(call.arguments);
    let [_, start, end] = arguments else {
        return None;
    };
    call_target_ensures_endpoint_extent(program, call.target_symbol).then_some((*start, *end))
}

fn call_target_ensures_endpoint_extent(
    program: &dyn crate::CheckingInput,
    symbol: symbols::SymbolHandle,
) -> bool {
    if let Some(operator) = typed_trees::operator::declaration_by_symbol(program, symbol)
        && operator_ensures_endpoint_extent(program, operator)
    {
        return true;
    }
    if let Some(machine) = program.machine_by_symbol(symbol) {
        let Some(entry) = program.machine_states(machine).first() else {
            return false;
        };
        let parameters = program.state_parameters(entry);
        return contracts_establish_endpoint_extent(
            program,
            machine.symbol,
            parameters,
            program.machine_contracts(machine),
        ) || contracts_establish_endpoint_extent(
            program,
            entry.symbol,
            parameters,
            program.state_contracts(entry),
        );
    }
    let Some(state) = program.state_by_symbol(symbol) else {
        return false;
    };
    let parameters = program.state_parameters(state);
    contracts_establish_endpoint_extent(
        program,
        state.symbol,
        parameters,
        program.state_contracts(state),
    ) || program
        .machine_holding_state(state.symbol)
        .is_some_and(|machine| {
            // A machine call may be retargeted to its exact entry. Header
            // guarantees do not describe a sibling state's return.
            program
                .machine_states(machine)
                .first()
                .is_some_and(|entry| entry.symbol == state.symbol)
                && contracts_establish_endpoint_extent(
                    program,
                    machine.symbol,
                    parameters,
                    program.machine_contracts(machine),
                )
        })
}

fn operator_ensures_endpoint_extent(
    program: &dyn crate::CheckingInput,
    operator: &typed_trees::operator::OperatorDefinition,
) -> bool {
    operator.spelling == Some(OperatorSpelling::Range)
        && contracts_establish_endpoint_extent(
            program,
            operator.symbol,
            program.operator_parameters(operator),
            program.operator_contracts(operator),
        )
}

fn contracts_establish_endpoint_extent(
    program: &dyn crate::CheckingInput,
    owner: symbols::SymbolHandle,
    parameters: &[typed_trees::signature::StateParameter],
    contracts: &[typed_trees::signature::SignatureContract],
) -> bool {
    let [_, start, end] = parameters else {
        return false;
    };
    // A syntactic subtraction is not an arithmetic license. Preserve the
    // source carriers and policy before importing a mathematical difference.
    if ![start, end].into_iter().all(|parameter| {
        program
            .primitive_type_reference(parameter.type_reference)
            .is_some_and(|primitive| primitive.integer_sign_bits().is_some())
            && program.arithmetic_domain_for_type_reference(parameter.type_reference)
                == numerics::arithmetic::ArithmeticDomain::Exact
    }) {
        return false;
    }
    contracts
        .iter()
        .filter(|contract| contract.kind == typed_trees::signature::SignatureContractKind::Ensures)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .any(|fact| {
            let typed_trees::domain::ProofFact::Expression(expression) = fact else {
                return false;
            };
            expression_establishes_endpoint_extent(program, owner, parameters, *expression, 0)
        })
}

fn expression_establishes_endpoint_extent(
    program: &dyn crate::CheckingInput,
    owner: symbols::SymbolHandle,
    parameters: &[typed_trees::signature::StateParameter],
    expression: ExpressionHandle,
    depth: usize,
) -> bool {
    if depth >= 128 {
        return false;
    }
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return false;
    };
    if binary.operator == typed_trees::expression::BinaryOperator::And {
        return [binary.left, binary.right].into_iter().any(|part| {
            expression_establishes_endpoint_extent(program, owner, parameters, part, depth + 1)
        });
    }
    if binary.operator != typed_trees::expression::BinaryOperator::Equal
        || !crate::has_builtin_parameter_bound_expression_meaning(
            program, owner, parameters, expression,
        )
    {
        return false;
    }
    [(binary.left, binary.right), (binary.right, binary.left)]
        .into_iter()
        .any(|(length, difference)| {
            let Some(member) = program.expression_table.member_node(length) else {
                return false;
            };
            let Some((result_owner, result_type)) =
                crate::reserved_result_owner(program, member.receiver)
            else {
                return false;
            };
            if member.case_variant.is_some()
                || member.member.as_str() != "len"
                || result_owner != owner
                || !reference_has_builtin_collection_geometry(program, result_type)
            {
                return false;
            }
            let Some(subtract) = program.expression_table.binary_node(difference) else {
                return false;
            };
            subtract.operator == typed_trees::expression::BinaryOperator::Subtract
                && expression_names_source_parameter(program, owner, parameters, subtract.left, 2)
                && expression_names_source_parameter(program, owner, parameters, subtract.right, 1)
        })
}

fn expression_names_source_parameter(
    program: &dyn crate::CheckingInput,
    owner: symbols::SymbolHandle,
    parameters: &[typed_trees::signature::StateParameter],
    expression: ExpressionHandle,
    position: usize,
) -> bool {
    let Some(parameter) = parameters
        .get(position)
        .filter(|parameter| parameter.symbol.is_valid())
    else {
        return false;
    };
    if crate::proof_contracts::contract_results::source_contract_declaration_owner(
        program, expression,
    ) != Ok(owner)
    {
        return false;
    }
    let Some(source_parameters) =
        crate::proof_contracts::contract_results::source_contract_parameters(
            program, owner, expression,
        )
    else {
        return false;
    };
    if source_parameters.len() != parameters.len()
        || !source_parameters
            .iter()
            .zip(parameters)
            .all(|(source, parameter)| {
                source.symbol == parameter.symbol
                    && source.type_reference == parameter.type_reference
            })
    {
        return false;
    }
    let Some(path) = program.expression_table.name_path_node(expression) else {
        return false;
    };
    let [name] = program.expression_table.name_path_members(path.members) else {
        return false;
    };
    name.as_str() == parameter.name.as_str()
        && source_parameters
            .iter()
            .filter(|source| source.name == parameter.name)
            .count()
            == 1
        && crate::expression_traversal::single_segment_name_symbol(program, expression)
            .filter(|symbol| symbol.is_valid())
            .is_none_or(|symbol| symbol == parameter.symbol)
}

fn reference_has_builtin_collection_geometry(
    program: &dyn crate::CheckingInput,
    mut reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    for _ in 0..128 {
        if !reference.is_valid() {
            return false;
        }
        match program.type_reference_table.type_reference(reference) {
            typed_trees::types::TypeReferenceNode::Reference { referee, .. }
            | typed_trees::types::TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => {
                reference = *referee;
            }
            typed_trees::types::TypeReferenceNode::FixedArray { .. }
            | typed_trees::types::TypeReferenceNode::Slice { .. } => return true,
            _ => return false,
        }
    }
    false
}

/// Authenticate this indexed occurrence, independently of how its collection
/// was produced. A call result or an earlier declared operator may supply a
/// collection without granting storage geometry for that whole expression.
/// The caller supplies the actual collection carrier; this query establishes
/// only operator meaning, not bounds, integer landing, or loan authority.
pub fn indexed_operation_has_builtin_meaning(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    collection: Option<typed_trees::types::TypeReferenceHandle>,
) -> bool {
    let Some(indexed) = program.expression_table.indexed_node(expression) else {
        return false;
    };
    let Some(collection) = collection else {
        return false;
    };
    let mut operands = [Some(collection), None, None];
    let (spelling, operand_count) =
        if let Some(range) = program.expression_table.range_node(indexed.index) {
            operands[1] = declared_place_type_raw(program, machine, state, range.start);
            operands[2] = declared_place_type_raw(program, machine, state, range.end);
            (OperatorSpelling::Range, 3)
        } else {
            operands[1] = declared_place_type_raw(program, machine, state, indexed.index);
            (OperatorSpelling::Index, 2)
        };
    let operands = &operands[..operand_count];
    if spelling == OperatorSpelling::Index {
        // Produced values and stored places share index meaning. In
        // particular, neither may bypass the lexical trait-bound exclusion
        // merely because its collection has a fixed-array carrier.
        return typed_trees::operator::builtin_index_read_is_spelled(
            program,
            machine.symbol,
            expression,
            operands,
        );
    }
    typed_trees::operator::resolve_indexed_spelling_for_operands(
        program,
        spelling,
        operands,
        program.expression_table.source_span(expression),
    )
    .is_empty()
        && typed_trees::operator::has_builtin_spelled_expression_meaning(
            program,
            machine.symbol,
            expression,
            spelling,
            operands,
        )
}

/// The source of an index-preserving whole view. This is storage geometry,
/// not borrow authority: consumers still check the selected access and loan.
/// Only builtin range meaning permits this equality. A declared operator with
/// identical bounds may return unrelated storage and establishes no such edge.
/// Omitted bounds and constant extents share this history-free judgment.
/// Captured scalar endpoints need a live fact correspondence; the declaration
/// of their initializer alone cannot establish the window's current geometry.
pub fn whole_collection_view_source(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let operation = subslice_expression(program, expression)?;
    if !has_builtin_subslice_meaning(program, machine, state, operation) {
        return None;
    }
    let indexed = program.expression_table.indexed_node(operation)?;
    let range = program.expression_table.range_node(indexed.index)?;
    let integer = |expression| {
        // Closed arithmetic still belongs to the selected numeric owner;
        // syntax-only folding could substitute a declared token machine.
        if let Some(value) = program.closed_integer_expression_value_in(expression, machine.symbol)
        {
            value.to_u64()
        } else if let Some(state) = state {
            crate::captured_integer_value(program, machine, state, expression, |_| None)
                .and_then(|value| value.to_u64())
        } else {
            None
        }
    };
    if range.start.is_valid() && integer(range.start) != Some(0) {
        return None;
    }
    if !range.end.is_valid() {
        return (!range.end_inclusive).then_some(indexed.collection);
    }
    if let Some(end) = integer(range.end)
        .and_then(|end| usize::try_from(end).ok())
        .and_then(|end| {
            if range.end_inclusive {
                end.checked_add(1)
            } else {
                Some(end)
            }
        })
        && declared_collection_extent(program, machine, state, indexed.collection) == Some(end)
    {
        return Some(indexed.collection);
    }
    None
}

/// The selector expressions of an assignment target's indexed steps, from
/// the target inward: `a[i][j]` yields `[j, i]`, `a[i].f` yields `[i]`, and
/// `a.f` yields none. A range selector names a view rather than an element,
/// so it contributes no selector. Position `depth` in the result is the
/// `AssignmentIndex { depth }` coordinate a checked assignment evaluates for
/// that step, before its value; producers and consumers of those rows share
/// this one walk.
pub fn assignment_target_selectors(
    program: &dyn crate::CheckingInput,
    target: ExpressionHandle,
) -> Vec<ExpressionHandle> {
    assignment_target_indexes(&program.expression_table, target)
        .into_iter()
        .filter_map(
            |expression| match program.expression_table.expression(expression) {
                ExpressionNode::Indexed(indexed) => Some(indexed.index),
                _ => None,
            },
        )
        .collect()
}

/// The scalar indexed occurrences along an assignment's destination, in its
/// outside-in coordinate order. An occurrence identifies its own collection
/// and bound, even when two steps use the same selector. The selector walk and
/// checked-decision consumers use this same ordering; `a[i][j]` retains the
/// outer `a[i][j]` occurrence at depth zero and `a[i]` at depth one.
pub fn assignment_target_indexes(
    expressions: &ExpressionTable,
    target: ExpressionHandle,
) -> Vec<ExpressionHandle> {
    let mut indexes = Vec::new();
    let mut cursor = target;
    for _ in 0..128 {
        if !expressions.expression_is_valid(cursor) {
            break;
        }
        match expressions.expression(cursor) {
            ExpressionNode::Indexed(indexed) => {
                if !matches!(
                    expressions.expression(indexed.index),
                    ExpressionNode::Range(_)
                ) {
                    indexes.push(cursor);
                }
                cursor = indexed.collection;
            }
            ExpressionNode::Member(member) => cursor = member.receiver,
            _ => break,
        }
    }
    indexes
}

/// Check only the operation meaning of a place spine. Consumers separately
/// establish declaration identity, reference origins, bounds, and authority.
/// A dynamic builtin selector may pass without establishing a fixed index.
pub fn place_has_builtin_coordinates(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> bool {
    builtin_coordinates(program, machine, state, expression, 0)
}

/// A value spine (a fresh call or constructor result) has no place type for
/// `place_has_builtin_coordinates` to name. Its walker supplies the collection
/// carrier it reached, and the same `[]` spelling and bound authentication
/// decide whether this selection is the builtin element coordinate.
pub fn value_element_has_builtin_coordinate(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    occurrence: ExpressionHandle,
    collection: typed_trees::types::TypeReferenceHandle,
) -> bool {
    let Some(indexed) = program.expression_table.indexed_node(occurrence) else {
        return false;
    };
    if program.expression_table.range_node(indexed.index).is_some() {
        return false;
    }
    indexed_operation_has_builtin_meaning(program, machine, state, occurrence, Some(collection))
        && crate::has_builtin_bound_expression_meaning(program, machine, state, indexed.index)
}

/// Authenticate selectors above an already produced value. Calls and
/// constructors own fresh observation namespaces; their result types supply
/// each collection carrier, but never grant builtin meaning to its selectors.
/// This grants no value, bounds, custody, or lifetime evidence.
pub fn value_has_builtin_coordinates(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    value_has_builtin_coordinates_with_types(program, machine, state, expression, |source| {
        crate::expression_result_type_reference(program, machine, state, source)
    })
}

/// Completed contextual operands may supply carriers unavailable to the
/// context-free query. The resolver contributes types only; every selector and
/// endpoint still authenticates its selected builtin meaning here.
pub fn value_has_builtin_coordinates_with_types(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    mut expression: ExpressionHandle,
    collection_type: impl Fn(ExpressionHandle) -> Option<typed_trees::types::TypeReferenceHandle>,
) -> bool {
    for depth in 0..program.expression_table.expression_count() {
        let Some(node) = program
            .expression_table
            .expression_at_depth(expression, depth)
        else {
            return false;
        };
        match node {
            ExpressionNode::Borrow(borrow) => expression = borrow.target,
            ExpressionNode::Member(member) => expression = member.receiver,
            ExpressionNode::Indexed(indexed) => {
                let collection = collection_type(indexed.collection);
                if !indexed_operation_has_builtin_meaning(
                    program,
                    machine,
                    Some(state),
                    expression,
                    collection,
                ) {
                    return false;
                }
                let bounds = program
                    .expression_table
                    .range_node(indexed.index)
                    .map_or([indexed.index, ExpressionHandle::invalid()], |range| {
                        [range.start, range.end]
                    });
                if bounds.into_iter().any(|bound| {
                    bound.is_valid()
                        && !crate::has_builtin_bound_expression_meaning(
                            program,
                            machine,
                            Some(state),
                            bound,
                        )
                }) {
                    return false;
                }
                expression = indexed.collection;
            }
            _ => return true,
        }
    }
    false
}

/// The element type when `symbol` names a state parameter whose declared
/// type is a mutable view over bytes (`&mut [u8]`). Other collection shapes
/// and other access permissions have no builtin byte-write geometry.
pub fn mutable_byte_slice_parameter_element(
    program: &dyn crate::CheckingInput,
    state: &State,
    symbol: symbols::SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    use typed_trees::types::TypeReferenceNode;
    let parameter = program.state_parameter_by_symbol(state, symbol)?;
    let TypeReferenceNode::Reference {
        referee,
        access: language_core::ReferenceAccess::Mutable,
        ..
    } = program
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        return None;
    };
    let TypeReferenceNode::Slice { element_type } =
        program.type_reference_table.type_reference(*referee)
    else {
        return None;
    };
    (program.primitive_type_reference(*element_type) == Some(typed_trees::types::PrimitiveType::U8))
        .then_some(*element_type)
}

fn builtin_coordinates(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    depth: usize,
) -> bool {
    let Some(node) = program
        .expression_table
        .expression_at_depth(expression, depth)
    else {
        return false;
    };
    match node {
        ExpressionNode::Name(_) => true,
        ExpressionNode::SavedSubject(saved) => program.saved_value_source(saved).is_some(),
        ExpressionNode::Borrow(borrow) => {
            builtin_coordinates(program, machine, state, borrow.target, depth + 1)
        }
        ExpressionNode::Member(member) => {
            builtin_coordinates(program, machine, state, member.receiver, depth + 1)
        }
        ExpressionNode::Indexed(indexed) => {
            // Check the collection spine before contextual projection. This
            // rules out calls and authored indexing beneath a later index.
            if !builtin_coordinates(program, machine, state, indexed.collection, depth + 1) {
                return false;
            }
            if let Some(range) = program.expression_table.range_node(indexed.index) {
                // A window keeps its collection's backing rather than
                // selecting an element type. Authenticate the range operator
                // and each present bound before interpreting that geometry.
                let collection =
                    declared_place_type_raw(program, machine, state, indexed.collection)
                        .and_then(|reference| super::unwrapped_type_reference(program, reference));
                return collection.is_some_and(|reference| {
                    matches!(
                        program.type_reference_table.type_reference(reference),
                        typed_trees::types::TypeReferenceNode::Slice { .. }
                            | typed_trees::types::TypeReferenceNode::FixedArray { .. }
                    )
                }) && has_builtin_subslice_meaning(program, machine, state, expression)
                    && [range.start, range.end].into_iter().all(|bound| {
                        !bound.is_valid()
                            || crate::has_builtin_bound_expression_meaning(
                                program, machine, state, bound,
                            )
                    });
            }
            if declared_indexed_projection_type(program, machine, state, expression).is_none() {
                return false;
            }
            let operands = [
                declared_place_type_raw(program, machine, state, indexed.collection).or_else(
                    || {
                        declared_indexed_projection_type(
                            program,
                            machine,
                            state,
                            indexed.collection,
                        )
                    },
                ),
                declared_place_type_raw(program, machine, state, indexed.index),
            ];
            // Indexed syntax has implicit shared collection adaptation. The
            // ordinary custody query alone does not perform that matching.
            typed_trees::operator::builtin_index_read_is_spelled(
                program,
                machine.symbol,
                expression,
                &operands,
            ) && crate::has_builtin_bound_expression_meaning(program, machine, state, indexed.index)
        }
        _ => false,
    }
}
