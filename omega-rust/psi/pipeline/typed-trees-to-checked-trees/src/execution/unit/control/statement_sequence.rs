//! Authored-order scalar bindings, primitive storage, and structural bindings.
//! Constructors and calls share structural binding ordinals. Completion selects
//! one existing result or constructs its final expression in this same sequence;
//! surrounding supported effects do not select a different producer family.
use super::super::{
    CheckedTrivialAffineStructuralLocalPlan, CheckedUnitEntryClaimPlan,
    CheckedUnitPartialAffineDiscardPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
    PermissionProvenance,
};
use super::{
    CheckFacts, CheckedScalarExpression, CheckedScalarExpressionRole, CheckedStructuralAccess,
    CheckedStructuralScalarParameterPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitScalarResultBindingPlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralResultBindingPlan, ExpressionNode, Multiplicity, PermissionEventSource,
    StatementNode, SymbolHandle, TypeReferenceNode, TypedTrees,
};
use crate::execution::terminal_unit::ScalarCalleePlans;
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::control::call_occurrences;
use crate::execution::terminal_unit::control::call_results::bind_scalar_call_result;
use crate::execution::terminal_unit::control::call_results::bind_structural_call_result;
use crate::execution::terminal_unit::control::call_results::checked_structural_result_type;
use crate::execution::terminal_unit::control::call_results::checked_unit_structural_result_local;
use crate::execution::terminal_unit::returns::checked_boolean_contains_short_circuit;
use crate::execution::terminal_unit::{
    ExpectedCallValueResult, ShapeCollector, base_type_identity, build_call_operation, is_unit,
    machine_binders, parameter_qualifications, scalar_expression_local_at, structural_operands,
};
use checked_trees::CheckedUnitStructuralReturnPlan;

/// Completion forwards a whole parameter only when its exact output contract
/// and, for linear values, checked input-origin claim outcome agree.
fn returned_parameter(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    entry_claims: &[CheckedUnitEntryClaimPlan],
) -> Option<CheckedUnitStructuralReturnPlan> {
    let StatementNode::Expression(expression) = program
        .statement_table
        .statements(state.statement_nodes)
        .last()?
    else {
        return None;
    };
    let ExpressionNode::Name(path) = program.expression_table.expression(*expression) else {
        return None;
    };
    if path.head_symbol != path.symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
    {
        return None;
    }
    let source_parameters = program.state_parameters(state);
    let (parameter_index, parameter) = parameters.iter().enumerate().find(|(_, parameter)| {
        source_parameters
            .get(parameter.position as usize)
            .is_some_and(|source| source.symbol == path.symbol)
    })?;
    let source = source_parameters.get(parameter.position as usize)?;
    if let Some((_, access)) = super::super::reference_results::parts(program, state.return_type) {
        if super::super::reference_results::source_parameter(program, state)?
            != parameter.position as usize
            || parameter.access != access
            || !parameter.qualifications.is_empty()
        {
            return None;
        }
        let source = CheckedUnitStructuralArgumentPlan {
            source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(parameter_index).ok()?,
            },
            path: Vec::new(),
            type_identity: parameter.type_identity.clone(),
            access,
        };
        return Some(CheckedUnitStructuralReturnPlan {
            source: source.source.clone(),
            type_identity: program
                .normalized_type_identity(state.return_type)
                .into_string(),
            multiplicity: Multiplicity::Affine,
            reference_sources: vec![checked_trees::CheckedReferenceResultSourcePlan {
                path: Vec::new(),
                source,
            }],
        });
    }
    if parameter.access != CheckedStructuralAccess::Owned
        || parameter.multiplicity != program.type_multiplicity(state.return_type)
        || program.normalized_type_identity(source.type_reference)
            != program.normalized_type_identity(state.return_type)
        || parameter.qualifications
            != validation::structural_result_qualifications(program, state.return_type).ok()?
    {
        return None;
    }
    if parameter.multiplicity == Multiplicity::Linear {
        let returned = validation::reconstruct_structural_parameter_return_claims(
            program,
            facts,
            machine.symbol,
            state.symbol,
            source.symbol,
        )
        .ok()?;
        let mut input = entry_claims
            .iter()
            .filter(|claim| claim.parameter_index as usize == parameter_index)
            .map(|claim| (claim.claim_identity, claim.path.clone()))
            .collect::<Vec<_>>();
        input.sort_by(|left, right| left.1.cmp(&right.1));
        // Completion and ordinary calls share one input-origin correspondence;
        // matching paths alone cannot replace the exact checked claim identities.
        if input != returned {
            return None;
        }
    }
    Some(CheckedUnitStructuralReturnPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
            parameter_index: u32::try_from(parameter_index).ok()?,
        },
        type_identity: parameter.type_identity.clone(),
        multiplicity: parameter.multiplicity,
        reference_sources: if validation::reference_result_custody::is_reference_record(
            program,
            state.return_type,
        ) {
            validation::reference_result_custody::returned_record_sources(program, state)?
        } else {
            Vec::new()
        },
    })
}

/// A `value.body`-style completion selects one borrowed carrier leaf of an
/// owned record formal rather than forwarding the whole parameter. The leaf
/// argument already carries its exact ingress path; the shared
/// reference-result block emits its establishment and rewrites the result
/// source to that binding. The establishment consumes the carrier whole, so
/// `source_leaf` only admits a record whose declared reference roster is
/// exactly the selected leaf.
fn returned_reference_leaf(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
) -> Option<CheckedUnitStructuralReturnPlan> {
    let (_, source) = validation::reference_result_custody::source_leaf(program, state)?;
    let CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } = source.source
    else {
        return None;
    };
    let parameter = parameters.get(parameter_index as usize)?;
    if parameter.access != CheckedStructuralAccess::Owned
        || parameter.multiplicity != Multiplicity::Affine
        || !parameter.qualifications.is_empty()
        || parameter.fused_service_erasure.is_some()
    {
        return None;
    }
    Some(CheckedUnitStructuralReturnPlan {
        source: source.source.clone(),
        type_identity: program
            .normalized_type_identity(state.return_type)
            .into_string(),
        multiplicity: Multiplicity::Affine,
        reference_sources: vec![checked_trees::CheckedReferenceResultSourcePlan {
            path: Vec::new(),
            source,
        }],
    })
}

pub(in crate::execution::terminal_unit) struct StatementSequence {
    pub(in crate::execution::terminal_unit) scalar_result:
        Option<CheckedUnitScalarResultBindingPlan>,
    pub(in crate::execution::terminal_unit) scalar_control:
        Option<checked_trees::CheckedUnitScalarControlPlan>,
    pub(in crate::execution::terminal_unit) structural_result:
        Option<CheckedUnitStructuralReturnPlan>,
    pub(in crate::execution::terminal_unit) operations: Vec<CheckedUnitEffectOperationPlan>,
    pub(in crate::execution::terminal_unit) local_count: usize,
    pub(in crate::execution::terminal_unit) structural_local_symbols: Vec<SymbolHandle>,
}

