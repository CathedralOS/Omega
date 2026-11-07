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

#[derive(Debug)]
pub(crate) struct InvocationEntrySubject {
    pub parameter: SymbolHandle,
    pub argument: ExpressionHandle,
    pub parameter_type: TypeReferenceHandle,
    pub sources: Vec<LocalWriteSource>,
    pub type_reference: TypeReferenceHandle,
}

impl InvocationEntrySubject {
    pub(crate) fn has_aggregate_view(&self, program: &dyn crate::CheckingInput) -> bool {
        program
            .primitive_type_reference(self.type_reference)
            .is_none()
            && self
                .sources
                .iter()
                .any(|source| source.representation.is_some())
    }

    /// Resolve every possible supplier of one schema-derived scalar field.
    /// Missing geometry or an unsupported interpretation withholds the field;
    /// selecting only one supplier would hide an unsafe alternative.
    pub(crate) fn scalar_projection_coordinates(
        &self,
        program: &dyn crate::CheckingInput,
        state: &typed_trees::state::State,
        path: &[facts::PlaceSegment],
    ) -> Option<Vec<super::super::ScalarStorageCoordinate>> {
        if self.sources.is_empty() {
            return None;
        }
        self.sources
            .iter()
            .map(|source| {
                if source.invocation_output.is_some() || source.is_image_storage {
                    return None;
                }
                let mut origin = source.frame_origin();
                origin.source = origin.source.append_segments(path);
                let projected = LocalWriteSource::from(origin);
                let scalar = projected.scalar_storage_coordinate(program, state)?;
                (scalar.preserves_scalar_value || scalar.preserves_primitive_bits).then_some(scalar)
            })
            .collect()
    }

    /// The borrowed declaration and its scalar presentation share storage,
    /// not necessarily numerical values. The existing recast owner authenticates
    /// the complete chain before either direction can transfer observations.
    pub(crate) fn scalar_storage_coordinate(
        &self,
        program: &dyn crate::CheckingInput,
        state: &typed_trees::state::State,
    ) -> Option<super::super::ScalarStorageCoordinate> {
        program.primitive_type_reference(self.type_reference)?;
        let [source] = self.sources.as_slice() else {
            return None;
        };
        if source.invocation_output.is_some() || source.is_image_storage {
            return None;
        }
        let scalar = source.scalar_storage_coordinate(program, state)?;
        (scalar.preserves_scalar_value || scalar.preserves_primitive_bits).then_some(scalar)
    }
}

pub(crate) struct InvocationReferenceSubject {
    pub entry: InvocationEntrySubject,
    pub stable_on_return: bool,
    pub preserved_on_return: bool,
}

