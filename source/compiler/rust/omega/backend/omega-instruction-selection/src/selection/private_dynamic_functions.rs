use crate::InstructionSelectionInput;
use omega_abstract_operations::{
    AbstractFunctionPlan, AbstractOperation, AbstractOperationKind, AbstractOperationPlan,
    PermissionRealizationCandidate, Place, PlaceStep, RuntimeStorageRegion,
};
use omega_calling_conventions::ValueLocation;
use omega_control_flow::{MachineFunctionIdentity, StateKey};
use psi_checked_trees::CheckedTrees;
use psi_diagnostics::Diagnostic;
use std::sync::Arc;

use super::bindings::RuntimeAliasBuffer;
use super::instruction_sink::SelectedInstructionSink;
use super::state_bodies::{StateBodyVisitStack, select_state_body_instructions};

#[derive(Debug, Clone, PartialEq, Eq)]
struct PrivateDynamicRealizationDemand {
    realization_identity: Arc<str>,
    realization: StateKey,
}

pub(super) fn select_private_dynamic_realization_functions(
    input: &InstructionSelectionInput<'_>,
    plan: &mut AbstractOperationPlan,
    permission_realization_candidates: &mut Vec<PermissionRealizationCandidate>,
    boundary_footprints: &mut omega_abstract_operations::BoundaryFootprintPlan,
    entry_boundary: Option<&omega_calling_conventions::ValidatedBoundaryEntryPlan>,
) -> Result<(), Diagnostic> {
    let demands = validated_private_dynamic_realization_demands(input)?;
    let entry_identity = MachineFunctionIdentity::source(input.entry_key);

    for demand in demands {
        let identity = MachineFunctionIdentity::source(demand.realization);
        if identity == entry_identity {
            continue;
        }
        if plan
            .code
            .functions
            .iter()
            .any(|(_, function)| function.identity == identity)
        {
            return Err(Diagnostic::error(format!(
                "dynamic conformance realization `{}` duplicates compiler-private function identity {identity:?}",
                demand.realization_identity,
            )));
        }

        let mut selected =
            SelectedInstructionSink::new(&mut plan.code.instructions, input.control_flow);
        selected.push(function_boundary_instruction(
            demand.realization,
            AbstractOperationKind::EnterFunction,
        ));
        let prologue_footprint =
            select_private_dynamic_realization_prologue(input, demand.realization, &mut selected)?;
        let boundary = entry_boundary.ok_or_else(|| {
            Diagnostic::error(
                "private dynamic realization has no enclosing root StatePlan boundary",
            )
        })?;
        omega_calling_conventions::validate_state_footprint(boundary, &prologue_footprint)
            .map_err(|error| Diagnostic::error(error.0))?;
        if !prologue_footprint.registers().as_slice().is_empty()
            || !prologue_footprint.machine_state().is_empty()
        {
            boundary_footprints
                .retain_validated_fragment(
                    boundary,
                    omega_abstract_operations::BoundaryFootprintFragment {
                        origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::EntryStorage,
                        evidence: prologue_footprint,
                    },
                )
                .map_err(|error| Diagnostic::error(error.0))?;
        }
        select_state_body_instructions(
            input,
            demand.realization,
            Some(exact_runtime_body_dispatch_index(
                input,
                demand.realization,
            )?),
            &RuntimeAliasBuffer::default(),
            &psi_checked_trees::expression::ExpressionTable::new(),
            &mut plan.code.operands,
            &mut plan.code.runtime_value_operands,
            &mut selected,
            &mut StateBodyVisitStack::with_capacity(input.control_flow.states.len()),
        );
        select_private_dynamic_realization_result(input, demand.realization, &mut selected)?;
        selected.push(function_boundary_instruction(
            demand.realization,
            AbstractOperationKind::LeaveFunction,
        ));
        let (instructions, candidates) = selected.finish();
        permission_realization_candidates.extend(candidates);
        let function_instructions = plan.code.instructions.span(instructions).ok_or_else(|| {
            Diagnostic::error(
                "private dynamic realization lost its exact selected-instruction span",
            )
        })?;
        super::retain_exit_footprints(
            boundary_footprints,
            entry_boundary,
            input,
            &plan.code.operands,
            &plan.code.runtime_value_operands,
            function_instructions,
        )?;

        let symbol =
            omega_object_file::private_function_symbol_name(identity).ok_or_else(|| {
                Diagnostic::error(format!(
                    "dynamic conformance realization `{}` has no canonical private function symbol",
                    demand.realization_identity,
                ))
            })?;
        plan.code.functions.insert(AbstractFunctionPlan {
            symbol: Arc::from(symbol),
            identity,
            instructions,
        });
    }

    Ok(())
}

