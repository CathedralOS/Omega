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
    /// A retained tuple needs its exact demand view, never the executable
    /// clone's erased contract as a substitute when that view is missing.
    pub fn has_applied_machine_selection(
        &self,
        invocation: StaticMachineApplicationHandle,
        ordinal: usize,
        selected: SymbolHandle,
    ) -> bool {
        self.applied_callable_signatures
            .iter()
            .any(|(_, prepared)| {
                matches!(&prepared.owner, AppliedCallableOwner::MachineSelection {
                invocation: source, ordinal: position, selected: entry, ..
            } if *source == invocation && *position == ordinal && *entry == selected)
            })
    }

    pub fn applied_machine_callable(
        &self,
        invocation: StaticMachineApplicationHandle,
        ordinal: usize,
        selected: SymbolHandle,
        demand: SymbolHandle,
        expected: &signature::StateSignature,
    ) -> Option<AppliedMachineCallable<'_>> {
        self.applied_callable_signatures
            .iter()
            .find_map(|(_, prepared)| {
                let AppliedCallableOwner::MachineSelection {
                    invocation: source,
                    ordinal: position,
                    demand: prepared_demand,
                    selected: entry,
                    provider,
                    required,
                    source_required,
                    caller_operands,
                    caller_lifetimes,
                    ..
                } = &prepared.owner
                else {
                    return None;
                };
                (*source == invocation
                    && *position == ordinal
                    && *entry == selected
                    && *prepared_demand == demand
                    && source_required == expected)
                    .then_some(AppliedMachineCallable {
                        lexical_owner: self
                            .expression_table
                            .static_machine_application(*provider)
                            .lexical_owner,
                        provider: *provider,
                        selected_entry: *entry,
                        required,
                        selected: &prepared.signature,
                        source_required,
                        caller_operands: self
                            .callable_substitutions
                            .span_or_empty(*caller_operands),
                        caller_lifetimes: self
                            .callable_lifetime_substitutions
                            .span_or_empty(*caller_lifetimes),
                    })
            })
    }

    pub fn applied_machine_callable_matches(
        &self,
        invocation: StaticMachineApplicationHandle,
        ordinal: usize,
        selected: SymbolHandle,
        demand: SymbolHandle,
        provider_bindings: &[StaticMachineBinding],
        provider_lifetimes: &[crate::static_applications::StaticMachineLifetimeBinding],
        required: &signature::StateSignature,
        caller_substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        caller_lifetime_bindings: &[crate::static_applications::StaticMachineLifetimeBinding],
    ) -> bool {
        self.applied_callable_signatures
            .iter()
            .any(|(_, prepared)| {
                let AppliedCallableOwner::MachineSelection {
                    invocation: source,
                    ordinal: position,
                    demand: prepared_demand,
                    selected: entry,
                    provider,
                    source_required,
                    caller_operands,
                    caller_lifetimes,
                    ..
                } = &prepared.owner
                else {
                    return false;
                };
                let operands = self.callable_substitutions.span_or_empty(*caller_operands);
                let owner = self
                    .expression_table
                    .static_machine_application(invocation)
                    .lexical_owner;
                let previous_bindings = self
                    .expression_table
                    .static_machine_application_bindings(*provider);
                *source == invocation
                    && *position == ordinal
                    && *entry == selected
                    && *prepared_demand == demand
                    && source_required == required
                    && self
                        .callable_lifetime_substitutions
                        .span_or_empty(*caller_lifetimes)
                        == caller_lifetime_bindings
                    && self
                        .expression_table
                        .static_machine_application(*provider)
                        .lexical_owner
                        == self
                            .expression_table
                            .static_machine_application(invocation)
                            .lexical_owner
                    && self
                        .expression_table
                        .static_machine_application_lifetimes(*provider)
                        == provider_lifetimes
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
                                    ) => self.same_callable_operand(*left, *right, owner),
                                    _ => previous.value == current.value,
                                }
                        })
                    && operands.len() == caller_substitutions.len()
                    && operands.iter().zip(caller_substitutions).all(
                        |(operand, (parameter, actual))| {
                            operand.parameter == *parameter
                                && self.same_callable_operand(operand.actual, *actual, owner)
                        },
                    )
            })
    }

    fn same_callable_operand(
        &self,
        left: TypeReferenceHandle,
        right: TypeReferenceHandle,
        owner: SymbolHandle,
    ) -> bool {
        // Checking views retain embedded regions even though executable
        // specialization intentionally shares equal erased carriers.
        left == right
            || crate::type_identity::applied_type_identity(self, left, owner, &[])
                .zip(crate::type_identity::applied_type_identity(
                    self,
                    right,
                    owner,
                    &[],
                ))
                .is_some_and(|(left, right)| left == right)
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
                    selected: entry,
                    provider,
                    required,
                    source_required,
                    caller_operands,
                    caller_lifetimes,
                    ..
                } = &prepared.owner
                else {
                    return None;
                };
                (*source == invocation).then_some(AppliedMachineCallable {
                    lexical_owner: self
                        .expression_table
                        .static_machine_application(*provider)
                        .lexical_owner,
                    provider: *provider,
                    selected_entry: *entry,
                    required,
                    selected: &prepared.signature,
                    source_required,
                    caller_operands: self.callable_substitutions.span_or_empty(*caller_operands),
                    caller_lifetimes: self
                        .callable_lifetime_substitutions
                        .span_or_empty(*caller_lifetimes),
                })
            })
    }
}
