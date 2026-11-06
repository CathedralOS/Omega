//! Exact occurrence and tuple joins for frozen callable admission.
//! Reuse authenticates operands, not just a source handle or a display name.

use super::super::{AppliedCallableOwner, AppliedMachineCallable};
use crate::type_identity::{IndexIdentityArgument, applied_index_identity, applied_type_identity};
use crate::{
    TypedTrees, signature,
    static_applications::{
        StaticMachineApplicationHandle, StaticMachineBinding, StaticMachineBindingValue,
    },
    types::TypeReferenceHandle,
};
use symbols::SymbolHandle;

impl TypedTrees {
    pub(super) fn same_selected_provider(
        &self,
        left: StaticMachineApplicationHandle,
        right: StaticMachineApplicationHandle,
    ) -> bool {
        let a = self.static_machine_application(left);
        let b = self.static_machine_application(right);
        a.registration_operation == b.registration_operation
            && a.binding_declaration == b.binding_declaration
            && a.lexical_owner == b.lexical_owner
            && a.lifetime_binding_declaration == b.lifetime_binding_declaration
            && a.conformance_selection == b.conformance_selection
            && self.static_machine_application_lifetimes(left)
                == self.static_machine_application_lifetimes(right)
            && self.same_selected_bindings(
                self.static_machine_application_bindings(left),
                self.static_machine_application_bindings(right),
                a.lexical_owner,
            )
    }

    fn same_selected_bindings(
        &self,
        left: &[StaticMachineBinding],
        right: &[StaticMachineBinding],
        owner: SymbolHandle,
    ) -> bool {
        left.len() == right.len()
            && left.iter().zip(right).all(|(previous, current)| {
                previous.parameter == current.parameter
                    && match (&previous.value, &current.value) {
                        (
                            StaticMachineBindingValue::Type(a),
                            StaticMachineBindingValue::Type(b),
                        ) => self.same_callable_operand(*a, *b, owner),
                        (
                            StaticMachineBindingValue::Const(a),
                            StaticMachineBindingValue::Const(b),
                        ) => self.same_callable_index(*a, *b, owner),
                        (
                            StaticMachineBindingValue::Machine(a),
                            StaticMachineBindingValue::Machine(b),
                        )
                        | (
                            StaticMachineBindingValue::Conformance(a),
                            StaticMachineBindingValue::Conformance(b),
                        ) => {
                            a == b
                                || applied_index_identity(
                                    self,
                                    IndexIdentityArgument::StaticArgument(a),
                                    owner,
                                    &[],
                                )
                                .zip(applied_index_identity(
                                    self,
                                    IndexIdentityArgument::StaticArgument(b),
                                    owner,
                                    &[],
                                ))
                                .is_some_and(|(a, b)| a == b)
                        }
                        _ => previous.value == current.value,
                    }
            })
    }

    pub(super) fn machine_selection_lifetimes_are_current(
        &self,
        owner: &AppliedCallableOwner,
    ) -> bool {
        let AppliedCallableOwner::MachineSelection {
            invocation,
            provider,
            caller_lifetimes,
            caller_bindings,
            caller_lifetime_declaration,
            caller_lifetime_owner,
            provider_lifetime_declaration,
            provider_lifetime_owner,
            provider_lifetimes,
            ..
        } = owner
        else {
            return false;
        };
        let caller = self.static_machine_application(*invocation);
        let selected = self.static_machine_application(*provider);
        let saved = self
            .expression_table
            .static_machine_binding_snapshot(*caller_bindings);
        let current = self.static_machine_application_bindings(*invocation);
        self.same_selected_bindings(saved, current, caller.lexical_owner)
            && *caller_lifetime_declaration == caller.lifetime_binding_declaration
            && *caller_lifetime_owner == caller.lexical_owner
            && *provider_lifetime_declaration == selected.lifetime_binding_declaration
            && *provider_lifetime_owner == selected.lexical_owner
            && self
                .callable_lifetime_substitutions
                .span_or_empty(*caller_lifetimes)
                == self.static_machine_application_lifetimes(*invocation)
            && self
                .callable_lifetime_substitutions
                .span_or_empty(*provider_lifetimes)
                == self.static_machine_application_lifetimes(*provider)
    }

    /// A retained tuple needs its exact demand view, never the executable
    /// clone's erased contract as a substitute when that view is missing.
    pub fn has_applied_machine_selection(
        &self,
        invocation: StaticMachineApplicationHandle,
        ordinal: usize,
        selected: SymbolHandle,
    ) -> bool {
        self.applied_callable_signatures
            .machine_rows(invocation)
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
            .machine_rows(invocation)
            .find_map(|(handle, _)| {
                let view = self
                    .applied_machine_callable_at(super::super::AppliedMachineCallableId(handle))?;
                (view.ordinal == ordinal
                    && view.selected_entry == selected
                    && view.demand == demand
                    && view.source_required == expected)
                    .then_some(view)
            })
    }

