//! Capture an invocation's entry subjects, then judge return preservation.
//! Entry correspondence is established after actual operands and before callee
//! execution. Future write summaries affect normal return, never entry origin.

use super::super::caller_aliases::{CallerWriteSite, caller_statement_owner};
use super::super::{LocalWriteSource, reference_subjects, stored_origins};
use super::CallFrameResolver;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::types::TypeReferenceHandle;

pub(crate) struct InvocationEntrySubject {
    pub parameter: SymbolHandle,
    pub argument: ExpressionHandle,
    pub parameter_type: TypeReferenceHandle,
    pub sources: Vec<LocalWriteSource>,
    pub type_reference: TypeReferenceHandle,
}

pub(crate) struct InvocationReferenceSubject {
    pub entry: InvocationEntrySubject,
    pub stable_on_return: bool,
    pub preserved_on_return: bool,
}

impl<'program> CallFrameResolver<'program> {
    pub(crate) fn expression_call_reference_subjects(
        &self,
        machine: &'program Machine,
        expression: ExpressionHandle,
        parameters: &[StateParameter],
    ) -> Option<Vec<InvocationReferenceSubject>> {
        let writes = self
            .operation_write_frame(machine, expression)
            .into_complete_paths()?;
        self.expression_entry_reference_subjects(machine, expression, parameters)?
            .into_iter()
            .map(|entry| {
                let parameter = parameters
                    .iter()
                    .find(|parameter| parameter.symbol == entry.parameter)?;
                let shell = crate::reference_shell(self.program, parameter.type_reference)?;
                let stable_on_return = !parameter.is_mutable || shell.access.is_exclusive();
                let preserved_on_return = stable_on_return
                    && !writes.iter().any(|written| {
                        entry
                            .sources
                            .iter()
                            .any(|source| super::frame_paths_overlap(written, &source.path))
                    });
                Some(InvocationReferenceSubject {
                    entry,
                    stable_on_return,
                    preserved_on_return,
                })
            })
            .collect()
    }

    pub(crate) fn expression_entry_reference_subjects(
        &self,
        machine: &'program Machine,
        expression: ExpressionHandle,
        parameters: &[StateParameter],
    ) -> Option<Vec<InvocationEntrySubject>> {
        self.entry_reference_subjects(machine, CallerWriteSite::Expression(expression), parameters)
    }

    pub(crate) fn statement_entry_reference_subjects(
        &self,
        machine: &'program Machine,
        call: &'program typed_trees::statement::TableCall,
        parameters: &[StateParameter],
    ) -> Option<Vec<InvocationEntrySubject>> {
        self.entry_reference_subjects(machine, CallerWriteSite::Call(call), parameters)
    }

    pub(crate) fn transition_entry_reference_subjects(
        &self,
        machine: &'program Machine,
        target: typed_trees::statement::TransitionTargetHandle,
        parameters: &[StateParameter],
    ) -> Option<Vec<InvocationEntrySubject>> {
        self.entry_reference_subjects(machine, CallerWriteSite::Transition(target), parameters)
    }