pub(super) fn has_structural_result(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    statement: &StatementNode,
) -> bool {
    let StatementNode::LocalData(local) = statement else {
        return false;
    };
    if !local.initial_value.is_valid() {
        return false;
    }
    if facts
        .values
        .structural_values
        .roots
        .iter()
        .any(|(_, root)| {
            root.machine == machine.symbol
                && root.expression == local.initial_value
                && root.type_reference == local.type_reference
        })
    {
        return true;
    }
    if validation::is_scalar_case_value(program, local.initial_value, local.type_reference) {
        return !local.is_mutable;
    }
    if validation::is_closed_primitive_array_type(program, local.type_reference) {
        return validation::scalar_array_elements(
            program,
            machine.symbol,
            local.initial_value,
            local.type_reference,
        )
        .is_some()
            || matches!(
                program.expression_table.expression(local.initial_value),
                ExpressionNode::Call(_)
            );
    }
    let ExpressionNode::Call(call) = program.expression_table.expression(local.initial_value)
    else {
        return false;
    };
    // The result signature is available before the producer body is selected.
    // This sequencing decision must not depend on the caller's result category
    // or on a legacy affine-return body recognizer; closure pruning still
    // requires the actual ordinary/composed callee plan.
    crate::flow::call_target_return_type(program, call.target_symbol).is_some_and(|reference| {
        program.normalized_type_identity(reference)
            == program.normalized_type_identity(local.type_reference)
            && super::checked_structural_result_type(
                program,
                &mut ShapeCollector::new(program),
                local.type_reference,
                &machine_binders(program, machine),
            )
            .is_some()
    })
}

/// The trace phase for one statement kind in the shared sequence, so an
/// omission names which statement family stopped local construction.
fn statement_phase(statement: &StatementNode) -> &'static str {
    match statement {
        StatementNode::Assignment(_) => "statement sequence: assignment",
        StatementNode::LocalData(_) => "statement sequence: local data",
        StatementNode::Call(_) => "statement sequence: call",
        StatementNode::Expression(_) => "statement sequence: expression",
        StatementNode::RootBinding(_) => "statement sequence: root binding",
        StatementNode::AssemblyFact(_) => "statement sequence: assembly fact",
        StatementNode::Transition(_) => "statement sequence: transition",
    }
}

pub(super) fn has_statement_shape(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    construction_statement_count: usize,
) -> bool {
    let completion = scalar_control(program, facts, machine, state);
    let prefix_count = completion
        .as_ref()
        .map(|(_, count)| *count)
        .unwrap_or(usize::MAX);
    // An erased borrow carrier is a checked alias, not a storage binding: its
    // declaration composes through the sequence while each use's planner joins
    // the captured referent's evidence.
    let borrow_aliases =
        super::super::receiver_aliases::aliases(program, facts, machine, state).unwrap_or_default();
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
        .take(prefix_count)
        .skip(construction_statement_count)
        .all(|(index, statement)| match statement {
            StatementNode::Call(_) | StatementNode::Assignment(_) => true,
            StatementNode::Expression(expression) => {
                call_occurrences::ordered_statement_call(program, machine, state, index).is_some()
                    || (index + 1
                        == program
                            .statement_table
                            .statements(state.statement_nodes)
                            .len()
                        && (u32::try_from(index).ok().is_some_and(|ordinal| {
                            facts
                                .values
                                .structural_values
                                .root_at(state.symbol, ordinal)
                                .is_some()
                        }) || validation::is_scalar_case_value(
                            program,
                            *expression,
                            state.return_type,
                        ) || matches!(
                            program.expression_table.expression(*expression),
                            ExpressionNode::Name(_)
                        ) || validation::reference_result_custody::source_leaf(program, state)
                            .is_some()
                            || validation::is_closed_primitive_array_type(
                                program,
                                state.return_type,
                            )
                            || program
                                .primitive_type_reference(state.return_type)
                                .is_some()))
            }
            StatementNode::LocalData(local) => {
                program
                    .primitive_type_reference(local.type_reference)
                    .is_some()
                    || has_structural_result(program, facts, machine, statement)
                    || borrow_aliases
                        .iter()
                        .any(|alias| alias.owner == local.symbol)
            }
            _ => false,
        })
}

/// The statement position a prebuilt assignment store belongs to.
fn store_statement_index(store: &CheckedUnitEffectOperationPlan) -> Option<u32> {
    match store {
        CheckedUnitEffectOperationPlan::ByteSequenceWrite(store) => Some(store.statement_index),
        CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            statement_index, ..
        } => Some(*statement_index),
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) => {
            Some(store.statement_index)
        }
        CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(store) => {
            Some(store.statement_index)
        }
        CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(store) => {
            Some(store.statement_index)
        }
        _ => None,
    }
}

/// The machine an assignment's right-hand side calls, when it is a direct call.
fn statement_call_target(
    program: &TypedTrees,
    assignment: &typed_trees::statement::TableAssignment,
) -> Option<SymbolHandle> {
    let ExpressionNode::Call(call) = program.expression_table.expression(assignment.value) else {
        return None;
    };
    Some(call.target_symbol)
}

