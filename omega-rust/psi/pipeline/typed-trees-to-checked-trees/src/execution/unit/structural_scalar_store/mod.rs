//! Exact projected scalar-store admission for parameter and local storage.
use super::{
    CheckFacts, CheckedScalarExpression, CheckedScalarExpressionRole, CheckedStructuralAccess,
    CheckedStructuralScalarFieldStorePlan, CheckedStructuralScalarParameterPlan,
    CheckedUnitEffectOperationPlan, CheckedUnitScalarResultBindingPlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment, DataMember,
    DataShapeKind, ExpressionNode, Multiplicity, PrimitiveType, StatementNode, SymbolHandle,
    TypeReferenceNode, TypedTrees,
};
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::terminal_field_identity;
use crate::execution::terminal_unit::types::byte_sequence_carrier;

mod frame;
#[cfg(test)]
mod tests;

/// Local storage uses the same ordered scalar effect as borrowed parameters.
/// Only the destination authority differs; the captured RHS keeps its original
/// checked evaluation and statement identity.
pub(in crate::execution) fn build_local_scalar_field_store(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
) -> Option<CheckedStructuralScalarFieldStorePlan> {
    let statements = program.statement_table.statements(state.statement_nodes);
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index as usize,
        assignment.target,
    )?;
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return None;
    };
    let mut locals =
        statements
            .iter()
            .enumerate()
            .filter_map(|(ordinal, statement)| match statement {
                StatementNode::LocalData(local) if local.symbol == symbol => Some((ordinal, local)),
                _ => None,
            });
    let (ordinal, local) = locals.next()?;
    if locals.next().is_some() || ordinal >= statement_index as usize {
        return None;
    }
    super::scalar_graph_record_shapes(program, local.type_reference)?;
    validation::record_local_disposition(
        program,
        facts,
        machine.symbol,
        state.symbol,
        u32::try_from(ordinal).ok()?,
    )?;
    let (leaf, carriers) = place.segments.split_last()?;
    let facts::PlaceSegment::Field {
        symbol: field_symbol,
    } = leaf
    else {
        return None;
    };
    let mut carrier_type = local.type_reference;
    let mut carrier_path = Vec::with_capacity(carriers.len());
    for segment in carriers {
        let facts::PlaceSegment::Field { symbol } = segment else {
            return None;
        };
        let owner =
            crate::facts::field_domain::data_definition_for_field_type(program, carrier_type)?;
        if !plain_record(owner, program) {
            return None;
        }
        let field = exact_relevant_field(program, owner, *symbol)?;
        carrier_path.push(CheckedUnitStructuralPathSegment::Field(
            terminal_field_identity(program, field.symbol)?,
        ));
        carrier_type = field.type_reference;
    }
    let owner = crate::facts::field_domain::data_definition_for_field_type(program, carrier_type)?;
    if !plain_record(owner, program) {
        return None;
    }
    let field = exact_relevant_field(program, owner, *field_symbol)?;
    // A bounded leaf needs its actual write obligation, not just the carrier.
    let TypeReferenceNode::Named {
        symbol: primitive_symbol,
        name,
    } = program
        .type_reference_table
        .type_reference(field.type_reference)
    else {
        return None;
    };
    let atom = program.symbols.builtin_type_atom(*primitive_symbol)?;
    if name.as_str() != atom.symbol_name() {
        return None;
    }
    let primitive_type = program.primitive_type_reference(field.type_reference)?;
    if !matches!(
        primitive_type,
        PrimitiveType::Bool
            | PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
    ) || program.arithmetic_domain_for_type_reference(field.type_reference)
        != numerics::arithmetic::ArithmeticDomain::Exact
    {
        return None;
    }
    let role = CheckedScalarExpressionRole::AssignmentValue;
    let computations = &facts.values.scalar_computations;
    let value = if let Some(root) = computations.root_at(state.symbol, statement_index, role) {
        if root.machine != machine.symbol
            || !computations.nodes.is_valid(root.root)
            || computations.nodes.get(root.root).authored_root != assignment.value
            || computations.nodes.get(root.root).primitive_type != primitive_type
            || facts
                .values
                .scalar_expressions
                .expression_at(state.symbol, statement_index, role)
                .is_some()
        {
            return None;
        }
        checked_trees::CheckedStructuralScalarFieldStoreValue::Computation(root.root)
    } else {
        let (binding, value) = facts.values.scalar_expressions.bound_expression_at(
            state.symbol,
            statement_index,
            role,
        )?;
        if binding.expression != assignment.value
            || crate::values::scalar_expression_type(value) != Some(primitive_type)
        {
            return None;
        }
        checked_trees::CheckedStructuralScalarFieldStoreValue::Pure(value.clone())
    };
    Some(CheckedStructuralScalarFieldStorePlan {
        statement_index,
        destination: checked_trees::CheckedStructuralScalarFieldStoreDestination::Local { symbol },
        carrier_path,
        field_identity: terminal_field_identity(program, field.symbol)?,
        primitive_type,
        value,
    })
}