    fn entry_reference_subjects(
        &self,
        machine: &'program Machine,
        site: CallerWriteSite<'program>,
        parameters: &[StateParameter],
    ) -> Option<Vec<InvocationEntrySubject>> {
        let program = self.program;
        let (arguments, leading_operands) = match site {
            CallerWriteSite::Expression(expression) => {
                let call = program.expression_table.call_node(expression)?;
                let arguments = program.expression_table.expression_handles(call.arguments);
                let leading = if let Some(slot) = program.dynamic_slot_by_symbol(call.target_symbol)
                {
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
                (arguments, leading)
            }
            CallerWriteSite::Call(call) => (
                program.statement_table.expression_handles(call.arguments),
                0,
            ),
            CallerWriteSite::Transition(target) => {
                let typed_trees::statement::TransitionTargetNode::Named { arguments, .. } =
                    program.statement_table.transition_target(target)
                else {
                    return None;
                };
                (program.statement_table.expression_handles(*arguments), 0)
            }
            _ => return None,
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
        let origins = match site {
            CallerWriteSite::Expression(expression) => {
                self.expression_call_write_origins(machine, expression)?
            }
            CallerWriteSite::Call(call) => self.call_write_origins(machine, call)?,
            CallerWriteSite::Transition(target) => {
                self.transition_write_origins(machine, target)?
            }
            _ => return None,
        };
        if origins.arguments.len() != arguments.len() {
            return None;
        }
        let (state, _, index) = caller_statement_owner(program, machine, site)?;
        Some(
            parameters
                .iter()
                .zip(arguments.get(leading_operands..)?)
                .zip(origins.arguments.get(leading_operands..)?)
                .filter_map(|((parameter, argument), origins)| {
                    let shell = crate::reference_shell(program, parameter.type_reference)?;
                    if !shell.access.is_readable() {
                        return None;
                    }
                    let origins = origins.as_ref()?;
                    if origins.is_empty() {
                        return None;
                    }
                    for source in origins {
                        if !source.root.is_valid()
                            || (source.collection_coarse
                                && !source.segments.iter().any(|segment| {
                                    matches!(segment, facts::PlaceSegment::FixedIndex { .. })
                                }))
                        {
                            return None;
                        }
                        let root_type = reference_subjects::source_root_type(
                            program,
                            state,
                            index,
                            source.root,
                        )?;
                        let root_type =
                            reference_subjects::carrier_storage_type(program, root_type)?;
                        declaration_preserving_views(program, root_type, source)?;
                        reference_subjects::validate_owned_projection(
                            program,
                            root_type,
                            &source.segments,
                        )?;
                        let reference =
                            stored_origins::projected_type(program, root_type, &source.segments)?;
                        if !crate::declared_carriers_match(program, reference, shell.referee)
                            || program.arithmetic_domain_for_type_reference(reference)
                                != program.arithmetic_domain_for_type_reference(shell.referee)
                        {
                            return None;
                        }
                    }
                    Some(InvocationEntrySubject {
                        parameter: parameter.symbol,
                        argument: *argument,
                        parameter_type: parameter.type_reference,
                        sources: origins.clone(),
                        type_reference: shell.referee,
                    })
                })
                .collect(),
        )
    }
}

/// A qualification view observes the original declaration, not a new byte
/// interpretation. Authenticate the complete retained chain before using its
/// declaration selectors. Recast provenance remains on the source for other
/// consumers; this grants neither formation validity nor additional access.
fn declaration_preserving_views(
    program: &dyn crate::CheckingInput,
    root_type: TypeReferenceHandle,
    source: &LocalWriteSource,
) -> Option<()> {
    let mut presentation = None;
    let mut prefix: &[facts::PlaceSegment] = &[];
    for recast in &source.recasts {
        if recast.root != source.root {
            return None;
        }
        match (presentation, &recast.representation) {
            (None, None) => {}
            (Some(previous), Some((reference, selectors)))
                if previous == *reference && recast.segments.strip_prefix(prefix)? == selectors => {
            }
            _ => return None,
        }
        reference_subjects::validate_owned_projection(program, root_type, &recast.segments)?;
        let backing = stored_origins::projected_type(program, root_type, &recast.segments)?;
        let formation = crate::reference_formation_source(program, recast.formation.expression())?;
        if formation.interpretation != crate::ReferenceFormationInterpretation::RepresentationView
            || !formation.view_type.is_valid()
            || formation.view_type != recast.target_type
            || crate::reference_view_storage(program, backing, recast.target_type)?
                != checked_trees::CheckedReferenceStorage::Declared
        {
            return None;
        }
        presentation = Some(recast.target_type);
        prefix = &recast.segments;
    }
    match (presentation, &source.representation) {
        (None, None) => Some(()),
        (Some(previous), Some((reference, selectors)))
            if previous == *reference && source.segments.strip_prefix(prefix)? == selectors =>
        {
            Some(())
        }
        _ => None,
    }
}
