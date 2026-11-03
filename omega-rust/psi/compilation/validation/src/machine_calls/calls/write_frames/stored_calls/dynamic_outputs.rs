//! Dispatch output custody is the union of every closed realization's summary.
//!
//! This judgment consumes the same authenticated selector roster as dispatch
//! and termination; it never chooses a representative body. Formal coordinates
//! map by position after the selector lane. Unknown bodies/sources withhold
//! coverage, and opaque lifetime dependencies remain nonspatial obligations.
//! No predicate or private postcondition is imported through this relation.

use super::*;
use typed_trees::dynamic_traits::DynamicSlotSignature;

pub(in crate::machine_calls::calls::write_frames) fn relative_effects(
    program: &dyn crate::CheckingInput,
    slot: &DynamicSlotSignature,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<SlotEffect>> {
    let selections = (crate::collect_dynamic_implementation_selections(program).ok())?;
    let mut states = Vec::new();
    for selection in selections.iter().filter(|selection| {
        program.normalized_type_identity(selection.selector_type) == slot.interface_identity
    }) {
        let conformance = (program.conformance_by_symbol(selection.implementation))?;
        let rows = conformance
            .rows
            .iter()
            .filter(|row| {
                row.requirement == slot.requirement && row.declaring_trait == slot.declaring_trait
            })
            .collect::<Vec<_>>();
        if rows.is_empty() {
            return None;
        }
        for row in rows {
            let machine = (program.machine_by_symbol(row.realization_machine))?;
            let state = (program.state_in_machine(machine, row.realization_state))?;
            if !machine.body_is_present
                || !machine.supply_mode.is_checked_body()
                || !program.machine_type_parameters(machine).is_empty()
                || program.machine_states(machine).first()?.symbol != state.symbol
            {
                return None;
            }
            if !states.contains(&state.symbol) {
                states.push(state.symbol);
            }
        }
    }
    if states.is_empty() {
        return None;
    }
    super::checked_alternatives::relative_effects(
        program,
        &states,
        slot.data_parameters(),
        symbols,
        inference,
    )
}

/// Whether a selector of this slot's interface can reach the calling state
/// through its inputs. Such a selector is an unknown selection: its body must
/// hold for every fitting implementation, not only those this program forms.
/// Types the walk cannot classify count as possible carriers.
pub(in crate::machine_calls::calls::write_frames) fn caller_may_receive_selector(
    program: &dyn crate::CheckingInput,
    caller: &typed_trees::state::State,
    slot: &DynamicSlotSignature,
) -> bool {
    let mut visiting = Vec::new();
    program.state_parameters(caller).iter().any(|parameter| {
        type_may_carry_selector(
            program,
            parameter.type_reference,
            slot.interface_identity,
            &mut visiting,
        )
    })
}

fn type_may_carry_selector(
    program: &dyn crate::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
    interface: typed_trees::type_identity::TypeIdentityId,
    visiting: &mut Vec<SymbolHandle>,
) -> bool {
    use typed_trees::types::TypeReferenceNode;
    if !type_reference.is_valid() {
        return true;
    }
    let table = &program.type_reference_table;
    if table.primitive_type(type_reference).is_some() {
        return false;
    }
    match table.type_reference(type_reference) {
        TypeReferenceNode::Unit | TypeReferenceNode::ConstExpression(_) => false,
        TypeReferenceNode::Reference { referee, .. } => {
            type_may_carry_selector(program, *referee, interface, visiting)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_may_carry_selector(program, *base_type, interface, visiting)
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            type_may_carry_selector(program, *element_type, interface, visiting)
        }
        TypeReferenceNode::DynamicTrait { .. } => {
            program.normalized_type_identity(type_reference) == interface
        }
        TypeReferenceNode::Named { symbol, .. } => {
            data_may_carry_selector(program, *symbol, interface, visiting)
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            table
                .type_reference_handles(*arguments)
                .iter()
                .any(|argument| type_may_carry_selector(program, *argument, interface, visiting))
                || data_may_carry_selector(program, *base_symbol, interface, visiting)
        }
    }
}

fn data_may_carry_selector(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
    interface: typed_trees::type_identity::TypeIdentityId,
    visiting: &mut Vec<SymbolHandle>,
) -> bool {
    if visiting.contains(&symbol) {
        return false;
    }
    let Some(definition) = program.data_definition_by_symbol(symbol) else {
        return true;
    };
    visiting.push(symbol);
    let carries = program
        .data_members(definition)
        .iter()
        .any(|member| match member {
            typed_trees::data::DataMember::Field(field) => {
                type_may_carry_selector(program, field.type_reference, interface, visiting)
            }
            typed_trees::data::DataMember::Variant(variant) => {
                program.data_payload_fields(variant).iter().any(|field| {
                    type_may_carry_selector(program, field.type_reference, interface, visiting)
                })
            }
        });
    visiting.pop();
    carries
}