pub(super) fn build_structural_scalar_field_store(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    statements: &[StatementNode],
    scalar_result_local: Option<&CheckedUnitScalarResultBindingPlan>,
    selected_scalar_result_local: Option<&CheckedUnitScalarResultBindingPlan>,
    trace: &LocalConstructionTrace,
) -> Option<CheckedStructuralScalarFieldStorePlan> {
    let result_local = scalar_result_local.or(selected_scalar_result_local);
    let (statement_index, assignment) = match (result_local, statements) {
        (None, [StatementNode::Assignment(assignment)]) => (0, assignment),
        (
            Some(result),
            [
                StatementNode::LocalData(_),
                StatementNode::Assignment(assignment),
            ],
        ) if result.statement_index == 0 && result.binding_ordinal == 0 => (1, assignment),
        _ => return None,
    };
    let operation = build_structural_field_store_at(
        program,
        facts,
        machine,
        state,
        structural_parameters,
        scalar_parameters,
        statement_index,
        assignment,
        result_local,
        selected_scalar_result_local.is_some(),
        false,
        None,
        trace,
    )?;
    let CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) = operation else {
        return None;
    };
    Some(store)
}

/// `build_structural_scalar_field_store_sequence` tracing which assignment
/// (or which write-frame guard) declined the body.
pub(super) fn build_structural_scalar_field_store_sequence_traced(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    statement_start: usize,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    trace: &LocalConstructionTrace,
) -> Option<Vec<CheckedUnitEffectOperationPlan>> {
    let statements = program.statement_table.statements(state.statement_nodes);
    if !statements
        .iter()
        .skip(statement_start)
        .any(|statement| matches!(statement, StatementNode::Assignment(_)))
    {
        return Some(Vec::new());
    }
    trace.phase("scalar field store sequence: state write frame");
    let frame = &match facts.mutation.for_machine(machine.symbol) {
        Some(mutation) => match mutation
            .state_write_frames
            .iter()
            .find(|frame| frame.state == state.symbol)
        {
            Some(plan) => &plan.frame,
            None => {
                return None;
            }
        },
        None => {
            return None;
        }
    };
    trace.phase("scalar field store sequence: write frame agreement");
    if !frame::matches(program, machine, state, frame, call_frames) {
        return None;
    }
    trace.phase("scalar field store sequence: assignment store");
    let mut stores = Vec::new();
    for (statement_index, statement) in statements.iter().enumerate().skip(statement_start) {
        let StatementNode::Assignment(assignment) = statement else {
            continue;
        };
        let statement_index = u32::try_from(statement_index).ok()?;
        trace.statement(Some(statement_index));
        if let Some(store) = super::primitive_store::build_primitive_store_at(
            program,
            facts,
            machine,
            state,
            structural_parameters,
            statement_index,
            assignment,
        ) {
            stores.push(store);
            continue;
        }
        trace.phase("scalar field store sequence: structural field store");
        if let Some(store) = build_structural_field_store_at(
            program,
            facts,
            machine,
            state,
            structural_parameters,
            scalar_parameters,
            statement_index,
            assignment,
            None,
            false,
            true,
            None,
            trace,
        ) {
            stores.push(store);
            continue;
        }
        // An assignment whose source is this statement's own call has no
        // authored scalar expression to store. Its call operation is sequenced
        // with the other calls, and the store consuming that result is
        // appended there; it deliberately produces no row here.
        if matches!(
            program.expression_table.expression(assignment.value),
            ExpressionNode::Call(_)
        ) {
            continue;
        }
        return None;
    }
    Some(stores)
}