fn select_private_dynamic_realization_prologue(
    input: &InstructionSelectionInput<'_>,
    realization: StateKey,
    selected: &mut SelectedInstructionSink<'_, '_>,
) -> Result<omega_calling_conventions::StateFootprintEvidence, Diagnostic> {
    let plan = super::dynamic_calls::dynamic_call_plan_for_realization(input, realization)?;
    let dispatch_index = exact_runtime_body_dispatch_index(input, realization)?;
    let receiver = unique_dynamic_slot(
        input,
        dispatch_index,
        realization,
        |slot| matches!(slot.kind, omega_runtime_storage::RuntimeFrameSlotKind::DynamicReceiver { realization: key } if key == realization),
    )?;
    let state = input
        .control_flow
        .state_by_key(realization)
        .ok_or_else(|| {
            Diagnostic::error("private dynamic realization has no exact control-flow state")
        })?;
    let mut destinations = vec![receiver];
    for parameter in input.control_flow.state_parameters(state) {
        let slot = unique_dynamic_slot(input, dispatch_index, realization, |slot| {
            matches!(
                slot.kind,
                omega_runtime_storage::RuntimeFrameSlotKind::Parameter
            ) && slot.symbol == parameter.symbol
        })?;
        destinations.push(slot);
    }
    if destinations.len() != plan.parameters.len() {
        return Err(Diagnostic::error(
            "private dynamic realization inbound storage does not match its calling plan",
        ));
    }
    let parameter_destinations = plan
        .parameters
        .iter()
        .zip(destinations)
        .map(|(placement, destination)| {
            if destination.byte_size != usize::from(placement.shape.byte_size) {
                return Err(Diagnostic::error(
                    "private dynamic realization parameter storage width drifted from its call plan",
                ));
            }
            Ok((destination.byte_offset, placement.shape))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let derived = crate::derive_internal_call_entry_storage(&plan, &parameter_destinations, None)
        .map_err(|error| Diagnostic::error(error.0))?;
    for kind in derived.writes {
        if matches!(
            kind,
            AbstractOperationKind::WriteEntryIndirectArgument { .. }
        ) {
            return Err(Diagnostic::error(
                "private dynamic realization currently rejects indirect aggregate parameters",
            ));
        }
        selected.push(function_boundary_instruction(realization, kind));
    }
    Ok(derived.footprint)
}

fn select_private_dynamic_realization_result(
    input: &InstructionSelectionInput<'_>,
    realization: StateKey,
    selected: &mut SelectedInstructionSink<'_, '_>,
) -> Result<(), Diagnostic> {
    let plan = super::dynamic_calls::dynamic_call_plan_for_realization(input, realization)?;
    let Some(result) = &plan.result else {
        return Ok(());
    };
    let [
        ValueLocation::Register {
            register,
            value_byte_offset: 0,
            byte_size,
        },
    ] = result.locations.as_slice()
    else {
        return Err(Diagnostic::error(
            "private dynamic realization currently requires one direct scalar result register",
        ));
    };
    let expression =
        private_realization_terminal_expression(input, realization).ok_or_else(|| {
            Diagnostic::error("private dynamic realization has no exact terminal result expression")
        })?;
    if let Some(value) = super::storage_places::static_integer_value_in_table(
        input.layouts,
        &input.program.expression_table,
        expression,
    ) {
        selected.push(function_boundary_instruction(
            realization,
            AbstractOperationKind::WriteReturnRegisterInteger {
                register: *register,
                byte_size: usize::from(*byte_size),
                value,
            },
        ));
        return Ok(());
    }
    let dispatch_index = exact_runtime_body_dispatch_index(input, realization)?;
    if let Some(pointee) = super::storage_places::resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        realization,
        &input.program.expression_table,
        expression,
    ) {
        if pointee.pointee_byte_size != usize::from(*byte_size) {
            return Err(Diagnostic::error(
                "private dynamic scalar result width does not match its calling plan",
            ));
        }
        let scratch = unique_dynamic_slot(
            input,
            dispatch_index,
            realization,
            |slot| matches!(slot.kind, omega_runtime_storage::RuntimeFrameSlotKind::DynamicResultScratch { realization: key } if key == realization),
        )?;
        let source = Place::at(
            RuntimeStorageRegion::RuntimeFrame,
            pointee.pointer_byte_offset,
        )
        .with_step(PlaceStep::Deref)
        .and_then(|place| place.with_step(PlaceStep::ConstOffset(pointee.field_byte_offset)))
        .ok_or_else(|| Diagnostic::error("private dynamic receiver result place is too complex"))?;
        selected.push(function_boundary_instruction(
            realization,
            AbstractOperationKind::CopyPlaces {
                source,
                target: Place::at(RuntimeStorageRegion::RuntimeFrame, scratch.byte_offset),
                byte_count: pointee.pointee_byte_size,
                role: omega_abstract_operations::CopyPlacesRole::Ordinary,
            },
        ));
        selected.push(function_boundary_instruction(
            realization,
            AbstractOperationKind::CopyRuntimeStorageToReturnRegister {
                register: *register,
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: scratch.byte_offset,
                byte_size: pointee.pointee_byte_size,
            },
        ));
        return Ok(());
    }
    if let Some(place) = super::storage_places::resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        realization,
        &input.program.expression_table,
        expression,
    ) && place.byte_count == usize::from(*byte_size)
    {
        let receiver = unique_dynamic_slot(
            input,
            dispatch_index,
            realization,
            |slot| matches!(slot.kind, omega_runtime_storage::RuntimeFrameSlotKind::DynamicReceiver { realization: key } if key == realization),
        )?;
        if place.region == RuntimeStorageRegion::RuntimeFrame
            && place.byte_offset >= receiver.byte_offset
            && let Some(field_byte_offset) = place.byte_offset.checked_sub(receiver.byte_offset)
            && let Some(data_layout) = input
                .layouts
                .data_layouts
                .iter()
                .find(|(_, data)| data.symbol == receiver.type_symbol)
                .map(|(_, data)| data)
            && field_byte_offset
                .checked_add(place.byte_count)
                .is_some_and(|end| end <= data_layout.layout.size)
        {
            let scratch = unique_dynamic_slot(
                input,
                dispatch_index,
                realization,
                |slot| matches!(slot.kind, omega_runtime_storage::RuntimeFrameSlotKind::DynamicResultScratch { realization: key } if key == realization),
            )?;
            let source = Place::at(RuntimeStorageRegion::RuntimeFrame, receiver.byte_offset)
                .with_step(PlaceStep::Deref)
                .and_then(|place| place.with_step(PlaceStep::ConstOffset(field_byte_offset)))
                .ok_or_else(|| {
                    Diagnostic::error("private dynamic receiver result place is too complex")
                })?;
            selected.push(function_boundary_instruction(
                realization,
                AbstractOperationKind::CopyPlaces {
                    source,
                    target: Place::at(RuntimeStorageRegion::RuntimeFrame, scratch.byte_offset),
                    byte_count: place.byte_count,
                    role: omega_abstract_operations::CopyPlacesRole::Ordinary,
                },
            ));
            selected.push(function_boundary_instruction(
                realization,
                AbstractOperationKind::CopyRuntimeStorageToReturnRegister {
                    register: *register,
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: scratch.byte_offset,
                    byte_size: place.byte_count,
                },
            ));
            return Ok(());
        }
        selected.push(function_boundary_instruction(
            realization,
            AbstractOperationKind::CopyRuntimeStorageToReturnRegister {
                register: *register,
                region: place.region,
                byte_offset: place.byte_offset,
                byte_size: place.byte_count,
            },
        ));
        return Ok(());
    }
    Err(Diagnostic::error(
        "private dynamic scalar result is not a resolvable receiver projection",
    ))
}

