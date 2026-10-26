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
    pub argument: ExpressionHandle,
    pub parameter_type: TypeReferenceHandle,
    pub sources: Vec<LocalWriteSource>,
    pub type_reference: TypeReferenceHandle,
    pub stable_on_return: bool,
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
        // The invocation origin capture authenticates each operand before the
        // callee runs and retires sources changed by earlier operands. A
        // whole-call binding-exposure veto would also discard the stable
        // destination of a descriptor store, preventing its fresh post-state
        // from consuming the callee's guarantees. Preservation after return is
        // decided below from the actual write frame, independently per subject.
        let (state, _, index) =
            caller_statement_owner(program, machine, CallerWriteSite::Expression(expression))?;
        // The observer has completed the operands and captured their origins.
        // Preservation is across the callee operation, not a replay of the
        // whole expression's earlier writes or binding-exposure fence.
        let writes = self
            .operation_write_frame(machine, expression)
            .into_complete_paths()?;
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
                // A selected reference can have several possible backings.
                // Authenticate the entire join; predicate truth belongs to the
                // actual selected on a normal path, not every possible payload.
                for source in origins {
                    if !source.root.is_valid()
                        || (source.collection_coarse
                            && !source
                                .segments
                                .iter()
                                .any(|segment| matches!(segment, facts::PlaceSegment::FixedIndex { .. })))
                    {
                        return None;
                    }
                    let root_type =
                        reference_subjects::source_root_type(program, state, index, source.root)?;
                    let root_type = reference_subjects::carrier_storage_type(program, root_type)?;
                    declaration_preserving_views(program, root_type, source)?;
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
                }
                Some(InvocationReferenceSubject {
                    parameter: parameter.symbol,
                    argument: *argument,
                    parameter_type: parameter.type_reference,
                    sources: origins.clone(),
                    type_reference: shell.referee,
                    // Exclusive parameters lend referent storage, not a mutable
                    // descriptor home. Their mutability bit records access.
                    stable_on_return: !parameter.is_mutable || shell.access.is_exclusive(),
                    preserved_on_return: (!parameter.is_mutable || shell.access.is_exclusive())
                        && !writes.iter().any(|written| {
                            origins
                                .iter()
                                .any(|source| super::frame_paths_overlap(written, &source.path))
                        }),
                })
            })
            .collect();
        Some(subjects)
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