/// Ordinary composition for `self.field = call(..)`: the statement's scalar
/// call establishes its result and this store consumes that SSA value. Every
/// destination check -- exclusive borrowed authority, carrier path, relevant
/// unconstrained scalar field, and the exact state write frame -- is the one
/// the authored-source route makes; only where the value comes from differs.
pub(super) fn build_structural_call_result_field_store(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    call_result: (u32, PrimitiveType),
    trace: &LocalConstructionTrace,
) -> Option<CheckedUnitEffectOperationPlan> {
    build_structural_field_store_at(
        program,
        facts,
        machine,
        state,
        structural_parameters,
        scalar_parameters,
        statement_index,
        assignment,
        None,
        false,
        true,
        Some(call_result),
        trace,
    )
}

/// Test convenience: the traced builder without a trace.
#[cfg(test)]
pub(super) fn build_structural_scalar_field_store_sequence(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    statement_start: usize,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CheckedUnitEffectOperationPlan>> {
    build_structural_scalar_field_store_sequence_traced(
        program,
        facts,
        machine,
        state,
        structural_parameters,
        scalar_parameters,
        statement_start,
        call_frames,
        &LocalConstructionTrace::default(),
    )
}

fn build_structural_field_store_at(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    result_local: Option<&CheckedUnitScalarResultBindingPlan>,
    selected_result: bool,
    exact_sequence_frame: bool,
    // Dense scalar-namespace position and result type of the call this same
    // statement performs, when the store's source is that call's SSA result
    // rather than an authored scalar expression.
    call_result: Option<(u32, PrimitiveType)>,
    trace: &LocalConstructionTrace,
) -> Option<CheckedUnitEffectOperationPlan> {
    // Every byte-store destination lane replays the same resolved scalar
    // source: the SSA result of the call this same statement performs, or the
    // bound pure authored expression.
    let byte_value = byte_store_scalar_value(
        program,
        facts,
        state,
        statement_index,
        assignment,
        call_result,
    );
    if let Some(write) = structural_parameters.iter().find_map(|destination| {
        let parameter = program
            .state_parameters(state)
            .get(destination.position as usize)?;
        build_byte_view_write(
            program,
            facts,
            machine,
            state,
            destination,
            parameter,
            statement_index,
            assignment,
            byte_value.as_ref(),
        )
    }) {
        return Some(CheckedUnitEffectOperationPlan::ByteSequenceWrite(write));
    }
    trace.phase("structural field store: destination parameter");
    let source_parameters = program.state_parameters(state);
    let target_place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        usize::try_from(statement_index).ok()?,
        assignment.target,
    )?;
    // The assignment selects its destination; unrelated borrowed inputs do not
    // change that root's authority or require it to occupy position zero.
    let mut destinations = structural_parameters.iter().filter_map(|destination| {
        let parameter = source_parameters.get(destination.position as usize)?;
        (target_place.root == facts::PlaceRoot::Symbol(parameter.symbol))
            .then_some((destination, parameter))
    });
    let (destination, parameter) = destinations.next()?;
    if destinations.next().is_some()
        || destination.multiplicity == Multiplicity::Linear
        || !matches!(
            destination.access,
            CheckedStructuralAccess::MutableBorrow | CheckedStructuralAccess::WriteOnlyBorrow
        )
        || !destination.qualifications.is_empty()
    {
        return None;
    }
    trace.phase("structural field store: parameter access");
    if crate::execution::terminal_unit::abi_parameter_count(source_parameters)
        != scalar_parameters.len() + structural_parameters.len()
        || parameter.is_self != destination.is_self
        || parameter.is_const
        || !parameter.is_mutable
    {
        return None;
    }
    let TypeReferenceNode::Reference {
        access, referee, ..
    } = program
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        return None;
    };
    let expected_access = match access {
        language_semantics::ReferenceAccess::Mutable => CheckedStructuralAccess::MutableBorrow,
        language_semantics::ReferenceAccess::WriteOnly => CheckedStructuralAccess::WriteOnlyBorrow,
        language_semantics::ReferenceAccess::Shared => return None,
    };
    if destination.access != expected_access {
        return None;
    }
    trace.phase("structural field store: root owner record");
    let mut carrier_type = *referee;
    // A receiver's source referent is `Self`; its declaration identity comes
    // from the machine attachment, not a global lookup of that spelling. A
    // borrowed fixed-array root has no record owner of its own: its element
    // record resolves through the first carrier `FixedIndex` hop below, so an
    // unresolved root owner stays admissible only behind an index segment.
    let root_owner = if destination.is_self {
        Some(
            program
                .data_definitions()
                .iter()
                .find(|data| data.symbol == machine.attached_data_symbol)?,
        )
    } else {
        crate::facts::field_domain::data_definition_for_field_type(program, carrier_type)
    };
    if let Some(owner) = root_owner
        && !plain_record(owner, program)
    {
        return None;
    }
    trace.phase("structural field store: target place");
    let (target, byte_index) = match program.expression_table.expression(assignment.target) {
        ExpressionNode::Indexed(indexed) => {
            if !validation::place_has_builtin_coordinates(
                program,
                machine,
                Some(state),
                assignment.target,
            ) || matches!(
                program.expression_table.expression(indexed.index),
                ExpressionNode::Range(_)
            ) {
                return None;
            }
            (indexed.collection, Some(indexed.index))
        }
        _ => (assignment.target, None),
    };
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        usize::try_from(statement_index).ok()?,
        target,
    )?;
    if place.root != facts::PlaceRoot::Symbol(parameter.symbol) {
        return None;
    }
    trace.phase("structural field store: carrier path");
    let (final_segment, carrier_segments) = place.segments.split_last()?;
    let facts::PlaceSegment::Field {
        symbol: field_symbol,
    } = final_segment
    else {
        return None;
    };
    let mut carrier_path = Vec::with_capacity(carrier_segments.len());
    let mut carrier_owner = root_owner;
    let mut reached_array = false;
    for segment in carrier_segments {
        match segment {
            facts::PlaceSegment::Field { symbol } if !reached_array => {
                let field_owner = carrier_owner?;
                if !plain_record(field_owner, program) {
                    return None;
                }
                let carrier = exact_relevant_field(program, field_owner, *symbol)?;
                if !crate::facts::field_domain::domain_constraint_symbols(
                    program,
                    carrier.type_reference,
                )
                .is_empty()
                {
                    return None;
                }
                carrier_path.push(CheckedUnitStructuralPathSegment::Field(
                    terminal_field_identity(program, carrier.symbol)?,
                ));
                carrier_type = carrier.type_reference;
                carrier_owner = crate::facts::field_domain::data_definition_for_field_type(
                    program,
                    carrier_type,
                );
            }
            facts::PlaceSegment::FixedIndex { index } if !reached_array => {
                reached_array = true;
                let TypeReferenceNode::FixedArray {
                    element_type,
                    length: typed_trees::types::FixedArrayLength::Literal(length),
                } = program.type_reference_table.type_reference(carrier_type)
                else {
                    return None;
                };
                if *index >= *length {
                    return None;
                }
                carrier_path.push(CheckedUnitStructuralPathSegment::FixedIndex(
                    u64::try_from(*index).ok()?,
                ));
                carrier_type = *element_type;
                carrier_owner = crate::facts::field_domain::data_definition_for_field_type(
                    program,
                    carrier_type,
                );
            }
            _ => return None,
        }
    }
    trace.phase("structural field store: field owner record");
    let field_owner = carrier_owner?;
    if !plain_record(field_owner, program) {
        return None;
    }
    let field = exact_relevant_field(program, field_owner, *field_symbol)?;
    trace.phase("structural field store: write frame");
    let source_path =
        crate::labels::canonical_place_label_from_parts(program, place.root, &place.segments);
    let source_root = crate::labels::canonical_place_label_from_parts(program, place.root, &[]);
    // Mutation summaries name the receiver separately from the ordinary
    // parameter roster, even when it occupies structural position zero.
    let mutation_root = if destination.is_self {
        "self".to_owned()
    } else {
        format!("$P{}", destination.position)
    };
    let expected_mutation_path =
        format!("{mutation_root}{}", source_path.strip_prefix(&source_root)?,);
    let array_collection_mutation_path = place
        .segments
        .iter()
        .position(|segment| matches!(segment, facts::PlaceSegment::FixedIndex { .. }))
        .and_then(|first_index| {
            let collection_path = crate::labels::canonical_place_label_from_parts(
                program,
                place.root,
                &place.segments[..first_index],
            );
            Some(format!(
                "{mutation_root}{}",
                collection_path.strip_prefix(&source_root)?
            ))
        });
    let frame = &facts
        .mutation
        .for_machine(machine.symbol)?
        .state_write_frames
        .iter()
        .find(|frame| frame.state == state.symbol)?
        .frame;
    let exact_frame =
        matches!(frame.complete_paths(), Some([path]) if path == &expected_mutation_path);
    let exact_collection_frame = matches!(
        (frame.complete_paths(), array_collection_mutation_path.as_ref()),
        (Some([path]), Some(collection_path)) if path == collection_path
    );
    // Provider selection resolves the boundary initializer after ordinary
    // mutation analysis, so that initializer leaves the pre-selection frame
    // opaque. The exact selected result, two-statement body, and canonical
    // projected destination are independently rejoined above and below.
    let unresolved_selected_frame = selected_result
        && !exact_sequence_frame
        && frame.completeness() == facts::WriteFrameCompleteness::Opaque;
    if !exact_frame
        && !exact_collection_frame
        && !unresolved_selected_frame
        && !exact_sequence_frame
    {
        return None;
    }
    trace.phase("structural field store: byte sequence carrier");
    if let Some(checked_trees::CheckedByteSequenceCarrier::BoundedOwned { capacity }) =
        byte_sequence_carrier(program, field.type_reference, &[])
    {
        if result_local.is_some() || selected_result {
            return None;
        }
        if let Some(byte_index) = byte_index {
            if !crate::facts::field_domain::domain_constraint_symbols(program, field.type_reference)
                .into_iter()
                .all(|symbol| {
                    program
                        .domain_definitions()
                        .iter()
                        .find(|domain| domain.symbol == symbol)
                        .is_some_and(|domain| {
                            domain.establishment_routes.is_empty()
                                && domain.semantic_roles == Default::default()
                        })
                })
            {
                return None;
            }
            let (index_binding, index) = facts.values.scalar_expressions.bound_expression_at(
                state.symbol,
                statement_index,
                CheckedScalarExpressionRole::AssignmentIndex,
            )?;
            if index_binding.expression != byte_index
                || crate::values::scalar_expression_type(index) != Some(PrimitiveType::U64)
            {
                return None;
            }
            return Some(
                CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(
                    checked_trees::CheckedStructuralByteSequenceFieldByteStorePlan {
                        statement_index,
                        destination_parameter_position: destination.position,
                        carrier_path,
                        field_identity: terminal_field_identity(program, field.symbol)?,
                        index: index.clone(),
                        value: byte_value?,
                    },
                ),
            );
        }
        let ExpressionNode::String(bytes) = program.expression_table.expression(assignment.value)
        else {
            return None;
        };
        if u64::try_from(bytes.len()).ok()? > capacity
            || !crate::facts::field_domain::domain_constraint_symbols(program, field.type_reference)
                .into_iter()
                .all(|symbol| {
                    program
                        .domain_definitions()
                        .iter()
                        .find(|domain| domain.symbol == symbol)
                        .is_some_and(|domain| {
                            domain.establishment_routes.is_empty()
                                && domain.semantic_roles == Default::default()
                                && crate::facts::field_domain::string_literal_expression_grants_domain(
                                    program,
                                    assignment.value,
                                    symbol,
                                )
                        })
                })
        {
            return None;
        }
        return Some(
            CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(
                checked_trees::CheckedStructuralByteSequenceFieldStorePlan {
                    statement_index,
                    destination_parameter_position: destination.position,
                    carrier_path,
                    field_identity: terminal_field_identity(program, field.symbol)?,
                    bytes: bytes.to_vec(),
                },
            ),
        );
    }
    trace.phase("structural field store: scalar field type");
    if byte_index.is_some() {
        return None;
    }
    if !crate::facts::field_domain::domain_constraint_symbols(program, field.type_reference)
        .is_empty()
    {
        return None;
    }
    let primitive_type = program.primitive_type_reference(field.type_reference)?;
    if !matches!(
        primitive_type,
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64
    ) && (!primitive_type.accepts_integer_literal() || primitive_type == PrimitiveType::Addr)
    {
        return None;
    }
    trace.phase("structural field store: computation source");
    let computations = &facts.values.scalar_computations;
    if let Some(root) = computations.root_at(
        state.symbol,
        statement_index,
        CheckedScalarExpressionRole::AssignmentValue,
    ) {
        if !exact_sequence_frame
            || result_local.is_some()
            || root.machine != machine.symbol
            || !computations.nodes.is_valid(root.root)
            || computations.nodes.get(root.root).authored_root != assignment.value
            || computations.nodes.get(root.root).primitive_type != primitive_type
            || facts
                .values
                .scalar_expressions
                .expression_at(
                    state.symbol,
                    statement_index,
                    CheckedScalarExpressionRole::AssignmentValue,
                )
                .is_some()
        {
            return None;
        }
        return Some(CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(
            CheckedStructuralScalarFieldStorePlan {
                statement_index,
                destination:
                    checked_trees::CheckedStructuralScalarFieldStoreDestination::Parameter {
                        position: destination.position,
                    },
                carrier_path,
                field_identity: terminal_field_identity(program, field.symbol)?,
                primitive_type,
                value: checked_trees::CheckedStructuralScalarFieldStoreValue::Computation(
                    root.root,
                ),
            },
        ));
    }
    trace.phase("structural field store: pure source");
    // A store may also read the SSA result of the scalar call this same
    // statement performs. That authored form binds no local, so no
    // `AssignmentValue` scalar-expression row names the value; the ordered
    // call operation established it, which is exactly the "already-defined,
    // exactly typed SSA value" the store vocabulary asks for
    // (wiki/spec/terminal-psi/structural_access.md, Store vocabulary).
    if let Some((position, result_type)) = call_result {
        if !exact_sequence_frame
            || result_local.is_some()
            || selected_result
            || result_type != primitive_type
            || !matches!(
                program.expression_table.expression(assignment.value),
                ExpressionNode::Call(_)
            )
        {
            return None;
        }
        return Some(CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(
            CheckedStructuralScalarFieldStorePlan {
                statement_index,
                destination:
                    checked_trees::CheckedStructuralScalarFieldStoreDestination::Parameter {
                        position: destination.position,
                    },
                carrier_path,
                field_identity: terminal_field_identity(program, field.symbol)?,
                primitive_type,
                value: checked_trees::CheckedStructuralScalarFieldStoreValue::ScalarResult {
                    position,
                },
            },
        ));
    }
    let (binding, value) = facts.values.scalar_expressions.bound_expression_at(
        state.symbol,
        statement_index,
        CheckedScalarExpressionRole::AssignmentValue,
    )?;
    if binding.expression != assignment.value {
        return None;
    }
    let direct_result_is_exact = matches!(
        (result_local, value),
        (
            Some(result),
            CheckedScalarExpression::Local {
                position: 0,
                primitive_type: source_type,
            },
        ) if *source_type == result.primitive_type
            && primitive_type == result.primitive_type
            && matches!(
                result.primitive_type,
                PrimitiveType::I8
                    | PrimitiveType::I16
                    | PrimitiveType::I32
                    | PrimitiveType::I64
                    | PrimitiveType::U8
                    | PrimitiveType::U16
                    | PrimitiveType::U32
                    | PrimitiveType::U64
            )
            && scalar_parameters.is_empty()
    );
    let literal = match value {
        CheckedScalarExpression::IeeeFloatLiteral { .. } => {
            crate::values::scalar_expression_type(value) == Some(primitive_type)
        }
        CheckedScalarExpression::IntegerLiteral { .. } => {
            primitive_type.accepts_integer_literal() && primitive_type != PrimitiveType::Addr
        }
        CheckedScalarExpression::Boolean(boolean) => {
            primitive_type == PrimitiveType::Bool
                && matches!(
                    boolean.as_ref(),
                    checked_trees::CheckedBooleanExpression::Constant(_)
                )
        }
        _ => false,
    };
    // IEEE replacement forwards existing bits; selected floating computation
    // must retain its own operation and call correspondence before admission.
    if matches!(primitive_type, PrimitiveType::F32 | PrimitiveType::F64)
        && !literal
        && checked_parameter_source(value).is_none()
    {
        return None;
    }
    let exact_source = if exact_sequence_frame || direct_result_is_exact || literal {
        true
    } else if scalar_parameters.is_empty() {
        false
    } else {
        let (position, source_type) = checked_parameter_source(value)?;
        scalar_parameters.get(position).is_some_and(|parameter| {
            Some(parameter.source_position) == authored_scalar_position(position)
                && parameter.primitive_type == primitive_type
                && source_type == primitive_type
        }) && scalar_parameters
            .iter()
            .enumerate()
            .all(|(index, parameter)| {
                Some(parameter.source_position) == authored_scalar_position(index)
            })
    };
    if !exact_source || crate::values::scalar_expression_type(value) != Some(primitive_type) {
        return None;
    }
    Some(CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(
        CheckedStructuralScalarFieldStorePlan {
            statement_index,
            destination: checked_trees::CheckedStructuralScalarFieldStoreDestination::Parameter {
                position: destination.position,
            },
            carrier_path,
            field_identity: terminal_field_identity(program, field.symbol)?,
            primitive_type,
            value: checked_trees::CheckedStructuralScalarFieldStoreValue::Pure(value.clone()),
        },
    ))
}

