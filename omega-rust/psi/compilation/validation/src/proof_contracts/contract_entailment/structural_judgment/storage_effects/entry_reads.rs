//! Input predicates can constrain values before the body loads their pointers.
//! Keep those initial values separate from reads performed by the body. They
//! describe the invocation's inputs and cannot authorize an actual read. A body
//! read must still establish its own pointer suppliers and evaluated selectors.

use super::super::StructuralTerm;
use super::{CapturedStorageCoordinate, StructuralJudge};
use typed_trees::{expression::ExpressionHandle, machine::Machine, state::State};

/// Largest fixed array whose slots each keep an entry observation.
const ENTRY_SLOT_OBSERVATION_LIMIT: usize = 64;

impl StructuralJudge<'_> {
    /// Discover input scalar coordinates without admitting the surrounding
    /// predicate. The entry owner separately checks each coordinate's cases,
    /// bounds and readable storage before retaining any observation.
    pub(super) fn entry_scalar_read_coordinates(
        &self,
        machine: &Machine,
        state: &State,
        fact: ExpressionHandle,
    ) -> Vec<(crate::CanonicalPlace, ExpressionHandle)> {
        let mut expressions = Vec::new();
        crate::queries::expression_traversal::collect_expression_tree(
            self.program,
            fact,
            &mut expressions,
        );
        let mut coordinates = Vec::new();
        for expression in expressions {
            if super::super::super::observed_projections::scalar_storage_read(
                self.program,
                machine,
                state,
                expression,
            )
            .is_none()
                || !self.ordinary_storage_path(Some((machine, state)), expression)
            {
                continue;
            }
            let Some(place) = crate::canonical_place_from_expression(self.program, expression)
            else {
                continue;
            };
            if !place.root.as_symbol().is_some_and(|symbol| {
                self.program
                    .state_parameter_by_symbol(state, symbol)
                    .is_some()
            }) || self.capture_entry_storage_place(place.clone()).is_none()
                || coordinates.iter().any(|(retained, _)| retained == &place)
            {
                continue;
            }
            coordinates.push((place, expression));
        }
        coordinates
    }

    /// Name initialized input scalars needed by the return contract, even when
    /// Requires does not constrain their values. The contract selects only
    /// coordinates: each new value stays unknown, and its read must already be
    /// well formed under the admitted entry facts. No guarantee is assumed.
    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn retain_declared_entry_scalar_reads(
        &mut self,
        machine: &Machine,
        state: &State,
    ) {
        use typed_trees::domain::ProofFact;
        use typed_trees::signature::SignatureContractKind;

        let program = self.program;
        let entry = program
            .machine_states(machine)
            .first()
            .is_some_and(|entry| entry.symbol == state.symbol);
        for contract in program
            .state_contracts(state)
            .iter()
            .chain(program.machine_contracts(machine).iter().filter(|_| entry))
            .filter(|contract| contract.kind == SignatureContractKind::Ensures)
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let ProofFact::Expression(expression) = fact else {
                    continue;
                };
                for (place, _) in self.entry_scalar_read_coordinates(machine, state, *expression) {
                    // The shared formation check authenticates declared cases
                    // and literal indices. An unproved contract cannot supply
                    // a dynamic selector's missing bounds or range evidence.
                    if !place.segments.iter().all(|segment| {
                        matches!(
                            segment,
                            facts::PlaceSegment::Field { .. }
                                | facts::PlaceSegment::Case { .. }
                                | facts::PlaceSegment::FixedIndex { .. }
                        )
                    }) {
                        continue;
                    }
                    let Some(coordinate) = self.capture_entry_storage_place(place.clone()) else {
                        continue;
                    };
                    if self
                        .stored_values
                        .value_at(&coordinate)
                        .is_some_and(|stored| stored.entry.is_some())
                    {
                        continue;
                    }
                    let _ = self.retain_entry_predicate_scalar(machine, state, place);
                }
            }
        }
    }

    /// A readable scalar parameter supplies initialized entry contents even
    /// when Requires says nothing about their value. Keep that unknown value
    /// only after the frame owner authenticates every reference crossing.
    /// Otherwise an unrelated counter update can discard the whole prefix.
    /// This is an entry observation, not an executed read or a new loan;
    /// ordinary writes still retire its current contents.
    /// Declaration checking supplies symbolic entry values; a completed call
    /// supplies only its independently authenticated actual observations.
    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn retain_entry_scalar_parameters(
        &mut self,
        machine: &Machine,
        state: &State,
        values: impl IntoIterator<Item = (symbols::SymbolHandle, StructuralTerm)>,
    ) {
        for (symbol, value) in values {
            let Some(parameter) = self.program.state_parameter_by_symbol(state, symbol) else {
                continue;
            };
            if parameter.is_const || parameter.relevance.is_erased() {
                continue;
            }
            let place = crate::CanonicalPlace {
                root: facts::PlaceRoot::Symbol(parameter.symbol),
                segments: Vec::new(),
            };
            if crate::value_custody::expression_types::scalar_observation_meaning(
                self.program,
                parameter.type_reference,
            )
            .is_none()
            {
                self.retain_entry_scalar_fields(
                    machine,
                    state,
                    parameter.type_reference,
                    place,
                    Some(value),
                    &mut Vec::new(),
                );
                continue;
            }
            if let Err(reason) = self.retain_entry_scalar_observation(
                machine,
                state,
                crate::CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(parameter.symbol),
                    segments: Vec::new(),
                },
                value,
            ) && super::super::super::struct_trace()
            {
                eprintln!(
                    "STRUCT entry-scalar refused machine={} parameter={} reason={reason}",
                    machine.name, parameter.name,
                );
            }
        }
    }

    /// Nominal input predicates constrain exact entry field values, too. Keep
    /// their independently authenticated scalar coordinates so a disjoint
    /// write cannot erase the unchanged sibling used to restore the parent.
    /// This never captures an owned reference descriptor or an inactive case.
    fn retain_entry_scalar_fields(
        &mut self,
        machine: &Machine,
        state: &State,
        reference: typed_trees::types::TypeReferenceHandle,
        place: crate::CanonicalPlace,
        value: Option<StructuralTerm>,
        active: &mut Vec<symbols::SymbolHandle>,
    ) -> Option<StructuralTerm> {
        let program = self.program;
        // Arrays carry the same independently admitted element obligations as
        // nominal fields. Keep each declared slot; no missing slot can reduce
        // the frontier selected by a later runtime index.
        if let Some((element, length)) =
            crate::field_domains::readable_fixed_array_elements(program, reference)
        {
            if !Self::nominal_type_has_data_predicates(program, element, active) {
                return None;
            }
            // Every judge for a state repeats this walk, so a per-slot copy of
            // a 131072-slot table cost the Psi product gate about 45 ms per
            // judge and over an hour per check. Past the limit an array keeps
            // no entry slot observations, as before slot retention existed.
            // ponytail: slot limit; one quantified observation over all slots
            // would keep large arrays' bounds if a program needs them.
            if length > ENTRY_SLOT_OBSERVATION_LIMIT {
                return None;
            }
            for element_index in 0..length {
                let mut child = place.clone();
                child.segments.push(facts::PlaceSegment::FixedIndex {
                    index: element_index,
                });
                self.retain_entry_scalar_fields(
                    machine,
                    state,
                    element,
                    child,
                    value.clone().and_then(|value| {
                        super::super::super::observed_projections::project_index(
                            program,
                            reference,
                            value,
                            StructuralTerm::Integer(numerics::bignum::BigInt::from_u64(
                                u64::try_from(element_index).ok()?,
                            )),
                        )
                    }),
                    active,
                );
            }
            return None;
        }
        let definition = crate::field_domains::readable_nominal_definition(program, reference)?;
        if active.contains(&definition.symbol) {
            return None;
        }
        active.push(definition.symbol);
        let mut observed_fields = Vec::new();
        for field in program
            .data_members(definition)
            .iter()
            .filter_map(|member| member.as_field())
        {
            let path = [facts::PlaceSegment::Field {
                symbol: field.symbol,
            }];
            let Some(field_reference) =
                crate::project_type_reference_from_segments(program, reference, &path)
            else {
                continue;
            };
            let projected = value.clone().and_then(|value| {
                super::super::super::observed_projections::project_observed_value(
                    program, reference, value, &path,
                )
            });
            let mut child = place.clone();
            child.segments.extend_from_slice(&path);
            if crate::value_custody::expression_types::scalar_observation_meaning(
                program,
                field_reference,
            )
            .is_some()
            {
                // A record's input predicate can constrain a scalar before its
                // stored pointers are loaded. Retain a read only when those
                // loads are established; otherwise keep just the entry value.
                let observed = projected.filter(|projected| {
                    super::super::super::structural_terms::scalar_operand_is_observed(
                        program,
                        field_reference,
                        projected,
                    ) && child.root.as_symbol().is_some_and(|root| {
                        self.entry_read_backing(machine, state, root, &child.segments)
                            .is_some()
                    })
                });
                let observed = if let Some(observed) = observed {
                    self.retain_entry_scalar_observation(machine, state, child, observed)
                } else {
                    self.retain_entry_predicate_scalar(machine, state, child)
                };
                match observed {
                    Ok(observed) => observed_fields.push((field.symbol, observed)),
                    Err(reason) if super::super::super::struct_trace() => {
                        eprintln!(
                            "STRUCT entry-field refused machine={} field={} reason={reason}",
                            machine.name, field.name
                        );
                    }
                    Err(_) => {}
                }
            } else {
                let nested = self.retain_entry_scalar_fields(
                    machine,
                    state,
                    field_reference,
                    child,
                    projected,
                    active,
                );
                // Inline data contributes its complete value. A reference field
                // retains its descriptor/content distinction and cannot be
                // filled by a logical constructor of the referenced contents.
                if crate::reference_shell(program, field_reference).is_none()
                    && let Some(nested) = nested
                {
                    observed_fields.push((field.symbol, nested));
                }
            }
        }
        // This is an admitted invocation input, not a receiving annotation.
        // Bind its complete predicate only when every required field has an
        // authenticated entry observation. The constructor is a logical view
        // of those contents; it is never installed as a reference descriptor.
        let mut completed = None;
        if !program
            .data_members(definition)
            .iter()
            .any(|member| member.as_variant().is_some())
            && let Some(subject) = super::super::super::structural_terms::captured_constructor_term(
                program,
                definition,
                None,
                |field| {
                    observed_fields
                        .iter()
                        .find(|(symbol, _)| *symbol == field.symbol)
                        .map(|(_, value)| value.clone())
                },
            )
        {
            if super::super::super::struct_trace() {
                eprintln!(
                    "STRUCT admitted-entry-predicate data={} receiving={reference:?} place={place:?} subject={subject:?}",
                    definition.name
                );
            }
            self.intake_admitted_value_predicates(reference, subject.clone());
            completed = Some(subject);
        } else if super::super::super::struct_trace() {
            let missing = program
                .data_members(definition)
                .iter()
                .filter_map(|member| member.as_field())
                .filter(|field| {
                    !observed_fields
                        .iter()
                        .any(|(symbol, _)| *symbol == field.symbol)
                })
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>();
            eprintln!(
                "STRUCT admitted-entry-predicate-refused data={} place={place:?} missing_fields={missing:?} phase=complete-current-field-roster-or-case",
                definition.name
            );
        }
        active.pop();
        completed
    }

    /// An admitted input's predicate names its entry scalar values even when
    /// no descriptor load has executed. Such a variable is Entry only: it is
    /// not current contents, a performed read, a loan, or a physical identity.
    /// The common read owner may consume it only after the actual load chain
    /// and preservation from that input cut have independently been proved.
    fn retain_entry_predicate_scalar(
        &mut self,
        machine: &Machine,
        state: &State,
        place: crate::CanonicalPlace,
    ) -> Result<StructuralTerm, &'static str> {
        let geometry = crate::structural_scalar_coordinate(
            self.program,
            self.program.state_parameters(state),
            place
                .root
                .as_symbol()
                .ok_or("entry scalar has no declaration root")?,
            &place.segments,
        )
        .ok_or("entry field has no ordinary scalar geometry")?;
        let result_type =
            crate::value_custody::expression_types::readable_observation_type_reference(
                self.program,
                geometry.result_type_reference,
            )
            .ok_or("entry scalar has no readable carrier")?;
        self.program
            .primitive_type_reference(result_type)
            .filter(|primitive| *primitive == geometry.primitive_type)
            .ok_or("entry scalar carrier disagrees with its stored interpretation")?;
        // The same entry coordinate already names one value. Reuse it before
        // creating a fresh unknown; relating a discarded unknown back to that
        // value adds proof work without supplying another fact.
        let retained = self
            .capture_entry_storage_place(place.clone())
            .and_then(|coordinate| self.stored_values.value_at(&coordinate)?.entry.clone());
        let value = match retained {
            Some(value) => value,
            None => {
                self.capture_sequence += 1;
                StructuralTerm::CapturedValue {
                    statement: arena::Handle::invalid(),
                    expression: arena::Handle::invalid(),
                    callable: symbols::SymbolHandle::invalid(),
                    occurrence: self.capture_sequence,
                    result_type,
                    integer_range: None,
                }
            }
        };
        self.retain_entry_scalar_snapshot(machine, state, place, value)
    }

    /// Keep an admitted entry scalar even when its pointer has not been
    /// loaded yet. Such a snapshot is not current contents; a later read must
    /// establish its complete backing and preservation from this entry.
    pub(super) fn retain_entry_scalar_snapshot(
        &mut self,
        machine: &Machine,
        state: &State,
        place: crate::CanonicalPlace,
        value: StructuralTerm,
    ) -> Result<StructuralTerm, &'static str> {
        if !self.requires_value_preservation()
            || self.declaration_state != state.symbol
            || self.storage_write_sequence != 0
        {
            return Err("the scalar snapshot is outside its declaration entry epoch");
        }
        let root = place
            .root
            .as_symbol()
            .ok_or("entry predicate has no declaration root")?;
        let program = self.program;
        let parameter = program
            .state_parameter_by_symbol(state, root)
            .ok_or("entry predicate is not rooted in an admitted input")?;
        let geometry = crate::structural_scalar_coordinate(
            program,
            program.state_parameters(state),
            root,
            &place.segments,
        )
        .ok_or("entry snapshot has no readable scalar geometry")?;
        let observed_type =
            crate::value_custody::expression_types::readable_observation_type_reference(
                program,
                geometry.result_type_reference,
            )
            .filter(|reference| {
                program.primitive_type_reference(*reference) == Some(geometry.primitive_type)
            })
            .ok_or("entry snapshot has no matching scalar carrier")?;
        let (cases, indices) =
            super::super::super::scoped_arithmetic::scalar_projection_requirements(
                program,
                parameter.type_reference,
                &place.segments,
            )
            .ok_or("entry snapshot lost its projection requirements")?;
        if !self.scalar_coordinate_is_formed(
            parameter.type_reference,
            &StructuralTerm::Variable(parameter.name.to_string()),
            &place.segments,
            &cases,
            &indices,
        ) || !super::super::super::structural_terms::scalar_operand_is_observed(
            program,
            geometry.result_type_reference,
            &value,
        ) {
            return Err("entry snapshot lacks a formed scalar observation");
        }
        let coordinate = self
            .capture_entry_storage_place(place.clone())
            .ok_or("entry predicate has no fixed declaration coordinate")?;
        // A private call may already have transferred this scalar's exact
        // value, together with a guard about it. Reuse that entry snapshot
        // when installing its readable backing. Replacing current contents
        // with a fresh predicate variable would detach later reads from the
        // guard, even though no write occurred.
        let value = match self
            .stored_values
            .value_at(&coordinate)
            .and_then(|stored| stored.entry.clone())
        {
            Some(previous) => {
                if !previous.same_resolution_payload(&value) {
                    self.assume_completed_value_equation(observed_type, previous.clone(), value);
                }
                previous
            }
            None => value,
        };
        if self
            .entry_read_backing(machine, state, root, &place.segments)
            .is_some()
        {
            return self.retain_entry_scalar_observation(machine, state, place, value);
        }
        let values = std::sync::Arc::make_mut(&mut self.stored_values);
        let previous = if let Some(stored) = values.value_at_mut(&coordinate) {
            stored.entry.get_or_insert_with(|| value.clone()).clone()
        } else {
            values.push(super::CapturedStorageValue {
                coordinate,
                entry: Some(value.clone()),
                current: None,
                selected_case: None,
                completed_read: None,
                backing: None,
                current_anchors: vec![super::StorageAnchor::Incoming],
                anchor_state: state.symbol,
            });
            value.clone()
        };
        if super::super::super::struct_trace() {
            eprintln!(
                "STRUCT entry-predicate-only place={place:?} value={value:?} awaiting=performed-load-and-preservation"
            );
        }
        Ok(previous)
    }

    /// Whole entry values use the entry selector namespace too. A header
    /// coordinate has no executed selector occurrence in the body ledger.
    pub(in crate::proof_contracts::contract_entailment) fn captured_entry_place_projection(
        &self,
        place: crate::CanonicalPlace,
    ) -> Option<StructuralTerm> {
        self.requires_value_preservation().then_some(())?;
        let coordinate = self.capture_entry_storage_place(place)?;
        self.captured_projection(
            &coordinate,
            language_semantics::content::ContentPlaceVersion::Entry,
        )
    }

    pub(super) fn capture_entry_storage_place(
        &self,
        place: crate::CanonicalPlace,
    ) -> Option<CapturedStorageCoordinate> {
        if self.storage_write_sequence != 0 {
            return None;
        }
        self.capture_storage_place_with_selectors(place, |expression| {
            self.declaration_term(expression)
        })
    }

    pub(super) fn entry_read_backing(
        &self,
        machine: &Machine,
        state: &State,
        root: symbols::SymbolHandle,
        path: &[facts::PlaceSegment],
    ) -> Option<crate::machine_calls::calls::CapturedStorageReadBacking> {
        let geometry = crate::structural_coordinate(
            self.program,
            self.program.state_parameters(state),
            root,
            path,
        )?;
        let first = self
            .program
            .statement_table
            .admitted_execution_order(state.statement_nodes)
            .indices()
            .first()
            .copied()
            .unwrap_or(0);
        let frames = self.frame_resolver()?;
        let mut backing = crate::machine_calls::calls::CapturedStorageReadBacking {
            sources: Vec::new(),
            values: Vec::new(),
            bindings: Vec::new(),
        };
        // Inline elements authenticate their containing collection. Stored
        // reference crossings retain the complete selected load requirement.
        // Geometry's list is authoritative; a non-executed dynamic selector
        // must never masquerade as an executed element-load occurrence.
        for requirement in &geometry.source_requirements {
            let part = frames.storage_read_backing_before_statement(
                machine,
                state,
                first,
                root,
                requirement,
            );
            let Some(part) = part else {
                if super::super::super::struct_trace() {
                    eprintln!(
                        "STRUCT entry-load-refused root={root:?} path={path:?} requirement={requirement:?} phase=complete-load-frontier"
                    );
                }
                return None;
            };
            for source in part.sources {
                if !backing.sources.contains(&source) {
                    backing.sources.push(source);
                }
            }
            for value in part.values {
                if !backing.values.contains(&value) {
                    backing.values.push(value);
                }
            }
            for binding in part.bindings {
                if !backing.bindings.contains(&binding) {
                    backing.bindings.push(binding);
                }
            }
        }
        (!backing.sources.is_empty()).then_some(backing)
    }

    /// The caller has independently admitted the source observation's geometry,
    /// selectors and formation. Authenticate its exact readable backing at entry
    /// before freezing its scalar value; no declaration grants a later reread.
    pub(in crate::proof_contracts::contract_entailment) fn admitted_entry_scalar_value(
        &mut self,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> Result<StructuralTerm, &'static str> {
        if self.declaration_state != state.symbol || self.storage_write_sequence != 0 {
            return Err("the rank entry observation is outside its declaration entry epoch");
        }
        if !self.ordinary_storage_path(Some((machine, state)), expression) {
            return Err("the rank entry input is not a selected builtin storage path");
        }
        super::super::super::observed_projections::scalar_storage_read(
            self.program,
            machine,
            state,
            expression,
        )
        .ok_or("the rank entry input has no declared primitive storage-read geometry")?;
        let canonical = crate::canonical_place_from_expression(self.program, expression)
            .ok_or("the rank entry input has no canonical storage coordinate")?;
        let value = self
            .declaration_term(expression)
            .ok_or("the rank entry input has no admitted entry value description")?;
        self.retain_entry_scalar_observation(machine, state, canonical, value)
    }

    /// A declared ordered view contributes resolved projection identities,
    /// rather than manufactured member-expression occurrences. Authenticate
    /// its complete coordinate and project the authored base's same entry
    /// value through the ordinary typed observation owner.
    pub(in crate::proof_contracts::contract_entailment) fn admitted_entry_scalar_projection(
        &mut self,
        machine: &Machine,
        state: &State,
        base: ExpressionHandle,
        projection: &[facts::PlaceSegment],
    ) -> Result<StructuralTerm, &'static str> {
        if projection.is_empty() {
            // Preserve the authored scalar-read gate, including the distinction
            // between a reference descriptor and an observed scalar referent.
            return self.admitted_entry_scalar_value(machine, state, base);
        }
        if self.declaration_state != state.symbol || self.storage_write_sequence != 0 {
            return Err("the rank entry observation is outside its declaration entry epoch");
        }
        if !self.ordinary_storage_path(Some((machine, state)), base) {
            return Err("the rank entry base is not a selected builtin storage path");
        }
        let mut canonical = crate::canonical_place_from_expression(self.program, base)
            .ok_or("the rank entry base has no canonical storage coordinate")?;
        let root = canonical
            .root
            .as_symbol()
            .ok_or("the rank entry base has no declared storage root")?;
        let parameter = self
            .program
            .state_parameter_by_symbol(state, root)
            .ok_or("the rank entry base is not rooted in its state formals")?;
        let reference = crate::value_custody::places::project_type_reference_from_segments(
            self.program,
            parameter.type_reference,
            &canonical.segments,
        )
        .ok_or("the rank entry base lost its declared projection carrier")?;
        let value = self
            .declaration_term(base)
            .ok_or("the rank entry base has no admitted entry value description")?;
        let value = super::super::super::observed_projections::project_observed_value_in_scope(
            self.program,
            self,
            reference,
            value,
            projection,
        )
        .ok_or("the rank entry component has no authenticated typed projection")?;
        canonical.segments.extend_from_slice(projection);
        self.retain_entry_scalar_observation(machine, state, canonical, value)
    }

    fn retain_entry_scalar_observation(
        &mut self,
        machine: &Machine,
        state: &State,
        canonical: crate::CanonicalPlace,
        value: StructuralTerm,
    ) -> Result<StructuralTerm, &'static str> {
        let root = canonical
            .root
            .as_symbol()
            .ok_or("the rank entry input has no declared storage root")?;
        self.program
            .state_parameter_by_symbol(state, root)
            .ok_or("the rank entry input is not rooted in its state formals")?;
        let backing = self
            .entry_read_backing(machine, state, root, &canonical.segments)
            .ok_or(
                "the rank entry input has an unauthenticated carrier or reference-load requirement",
            )?;
        let coordinate = self
            .capture_entry_storage_place(canonical)
            .ok_or("the rank entry input has no exact formal selector snapshot")?;
        let backing = self.freeze_entry_read_backing(state, backing);
        self.retain_storage_value(coordinate.clone(), value, true);
        let stored = std::sync::Arc::make_mut(&mut self.stored_values)
            .value_at_mut(&coordinate)
            .ok_or("the rank entry input lost its retained storage observation")?;
        stored.backing = Some(backing);
        stored.completed_read = None;
        stored
            .entry
            .clone()
            .ok_or("the rank entry input lost its entry scalar snapshot")
    }
}