fn exact_runtime_body_dispatch_index(
    input: &InstructionSelectionInput<'_>,
    realization: StateKey,
) -> Result<u32, Diagnostic> {
    let mut slots = input
        .runtime_storage
        .frame_slots
        .iter()
        .filter_map(|(_, slot)| {
            matches!(
                slot.kind,
                omega_runtime_storage::RuntimeFrameSlotKind::DynamicReceiver {
                    realization: key
                } if key == realization
            )
            .then_some(slot.dispatch_index)
        });
    let dispatch_index = slots
        .next()
        .ok_or_else(|| Diagnostic::error("private dynamic realization has no storage namespace"))?;
    if slots.next().is_some() {
        return Err(Diagnostic::error(
            "private dynamic realization has duplicate storage namespaces",
        ));
    }
    Ok(dispatch_index)
}

fn unique_dynamic_slot<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    dispatch_index: u32,
    realization: StateKey,
    slot_matches: impl Fn(&omega_runtime_storage::RuntimeFrameSlot) -> bool,
) -> Result<&'plan omega_runtime_storage::RuntimeFrameSlot, Diagnostic> {
    let mut slots = input
        .runtime_storage
        .frame_slots
        .iter()
        .filter_map(|(_, slot)| {
            (slot.dispatch_index == dispatch_index
                && slot.source_key == realization
                && slot_matches(slot))
            .then_some(slot)
        });
    let slot = slots
        .next()
        .ok_or_else(|| Diagnostic::error("private dynamic realization frame slot is missing"))?;
    if slots.next().is_some() {
        return Err(Diagnostic::error(
            "private dynamic realization frame slot is duplicated",
        ));
    }
    Ok(slot)
}

