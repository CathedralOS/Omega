//! Dispatch output custody is the union of every closed realization's summary.
//!
//! This judgment consumes the same authenticated selector roster as dispatch
//! and termination; it never chooses a representative body. Formal coordinates
//! map by position after the selector lane. Unknown bodies/sources withhold
//! coverage, and opaque lifetime dependencies remain nonspatial obligations.
//! No predicate or private postcondition is imported through this relation.

use super::*;
use crate::borrow_contracts::{ProviderOutputDestination, resolve_provider_outputs};
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

/// Whether this dynamic call's selector may be one the program never forms,
/// so the closed realization roster does not bound it. A selector received
/// through the calling state's inputs may be any fitting implementation, and
/// so may one obtained by dispatching on any received selector, whatever its
/// interface. A boundary requirement, or a machine without a checked body,
/// that can return or write a selector of this interface admits outside
/// implementations everywhere: provider
/// selection supplies no coercion evidence (dynamic_dispatch.md, source
/// eligibility). Types the walk cannot classify count as possible carriers.
pub(in crate::machine_calls::calls::write_frames) fn caller_may_receive_selector(
    program: &dyn crate::CheckingInput,
    caller: &typed_trees::state::State,
    slot: &DynamicSlotSignature,
) -> bool {
    let mut visiting = Vec::new();
    let received = program.state_parameters(caller).iter().any(|parameter| {
        type_may_carry_selector(program, parameter.type_reference, None, &mut visiting)
    });
    received || boundary_may_supply_selector(program, slot.interface_identity)
}

fn boundary_may_supply_selector(
    program: &dyn crate::CheckingInput,
    interface: typed_trees::type_identity::TypeIdentityId,
) -> bool {
    let mut visiting = Vec::new();
    // Every machine without a checked body here (boundary, admitted or
    // externally realized supplies) is an outside producer, like a boundary
    // trait requirement.
    let opaque_machine = program
        .machines()
        .iter()
        .filter(|machine| !(machine.body_is_present && machine.supply_mode.is_checked_body()))
        .flat_map(|machine| program.machine_states(machine))
        .any(|state| {
            type_may_carry_selector(program, state.return_type, Some(interface), &mut visiting)
                || program.state_parameters(state).iter().any(|parameter| {
                    type_may_carry_selector(
                        program,
                        parameter.type_reference,
                        Some(interface),
                        &mut visiting,
                    )
                })
        });
    opaque_machine
        || program
            .traits()
            .iter()
            .filter(|definition| definition.is_boundary)
            .flat_map(|definition| program.trait_machine_signatures(definition))
            .any(|signature| {
                type_may_carry_selector(
                    program,
                    signature.return_type,
                    Some(interface),
                    &mut visiting,
                ) || program
                    .state_signature_parameters(signature)
                    .iter()
                    .any(|parameter| {
                        type_may_carry_selector(
                            program,
                            parameter.type_reference,
                            Some(interface),
                            &mut visiting,
                        )
                    })
            })
}

fn type_may_carry_selector(
    program: &dyn crate::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
    interface: Option<typed_trees::type_identity::TypeIdentityId>,
    visiting: &mut Vec<SymbolHandle>,
) -> bool {
    use typed_trees::types::TypeReferenceNode;
    if !type_reference.is_valid() {
        return true;
    }
    let table = &program.type_reference_table;
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
        // `None` asks for any selector; `Some` for one exact interface.
        TypeReferenceNode::DynamicTrait { .. } => interface
            .is_none_or(|interface| program.normalized_type_identity(type_reference) == interface),
        // A named primitive scalar holds no selector; `dyn` is itself
        // classified as the primitive `Selector`, so it is matched above.
        TypeReferenceNode::Named { .. } if table.primitive_type(type_reference).is_some() => false,
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
    interface: Option<typed_trees::type_identity::TypeIdentityId>,
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

/// The requirement's declared slot outputs in the slot's own formal namespace.
/// An unknown selection may install any reference its declared lifetimes
/// permit, so each output keeps only its lifetime debts: no supplier
/// completes its pointer coverage, and the written slot's prior descriptor
/// does not survive.
pub(in crate::machine_calls::calls::write_frames) fn declared_formal_effects(
    program: &dyn crate::CheckingInput,
    slot: &DynamicSlotSignature,
) -> Option<Vec<StoredReferenceEffect>> {
    let parameters = slot.parameters.as_slice();
    let relation = resolve_provider_outputs(program, parameters).ok()?;
    if relation.readiness != crate::borrow_contracts::BorrowRelationReadiness::Complete {
        return None;
    }
    let mut effects = Vec::new();
    for output in relation.outputs {
        let ProviderOutputDestination::Parameter(position) = output.destination;
        let parameter = parameters.get(position)?;
        let segments = super::opaque_outputs::owner_segments(&output.owner_path)?;
        let mut sources = Vec::new();
        for dependency in output.dependencies {
            let source_parameter = parameters.get(dependency.parameter_index)?;
            if source_parameter.type_reference != dependency.source_type {
                return None;
            }
            let source = FrameSourcePlace {
                segments: super::opaque_outputs::owner_segments(&dependency.source_path)?,
                ..FrameSourcePlace::parameter_root(source_parameter)
            };
            sources.push(SlotSource::from_place(
                program,
                source,
                FramePathPrecision::Exact,
                Some(dependency.access),
            ));
        }
        effects.push(SlotEffect {
            root: parameter.symbol,
            segments,
            sources: Some(sources),
            incoming: false,
            may_select: false,
            kind: stored_origins::StoredOriginKind::Reference(output.access),
        });
    }
    super::public_effects(effects)
}
