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
        let program = self.program;
        let call = program.expression_table.call_node(expression)?;
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
            .any(|parameter| crate::reference_shell(program, parameter.type_reference).is_some())
        {
            return parameters
                .iter()
                .all(|parameter| self.proof_value_is_caller_isolated(parameter.type_reference))
                .then(Vec::new);
        }
        if !self.expression_reference_bindings_are_stable(machine, expression) {
            return None;
        }
        let (state, _, index) =
            caller_statement_owner(program, machine, CallerWriteSite::Expression(expression))?;
        let writes = self.expression_may_write_paths(machine, expression)?;
        let origins = self.expression_call_write_origins(machine, expression)?;
        if origins.arguments.len() != arguments.len() {
            return None;
        }
        // Observation is per formal. An output descriptor or an opaque owned
        // aggregate supplies no readable subject, but cannot suppress the
        // independently authenticated input subjects of this invocation.
        // Consumers look up each required formal and refuse missing rows.
        let subjects = parameters
            .iter()
            .zip(origins.arguments.get(leading_operands..)?)
            .filter_map(|(parameter, origins)| {
                let shell = crate::reference_shell(program, parameter.type_reference)?;
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
                reference_subjects::validate_owned_projection(
                    program,
                    root_type,
                    &source.segments,
                )?;
                let type_reference =
                    stored_origins::projected_type(program, root_type, &source.segments)?;
                if !crate::declared_carriers_match(program, type_reference, shell.referee)
                    || program.arithmetic_domain_for_type_reference(type_reference)
                        != program.arithmetic_domain_for_type_reference(shell.referee)
                {
                    return None;
                }
                Some(InvocationReferenceSubject {
                    parameter: parameter.symbol,
                    source: source.clone(),
                    type_reference,
                    preserved_on_return: !parameter.is_mutable
                        && !writes
                            .iter()
                            .any(|written| super::frame_paths_overlap(written, &source.path)),
                })
            })
            .collect();
        Some(subjects)
    }
}
