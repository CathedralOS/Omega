//! Capture normal-return descriptor associations from the actual evaluated
//! tuple before invocation writes, then publish them on the one immutable
//! storage timeline. The checked relation supplies correspondence; retained
//! captures supply geometry and formation, never source replay or new authority.
mod lifetime_sources;
mod suppliers;

use super::super::stored_values::append_preserved_binding;
use super::super::stored_values::append_store;
use super::super::stored_values::join;
use super::{Execution, InvocationSite};
use arena::{Handle, HandleSpan};
use checked_trees::{BorrowCallFact, FlowOperandFact, FlowStorageSnapshot};
use validation::CanonicalPlace;

pub(super) struct CallStore {
    pub(super) destination: CanonicalPlace,
    pub(super) value: CallStoredValue,
}

pub(super) enum CallStoredValue {
    Unavailable,
    Incoming(Handle<FlowStorageSnapshot>),
    Captured(Handle<FlowOperandFact>),
    Alternatives(Vec<CallStoredValue>),
}

impl Execution<'_, '_, '_> {
    pub(super) fn capture_call_stores(
        &mut self,
        site: InvocationSite,
        call: &BorrowCallFact,
        inputs: HandleSpan<FlowOperandFact>,
        binding_writes: Option<&[CanonicalPlace]>,
    ) -> Vec<CallStore> {
        // State jumps have an arrival owner. This inventory describes normal
        // returns to the invocation's next operand, never transfer edges.
        if matches!(site, InvocationSite::Transition(_)) {
            return Vec::new();
        }
        let mut inputs = inputs;
        let contract = if let Some(slot) = self.program.dynamic_slot_by_symbol(call.target_symbol) {
            if let Some((machine, state, selected)) = self.selected_store_target(call, inputs) {
                inputs = selected;
                validation::ReferenceSlotContract::CheckedBody { machine, state }
            } else {
                // Without a selection proved in this state, a selector that
                // may come from outside the program keeps the requirement's
                // declared outputs; validation's loan formation, which has no
                // selection step, applies the same rule to every such call.
                let effects = self.context.call_frames.and_then(|frames| {
                    match frames.received_selector_stored_reference_effects(self.state, slot.symbol)
                    {
                        Some(declared) => declared,
                        None => frames.dynamic_stored_reference_effects(slot.symbol),
                    }
                });
                let Some(effects) = effects else {
                    return Vec::new();
                };
                validation::ReferenceSlotContract::Opaque {
                    parameters: slot.parameters.as_slice(),
                    effects,
                }
            }
        } else {
            let Some(contract) = self.context.call_frames.and_then(|frames| {
                frames.reference_slot_contract(
                    self.machine,
                    self.state,
                    self.statement_index,
                    call.call_ordinal,
                    call.target_symbol,
                    call.authored_expression,
                )
            }) else {
                return Vec::new();
            };
            contract
        };
        let (parameters, effects) = match contract {
            validation::ReferenceSlotContract::CheckedBody { machine, state } => {
                let parameters = self.program.state_parameters(state);
                if inputs.len() != parameters.len() {
                    return Vec::new();
                }
                // An established empty body relation has no descriptor slot to refine.
                // Unavailable is not empty; scalar cuts are read only when needed.
                if self
                    .context
                    .stored_reference_effects(machine, state, &[])
                    .is_some_and(|effects| effects.is_empty())
                {
                    return Vec::new();
                }
                let Some(indices) = self.captured_argument_indices(
                    parameters,
                    inputs
                        .handles()
                        .map(|handle| self.context.control.operands.get(handle)),
                ) else {
                    return Vec::new();
                };
                let selectors = self.known_dynamic_selectors(parameters, inputs);
                let effects = if selectors.is_empty() {
                    self.context
                        .stored_reference_effects(machine, state, &indices)
                } else {
                    self.context.stored_reference_effects_for_selectors(
                        machine, state, &indices, &selectors,
                    )
                };
                let Some(effects) = effects else {
                    return Vec::new();
                };
                (parameters, effects)
            }
            validation::ReferenceSlotContract::Opaque {
                parameters,
                effects,
            } => (parameters, std::rc::Rc::new(effects)),
        };
        if inputs.len() != parameters.len() {
            return Vec::new();
        }
        self.capture_stored_suppliers(parameters, &effects, inputs, call, binding_writes)
    }

    /// Install the call's normal-return stores, then mark the storage version
    /// at which the call is complete: after its writes and its own stores.
    pub(super) fn install_call_stores(
        &mut self,
        call: Handle<checked_trees::FlowCallFact>,
        stores: Vec<CallStore>,
    ) {
        for store in stores {
            self.storage_snapshot = self.install_call_stored_value(
                self.storage_snapshot,
                &store.destination,
                store.value,
            );
        }
        if call.is_valid() {
            self.context.control.calls.get_mut(call).completed_storage = self.storage_snapshot;
        }
    }

    fn install_call_stored_value(
        &mut self,
        previous: Handle<FlowStorageSnapshot>,
        destination: &CanonicalPlace,
        value: CallStoredValue,
    ) -> Handle<FlowStorageSnapshot> {
        match value {
            CallStoredValue::Unavailable => append_store(
                &mut self.context.control,
                previous,
                destination,
                Handle::invalid(),
                &[],
            ),
            CallStoredValue::Captured(value) => {
                append_store(&mut self.context.control, previous, destination, value, &[])
            }
            CallStoredValue::Incoming(source) => {
                append_preserved_binding(&mut self.context.control, previous, destination, source)
            }
            CallStoredValue::Alternatives(values) => {
                let predecessors = values
                    .into_iter()
                    .map(|value| self.install_call_stored_value(previous, destination, value))
                    .collect::<Vec<_>>();
                join(&mut self.context.control, &predecessors)
            }
        }
    }
}
