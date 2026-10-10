//! Exact occurrence and tuple joins for frozen callable admission.
//! Reuse authenticates operands, not just a source handle or a display name.

use super::super::{AppliedCallableOwner, AppliedMachineCallable};
use crate::{
    TypedTrees, signature,
    static_applications::{
        StaticMachineApplicationHandle, StaticMachineBinding, StaticMachineBindingValue,
    },
    types::TypeReferenceHandle,
};
use symbols::SymbolHandle;

impl TypedTrees {
    pub fn applied_machine_callable(
        &self,
        invocation: StaticMachineApplicationHandle,
        ordinal: usize,
        selected: SymbolHandle,
    ) -> Option<AppliedMachineCallable<'_>> {
        self.applied_callable_signatures
            .iter()
            .find_map(|(_, prepared)| {
                let AppliedCallableOwner::MachineSelection {
                    invocation: source,
                    ordinal: position,
                    selected: entry,
                    provider,
                    required,
                    ..
                } = &prepared.owner
                else {
                    return None;
                };
                (*source == invocation && *position == ordinal && *entry == selected).then_some(
                    AppliedMachineCallable {
                        provider: *provider,
                        required,
                        selected: &prepared.signature,
                    },
                )
            })
    }

    pub fn applied_machine_callable_matches(
        &self,
        invocation: StaticMachineApplicationHandle,
        ordinal: usize,
        selected: SymbolHandle,
        provider_bindings: &[StaticMachineBinding],
        required: &signature::StateSignature,
        caller_substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> bool {
        self.applied_callable_signatures
            .iter()
            .any(|(_, prepared)| {
                let AppliedCallableOwner::MachineSelection {
                    invocation: source,
                    ordinal: position,
                    selected: entry,
                    provider,
                    source_required,
                    caller_operands,
                    ..
                } = &prepared.owner
                else {
                    return false;
                };
                let operands = self.callable_substitutions.span_or_empty(*caller_operands);
                let previous_bindings = self
                    .expression_table
                    .static_machine_application_bindings(*provider);
                *source == invocation
                    && *position == ordinal
                    && *entry == selected
                    && source_required == required
                    && previous_bindings.len() == provider_bindings.len()
                    && previous_bindings
                        .iter()
                        .zip(provider_bindings)
                        .all(|(previous, current)| {
                            previous.parameter == current.parameter
                                && match (&previous.value, &current.value) {
                                    (
                                        StaticMachineBindingValue::Type(left),
                                        StaticMachineBindingValue::Type(right),
                                    )
                                    | (
                                        StaticMachineBindingValue::Const(left),
                                        StaticMachineBindingValue::Const(right),
                                    ) => self.same_callable_operand(*left, *right),
                                    _ => previous.value == current.value,
                                }
                        })
                    && operands.len() == caller_substitutions.len()
                    && operands.iter().zip(caller_substitutions).all(
                        |(operand, (parameter, actual))| {
                            operand.parameter == *parameter
                                && self.same_callable_operand(operand.actual, *actual)
                        },
                    )
            })
    }

    fn same_callable_operand(&self, left: TypeReferenceHandle, right: TypeReferenceHandle) -> bool {
        let identity = self.normalized_type_identity(left);
        left == right || (identity.is_valid() && identity == self.normalized_type_identity(right))
    }

    pub fn applied_machine_callables(
        &self,
        invocation: StaticMachineApplicationHandle,
    ) -> impl Iterator<Item = AppliedMachineCallable<'_>> {
        self.applied_callable_signatures
            .iter()
            .filter_map(move |(_, prepared)| {
                let AppliedCallableOwner::MachineSelection {
                    invocation: source,
                    provider,
                    required,
                    ..
                } = &prepared.owner
                else {
                    return None;
                };
                (*source == invocation).then_some(AppliedMachineCallable {
                    provider: *provider,
                    required,
                    selected: &prepared.signature,
                })
            })
    }
}