pub(in crate::execution::terminal_unit) fn build(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    entry_claims: &[CheckedUnitEntryClaimPlan],
    calls: &[&checked_trees::FlowCallFact],
    trivial_affine_locals: &[(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)],
    construction_statement_count: usize,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    trace: &LocalConstructionTrace,
) -> Option<StatementSequence> {
    trace.phase("statement sequence: scalar control guard");
    let scalar_control = scalar_control(program, facts, machine, state).map(|(control, _)| control);
    if scalar_control.is_some()
        && (!entry_claims.is_empty()
            || !trivial_affine_locals.is_empty()
            || structural_parameters.iter().any(|parameter| {
                parameter.multiplicity != Multiplicity::Unrestricted
                    || !parameter.qualifications.is_empty()
            }))
    {
        return None;
    }
    let mut operations = Vec::new();
    let mut local_count = construction_statement_count;
    // Erased borrow carriers have no storage binding: the sequence advances
    // past their declarations while each use's planner joins the captured
    // referent's checked loan evidence.
    let borrow_aliases =
        super::super::receiver_aliases::aliases(program, facts, machine, state).unwrap_or_default();
    let mut scalar_count = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(construction_statement_count)
        .filter(|statement| {
            matches!(statement,
            StatementNode::LocalData(local) if !local.is_mutable
                && program.primitive_type_reference(local.type_reference).is_some())
        })
        .count();
    let mut structural_count = 0_usize;
    let mut structural_local_symbols = Vec::new();
    let mut array_bindings = Vec::<(SymbolHandle, CheckedUnitStructuralResultBindingPlan)>::new();
    let mut returned_call = None;
    let mut returned_scalar_call = None;
    // Value bindings remain separate from the call's replayed claim custody.
    let mut structural_results = Vec::new();
    let mut call_count = 0_usize;
    let binders = machine_binders(program, machine);
    trace.phase("statement sequence: scalar field store sequence");
    let mut stores =
        super::super::structural_scalar_store::build_structural_scalar_field_store_sequence_traced(
            program,
            facts,
            machine,
            state,
            structural_parameters,
            scalar_parameters,
            construction_statement_count,
            call_frames,
            trace,
        )?
        .into_iter()
        .peekable();
    for (index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
        .skip(construction_statement_count)
        .take_while(|(index, statement)| {
            !matches!(statement, StatementNode::Transition(_))
                && !matches!(statement, StatementNode::LocalData(local) if local.name.as_str().starts_with("__arm_destructure#V="))
                && !matches!(statement, StatementNode::Expression(expression)
                    if !is_unit(program, state.return_type)
                        && (!matches!(program.expression_table.expression(*expression), ExpressionNode::Call(_))
                            || u32::try_from(*index).ok().is_some_and(|ordinal| facts.values.scalar_computations.root_at(
                                state.symbol, ordinal, CheckedScalarExpressionRole::Return,
                            ).is_some())))
        })
    {
        let statement_index = u32::try_from(index).ok()?;
        trace.phase(statement_phase(statement));
        trace.statement(Some(statement_index));
        append_reference_releases(facts, machine.symbol, state.symbol, statement_index, &mut operations)?;
        let completes_machine = matches!(statement, StatementNode::Expression(_))
            && !is_unit(program, state.return_type);
        let mut structural_result = None;
        // A store whose source is this statement's own call is appended after
        // that call operation, in ordinary evaluation order.
        let mut call_result_store = None;
        let result = match statement {
            StatementNode::Assignment(assignment) => {
                if let Some(store) = stores
                    .next_if(|store| store_statement_index(store) == Some(statement_index))
                {
                    operations.push(store);
                    continue;
                }
                // The store sequence deliberately left this assignment to the
                // ordinary call route: its right-hand side is the call this
                // statement performs, not an authored scalar value. Guard-group
                // markers beneath the statement phase keep the statement
                // position that `phase` resets.
                let assignment_phase = |phase: &'static str| {
                    trace.phase(phase);
                    trace.statement(Some(statement_index));
                };
                assignment_phase("statement sequence: assignment: pending store order");
                if match stores.peek() {
                    None => false,
                    Some(pending) => match store_statement_index(pending) {
                        None => true,
                        Some(ordinal) => ordinal <= statement_index,
                    },
                } {
                    return None;
                }
                assignment_phase("statement sequence: assignment: call source result type");
                let result_type = crate::flow::call_target_return_type(
                    program,
                    statement_call_target(program, assignment)?,
                )?;
                let primitive_type = program.primitive_type_reference(result_type)?;
                let binding_ordinal = u32::try_from(scalar_count).ok()?;
                let position = u32::try_from(scalar_parameters.len())
                    .ok()?
                    .checked_add(binding_ordinal)?;
                assignment_phase("statement sequence: assignment: call result field store");
                call_result_store = Some(
                    super::super::structural_scalar_store::build_structural_call_result_field_store(
                        program,
                        facts,
                        machine,
                        state,
                        structural_parameters,
                        scalar_parameters,
                        statement_index,
                        assignment,
                        (position, primitive_type),
                        trace,
                    )?,
                );
                scalar_count = scalar_count.checked_add(1)?;
                Some(CheckedUnitScalarResultBindingPlan {
                    statement_index,
                    binding_ordinal,
                    primitive_type,
                })
            }
            StatementNode::LocalData(local) => {
                // Guard-group markers beneath the local-data phase keep the
                // statement position that `phase` resets.
                let local_phase = |phase: &'static str| {
                    trace.phase(phase);
                    trace.statement(Some(statement_index));
                };
                local_phase("statement sequence: local data: initializer expression");
                if !program
                    .expression_table
                    .expression_is_valid(local.initial_value)
                {
                    return None;
                }
                if borrow_aliases
                    .iter()
                    .any(|alias| alias.owner == local.symbol)
                {
                    continue;
                }
                local_count = local_count.checked_add(1)?;
                if let Some(root) = facts.values.structural_values.root_at(state.symbol, statement_index) {
                    local_phase("statement sequence: local data: structural value: reference record loans");
                    if super::super::reference_results::is_reference_record(program, local.type_reference) {
                        super::super::reference_results::local_record_loans(program, facts, machine.symbol, state, statement_index)?;
                    }
                    local_phase("statement sequence: local data: structural value: root identity");
                    if root.machine != machine.symbol
                        || root.expression != local.initial_value || root.type_reference != local.type_reference {
                        return None;
                    }
                    local_phase("statement sequence: local data: structural value: value calls");
                    let calls = structural_operands::value_calls(program, facts, scalar_callees, shapes, machine,
                        state, structural_parameters, trivial_affine_locals, entry_claims, &structural_results,
                        &mut structural_count, root.root)?;
                    local_phase("statement sequence: local data: structural value: source custody");
                    if facts.flow.ownership.owned_selection_at(state.symbol, statement_index).is_some() {
                    retain_selected_sources(facts, state.symbol, statement_index, &structural_results, &mut operations)?;
                    } else {
                    consume_value_places(facts, root.root, &structural_results, &mut operations)?;
                    }
                    for call in &calls { consume_results(&mut operations, call.operation())?; }
                    local_phase("statement sequence: local data: structural value: type shape");
                    let result = CheckedUnitStructuralResultBindingPlan {
                        statement_index,
                        binding_ordinal: u32::try_from(structural_count).ok()?,
                        type_identity: shapes.add_type(local.type_reference, &binders, &[])?,
                        multiplicity: program.type_multiplicity(local.type_reference),
                    };
                    structural_count = structural_count.checked_add(1)?;
                    structural_results.push((result.clone(), facts::PlaceRoot::Symbol(local.symbol)));
                    structural_local_symbols.push(local.symbol);
                    // Only affine values create disposal debt; copy locals
                    // retain their result identity without a cleanup action.
                    let discard_result_on_return = result.multiplicity == Multiplicity::Affine;
                    operations.push(CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                        result, value: root.root, calls, discard_result_on_return,
                    });
                    continue;
                }
                if validation::is_closed_primitive_array_type(program, local.type_reference)
                    && !matches!(program.expression_table.expression(local.initial_value), ExpressionNode::Call(_)) {
                    local_phase("statement sequence: local data: scalar array: immutable binding");
                    if local.is_mutable {
                        return None;
                    }
                    let result = CheckedUnitStructuralResultBindingPlan {
                        statement_index,
                        binding_ordinal: u32::try_from(structural_count).ok()?,
                        type_identity: shapes.add_type(local.type_reference, &binders, &[])?,
                        multiplicity: Multiplicity::Unrestricted,
                    };
                    local_phase("statement sequence: local data: scalar array: elements");
                    let elements = super::scalar_arrays::elements(
                        program,
                        facts,
                        machine.symbol,
                        state.symbol,
                        statement_index,
                        checked_trees::CheckedArrayConstructionSource::Statement,
                        local.initial_value,
                        local.type_reference,
                    )?;
                    structural_count = structural_count.checked_add(1)?;
                    array_bindings.push((local.symbol, result.clone()));
                    structural_results.push((result.clone(), facts::PlaceRoot::Symbol(local.symbol)));
                    structural_local_symbols.push(local.symbol);
                    operations.push(CheckedUnitEffectOperationPlan::EstablishScalarArray {
                        source: checked_trees::CheckedArrayConstructionSource::Statement,
                        result,
                        elements,
                    });
                    continue;
                }
                if let Some(primitive_type) = program.primitive_type_reference(local.type_reference)
                {
                    if local.is_mutable {
                        local_phase("statement sequence: local data: mutable primitive local: named type");
                        if !matches!(
                            program
                                .type_reference_table
                                .type_reference(local.type_reference),
                            TypeReferenceNode::Named { .. }
                        ) {
                            return None;
                        }
                        local_phase("statement sequence: local data: mutable primitive local: storage initializer");
                        let (binding, value) =
                            facts.values.scalar_expressions.bound_expression_at(
                                state.symbol,
                                statement_index,
                                CheckedScalarExpressionRole::StorageInitializer,
                            )?;
                        if binding.expression != local.initial_value
                            || binding.destination != local.symbol
                            || crate::values::scalar_expression_type(value) != Some(primitive_type)
                            || !super::super::primitive_store::scalar_custody_is_exact(
                                program,
                                facts,
                                state,
                                binding,
                                value,
                                primitive_type,
                            )
                            || matches!(value, CheckedScalarExpression::Boolean(expression) if checked_boolean_contains_short_circuit(expression))
                        {
                            return None;
                        }
                        operations.push(CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal {
                            statement_index,
                            symbol: local.symbol,
                            type_identity: shapes.add_type(local.type_reference, &binders, &[])?,
                            primitive_type,
                            value: value.clone(),
                        });
                        continue;
                    }
                    let binding_ordinal = u32::try_from(scalar_count).ok()?;
                    scalar_count = scalar_count.checked_add(1)?;
                    if !matches!(
                        program.expression_table.expression(local.initial_value),
                        ExpressionNode::Call(_)
                    ) || facts.values.scalar_computations.root_at(
                        state.symbol,
                        statement_index,
                        CheckedScalarExpressionRole::LocalInitializer { binding_ordinal },
                    ).is_some() {
                        let (result, value) = scalar_computation_local_at(
                            program,
                            facts,
                            machine.symbol,
                            state,
                            statement_index,
                            binding_ordinal,
                            local,
                            trace,
                        )?;
                        operations.push(CheckedUnitEffectOperationPlan::EstablishScalarLocal {
                            result,
                            value,
                        });
                        continue;
                    }
                    local_phase("statement sequence: local data: scalar call binding");
                    Some(CheckedUnitScalarResultBindingPlan {
                        statement_index,
                        binding_ordinal,
                        primitive_type,
                    })
                } else {
                    local_phase("statement sequence: local data: structural call binding");
                    let (mut result, symbol) = checked_unit_structural_result_local(
                        program,
                        shapes,
                        std::slice::from_ref(statement),
                        &binders,
                    )?;
                    result.statement_index = statement_index;
                    result.binding_ordinal = u32::try_from(structural_count).ok()?;
                    structural_result = Some((result, Some(symbol)));
                    None
                }
            }
            StatementNode::Call(call) if call.discards_result => {
                trace.phase("statement sequence: call: discarded result type");
                trace.statement(Some(statement_index));
                let result_type =
                    crate::flow::call_target_return_type(program, call.target_symbol)?;
                if let Some(primitive_type) = program.primitive_type_reference(result_type) {
                    // Discarding the value does not discard the invocation or
                    // change its result signature. Keep the ordinary SSA result
                    // without introducing a source local.
                    let result = CheckedUnitScalarResultBindingPlan {
                        statement_index,
                        binding_ordinal: u32::try_from(scalar_count).ok()?,
                        primitive_type,
                    };
                    Some(result)
                } else if !is_unit(program, result_type) {
                    let mut result =
                        checked_structural_result_type(program, shapes, result_type, &binders)?;
                    result.statement_index = statement_index;
                    // No source local exists for an explicit discard. The call
                    // still produces its own typed result and disposal debt.
                    structural_result = Some((result, None));
                    None
                } else {
                    None
                }
            }
            StatementNode::Call(_) => None,
            StatementNode::Expression(_) if completes_machine
                && program.primitive_type_reference(state.return_type).is_some() =>
            {
                let result = CheckedUnitScalarResultBindingPlan {
                    statement_index,
                    binding_ordinal: u32::try_from(scalar_count).ok()?,
                    primitive_type: program.primitive_type_reference(state.return_type)?,
                };
                scalar_count = scalar_count.checked_add(1)?;
                returned_scalar_call = Some(result);
                Some(result)
            }
            StatementNode::Expression(_) if completes_machine => {
                let mut result = checked_structural_result_type(program, shapes, state.return_type, &binders)?;
                result.statement_index = statement_index;
                structural_result = Some((result, None));
                None
            }
            StatementNode::Expression(_)
                if call_occurrences::tail_call(program, state, index).is_some() =>
            {
                None
            }
            // A Unit-result call in expression position mid-body composes like
            // a call statement: its erased result binds nothing, and the
            // shared call roster below owns its operands and receiver.
            StatementNode::Expression(_)
                if call_occurrences::unit_statement_call(program, machine, state, index)
                    .is_some() =>
            {
                None
            }
            _ => return None,
        };
        // Guard-group markers beneath the statement phase keep the statement
        // position that `phase` resets.
        let call_phase = |phase: &'static str| {
            trace.phase(phase);
            trace.statement(Some(statement_index));
        };
        call_phase("statement sequence: call: flow call");
        let mut matching = calls
            .iter()
            .copied()
            .filter(|call| call.statement_index == index && call.call_ordinal == 0);
        let call = matching.next()?;
        if matching.next().is_some() {
            return None;
        }
        let authored_expression = match statement {
            StatementNode::LocalData(local) => Some(local.initial_value),
            StatementNode::Expression(expression) => Some(*expression),
            StatementNode::Assignment(assignment) => Some(assignment.value),
            _ => None,
        };
        if let Some(expression) = authored_expression {
            let ExpressionNode::Call(authored) = program.expression_table.expression(expression)
            else {
                return None;
            };
            if call.authored_expression != expression
                || call.target_symbol != authored.target_symbol
            {
                return None;
            }
        }
        call_count = call_count.checked_add(1)?;
        call_phase("statement sequence: call: structural operands");
        for operand in structural_operands::operations_for_call(program, facts, machine, state, call)? {
            let nested = match operand {
                structural_operands::Operand::Array(array) => {
                    let result = CheckedUnitStructuralResultBindingPlan {
                        statement_index,
                        binding_ordinal: u32::try_from(structural_count).ok()?,
                        type_identity: shapes.add_type(array.type_reference, &[], &[])?,
                        multiplicity: Multiplicity::Unrestricted,
                    };
                    let elements = super::scalar_arrays::elements(program, facts, machine.symbol,
                        state.symbol, statement_index, array.source, array.expression, array.type_reference)?;
                    structural_count = structural_count.checked_add(1)?;
                    structural_results.push((result.clone(), facts::PlaceRoot::Expression(array.expression)));
                    operations.push(CheckedUnitEffectOperationPlan::EstablishScalarArray {
                        source: array.source, result, elements,
                    });
                    continue;
                }
                structural_operands::Operand::Call(nested) => nested,
            };
            let target = structural_operands::result(
                program,
                facts,
                machine.symbol,
                nested.authored_expression,
                shapes,
            )?;
            let result = CheckedUnitStructuralResultBindingPlan {
                statement_index,
                binding_ordinal: u32::try_from(structural_count).ok()?,
                type_identity: target.type_identity,
                multiplicity: target.multiplicity,
            };
            let operation = build_call_operation(
                program,
                facts,
                Some(scalar_callees),
                machine,
                state,
                structural_parameters,
                trivial_affine_locals,
                entry_claims,
                nested,
                false,
                Some(ExpectedCallValueResult::Structural(&result)),
                &structural_results,
            )?;
            let operation = bind_structural_call_result(operation, result.clone())?;
            consume_results(&mut operations, &operation)?;
            operations.push(operation);
            structural_results.push((
                result,
                facts::PlaceRoot::Expression(nested.authored_expression),
            ));
            structural_count = structural_count.checked_add(1)?;
        }
        if let Some((result, _)) = &mut structural_result {
            result.binding_ordinal = u32::try_from(structural_count).ok()?;
        }
        // The existing sole-call partial-return route remains available to
        // native consumers. Wider statement schedules use a dying continuation;
        // every anonymous operand of this consumer owns one row in it.
        let anonymous_lane_claims = program
            .statement_table
            .statements(state.statement_nodes)
            .len()
            == 1
            && super::super::cleanup::anonymous::binding(program, facts, shapes, machine, state)
                .is_some();
        let partial_temporaries = if entry_claims.is_empty() && !anonymous_lane_claims {
            structural_results
                .iter()
                .filter(|(result, root)| {
                    result.statement_index == statement_index
                        && matches!(root, facts::PlaceRoot::Expression(_))
                })
                .filter_map(|(result, root)| {
                    let candidate = super::super::cleanup::anonymous::binding_at(
                        program,
                        facts,
                        shapes,
                        machine,
                        state,
                        index,
                        result.binding_ordinal,
                        *root,
                    )?;
                    (candidate.0 == *result && candidate.1 == *root).then_some(candidate)
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        call_phase("statement sequence: call: call operation");
        let mut operation = build_call_operation(
            program,
            facts,
            Some(scalar_callees),
            machine,
            state,
            structural_parameters,
            trivial_affine_locals,
            entry_claims,
            call,
            !partial_temporaries.is_empty(),
            result
                .as_ref()
                .map(|result| ExpectedCallValueResult::Scalar(result.primitive_type))
                .or_else(|| {
                    structural_result
                        .as_ref()
                        .map(|(result, _)| ExpectedCallValueResult::Structural(result))
                }),
            &structural_results,
        )?;
        call_phase("statement sequence: call: result binding");
        if let Some((result, None)) = &structural_result
            && !completes_machine {
            // An explicit discard still invokes the value-returning machine.
            // Its anonymous result cannot enter the named-local operand roster.
            // Dispose plain affine contents on this normal continuation, before
            // the next authored effect, rather than extending custody to return.
            operation = bind_structural_call_result(operation, result.clone())?;
            let (CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                coordinate,
                discard_result_on_return,
                ..
            }
            | CheckedUnitEffectOperationPlan::StructuralCall {
                coordinate,
                discard_result_on_return,
                ..
            }) = &mut operation
            else {
                return None;
            };
            *discard_result_on_return = false;
            let coordinate = *coordinate;
            consume_results(&mut operations, &operation)?;
            operations.push(operation);
            if result.multiplicity == Multiplicity::Affine {
                operations.push(CheckedUnitEffectOperationPlan::CallContinuationCleanup {
                    coordinate,
                    affine_discards: vec![CheckedUnitPartialAffineDiscardPlan {
                        source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal: result.binding_ordinal,
                        },
                        path: Vec::new(),
                        type_identity: result.type_identity.clone(),
                    }],
                });
            }
            structural_count = structural_count.checked_add(1)?;
            continue;
        }
        if let Some((result, symbol)) = structural_result {
            operation = bind_structural_call_result(operation, result.clone())?;
            if completes_machine {
                returned_call = Some(result.clone());
            }
            if let Some(symbol) = symbol {
                structural_local_symbols.push(symbol);
                if matches!(statement, StatementNode::LocalData(local)
                    if validation::is_closed_primitive_array_type(program, local.type_reference)) {
                    array_bindings.push((symbol, result.clone()));
                }
                if matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::StructuralCall { .. }
                        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { .. }
                )
                {
                    structural_results.push((result, facts::PlaceRoot::Symbol(symbol)));
                }
            }
            structural_count = structural_count.checked_add(1)?;
        }
        consume_results(&mut operations, &operation)?;
        operations.push(match result {
            // Returning the value directly uses the same call completion as a
            // local binding. Only the following return consumes that result;
            // a boundary crash establishes neither value nor normal exit.
            Some(result) => bind_scalar_call_result(facts, operation, result, true)?,
            None => operation,
        });
        // The store consuming this call's scalar result is its immediate
        // continuation: the result is live, and no cleanup separates the call
        // from the field it was called to fill.
        if let Some(store) = call_result_store {
            operations.push(store);
        }
        if !partial_temporaries.is_empty() {
            super::super::cleanup::anonymous::append_continuation(
                program,
                facts,
                shapes,
                machine,
                state,
                &partial_temporaries,
                &mut operations,
            )?;
        } else {
            append_call_cleanup(program, facts, machine.symbol, state, &mut operations, statement_index, &structural_results)?;
        }
    }
    trace.statement(None);
    trace.phase("statement sequence: structural return types");
    if operations.iter().any(|operation| {
        matches!(
            operation,
            CheckedUnitEffectOperationPlan::StructuralCall { .. }
        )
    }) {
        for plan in &facts.flow.terminal_structural_returns.structural_types {
            if shapes
                .types
                .get(&plan.identity)
                .is_some_and(|existing| existing != plan)
            {
                return None;
            }
            shapes.types.insert(plan.identity.clone(), plan.clone());
        }
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let returned_value = statements.len().checked_sub(1)
        .and_then(|ordinal| u32::try_from(ordinal).ok())
        .and_then(|ordinal| facts.values.structural_values.root_at(state.symbol, ordinal))
        .filter(|root| matches!(statements.last(), Some(StatementNode::Expression(expression)) if *expression == root.expression));
    let returned_local = statements.last().and_then(|statement| {
        let StatementNode::Expression(expression) = statement else {
            return None;
        };
        let ExpressionNode::Name(path) = program.expression_table.expression(*expression) else {
            return None;
        };
        if path.head_symbol != path.symbol
            || program
                .expression_table
                .name_path_members(path.members)
                .len()
                != 1
        {
            return None;
        }
        structural_results
            .iter()
            .find(|(_, source)| *source == facts::PlaceRoot::Symbol(path.symbol))
            .map(|(binding, _)| binding.clone())
    });
    let mut structural_result = if let Some(binding) = returned_local {
        trace.phase("statement sequence: structural result: returned local");
        if Some(binding.type_identity.as_str())
            != base_type_identity(program, state.return_type, &binders).as_deref()
            || binding.multiplicity != program.type_multiplicity(state.return_type)
        {
            return None;
        }
        let qualifications =
            parameter_qualifications(program, shapes, state.return_type, &binders)?;
        for operation in &mut operations {
            if let CheckedUnitEffectOperationPlan::StructuralCall {
                result, custody, ..
            } = operation
                && result.binding_ordinal == binding.binding_ordinal
                && custody.result_qualifications != qualifications
            {
                return None;
            }
            match operation {
                CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    result,
                    discard_result_on_return,
                    ..
                }
                | CheckedUnitEffectOperationPlan::StructuralCall {
                    result,
                    discard_result_on_return,
                    ..
                }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    result,
                    discard_result_on_return,
                    ..
                } if result.binding_ordinal == binding.binding_ordinal => {
                    // Only affine results have automatic disposal debt.
                    // Linear results retain the call's checked claim frontier.
                    if binding.multiplicity == Multiplicity::Affine && !*discard_result_on_return {
                        return None;
                    }
                    *discard_result_on_return = false;
                }
                _ => {}
            }
        }
        Some(binding.into())
    } else if let Some(result) = returned_parameter(
        program,
        facts,
        machine,
        state,
        structural_parameters,
        entry_claims,
    ) {
        // Completion can forward an existing formal directly. A retained Name
        // value describes possible materialization, not a requirement to create
        // a second binding and bypass the exact parameter-return custody owner.
        Some(result)
    } else if let Some(result) = returned_reference_leaf(program, state, structural_parameters) {
        Some(result)
    } else if let Some(root) = returned_value {
        trace.phase("statement sequence: structural result: returned value");
        if root.machine != machine.symbol || root.type_reference != state.return_type {
            return None;
        }
        let calls = structural_operands::value_calls(
            program,
            facts,
            scalar_callees,
            shapes,
            machine,
            state,
            structural_parameters,
            trivial_affine_locals,
            entry_claims,
            &structural_results,
            &mut structural_count,
            root.root,
        )?;
        if facts
            .flow
            .ownership
            .owned_selection_at(state.symbol, root.statement_ordinal)
            .is_some()
        {
            retain_selected_sources(
                facts,
                state.symbol,
                root.statement_ordinal,
                &structural_results,
                &mut operations,
            )?;
        } else {
            consume_value_places(facts, root.root, &structural_results, &mut operations)?;
        }
        for call in &calls {
            consume_results(&mut operations, call.operation())?;
        }
        let result = CheckedUnitStructuralResultBindingPlan {
            statement_index: root.statement_ordinal,
            binding_ordinal: u32::try_from(structural_count).ok()?,
            type_identity: shapes.add_type(state.return_type, &binders, &[])?,
            multiplicity: program.type_multiplicity(state.return_type),
        };
        operations.push(CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result: result.clone(),
            value: root.root,
            calls,
            discard_result_on_return: false,
        });
        let mut returned: CheckedUnitStructuralReturnPlan = result.into();
        if super::super::reference_results::is_reference_record(program, state.return_type) {
            returned.reference_sources =
                super::super::reference_results::returned_record_sources(program, state)?;
        }
        Some(returned)
    } else if let Some(binding) = returned_call {
        Some(binding.into())
    } else if validation::is_closed_primitive_array_type(program, state.return_type) {
        trace.phase("statement sequence: structural result: returned scalar array");
        let statements = program.statement_table.statements(state.statement_nodes);
        let StatementNode::Expression(expression) = statements.last()? else {
            return None;
        };
        let statement_index = u32::try_from(statements.len().checked_sub(1)?).ok()?;
        if let ExpressionNode::Name(path) = program.expression_table.expression(*expression) {
            if let Some((_, binding)) = array_bindings
                .iter()
                .find(|(symbol, _)| *symbol == path.symbol)
            {
                if binding.type_identity
                    != program.normalized_type_identity(state.return_type).as_str()
                {
                    return None;
                }
                Some(binding.clone().into())
            } else {
                return None;
            }
        } else {
            let elements = super::scalar_arrays::elements(
                program,
                facts,
                machine.symbol,
                state.symbol,
                statement_index,
                checked_trees::CheckedArrayConstructionSource::Statement,
                *expression,
                state.return_type,
            )?;
            let result = CheckedUnitStructuralResultBindingPlan {
                statement_index,
                binding_ordinal: u32::try_from(structural_count).ok()?,
                type_identity: shapes.add_type(state.return_type, &binders, &[])?,
                multiplicity: Multiplicity::Unrestricted,
            };
            operations.push(CheckedUnitEffectOperationPlan::EstablishScalarArray {
                source: checked_trees::CheckedArrayConstructionSource::Statement,
                result: result.clone(),
                elements,
            });
            Some(result.into())
        }
    } else {
        None
    };
    trace.phase("statement sequence: reference result");
    if let Some(result) = &mut structural_result
        && super::super::reference_results::parts(program, state.return_type).is_some()
        && let [reference] = result.reference_sources.as_slice()
    {
        let binding = CheckedUnitStructuralResultBindingPlan {
            statement_index: u32::try_from(
                program
                    .statement_table
                    .statements(state.statement_nodes)
                    .len()
                    .checked_sub(1)?,
            )
            .ok()?,
            binding_ordinal: u32::try_from(structural_count).ok()?,
            type_identity: shapes.add_reference_type(state.return_type, &binders)?,
            multiplicity: Multiplicity::Affine,
        };
        operations.push(CheckedUnitEffectOperationPlan::EstablishReference {
            result: binding.clone(),
            source: reference.source.clone(),
        });
        result.source = CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: binding.binding_ordinal,
        };
    }
    // Release each returned carrier at its actual checked weakening boundary,
    // before any scalar completion or following statement can reuse the parent.
    // An empty body cannot have established a call-result reference. Do not
    // require a last-statement coordinate for an ordinary empty helper/state.
    trace.phase("statement sequence: returned carrier releases");
    if let Some(statement_index) = program
        .statement_table
        .statements(state.statement_nodes)
        .len()
        .checked_sub(1)
    {
        append_reference_releases(
            facts,
            machine.symbol,
            state.symbol,
            u32::try_from(statement_index).ok()?,
            &mut operations,
        )?;
    }
    if scalar_control.is_none()
        && returned_scalar_call.is_none()
        && let Some(primitive_type) = program.primitive_type_reference(state.return_type)
    {
        trace.phase("statement sequence: scalar completion");
        let statements = program.statement_table.statements(state.statement_nodes);
        let StatementNode::Expression(expression) = statements.last()? else {
            return None;
        };
        let statement_index = u32::try_from(statements.len().checked_sub(1)?).ok()?;
        let role = CheckedScalarExpressionRole::Return;
        let computations = &facts.values.scalar_computations;
        let mut roots = computations
            .roots
            .iter()
            .map(|(_, root)| root)
            .filter(|root| {
                root.state == state.symbol
                    && root.statement_ordinal == statement_index
                    && root.role == role
            });
        let value = if let Some(root) = roots.next() {
            if roots.next().is_some()
                || root.machine != machine.symbol
                || !computations.nodes.is_valid(root.root)
                || computations.nodes.get(root.root).authored_root != *expression
                || computations.nodes.get(root.root).primitive_type != primitive_type
                || facts
                    .values
                    .scalar_expressions
                    .expression_at(state.symbol, statement_index, role)
                    .is_some()
            {
                return None;
            }
            checked_trees::CheckedCallScalarArgument::Computation(root.root)
        } else {
            let (binding, value) = facts.values.scalar_expressions.bound_expression_at(
                state.symbol,
                statement_index,
                role,
            )?;
            if binding.expression != *expression
                || crate::values::scalar_expression_type(value) != Some(primitive_type)
            {
                return None;
            }
            checked_trees::CheckedCallScalarArgument::Pure(value.clone())
        };
        let result = CheckedUnitScalarResultBindingPlan {
            statement_index,
            binding_ordinal: u32::try_from(scalar_count).ok()?,
            primitive_type,
        };
        // Completion evaluates its actual expression after the preceding
        // operations. A final name reuses its value without replaying its call.
        operations.push(CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, value });
        returned_scalar_call = Some(result);
    }
    trace.phase("statement sequence: scalar control ownership");
    if scalar_control.is_some()
        && operations.iter().any(|operation| {
            matches!(operation,
            CheckedUnitEffectOperationPlan::StructuralCall { result, .. }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, .. }
            | CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. }
                if result.multiplicity != Multiplicity::Unrestricted)
        })
    {
        return None;
    }
    trace.phase("statement sequence: call count agreement");
    (call_count == calls.len()).then_some(StatementSequence {
        scalar_result: returned_scalar_call,
        scalar_control,
        structural_result,
        operations,
        local_count,
        structural_local_symbols,
    })
}