/// The byte-store scalar source, resolved once for every destination lane: the
/// SSA result of the scalar call this same statement performs, or the bound
/// pure authored expression. The call-result source binds no local, so no
/// `AssignmentValue` scalar-expression row names it; its dense position names
/// the value the ordered call operation established.
fn byte_store_scalar_value(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    call_result: Option<(u32, PrimitiveType)>,
) -> Option<checked_trees::CheckedByteSequenceStoreValue> {
    if let Some((position, result_type)) = call_result {
        if result_type != PrimitiveType::U8
            || !matches!(
                program.expression_table.expression(assignment.value),
                ExpressionNode::Call(_)
            )
        {
            return None;
        }
        return Some(checked_trees::CheckedByteSequenceStoreValue::ScalarResult { position });
    }
    let (binding, value) = facts.values.scalar_expressions.bound_expression_at(
        state.symbol,
        statement_index,
        CheckedScalarExpressionRole::AssignmentValue,
    )?;
    if binding.expression != assignment.value
        || crate::values::scalar_expression_type(value) != Some(PrimitiveType::U8)
    {
        return None;
    }
    Some(checked_trees::CheckedByteSequenceStoreValue::Pure(
        value.clone(),
    ))
}

fn build_byte_view_write(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    destination: &CheckedUnitStructuralParameterPlan,
    parameter: &typed_trees::signature::StateParameter,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    value: Option<&checked_trees::CheckedByteSequenceStoreValue>,
) -> Option<checked_trees::CheckedByteSequenceWritePlan> {
    if destination.is_self
        || destination.access != CheckedStructuralAccess::MutableBorrow
        || destination.multiplicity != Multiplicity::Unrestricted
        || !destination.qualifications.is_empty()
        || destination.fused_service_erasure.is_some()
        || byte_sequence_carrier(program, parameter.type_reference, &[])
            != Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView)
        || !validation::place_has_builtin_coordinates(
            program,
            machine,
            Some(state),
            assignment.target,
        )
    {
        return None;
    }
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(assignment.target)
    else {
        return None;
    };
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index as usize,
        indexed.collection,
    )?;
    if place.root != facts::PlaceRoot::Symbol(parameter.symbol) || !place.segments.is_empty() {
        return None;
    }
    let (index_binding, index) = facts.values.scalar_expressions.bound_expression_at(
        state.symbol,
        statement_index,
        CheckedScalarExpressionRole::AssignmentIndex,
    )?;
    if index_binding.expression != indexed.index
        || crate::values::scalar_expression_type(index) != Some(PrimitiveType::U64)
    {
        return None;
    }
    Some(checked_trees::CheckedByteSequenceWritePlan {
        statement_index,
        destination_parameter_position: destination.position,
        index: index.clone(),
        value: value?.clone(),
    })
}