impl<'program> CallFrameResolver<'program> {
    /// Exact caller storage whose current contents a completing invocation
    /// returns through an exclusive formal. This transports a post-state goal,
    /// never the entry contents or a promise that those contents were preserved.
    /// Ambiguous suppliers and representation changes supply no correspondence.
    pub fn normal_return_reference_correspondence(
        &self,
        machine: &'program Machine,
        state: &typed_trees::state::State,
        call: &checked_trees::FlowCallFact,
        parameters: &[StateParameter],
    ) -> Option<Vec<(SymbolHandle, crate::CanonicalPlace)>> {
        if !call.normal_return_writes_complete {
            return None;
        }
        let target = crate::call_target_state(self.program, call.target_symbol)?;
        let callee = self.program.machine_holding_state(target.symbol)?;
        let entries = match crate::find_call_site(
            self.program,
            machine.symbol,
            state.symbol,
            call.statement_index,
            call.call_ordinal,
        )? {
            crate::CallSite::Expression { expression, .. } => {
                self.expression_entry_reference_subjects(machine, expression, parameters)
            }
            crate::CallSite::Statement(call) => {
                self.statement_entry_reference_subjects(machine, call, parameters)
            }
            crate::CallSite::TransitionNamed { .. } => {
                let target = crate::transition_call_target(
                    self.program,
                    machine,
                    state,
                    call.statement_index,
                    call.call_ordinal,
                )?;
                self.transition_entry_reference_subjects(machine, target, parameters)
            }
        }?;
        Some(
            entries
                .into_iter()
                .filter_map(|entry| {
                    let parameter = parameters
                        .iter()
                        .find(|parameter| parameter.symbol == entry.parameter)?;
                    let shell = crate::reference_shell(self.program, parameter.type_reference)?;
                    // Mutable referent access does not replace the reference
                    // itself. Check the destination body's descriptor identity.
                    if !shell.access.is_exclusive()
                        || !crate::state_reference_parameter_binding_is_stable(
                            self.program,
                            callee,
                            target,
                            parameter.symbol,
                        )
                    {
                        return None;
                    }
                    let [source] = entry.sources.as_slice() else {
                        return None;
                    };
                    if source.invocation_output.is_some()
                        || source.is_image_storage
                        || source.representation.is_some()
                        || !source.recasts.is_empty()
                        || source.collection_coarse
                    {
                        return None;
                    }
                    Some((
                        entry.parameter,
                        crate::CanonicalPlace {
                            root: facts::PlaceRoot::Symbol(source.root),
                            segments: source.segments.clone(),
                        },
                    ))
                })
                .collect(),
        )
    }