/// Share scalar exit discovery while leaving operation storage with its sequence.
pub(in crate::execution::terminal_unit) fn scalar_control(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> Option<(checked_trees::CheckedUnitScalarControlPlan, usize)> {
    let primitive_type = program.primitive_type_reference(state.return_type)?;
    let statements = program.statement_table.statements(state.statement_nodes);
    let prefix_count = statements
        .iter()
        .take_while(|statement| {
            matches!(
                statement,
                StatementNode::LocalData(_) | StatementNode::Assignment(_) | StatementNode::Call(_)
            )
        })
        .count();
    let mut guarded = facts
        .flow
        .terminal_scalar_graphs
        .guarded_tails
        .iter()
        .filter(|tail| tail.state == state.symbol);
    // Keep the existing scalar binary route when it already describes this
    // completion; the shared roster also serves structural return consumers.
    let binary = crate::execution::terminal_scalar::checked_terminator(
        program,
        machine,
        state,
        &facts.values.scalar_expressions,
        prefix_count,
    );
    let terminator =
        if let Some(terminator @ checked_trees::CheckedScalarStateTerminator::Conditional { .. }) =
            binary
        {
            terminator
        } else if let Some(tail) = guarded.next() {
            if guarded.next().is_some() {
                return None;
            }
            let arms = facts
                .flow
                .terminal_scalar_graphs
                .guarded_exits
                .span(tail.arms)?;
            if arms.first()?.guard_statement_ordinal as usize != prefix_count
                || !arms.iter().all(|arm| {
                    matches!(
                        arm.destination,
                        checked_trees::CheckedScalarBranchDestination::Return { .. }
                    )
                })
                || tail.fallback.as_ref().is_some_and(|fallback| {
                    !matches!(
                        fallback,
                        checked_trees::CheckedScalarBranchDestination::Return { .. }
                    )
                })
            {
                return None;
            }
            checked_trees::CheckedScalarStateTerminator::Guarded {
                arms: tail.arms,
                fallback: tail.fallback.clone(),
            }
        } else {
            crate::execution::terminal_scalar::checked_terminator(
                program,
                machine,
                state,
                &facts.values.scalar_expressions,
                prefix_count,
            )?
        };
    let returning_tail = match &terminator {
        // Final expressions already belong to the scalar-result route, which
        // also supports ownership frontiers outside scalar-control admission.
        // Explicit unconditional transitions use the same retained return
        // coordinate as guarded control, without inventing a Boolean guard.
        checked_trees::CheckedScalarStateTerminator::Return { statement_ordinal } => matches!(
            statements.get(*statement_ordinal as usize),
            Some(StatementNode::Transition(_))
        ),
        checked_trees::CheckedScalarStateTerminator::Guarded { .. }
        | checked_trees::CheckedScalarStateTerminator::Conditional {
            when_true: checked_trees::CheckedScalarBranchDestination::Return { .. },
            when_false: checked_trees::CheckedScalarBranchDestination::Return { .. },
            ..
        } => true,
        _ => false,
    };
    if !returning_tail {
        return None;
    }
    Some((
        checked_trees::CheckedUnitScalarControlPlan {
            primitive_type,
            terminator,
        },
        prefix_count,
    ))
}

fn scalar_computation_local_at(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    binding_ordinal: u32,
    local: &typed_trees::statement::TableLocalData,
    trace: &LocalConstructionTrace,
) -> Option<(
    CheckedUnitScalarResultBindingPlan,
    checked_trees::CheckedCallScalarArgument,
)> {
    let role = CheckedScalarExpressionRole::LocalInitializer { binding_ordinal };
    let computations = &facts.values.scalar_computations;
    let mut roots = computations
        .roots
        .iter()
        .map(|(_, root)| root)
        .filter(|root| {
            root.state == state.symbol
                && root.statement_ordinal == statement_index
                && root.role == role
        });
    let Some(root) = roots.next() else {
        trace.phase("statement sequence: local data: scalar local: pure initializer");
        trace.statement(Some(statement_index));
        let (result, value) = scalar_expression_local_at(
            program,
            facts,
            state,
            statement_index,
            binding_ordinal,
            local,
        )?;
        return Some((
            result,
            checked_trees::CheckedCallScalarArgument::Pure(value),
        ));
    };
    trace.phase("statement sequence: local data: scalar local: computation initializer");
    trace.statement(Some(statement_index));
    let primitive_type = program.primitive_type_reference(local.type_reference)?;
    if roots.next().is_some()
        || root.machine != machine
        || local.is_mutable
        || !computations.nodes.is_valid(root.root)
        || computations.nodes.get(root.root).authored_root != local.initial_value
        || computations.nodes.get(root.root).primitive_type != primitive_type
        || facts
            .values
            .scalar_expressions
            .expression_at(state.symbol, statement_index, role)
            .is_some()
    {
        return None;
    }
    Some((
        CheckedUnitScalarResultBindingPlan {
            statement_index,
            binding_ordinal,
            primitive_type,
        },
        checked_trees::CheckedCallScalarArgument::Computation(root.root),
    ))
}

fn append_reference_releases(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: u32,
    operations: &mut Vec<CheckedUnitEffectOperationPlan>,
) -> Option<()> {
    let mut pending = Vec::new();
    for operation in operations.iter() {
        if let CheckedUnitEffectOperationPlan::StructuralCall {
            result, custody, ..
        } = operation
            && custody.reference_loan.is_valid()
        {
            let loan = custody.reference_loan;
            let boundary =
                super::super::reference_results::release_statement(facts, machine, state, loan)?;
            if boundary == statement_index && !operations.iter().any(|operation| {
                matches!(operation, CheckedUnitEffectOperationPlan::ReleaseReference { loan: released, .. } if *released == loan)
            }) {
                pending.push(CheckedUnitEffectOperationPlan::ReleaseReference {
                    statement_index, binding_ordinal: result.binding_ordinal, loan,
                });
            }
        }
    }
    operations.extend(pending);
    Some(())
}

#[allow(clippy::too_many_arguments)]
fn append_call_cleanup(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    operations: &mut Vec<CheckedUnitEffectOperationPlan>,
    statement_index: u32,
    results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
) -> Option<()> {
    let Some(CheckedUnitEffectOperationPlan::CallUnit {
        coordinate,
        structural_arguments,
        ..
    }) = operations.last()
    else {
        return Some(());
    };
    let coordinate = *coordinate;
    let mut discards = Vec::new();
    for (result, root) in results.iter().rev() {
        let reference_record = matches!(root, facts::PlaceRoot::Symbol(_))
            && super::super::reference_results::local_record_loans(
                program,
                facts,
                machine,
                state,
                result.statement_index,
            )
            .is_some_and(|loans| {
                !loans.is_empty()
                    && loans.iter().all(|(_, loan)| {
                        super::super::reference_results::release_statement(
                            facts,
                            machine,
                            state.symbol,
                            *loan,
                        ) == statement_index.checked_add(1)
                    })
            })
            && structural_arguments.iter().any(|argument| {
                argument.source_structural_result_binding_ordinal() == Some(result.binding_ordinal)
                    && argument.path.last() == Some(&CheckedUnitStructuralPathSegment::Referent)
            });
        if !reference_record
            && (!matches!(root, facts::PlaceRoot::Expression(_))
                || result.statement_index != statement_index
                || !structural_arguments.iter().any(|argument| {
                    argument.source_structural_result_binding_ordinal()
                        == Some(result.binding_ordinal)
                        && argument.access == CheckedStructuralAccess::SharedBorrow
                }))
        {
            continue;
        }
        discards.push(checked_trees::CheckedUnitPartialAffineDiscardPlan {
            source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            },
            path: Vec::new(),
            type_identity: result.type_identity.clone(),
        });
    }
    if discards.is_empty() {
        return Some(());
    }
    for discard in &discards {
        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal,
        } = discard.source
        else {
            return None;
        };
        let producer = operations.iter_mut().find(|operation| {
            matches!(operation,
            CheckedUnitEffectOperationPlan::StructuralCall { result, .. }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, .. }
            | CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. }
                if result.binding_ordinal == binding_ordinal)
        })?;
        let (CheckedUnitEffectOperationPlan::StructuralCall {
            discard_result_on_return,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            discard_result_on_return,
            ..
        }
        | CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            discard_result_on_return,
            ..
        }) = producer
        else {
            return None;
        };
        if !*discard_result_on_return {
            return None;
        }
        *discard_result_on_return = false;
    }
    operations.push(CheckedUnitEffectOperationPlan::CallContinuationCleanup {
        coordinate,
        affine_discards: discards,
    });
    Some(())
}

