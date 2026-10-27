//! Bind one completed actual roster before entering a checked runtime body.
//! Caller values are captured before any formal name is installed. Live storage
//! moves into the callee's declaration coordinates; detached evidence keeps its
//! identity. This imports neither the callee's requirements nor its completion.

use super::{StructuralJudge, StructuralTerm, TypeReferenceHandle};
use crate::machine_calls::calls::InvocationEntrySubject;
use typed_trees::{machine::Machine, signature::StateParameter, state::State};

impl<'program> StructuralJudge<'program> {
    pub(in crate::proof_contracts::contract_entailment) fn entry_argument_value(
        &self,
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
                let value = self
                    .entry_argument_value(parameter, value, subject, environment, scope)
                    .unwrap_or_else(|| {
                        StructuralTerm::Opaque("unobserved invocation argument".to_owned())
                    });
                (parameter.symbol, value)
            })
            .collect::<Vec<_>>();
        self.enter_storage_scope(entry, subjects, &bindings);
        // Expression caches and source premises are invocation-local. Their
        // captured equations/predicates remain detached values, not a license
        // to reevaluate a caller expression through new callee bindings.
        self.normal_results = std::sync::Arc::new(Vec::new());
        self.premise_facts.clear();
        self.declaration_state = entry.symbol;
        let (callee, _) = self.runtime_declaration_scope()?;
        self.body_type_parameters = self.program.machine_type_parameters(callee);
        let incoming = parameters
            .iter()
            .zip(bindings)
            .map(|(parameter, (_, value))| (parameter.name.to_string(), value))
            .collect();
        Some((incoming, self))
    }
}