fn private_realization_terminal_expression(
    input: &InstructionSelectionInput<'_>,
    realization: StateKey,
) -> Option<psi_checked_trees::expression::ExpressionHandle> {
    use psi_checked_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
    let machine = input
        .program
        .machines()
        .iter()
        .find(|machine| machine.symbol == realization.machine)?;
    let state = input
        .program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == realization.state)?;
    match input
        .program
        .statement_table
        .statements(state.statement_nodes)
        .last()?
    {
        StatementNode::Expression(expression) => expression.is_valid().then_some(*expression),
        StatementNode::Transition(transition)
            if !transition.continuation.is_valid()
                && matches!(transition.guard, TransitionGuardNode::Always) =>
        {
            match input
                .program
                .statement_table
                .transition_target(transition.target)
            {
                TransitionTargetNode::Value(expression) => {
                    expression.is_valid().then_some(*expression)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn validated_private_dynamic_realization_demands(
    input: &InstructionSelectionInput<'_>,
) -> Result<Vec<PrivateDynamicRealizationDemand>, Diagnostic> {
    let mut retained_tables = Vec::new();
    let mut demands: Vec<PrivateDynamicRealizationDemand> = Vec::new();

    for (_, table) in input.data.dynamic_conformance_tables.iter() {
        let table_identity = (
            table.target_trait,
            table.conformance,
            Arc::clone(&table.trait_identity),
            Arc::clone(&table.conformance_identity),
        );
        if retained_tables.contains(&table_identity) {
            return Err(Diagnostic::error(
                "dynamic conformance private-function planning received a duplicate exact table demand",
            ));
        }
        retained_tables.push(table_identity);

        let expected_rows = validated_checked_table_rows(input.program, table)?;
        if table.rows.len() != expected_rows.len()
            || table
                .rows
                .iter()
                .zip(&expected_rows)
                .any(|(row, expected)| {
                    row.requirement_identity.as_ref() != expected.0
                        || row.realization_identity.as_ref() != expected.1
                        || row.realization != expected.2
                })
        {
            return Err(Diagnostic::error(
                "dynamic conformance private-function demand does not match its exact checked table rows",
            ));
        }

        for row in &table.rows {
            let control_flow_match_count = input
                .control_flow
                .states
                .iter()
                .filter(|(_, state)| state.key == row.realization)
                .count();
            if control_flow_match_count != 1 {
                return Err(Diagnostic::error(format!(
                    "dynamic conformance realization `{}` names {control_flow_match_count} exact control-flow states",
                    row.realization_identity,
                )));
            }

            retain_realization_demand(
                &mut demands,
                Arc::clone(&row.realization_identity),
                row.realization,
            )?;
        }
    }

    Ok(demands)
}

fn validated_checked_table_rows(
    program: &CheckedTrees,
    table: &omega_abstract_operations::AbstractDynamicConformanceTable,
) -> Result<Vec<(String, String, StateKey)>, Diagnostic> {
    let mut traits = program
        .traits()
        .iter()
        .filter(|definition| definition.symbol == table.target_trait);
    let target_trait = traits.next().ok_or_else(|| {
        Diagnostic::error("dynamic conformance private-function trait identity is missing")
    })?;
    if traits.next().is_some()
        || target_trait.is_boundary
        || program.symbols.display_path(table.target_trait, "::") != table.trait_identity.as_ref()
    {
        return Err(Diagnostic::error(
            "dynamic conformance private-function trait identity is duplicated or mismatched",
        ));
    }

    let mut conformances = program
        .conformances()
        .iter()
        .filter(|conformance| conformance.symbol == table.conformance);
    let conformance = conformances.next().ok_or_else(|| {
        Diagnostic::error("dynamic conformance private-function conformance identity is missing")
    })?;
    if conformances.next().is_some()
        || conformance.trait_symbol != table.target_trait
        || program.symbols.display_path(table.conformance, "::")
            != table.conformance_identity.as_ref()
    {
        return Err(Diagnostic::error(
            "dynamic conformance private-function conformance identity is duplicated or mismatched",
        ));
    }

    let declared_rows = program
        .closed_conformance_rows(conformance)
        .ok_or_else(|| {
            Diagnostic::error("dynamic conformance private-function table is not closed")
        })?;
    let mut rows = Vec::new();
    for declared in declared_rows {
        let declaring_trait = exact_trait(program, declared.declaring_trait)?;
        let requirement = program
            .trait_machine_signatures(declaring_trait)
            .iter()
            .filter(|requirement| requirement.symbol == declared.requirement)
            .collect::<Vec<_>>();
        let [requirement] = requirement.as_slice() else {
            return Err(Diagnostic::error(
                "dynamic conformance private-function requirement identity is missing or duplicated",
            ));
        };
        if program
            .dynamic_signature_eligibility(declaring_trait, requirement)
            .is_err()
        {
            continue;
        }
        let realization = exact_machine(program, declared.realization_machine)?;
        let realization_identity = program
            .normalized_machine_overload_identity(realization)
            .ok_or_else(|| {
                Diagnostic::error(
                    "dynamic conformance private-function realization has no normalized callable identity",
                )
            })?
            .identity();
        if program
            .machine_states(realization)
            .iter()
            .filter(|state| state.symbol == declared.realization_state)
            .count()
            != 1
        {
            return Err(Diagnostic::error(
                "dynamic conformance private-function realization state is missing or duplicated",
            ));
        }
        rows.push((
            program
                .normalized_trait_requirement_overload_identity(declaring_trait, requirement)
                .identity(),
            realization_identity,
            StateKey {
                machine: declared.realization_machine,
                state: declared.realization_state,
                segment_index: 0,
            },
        ));
    }
    rows.sort_by(|left, right| left.0.cmp(&right.0));
    if rows.is_empty() || rows.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(Diagnostic::error(
            "dynamic conformance private-function table has no unique normalized requirement rows",
        ));
    }
    Ok(rows)
}

fn exact_trait(
    program: &CheckedTrees,
    symbol: psi_symbols::SymbolHandle,
) -> Result<&psi_checked_trees::trait_definition::TraitDefinition, Diagnostic> {
    let matches = program
        .traits()
        .iter()
        .filter(|definition| definition.symbol == symbol)
        .collect::<Vec<_>>();
    let [value] = matches.as_slice() else {
        return Err(Diagnostic::error(
            "dynamic conformance private-function declaring trait is missing or duplicated",
        ));
    };
    Ok(value)
}

fn exact_machine(
    program: &CheckedTrees,
    symbol: psi_symbols::SymbolHandle,
) -> Result<&psi_checked_trees::machine::Machine, Diagnostic> {
    let matches = program
        .machines()
        .iter()
        .filter(|machine| machine.symbol == symbol)
        .collect::<Vec<_>>();
    let [value] = matches.as_slice() else {
        return Err(Diagnostic::error(
            "dynamic conformance private-function realization identity is missing or duplicated",
        ));
    };
    Ok(value)
}

fn retain_realization_demand(
    demands: &mut Vec<PrivateDynamicRealizationDemand>,
    realization_identity: Arc<str>,
    realization: StateKey,
) -> Result<(), Diagnostic> {
    if let Some(existing) = demands.iter().find(|demand| {
        demand.realization == realization || demand.realization_identity == realization_identity
    }) {
        if existing.realization != realization
            || existing.realization_identity != realization_identity
        {
            return Err(Diagnostic::error(
                "dynamic conformance private-function demands disagree on realization identity or StateKey",
            ));
        }
        return Ok(());
    }
    demands.push(PrivateDynamicRealizationDemand {
        realization_identity,
        realization,
    });
    Ok(())
}

fn function_boundary_instruction(
    source_key: StateKey,
    kind: AbstractOperationKind,
) -> AbstractOperation {
    AbstractOperation {
        kind,
        source_key,
        source_statement: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{PrivateDynamicRealizationDemand, retain_realization_demand};
    use omega_control_flow::StateKey;
    use psi_symbols::SymbolHandle;
    use std::sync::Arc;

    fn key(machine: u32, state: u32) -> StateKey {
        StateKey {
            machine: SymbolHandle::from_arena_index(machine),
            state: SymbolHandle::from_arena_index(state),
            segment_index: 0,
        }
    }

    #[test]
    fn exact_repeated_realization_demand_deduplicates() {
        let mut demands = Vec::<PrivateDynamicRealizationDemand>::new();
        retain_realization_demand(&mut demands, Arc::from("Item::Primary::code"), key(1, 2))
            .expect("first exact demand");
        retain_realization_demand(&mut demands, Arc::from("Item::Primary::code"), key(1, 2))
            .expect("identical shared realization");

        assert_eq!(demands.len(), 1);
    }

    #[test]
    fn realization_identity_and_state_key_must_remain_one_to_one() {
        for (second_identity, second_key) in [
            ("Item::Secondary::code", key(1, 2)),
            ("Item::Primary::code", key(1, 3)),
        ] {
            let mut demands = Vec::<PrivateDynamicRealizationDemand>::new();
            retain_realization_demand(&mut demands, Arc::from("Item::Primary::code"), key(1, 2))
                .expect("first exact demand");
            let diagnostic =
                retain_realization_demand(&mut demands, Arc::from(second_identity), second_key)
                    .expect_err("identity or StateKey drift must fail closed");
            assert!(diagnostic.message.contains("disagree"));
        }
    }
}
