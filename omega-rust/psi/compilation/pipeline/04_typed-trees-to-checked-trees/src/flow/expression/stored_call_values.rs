//! Capture normal-return descriptor associations from the actual evaluated
//! tuple before invocation writes, then publish them on the one immutable
//! storage timeline. The checked relation supplies correspondence; retained
//! captures supply geometry and formation, never source replay or new authority.
mod suppliers;

use super::{Execution, InvocationSite};
use arena::Handle;
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
        inputs: &[FlowOperandFact],
        binding_writes: Option<&[CanonicalPlace]>,
    ) -> Vec<CallStore> {
        // State jumps have an arrival owner. This inventory describes normal
        // returns to the invocation's next operand, never transfer edges.
        if matches!(site, InvocationSite::Transition(_)) {
            return Vec::new();
        }
        let (parameters, effects) = if let Some(slot) =
            self.program.dynamic_slot_by_symbol(call.target_symbol)
        {
            let Some(effects) = self
                .context
                .call_frames
                .and_then(|frames| frames.dynamic_stored_reference_effects(slot.symbol))
            else {
                return Vec::new();
            };
            (slot.parameters.as_slice(), std::rc::Rc::new(effects))
        } else {
            let contract = self.context.call_frames.and_then(|frames| {
                frames.reference_slot_contract(
                    self.machine,
                    self.state,
                    self.statement_index,
                    call.call_ordinal,
                    call.target_symbol,
                    call.authored_expression,
                )
            });
            let Some(contract) = contract else {
                return Vec::new();
            };
            match contract {
                validation::ReferenceSlotContract::CheckedBody { machine, state } => {
                    let parameters = self.program.state_parameters(state);
                    if inputs.len() != parameters.len() {
                        return Vec::new();
                    }
                    // A complete ordinary empty union has no descriptor slot to
                    // refine. Reuse that target memo before observing scalar cuts.
                    // Unavailable relations are not empty and continue below.
                    if self
                        .context
                        .stored_reference_effects(machine, state, &[])
                        .is_some_and(|effects| effects.is_empty())
                    {
                        return Vec::new();
                    }
                    let indices = parameters
                        .iter()
                        .zip(inputs)
                        .map(|(parameter, input)| {
                            // Read the operand's immutable completed coordinate, never
                            // its current source spelling after another operand wrote.
                            if validation::reference_shell(self.program, parameter.type_reference)
                                .is_none()
                                && self
                                    .program
                                    .primitive_type_reference(parameter.type_reference)
                                    .is_some_and(|primitive| primitive.is_fixed_integer())
                            {
                                self.captured_coordinate(input)
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>();
                    let Some(effects) = self
                        .context
                        .stored_reference_effects(machine, state, &indices)
                    else {
                        return Vec::new();
                    };
                    (parameters, effects)
                }
                validation::ReferenceSlotContract::Opaque {
                    parameters,
                    effects,
                } => (parameters, std::rc::Rc::new(effects)),
            }
        };
        if inputs.len() != parameters.len() {
            return Vec::new();
        }
        self.capture_stored_suppliers(parameters, &effects, inputs, call, binding_writes)
    }

    pub(super) fn install_call_stores(&mut self, stores: Vec<CallStore>) {
        for store in stores {
            self.storage_snapshot = self.install_call_stored_value(
                self.storage_snapshot,
                &store.destination,
                store.value,
            );
        }
    }

    fn install_call_stored_value(
        &mut self,
        previous: Handle<FlowStorageSnapshot>,
        destination: &CanonicalPlace,
        value: CallStoredValue,
    ) -> Handle<FlowStorageSnapshot> {
        match value {
            CallStoredValue::Unavailable => super::super::stored_values::append_store(
                &mut self.context.control,
                previous,
                destination,
                Handle::invalid(),
                &[],
            ),
            CallStoredValue::Captured(value) => super::super::stored_values::append_store(
                &mut self.context.control,
                previous,
                destination,
                value,
                &[],
            ),
            CallStoredValue::Incoming(source) => {
                super::super::stored_values::append_preserved_binding(
                    &mut self.context.control,
                    previous,
                    destination,
                    source,
                )
            }
            CallStoredValue::Alternatives(values) => {
                let predecessors = values
                    .into_iter()
                    .map(|value| self.install_call_stored_value(previous, destination, value))
                    .collect::<Vec<_>>();
                super::super::stored_values::join(&mut self.context.control, &predecessors)
            }
        }
    }
}