/// A selected continuation owns both the chosen result and the conditional
/// complement until the actual death edge. Original producers must not also
/// publish unconditional return drops for those same owners.
fn retain_selected_sources(
    facts: &CheckFacts,
    state: SymbolHandle,
    statement: u32,
    sources: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    operations: &mut [CheckedUnitEffectOperationPlan],
) -> Option<()> {
    let Some((_, receipt)) = facts.flow.ownership.owned_selection_at(state, statement) else {
        return Some(());
    };
    for source in facts
        .flow
        .ownership
        .selection_sources
        .span(receipt.sources)?
    {
        // A state-entry source (an owned parameter) has no producing
        // operation to unmark: its residual custody already rides the
        // receipt's selected edges instead of an unconditional return drop.
        if matches!(
            source.provenance,
            PermissionProvenance::Established {
                source: PermissionEventSource::StateEntry,
                ..
            }
        ) {
            continue;
        }
        let (binding, _) = sources
            .iter()
            .find(|(_, root)| *root == facts::PlaceRoot::Symbol(source.symbol))?;
        let mut producers = operations
            .iter_mut()
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    result,
                    discard_result_on_return,
                    ..
                }
                | CheckedUnitEffectOperationPlan::StructuralCall {
                    result,
                    discard_result_on_return,
                    ..
                }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    result,
                    discard_result_on_return,
                    ..
                } if result.binding_ordinal == binding.binding_ordinal => {
                    Some((result, discard_result_on_return))
                }
                _ => None,
            });
        let (result, discard) = producers.next()?;
        if producers.next().is_some()
            || result.multiplicity != Multiplicity::Affine
            || result.statement_index != source.statement_ordinal
            || !*discard
        {
            return None;
        }
        *discard = false;
    }
    Some(())
}

