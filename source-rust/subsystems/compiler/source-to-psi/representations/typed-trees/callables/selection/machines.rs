pub(crate) mod queries {
    use super::super::{AppliedCallableOwner, AppliedMachineCallable};
    use crate::source_state::type_identity::{
        IndexIdentityArgument, applied_index_identity, applied_type_identity,
    };
    use crate::source_state::{
        TypedTrees,
        static_applications::{
            StaticMachineApplicationHandle, StaticMachineBinding, StaticMachineBindingValue,
        },
        types::TypeReferenceHandle,
    };
    use symbols::SymbolHandle;

    impl TypedTrees {
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

        fn same_callable_index(
            &self,
            left: TypeReferenceHandle,
            right: TypeReferenceHandle,
            owner: SymbolHandle,
        ) -> bool {
            left == right
                || applied_index_identity(
                    self,
                    IndexIdentityArgument::TypeReference(left),
                    owner,
                    &[],
                )
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
            left == right
                || applied_type_identity(self, left, owner, &[])
                    .zip(applied_type_identity(self, right, owner, &[]))
                    .is_some_and(|(left, right)| left == right)
        }

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
                && (lifetimes.is_empty()
                    || retained.lifetime_binding_declaration
                        == original.lifetime_binding_declaration)
                && retained.conformance_selection == original.conformance_selection
                && lifetimes == self.static_machine_application_lifetimes(source)
                && self.same_selected_bindings(
                    self.static_machine_application_bindings(invocation),
                    self.static_machine_application_bindings(source),
                    original.lexical_owner,
                ))
            .then_some(source)
        }

        pub(crate) fn same_registered_machine_operation(
            &self,
            left: SymbolHandle,
            right: SymbolHandle,
        ) -> bool {
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
}

#[cfg(test)]
#[path = "../tests/machine_selections.rs"]
pub(crate) mod record_tests;