    pub(crate) fn call_reference_subjects(
        &self,
        machine: &'program Machine,
        site: crate::CallSite<'program>,
        parameters: &[StateParameter],
        writes: &facts::NormalizedWriteFrame,
    ) -> Option<Vec<InvocationReferenceSubject>> {
        let entries = match site {
            crate::CallSite::Expression { expression, .. } => {
                self.expression_entry_reference_subjects(machine, expression, parameters)
            }
            crate::CallSite::Statement(call) => {
                self.statement_entry_reference_subjects(machine, call, parameters)
            }
            _ => return None,
        };
        if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
            eprintln!(
                "STRUCT invocation-entry-subjects caller={:?} entries={entries:?}",
                machine.symbol
            );
        }
        let entries = entries?;
        let writes = writes.complete_paths();
        entries
            .into_iter()
            .map(|entry| {
                let parameter = parameters
                    .iter()
                    .find(|parameter| parameter.symbol == entry.parameter)?;
                let shell = crate::reference_shell(self.program, parameter.type_reference)?;
                // Entry origins were captured before invocation. An unknown
                // future frame withholds return correspondence, not those
                // independently established incoming subjects.
                let stable_on_return =
                    writes.is_some() && (!parameter.is_mutable || shell.access.is_exclusive());
                let preserved_on_return = stable_on_return
                    && writes.as_ref().is_some_and(|writes| {
                        !writes.iter().any(|written| {
                            entry
                                .sources
                                .iter()
                                .any(|source| super::frame_paths_overlap(written, &source.path))
                        })
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
        let mut entries = Vec::new();
        for ((parameter, argument), origins) in parameters
            .iter()
            .zip(arguments.get(leading_operands..)?)
            .zip(origins.arguments.get(leading_operands..)?)
        {
            match self.prepare_entry_reference_subject(
                machine, state, index, parameter, *argument, origins,
            ) {
                Ok(Some(subject)) => entries.push(subject),
                Ok(None) => {}
                Err(reason) => {
                    if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
                        eprintln!(
                            "STRUCT reference-entry-refused caller={} parameter={} argument={argument:?} reason={reason}",
                            machine.name, parameter.name,
                        );
                    }
                }
            }
        }
        Some(entries)
    }

    /// Check that every source of this borrowed argument supports its logical type.
    /// Keep the complete source list so later writes invalidate all affected
    /// observations. Ordinary value parameters need no borrowed source record.
    fn prepare_entry_reference_subject(
        &self,
        machine: &'program Machine,
        state: &typed_trees::state::State,
        index: usize,
        parameter: &StateParameter,
        argument: ExpressionHandle,
        origins: &Option<Vec<LocalWriteSource>>,
    ) -> Result<Option<InvocationEntrySubject>, &'static str> {
        let program = self.program;
        let Some(shell) = crate::reference_shell(program, parameter.type_reference) else {
            return Ok(None);
        };
        let origins = origins
            .as_ref()
            .ok_or("the argument has no captured source list")?;
        if origins.is_empty() {
            return Err("the argument source list is empty");
        }
        if program.primitive_type_reference(shell.referee).is_none()
            && origins.iter().any(|source| source.representation.is_some())
        {
            let suppliers = self
                .completed_reference_read_sources(machine, state, argument)
                .ok_or("the completed descriptor has no readable supplier list")?;
            // Entry observations and later write checks must retain
            // the same complete supplier list. Compare full records
            // in both directions; neither query may omit a source.
            if suppliers.len() != origins.len()
                || !origins.iter().all(|source| suppliers.contains(source))
                || !suppliers.iter().all(|source| origins.contains(source))
            {
                return Err("the call origins and completed descriptor suppliers differ");
            }
        }
        for source in origins {
            if !source.root.is_valid()
                || (source.collection_coarse
                    && source.representation.is_none()
                    && !source
                        .segments
                        .iter()
                        .any(|segment| matches!(segment, facts::PlaceSegment::FixedIndex { .. })))
            {
                return Err("a source has no declaration root or usable collection coordinate");
            }
            let root_type =
                reference_subjects::source_root_type(program, state, index, source.root)
                    .ok_or("a source has no declaration type at this call")?;
            let root_type = reference_subjects::carrier_storage_type(program, root_type)
                .ok_or("a source carrier does not expose readable storage")?;
            let reference = if let Some(expected) = program.primitive_type_reference(shell.referee)
            {
                if source.invocation_output.is_some() || source.is_image_storage {
                    return Err("a scalar source has no declaration-relative storage");
                }
                let scalar = source
                    .scalar_storage_coordinate(program, state)
                    .ok_or("the scalar view has no exact storage interpretation")?;
                if scalar.view_primitive != expected
                    || !(scalar.preserves_scalar_value || scalar.preserves_primitive_bits)
                {
                    return Err("the scalar storage interpretation differs from the formal");
                }
                // Use the scalar read's checked logical type. Walking its
                // backing again would lose intermediate stored references.
                // This check identifies the type; it does not observe contents.
                scalar.view_reference
            } else if let Some((reference, selectors)) = &source.representation {
                // The completed descriptor retains its view and every
                // readable supplier. Its fields belong to that view,
                // not to the byte array that supplies its backing.
                reference_subjects::validate_owned_projection(program, *reference, selectors)
                    .ok_or("the logical view path has no valid owned projection")?;
                stored_origins::projected_type(program, *reference, selectors)
                    .ok_or("the logical view path has no projected carrier")?
            } else {
                reference_subjects::validate_owned_projection(program, root_type, &source.segments)
                    .ok_or("the declaration path has no valid owned projection")?;
                declaration_preserving_views(program, root_type, source)
                    .ok_or("the source view does not preserve its declaration namespace")?;
                stored_origins::projected_type(program, root_type, &source.segments)
                    .ok_or("the declaration path has no projected carrier")?
            };
            if !crate::declared_carriers_match(program, reference, shell.referee)
                || program.arithmetic_domain_for_type_reference(reference)
                    != program.arithmetic_domain_for_type_reference(shell.referee)
            {
                return Err("the logical carrier or arithmetic policy differs from the formal");
            }
        }
        Ok(Some(InvocationEntrySubject {
            parameter: parameter.symbol,
            argument,
            parameter_type: parameter.type_reference,
            sources: origins.clone(),
            type_reference: shell.referee,
        }))
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
