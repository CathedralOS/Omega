//! Bind a call's evaluated arguments before checking its runtime body.
//! Save caller values before binding the callee's parameter names. Storage
//! observations use the callee's parameter paths; saved values keep their
//! original identity. Entering the body does not establish its preconditions
//! or prove its return guarantees.

use super::{StructuralJudge, StructuralTerm, TypeReferenceHandle};
use crate::machine_calls::calls::InvocationEntrySubject;
use typed_trees::{machine::Machine, signature::StateParameter, state::State};

impl<'program> StructuralJudge<'program> {
    pub(in crate::proof_contracts::contract_entailment) fn entry_argument_value(
        &mut self,
        parameter: &StateParameter,
        value: &StructuralTerm,
        subject: Option<&InvocationEntrySubject>,
        environment: &[(String, StructuralTerm)],
        scope: (&Machine, &State, TypeReferenceHandle),
    ) -> Option<StructuralTerm> {
        if crate::reference_shell(self.program, parameter.type_reference).is_some() {
            super::super::structural_terms::reference_argument_observation(
                self.program,
                parameter.type_reference,
                value,
                subject.and_then(|subject| {
                    self.reference_subject_term(subject, value, environment, scope)
                }),
            )
        } else {
            self.owned_argument_observation(parameter.type_reference, value)
        }
    }

    pub(in crate::proof_contracts::contract_entailment) fn bind_invocation(
        mut self,
        entry: &State,
        arguments: &[StructuralTerm],
        subjects: &[InvocationEntrySubject],
        environment: &[(String, StructuralTerm)],
        scope: (&Machine, &State, TypeReferenceHandle),
    ) -> Option<(Vec<(String, StructuralTerm)>, Self)> {
        let parameters = self.program.state_parameters(entry);
        if parameters.len() != arguments.len() {
            return None;
        }
        self.returned_value_source = None;
        if !self.requires_value_preservation() {
            return Some((
                parameters
                    .iter()
                    .zip(arguments)
                    .map(|(parameter, value)| (parameter.name.to_string(), value.clone()))
                    .collect(),
                self,
            ));
        }
        let bindings = parameters
            .iter()
            .zip(arguments)
            .map(|(parameter, value)| {
                let subject = subjects
                    .iter()
                    .find(|subject| subject.parameter == parameter.symbol);
                let value =
                    self.entry_argument_value(parameter, value, subject, environment, scope);
                (parameter.symbol, value)
            })
            .collect::<Vec<_>>();
        self.enter_storage_scope(entry, scope.1, subjects, &bindings);
        // Cached expressions and source assumptions belong to this invocation.
        // Saved equations still describe the original evaluated values; they
        // cannot reevaluate caller expressions using the callee's parameters.
        self.normal_results = std::sync::Arc::new(Vec::new());
        // Nominal receipts name detached completed values, not callee source
        // expressions. Passed values retain those receipts across activation.
        self.premise_facts.clear();
        std::sync::Arc::make_mut(&mut self.body_invocations).push(entry.symbol);
        self.declaration_state = entry.symbol;
        let (callee, _) = self.runtime_declaration_scope()?;
        self.body_type_parameters = self.program.machine_type_parameters(callee);
        self.retain_entry_scalar_parameters(
            callee,
            entry,
            bindings
                .iter()
                .filter_map(|(symbol, value)| value.as_ref().map(|value| (*symbol, value.clone()))),
        );
        let incoming = parameters
            .iter()
            .zip(bindings)
            .map(|(parameter, (_, value))| {
                (
                    parameter.name.to_string(),
                    value.unwrap_or_else(|| {
                        StructuralTerm::Opaque("unobserved invocation argument".to_owned())
                    }),
                )
            })
            .collect();
        Some((incoming, self))
    }

    /// Resume the caller with the completed path's saved facts and values.
    /// Map storage back through the actual argument origins, accounting for
    /// every write recorded for the call.
    pub(in crate::proof_contracts::contract_entailment) fn complete_invocation(
        mut self,
        caller: &Self,
        state: &State,
        subjects: &[InvocationEntrySubject],
        writes: &facts::NormalizedWriteFrame,
        returned: &StructuralTerm,
        completion_version: Option<super::storage_effects::StorageVersion>,
    ) -> Option<Self> {
        if !self.requires_value_preservation() {
            self.returned_value_source = caller.returned_value_source.clone();
            return Some(self);
        }
        if (caller.graph_storage_mode() && completion_version.is_none())
            || completion_version.is_some_and(|version| version.state != state.symbol)
        {
            return None;
        }
        let returned_nominal_values = self.returned_nominal_values(returned, caller);
        self.returned_value_source = caller.returned_value_source.clone();
        self.leave_storage_scope(caller, state, subjects, writes)?;
        self.body_invocations = std::sync::Arc::clone(&caller.body_invocations);
        self.declaration_state = caller.declaration_state;
        self.body_type_parameters = caller.body_type_parameters;
        self.normal_results = std::sync::Arc::clone(&caller.normal_results);
        // Restore the caller's snapshots and the saved field information
        // carried by this returned value. Other callee captures stay private.
        self.completed_nominal_values = std::sync::Arc::clone(&caller.completed_nominal_values);
        if !returned_nominal_values.is_empty() {
            std::sync::Arc::make_mut(&mut self.completed_nominal_values)
                .extend(returned_nominal_values);
        }
        // Preservation checks may read declaration-local scalar bindings.
        // Run them only after restoring the caller's scope and saved values.
        self.advance_storage_version(completion_version);
        Some(self)
    }
}