pub(super) fn consume_results(
    operations: &mut [CheckedUnitEffectOperationPlan],
    consumer: &CheckedUnitEffectOperationPlan,
) -> Option<()> {
    if let CheckedUnitEffectOperationPlan::StructuralCall {
        structural_arguments,
        ..
    }
    | CheckedUnitEffectOperationPlan::CallUnit {
        structural_arguments,
        ..
    }
    | CheckedUnitEffectOperationPlan::ScalarCall {
        structural_arguments,
        ..
    }
    | CheckedUnitEffectOperationPlan::BoundaryCall {
        structural_arguments,
        ..
    }
    | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
        structural_arguments,
        ..
    }
    | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
        structural_arguments,
        ..
    } = consumer
    {
        for (binding_ordinal, access, projected) in
            structural_arguments.iter().filter_map(|argument| {
                argument
                    .source_structural_result_binding_ordinal()
                    .map(|ordinal| (ordinal, argument.access, !argument.path.is_empty()))
            })
        {
            consume_result(operations, binding_ordinal, access, projected)?;
        }
    }
    Some(())
}

fn consume_result(
    operations: &mut [CheckedUnitEffectOperationPlan],
    binding_ordinal: u32,
    access: CheckedStructuralAccess,
    projected: bool,
) -> Option<()> {
    let mut producers = operations.iter_mut().filter(|operation| {
        matches!(operation,
                CheckedUnitEffectOperationPlan::StructuralCall { result, .. }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, .. }
                | CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. }
                | CheckedUnitEffectOperationPlan::EstablishScalarArray { result, .. }
                    if result.binding_ordinal == binding_ordinal)
    });
    let producer = producers.next()?;
    if producers.next().is_some() {
        return None;
    }
    if let CheckedUnitEffectOperationPlan::StructuralCall { custody, .. } = producer
        && custody.reference_loan.is_valid()
    {
        return (projected && access == CheckedStructuralAccess::MutableBorrow).then_some(());
    }
    // Unrestricted whole-value arguments copy their payload. They retain
    // a live producer without acquiring an affine disposal obligation.
    if matches!(producer,
                CheckedUnitEffectOperationPlan::EstablishScalarArray { result, .. }
                | CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. }
                | CheckedUnitEffectOperationPlan::StructuralCall { result, .. }
                    if result.multiplicity == Multiplicity::Unrestricted)
    {
        if access == CheckedStructuralAccess::Owned && projected {
            return None;
        }
        return Some(());
    }
    let (CheckedUnitEffectOperationPlan::StructuralCall {
        discard_result_on_return,
        result,
        ..
    }
    | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
        discard_result_on_return,
        result,
        ..
    }
    | CheckedUnitEffectOperationPlan::EstablishStructuralValue {
        discard_result_on_return,
        result,
        ..
    }) = producer
    else {
        return None;
    };
    if result.multiplicity != Multiplicity::Affine || !*discard_result_on_return {
        // Linear results never acquire automatic disposal debt. Moving a
        // whole result continues its checked call claim frontier; source flow
        // and independent Terminal replay reject a second use of that claim.
        if result.multiplicity == Multiplicity::Linear
            && !*discard_result_on_return
            && access == CheckedStructuralAccess::Owned
            && !projected
        {
            return Some(());
        }
        return None;
    }
    match access {
        // A projected transfer leaves its root owner alive until the
        // exact complement is committed on the consumer continuation.
        CheckedStructuralAccess::Owned if projected => {}
        CheckedStructuralAccess::Owned => *discard_result_on_return = false,
        CheckedStructuralAccess::SharedBorrow
        | CheckedStructuralAccess::MutableBorrow
        | CheckedStructuralAccess::WriteOnlyBorrow => {}
    }
    Some(())
}

