//! Entry subjects and their preservation in an invocation's live storage frame.
//!
//! These rows describe referents, not detached argument values or returned
//! reference origins. Complete disjoint write footprints preserve a subject;
//! binding exposure and exact prefix origin capture remain independent gates.
//! A consumer still owns value observations and loan/access authorization.

use super::super::caller_aliases::{CallerWriteSite, caller_statement_owner};
use super::super::{LocalWriteSource, reference_subjects, stored_origins};
use super::CallFrameResolver;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::types::TypeReferenceHandle;

pub(crate) struct InvocationReferenceSubject {
    pub parameter: SymbolHandle,
    pub source: LocalWriteSource,
    pub type_reference: TypeReferenceHandle,
    pub preserved_on_return: bool,
}

impl<'program> CallFrameResolver<'program> {
    /// Capture the readable entry subjects of the exact public telescope.
    /// A mutable formal binding or a written referent is explicitly not a
    /// preserved normal-return subject; its post-state relation remains owed.
    /// Coarse/ambiguous origins cannot identify an observation by spelling.
    pub(crate) fn expression_call_reference_subjects(
        &self,
        machine: &'program Machine,
        expression: ExpressionHandle,
        parameters: &[StateParameter],
    ) -> Option<Vec<InvocationReferenceSubject>> {
        let requested = parameters
            .iter()
            .map(|parameter| parameter.symbol)
            .collect::<Vec<_>>();
        self.call_reference_subjects(
            machine,
            expression,
            parameters,
            &requested,
            &self.expression_write_frame(machine, expression),
        )
    }

    /// Dependency-projected subjects of the same exact invocation. Unused
    /// owned operands grant no reference relation and require no isolation.
    pub(crate) fn expression_call_reference_subjects_for_parameters(
        &self,
        machine: &'program Machine,
        expression: ExpressionHandle,
        parameters: &[StateParameter],
        requested: &[SymbolHandle],
        writes: &facts::NormalizedWriteFrame,
    ) -> Option<Vec<InvocationReferenceSubject>> {
        self.call_reference_subjects(machine, expression, parameters, requested, writes)
    }

    fn call_reference_subjects(
        &self,
        machine: &'program Machine,
        expression: ExpressionHandle,
        parameters: &[StateParameter],
        requested: &[SymbolHandle],
        writes: &facts::NormalizedWriteFrame,
    ) -> Option<Vec<InvocationReferenceSubject>> {
        let program = self.program;
        let call = program.expression_table.source_call_node(expression)?;
        let arguments = program.expression_table.expression_handles(call.arguments);
        let leading_operands =
            if let Some(slot) = program.dynamic_slot_by_symbol(call.target_symbol) {
                // The slot pairs every evaluated operand with its own fresh
                // invocation formal. Public facts retain the requirement's symbols,
                // rejoined here only by its exact declaration-owned positions.
                let contract =
                    crate::machine_calls::calls::callee_entry_contract(program, slot.symbol)?;
                let declared = contract.parameters;
                let instantiated = slot.data_parameters();
                if declared.len() != parameters.len()
                    || instantiated.len() != parameters.len()
                    || !parameters.iter().zip(declared).zip(instantiated).all(
                        |((parameter, declared), instantiated)| {
                            parameter.symbol == declared.symbol
                                && parameter.type_reference == instantiated.type_reference
                        },
                    )
                    || slot.parameters.len() != arguments.len()
                {
                    return None;
                }
                contract.leading_operands
            } else {
                0
            };
        if parameters.len() != arguments.get(leading_operands..)?.len() {
            return None;
        }
        if !parameters
            .iter()
            .filter(|parameter| requested.contains(&parameter.symbol))
            .any(|parameter| crate::reference_shell(program, parameter.type_reference).is_some())
        {
            return parameters
                .iter()
                .filter(|parameter| requested.contains(&parameter.symbol))
                .all(|parameter| self.proof_value_is_caller_isolated(parameter.type_reference))
                .then(Vec::new);
        }
        if !self.expression_reference_bindings_are_stable(machine, expression) {
            return None;
        }
        let (state, _, index) =
            caller_statement_owner(program, machine, CallerWriteSite::Expression(expression))?;
        let origins = self.expression_call_write_origins(machine, expression)?;
        if origins.arguments.len() != arguments.len() {
            return None;
        }
        let mut subjects = Vec::new();
        for (parameter, origins) in parameters
            .iter()
            .zip(origins.arguments.get(leading_operands..)?)
        {
            if !requested.contains(&parameter.symbol) {
                continue;
            }
            let Some(shell) = crate::reference_shell(program, parameter.type_reference) else {
                // An owned aggregate can itself carry live reference leaves.
                // A detached whole-value term is not their post-state relation.
                if !self.proof_value_is_caller_isolated(parameter.type_reference) {
                    return None;
                }
                continue;
            };
            if !shell.access.is_readable() {
                return None;
            }
            let origins = origins.as_ref()?;
            let source = origins.first()?;
            if !source.root.is_valid()
                || source.collection_coarse
                || !source.recasts.is_empty()
                || source.representation.is_some()
                || origins.iter().any(|candidate| candidate != source)
            {
                return None;
            }
            let root_type =
                reference_subjects::source_root_type(program, state, index, source.root)?;
            let root_type = reference_subjects::carrier_storage_type(program, root_type)?;
            reference_subjects::validate_owned_projection(program, root_type, &source.segments)?;
            let type_reference =
                stored_origins::projected_type(program, root_type, &source.segments)?;
            if !crate::declared_carriers_match(program, type_reference, shell.referee)
                || program.arithmetic_domain_for_type_reference(type_reference)
                    != program.arithmetic_domain_for_type_reference(shell.referee)
            {
                return None;
            }
            subjects.push(InvocationReferenceSubject {
                parameter: parameter.symbol,
                source: source.clone(),
                type_reference,
                preserved_on_return: !parameter.is_mutable
                    && writes.complete_paths().is_some_and(|writes| {
                        !writes
                            .iter()
                            .any(|written| super::frame_paths_overlap(written, &source.path))
                    }),
            });
        }
        Some(subjects)
    }
}