fn authored_scalar_position(dense_position: usize) -> Option<u32> {
    u32::try_from(dense_position).ok()?.checked_add(1)
}

fn checked_parameter_source(value: &CheckedScalarExpression) -> Option<(usize, PrimitiveType)> {
    match value {
        CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        } => Some((*position, *primitive_type)),
        CheckedScalarExpression::Boolean(boolean) => {
            let checked_trees::CheckedBooleanExpression::Parameter { position } = boolean.as_ref()
            else {
                return None;
            };
            Some((*position, PrimitiveType::Bool))
        }
        _ => None,
    }
}

fn plain_record(data: &typed_trees::data::DataDefinition, program: &TypedTrees) -> bool {
    data.supply_mode == language_semantics::DataSupplyMode::CheckedShape
        && data.lifetime_parameters.is_empty()
        && program.data_type_parameters(data).is_empty()
        && retained_record_owner_application(data, program)
        && data.quotient.is_none()
        && data.where_facts.is_empty()
        && !data.zero_gated
        && typed_trees::data::DataDefinition::shape_kind_from_members(program.data_members(data))
            == DataShapeKind::Record
}

fn retained_record_owner_application(
    data: &typed_trees::data::DataDefinition,
    program: &TypedTrees,
) -> bool {
    let Some(application) = data.generic_instance else {
        return true;
    };
    let TypeReferenceNode::Generic {
        base_symbol,
        lifetime_arguments,
        arguments,
        ..
    } = program.type_reference_table.type_reference(application)
    else {
        return false;
    };
    let Some(template) = program
        .data_definitions()
        .iter()
        .find(|template| template.symbol == *base_symbol)
    else {
        return false;
    };
    let parameters = program.data_type_parameters(template);
    // Generated-instance arguments are checked in an empty type-parameter
    // scope before flow planning. Retain that exact closed owner application,
    // rather than excluding its substituted fields merely for being generated.
    // Field shape, access, arithmetic policy, and mutation custody are still
    // checked independently by the ordinary store route.
    base_symbol.is_valid()
        && *base_symbol != data.symbol
        && template.generic_instance.is_none()
        && template.lifetime_parameters.is_empty()
        && lifetime_arguments.is_empty()
        && !parameters.is_empty()
        && parameters.len()
            == program
                .type_reference_table
                .type_reference_handles(*arguments)
                .len()
}

fn exact_relevant_field<'a>(
    program: &'a TypedTrees,
    owner: &'a typed_trees::data::DataDefinition,
    symbol: SymbolHandle,
) -> Option<&'a typed_trees::data::DataField> {
    let fields = program
        .data_members(owner)
        .iter()
        .filter_map(|member| {
            let DataMember::Field(field) = member else {
                return None;
            };
            (field.symbol == symbol && !field.relevance.is_erased()).then_some(field)
        })
        .collect::<Vec<_>>();
    let [field] = fields.as_slice() else {
        return None;
    };
    Some(*field)
}