fn consume_value_places(
    facts: &CheckFacts,
    root: checked_trees::CheckedStructuralValueHandle,
    results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    operations: &mut [CheckedUnitEffectOperationPlan],
) -> Option<()> {
    let plans = &facts.values.structural_values;
    let mut pending = vec![root];
    let mut visited = Vec::new();
    while let Some(value) = pending.pop() {
        if !plans.nodes.is_valid(value) || visited.contains(&value) {
            return None;
        }
        visited.push(value);
        match &plans.nodes.get(value).kind {
            checked_trees::CheckedStructuralValueKind::Place(argument) => {
                if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol,
                } = argument.source
                {
                    let mut matching = results
                        .iter()
                        .filter(|(_, source)| *source == facts::PlaceRoot::Symbol(symbol));
                    let (result, _) = matching.next()?;
                    if matching.next().is_some() || result.type_identity != argument.type_identity {
                        return None;
                    }
                    consume_result(
                        operations,
                        result.binding_ordinal,
                        argument.access,
                        !argument.path.is_empty(),
                    )?;
                }
            }
            checked_trees::CheckedStructuralValueKind::Record { fields, .. } => {
                for field in plans.record_fields.span(*fields)? {
                    if let checked_trees::CheckedStructuralRecordFieldValue::Structural(value) =
                        field.value
                    {
                        pending.push(value);
                    }
                }
            }
            checked_trees::CheckedStructuralValueKind::Dispatch { arms, .. } => {
                pending.extend(plans.dispatch_arms.span(*arms)?.iter().map(|arm| arm.value));
            }
            checked_trees::CheckedStructuralValueKind::Projection { source, .. } => {
                // The selected edge moves a projected child: its root owner
                // remains live for the residual complement that edge commits.
                if let checked_trees::CheckedStructuralValueKind::Place(argument) =
                    &plans.nodes.get(*source).kind
                    && let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                        symbol,
                    } = argument.source
                {
                    let mut matching = results
                        .iter()
                        .filter(|(_, source)| *source == facts::PlaceRoot::Symbol(symbol));
                    let (result, _) = matching.next()?;
                    if matching.next().is_some() || result.type_identity != argument.type_identity {
                        return None;
                    }
                    consume_result(operations, result.binding_ordinal, argument.access, true)?;
                }
            }
            checked_trees::CheckedStructuralValueKind::Reference { .. }
            | checked_trees::CheckedStructuralValueKind::Call { .. }
            | checked_trees::CheckedStructuralValueKind::Case(_) => {}
        }
    }
    Some(())
}