    /// Read a row retained by the producing owner. A retired handle or a row
    /// without its required view supplies no contract. Consumers still compare
    /// their full application, binder, ordinal and selected-entry correspondence.
    pub fn applied_machine_callable_at(
        &self,
        identity: super::super::AppliedMachineCallableId,
    ) -> Option<AppliedMachineCallable<'_>> {
        let prepared = self.applied_callable_signatures.get(identity.0);
        let AppliedCallableOwner::MachineSelection {
            invocation,
            ordinal,
            demand,
            selected,
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
        Some(AppliedMachineCallable {
            identity,
            invocation: *invocation,
            ordinal: *ordinal,
            demand: *demand,
            lexical_owner: self.static_machine_application(*provider).lexical_owner,
            provider: *provider,
            selected_entry: *selected,
            required: required.as_ref()?,
            selected: &prepared.signature,
            source_required,
            caller_operands: self.callable_substitutions.span_or_empty(*caller_operands),
            caller_lifetimes: self
                .callable_lifetime_substitutions
                .span_or_empty(*caller_lifetimes),
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
        self.applied_machine_callable_mismatch(
            invocation,
            ordinal,
            selected,
            demand,
            provider_bindings,
            provider_lifetimes,
            required,
            caller_substitutions,
            caller_lifetime_bindings,
        )
        .is_none()
    }

    pub fn applied_machine_callable_mismatch(
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
    ) -> Option<&'static str> {
        let mut mismatch = "no checking view names this exact function and requirement";
        let matches = self
            .applied_callable_signatures
            .machine_rows(invocation)
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
                    caller_lifetime_declaration,
                    caller_lifetime_owner,
                    provider_lifetime_declaration,
                    provider_lifetime_owner,
                    provider_lifetimes: prepared_provider_lifetimes,
                    ..
                } = &prepared.owner
                else {
                    return false;
                };
                let operands = self.callable_substitutions.span_or_empty(*caller_operands);
                let owner = self.static_machine_application(invocation).lexical_owner;
                let previous_bindings = self.static_machine_application_bindings(*provider);
                if *source != invocation
                    || *position != ordinal
                    || *entry != selected
                    || *prepared_demand != demand
                    || source_required != required
                {
                    return false;
                }
                let checks = [
                    (
                        self.machine_selection_lifetimes_are_current(&prepared.owner),
                        "retained caller arguments or lifetime bindings changed",
                    ),
                    (*source == invocation, "source invocation changed"),
                    (*position == ordinal, "machine argument position changed"),
                    (*entry == selected, "selected function changed"),
                    (
                        *prepared_demand == demand,
                        "required machine binder changed",
                    ),
                    (
                        source_required == required,
                        "source requirement signature changed",
                    ),
                    (
                        *caller_lifetime_declaration
                            == self
                                .static_machine_application(invocation)
                                .lifetime_binding_declaration,
                        "caller lifetime declaration changed",
                    ),
                    (
                        *caller_lifetime_owner == owner,
                        "caller lexical owner changed",
                    ),
                    (
                        *provider_lifetime_declaration
                            == self
                                .static_machine_application(*provider)
                                .lifetime_binding_declaration,
                        "provider lifetime declaration changed",
                    ),
                    (
                        *provider_lifetime_owner
                            == self.static_machine_application(*provider).lexical_owner,
                        "provider lexical owner changed",
                    ),
                    (
                        self.callable_lifetime_substitutions
                            .span_or_empty(*prepared_provider_lifetimes)
                            == provider_lifetimes,
                        "provider lifetime arguments changed",
                    ),
                    (
                        self.callable_lifetime_substitutions
                            .span_or_empty(*caller_lifetimes)
                            == caller_lifetime_bindings,
                        "caller lifetime arguments changed",
                    ),
                    (
                        self.static_machine_application(*provider).lexical_owner
                            == self.static_machine_application(invocation).lexical_owner,
                        "provider and caller have different lexical owners",
                    ),
                    (
                        self.static_machine_application_lifetimes(*provider) == provider_lifetimes,
                        "provider lifetime arguments changed",
                    ),
                    (
                        previous_bindings.len() == provider_bindings.len(),
                        "provider argument count changed",
                    ),
                    (
                        previous_bindings.iter().zip(provider_bindings).all(
                            |(previous, current)| {
                                previous.parameter == current.parameter
                                    && match (&previous.value, &current.value) {
                                        (
                                            StaticMachineBindingValue::Type(left),
                                            StaticMachineBindingValue::Type(right),
                                        ) => self.same_callable_operand(*left, *right, owner),
                                        (
                                            StaticMachineBindingValue::Const(left),
                                            StaticMachineBindingValue::Const(right),
                                        ) => self.same_callable_index(*left, *right, owner),
                                        _ => previous.value == current.value,
                                    }
                            },
                        ),
                        "provider arguments changed",
                    ),
                    (
                        operands.len() == caller_substitutions.len(),
                        "caller argument count changed",
                    ),
                    (
                        operands.iter().zip(caller_substitutions).all(
                            |(operand, (parameter, actual))| {
                                operand.parameter == *parameter
                                    && self.same_caller_operand(
                                        *parameter,
                                        operand.actual,
                                        *actual,
                                        owner,
                                    )
                            },
                        ),
                        "caller arguments changed",
                    ),
                ];
                if let Some((_, reason)) = checks.iter().find(|(matches, _)| !matches) {
                    mismatch = reason;
                    false
                } else {
                    true
                }
            });
        (!matches).then_some(mismatch)
    }

    fn same_caller_operand(
        &self,
        parameter: SymbolHandle,
        left: TypeReferenceHandle,
        right: TypeReferenceHandle,
        owner: SymbolHandle,
    ) -> bool {
        // Closing a requirement can replace a const expression with its value.
        // The declared binder decides whether these are values or types; equal
        // storage carriers cannot substitute for equal constant arguments.
        let Some(parameter) = self.data_type_parameter_by_symbol(parameter) else {
            return false;
        };
        match parameter.kind {
            crate::data::TypeParameterKind::Type => self.same_callable_operand(left, right, owner),
            crate::data::TypeParameterKind::Const { .. }
            | crate::data::TypeParameterKind::Value { .. } => {
                self.same_callable_index(left, right, owner)
            }
            _ => false,
        }
    }

    fn same_callable_index(
        &self,
        left: TypeReferenceHandle,
        right: TypeReferenceHandle,
        owner: SymbolHandle,
    ) -> bool {
        left == right
            || applied_index_identity(self, IndexIdentityArgument::TypeReference(left), owner, &[])
                .zip(applied_index_identity(
                    self,
                    IndexIdentityArgument::TypeReference(right),
                    owner,
                    &[],
                ))
                .is_some_and(|(left, right)| left == right)
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
            || applied_type_identity(self, left, owner, &[])
                .zip(applied_type_identity(self, right, owner, &[]))
                .is_some_and(|(left, right)| left == right)
    }

    /// A retained specialization describes an existing application, rather
    /// than supplying another invocation. Its complete typed tuple must still
    /// match before its selected helpers can use that application's contracts.
    /// Only a direct original application is permitted; missing or changed
    /// correspondence supplies no contract.
    pub fn applied_machine_callable_origin(
        &self,
        invocation: StaticMachineApplicationHandle,
    ) -> Option<StaticMachineApplicationHandle> {
        if !invocation.is_valid() {
            return None;
        }
        let retained = self.static_machine_application(invocation);
        if !retained.registration_operation.is_valid() {
            return None;
        }
        let source = retained.callable_contract_source;
        if !source.is_valid() {
            return Some(invocation);
        }
        let original = self.static_machine_application(source);
        let lifetimes = self.static_machine_application_lifetimes(invocation);
        (!original.callable_contract_source.is_valid()
            && original.registration_operation.is_valid()
            && self.same_registered_machine_operation(
                retained.registration_operation,
                original.registration_operation,
            )
            && original.binding_declaration.is_valid()
            && original.lexical_owner.is_valid()
            && retained.binding_declaration == original.binding_declaration
            && retained.lexical_owner == original.lexical_owner
            // Empty region tuples have no named region owner to substitute.
            // Nonempty tuples retain their exact declaration and supplying scope.
            && (lifetimes.is_empty()
                || retained.lifetime_binding_declaration == original.lifetime_binding_declaration)
            && retained.conformance_selection == original.conformance_selection
            && lifetimes == self.static_machine_application_lifetimes(source)
            && self.same_selected_bindings(
                self.static_machine_application_bindings(invocation),
                self.static_machine_application_bindings(source),
                original.lexical_owner,
            ))
        .then_some(source)
    }

    fn same_registered_machine_operation(&self, left: SymbolHandle, right: SymbolHandle) -> bool {
        // A machine head and its unique entry name the same operation. Public
        // requirements remain distinct even when their implementation tuple matches.
        let head_and_entry = |head, entry| {
            self.machine_by_symbol(head).is_some_and(|machine| {
                self.unique_machine_by_entry_state_symbol(entry)
                    .is_some_and(|(owner, _)| owner.symbol == machine.symbol)
            })
        };
        left == right || head_and_entry(left, right) || head_and_entry(right, left)
    }

    pub fn applied_machine_callables(
        &self,
        invocation: StaticMachineApplicationHandle,
    ) -> impl Iterator<Item = AppliedMachineCallable<'_>> {
        self.applied_machine_callable_origin(invocation)
            .into_iter()
            .flat_map(|origin| self.applied_callable_signatures.machine_rows(origin))
            .filter_map(|(handle, _)| {
                self.applied_machine_callable_at(super::super::AppliedMachineCallableId(handle))
            })
    }
}
