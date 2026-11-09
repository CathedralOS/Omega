/*

    `linear_obligations.rs` establishes claims and checks moves,
    `consumed_owners.rs` rejects uses of an owner after its transfer,
    `permission_events.rs` records the events, `claim_outcomes.rs` settles
    which claim each outcome reaches, `linear_validation.rs` walks the
    statements, and `type_multiplicity.rs` answers what a type carries;
    `owned_selection` transports actual completed sources through guarded joins;
    `returned_claims` queries their exact consuming occurrences. Projected
    ownership and temporary completion retain the same source custody owner.
*/

mod body_input_frontier {
    /*
        Bind body conservation to custody at the value's actual completion.

        The permission producer supplies its current selected frontier. Observing
        entry identity or eventual exit availability is insufficient: every current
        descendant must be live, routed, unchanged and unborrowed at this completion. These rows
        remain evidence inputs, not parent-consumption or child-establishment acts.
    */
    use super::content_snapshots::Observation;
    use super::content_snapshots::preserve_projection;
    use super::content_snapshots::preserve_replacement;
    use super::linear_obligations::LinearPlace;
    use checked_trees::facts::{
        ContentBodyAvailableInput, ContentBodyClaimBinding, ContentBodyConservationFact,
    };
    use language_semantics::{PermissionClaimIdentity, PermissionProvenance};
    use validation::CanonicalPlace;

    pub(crate) fn observe_inputs(
        program: &dyn validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        conservation: &ContentBodyConservationFact,
        places: &[LinearPlace],
    ) -> Result<
        Vec<(
            ContentBodyAvailableInput,
            Vec<facts::PlaceSegment>,
            Vec<ContentBodyClaimBinding>,
        )>,
        &'static str,
    > {
        let machine = program
            .machine_by_symbol(conservation.machine_symbol)
            .ok_or("missing owning machine")?;
        let state = program
            .state_by_symbol(conservation.state_symbol)
            .ok_or("missing owning state")?;
        if !facts
            .flow
            .control
            .operands
            .is_valid(conservation.completed_value)
        {
            return Err("missing completion capture");
        }
        let operand = facts
            .flow
            .control
            .operands
            .get(conservation.completed_value);
        let contexts = facts
            .flow
            .semantic_constraint_contexts(operand.constraints)
            .collect::<Vec<_>>();
        let mut inputs = Vec::new();
        let descendants = facts.qualifications.content.descendant_frontier(
            conservation.inputs,
            conservation.machine_symbol,
            conservation.state_symbol,
            operand.statement_index,
            conservation.evaluation,
            arena::Handle::invalid(),
        )?;
        for (transformation, expected, consumed) in descendants.replacements {
            preserve_replacement(program, facts, transformation, expected, consumed)?;
        }
        for descendant in descendants.bindings {
            let current_binding = facts
                .qualifications
                .content
                .body_claim_bindings
                .get(descendant.binding);
            let place = unique_live_input(places, current_binding.claim_identity)?;
            let current = CanonicalPlace {
                root: place.root,
                segments: place.path.clone(),
            };
            if !crate::flow::place_cases_are_selected(
                program,
                facts.semantic,
                &contexts,
                conservation.machine_symbol,
                conservation.state_symbol,
                operand.statement_index,
                &current,
            ) {
                return Err("input case is not selected at completion");
            }
            if !crate::checks::contracts::exact_scalar_membership(
                program,
                facts,
                &contexts,
                &current,
                current_binding.domain,
                current_binding.semantic_domain,
            ) {
                return Err("input's exact routed membership is not live at completion");
            }
            preserve_projection(
                program,
                facts,
                descendant.binding,
                Observation::Place {
                    at: operand,
                    place: current,
                },
            )?;
            inputs.push((
                ContentBodyAvailableInput {
                    binding: descendant.binding,
                    entry_bindings: Default::default(),
                    root: place.root,
                    segments: Default::default(),
                    provenance: place.provenance.ok_or("missing input provenance")?,
                },
                place.path.clone(),
                descendant
                    .entries
                    .iter()
                    .map(|entry| {
                        facts
                            .qualifications
                            .content
                            .body_claim_bindings
                            .get(*entry)
                            .clone()
                    })
                    .collect(),
            ));
        }
        if inputs.is_empty() {
            return Err("no owned input");
        }
        crate::checks::borrows::require_unborrowed_completion(
            program,
            facts,
            machine,
            state,
            operand,
            conservation.evaluation,
            &inputs
                .iter()
                .map(|(input, path, _)| CanonicalPlace {
                    root: input.root,
                    segments: path.clone(),
                })
                .collect::<Vec<_>>(),
        )?;
        Ok(inputs)
    }

    fn unique_live_input(
        places: &[LinearPlace],
        identity: PermissionClaimIdentity,
    ) -> Result<&LinearPlace, &'static str> {
        if identity == PermissionClaimIdentity::Unknown {
            return Err("unknown entry claim");
        }
        let mut matching = places.iter().filter(|place| {
            place.claim_identity == Some(identity) && place.live && !place.case_excluded
        });
        let place = matching.next().ok_or("entry claim is no longer live")?;
        if matching.next().is_some() {
            return Err("entry claim has more than one live owner");
        }
        if !place.ever_established
            || place.zero_image
            || !place.root.is_valid()
            || matches!(place.provenance, None | Some(PermissionProvenance::Unknown))
        {
            return Err("live input lacks established custody");
        }
        Ok(place)
    }

    pub(crate) fn publish(
        facts: &mut checked_trees::CheckFactsWalk<'_>,
        conservation: arena::Handle<ContentBodyConservationFact>,
        observed: Vec<(
            ContentBodyAvailableInput,
            Vec<facts::PlaceSegment>,
            Vec<ContentBodyClaimBinding>,
        )>,
    ) {
        let inputs = observed
            .into_iter()
            .map(|(mut input, path, entries)| {
                input.entry_bindings = facts
                    .qualifications
                    .content
                    .body_claim_bindings
                    .insert_many(entries);
                input.segments = facts.flow.ownership.segments.insert_many(path);
                input
            })
            .collect::<Vec<_>>();
        let inputs = facts
            .qualifications
            .content
            .body_available_inputs
            .insert_many(inputs);
        facts
            .qualifications
            .content
            .body_conservations
            .get_mut(conservation)
            .available_inputs = inputs;
    }
}
mod content_snapshots {
    /*
        Preserve the projected contents at every edge of a custody derivation.

        Replaying claim replacements proves ancestry, not byte stability. Each
        consumed argument must still denote its predecessor's projection, and each
        final live child must still denote its recorded producer. Following the
        captured store ledger, including every alternative and intervening write,
        composes the callee's conservation theorem without inventing scalar promises
        about how the callee chose the individual child geometries.
    */
    use crate::published::places::place_at;
    use arena::Handle;
    use checked_trees::FlowOperandFact;
    use checked_trees::facts::{
        ContentBodyClaimBinding, ContentClaimTransformation, ContentClaimTransformationSource,
    };
    use facts::PlaceSegment;
    use validation::CanonicalPlace;

    pub(super) enum Observation<'capture> {
        Place {
            at: &'capture FlowOperandFact,
            place: CanonicalPlace,
        },
        Captured {
            value: Handle<FlowOperandFact>,
            path: Vec<PlaceSegment>,
        },
    }

    enum Snapshot {
        Entry(CanonicalPlace),
        Produced {
            state: symbols::SymbolHandle,
            statement: usize,
            expression: typed_trees::expression::ExpressionHandle,
            path: Vec<PlaceSegment>,
        },
    }

    type CaptureProjection = (Handle<FlowOperandFact>, Vec<PlaceSegment>);

    /*
        One expected snapshot owns this scratch: a cached match must never answer
        a question about a different producer or field. Completed matches share
        reconvergent paths; explicit limits refuse before exhausting the host stack
        or spending unbounded work on a source-controlled graph.
    */
    #[derive(Default)]
    struct SnapshotTraversal {
        active: Vec<CaptureProjection>,
        completed: Vec<(CaptureProjection, bool)>,
        visits: usize,
    }

    pub(super) fn preserve_replacement(
        program: &dyn validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        transformation: Handle<ContentClaimTransformation>,
        expected: Handle<ContentBodyClaimBinding>,
        consumed: Handle<ContentBodyClaimBinding>,
    ) -> Result<(), &'static str> {
        let content = &facts.qualifications.content;
        let transformation = content.claim_transformations.get(transformation);
        let binding = content.body_claim_bindings.get(consumed);
        let observation = match transformation.source {
            ContentClaimTransformationSource::Call { .. } => Observation::Captured {
                value: binding.observation,
                path: facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(binding.segments)
                    .to_vec(),
            },
            ContentClaimTransformationSource::Body { conservation } => {
                let conservation = content.body_conservations.get(conservation);
                let available = content
                    .body_available_inputs
                    .span_or_empty(conservation.available_inputs)
                    .iter()
                    .find(|available| {
                        content
                            .body_claim_bindings
                            .get(available.binding)
                            .claim_identity
                            == binding.claim_identity
                    })
                    .ok_or("body replacement has no observed parent")?;
                Observation::Place {
                    at: facts
                        .flow
                        .control
                        .operands
                        .get(conservation.completed_value),
                    place: crate::checks::places::place_or_root_in(
                        &facts.flow.ownership.segments,
                        available.root,
                        available.segments,
                    ),
                }
            }
            ContentClaimTransformationSource::Unknown => {
                return Err("replacement has no completed producer");
            }
        };
        preserve_projection(program, facts, expected, observation)
    }

    pub(super) fn preserve_projection(
        program: &dyn validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        expected: Handle<ContentBodyClaimBinding>,
        observed: Observation<'_>,
    ) -> Result<(), &'static str> {
        let content = &facts.qualifications.content;
        let binding = content.body_claim_bindings.get(expected);
        let projection = content
            .plans
            .iter()
            .find(|projection| {
                projection.domain == binding.domain
                    && projection.semantic_domain == binding.semantic_domain
                    && projection.machine == binding.projection_machine
            })
            .ok_or("missing exact owner projection")?;
        let observations = crate::checks::content::projection_observations(&projection.expression)
            .ok_or("unresolved projection dependencies")?;
        let path = facts
            .flow
            .ownership
            .segments
            .span_or_empty(binding.segments);
        let mut producers = content
            .claim_transformations
            .iter()
            .filter(|(_, transformation)| {
                transformation
                    .outputs
                    .handles()
                    .any(|output| output == expected)
            });
        let producer = producers.next().map(|(_, producer)| producer);
        if producers.next().is_some() {
            return Err("claim snapshot has more than one producer");
        }
        for field in observations {
            let mut expected_path = path.to_vec();
            expected_path.extend_from_slice(&field);
            let snapshot = if let Some(producer) = producer {
                Snapshot::Produced {
                    state: producer.state_symbol,
                    statement: producer.statement_index,
                    expression: producer.expression,
                    path: expected_path,
                }
            } else if binding.parameter_symbol.is_valid() {
                Snapshot::Entry(CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(binding.parameter_symbol),
                    segments: expected_path,
                })
            } else {
                return Err("claim snapshot has neither an entry nor a completed producer");
            };
            let mut traversal = SnapshotTraversal::default();
            let preserved = match &observed {
                Observation::Place { at, place } => {
                    let mut selected = place.clone();
                    selected.segments.extend_from_slice(&field);
                    if let Snapshot::Entry(entry) = &snapshot {
                        let machine = program
                            .machine_holding_state(at.state_symbol)
                            .ok_or("missing snapshot machine")?;
                        let state = program
                            .state_by_symbol(at.state_symbol)
                            .ok_or("missing snapshot state")?;
                        validation::completed_value_preserves_entry_projection(
                            program,
                            machine,
                            state,
                            at.statement_index,
                            at.expression,
                            &selected,
                            entry,
                        )
                    } else {
                        place_matches(program, facts, at, &selected, &snapshot, &mut traversal)?
                    }
                }
                Observation::Captured { value, path } => {
                    let mut path = path.clone();
                    path.extend_from_slice(&field);
                    capture_matches(program, facts, *value, &path, &snapshot, &mut traversal)?
                }
            };
            if !preserved {
                return Err(
                    "qualified claim projection changed since its recorded custody snapshot",
                );
            }
        }
        Ok(())
    }

    fn place_matches(
        program: &dyn validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        at: &FlowOperandFact,
        place: &CanonicalPlace,
        expected: &Snapshot,
        traversal: &mut SnapshotTraversal,
    ) -> Result<bool, &'static str> {
        let Some(_) = program.state_by_symbol(at.state_symbol) else {
            return Ok(false);
        };
        let Some(machine) = program.machine_holding_state(at.state_symbol) else {
            return Ok(false);
        };
        let Some(_) = facts.flow.control.state(machine.symbol, at.state_symbol) else {
            return Ok(false);
        };
        match crate::flow::stored_values::at_snapshot(
            program,
            facts.flow.control,
            at.storage_snapshot,
            place,
            crate::flow::stored_values::StoredObservation::Contents,
        ) {
            Some(crate::flow::stored_values::StoredOperand::Value {
                capture,
                projection,
                ..
            }) => capture_matches(program, facts, capture, &projection, expected, traversal),
            Some(stored @ crate::flow::stored_values::StoredOperand::Alternatives(_)) => {
                let Some(captures) = stored.captures() else {
                    return Ok(false);
                };
                for selected in captures {
                    if !capture_matches(
                        program,
                        facts,
                        selected.capture,
                        &selected.projection,
                        expected,
                        traversal,
                    )? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Some(
                crate::flow::stored_values::StoredOperand::Unavailable
                | crate::flow::stored_values::StoredOperand::Incoming(_),
            ) => Ok(false),
            None => Ok(matches!(expected, Snapshot::Entry(entry) if entry == place)),
        }
    }

    fn capture_matches(
        program: &dyn validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        value: Handle<FlowOperandFact>,
        path: &[PlaceSegment],
        expected: &Snapshot,
        traversal: &mut SnapshotTraversal,
    ) -> Result<bool, &'static str> {
        if traversal.visits >= 65_536 {
            return Err("content snapshot work budget exhausted");
        }
        traversal.visits += 1;
        if !facts.flow.control.operands.is_valid(value) {
            return Ok(false);
        }
        if let Some((_, preserved)) = traversal
            .completed
            .iter()
            .find(|((previous, projection), _)| *previous == value && projection == path)
        {
            return Ok(*preserved);
        }
        if traversal
            .active
            .iter()
            .any(|(previous, projection)| *previous == value && projection == path)
        {
            return Err("content snapshot contains a capture cycle");
        }
        if traversal.active.len() >= 128 {
            return Err("content snapshot depth budget exhausted");
        }
        let capture = facts.flow.control.operands.get(value);
        if matches!(expected, Snapshot::Produced { state, statement, expression, path: expected_path }
            if capture.state_symbol == *state && capture.statement_index == *statement
                && capture.expression == *expression && path == expected_path)
        {
            return Ok(true);
        }
        let key = (value, path.to_vec());
        traversal.active.push(key.clone());
        let preserved = (|| {
            if let Some(predecessors) =
                crate::flow::completed_values::predecessors(program, facts, value, path)
            {
                if predecessors.is_empty() {
                    return Ok(false);
                }
                for (value, path) in predecessors {
                    if !capture_matches(program, facts, value, &path, expected, traversal)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            } else if let Some(mut place) = place_at(
                program,
                capture.state_symbol,
                capture.statement_index,
                capture.expression,
            ) {
                place.segments.extend_from_slice(path);
                place_matches(program, facts, capture, &place, expected, traversal)
            } else {
                Ok(false)
            }
        })();
        traversal.active.pop();
        if let Ok(result) = preserved {
            traversal.completed.push((key, result));
        }
        preserved
    }
}
mod content_transformations {
    /*
        Commit jointly accounted content derivations at the completed value.

        Each equation already has its completion-time proof and exact unborrowed
        parent frontier. That is not enough to spend it: two equations can each
        spend the same parent, or omit another content qualification on that claim.
        Prepare the entire transaction before changing any live place. Children
        retain their nominal identities and construction provenance; their new
        qualification authority lives in the derivation, not in a replacement ID.
    */
    mod boundary_correspondence {
        /*
            Prepare a checked boundary implementation's raw-result custody relation.

            Public result authorization and a proved content equation do not identify
            backing. Every address in the consumed owned accounts must correspond to
            exactly one address in the raw result in the same completed observation.
            This preparation grants no result qualification and consults no Build row.
        */
        use crate::checks::content::content_path;
        use crate::checks::places::place_or_root_in;
        use arena::Handle;
        use checked_trees::facts::{
            ContentBodyAvailableInput, ContentBodyClaimBinding, ContentBodyConservationFact,
        };
        use language_semantics::PermissionClaimIdentity;
        use language_semantics::content::{
            ContentPlaceRoot, ContentPlaceVersion, ContentStructuralPlace,
        };
        use typed_trees::types::{
            FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
        };
        use validation::CanonicalPlace;

        pub(super) struct PreparedCorrespondence {
            pub requirements: Vec<validation::ContentBoundaryBodyRequirement>,
            pub addresses: Vec<PreparedAddress>,
        }

        pub(super) struct PreparedAddress {
            pub input: Handle<ContentBodyAvailableInput>,
            pub output: Handle<ContentBodyClaimBinding>,
            pub input_segments: Vec<facts::PlaceSegment>,
            pub output_segments: Vec<facts::PlaceSegment>,
        }

        struct InputAddress {
            available: Handle<ContentBodyAvailableInput>,
            identity: PermissionClaimIdentity,
            relative: Vec<facts::PlaceSegment>,
            entry: CanonicalPlace,
            current: CanonicalPlace,
        }

        struct OutputAddress {
            binding: Handle<ContentBodyClaimBinding>,
            identity: PermissionClaimIdentity,
            relative: Vec<facts::PlaceSegment>,
            result: ContentStructuralPlace,
        }

        pub(super) fn prepare(
            program: &dyn validation::CheckingInput,
            facts: checked_trees::CheckFactsRef<'_>,
            equation: &ContentBodyConservationFact,
        ) -> Result<PreparedCorrespondence, &'static str> {
            let content = &facts.qualifications.content;
            let law = content
                .conservation_plans
                .get(equation.plan_index)
                .ok_or("boundary custody law is absent")?;
            let completed_law = content
                .body_conservation_plans
                .get(equation.body_plan_index)
                .ok_or("boundary custody has no exact completed-body equation")?;
            if completed_law.owner != equation.machine_symbol
                || completed_law.callable != equation.state_symbol
            {
                return Err("boundary custody equation belongs to another completion frame");
            }
            // Transporting an equation does not make a terminal helper the declared
            // provider implementation. Its caller's public authorization stays there.
            if law.owner != completed_law.owner {
                return Err("terminal content demands do not transfer boundary authorization");
            }
            let sources = validation::content_conservation_plans(program);
            let body = sources
                .iter()
                .find(|source| &source.plan == law)
                .ok_or("boundary custody law has no exact normalized source")?;
            let mut requirements = validation::boundary_body_content_requirements(program, body)?;
            requirements.retain(|requirement| {
                content
                    .body_claim_bindings
                    .span_or_empty(equation.outputs)
                    .iter()
                    .all(|output| {
                        body.boundary_subjects.iter().any(|subject| {
                            subject.declaring_trait == requirement.declaring_trait
                                && subject.requirement == requirement.requirement
                                && subject.realization_state == requirement.realization_state
                                && !subject.parameter.is_valid()
                                && subject.domain == output.domain
                                && subject.semantic_domain == output.semantic_domain
                                && output.segments.is_empty()
                        })
                    })
            });
            if requirements.is_empty() {
                return Err("boundary custody outputs have no complete exact result authorization");
            }
            let completed = facts.flow.control.operands.get(equation.completed_value);
            let machine = program
                .machine_by_symbol(equation.machine_symbol)
                .ok_or("boundary custody machine is absent")?;
            let state = program
                .state_by_symbol(equation.state_symbol)
                .ok_or("boundary custody state is absent")?;
            let ownership = &facts.flow.ownership;
            let mut inputs: Vec<InputAddress> = Vec::new();
            for handle in equation.available_inputs.handles() {
                let available = content.body_available_inputs.get(handle);
                let binding = content.body_claim_bindings.get(available.binding);
                let [entry] = content
                    .body_claim_bindings
                    .span_or_empty(available.entry_bindings)
                else {
                    return Err(
                        "boundary custody needs exact original-to-current backing correspondence; a joint ancestry roster alone does not establish it",
                    );
                };
                let current =
                    place_or_root_in(&ownership.segments, available.root, available.segments);
                let reference = validation::canonical_place_type_reference(
                    program,
                    state.symbol,
                    completed.statement_index,
                    &current,
                )
                .ok_or("boundary custody input has no exact carrier")?;
                for relative in owned_addresses(program, reference)? {
                    if inputs.iter().any(|previous| {
                        previous.identity == binding.claim_identity && previous.relative == relative
                    }) {
                        continue;
                    }
                    let mut entry_path = ownership.segments.span_or_empty(entry.segments).to_vec();
                    entry_path.extend_from_slice(&relative);
                    let mut current = current.clone();
                    current.segments.extend_from_slice(&relative);
                    inputs.push(InputAddress {
                        available: handle,
                        identity: binding.claim_identity,
                        relative,
                        entry: CanonicalPlace {
                            root: facts::PlaceRoot::Symbol(entry.parameter_symbol),
                            segments: entry_path,
                        },
                        current,
                    });
                }
            }
            let mut outputs: Vec<OutputAddress> = Vec::new();
            for handle in equation.outputs.handles() {
                let binding = content.body_claim_bindings.get(handle);
                let prefix = ownership.segments.span_or_empty(binding.segments);
                let reference = validation::owned_projection_type_reference(
                    program,
                    completed.type_reference,
                    prefix,
                )
                .ok_or("boundary custody result has no exact owned carrier")?;
                for relative in owned_addresses(program, reference)? {
                    if outputs.iter().any(|previous| {
                        previous.identity == binding.claim_identity && previous.relative == relative
                    }) {
                        continue;
                    }
                    let mut path = prefix.to_vec();
                    path.extend_from_slice(&relative);
                    outputs.push(OutputAddress {
                        binding: handle,
                        identity: binding.claim_identity,
                        relative,
                        result: ContentStructuralPlace {
                            root: ContentPlaceRoot::Result,
                            version: ContentPlaceVersion::Current,
                            segments: content_path(program, &path)
                                .ok_or("boundary custody result address path is not finite")?,
                        },
                    });
                }
            }
            if inputs.is_empty() || inputs.len() != outputs.len() {
                return Err(
                    "boundary custody requires the complete same-sized backing address roster",
                );
            }
            let goals = inputs
                .iter()
                .flat_map(|input| {
                    outputs
                        .iter()
                        .map(move |output| validation::ContentBackingAddressGoal {
                            entry: input.entry.clone(),
                            current: input.current.clone(),
                            result: output.result.clone(),
                        })
                })
                .collect::<Vec<_>>();
            let proven = validation::completed_value_proves_backing_addresses(
                program,
                machine,
                state,
                completed.statement_index,
                completed.expression,
                completed.type_reference,
                &goals,
            )?;
            let mut used = vec![false; outputs.len()];
            let mut addresses = Vec::new();
            for (position, input) in inputs.iter().enumerate() {
                let mut matching = proven[position * outputs.len()..(position + 1) * outputs.len()]
                    .iter()
                    .enumerate()
                    .filter_map(|(position, proven)| proven.then_some(position));
                let selected = matching
                    .next()
                    .ok_or("boundary custody does not prove an input's exact backing transport")?;
                if matching.next().is_some() || used[selected] {
                    return Err(
                        "boundary custody backing correspondence is ambiguous or repeats an address",
                    );
                }
                used[selected] = true;
                let output = &outputs[selected];
                addresses.push(PreparedAddress {
                    input: input.available,
                    output: output.binding,
                    input_segments: input.relative.clone(),
                    output_segments: output.relative.clone(),
                });
            }
            Ok(PreparedCorrespondence {
                requirements,
                addresses,
            })
        }

        /*
            Enumerate the address-bearing leaves of finite owned storage. Project from
            the actual root type at each step, retaining scoped generic substitutions.
            A reference shell is a distinct loan, never an owned backing traversal.
        */
        fn owned_addresses(
            program: &dyn validation::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> Result<Vec<Vec<facts::PlaceSegment>>, &'static str> {
            fn visit(
                program: &dyn validation::CheckingInput,
                root: TypeReferenceHandle,
                path: &mut Vec<facts::PlaceSegment>,
                ancestors: &mut Vec<symbols::SymbolHandle>,
                output: &mut Vec<Vec<facts::PlaceSegment>>,
            ) -> Result<(), &'static str> {
                let reference = validation::owned_projection_type_reference(program, root, path)
                    .ok_or("boundary custody backing projection is unresolved")?;
                let carrier = program
                    .type_reference_table
                    .unconstrained_type_reference(reference);
                if validation::reference_shell(program, carrier).is_some() {
                    return Err(
                        "boundary custody cannot reinterpret a borrowed address descriptor as owned backing",
                    );
                }
                if let Some(primitive) = program.primitive_type_reference(carrier) {
                    if primitive == PrimitiveType::Addr {
                        output.push(path.clone());
                    }
                    return Ok(());
                }
                if let Some(data) = program.closed_data_definition(carrier).or_else(|| {
                    validation::field_domains::owned_nominal_data_definition(program, carrier)
                }) {
                    if ancestors.contains(&data.symbol) {
                        return Err("boundary custody backing shape is recursive");
                    }
                    ancestors.push(data.symbol);
                    for field in program
                        .data_member_fields(program.data_members(data))
                        .filter(|field| !field.relevance.is_erased())
                    {
                        let length = path.len();
                        validation::push_field_place_segments(program, path, field.symbol);
                        visit(program, root, path, ancestors, output)?;
                        path.truncate(length);
                    }
                    ancestors.pop();
                    return Ok(());
                }
                match program.type_reference_table.type_reference(carrier) {
                    TypeReferenceNode::Unit => Ok(()),
                    TypeReferenceNode::FixedArray {
                        length: FixedArrayLength::Literal(length),
                        ..
                    } => {
                        for index in 0..*length {
                            path.push(facts::PlaceSegment::FixedIndex { index });
                            visit(program, root, path, ancestors, output)?;
                            path.pop();
                        }
                        Ok(())
                    }
                    _ => Err("boundary custody backing storage has no closed finite shape"),
                }
            }
            let mut output = Vec::new();
            visit(
                program,
                reference,
                &mut Vec::new(),
                &mut Vec::new(),
                &mut output,
            )?;
            Ok(output)
        }
    }

    use crate::checks::places::place_or_root_in;
    use arena::Handle;
    use checked_trees::facts::{
        ContentBodyAvailableInput, ContentBodyClaimBinding, ContentBodyConservationFact,
        ContentBodyCustody, ContentBodyDerivation, ContentBodyTransformation,
        ContentClaimTransformation, ContentClaimTransformationSource,
    };
    use checked_trees::{CheckFacts, FlowPermissionEventFact};
    use language_semantics::{
        PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
    };
    use validation::CanonicalPlace;

    use super::linear_obligations::LinearPlace;

    /*
        One parent may carry several independent qualifications but is consumed
        only once. Its original available-input row retains the exact source place.
    */
    struct Parent {
        identity: PermissionClaimIdentity,
        available: Handle<ContentBodyAvailableInput>,
        place_index: usize,
    }

    struct Transaction {
        completion: Handle<checked_trees::FlowOperandFact>,
        equations: Vec<PreparedDerivation>,
        parents: Vec<Parent>,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        evaluation: checked_trees::FlowPermissionEvaluation,
    }

    struct PreparedDerivation {
        conservation: Handle<ContentBodyConservationFact>,
        lineage_roots: Vec<ContentBodyClaimBinding>,
        custody: PreparedCustody,
    }

    enum PreparedCustody {
        Derived,
        Boundary(boundary_correspondence::PreparedCorrespondence),
    }

    pub(super) fn commit(
        program: &dyn validation::CheckingInput,
        facts: &mut checked_trees::CheckFactsWalk<'_>,
        observed: &[Handle<ContentBodyConservationFact>],
        places: &mut [LinearPlace],
        events: &mut Vec<FlowPermissionEventFact>,
    ) -> Result<(), &'static str> {
        if observed.is_empty() {
            return Ok(());
        }
        let transaction = prepare(program, facts.view(), observed, places)?;
        let completed = facts.flow.control.operands.get(transaction.completion);
        let expression = completed.expression;
        let source = PermissionEventSource::Statement {
            statement_index: completed.statement_index,
        };
        for derived in &transaction.equations {
            let content = &mut facts.qualifications.content;
            let equation = content.body_conservations.get(derived.conservation);
            let outputs = equation.outputs;
            let inputs = content
                .body_available_inputs
                .span_or_empty(equation.available_inputs)
                .iter()
                .map(|available| content.body_claim_bindings.get(available.binding).clone())
                .collect::<Vec<_>>();
            let inputs = content.body_claim_bindings.insert_many(inputs);
            content
                .claim_transformations
                .insert(ContentClaimTransformation {
                    machine_symbol: transaction.machine,
                    state_symbol: transaction.state,
                    statement_index: completed.statement_index,
                    expression,
                    evaluation: transaction.evaluation,
                    source: ContentClaimTransformationSource::Body {
                        conservation: derived.conservation,
                    },
                    inputs,
                    outputs,
                });
        }
        let content = &mut facts.qualifications.content;
        let mut derivations = Vec::with_capacity(transaction.equations.len());
        for prepared in transaction.equations {
            let custody = match prepared.custody {
                PreparedCustody::Derived => ContentBodyCustody::Derived,
                PreparedCustody::Boundary(prepared) => {
                    let requirements = prepared
                        .requirements
                        .into_iter()
                        .map(|requirement| {
                            let conservation_plan_index = content.boundary_requirement_plans.len();
                            content
                                .boundary_requirement_plans
                                .push(requirement.conservation);
                            checked_trees::facts::ContentBoundaryBodyRequirement {
                                declaring_trait: requirement.declaring_trait,
                                requirement: requirement.requirement,
                                realization_state: requirement.realization_state,
                                conservation_plan_index,
                            }
                        })
                        .collect::<Vec<_>>();
                    let requirements = content.boundary_body_requirements.insert_many(requirements);
                    let addresses = prepared
                        .addresses
                        .into_iter()
                        .map(
                            |address| checked_trees::facts::ContentBackingAddressTransport {
                                input: address.input,
                                output: address.output,
                                input_segments: facts
                                    .flow
                                    .ownership
                                    .segments
                                    .insert_many(address.input_segments),
                                output_segments: facts
                                    .flow
                                    .ownership
                                    .segments
                                    .insert_many(address.output_segments),
                            },
                        )
                        .collect::<Vec<_>>();
                    let addresses = content.backing_addresses.insert_many(addresses);
                    ContentBodyCustody::Boundary {
                        requirements,
                        addresses,
                    }
                }
            };
            derivations.push(ContentBodyDerivation {
                conservation: prepared.conservation,
                lineage_roots: content
                    .body_claim_bindings
                    .insert_many(prepared.lineage_roots),
                custody,
            });
        }
        let derivations = content.body_derivations.insert_many(derivations);
        facts
            .qualifications
            .content
            .body_transformations
            .insert(ContentBodyTransformation {
                machine_symbol: transaction.machine,
                state_symbol: transaction.state,
                completed_value: transaction.completion,
                evaluation: transaction.evaluation,
                derivations,
            });
        for parent in transaction.parents {
            let available = facts
                .qualifications
                .content
                .body_available_inputs
                .get(parent.available);
            let place = &mut places[parent.place_index];
            events.push(FlowPermissionEventFact {
                expression,
                evaluation: transaction.evaluation,
                machine_symbol: transaction.machine,
                state_symbol: transaction.state,
                source,
                kind: PermissionEventKind::Consume,
                multiplicity: place.multiplicity,
                access: PermissionAccess::Owned,
                claim_identity: parent.identity,
                provenance: available.provenance,
                root: available.root,
                segments: available.segments,
                obligation_live: place.live
                    && place.multiplicity == language_semantics::Multiplicity::Linear,
            });
            place.live = false;
        }
        Ok(())
    }

    fn prepare(
        program: &dyn validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        observed: &[Handle<ContentBodyConservationFact>],
        places: &[LinearPlace],
    ) -> Result<Transaction, &'static str> {
        let content = &facts.qualifications.content;
        let first = content.body_conservations.get(observed[0]);
        let mut transaction = Transaction {
            completion: first.completed_value,
            equations: Vec::new(),
            parents: Vec::new(),
            machine: first.machine_symbol,
            state: first.state_symbol,
            evaluation: first.evaluation,
        };
        let mut inputs: Vec<&ContentBodyClaimBinding> = Vec::new();
        let mut outputs: Vec<&ContentBodyClaimBinding> = Vec::new();
        for handle in observed {
            let equation = content.body_conservations.get(*handle);
            if equation.completed_value != first.completed_value
                || equation.evaluation != first.evaluation
            {
                return Err("derived transaction has more than one completion");
            }
            let body_plan = content
                .body_conservation_plans
                .get(equation.body_plan_index)
                .ok_or("derived transaction has no exact completed-body equation")?;
            if body_plan.owner != equation.machine_symbol
                || body_plan.callable != equation.state_symbol
            {
                return Err("derived equation does not retain its exact completion frame");
            }
            let available = content
                .body_available_inputs
                .span_or_empty(equation.available_inputs);
            if available.is_empty() {
                return Err("derived transaction lacks every available parent");
            }
            let first_owner = available.first().map(|available| {
                let binding = content.body_claim_bindings.get(available.binding);
                (
                    binding.domain,
                    binding.semantic_domain,
                    binding.projection_machine,
                )
            });
            let ordinary = available
                .iter()
                .map(|available| content.body_claim_bindings.get(available.binding))
                .chain(content.body_claim_bindings.span_or_empty(equation.outputs))
                .all(|binding| {
                    Some((
                        binding.domain,
                        binding.semantic_domain,
                        binding.projection_machine,
                    )) == first_owner
                });
            let custody = if ordinary {
                PreparedCustody::Derived
            } else {
                PreparedCustody::Boundary(boundary_correspondence::prepare(
                    program, facts, equation,
                )?)
            };
            // Independently checked declarations can impose the same completed
            // equation. Keep one exact transaction rather than spending it twice.
            // Boundary replay still retains its own authenticated source law.
            if transaction.equations.iter().any(|derived| {
                let previous = content.body_conservations.get(derived.conservation);
                let same_custody = match (&derived.custody, &custody) {
                    (PreparedCustody::Derived, PreparedCustody::Derived) => true,
                    (PreparedCustody::Boundary(_), PreparedCustody::Boundary(_)) => {
                        previous.plan_index == equation.plan_index
                    }
                    _ => false,
                };
                same_custody && same_completed_equation(facts, previous, equation)
            }) {
                continue;
            }
            let mut roots = Vec::new();
            let mut owner = None;
            for input in available {
                let binding = content.body_claim_bindings.get(input.binding);
                let projection = (
                    binding.domain,
                    binding.semantic_domain,
                    binding.projection_machine,
                );
                if ordinary && owner.is_some_and(|owner| owner != projection) {
                    return Err("one derivation cannot unite distinct qualification owners");
                }
                owner = Some(projection);
                if inputs
                    .iter()
                    .any(|previous| same_qualification(previous, binding))
                {
                    return Err("two equations spend the same parent qualification");
                }
                inputs.push(binding);
            }
            for entry in equation.inputs.handles() {
                for root in lineage_roots(facts, content.body_claim_bindings.get(entry))? {
                    if !roots
                        .iter()
                        .any(|previous| same_qualification(previous, &root))
                    {
                        roots.push(root);
                    }
                }
            }
            for output in content.body_claim_bindings.span_or_empty(equation.outputs) {
                if ordinary
                    && owner
                        != Some((
                            output.domain,
                            output.semantic_domain,
                            output.projection_machine,
                        ))
                {
                    return Err("a conservation equation cannot introduce another qualification");
                }
                if outputs
                    .iter()
                    .any(|previous| same_qualification(previous, output))
                {
                    return Err("two equations establish the same output qualification");
                }
                outputs.push(output);
            }
            for available_handle in equation.available_inputs.handles() {
                let available = content.body_available_inputs.get(available_handle);
                let binding = content.body_claim_bindings.get(available.binding);
                if transaction
                    .parents
                    .iter()
                    .any(|parent| parent.identity == binding.claim_identity)
                {
                    continue;
                }
                let path = facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(available.segments);
                let mut live = places.iter().enumerate().filter(|(_, place)| {
                    place.live
                        && !place.case_excluded
                        && place.root == available.root
                        && place.path == path
                        && place.claim_identity == Some(binding.claim_identity)
                });
                let (place_index, _) = live.next().ok_or("transaction parent is no longer live")?;
                if live.next().is_some() {
                    return Err("transaction parent has duplicate custody");
                }
                if transaction.parents.iter().any(|parent| {
                    let other = &places[parent.place_index];
                    other.root == available.root
                        && (other.path.starts_with(path) || path.starts_with(&other.path))
                }) {
                    return Err("transaction parent ownership regions overlap");
                }
                transaction.parents.push(Parent {
                    identity: binding.claim_identity,
                    available: available_handle,
                    place_index,
                });
            }
            transaction.equations.push(PreparedDerivation {
                conservation: *handle,
                lineage_roots: roots,
                custody,
            });
        }
        if inputs.iter().any(|input| {
            outputs
                .iter()
                .any(|output| input.claim_identity == output.claim_identity)
        }) {
            return Err("identity forwarding belongs to the existing claim transfer");
        }
        let completed =
            crate::flow::CapturedOperand::recorded(facts.flow.control, transaction.completion)
                .ok_or("missing raw output observation")?;
        let contexts = facts
            .flow
            .semantic_constraint_contexts(completed.constraints)
            .collect::<Vec<_>>();
        let state_flow = facts
            .flow
            .control
            .state(transaction.machine, transaction.state)
            .ok_or("missing transformation state flow")?;
        for output in &outputs {
            if !validation::field_domains::domain_requires_provenance(program, output.domain) {
                continue;
            }
            let values = crate::flow::captured_operand_values(
                program,
                facts.flow,
                facts.semantic,
                state_flow,
                completed,
                completed.type_reference,
                facts.flow.ownership.segments.span_or_empty(output.segments),
            )
            .ok_or("missing raw output observation")?;
            for value in values {
                let contexts = crate::flow::operand_contexts(
                    program,
                    facts.flow.contexts,
                    facts.flow.control,
                    facts.semantic,
                    value.operand.row(),
                    value.type_reference,
                    value.operand.constraints,
                );
                let sources = crate::flow::captured_value_places(program, facts.flow, &value)
                    .ok_or("missing raw output source")?;
                if sources.iter().any(|source| {
                    crate::checks::contracts::exact_scalar_membership(
                        program,
                        facts,
                        &contexts,
                        &source.projected,
                        output.domain,
                        output.semantic_domain,
                    )
                }) {
                    return Err("derived output already carries independent routed authority");
                }
            }
        }
        for parent in &transaction.parents {
            let available = content.body_available_inputs.get(parent.available);
            let current = place_or_root_in(
                &facts.flow.ownership.segments,
                available.root,
                available.segments,
            );
            let reference = validation::canonical_place_type_reference(
                program,
                transaction.state,
                completed.statement_index,
                &current,
            )
            .ok_or("missing consumed parent carrier")?;
            require_accounted_subtree(
                program,
                facts,
                &transaction,
                &contexts,
                &current,
                reference,
            )?;
        }
        Ok(transaction)
    }

    /*
        Spending a nominal owner spends its entire owned subtree, even when its
        multiplicity hid separate child claims. Declaration obligations survive
        missing membership facts; current facts additionally retain qualifications
        acquired after entry. Neither scalar totals nor missing case evidence may
        erase a nested account.
    */
    fn require_accounted_subtree(
        program: &dyn validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        transaction: &Transaction,
        contexts: &[facts::FactContextHandle],
        parent: &CanonicalPlace,
        reference: typed_trees::types::TypeReferenceHandle,
    ) -> Result<(), &'static str> {
        let completed = facts.flow.control.operands.get(transaction.completion);
        let inactive = |subject: &CanonicalPlace| {
            crate::flow::place_case_path_is_inactive(
                program,
                facts.semantic,
                contexts,
                transaction.machine,
                transaction.state,
                completed.statement_index,
                subject,
            )
        };
        let subject_at = |path: &[facts::PlaceSegment]| {
            let mut subject = parent.clone();
            subject.segments.extend_from_slice(path);
            subject
        };
        let census =
            validation::field_domains::declared_owned_field_obligations(program, reference);
        for path in census.unresolved {
            if !inactive(&subject_at(&path)) {
                return Err("consumed parent has unresolved owned storage");
            }
        }
        let content = &facts.qualifications.content;
        for path in census.custody {
            let subject = subject_at(&path);
            if inactive(&subject) {
                continue;
            }
            let accounted = transaction.equations.iter().any(|derivation| {
                let equation = content.body_conservations.get(derivation.conservation);
                content
                    .body_available_inputs
                    .span_or_empty(equation.available_inputs)
                    .iter()
                    .any(|available| {
                        available.root == subject.root
                            && facts
                                .flow
                                .ownership
                                .segments
                                .span_or_empty(available.segments)
                                == subject.segments
                    })
            });
            if !accounted {
                return Err("consumed parent has an unaccounted nested custody obligation");
            }
        }
        let mut obligations =
            validation::field_domains::domain_constraint_identities(program, reference)
                .into_iter()
                .map(|(domain, instance)| (parent.clone(), domain, instance))
                .collect::<Vec<_>>();
        obligations.extend(census.domains.into_iter().map(|row| {
            let (path, domain, instance) = row.identity;
            (subject_at(&path), domain, instance)
        }));
        for context in contexts {
            for fact in facts
                .semantic
                .context_view(facts.semantic.contexts.get(*context))
                .facts()
            {
                let Some((_, _, domain, instance)) = fact.payload.domain_membership() else {
                    continue;
                };
                if !content.plans.iter().any(|projection| {
                    projection.domain == domain && projection.semantic_domain == instance
                }) {
                    continue;
                }
                let Some(place) = fact.place.as_place() else {
                    continue;
                };
                let Some(subject) = crate::flow::canonical_place_from_semantic_place(
                    program,
                    facts.semantic,
                    facts.semantic.places.get(place),
                ) else {
                    continue;
                };
                if subject.root != parent.root || !subject.segments.starts_with(&parent.segments) {
                    continue;
                }
                if inactive(&subject) {
                    continue;
                }
                let path = &subject.segments[parent.segments.len()..];
                let mut borrowed = false;
                for length in 0..=path.len() {
                    let projected = validation::project_type_reference_from_segments(
                        program,
                        reference,
                        &path[..length],
                    )
                    .ok_or("content qualification has unresolved owned projection")?;
                    if validation::reference_shell(program, projected).is_some() {
                        borrowed = true;
                        break;
                    }
                }
                if !borrowed && !obligations.contains(&(subject.clone(), domain, instance)) {
                    obligations.push((subject, domain, instance));
                }
            }
        }
        for (subject, domain, instance) in obligations {
            if inactive(&subject) {
                continue;
            }
            for projection in content.plans.iter().filter(|projection| {
                projection.domain == domain && projection.semantic_domain == instance
            }) {
                let accounted = transaction.equations.iter().any(|derivation| {
                    let equation = content.body_conservations.get(derivation.conservation);
                    content
                        .body_available_inputs
                        .span_or_empty(equation.available_inputs)
                        .iter()
                        .any(|available| {
                            let binding = content.body_claim_bindings.get(available.binding);
                            available.root == subject.root
                                && facts
                                    .flow
                                    .ownership
                                    .segments
                                    .span_or_empty(available.segments)
                                    == subject.segments
                                && binding.domain == domain
                                && binding.semantic_domain == instance
                                && binding.projection_machine == projection.machine
                        })
                });
                if !accounted {
                    return Err("consumed parent has an unaccounted content qualification");
                }
            }
        }
        Ok(())
    }

    /*
        Storage consumers follow the committed operation, not a fabricated source
        move whose expression would incorrectly name the result as its parent.
    */
    pub(super) fn consumed_places(
        facts: &CheckFacts,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        statement: usize,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Vec<CanonicalPlace> {
        let content = &facts.qualifications.content;
        let mut places = Vec::new();
        for (_, transaction) in content.body_transformations.iter() {
            let completed = facts.flow.control.operands.get(transaction.completed_value);
            if transaction.machine_symbol != machine
                || transaction.state_symbol != state
                || completed.statement_index != statement
                || completed.expression != expression
            {
                continue;
            }
            for derivation in content
                .body_derivations
                .span_or_empty(transaction.derivations)
            {
                let equation = content.body_conservations.get(derivation.conservation);
                for input in content
                    .body_available_inputs
                    .span_or_empty(equation.available_inputs)
                {
                    let place = place_or_root_in(
                        &facts.flow.ownership.segments,
                        input.root,
                        input.segments,
                    );
                    crate::shared::collections::push_unique(&mut places, place);
                }
            }
        }
        places
    }

    fn same_completed_equation(
        facts: checked_trees::CheckFactsRef<'_>,
        left: &ContentBodyConservationFact,
        right: &ContentBodyConservationFact,
    ) -> bool {
        let content = &facts.qualifications.content;
        let paths = &facts.flow.ownership.segments;
        let same_binding = |left: &ContentBodyClaimBinding, right: &ContentBodyClaimBinding| {
            same_qualification(left, right)
                && left.parameter_symbol == right.parameter_symbol
                && left.observation == right.observation
                && paths.span_or_empty(left.segments) == paths.span_or_empty(right.segments)
        };
        let same_bindings = |left, right| {
            let left = content.body_claim_bindings.span_or_empty(left);
            let right = content.body_claim_bindings.span_or_empty(right);
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| same_binding(left, right))
        };
        let left_available = content
            .body_available_inputs
            .span_or_empty(left.available_inputs);
        let right_available = content
            .body_available_inputs
            .span_or_empty(right.available_inputs);
        left.machine_symbol == right.machine_symbol
            && left.state_symbol == right.state_symbol
            && left.returned_at == right.returned_at
            && left.completed_value == right.completed_value
            && left.evaluation == right.evaluation
            && content.body_conservation_plans.get(left.body_plan_index)
                == content.body_conservation_plans.get(right.body_plan_index)
            && same_bindings(left.inputs, right.inputs)
            && same_bindings(left.outputs, right.outputs)
            && left_available.len() == right_available.len()
            && left_available
                .iter()
                .zip(right_available)
                .all(|(left, right)| {
                    left.root == right.root
                        && left.provenance == right.provenance
                        && paths.span_or_empty(left.segments) == paths.span_or_empty(right.segments)
                        && same_binding(
                            content.body_claim_bindings.get(left.binding),
                            content.body_claim_bindings.get(right.binding),
                        )
                        && same_bindings(left.entry_bindings, right.entry_bindings)
                })
    }

    fn same_qualification(left: &ContentBodyClaimBinding, right: &ContentBodyClaimBinding) -> bool {
        left.claim_identity == right.claim_identity
            && left.domain == right.domain
            && left.semantic_domain == right.semantic_domain
            && left.projection_machine == right.projection_machine
    }

    fn lineage_roots(
        facts: checked_trees::CheckFactsRef<'_>,
        input: &ContentBodyClaimBinding,
    ) -> Result<Vec<ContentBodyClaimBinding>, &'static str> {
        let content = &facts.qualifications.content;
        let mut roots: Option<Vec<ContentBodyClaimBinding>> = None;
        for (_, derivation) in content.body_derivations.iter() {
            if !matches!(derivation.custody, ContentBodyCustody::Derived) {
                continue;
            }
            let equation = content.body_conservations.get(derivation.conservation);
            if content
                .body_claim_bindings
                .span_or_empty(equation.outputs)
                .iter()
                .any(|output| same_qualification(input, output))
            {
                let candidate = content
                    .body_claim_bindings
                    .span_or_empty(derivation.lineage_roots);
                if candidate.is_empty()
                    || roots.as_ref().is_some_and(|previous| {
                        previous.len() != candidate.len()
                            || previous.iter().any(|root| {
                                !candidate
                                    .iter()
                                    .any(|other| same_qualification(root, other))
                            })
                    })
                {
                    return Err("ambiguous qualified root roster");
                }
                roots = Some(candidate.to_vec());
            }
        }
        Ok(roots.unwrap_or_else(|| vec![input.clone()]))
    }
}
pub(crate) use body_input_frontier::{
    observe_inputs as observe_body_inputs, publish as publish_body_inputs,
};
#[path = "checks_multiplicity_borrowed_windows.rs"]
mod borrowed_windows;
#[path = "checks_multiplicity_claim_outcomes.rs"]
mod claim_outcomes;
pub(crate) use claim_outcomes::claim_paths_are_case_alternatives;
#[path = "checks_multiplicity_cleanup_dispositions.rs"]
mod cleanup_dispositions;
mod construction_frontier {
    /*
        Values established in reserved storage, in establishment order.

        Reserving gated storage is not whole-value establishment. Keep only the
        established subtrees; finishing an aggregate folds those children into one
        value, after which the ordinary recursive declaration-order cleanup applies.
        Arrays remain sparse until complete: untouched gated elements are never
        enumerated. This is source permission-production scratch, forked at edges,
        not a runtime liveness map or a second ownership replay.
    */
    use facts::PlaceSegment;
    use language_semantics::{DataSupplyMode, Multiplicity};
    use typed_trees::data::DataMember;
    use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};
    use validation::peel_declared_constraints;
    use validation::project_type_reference_from_segments;
    use validation::reference_result_custody::result_multiplicity;
    use validation::reference_shell;
    use validation::type_requires_establishment;

    /*
        The first case mismatch in a zero value's exact closed structural path.
        Both claim production and residual cleanup use this absence: a nonselected
        payload is not a moved child and cannot suppress its owner's nominal hook.
        Unresolved projections prove no absence, and an active linear payload is
        never established by the zero representation.
    */
    pub(super) fn zero_image_inactive_case<'path>(
        program: &dyn ::validation::CheckingInput,
        reference: TypeReferenceHandle,
        path: &'path [PlaceSegment],
    ) -> Option<&'path [PlaceSegment]> {
        for (segment_index, segment) in path.iter().enumerate() {
            let PlaceSegment::Case { variant } = segment else {
                continue;
            };
            let projected =
                project_type_reference_from_segments(program, reference, &path[..segment_index])?;
            let definition =
                program.closed_data_definition(peel_declared_constraints(program, projected))?;
            let zero_case = program
                .data_members(definition)
                .iter()
                .find_map(|member| member.as_variant())?;
            if zero_case.symbol != *variant {
                return Some(&path[..=segment_index]);
            }
        }
        None
    }

    #[derive(Clone, PartialEq, Eq)]
    pub(super) struct ConstructionFrontier {
        reference: TypeReferenceHandle,
        pub(super) established: Vec<Vec<PlaceSegment>>,
    }

    impl ConstructionFrontier {
        /*
            A completed ingress or whole receiving write establishes its actual
            value, independently of whether it carries a claim at the root.
        */
        pub(super) fn completed(
            program: &dyn validation::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> Self {
            let mut frontier = Self::new(program, reference);
            frontier.establish(program, &[]);
            frontier
        }

        pub(super) fn new(
            program: &dyn ::validation::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> Self {
            let mut frontier = Self {
                reference,
                established: Vec::new(),
            };
            frontier.seed_zero_values(program, reference, &mut Vec::new());
            frontier
        }

        fn seed_zero_values(
            &mut self,
            program: &dyn ::validation::CheckingInput,
            reference: TypeReferenceHandle,
            path: &mut Vec<PlaceSegment>,
        ) {
            if nominal_linear_owner(program, reference) {
                return;
            }
            if !requires_owned_establishment(program, reference) {
                self.established.push(path.clone());
                return;
            }
            for child_path in record_children(program, reference).unwrap_or_default() {
                let parent_length = path.len();
                path.extend(child_path);
                if let Some(child) =
                    project_type_reference_from_segments(program, self.reference, path)
                {
                    self.seed_zero_values(program, child, path);
                }
                path.truncate(parent_length);
            }
            if !type_requires_establishment(program, reference)
                && (record_children(program, reference).is_some_and(|children| children.is_empty())
                    || self.complete(program, reference, path))
            {
                self.replace_subtrees(path);
            }
        }

        pub(super) fn cleanup_free(
            &self,
            program: &dyn ::validation::CheckingInput,
            path: &[PlaceSegment],
        ) -> bool {
            project_type_reference_from_segments(program, self.reference, path).is_some_and(
                |reference| result_multiplicity(program, reference) == Multiplicity::Unrestricted,
            )
        }

        pub(super) fn contains(&self, path: &[PlaceSegment]) -> bool {
            self.established
                .iter()
                .any(|established| path.starts_with(established))
        }

        pub(super) fn retire(&mut self, path: &[PlaceSegment]) {
            self.established
                .retain(|established| !established.starts_with(path));
        }

        pub(super) fn establish(
            &mut self,
            program: &dyn ::validation::CheckingInput,
            path: &[PlaceSegment],
        ) {
            if self
                .established
                .iter()
                .any(|established| established.len() < path.len() && path.starts_with(established))
            {
                return;
            }
            self.replace_subtrees(path);
            for length in (0..path.len()).rev() {
                let prefix = &path[..length];
                let Some(reference) =
                    project_type_reference_from_segments(program, self.reference, prefix)
                else {
                    break;
                };
                if !self.complete(program, reference, prefix) {
                    break;
                }
                self.replace_subtrees(prefix);
            }
        }

        /*
            Join already established values without replaying their zero-filled
            children as validated writes. Only complete direct children can finish
            the containing value; a gated child's raw fields do not establish it.
        */
        pub(super) fn completes_with(
            &self,
            program: &dyn ::validation::CheckingInput,
            later: &[Vec<PlaceSegment>],
            receiving_writes: &[Vec<PlaceSegment>],
        ) -> bool {
            let mut combined = self.clone();
            for path in later {
                if !combined.contains(path) {
                    combined.replace_subtrees(path);
                }
            }
            for path in receiving_writes {
                combined.establish(program, path);
            }
            combined.contains(&[])
        }

        fn replace_subtrees(&mut self, path: &[PlaceSegment]) {
            self.established.retain(|prior| !prior.starts_with(path));
            self.established.push(path.to_vec());
        }

        fn complete(
            &self,
            program: &dyn ::validation::CheckingInput,
            reference: TypeReferenceHandle,
            path: &[PlaceSegment],
        ) -> bool {
            if nominal_linear_owner(program, reference) {
                return false;
            }
            let Some(reference) = program.unwrapped_type_reference(reference) else {
                return false;
            };
            if let TypeReferenceNode::FixedArray {
                length: FixedArrayLength::Literal(length),
                ..
            } = program.type_reference_table.type_reference(reference)
            {
                return self.established.iter().filter(|child| {
                    child.len() == path.len() + 1
                        && child.starts_with(path)
                        && matches!(child.last(), Some(PlaceSegment::FixedIndex { index }) if *index < *length)
                }).count() == *length;
            }
            record_children(program, reference).is_some_and(|children| {
                !children.is_empty()
                    && children.into_iter().all(|segments| {
                        let mut child = path.to_vec();
                        child.extend(segments);
                        self.contains(&child)
                    })
            })
        }
    }

    /*
        Arrays inherit element multiplicity but own only element claims. Only
        a nominal type (including a bounded type parameter) can own a root claim.
    */
    fn nominal_linear_owner(
        program: &dyn ::validation::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> bool {
        program
            .unwrapped_type_reference(reference)
            .is_some_and(|reference| {
                matches!(
                    program.type_reference_table.type_reference(reference),
                    TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. }
                ) && program.type_multiplicity(reference) == Multiplicity::Linear
            })
    }

    pub(super) fn transparent_storage(
        program: &dyn ::validation::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> bool {
        program.type_multiplicity(reference) == Multiplicity::Affine
            || program
                .unwrapped_type_reference(reference)
                .is_some_and(|reference| {
                    matches!(
                        program.type_reference_table.type_reference(reference),
                        TypeReferenceNode::FixedArray { .. }
                    )
                })
            || (program.type_multiplicity(reference) == Multiplicity::Unrestricted
                && reference_shell(program, reference).is_none()
                && program.primitive_type_reference(reference).is_none()
                && program
                    .closed_data_definition(reference)
                    .is_some_and(|data| {
                        data.supply_mode == DataSupplyMode::CheckedShape && data.quotient.is_none()
                    }))
    }

    /*
        Default-domain validity and owned establishment are separate judgments.
        Transparent storage containing claims is reserved even if its zero bits
        satisfy every value-domain predicate.
    */
    pub(super) fn requires_owned_establishment(
        program: &dyn ::validation::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> bool {
        type_requires_establishment(program, reference)
            || super::type_carries_linear_obligation(program, reference)
    }

    fn record_children(
        program: &dyn ::validation::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Option<Vec<Vec<PlaceSegment>>> {
        let reference = program.unwrapped_type_reference(reference)?;
        let definition = program.closed_data_definition(reference)?;
        let mut children = Vec::new();
        for member in program.data_members(definition) {
            if let Some(field) = member.as_field() {
                children.push(vec![PlaceSegment::Field {
                    symbol: field.symbol,
                }]);
            }
        }
        if let Some(variant) =
            program
                .data_members(definition)
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(variant) => Some(variant),
                    DataMember::Field(_) => None,
                })
        {
            let case = PlaceSegment::Case {
                variant: variant.symbol,
            };
            for field in program.data_payload_fields(variant) {
                children.push(vec![
                    case,
                    PlaceSegment::Field {
                        symbol: field.symbol,
                    },
                ]);
            }
        }
        Some(children)
    }
}
mod consumed_owners {
    /*
        Owned places a state body has already transferred or consumed.

        A by-value move out of a local, an owned parameter, or an owned `self`
        receiver hands the place's custody to its destination; the place then
        holds no established value until a whole store re-establishes it
        (wiki/spec/language/ownership.md, "Paths and control flow": the
        permission context tracks exact established places). Linear
        validation rejects a second transfer of a tracked claim, but a read, a
        borrow, a method receiver or a projected move produces no permission event
        of its own, so `drop(g); g.handle` and `consume(g); g.handle` used to
        check. Early disposal is an ordinary consuming call, so neither the callee
        name nor a cleanup hook decides anything here.

        The tracker rides the occurrence-ordered replay that borrowed-storage
        windows already use (`ownership_occurrences::statement_steps`): each evaluated
        place is observed before its own move applies, so `take(g.handle, g)`
        reads before it moves while `take(g); g.handle` reads a consumed owner.
        Its limits keep it from inventing moves the ownership lane did not record:

        - Only owned custody chains are tracked. A move that crosses a reference
          (a `&mut` receiver, parameter, local or field) opens a borrowed-storage
          window instead, and a move of a reference value itself transfers a loan
          carrier, not storage; both keep their existing rules.
        - A move on a transition arm leaves the state when that arm is taken; a
          guard miss falls through to the next arm still holding the value, so
          those moves retire when the arm's statement ends.
        - A conditional move — inside one match arm or a short-circuit operand —
          leaves the owner consumed on some paths only. The move applies immediately
          on compatible evaluation routes; mutually exclusive sibling arms retain
          the value. After the join, every later use sees a possibly consumed owner.
          A move no evaluated occurrence claimed (an operand of an unreachable arm)
          never applies.
        - A use at, below or above a consumed place observes absent storage. Partial
          moves preserve disjoint siblings, not a whole ancestor value to pass to
          another machine. Selected structural cleanup uses its exact residual
          frontier without evaluating that ancestor as an authored value.
        - A store to the consumed place or to an enclosing owner re-establishes
          it. A store into a strict projection of a consumed place would write
          through storage that holds no value and rejects.
        - Logical assembly facts and parser-synthesized destructure markers are
          proof reads, not operational uses, and are not checked.

        The diagnostics are reported only after linear validation succeeds (see
        `check_linear_obligations`): a repeated whole transfer is also a use of a
        consumed owner, and its established "already transferred or consumed"
        diagnostic stays the one reported for that shape.
    */
    use validation::CanonicalPlace;

    use super::borrowed_windows::place_spelling;
    use super::linear_obligations::type_reference_is_reference;
    use super::owned_selection::place_paths_overlap;
    use crate::checks::ownership_occurrences::{EvaluationBranch, routes_compatible};
    use crate::published::places::place_at;

    use diagnostics::Diagnostic;
    use typed_trees::statement::StatementNode;

    /*
        The occurrence replay and permission producer reject the same consumed
        storage use. A producer with only exact path provenance omits the move's
        statement number rather than inventing one or replacing the cause with an
        unrelated missing-establishment diagnostic.
    */
    pub(super) fn consumed_use_message(
        machine_name: &str,
        state_name: &str,
        statement_index: usize,
        spelling: &str,
        consumed_spelling: &str,
        moved_statement: Option<usize>,
    ) -> String {
        let moved_at = moved_statement
            .map(|index| format!(" at statement {index}"))
            .unwrap_or_default();
        format!(
            "cannot use `{spelling}` in machine `{machine_name}` state `{state_name}` \
         (statement {statement_index}) after `{consumed_spelling}` was transferred or \
         consumed{moved_at}: it holds no value; read what is needed before the transfer \
         or assign a whole value to `{consumed_spelling}` first"
        )
    }

    /* One owned place whose custody an earlier occurrence transferred. */
    struct ConsumedPlace {
        root: facts::PlaceRoot,
        path: Vec<facts::PlaceSegment>,
        spelling: String,
        moved_statement: usize,
        route: Vec<EvaluationBranch>,
    }

    #[derive(Default)]
    pub(super) struct ConsumedOwners {
        consumed: Vec<ConsumedPlace>,
        /*
            `consumed` rows that predate the current statement. A transition
            statement truncates back to this length when it ends.
        */
        statement_start: usize,
    }

    impl ConsumedOwners {
        pub(super) fn begin_statement(&mut self) {
            self.statement_start = self.consumed.len();
        }

        /*
            Apply one ordered move. Moves that do not transfer owned storage of
            this activation are ignored rather than guessed at.
        */
        #[allow(clippy::too_many_arguments)]
        pub(super) fn record_move(
            &mut self,
            program: &dyn ::validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            statement_index: usize,
            event: &crate::flow::DiscoveredMoveEvent,
            path: &[facts::PlaceSegment],
            route: &[EvaluationBranch],
        ) {
            self.record_owned_place(
                program,
                machine,
                state,
                statement_index,
                &CanonicalPlace {
                    root: event.root,
                    segments: path.to_vec(),
                },
                route,
            );
        }

        /*
            Explicit moves and implicit content transformations retire the same
            exact owned storage. An implicit operation supplies its checked parent
            place directly, never an invented expression for the source.
        */
        pub(super) fn record_owned_place(
            &mut self,
            program: &dyn validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            statement_index: usize,
            source: &CanonicalPlace,
            route: &[EvaluationBranch],
        ) {
            let place = storage_place(program, machine, state, source);
            if !owned_custody_chain(program, machine, state, statement_index, &place) {
                return;
            }
            let consumed = ConsumedPlace {
                spelling: place_spelling(program, place.root, &place.segments),
                root: place.root,
                path: place.segments,
                moved_statement: statement_index,
                route: route.to_vec(),
            };
            self.consumed.push(consumed);
        }

        /*
            A read, borrow, receiver or move overlapping a consumed place touches
            storage that holds no complete established value.
        */
        #[allow(clippy::too_many_arguments)]
        pub(super) fn check_use(
            &self,
            program: &dyn ::validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            statement: &StatementNode,
            statement_index: usize,
            use_place: &CanonicalPlace,
            route: &[EvaluationBranch],
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            if self.consumed.is_empty() || !is_operational(statement) {
                return;
            }
            let place = storage_place(program, machine, state, use_place);
            let Some(consumed) = self.consumed_through(&place, route) else {
                return;
            };
            let spelling = place_spelling(program, place.root, &place.segments);
            push_once(
                diagnostics,
                consumed_use_message(
                    machine.name.as_str(),
                    state.name.as_str(),
                    statement_index,
                    &spelling,
                    &consumed.spelling,
                    Some(consumed.moved_statement),
                ),
            );
        }

        /*
            A whole store at or above a consumed place re-establishes it; a store
            strictly inside one writes through an owner that holds no value.
        */
        #[allow(clippy::too_many_arguments)]
        pub(super) fn check_store(
            &mut self,
            program: &dyn ::validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            statement_index: usize,
            assignment: &typed_trees::statement::TableAssignment,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            if self.consumed.is_empty() {
                return;
            }
            let Some(target) = place_at(program, state.symbol, statement_index, assignment.target)
            else {
                return;
            };
            let target = storage_place(program, machine, state, &target);
            if let Some(consumed) = self.consumed.iter().find(|consumed| {
                consumed.root == target.root
                    && target.segments.len() > consumed.path.len()
                    && place_paths_overlap(&target.segments, &consumed.path)
            }) {
                let spelling = place_spelling(program, target.root, &target.segments);
                push_once(
                    diagnostics,
                    format!(
                        "cannot store into `{spelling}` in machine `{}` state `{}` (statement \
                     {statement_index}) after `{}` was transferred or consumed at statement \
                     {}: assign a whole value to `{}` to re-establish it",
                        machine.name.as_str(),
                        state.name.as_str(),
                        consumed.spelling,
                        consumed.moved_statement,
                        consumed.spelling,
                    ),
                );
                return;
            }
            self.consumed.retain(|consumed| {
                consumed.root != target.root || !consumed.path.starts_with(&target.segments)
            });
        }

        /*
            Close the current statement. A taken transition leaves the state, so
            its moves never reach the fall-through; every other statement's
            conditional moves now apply to later statements.
        */
        pub(super) fn finish_statement(&mut self, leaves_on_transition: bool) {
            if leaves_on_transition {
                self.consumed.truncate(self.statement_start);
            } else {
                for consumed in &mut self.consumed {
                    consumed.route.clear();
                }
            }
        }

        /*
            An ancestor observation also needs its missing subtree. Address-only
            traversal is not an observation: the occurrence owner already retains
            only each operation's maximal selected place, so disjoint siblings do
            not spuriously observe their common parent.
        */
        fn consumed_through(
            &self,
            place: &CanonicalPlace,
            route: &[EvaluationBranch],
        ) -> Option<&ConsumedPlace> {
            self.consumed.iter().find(|consumed| {
                routes_compatible(&consumed.route, route)
                    && consumed.root == place.root
                    && place_paths_overlap(&place.segments, &consumed.path)
            })
        }
    }

    /* Repeated occurrences of one use in a statement report once. */
    fn push_once(diagnostics: &mut Vec<Diagnostic>, message: String) {
        if !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == message)
        {
            diagnostics.push(Diagnostic::error(message));
        }
    }

    /*
        Assembly facts are logical and destructure markers are parser-synthesized
        proof reads; neither evaluates the owner at run time.
    */
    fn is_operational(statement: &StatementNode) -> bool {
        match statement {
            StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => false,
            StatementNode::LocalData(local) => {
                local.role != language_core::LocalBindingRole::ArmPattern
            }
            _ => true,
        }
    }

    /*
        Key a place by its storage identity: `self` spellings re-root at the
        machine exactly as move events do, and a bare attached-field root joins
        its receiver storage.
    */
    fn storage_place(
        _program: &dyn ::validation::CheckingInput,
        _machine: &typed_trees::machine::Machine,
        _state: &typed_trees::state::State,
        place: &CanonicalPlace,
    ) -> CanonicalPlace {
        CanonicalPlace {
            root: (place.root),
            segments: place.segments.clone(),
        }
    }

    /*
        Whether `place` is owned storage of this activation reached without
        crossing a reference: a local declared in this state, a by-value
        parameter, and every prefix through the
        place itself has a non-reference type.
    */
    fn owned_custody_chain(
        program: &dyn ::validation::CheckingInput,
        _machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        place: &CanonicalPlace,
    ) -> bool {
        let Some(root) = place.root.as_symbol() else {
            return false;
        };
        let parameters = program.state_parameters(state);
        let owned_root = program.state_local_declaration(state, root).is_some()
            || parameters.iter().any(|parameter| parameter.symbol == root);
        owned_root
            && (0..=place.segments.len()).all(|length| {
                validation::canonical_place_type_reference(
                    program,
                    state.symbol,
                    statement_index,
                    &CanonicalPlace {
                        root: place.root,
                        segments: place.segments[..length].to_vec(),
                    },
                )
                .is_some_and(|type_reference| !type_reference_is_reference(program, type_reference))
            })
    }
}
mod linear_obligations {
    /*
        Linear obligations: the places and claims a linear value establishes,
        the claim outcome maps, and the checks that every partial move and
        nominal drop respects them.
    */
    use super::borrowed_windows;
    use super::consumed_owners::ConsumedOwners;
    use crate::checks::multiplicity::linear_validation::{
        event_statement_index, validate_linear_permission_events,
    };
    use crate::checks::multiplicity::permission_events::{
        derive_crash_frontier_lower_bounds, record_crash_frontier_lower_bounds,
    };
    use crate::checks::multiplicity::projected_affine;
    use crate::published::places::place_at;
    use checked_trees::CheckFacts;
    use diagnostics::Diagnostic;
    use language_semantics::{
        Multiplicity, PermissionClaimIdentity, PermissionEventSource, PermissionProvenance,
    };
    use symbols::SymbolHandle;
    use typed_trees::statement::StatementNode;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct LinearPlace {
        /*
            Custody is rooted at a real named place or producer expression. An
            anonymous owner does not acquire a fabricated local to carry claims.
        */
        pub(crate) root: facts::PlaceRoot,
        pub(crate) name: String,
        /*
            Canonical claim path below `root`. An empty path is one nominal claim;
            transparent records, active cases, and fixed arrays contribute one
            entry per contained linear claim instead of inventing an aggregate root.
        */
        pub(crate) path: Vec<facts::PlaceSegment>,
        pub(crate) multiplicity: Multiplicity,
        pub(crate) claim_identity: Option<PermissionClaimIdentity>,
        pub(crate) provenance: Option<PermissionProvenance>,
        pub(crate) live: bool,
        /*
            Parameters are established on entry. A local is established only by an
            explicit initializer/assignment; implicit zero-fill creates no debt.
        */
        pub(crate) ever_established: bool,
        /*
            A linear local declared without an initializer holds its zero image:
            affine storage of its type with no obligation (ownership.md,
            "Establishment and erasure"). The image dies when the first assignment
            replaces it, exactly as an affine local's zero does, while the place
            stays unestablished for moves until that assignment.
        */
        pub(crate) zero_image: bool,
        /*
            An affine sum can carry a linear payload only in selected cases. When
            false, `live` is unconditional for every established value; when true,
            `live` follows the active case.
        */
        pub(crate) conditional: bool,
        /*
            Proven absent in the selected case, rather than dead after a move.
            Moving a carrier waives only absent payloads, never consumed claims.
        */
        pub(crate) case_excluded: bool,
    }

    impl LinearPlace {
        pub(crate) fn unconsumed_scope_exit(&self) -> Diagnostic {
            Diagnostic::error(format!(
                "linear value `{}` reaches scope exit without being consumed or transferred",
                self.name
            ))
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct WrittenLinearTarget {
        pub(crate) value: typed_trees::expression::ExpressionHandle,
        pub(crate) root: facts::PlaceRoot,
        pub(crate) destination_path: Vec<facts::PlaceSegment>,
        pub(crate) place_index: usize,
        pub(crate) obligation_live: bool,
        pub(crate) claim_identity: Option<PermissionClaimIdentity>,
        pub(crate) provenance: Option<PermissionProvenance>,
        /*
            A direct source coordinate is bound when its move occurs, after any
            anonymous producer has completed, not while planning the statement.
        */
        pub(crate) source_place: Option<validation::CanonicalPlace>,
    }

    #[derive(Debug, Clone)]
    pub(crate) struct LinearClaimTemplate {
        pub(crate) path: Vec<facts::PlaceSegment>,
        pub(crate) type_reference: TypeReferenceHandle,
        pub(super) multiplicity: Multiplicity,
        pub(super) conditional: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct CheckedClaimOutcomeMap {
        pub(crate) machine_symbol: SymbolHandle,
        pub(crate) state_symbol: SymbolHandle,
        pub(crate) entries: Vec<CheckedClaimOutcomeEntry>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub(crate) struct CheckedClaimOutcomeEntry {
        pub(crate) output_path: Vec<facts::PlaceSegment>,
        pub(crate) source: CheckedClaimOutcomeSource,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub(crate) enum CheckedClaimOutcomeSource {
        Input {
            parameter_symbol: SymbolHandle,
            path: Vec<facts::PlaceSegment>,
        },
        Established {
            claim_identity: PermissionClaimIdentity,
            provenance: PermissionProvenance,
        },
    }

    #[derive(Clone, Debug, Default)]
    pub(crate) struct ClaimIdentityAllocator {
        next_ordinal: u32,
        /*
            One past the highest ordinal handed out so far, kept across
            `begin_state`, so identities minted after the per-state walk stay above
            every walk ordinal.
        */
        high_water: u32,
    }

    /*
        Ordinals reserved per roster position before the next state's base. A state
        that mints more keeps counting upward; its identities still differ from
        every other state's, because each identity also names its machine and state.
    */
    const CLAIM_ORDINALS_PER_STATE: u32 = 1 << 12;

    impl ClaimIdentityAllocator {
        /*
            Mint the walk's identities for the state at roster `position` from a
            base fixed by that position alone. A state's ordinals then never depend
            on how many identities the states before it minted, which is what lets
            states be walked apart from one another and still mint the same ones.
        */
        pub(crate) fn begin_state(&mut self, position: usize) {
            self.high_water = self.high_water.max(self.next_ordinal);
            let window = u32::try_from(position % (1 << 20)).expect("window fits in u32");
            self.next_ordinal = window * CLAIM_ORDINALS_PER_STATE;
        }

        /* Continue after every ordinal this allocator or `other` handed out. */
        pub(crate) fn absorb(&mut self, other: Self) {
            self.high_water = self
                .high_water
                .max(self.next_ordinal)
                .max(other.high_water)
                .max(other.next_ordinal);
            self.next_ordinal = self.high_water;
        }

        /* Mint after the walk: above every ordinal the walk's states used. */
        pub(crate) fn finish_walk(&mut self) {
            self.high_water = self.high_water.max(self.next_ordinal);
            self.next_ordinal = self.high_water;
        }
        pub(crate) fn mint(
            &mut self,
            machine_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
            source: PermissionEventSource,
        ) -> PermissionClaimIdentity {
            let ordinal = self.next_ordinal;
            self.next_ordinal = self
                .next_ordinal
                .checked_add(1)
                .expect("permission claim identity ordinal overflow");
            PermissionClaimIdentity::Established {
                machine_symbol,
                state_symbol,
                source,
                ordinal,
            }
        }
    }

    pub(crate) fn check_linear_obligations(
        program: &dyn ::validation::CheckingInput,
        facts: &mut CheckFacts,
        incoming_guards: &crate::checks::ranges::incoming_guards::IncomingGuardIndex,
    ) -> Result<(), Vec<Diagnostic>> {
        let frontiers = derive_crash_frontier_lower_bounds(program, facts, incoming_guards);
        record_crash_frontier_lower_bounds(&mut facts.contract_plans, &frontiers);
        validate_linear_permission_events(program, facts)?;
        borrowed_windows::check_recorded_invocations(program, facts)
    }

    /*
        A nominal cleanup machine is entitled to one whole valid value. Reject a
        move below any prefix carrying that entitlement; structural records without
        nominal cleanup remain decomposable, and moving the entitled value itself
        remains legal. Temporary partial moves must also retain every linear claim
        because there is no remaining local owner for an unselected sibling.

        The same occurrence walk tracks consumed owners and borrowed-storage
        holes, and selects old referents killed by replacement. These deaths must
        exist before contextual cleanup derives the hooks' entry requirements.
    */
    pub(super) fn prepare_storage_deaths(
        program: &dyn ::validation::CheckingInput,
        facts: &mut CheckFacts,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();
        let mut consumed_owner_uses = Vec::new();
        let mut deaths = Vec::new();
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                let Some(flow) = facts.flow.control.state(machine.symbol, state.symbol) else {
                    diagnostics.push(Diagnostic::error(format!(
                        "state `{}` has no checked flow for storage-transfer validation",
                        crate::published::labels::symbol_name(program, state.symbol),
                    )));
                    continue;
                };
                let segments = &facts.flow.ownership.segments;
                let moves = facts
                    .flow
                    .ownership
                    .storage_transfers
                    .span_or_empty(flow.storage_transfers);
                let statements = program.statement_table.statements(state.statement_nodes);
                let walk = borrowed_windows::StorageWalk::new(program, machine, state, statements);
                let state_calls = facts.flow.control.calls.span_or_empty(flow.calls);
                borrowed_windows::check_call_occurrences(
                    program,
                    machine,
                    state,
                    state_calls,
                    &facts.flow.control,
                    &mut diagnostics,
                );
                let captures = crate::checks::StorageSelectorFacts {
                    flow: facts.flow.view(),
                    semantic: &facts.semantic,
                };
                let mut windows = borrowed_windows::BorrowedStorageWindows::new(captures);
                let mut consumed = ConsumedOwners::default();
                for (statement_index, statement) in program
                    .statement_table
                    .admitted_execution_statements(state.statement_nodes)
                {
                    let is_transition = matches!(statement, StatementNode::Transition(_));
                    consumed.begin_statement();
                    let mut arm_plan = borrowed_windows::ArmWindowPlan::new(
                        program,
                        statement,
                        &facts.operators,
                        captures,
                    );
                    if !is_transition && arm_plan.has_frames() {
                        for event in moves.iter().filter(|event| {
                            event_statement_index(event.source) == Some(statement_index)
                        }) {
                            let path = segments.span_or_empty(event.segments);
                            if move_event_is_production_target(program, state, event, path)
                                || !projected_affine::is_borrowed_place_transfer(
                                    program,
                                    machine.symbol,
                                    state,
                                    event,
                                    path,
                                )
                                || borrowed_move_crosses_nominal_drop(program, state, event, path)
                            {
                                continue;
                            }
                            arm_plan.add_borrowed_move(&walk, statement_index, event, path);
                        }
                        arm_plan.seal(&walk);
                    }
                    for step in crate::checks::ownership_occurrences::statement_steps(
                        program,
                        machine,
                        state,
                        statement_index,
                        statement,
                        moves,
                        &facts.operators,
                    ) {
                        let (event_index, conditional, ordered) = match step.step {
                            crate::checks::ownership_occurrences::StatementStep::Observe {
                                place,
                                expression,
                            } => {
                                windows.check_use(&walk, statement_index, &place, &mut diagnostics);
                                arm_plan.check_edge_use(
                                    &walk,
                                    statement_index,
                                    expression,
                                    &place,
                                    &mut diagnostics,
                                );
                                consumed.check_use(
                                    program,
                                    machine,
                                    state,
                                    statement,
                                    statement_index,
                                    &place,
                                    &step.route,
                                    &mut consumed_owner_uses,
                                );
                                continue;
                            }
                            crate::checks::ownership_occurrences::StatementStep::Complete(
                                expression,
                            ) => {
                                for parent in super::content_transformations::consumed_places(
                                    facts,
                                    machine.symbol,
                                    state.symbol,
                                    statement_index,
                                    expression,
                                ) {
                                    consumed.record_owned_place(
                                        program,
                                        machine,
                                        state,
                                        statement_index,
                                        &parent,
                                        &step.route,
                                    );
                                }
                                for pending in arm_plan.commit_pending(expression) {
                                    let event = &moves[pending];
                                    if let Some(diagnostic) = windows.open(
                                        &walk,
                                        statement_index,
                                        event,
                                        segments.span_or_empty(event.segments),
                                    ) {
                                        diagnostics.push(diagnostic);
                                    }
                                }
                                if (windows.has_open_windows() || arm_plan.has_edge_absent())
                                    && (state_calls.iter().any(|call| {
                                        call.statement_index == statement_index
                                            && call.authored_expression == expression
                                    }) || borrowed_windows::state_has_boundary_operator(
                                        program,
                                        &facts.flow.control,
                                        &facts.operators,
                                        state.symbol,
                                        Some(statement_index),
                                        Some(expression),
                                    ))
                                {
                                    windows.record_invocation(
                                        machine.symbol,
                                        state.symbol,
                                        statement_index,
                                        expression,
                                        &mut facts.contextual_cleanup,
                                    );
                                    arm_plan.record_edge_invocation(
                                        machine.symbol,
                                        state.symbol,
                                        statement_index,
                                        expression,
                                        &mut facts.contextual_cleanup,
                                    );
                                }
                                continue;
                            }
                            crate::checks::ownership_occurrences::StatementStep::Move {
                                event,
                                conditional,
                                ordered,
                            } => (event, conditional, ordered),
                        };
                        let event = &moves[event_index];
                        let path = segments.span_or_empty(event.segments);
                        if move_event_is_production_target(program, state, event, path) {
                            continue;
                        }
                        if projected_affine::is_borrowed_place_transfer(
                            program,
                            machine.symbol,
                            state,
                            event,
                            path,
                        ) {
                            if is_transition
                                || borrowed_move_crosses_nominal_drop(program, state, event, path)
                            {
                                diagnostics.push(borrowed_windows::borrowed_transfer_diagnostic(
                                    machine,
                                    state,
                                    event_statement_index(event.source).unwrap_or(0),
                                ));
                                continue;
                            }
                            match arm_plan.verdict(&walk, statement_index, event_index, event, path)
                            {
                                Some(borrowed_windows::ArmWindowVerdict::Open) => {}
                                None => {
                                    if conditional {
                                        diagnostics.push(
                                            borrowed_windows::borrowed_transfer_diagnostic(
                                                machine,
                                                state,
                                                event_statement_index(event.source).unwrap_or(0),
                                            ),
                                        );
                                    } else if let Some(diagnostic) =
                                        windows.open(&walk, statement_index, event, path)
                                    {
                                        diagnostics.push(diagnostic);
                                    }
                                }
                                Some(borrowed_windows::ArmWindowVerdict::SiblingRoute) => {
                                    if !borrowed_windows::authored_route_exclusive(
                                        &walk,
                                        statement_index,
                                        event.root,
                                        path,
                                    ) {
                                        diagnostics.push(
                                            borrowed_windows::borrowed_transfer_diagnostic(
                                                machine,
                                                state,
                                                event_statement_index(event.source).unwrap_or(0),
                                            ),
                                        );
                                    }
                                }
                                Some(borrowed_windows::ArmWindowVerdict::Reject) => diagnostics
                                    .push(borrowed_windows::borrowed_transfer_diagnostic(
                                        machine,
                                        state,
                                        event_statement_index(event.source).unwrap_or(0),
                                    )),
                                Some(borrowed_windows::ArmWindowVerdict::DeadArm) => {}
                            }
                            continue;
                        }
                        if !path.is_empty() {
                            for prefix_len in 0..path.len() {
                                let prefix = validation::CanonicalPlace {
                                    root: event.root,
                                    segments: path[..prefix_len].to_vec(),
                                };
                                let Some((type_name, hook)) = nominal_drop_place_name(
                                    program,
                                    state.symbol,
                                    event_statement_index(event.source).unwrap_or(0),
                                    &prefix,
                                ) else {
                                    continue;
                                };
                                diagnostics.push(Diagnostic::error(format!(
                                    "cannot partially move a value of `{type_name}` because `{hook}` requires the whole value"
                                )));
                                break;
                            }
                        }
                        windows.refuse_move_over_open(
                            &walk,
                            statement_index,
                            event,
                            path,
                            &mut diagnostics,
                        );
                        if ordered {
                            consumed.record_move(
                                program,
                                machine,
                                state,
                                statement_index,
                                event,
                                path,
                                &step.route,
                            );
                        }
                    }
                    if let Some(assignment) = statement.as_assignment() {
                        if let Some(death) =
                            windows.replacement_death(&walk, statement_index, assignment)
                        {
                            deaths.push((machine.symbol, state.symbol, statement_index, death));
                        }
                        windows.check_repair(&walk, statement_index, assignment, &mut diagnostics);
                        consumed.check_store(
                            program,
                            machine,
                            state,
                            statement_index,
                            assignment,
                            &mut consumed_owner_uses,
                        );
                    }
                    consumed.finish_statement(is_transition);
                    let is_crash = matches!(
                        statement,
                        StatementNode::Transition(transition)
                            if matches!(
                                transition.exit,
                                typed_trees::statement::TransitionExit::Crash(_)
                            )
                    );
                    if is_transition && !is_crash {
                        windows.refuse_open_at_exit(machine, state, &mut diagnostics);
                    }
                }
                if !matches!(statements.last(), Some(StatementNode::Transition(_))) {
                    windows.refuse_open_at_exit(machine, state, &mut diagnostics);
                }
            }
        }
        diagnostics.extend(consumed_owner_uses);
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        for (machine, state, statement_index, (place, disposed_type, excluded)) in deaths {
            let path = facts.flow.ownership.segments.insert_many(place.segments);
            let exclusions = excluded
                .into_iter()
                .map(|segments| checked_trees::ContextualCleanupExcludedRegion {
                    root: place.root,
                    segments: facts.contextual_cleanup.segments.insert_many(segments),
                    cause: checked_trees::ContextualCleanupExclusionCause::Unknown,
                })
                .collect::<Vec<_>>();
            facts.contextual_cleanup.dispositions.append(
                checked_trees::ContextualCleanupDisposition {
                    selection: Default::default(),
                    establishment_ordered: false,
                    zero_image: false,
                    machine,
                    state,
                    source: PermissionEventSource::Statement { statement_index },
                    exit: checked_trees::FlowClaimJoinExit::default(),
                    completion: Default::default(),
                    completion_constraints: None,
                    root: place.root,
                    disposed_type,
                    path,
                    exclusions: facts.contextual_cleanup.exclusions.insert_many(exclusions),
                    definite_exclusions: Default::default(),
                    inactive_cases: Default::default(),
                },
            );
        }
        Ok(())
    }

    /*
        Nominal cleanup is entitled to one whole value: a move out of borrowed
        storage that crosses an entitled prefix stays rejected even when a later
        repair would reseat the hole.
    */
    fn borrowed_move_crosses_nominal_drop(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        event: &crate::flow::DiscoveredMoveEvent,
        path: &[facts::PlaceSegment],
    ) -> bool {
        (0..path.len()).any(|prefix_len| {
            let prefix = validation::CanonicalPlace {
                root: event.root,
                segments: path[..prefix_len].to_vec(),
            };
            nominal_drop_place_name(
                program,
                state.symbol,
                event_statement_index(event.source).unwrap_or(0),
                &prefix,
            )
            .is_some()
        })
    }

    pub(crate) fn type_reference_is_reference(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        crate::checks::reference_access(program, type_reference).is_some()
    }

    /* The owned type at `place` and the hook its `dropped by` selection names. */
    fn nominal_drop_place_name<'program>(
        program: &'program dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        place: &validation::CanonicalPlace,
    ) -> Option<(&'program str, &'program str)> {
        let type_reference = validation::canonical_place_type_reference(
            program,
            state_symbol,
            statement_index,
            place,
        )?;
        nominal_drop_type_name(program, type_reference)
    }

    pub(super) fn move_event_is_production_target(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        event: &crate::flow::DiscoveredMoveEvent,
        path: &[facts::PlaceSegment],
    ) -> bool {
        if event.expression.is_valid()
            || !matches!(
                event.source,
                crate::flow::FlowOwnershipEventSource::Statement { .. }
            )
        {
            return false;
        }
        let Some(statement_index) = event_statement_index(event.source) else {
            return false;
        };
        let Some(statement) = program.statement_table.statement_at(state, statement_index) else {
            return false;
        };
        let target = match statement {
            StatementNode::LocalData(local) => validation::CanonicalPlace {
                root: facts::PlaceRoot::Symbol(local.symbol),
                segments: Vec::new(),
            },
            StatementNode::Assignment(assignment) => {
                let Some(target) =
                    place_at(program, state.symbol, statement_index, assignment.target)
                else {
                    return false;
                };
                target
            }
            StatementNode::Expression(expression) => validation::CanonicalPlace {
                root: facts::PlaceRoot::Expression(*expression),
                segments: Vec::new(),
            },
            _ => return false,
        };
        (target.root) == event.root && target.segments.as_slice() == path
    }

    fn nominal_drop_type_name(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<(&str, &str)> {
        let mut owned = type_reference;
        while let TypeReferenceNode::Reference { referee, .. } =
            program.type_reference_table.type_reference(owned)
        {
            owned = *referee;
        }
        let hook = validation::nominal_cleanup_machine_for_type(program, owned, &[])?;
        let mut nominal = owned;
        let (symbol, name) = loop {
            match program.type_reference_table.type_reference(nominal) {
                TypeReferenceNode::Constrained { base_type, .. } => nominal = *base_type,
                TypeReferenceNode::Named { symbol, name } => break (*symbol, name),
                TypeReferenceNode::Generic {
                    base_symbol,
                    base_name,
                    ..
                } => break (*base_symbol, base_name),
                _ => return None,
            }
        };
        let definition =
            ::validation::field_domains::data_definition_by_symbol_or_name(program, symbol, name)?;
        let authored = program.symbols.authored_origin(hook.symbol);
        Some((definition.name.as_str(), program.symbols.name(authored)))
    }
}
#[path = "checks_multiplicity_linear_validation.rs"]
mod linear_validation;
mod owned_cleanup {
    /*
        Exact claim receipts for an owner-authorized recursive cleanup region.
    */
    use super::linear_obligations::LinearPlace;
    use facts::{PlaceRoot, PlaceSegment};
    use language_semantics::{Multiplicity, PermissionClaimIdentity, PermissionProvenance};
    use typed_trees::types::TypeReferenceHandle;

    /*
        A transparent container has no aggregate claim. Its one recursive cleanup
        plan consumes the existing live claims below the selected region. Absence
        comes from the retained contextual disposition's exact exclusions, checked
        against current claim custody, never from a missing type-frontier entry.
    */
    pub(super) fn claim_receipts<'a>(
        program: &dyn validation::CheckingInput,
        reference: TypeReferenceHandle,
        root: PlaceRoot,
        path: &[PlaceSegment],
        places: &'a [LinearPlace],
        removed: &[&[PlaceSegment]],
        inactive_cases: &[&[PlaceSegment]],
    ) -> Option<Vec<&'a LinearPlace>> {
        let mut receipts = places
            .iter()
            .filter(|place| {
                place.root == root
                    && place.live
                    && place.multiplicity == Multiplicity::Linear
                    && place.path.starts_with(path)
            })
            .collect::<Vec<_>>();
        receipts.sort_by_key(|place| place.path != path);
        if receipts.is_empty() && !super::type_carries_linear_obligation(program, reference) {
            return Some(receipts);
        }
        if receipts.iter().any(|place| {
            matches!(
                place.claim_identity,
                None | Some(PermissionClaimIdentity::Unknown)
            ) || matches!(place.provenance, None | Some(PermissionProvenance::Unknown))
        }) {
            return None;
        }
        if receipts.iter().any(|place| {
            removed
                .iter()
                .chain(inactive_cases)
                .any(|removed| place.path[path.len()..].starts_with(removed))
        }) {
            return None;
        }
        crate::flow::residual_cleanup_is_eligible(program, reference, removed, inactive_cases)
            .then_some(receipts)
    }
}
mod owned_selection {
    /*
        Conditional completed-value transfers, keyed by retained selection captures.
        Availability is conservative; residual custody remains in the receipts until
        the actual death edge. Every candidate keeps its physical source home;
        selected transfer and complementary disposal retain their exact occurrences.
        A shared-reference member owns descriptor storage, not its referent. Its
        carrier therefore needs the same selected-source receipt as other owned
        storage; loan checking independently retains its complete backing frontier.
        The occurrence frontier ends only at an authenticated receiving write, not
        at inherited provenance or a sibling edge's establishment.
    */
    pub(super) use availability::{apply_availability, retire_source_storage};
    pub(super) use captures::SelectionProduction;
    pub(in crate::checks::multiplicity) use captures::evaluated_source_custody;
    pub(in crate::checks) use operands::reachable_arms;
    mod availability {
        /*
            Apply the all-path availability of an already recorded captured selection.
            Its incoming receipt keeps alternatives; an unknown joined scalar claim is
            never an alternative origin by itself.
        */
        use crate::checks::multiplicity::linear_obligations::LinearPlace;
        use checked_trees::{FlowOwnedSelectionReceipt, FlowOwnershipFacts};
        use language_semantics::{Multiplicity, PermissionClaimIdentity, PermissionProvenance};

        pub(in crate::checks::multiplicity) fn apply_availability(
            program: &dyn validation::CheckingInput,
            ownership: &checked_trees::FlowOwnershipFacts,
            receipt: &FlowOwnedSelectionReceipt,
            places: &mut [LinearPlace],
        ) {
            let transfers = ownership
                .selection_transfers
                .span_or_empty(receipt.transfers);
            for transfer in transfers {
                if transfer.claims.is_empty()
                    && validation::reference_result_custody::source_value_is_copied(
                        program,
                        transfer.source_type_reference,
                    )
                {
                    continue;
                }
                let path = ownership.segments.span_or_empty(transfer.path);
                for place in places
                    .iter_mut()
                    .filter(|place| super::source_frontier_matches(place, transfer.root, path))
                {
                    place.live = false;
                }
            }
            if !receipt.destination.is_valid() {
                return;
            }
            let prefix = ownership.segments.span_or_empty(receipt.destination_path);
            for place in places
                .iter_mut()
                .filter(|place| place.root == receipt.destination && place.path.starts_with(prefix))
            {
                let relative = &place.path[prefix.len()..];
                let mut origins = Vec::new();
                for transfer in transfers {
                    let destination = ownership.segments.span_or_empty(transfer.destination_path);
                    let Some(suffix) = relative.strip_prefix(destination) else {
                        continue;
                    };
                    let mut source = ownership.segments.span_or_empty(transfer.path).to_vec();
                    source.extend_from_slice(suffix);
                    for claim in ownership
                        .selection_transfer_claims
                        .span_or_empty(transfer.claims)
                    {
                        if ownership.segments.span_or_empty(claim.path) == source {
                            origins.push((claim.claim_identity, claim.provenance));
                        }
                    }
                }
                let origin = origins
                    .first()
                    .copied()
                    .filter(|first| origins.iter().all(|origin| origin == first));
                place.live = true;
                place.case_excluded = false;
                place.ever_established = true;
                place.zero_image = false;
                place.claim_identity = Some(
                    origin
                        .map(|origin| origin.0)
                        .unwrap_or(PermissionClaimIdentity::Unknown),
                );
                place.provenance = Some(
                    origin
                        .map(|origin| origin.1)
                        .unwrap_or(PermissionProvenance::Unknown),
                );
                if place.multiplicity == Multiplicity::Affine {
                    place.claim_identity = None;
                }
            }
        }

        /*
            A captured source now lives in the selection's continuation roster. Its
            unavailable affine home cannot also retain an ordinary exit disposition.
            A receiving whole write is still live and keeps its new storage occurrence;
            exact linear descendants keep their separate claim and cleanup accounting.
        */
        pub(in crate::checks::multiplicity) fn retire_source_storage(
            ownership: &FlowOwnershipFacts,
            receipt: &FlowOwnedSelectionReceipt,
            places: &[LinearPlace],
            remaining: &mut crate::checks::multiplicity::cleanup_dispositions::RemainingCustody,
        ) {
            for source in ownership.selection_sources.span_or_empty(receipt.sources) {
                let root = facts::PlaceRoot::Symbol(source.symbol);
                if places.iter().any(|place| {
                    place.root == root
                        && place.path.is_empty()
                        && place.multiplicity == Multiplicity::Affine
                        && place.ever_established
                        && !place.live
                }) {
                    remaining.transfer_storage(root, &[]);
                }
            }
        }
    }
    mod captures {
        /*
            Selected custody is derived from the completed value graph. The source
            occurrence schedule still owns evaluation and all operand effects. These
            rows describe only the exact value transport at completion, not an AST
            recognizer or another ownership evaluator.
        */
        use crate::checks::multiplicity::linear_obligations::LinearPlace;
        use crate::checks::multiplicity::linear_validation::linear_claim_frontier;
        use crate::flow::canonical_place_from_semantic_place;
        use arena::Handle;
        use checked_trees::{
            BorrowAccessKind, BorrowLoanLineage, FlowCallFact, FlowOperandCompletion,
            FlowOperandFact, FlowOwnershipFacts, FlowPermissionEvaluation, FlowPermissionEventFact,
        };
        use diagnostics::Diagnostic;
        use facts::{PlaceRoot, PlaceSegment};
        use language_semantics::{Multiplicity, ReferenceAccess};
        use symbols::SymbolHandle;
        use typed_trees::expression::{ExpressionHandle, TableMatchArm};
        use typed_trees::types::TypeReferenceHandle;
        use validation::CallFrameResolver;
        use validation::CanonicalPlace;
        use validation::CheckingInput;
        use validation::canonical_place_type_reference;
        use validation::ownership_place_from_expression_in_state;
        use validation::reference_formation_source;
        use validation::reference_result_custody::binding_multiplicity;
        use validation::reference_result_custody::initializer_source;
        use validation::reference_result_custody::source_value_is_copied;
        use validation::reference_shell;

        macro_rules! selection {
            ($message:literal) => {
                return Err(Diagnostic::error($message))
            };
        }

        pub(in super::super) struct CapturedOwner {
            pub expression: ExpressionHandle,
            pub source_arm: Handle<TableMatchArm>,
            pub source: CanonicalPlace,
            pub destination: Vec<PlaceSegment>,
            pub type_reference: TypeReferenceHandle,
        }

        pub(in super::super) struct SelectionProduction {
            value: Handle<FlowOperandFact>,
            pub expression: ExpressionHandle,
            pub evaluation: FlowPermissionEvaluation,
            pub destination: CanonicalPlace,
            pub type_reference: TypeReferenceHandle,
            pub owners: Vec<CapturedOwner>,
            pub transported: Vec<ExpressionHandle>,
        }

        impl SelectionProduction {
            /*
                A completed source must exist before capture can transfer its custody,
                even when the same expression also names the outgoing receiving value.
            */
            pub(in super::super) fn captures_anonymous_source(
                &self,
                expression: ExpressionHandle,
            ) -> bool {
                self.owners.iter().any(|owner| {
                    owner.expression == expression
                        && owner.source.root == PlaceRoot::Expression(expression)
                        && owner.source.segments.is_empty()
                })
            }

            pub(in super::super) fn for_statement(
                program: &dyn CheckingInput,
                facts: checked_trees::CheckFactsRef<'_>,
                machine: SymbolHandle,
                state: SymbolHandle,
                statement: usize,
                evaluation: FlowPermissionEvaluation,
                places: &[LinearPlace],
            ) -> Result<Option<Self>, Diagnostic> {
                let Some(flow) = facts.flow.control.state(machine, state) else {
                    return Ok(None);
                };
                let stored = facts.flow.state_statement(flow, statement);
                let (capture, destination) = if let Some(stored) = stored.filter(|row| {
                    facts.flow.control.operands.is_valid(row.stored_value)
                        && evaluation == FlowPermissionEvaluation::Statement
                }) {
                    let operand = facts.flow.control.operands.get(stored.stored_value);
                    if !has_selection(facts, operand)
                        || binding_multiplicity(program, operand.type_reference)
                            == Multiplicity::Unrestricted
                    {
                        return Ok(None);
                    }
                    let destination = facts
                        .semantic
                        .places
                        .is_valid(stored.stored_destination)
                        .then(|| facts.semantic.places.get(stored.stored_destination))
                        .and_then(|place| {
                            canonical_place_from_semantic_place(program, facts.semantic, place)
                        })
                        .ok_or_else(|| {
                            Diagnostic::error("selected store has no exact receiving place")
                        })?;
                    (stored.stored_value, destination)
                } else {
                    let Some(outgoing) = facts
                        .flow
                        .control
                        .outgoing
                        .span_or_empty(flow.outgoing)
                        .iter()
                        .find(|row| {
                            row.statement_index == statement
                                && row.returned_value.is_valid()
                                && match evaluation {
                                    FlowPermissionEvaluation::Statement => {
                                        !row.transition_target.is_valid()
                                    }
                                    FlowPermissionEvaluation::TransitionTarget(target) => {
                                        row.transition_target == target
                                    }
                                    _ => false,
                                }
                        })
                    else {
                        return Ok(None);
                    };
                    (
                        outgoing.returned_value,
                        CanonicalPlace {
                            root: PlaceRoot::Unknown,
                            segments: Vec::new(),
                        },
                    )
                };
                let operand = facts.flow.control.operands.get(capture);
                if !has_selection(facts, operand)
                    || binding_multiplicity(program, operand.type_reference)
                        == Multiplicity::Unrestricted
                {
                    return Ok(None);
                }
                let mut owners = Vec::new();
                let mut transported = Vec::new();
                collect(
                    program,
                    facts,
                    machine,
                    evaluation,
                    places,
                    operand,
                    Handle::invalid(),
                    &[],
                    &mut owners,
                    &mut transported,
                )?;
                if owners.is_empty() && reference_shell(program, operand.type_reference).is_some() {
                    return Ok(None);
                }
                if owners.is_empty() {
                    selection!("completed owned selection has no captured storage frontier");
                }
                Ok(Some(Self {
                    value: capture,
                    expression: operand.expression,
                    evaluation,
                    destination,
                    type_reference: operand.type_reference,
                    owners,
                    transported,
                }))
            }

            pub(in super::super) fn record(
                &self,
                program: &dyn CheckingInput,
                facts: &mut checked_trees::CheckFactsWalk<'_>,
                machine: SymbolHandle,
                state: SymbolHandle,
                statement: usize,
                evaluated_transfers: &[super::super::cleanup_dispositions::TransferCustody],
                evaluated_observations: &[super::super::linear_validation::ObservedCustody],
                routes: &super::super::selected_evaluation::ExpressionRoutes,
                calls: &[FlowCallFact],
                events: &[FlowPermissionEventFact],
                call_frames: Option<&CallFrameResolver<'_>>,
            ) -> Result<(), Diagnostic> {
                use checked_trees::{
                    FlowOwnedSelectionClaim, FlowOwnedSelectionReceipt, FlowOwnedSelectionSource,
                    FlowOwnedSelectionTransfer,
                };
                use language_semantics::{
                    PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
                    PermissionEventSource, PermissionProvenance,
                };
                let mut sources = Vec::new();
                for owner in &self.owners {
                    if source_value_is_copied(program, owner.type_reference) {
                        continue;
                    }

                    let Some(symbol) = owner.source.root.as_symbol() else {
                        continue;
                    };
                    if sources
                        .iter()
                        .any(|source: &FlowOwnedSelectionSource| source.symbol == symbol)
                    {
                        continue;
                    }
                    if !super::source_outputs_are_attributed(
                        program,
                        facts.view(),
                        machine,
                        state,
                        statement,
                        symbol,
                        call_frames,
                    ) {
                        selection!(
                            "selected source has an incompletely attributed retained output"
                        );
                    }
                    let declaration = program
                        .state_by_symbol(state)
                        .ok_or_else(|| Diagnostic::error("selected source state is unavailable"))?;
                    let parameter = program
                        .state_parameters(declaration)
                        .iter()
                        .enumerate()
                        .find(|(_, parameter)| parameter.symbol == symbol);
                    let ordinal = if let Some((position, _)) = parameter {
                        position
                    } else {
                        program
                            .state_local_declaration(declaration, symbol)
                            .map(|(ordinal, _)| ordinal)
                            .ok_or_else(|| {
                                Diagnostic::error(
                                    "selected source has no declaring storage occurrence",
                                )
                            })?
                    };
                    sources.push(FlowOwnedSelectionSource {
                        symbol,
                        statement_ordinal: ordinal as u32,
                        retains_storage: self.retains_source_storage(
                            owner.source.root,
                            routes,
                            calls,
                            events,
                        )?,
                    });
                }
                let declaration = program
                    .state_by_symbol(state)
                    .ok_or_else(|| Diagnostic::error("selected source state is unavailable"))?;
                sources.sort_by_key(|source| {
                    let local = program
                        .state_parameters(declaration)
                        .iter()
                        .all(|parameter| parameter.symbol != source.symbol);
                    std::cmp::Reverse((local, source.statement_ordinal))
                });
                let sources = facts.flow.ownership.selection_sources.insert_many(sources);
                let mut transfers = Vec::new();
                for owner in &self.owners {
                    let source = if source_value_is_copied(program, owner.type_reference) {
                        Handle::invalid()
                    } else {
                        owner
                            .source
                            .root
                            .as_symbol()
                            .and_then(|symbol| {
                                facts
                                    .flow
                                    .ownership
                                    .selection_sources
                                    .span_or_empty(sources)
                                    .iter()
                                    .position(|source| source.symbol == symbol)
                            })
                            .map(|position| sources.handle_at(position as u32))
                            .unwrap_or_default()
                    };
                    let evaluated_places = if owner.source.root.as_symbol().is_some() {
                        let (places, _, _) = evaluated_source_custody(
                            facts.flow.ownership,
                            owner.expression,
                            &owner.source,
                            self.evaluation,
                            evaluated_transfers,
                            evaluated_observations,
                            events,
                        )?;
                        Some(places)
                    } else {
                        None
                    };
                    let mut claims = Vec::new();
                    for template in linear_claim_frontier(program, owner.type_reference) {
                        let mut path = owner.source.segments.clone();
                        path.extend_from_slice(&template.path);
                        let same_completion = |event: &&FlowPermissionEventFact| {
                            event.expression == owner.expression
                                && event.root == owner.source.root
                                && event.machine_symbol == machine
                                && event.state_symbol == state
                                && event.evaluation == self.evaluation
                                && event.access == PermissionAccess::Owned
                                && event.source
                                    == PermissionEventSource::Statement {
                                        statement_index: statement,
                                    }
                                && facts.flow.ownership.segments.span_or_empty(event.segments)
                                    == path
                        };
                        let mut matches = events
                            .iter()
                            .filter(same_completion)
                            .filter(|event| event.kind == PermissionEventKind::Transfer);
                        let event = if let Some(event) = matches.next() {
                            event
                        } else {
                            let mut inactive =
                                events.iter().filter(same_completion).filter(|event| {
                                    template.conditional
                                        && event.kind == PermissionEventKind::Establish
                                        && event.multiplicity == Multiplicity::Linear
                                        && !event.obligation_live
                                });
                            if inactive.next().is_some() && inactive.next().is_none() {
                                continue;
                            }
                            selection!(
                                "selected completion has no exact consumed claim occurrence"
                            );
                        };
                        if matches.next().is_some() {
                            selection!("selected completion has ambiguous claim custody");
                        }
                        if let Some(places) = evaluated_places {
                            let mut matching = places.iter().filter(|place| {
                                place.root == owner.source.root && place.path == path
                            });
                            let (Some(place), None) = (matching.next(), matching.next()) else {
                                selection!("selected claim has no unique evaluated source custody");
                            };
                            if !place.live
                                || !place.ever_established
                                || place.case_excluded
                                || place
                                    .claim_identity
                                    .unwrap_or(PermissionClaimIdentity::Unknown)
                                    != event.claim_identity
                                || place.provenance.unwrap_or(PermissionProvenance::Unknown)
                                    != event.provenance
                            {
                                selection!(
                                    "selected claim disagrees with its evaluated source custody"
                                );
                            }
                        }
                        claims.push(FlowOwnedSelectionClaim {
                            path: facts.flow.ownership.segments.insert_many(path),
                            claim_identity: event.claim_identity,
                            provenance: event.provenance,
                        });
                    }
                    let (provenance, claim_identity) = if let [claim] = claims.as_slice() {
                        (claim.provenance, claim.claim_identity)
                    } else if !claims.is_empty() {
                        let provenance = evaluated_places
                            .and_then(|places| {
                                let mut matching = places.iter().filter(|place| {
                                    place.root == owner.source.root
                                        && place.path == owner.source.segments
                                        && place.multiplicity == Multiplicity::Affine
                                        && place.live
                                        && place.ever_established
                                });
                                let (Some(place), None) = (matching.next(), matching.next()) else {
                                    return None;
                                };
                                place.provenance
                            })
                            .unwrap_or(PermissionProvenance::Unknown);
                        (provenance, PermissionClaimIdentity::Unknown)
                    } else if source_value_is_copied(program, owner.type_reference) {
                        if evaluated_places.is_some_and(|places| {
                            places.iter().any(|place| {
                                super::source_frontier_matches(
                                    place,
                                    owner.source.root,
                                    &owner.source.segments,
                                ) && (!place.live || !place.ever_established || place.case_excluded)
                            })
                        }) {
                            selection!("selected copy has no live evaluated source custody");
                        }
                        (
                            PermissionProvenance::Unknown,
                            PermissionClaimIdentity::Unknown,
                        )
                    } else if let Some(places) = evaluated_places {
                        let depth = places
                            .iter()
                            .filter(|place| {
                                place.root == owner.source.root
                                    && owner.source.segments.starts_with(&place.path)
                                    && place.multiplicity == Multiplicity::Affine
                            })
                            .map(|place| place.path.len())
                            .max()
                            .ok_or_else(|| {
                                Diagnostic::error(
                                    "selected descriptor has no evaluated carrier custody",
                                )
                            })?;
                        let mut matching = places.iter().filter(|place| {
                            place.root == owner.source.root
                                && owner.source.segments.starts_with(&place.path)
                                && place.path.len() == depth
                                && place.multiplicity == Multiplicity::Affine
                        });
                        let (Some(place), None) = (matching.next(), matching.next()) else {
                            selection!(
                                "selected descriptor has ambiguous evaluated carrier custody"
                            );
                        };
                        if !place.live || !place.ever_established {
                            selection!("selected descriptor has no live evaluated carrier custody");
                        }
                        (
                            place.provenance.unwrap_or(PermissionProvenance::Unknown),
                            PermissionClaimIdentity::Unknown,
                        )
                    } else {
                        (
                            PermissionProvenance::Unknown,
                            PermissionClaimIdentity::Unknown,
                        )
                    };
                    let origin_selection = facts.flow.ownership.with_state_owned_selections(
                        state,
                        |handles| -> Result<
                            arena::Handle<checked_trees::FlowOwnedSelectionReceipt>,
                            Diagnostic,
                        > {
                            let mut origins = handles.iter().copied().filter(|&handle| {
                                let receipt = facts.flow.ownership.owned_selections.get(handle);
                                receipt.machine == machine
                                    && receipt.state == state
                                    && receipt.destination == owner.source.root
                                    && receipt.value.is_valid()
                                    && owner.source.segments.starts_with(
                                        facts
                                            .flow
                                            .ownership
                                            .segments
                                            .span_or_empty(receipt.destination_path),
                                    )
                                    && provenance
                                        == PermissionProvenance::Established {
                                            machine_symbol: machine,
                                            state_symbol: state,
                                            source: PermissionEventSource::Statement {
                                                statement_index: receipt.statement_ordinal as usize,
                                            },
                                        }
                            });
                            let origin = origins.next().unwrap_or_default();
                            if origins.next().is_some() {
                                selection!("selected source has ambiguous receiving custody");
                            }
                            Ok(origin)
                        },
                    )?;
                    transfers.push(FlowOwnedSelectionTransfer {
                        source_type_reference: owner.type_reference,
                        root: owner.source.root,
                        destination_path: facts
                            .flow
                            .ownership
                            .segments
                            .insert_many(owner.destination.iter().copied()),
                        expression: owner.expression,
                        source_arm: owner.source_arm,
                        source,
                        provenance,
                        claim_identity,
                        origin_selection,
                        path: facts
                            .flow
                            .ownership
                            .segments
                            .insert_many(owner.source.segments.iter().copied()),
                        claims: facts
                            .flow
                            .ownership
                            .selection_transfer_claims
                            .insert_many(claims),
                    });
                }
                let transfers = facts
                    .flow
                    .ownership
                    .selection_transfers
                    .insert_many(transfers);
                facts
                    .flow
                    .ownership
                    .owned_selections
                    .append(FlowOwnedSelectionReceipt {
                        value: self.value,
                        evaluation: self.evaluation,
                        machine,
                        state,
                        statement_ordinal: statement as u32,
                        expression: self.expression,
                        destination: self.destination.root,
                        destination_path: facts
                            .flow
                            .ownership
                            .segments
                            .insert_many(self.destination.segments.iter().copied()),
                        type_reference: self.type_reference,
                        sources,
                        transfers,
                        death: PermissionEventSource::StateExit,
                    });
                Ok(())
            }

            /*
                Derive residual presence through the existing completed occurrence
                schedule. Every branch joins immediately; a projected move or an arm
                which forms another value keeps this source's complement live.
            */
            fn retains_source_storage(
                &self,
                root: PlaceRoot,
                routes: &super::super::selected_evaluation::ExpressionRoutes,
                calls: &[FlowCallFact],
                events: &[FlowPermissionEventFact],
            ) -> Result<bool, Diagnostic> {
                let mut present = true;
                let mut evaluation = super::super::selected_evaluation::SelectedEvaluation::new();
                let join = |_, current: &mut bool, sibling: &bool| {
                    *current |= *sibling;
                    Ok(())
                };
                for event in events {
                    evaluation.enter(
                        routes.event(event, calls),
                        &mut present,
                        |_, _| Ok(()),
                        join,
                    )?;
                    if event.root == root
                        && event.access == language_semantics::PermissionAccess::Owned
                        && event.kind == language_semantics::PermissionEventKind::Transfer
                        && self.owners.iter().any(|owner| {
                            owner.source.root == root
                                && owner.expression == event.expression
                                && owner.source.segments.is_empty()
                        })
                    {
                        present = false;
                    }
                }
                evaluation.enter(&[], &mut present, |_, _| Ok(()), join)?;
                Ok(present)
            }

            pub(in super::super) fn receives(
                &self,
                root: PlaceRoot,
                path: &[PlaceSegment],
            ) -> bool {
                root == self.root() && path.starts_with(&self.destination.segments)
            }

            pub(in super::super) fn root(&self) -> PlaceRoot {
                if self.destination.root.is_valid() {
                    self.destination.root
                } else {
                    PlaceRoot::Expression(self.expression)
                }
            }
        }

        /*
            Rejoin one source with its actual evaluated custody. The event range names
            the pre-move snapshot; a descriptor read without a value Transfer instead
            uses its exact observation. A source home or statement-entry inventory can
            never substitute for either occurrence. The returned source cut starts with the
            next evaluation position: a retained source association starts only after this
            exact transfer/read, not at statement entry.
        */
        #[allow(clippy::too_many_arguments)]
        pub(in super::super) fn evaluated_source_custody<'evaluation>(
            ownership: &FlowOwnershipFacts,
            expression: ExpressionHandle,
            source: &CanonicalPlace,
            evaluation: FlowPermissionEvaluation,
            transfers: &'evaluation [super::super::cleanup_dispositions::TransferCustody],
            observations: &'evaluation [super::super::linear_validation::ObservedCustody],
            events: &[FlowPermissionEventFact],
        ) -> Result<
            (
                &'evaluation [LinearPlace],
                &'evaluation super::super::cleanup_dispositions::RemainingCustody,
                super::super::cleanup_dispositions::CustodyCut,
            ),
            Diagnostic,
        > {
            let mut snapshots = transfers.iter().filter(|snapshot| {
                snapshot.events.clone().any(|ordinal| {
                    events.get(ordinal).is_some_and(|event| {
                        event.expression == expression
                            && event.root == source.root
                            && event.evaluation == evaluation
                            && event.access == language_semantics::PermissionAccess::Owned
                            && event.kind == language_semantics::PermissionEventKind::Transfer
                            && ownership
                                .segments
                                .span_or_empty(event.segments)
                                .starts_with(&source.segments)
                    })
                })
            });
            if let Some(snapshot) = snapshots.next() {
                if snapshots.next().is_some() {
                    selection!("selected source has multiple evaluated transfer snapshots");
                }
                return Ok((
                    &snapshot.places,
                    &snapshot.remaining,
                    super::super::cleanup_dispositions::CustodyCut {
                        occurrence_ordinal: snapshot.occurrence_ordinal,
                        permission_prefix: snapshot.events.end,
                    },
                ));
            }
            let mut snapshots = observations
                .iter()
                .filter(|snapshot| snapshot.expression == expression && snapshot.place == *source);
            let (Some(snapshot), None) = (snapshots.next(), snapshots.next()) else {
                selection!("selected source has no unique evaluated read or transfer snapshot");
            };
            Ok((&snapshot.places, &snapshot.remaining, snapshot.cut))
        }

        fn has_selection(
            facts: checked_trees::CheckFactsRef<'_>,
            operand: &FlowOperandFact,
        ) -> bool {
            match operand.completion {
                FlowOperandCompletion::Selected(_) => true,
                FlowOperandCompletion::Constructed(children) => facts
                    .flow
                    .control
                    .operand_children
                    .span_or_empty(children)
                    .iter()
                    .any(|child| {
                        has_selection(facts, facts.flow.control.operands.get(child.value))
                    }),
                FlowOperandCompletion::Copied { .. }
                | FlowOperandCompletion::IncomingContents { .. }
                | FlowOperandCompletion::InvocationResult { .. }
                | FlowOperandCompletion::Observation => false,
            }
        }

        fn collect(
            program: &dyn CheckingInput,
            facts: checked_trees::CheckFactsRef<'_>,
            machine: SymbolHandle,
            evaluation: FlowPermissionEvaluation,
            places: &[LinearPlace],
            operand: &FlowOperandFact,
            arm: Handle<TableMatchArm>,
            destination: &[PlaceSegment],
            owners: &mut Vec<CapturedOwner>,
            transported: &mut Vec<ExpressionHandle>,
        ) -> Result<(), Diagnostic> {
            if binding_multiplicity(program, operand.type_reference) == Multiplicity::Unrestricted {
                return Ok(());
            }
            match operand.completion {
                FlowOperandCompletion::Selected(alternatives) => {
                    transported.push(operand.expression);
                    let rows = facts
                        .flow
                        .control
                        .operand_alternatives
                        .span(alternatives)
                        .filter(|rows| !rows.is_empty())
                        .ok_or_else(|| {
                            Diagnostic::error("owned selection has no completed alternatives")
                        })?;
                    for alternative in rows {
                        if !alternative.source_arm.is_valid()
                            || !facts.flow.control.operands.is_valid(alternative.value)
                        {
                            selection!("owned selection has an unavailable completed arm");
                        }
                        collect(
                            program,
                            facts,
                            machine,
                            evaluation,
                            places,
                            facts.flow.control.operands.get(alternative.value),
                            alternative.source_arm,
                            destination,
                            owners,
                            transported,
                        )?;
                    }
                    return Ok(());
                }
                FlowOperandCompletion::Constructed(children) => {
                    let frontier = linear_claim_frontier(program, operand.type_reference);
                    if !frontier.iter().any(|claim| claim.path.is_empty()) {
                        let before = owners.len();
                        for child in facts.flow.control.operand_children.span_or_empty(children) {
                            let mut path = destination.to_vec();
                            path.extend_from_slice(
                                facts
                                    .flow
                                    .control
                                    .operand_child_paths
                                    .span_or_empty(child.path),
                            );
                            collect(
                                program,
                                facts,
                                machine,
                                evaluation,
                                places,
                                facts.flow.control.operands.get(child.value),
                                arm,
                                &path,
                                owners,
                                transported,
                            )?;
                        }
                        if owners.len() != before {
                            transported.push(operand.expression);
                            return Ok(());
                        }
                    }
                }
                FlowOperandCompletion::Copied { .. }
                | FlowOperandCompletion::IncomingContents { .. }
                | FlowOperandCompletion::InvocationResult { .. }
                | FlowOperandCompletion::Observation => {}
            }
            let source = ownership_place_from_expression_in_state(
                program,
                operand.state_symbol,
                operand.statement_index,
                operand.expression,
            )
            .ok_or_else(|| Diagnostic::error("completed owned value has no exact source place"))?;
            let installed_descriptor = canonical_place_type_reference(
                program,
                operand.state_symbol,
                operand.statement_index,
                &source,
            )
            .is_some_and(|reference| {
                program.normalized_type_identity(reference)
                    == program.normalized_type_identity(operand.type_reference)
            }) && places.iter().any(|place| {
                super::source_frontier_matches(place, source.root, &source.segments)
                    && place.ever_established
            });
            if reference_shell(program, operand.type_reference).is_some()
                && (forms_checked_loan(program, facts, machine, evaluation, operand)
                    || (!installed_descriptor
                        && program
                            .state_by_symbol(operand.state_symbol)
                            .is_some_and(|state| {
                                initializer_source(
                                    program,
                                    state,
                                    operand.expression,
                                    operand.type_reference,
                                )
                                .is_some()
                            })))
            {
                return Ok(());
            }
            if source.segments.iter().any(|segment| {
                !matches!(segment, PlaceSegment::Field { symbol } if symbol.is_valid())
                    && !matches!(
                        segment,
                        PlaceSegment::FixedIndex { .. } | PlaceSegment::Case { .. }
                    )
            }) {
                selection!("completed owned selection has an unresolved source projection");
            }
            owners.push(CapturedOwner {
                expression: operand.expression,
                source_arm: arm,
                source,
                destination: destination.to_vec(),
                type_reference: operand.type_reference,
            });
            Ok(())
        }

        /*
            Formation and descriptor transport can have the same carrier and canonical
            referent. Keep the semantic formation operation joined to its checked loan
            occurrence rather than inferring a move from that carrier or referent.
        */
        fn forms_checked_loan(
            program: &dyn CheckingInput,
            facts: checked_trees::CheckFactsRef<'_>,
            machine: SymbolHandle,
            evaluation: FlowPermissionEvaluation,
            operand: &FlowOperandFact,
        ) -> bool {
            let Some(formation) = reference_formation_source(program, operand.expression) else {
                return false;
            };
            let access = match formation.access {
                ReferenceAccess::Shared => BorrowAccessKind::Read,
                ReferenceAccess::Mutable => BorrowAccessKind::Mutable,
                ReferenceAccess::WriteOnly => BorrowAccessKind::WriteOnly,
            };
            let Some((_, state)) = facts
                .borrow
                .first_state_borrow(machine, operand.state_symbol)
            else {
                return false;
            };
            let local = facts
                .borrow
                .loans
                .span_or_empty(state.loans)
                .iter()
                .filter(|loan| {
                    loan.statement_index == operand.statement_index
                        && loan.formation_expression == operand.expression
                        && loan.kind == access
                })
                .map(|loan| &loan.lineage);
            let returned = facts
                .borrow
                .returned_loans
                .span_or_empty(state.returned_loans)
                .iter()
                .filter(|loan| {
                    let loan_evaluation = if loan.transition_target.is_valid() {
                        FlowPermissionEvaluation::TransitionTarget(loan.transition_target)
                    } else {
                        FlowPermissionEvaluation::Statement
                    };
                    loan.statement_index == operand.statement_index
                        && loan.formation_expression == operand.expression
                        && loan.kind == access
                        && loan_evaluation == evaluation
                })
                .map(|loan| &loan.lineage);
            let mut found = false;
            for lineage in local.chain(returned) {
                match lineage {
                    BorrowLoanLineage::DirectRoot => {}
                    BorrowLoanLineage::Reborrow { parent_loan } => {
                        if !facts.borrow.state_owns_loan(state, *parent_loan) {
                            return false;
                        }
                    }
                    BorrowLoanLineage::UnretainedDerived => {}
                    BorrowLoanLineage::SuppliedOutput | BorrowLoanLineage::LifetimeDependency => {
                        return false;
                    }
                }
                found = true;
            }
            found
        }
    }
    mod occurrences {
        /*
            Selection consumes an occurrence, not its variable forever. This scratch
            frontier travels with the permission schedule's edge-local custody. Only
            an authenticated covering write, after RHS evaluation, ends its barrier;
            inherited claim identity or provenance does not date that new occurrence.
        */
        use super::place_paths_overlap;
        use crate::checks::multiplicity::linear_obligations::LinearPlace;
        use crate::published::places::place_at;
        use checked_trees::{
            FlowOwnedSelectionReceipt, FlowOwnershipFacts, FlowPermissionEventFact,
        };
        use diagnostics::Diagnostic;
        use language_semantics::{Multiplicity, PermissionAccess, PermissionEventKind};
        use validation::CanonicalPlace;

        #[derive(Clone)]
        struct ConsumedRegion {
            place: CanonicalPlace,
            linear: bool,
        }

        #[derive(Clone, Default)]
        pub(in crate::checks::multiplicity) struct SelectionOccurrences {
            consumed: Vec<ConsumedRegion>,
        }

        impl SelectionOccurrences {
            pub(in crate::checks::multiplicity) fn consume(
                &mut self,
                program: &dyn validation::CheckingInput,
                ownership: &FlowOwnershipFacts,
                receipt: &FlowOwnedSelectionReceipt,
            ) {
                let linear =
                    program.type_multiplicity(receipt.type_reference) == Multiplicity::Linear;
                let mut regions = Vec::new();
                if linear {
                    for transfer in ownership
                        .selection_transfers
                        .span_or_empty(receipt.transfers)
                    {
                        if !transfer.source.is_valid() {
                            continue;
                        }
                        let root = facts::PlaceRoot::Symbol(
                            ownership.selection_sources.get(transfer.source).symbol,
                        );
                        let claims = ownership
                            .selection_transfer_claims
                            .span_or_empty(transfer.claims);
                        if claims.is_empty() {
                            regions.push(CanonicalPlace {
                                root,
                                segments: ownership.segments.span_or_empty(transfer.path).to_vec(),
                            });
                        } else {
                            regions.extend(claims.iter().map(|claim| CanonicalPlace {
                                root,
                                segments: ownership.segments.span_or_empty(claim.path).to_vec(),
                            }));
                        }
                    }
                } else {
                    regions.extend(
                        ownership
                            .selection_sources
                            .span_or_empty(receipt.sources)
                            .iter()
                            .map(|source| CanonicalPlace {
                                root: facts::PlaceRoot::Symbol(source.symbol),
                                segments: Vec::new(),
                            }),
                    );
                }
                for place in regions {
                    if !self.consumed.iter().any(|region| region.place == place) {
                        self.consumed.push(ConsumedRegion { place, linear });
                    }
                }
            }

            /*
                Observe actual reads, not the names nested inside projections or a
                whole assignment destination. A strict child write cannot fill its
                unavailable containing owner, even if no later read occurs.
            */
            pub(in crate::checks::multiplicity) fn check_statement(
                &self,
                program: &dyn validation::CheckingInput,
                facts: checked_trees::CheckFactsRef<'_>,
                machine: &typed_trees::machine::Machine,
                state: &typed_trees::state::State,
                statement_index: usize,
                statement: &typed_trees::statement::StatementNode,
            ) -> Result<(), Diagnostic> {
                if self.consumed.is_empty() {
                    return Ok(());
                }
                for occurrence in crate::checks::ownership_occurrences::statement_steps(
                    program,
                    machine,
                    state,
                    statement_index,
                    statement,
                    &[],
                    facts.operators,
                ) {
                    if let crate::checks::ownership_occurrences::StatementStep::Observe {
                        place,
                        ..
                    } = occurrence.step
                        && let Some(region) = self.conflict(place.root, &place.segments)
                    {
                        return Err(consumed_use(region.linear));
                    }
                }
                if let typed_trees::statement::StatementNode::Assignment(assignment) = statement
                    && let Some(destination) =
                        place_at(program, state.symbol, statement_index, assignment.target)
                    && let Some(region) = self.consumed.iter().find(|region| {
                        region.place.root == destination.root
                            && place_paths_overlap(&destination.segments, &region.place.segments)
                            && !region.place.segments.starts_with(&destination.segments)
                    })
                {
                    return Err(consumed_use(region.linear));
                }
                Ok(())
            }

            fn conflict(
                &self,
                root: facts::PlaceRoot,
                path: &[facts::PlaceSegment],
            ) -> Option<&ConsumedRegion> {
                self.consumed.iter().find(|region| {
                    region.place.root == root && place_paths_overlap(path, &region.place.segments)
                })
            }

            /*
                No unconditional event may consume or restore a still-selected source.
                The producer checks its RHS prefix before calling `establish`; replay
                checks the same ordered events using the checked write destination.
            */
            pub(in crate::checks::multiplicity) fn check_events<'a>(
                &self,
                program: &dyn validation::CheckingInput,
                events: impl IntoIterator<Item = &'a FlowPermissionEventFact>,
                segments: &arena::PagedArenaSet<facts::PlaceSegment>,
            ) -> Result<(), Diagnostic> {
                for event in events {
                    if event.access == PermissionAccess::Owned
                        && event.kind != language_semantics::PermissionEventKind::AffineDrop
                        && self
                            .conflict(event.root, segments.span_or_empty(event.segments))
                            .is_some()
                    {
                        return Err(Diagnostic::error(format!(
                            "owned selection acquired unconditional transfer, establishment, or residual cleanup events in machine `{}` state `{}` at {:?}",
                            program.symbols.name(event.machine_symbol),
                            program.symbols.name(event.state_symbol),
                            event.source,
                        )));
                    }
                }
                Ok(())
            }

            pub(in crate::checks::multiplicity) fn establish(
                &mut self,
                root: facts::PlaceRoot,
                destination: &[facts::PlaceSegment],
            ) {
                self.consumed.retain(|region| {
                    region.place.root != root || !region.place.segments.starts_with(destination)
                });
            }

            pub(in crate::checks::multiplicity) fn replay_statement(
                &mut self,
                program: &dyn validation::CheckingInput,
                state: &typed_trees::state::State,
                statement_index: usize,
                evaluation: checked_trees::FlowPermissionEvaluation,
                events: &[&FlowPermissionEventFact],
                ownership: &FlowOwnershipFacts,
                places: &[LinearPlace],
            ) -> Result<(), Diagnostic> {
                let statement =
                    &program.statement_table.statements(state.statement_nodes)[statement_index];
                let targets = super::super::linear_validation::written_linear_targets(
                    program,
                    state.symbol,
                    statement_index,
                    statement,
                    evaluation,
                    places,
                );
                for event in events.iter().copied().filter(|event| {
                    super::super::linear_validation::permission_event_statement_index(event.source)
                        == Some(statement_index)
                        && event.evaluation == evaluation
                }) {
                    if event.kind == PermissionEventKind::Establish
                        && event.access == PermissionAccess::Owned
                    {
                        let path = ownership.segments.span_or_empty(event.segments);
                        for target in targets.iter().filter(|target| {
                            target.root == event.root && places[target.place_index].path == path
                        }) {
                            self.establish(target.root, &target.destination_path);
                        }
                    }
                    self.check_events(program, std::iter::once(event), &ownership.segments)?;
                }
                Ok(())
            }
        }

        fn consumed_use(linear: bool) -> Diagnostic {
            Diagnostic::error(if linear {
                "linear match transfer consumed this exact claim on every edge; it cannot be used here"
            } else {
                "owned value may have been transferred by an earlier match; it cannot be used here"
            })
        }
    }
    mod operands {
        /*
            First-match reachability shared by flow capture and custody consumers.
        */
        use arena::{Handle, HandleSpan};
        use typed_trees::expression::{ExpressionNode, MatchPattern, TableMatchArm};

        /*
            Ordered first-match reachability. The source type checker still visits all
            authored arms. Only literal equality and complete Boolean/wildcard coverage
            remove ownership alternatives here; no predicate theorem is invented.
        */
        pub(in crate::checks) fn reachable_arms(
            program: &dyn ::validation::CheckingInput,
            arms: HandleSpan<TableMatchArm>,
        ) -> Vec<(Handle<TableMatchArm>, &TableMatchArm)> {
            let mut selected = Vec::new();
            let mut patterns = Vec::new();
            let mut booleans = [false; 2];
            for (ordinal, arm) in program.expression_table.match_arms(arms).iter().enumerate() {
                let mut covered = false;
                match arm.pattern {
                    MatchPattern::Destructure(_) => {}
                    MatchPattern::Wildcard => covered = true,
                    MatchPattern::Value(pattern) => {
                        let node = program.expression_table.expression(pattern);
                        let duplicate = patterns.iter().any(|previous| {
                            match (program.expression_table.expression(*previous), node) {
                                (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) => {
                                    left == right
                                }
                                (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => {
                                    left.value_bignum()
                                        .zip(right.value_bignum())
                                        .is_some_and(|(left, right)| left == right)
                                }
                                _ => false,
                            }
                        });
                        if duplicate {
                            continue;
                        }
                        patterns.push(pattern);
                        if let ExpressionNode::Boolean(value) = node {
                            booleans[usize::from(*value)] = true;
                            covered = booleans == [true, true];
                        }
                    }
                }
                let handle = arms.handle_at(ordinal as u32);
                selected.push((handle, arm));
                if covered {
                    break;
                }
            }
            selected
        }
    }
    mod routes {
        /*
            Source arm routes shared by selected-storage claim compatibility.
        */
        use crate::checks::ownership_occurrences::EvaluationBranch;
        use arena::Handle;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableMatchArm};

        /*
            The same first-matching-arm schedule as ownership occurrence evaluation.
            The final reachable arm is the remaining edge, not another binary choice.
        */
        pub(in crate::checks::multiplicity) fn terminal_arm_route(
            program: &dyn validation::CheckingInput,
            expression: ExpressionHandle,
            terminal: Handle<TableMatchArm>,
        ) -> Option<Vec<EvaluationBranch>> {
            let ExpressionNode::Match(dispatch) = program.expression_table.expression(expression)
            else {
                return None;
            };
            let arms = super::operands::reachable_arms(program, dispatch.arms);
            let mut prefix = Vec::new();
            for (decision, (handle, arm)) in arms.iter().enumerate() {
                let mut route = prefix.clone();
                if decision + 1 < arms.len() {
                    route.push(EvaluationBranch::new(expression, decision, true));
                }
                if *handle == terminal {
                    return Some(route);
                }
                if let Some(nested) = terminal_arm_route(program, arm.value, terminal) {
                    route.extend(nested);
                    return Some(route);
                }
                prefix.push(EvaluationBranch::new(expression, decision, false));
            }
            None
        }
    }
    pub(super) use occurrences::SelectionOccurrences;
    pub(in crate::checks::multiplicity) use routes::terminal_arm_route;

    /*
        Exact descendant claims and the affine containing home share source
        availability. A projected transfer makes its containing value unavailable
        for whole use or ordinary cleanup; it does not transfer an ancestor claim.
    */
    fn source_frontier_matches(
        place: &super::linear_obligations::LinearPlace,
        root: facts::PlaceRoot,
        path: &[facts::PlaceSegment],
    ) -> bool {
        place.root == root
            && (place.path.starts_with(path)
                || (place.multiplicity == language_semantics::Multiplicity::Affine
                    && path.starts_with(&place.path)))
    }

    /*
        Two canonical paths overlap when one reaches inside the other or both
        name the same place; a dynamic index cannot be statically excluded from a
        fixed element claim, so it conflicts on the element axis.
    */
    pub(super) fn place_paths_overlap(
        used: &[facts::PlaceSegment],
        consumed: &[facts::PlaceSegment],
    ) -> bool {
        used.iter().zip(consumed.iter()).all(|(used, consumed)| {
            used == consumed
                || (matches!(used, facts::PlaceSegment::Index { .. })
                    && matches!(
                        consumed,
                        facts::PlaceSegment::FixedIndex { .. }
                            | facts::PlaceSegment::Index { .. }
                            | facts::PlaceSegment::FixedRange { .. }
                    ))
        })
    }

    /*
        Before a named source moves, every previously completed receiver retaining
        it must belong to its actual published loan occurrence. This is attribution,
        not a second availability walk: ordered borrow-move checking owns live-loan
        exclusion at the exact source path and operand cut, including projections.
    */
    fn source_outputs_are_attributed(
        program: &dyn validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        statement: usize,
        source: symbols::SymbolHandle,
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> bool {
        let Some(machine) = program.machine_by_symbol(machine) else {
            return false;
        };
        let Some(state) = program.state_in_machine(machine, state) else {
            return false;
        };
        let Some(flow) = facts
            .flow
            .control
            .unique_state(machine.symbol, state.symbol)
        else {
            return false;
        };
        let order = program
            .statement_table
            .admitted_execution_order(state.statement_nodes);
        let Some(position) = order.position(statement) else {
            return false;
        };
        for &index in &order.indices()[..position] {
            let Some(statement) = program.statement_table.statement_at(state, index) else {
                return false;
            };
            if !crate::checks::borrows::retained_outputs_are_attributed(
                program, facts, machine, flow, index, statement, source, frames,
            ) {
                return false;
            }
        }
        true
    }
}
mod owned_transfer_paths {
    /*
        Where each live owned linear transfer lands within the operand that
        consumed it.

        Lowering binds a consuming operand to the claims it hands over. That needs
        the operand's owning root, the operand's storage path under that root, and
        each claim's path relative to that storage. All three are queries over the
        immutable typed program and the final permission ledger, so checking answers
        them once, after the flow fixed point seals the ledger, and publishes one
        placement per event. The queries deliberately use the bare typed program,
        without checking memos, deferred layout geometry or opaque selections:
        for retained machines it answers exactly as the checked program's `typed`
        field does, so a placement never depends on checking-only context. A
        placement names storage only: the event keeps the claim identity,
        evaluation lane and consuming coordinate.
    */
    use checked_trees::{
        CheckFacts, FlowOwnedTransferPath, FlowOwnedTransferPlacement, FlowPermissionEventFact,
    };
    use language_semantics::{
        Multiplicity, PermissionAccess, PermissionEventKind, PermissionEventSource,
    };

    type Arena<T> = arena::PagedArenaSet<T>;

    pub(crate) fn publish_owned_transfer_paths(
        program: &dyn ::validation::CheckingInput,
        facts: &mut CheckFacts,
    ) {
        let typed: &dyn ::validation::CheckingInput = program.typed_program();
        let ownership = &mut facts.flow.ownership;
        let mut paths = Arena::default();
        let mut rows = Arena::default();
        for (event_handle, event) in ownership.permissions.iter() {
            if !event.expression.is_valid()
                || event.kind != PermissionEventKind::Transfer
                || event.access != PermissionAccess::Owned
                || event.multiplicity != Multiplicity::Linear
                || !event.obligation_live
            {
                continue;
            }
            let placement = match event.source {
                PermissionEventSource::Statement { statement_index }
                | PermissionEventSource::Call {
                    statement_index, ..
                }
                | PermissionEventSource::InstructionDisposition { statement_index }
                | PermissionEventSource::InstructionExitDisposition { statement_index } => {
                    transfer_placement(
                        typed,
                        event,
                        statement_index,
                        ownership.segments.span_or_empty(event.segments),
                        &mut paths,
                    )
                }
                _ => FlowOwnedTransferPlacement::Unresolved,
            };
            rows.append(FlowOwnedTransferPath {
                event: event_handle,
                placement,
            });
        }
        ownership.owned_transfer_paths = rows;
        ownership.owned_transfer_path_segments = paths;
    }

    fn transfer_placement(
        program: &dyn ::validation::CheckingInput,
        event: &FlowPermissionEventFact,
        statement: usize,
        claim: &[facts::PlaceSegment],
        paths: &mut Arena<checked_trees::CheckedUnitStructuralPathSegment>,
    ) -> FlowOwnedTransferPlacement {
        let state = event.state_symbol;
        let Some(place) = ::validation::ownership_place_from_expression_in_state(
            program,
            state,
            statement,
            event.expression,
        ) else {
            return FlowOwnedTransferPlacement::Unresolved;
        };
        if place.root != event.root {
            return FlowOwnedTransferPlacement::OtherRoot;
        }
        let Some(root_type) = ::validation::canonical_place_type_reference(
            program,
            state,
            statement,
            &::validation::CanonicalPlace {
                root: place.root,
                segments: Vec::new(),
            },
        ) else {
            return FlowOwnedTransferPlacement::Unresolved;
        };
        let Ok((storage, _)) =
            ::validation::structural_transfer_storage_path(program, root_type, &place.segments)
        else {
            return FlowOwnedTransferPlacement::Unresolved;
        };
        let Ok(path) = ::validation::structural_claim_path(program, root_type, claim) else {
            return FlowOwnedTransferPlacement::Unresolved;
        };
        match path.strip_prefix(storage.as_slice()) {
            Some(relative) => {
                FlowOwnedTransferPlacement::Within(paths.insert_many(relative.iter().cloned()))
            }
            None => FlowOwnedTransferPlacement::Outside,
        }
    }
}
pub(super) use owned_selection::reachable_arms;
pub(super) use projected_affine::transfers_owned_storage;
mod instruction_dispositions {
    /*
        Terminal dispositions reuse the ordinary claim ledger. A consuming boundary
        requirement must be authored by the resource owner, the proposition must
        name one exact live owned argument, and an unspent modeled instruction must
        establish it at this point. Logical truth alone grants no terminal disposal.
    */
    use super::linear_obligations::LinearPlace;
    use super::linear_validation::exit_evaluation;
    use super::linear_validation::linear_claim_frontier;
    use crate::flow::expressions;
    use crate::published::places::place_at;
    use checked_trees::{CheckFacts, FlowPermissionEvaluation, FlowPermissionEventFact};
    use language_semantics::{
        Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
        PermissionEventSource, PermissionProvenance,
    };
    use symbols::SymbolHandle;
    use typed_trees::domain::ProofFact;
    use typed_trees::statement::{AssemblyFactKind, StatementNode};
    use typed_trees::types::TypeReferenceHandle;

    #[derive(Clone, Default)]
    pub(super) struct InstructionDispositions {
        spent: Vec<crate::checks::contracts::prover::InstructionEffectEvidence>,
    }
    impl InstructionDispositions {
        pub(super) fn statement(
            &mut self,
            program: &dyn validation::CheckingInput,
            facts: &mut checked_trees::CheckFactsWalk<'_>,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            statement: usize,
            evaluation: FlowPermissionEvaluation,
            places: &mut [LinearPlace],
            events: &mut Vec<FlowPermissionEventFact>,
        ) {
            if evaluation != FlowPermissionEvaluation::Statement {
                return;
            }
            let Some(StatementNode::AssemblyFact(assertion)) =
                crate::flow::state_statements(program, state.statement_nodes).get(statement)
            else {
                return;
            };
            if assertion.kind != AssemblyFactKind::Ensures {
                return;
            }
            let Some(flow) = facts.flow.control.state(machine.symbol, state.symbol) else {
                return;
            };
            let Some(row) = facts.flow.view().state_statement(flow, statement) else {
                return;
            };
            let contexts = facts
                .flow
                .view()
                .semantic_constraint_contexts(row.entry_constraints)
                .collect::<Vec<_>>();
            self.settle(
                program,
                facts,
                machine,
                state,
                statement,
                PermissionEventSource::InstructionDisposition {
                    statement_index: statement,
                },
                evaluation,
                assertion.fact,
                &contexts,
                places,
                events,
            );
        }
        pub(super) fn exit(
            &mut self,
            program: &dyn validation::CheckingInput,
            facts: &mut checked_trees::CheckFactsWalk<'_>,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            coordinate: checked_trees::FlowClaimJoinExit,
            places: &mut [LinearPlace],
            events: &mut Vec<FlowPermissionEventFact>,
        ) {
            if !places.iter().any(|held| {
                held.live && held.ever_established && held.multiplicity == Multiplicity::Linear
            }) {
                return;
            }
            let Some(state_flow) = facts.flow.control.state(machine.symbol, state.symbol) else {
                return;
            };
            let evaluation = exit_evaluation(program, state, coordinate);
            let target = match evaluation {
                FlowPermissionEvaluation::TransitionTarget(target) => target,
                _ => Default::default(),
            };
            let exits = facts
                .flow
                .control
                .exits
                .span_or_empty(state_flow.exits)
                .iter()
                .filter_map(|exit| {
                    (exit.machine_symbol == machine.symbol
                        && exit.state_symbol == state.symbol
                        && exit.statement_index == coordinate.statement_index
                        && exit.transition_target == target)
                        .then_some(exit.clone())
                })
                .collect::<Vec<_>>();
            for exit in exits {
                let contexts = facts
                    .flow
                    .view()
                    .semantic_constraint_contexts(exit.entry_constraints)
                    .collect::<Vec<_>>();
                let mut contracts = Vec::new();
                for context in facts
                    .flow
                    .view()
                    .semantic_constraint_contexts(exit.ensures_constraints)
                {
                    for fact in facts
                        .semantic
                        .context_view(facts.semantic.contexts.get(context))
                        .facts()
                    {
                        let source = match fact.payload {
                            facts::FactPayload::PropositionApplication { fact, .. }
                            | facts::FactPayload::ContractPropositionApplication { fact, .. } => {
                                fact
                            }
                            _ => continue,
                        };
                        if !contracts.contains(&source) {
                            contracts.push(source);
                        }
                    }
                }
                for contract in contracts {
                    self.settle(
                        program,
                        facts,
                        machine,
                        state,
                        coordinate.statement_index,
                        PermissionEventSource::InstructionExitDisposition {
                            statement_index: coordinate.statement_index,
                        },
                        evaluation,
                        contract,
                        &contexts,
                        places,
                        events,
                    );
                }
            }
        }
        fn settle(
            &mut self,
            program: &dyn validation::CheckingInput,
            facts: &mut checked_trees::CheckFactsWalk<'_>,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            statement: usize,
            source: PermissionEventSource,
            evaluation: FlowPermissionEvaluation,
            fact: arena::Handle<ProofFact>,
            contexts: &[facts::FactContextHandle],
            places: &mut [LinearPlace],
            events: &mut Vec<FlowPermissionEventFact>,
        ) {
            let Some(application) = program.proof_facts.get(fact).as_proposition() else {
                return;
            };
            if validation::instruction_effect_contract(program, application).is_none() {
                return;
            }
            let mut candidates = Vec::new();
            for argument in expressions(program).expression_handles(application.arguments) {
                let Some(place) = place_at(program, state.symbol, statement, *argument) else {
                    continue;
                };
                let Some(position) = places.iter().position(|held| {
                    held.live
                        && held.ever_established
                        && held.multiplicity == Multiplicity::Linear
                        && held.root == place.root
                        && held.path == place.segments
                        && held
                            .claim_identity
                            .is_some_and(|identity| identity != PermissionClaimIdentity::Unknown)
                        && held
                            .provenance
                            .is_some_and(|origin| origin != PermissionProvenance::Unknown)
                }) else {
                    continue;
                };
                if candidates.iter().all(|(other, _)| *other != position) {
                    candidates.push((position, *argument));
                }
            }
            let [(position, argument)] = candidates.as_slice() else {
                return;
            };
            let position = *position;
            let argument = *argument;
            let Some(reference) = validation::canonical_place_type_reference(
                program,
                state.symbol,
                statement,
                &validation::CanonicalPlace {
                    root: places[position].root,
                    segments: places[position].path.clone(),
                },
            ) else {
                return;
            };
            let Some(requirement) = consuming_requirement(program, machine, reference) else {
                return;
            };
            let Some(mut event) = crate::checks::contracts::prover::instruction_effect_evidence(
                program,
                facts.view(),
                state.symbol,
                statement,
                contexts,
                application,
                &self.spent,
            ) else {
                return;
            };
            let held = &mut places[position];
            let segments = facts
                .flow
                .ownership
                .segments
                .insert_many(held.path.iter().copied());
            if matches!(
                source,
                PermissionEventSource::InstructionExitDisposition { .. }
            ) {
                event.observe_at(facts::ProgramPoint::Exit {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                    statement_index: statement,
                    transition_target: match evaluation {
                        FlowPermissionEvaluation::TransitionTarget(target) => target,
                        _ => Default::default(),
                    },
                });
            }
            let evidence = event.publish(&mut facts.flow.ownership.instruction_effect_receipts);
            facts.flow.ownership.instruction_dispositions.append(
                checked_trees::flow::FlowInstructionDispositionFact {
                    machine: machine.symbol,
                    state: state.symbol,
                    source,
                    evaluation,
                    requirement,
                    fact,
                    argument,
                    event: event.event,
                    evidence,
                    root: held.root,
                    segments,
                },
            );
            events.push(FlowPermissionEventFact {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
                source,
                evaluation,
                expression: Default::default(),
                kind: PermissionEventKind::Consume,
                multiplicity: held.multiplicity,
                access: PermissionAccess::Owned,
                claim_identity: held
                    .claim_identity
                    .expect("a selected live claim has identity"),
                provenance: held
                    .provenance
                    .expect("a selected live claim has provenance"),
                root: held.root,
                segments,
                obligation_live: true,
            });
            held.live = false;
            self.spent.push(event);
        }
    }

    fn consuming_requirement(
        program: &dyn validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        reference: TypeReferenceHandle,
    ) -> Option<SymbolHandle> {
        if super::type_reference_is_reference(program, reference) {
            return None;
        }
        let reference = validation::peel_declared_constraints(program, reference);
        if !matches!(
            program.type_reference_table.type_reference(reference),
            typed_trees::types::TypeReferenceNode::Named { .. }
                | typed_trees::types::TypeReferenceNode::Generic { .. }
        ) {
            return None;
        }
        let resource =
            program.data_definition_by_symbol(program.type_reference_symbol(reference))?;
        let resource_identity = program.package_qualified_type_identity(reference);
        let entry = program.machine_states(machine).first()?;
        let actual = program.state_parameters(entry);
        let mut declarations = Vec::new();
        for conformance in program.machine_trait_conformances(machine) {
            if let Some(typed_trees::machine::SatisfiedDeclaration::Trait {
                definition,
                requirement,
            }) =
                typed_trees::machine::resolve_satisfied_declaration(program, machine, conformance)
            {
                declarations.push((definition, requirement));
            }
        }
        for conformance in program.conformances() {
            for row in &conformance.rows {
                if row.realization_machine != machine.symbol {
                    continue;
                }
                if let Some(definition) = program.trait_definition_by_symbol(row.declaring_trait)
                    && let Some(requirement) =
                        program.trait_machine_signature_by_symbol(definition, row.requirement)
                {
                    declarations.push((definition, requirement));
                }
            }
        }
        declarations
            .into_iter()
            .find_map(|(definition, requirement)| {
                if !definition.is_boundary
                    || !program
                        .symbols
                        .same_symbol_source_package(definition.symbol, resource.symbol)
                {
                    return None;
                }
                let formal = program.state_signature_parameters(requirement);
                if formal.len() != actual.len() {
                    return None;
                }
                formal
                    .iter()
                    .zip(actual)
                    .any(|(formal, actual)| {
                        !super::type_reference_is_reference(program, formal.type_reference)
                            && !super::type_reference_is_reference(program, actual.type_reference)
                            && linear_claim_frontier(program, actual.type_reference)
                                .iter()
                                .any(|claim| {
                                    program.package_qualified_type_identity(
                                        validation::peel_declared_constraints(
                                            program,
                                            claim.type_reference,
                                        ),
                                    ) == resource_identity
                                })
                    })
                    .then_some(requirement.symbol)
            })
    }

    /*
        Bind the published permission occurrence to the retained proof receipt.
        Proof search happens once in production; replay verifies custody identities
        and this source association instead of reconstructing the instruction CFG.
    */
    pub(super) fn valid_receipt(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        event: &FlowPermissionEventFact,
    ) -> bool {
        let statement_index = match event.source {
            PermissionEventSource::InstructionDisposition { statement_index }
            | PermissionEventSource::InstructionExitDisposition { statement_index } => {
                statement_index
            }
            _ => return false,
        };
        let mut receipts = facts
            .flow
            .ownership
            .instruction_dispositions
            .iter()
            .filter_map(|(_, receipt)| {
                (receipt.machine == machine.symbol
                    && receipt.state == state.symbol
                    && receipt.source == event.source
                    && receipt.evaluation == event.evaluation
                    && receipt.root == event.root
                    && facts
                        .flow
                        .ownership
                        .segments
                        .span_or_empty(receipt.segments)
                        == facts.flow.ownership.segments.span_or_empty(event.segments))
                .then_some(receipt)
            });
        let Some(receipt) = receipts.next() else {
            return false;
        };
        let observation = match event.source {
            PermissionEventSource::InstructionDisposition { .. } => {
                facts::ProgramPoint::Statement {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                    statement_index,
                }
            }
            PermissionEventSource::InstructionExitDisposition { .. } => facts::ProgramPoint::Exit {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
                statement_index,
                transition_target: match event.evaluation {
                    FlowPermissionEvaluation::TransitionTarget(target) => target,
                    _ => Default::default(),
                },
            },
            _ => return false,
        };
        if !facts
            .flow
            .ownership
            .instruction_effect_receipts
            .is_valid(receipt.evidence)
            || facts
                .flow
                .ownership
                .instruction_effect_receipts
                .get(receipt.evidence)
                .observation
                != observation
        {
            return false;
        }
        if receipts.next().is_some()
            || event.kind != PermissionEventKind::Consume
            || event.access != PermissionAccess::Owned
            || !event.obligation_live
            || !facts.semantic.facts.is_valid(receipt.event)
        {
            return false;
        }
        match event.source {
            PermissionEventSource::InstructionDisposition { .. } => {
                let Some(StatementNode::AssemblyFact(assertion)) = program.statement_table.statements(state.statement_nodes).get(statement_index)
                else { return false; };
                if assertion.kind != AssemblyFactKind::Ensures || assertion.fact != receipt.fact { return false; }
            }
            PermissionEventSource::InstructionExitDisposition { .. } => {
                if !facts.flow.control.exits.iter().any(|(_, exit)| {
                    exit.machine_symbol == machine.symbol && exit.state_symbol == state.symbol
                        && exit.statement_index == statement_index
                        && facts.flow.semantic_constraint_contexts(exit.ensures_constraints).any(|context| {
                            facts.semantic.context_view(facts.semantic.contexts.get(context)).facts().any(|fact| {
                                matches!(fact.payload, facts::FactPayload::PropositionApplication { fact, .. }
                                    | facts::FactPayload::ContractPropositionApplication { fact, .. } if fact == receipt.fact)
                            })
                        })
                }) { return false; }
            }
            _ => return false,
        }
        let Some(application) = program.proof_facts.get(receipt.fact).as_proposition() else {
            return false;
        };
        let Some(contract) = validation::instruction_effect_contract(program, application) else {
            return false;
        };
        if !valid_effect_tree(
            program,
            facts,
            receipt.evidence,
            receipt.event,
            contract.operation,
        ) || !expressions(program)
            .expression_handles(application.arguments)
            .contains(&receipt.argument)
        {
            return false;
        }
        let Some(place) = place_at(program, state.symbol, statement_index, receipt.argument) else {
            return false;
        };
        place.root == receipt.root
            && place.segments
                == facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(receipt.segments)
            && validation::canonical_place_type_reference(
                program,
                state.symbol,
                statement_index,
                &place,
            )
            .and_then(|reference| consuming_requirement(program, machine, reference))
                == Some(receipt.requirement)
    }

    fn valid_effect_tree(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        handle: arena::Handle<checked_trees::flow::FlowInstructionEffectReceiptFact>,
        event: facts::FactHandle,
        operation: facts::InstructionEffectOperation,
    ) -> bool {
        let arena = &facts.flow.ownership.instruction_effect_receipts;
        if !arena.is_valid(handle) {
            return false;
        }
        let root = arena.get(handle);
        if root.event != event {
            return false;
        }
        let mut pending = vec![handle];
        while let Some(handle) = pending.pop() {
            let receipt = arena.get(handle);
            if !facts.semantic.facts.is_valid(receipt.event) {
                return false;
            }
            let effect = facts.semantic.facts.get(receipt.event);
            if !effect_visible_at(facts, receipt.observation, receipt.event) {
                return false;
            }
            if operation == facts::InstructionEffectOperation::AtomicReleaseStore
                && matches!(
                    effect.payload,
                    facts::FactPayload::InstructionEffect {
                        operation: facts::InstructionEffectOperation::AtomicReleaseStore
                    }
                )
            {
                let facts::ProgramPoint::Statement {
                    machine_symbol,
                    state_symbol,
                    statement_index,
                } = effect.point
                else {
                    return false;
                };
                let Some(machine) = program.machine_by_symbol(machine_symbol) else {
                    return false;
                };
                let Some(state) = program.state_by_symbol(state_symbol) else {
                    return false;
                };
                let Some(statement) = crate::flow::state_statements(program, state.statement_nodes)
                    .get(statement_index)
                else {
                    return false;
                };
                if crate::flow::atomic_effects::release_store(program, machine, state, statement)
                    .is_none()
                    || !arena
                        .span(receipt.children)
                        .is_some_and(|children| children.is_empty())
                {
                    return false;
                }
                continue;
            }
            let facts::ProgramPoint::CallEnsures {
                machine_symbol,
                state_symbol,
                statement_index,
                call_ordinal,
            } = effect.point
            else {
                return false;
            };
            let Some(flow) = facts.flow.control.state(machine_symbol, state_symbol) else {
                return false;
            };
            let Some(call) = crate::flow::call_flows(&facts.flow.control, flow.calls)
                .iter()
                .find(|call| {
                    call.statement_index == statement_index && call.call_ordinal == call_ordinal
                })
            else {
                return false;
            };
            let Some(children) = arena.span(receipt.children) else {
                return false;
            };
            if children.is_empty() {
                if !matches!(effect.payload, facts::FactPayload::InstructionEffect { operation: recorded } if recorded == operation)
                    || program
                        .symbols
                        .builtin_function_for_symbol(call.target_symbol)
                        .and_then(|builtin| {
                            language_core::inline_assembly::AsmRegisterOperation::from_intrinsic_name(
                                builtin.name(),
                            )
                        })
                        .map(facts::InstructionEffectOperation::Register)
                        != Some(operation)
                {
                    return false;
                }
            } else {
                if !matches!(effect.payload, facts::FactPayload::CallCompletion { target } if target == call.target_symbol)
                    || !facts.proof.contract_facts.is_valid(receipt.contract)
                {
                    return false;
                }
                let guarantee = facts.proof.contract_facts.get(receipt.contract);
                if guarantee.kind != checked_trees::ContractProofFactKind::Ensures
                    || !facts
                        .proof
                        .contract_fact_refs
                        .span_or_empty(call.ensures)
                        .iter()
                        .any(|reference| reference.fact == receipt.contract)
                {
                    return false;
                }
                let Some(application) = program.proof_facts.get(guarantee.fact).as_proposition()
                else {
                    return false;
                };
                if validation::instruction_effect_contract(program, application)
                    .is_none_or(|contract| contract.operation != operation)
                {
                    return false;
                }
                let Some(callee) = facts
                    .flow
                    .control
                    .states
                    .iter()
                    .map(|(_, state)| state)
                    .find(|state| state.state_symbol == call.target_symbol)
                else {
                    return false;
                };
                let exits = facts
                    .flow
                    .control
                    .exits
                    .iter()
                    .filter_map(|(_, exit)| {
                        (exit.machine_symbol == callee.machine_symbol).then_some(exit)
                    })
                    .collect::<Vec<_>>();
                if exits.len() != children.len()
                    || !exits.iter().all(|exit| {
                        let observation = facts::ProgramPoint::Exit {
                            machine_symbol: exit.machine_symbol,
                            state_symbol: exit.state_symbol,
                            statement_index: exit.statement_index,
                            transition_target: exit.transition_target,
                        };
                        children
                            .iter()
                            .filter(|child| child.observation == observation)
                            .count()
                            == 1
                    })
                {
                    return false;
                }
                for child in receipt.children.handles() {
                    if child.arena_index() >= handle.arena_index() {
                        return false;
                    }
                    pending.push(child);
                }
            }
        }
        true
    }

    /*
        Visibility is bound to the producer's actual statement or selected exit.
        Replaying a checked receipt does not repeat scalar proof search.
    */
    fn effect_visible_at(
        facts: &CheckFacts,
        point: facts::ProgramPoint,
        event: facts::FactHandle,
    ) -> bool {
        let constraints = match point {
            facts::ProgramPoint::Statement {
                machine_symbol,
                state_symbol,
                statement_index,
            } => {
                let Some(state) = facts.flow.control.state(machine_symbol, state_symbol) else {
                    return false;
                };
                let Some(statement) = facts.flow.state_statement(state, statement_index) else {
                    return false;
                };
                statement.entry_constraints
            }
            facts::ProgramPoint::Exit {
                machine_symbol,
                state_symbol,
                statement_index,
                transition_target,
            } => {
                let Some(exit) =
                    facts
                        .flow
                        .control
                        .exits
                        .iter()
                        .map(|(_, exit)| exit)
                        .find(|exit| {
                            exit.machine_symbol == machine_symbol
                                && exit.state_symbol == state_symbol
                                && exit.statement_index == statement_index
                                && exit.transition_target == transition_target
                        })
                else {
                    return false;
                };
                exit.entry_constraints
            }
            _ => return false,
        };
        facts
            .flow
            .semantic_constraint_contexts(constraints)
            .any(|context| {
                facts
                    .semantic
                    .refs
                    .span_or_empty(facts.semantic.contexts.get(context).facts)
                    .iter()
                    .any(|reference| reference.fact == event)
            })
    }
}
#[path = "checks_multiplicity_permission_events.rs"]
mod permission_events;
mod permission_schedule {
    /*
        One source evaluation schedule for permission production and replay.
        Custody forks after guard evaluation, and only the missed guard state
        continues to the next statement. No expression traversal is duplicated.
    */
    use super::cleanup_dispositions::RemainingCustody;
    use super::linear_obligations::LinearPlace;
    use super::permission_events::case_transition_run_is_exhaustive;
    use crate::flow::CaseEvidence;
    use crate::flow::call_permission_evaluation;
    use checked_trees::{FlowClaimJoinExit, FlowClaimJoinExitKind, FlowPermissionEvaluation};
    use std::sync::Arc;
    use typed_trees::statement::{StatementNode, TransitionExit, TransitionGuardNode};

    #[derive(Clone, Copy)]
    pub(super) enum PermissionStep {
        Evaluate {
            statement: usize,
            evaluation: FlowPermissionEvaluation,
        },
        MissedGuard,
        Exit {
            coordinate: FlowClaimJoinExit,
            crash: bool,
        },
    }

    pub(super) fn walk<S: Clone>(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        calls: &[checked_trees::FlowCallFact],
        mut custody: S,
        mut visit: impl FnMut(PermissionStep, &mut S),
    ) {
        let statements = program.statement_table.statements(state.statement_nodes);
        let mut transitions = Vec::new();
        for (statement, node) in program
            .statement_table
            .admitted_execution_statements(state.statement_nodes)
        {
            let Some(transition) = node.as_transition() else {
                visit(
                    PermissionStep::Evaluate {
                        statement,
                        evaluation: FlowPermissionEvaluation::Statement,
                    },
                    &mut custody,
                );
                if evaluation_has_no_normal_completion(
                    program,
                    state,
                    calls,
                    statement,
                    FlowPermissionEvaluation::Statement,
                ) {
                    return;
                }
                continue;
            };
            transitions.push(statement);
            visit(
                PermissionStep::Evaluate {
                    statement,
                    evaluation: FlowPermissionEvaluation::TransitionGuard,
                },
                &mut custody,
            );
            if evaluation_has_no_normal_completion(
                program,
                state,
                calls,
                statement,
                FlowPermissionEvaluation::TransitionGuard,
            ) {
                return;
            }
            for (target, kind) in [
                (transition.target, FlowClaimJoinExitKind::TransitionTarget),
                (
                    transition.continuation,
                    FlowClaimJoinExitKind::TransitionContinuation,
                ),
            ] {
                if !target.is_valid() {
                    continue;
                }
                let mut branch = custody.clone();
                visit(
                    PermissionStep::Evaluate {
                        statement,
                        evaluation: FlowPermissionEvaluation::TransitionTarget(target),
                    },
                    &mut branch,
                );
                if target_has_no_normal_completion(program, state, target) {
                    continue;
                }
                visit(
                    PermissionStep::Exit {
                        coordinate: FlowClaimJoinExit {
                            state_symbol: state.symbol,
                            statement_index: statement,
                            kind,
                        },
                        crash: matches!(transition.exit, TransitionExit::Crash(_)),
                    },
                    &mut branch,
                );
            }
            if transition.continuation.is_valid()
                || matches!(transition.guard, TransitionGuardNode::Always)
            {
                return;
            }
            visit(PermissionStep::MissedGuard, &mut custody);
        }
        if !transitions.is_empty()
            && case_transition_run_is_exhaustive(program, state.symbol, statements, &transitions)
        {
            return;
        }
        visit(
            PermissionStep::Exit {
                coordinate: FlowClaimJoinExit {
                    state_symbol: state.symbol,
                    statement_index: if matches!(
                        statements.last(),
                        Some(StatementNode::Expression(_))
                    ) {
                        statements.len() - 1
                    } else {
                        statements.len()
                    },
                    kind: FlowClaimJoinExitKind::FinalExpression,
                },
                crash: false,
            },
            &mut custody,
        );
    }

    /*
        An invocation without normal completion has no normal caller successor. Its
        argument evaluation still runs above, but neither cleanup nor a fabricated
        consumption event may discharge the remaining frontier. Use the producer's
        unconditional call occurrences: a call in only one selected expression arm
        cannot erase the other arm's normal-exit obligations.
    */
    fn evaluation_has_no_normal_completion(
        program: &dyn validation::CheckingInput,
        state: &typed_trees::state::State,
        calls: &[checked_trees::FlowCallFact],
        statement: usize,
        evaluation: FlowPermissionEvaluation,
    ) -> bool {
        let Some(machine) = program.machine_holding_state(state.symbol) else {
            return false;
        };
        calls.iter().any(|call| {
            call.statement_index == statement
                && !call.authored_selected
                && call.authored_dispatch_role
                    != checked_trees::FlowCallDispatchRole::SavedSubjectRead
                && call_permission_evaluation(program, machine, state, statement, call.call_ordinal)
                    == evaluation
                && call_has_no_normal_completion(program, call.target_symbol)
        })
    }

    fn call_has_no_normal_completion(
        program: &dyn validation::CheckingInput,
        target: symbols::SymbolHandle,
    ) -> bool {
        matches!(
            validation::boundary_requirement_completion(program, target),
            semantic_vocabulary::BoundaryCompletion::External(_)
        ) || validation::call_target_state(program, target).is_some_and(|state| {
            validation::normal_completion_possible(program, state) == Some(false)
        })
    }

    fn target_has_no_normal_completion(
        program: &dyn validation::CheckingInput,
        state: &typed_trees::state::State,
        target: typed_trees::statement::TransitionTargetHandle,
    ) -> bool {
        match program.statement_table.transition_target(target) {
            typed_trees::statement::TransitionTargetNode::Named { path, .. } => {
                let Some(machine) = program.machine_holding_state(state.symbol) else {
                    return false;
                };
                validation::terminal_callable_target(program, machine, path.symbol).is_some()
                    && call_has_no_normal_completion(program, path.symbol)
            }
            typed_trees::statement::TransitionTargetNode::Value(value) => {
                validation::expression_normal_completion_possible(program, *value) == Some(false)
            }
            typed_trees::statement::TransitionTargetNode::Terminal => false,
        }
    }

    /*
        Case exclusions at one evaluation cut. Flow already authenticated the saved
        guard receiver, selected polarity and intervening writes. Custody consumes
        those exact observations instead of resolving the guard's descriptor again.
    */
    pub(super) struct SelectedCases<'program> {
        evidence: CaseEvidence<'program>,
    }

    impl<'program> SelectedCases<'program> {
        pub(super) fn at_step(
            program: &'program dyn validation::CheckingInput,
            facts: &'program checked_trees::CheckFacts,
            flow: &checked_trees::FlowStateFact,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            step: PermissionStep,
        ) -> Option<Self> {
            let (statement, constraints) = match step {
                PermissionStep::Evaluate {
                    statement,
                    evaluation:
                        FlowPermissionEvaluation::Statement | FlowPermissionEvaluation::TransitionGuard,
                } => {
                    let mut rows = facts
                        .flow
                        .control
                        .statements
                        .span(flow.statements)?
                        .iter()
                        .filter(|row| row.statement_index == statement);
                    let row = rows.next()?;
                    if rows.next().is_some() {
                        return None;
                    }
                    (statement, row.entry_constraints)
                }
                PermissionStep::Evaluate {
                    statement,
                    evaluation: FlowPermissionEvaluation::TransitionTarget(target),
                } => {
                    let mut rows = facts
                        .flow
                        .control
                        .outgoing
                        .span(flow.outgoing)?
                        .iter()
                        .filter(|row| {
                            row.statement_index == statement
                                && row.transition_target == target
                                && row.machine_symbol == machine.symbol
                                && row.state_symbol == state.symbol
                        });
                    let row = rows.next()?;
                    if rows.next().is_some() {
                        return None;
                    }
                    (statement, row.operand_entry_constraints)
                }
                PermissionStep::Exit { coordinate, .. }
                    if coordinate.kind == FlowClaimJoinExitKind::FinalExpression =>
                {
                    let mut rows = facts
                        .flow
                        .control
                        .outgoing
                        .span(flow.outgoing)?
                        .iter()
                        .filter(|row| {
                            row.statement_index == coordinate.statement_index
                                && !row.transition_target.is_valid()
                                && row.machine_symbol == machine.symbol
                                && row.state_symbol == state.symbol
                        });
                    let row = rows.next()?;
                    if rows.next().is_some() {
                        return None;
                    }
                    (coordinate.statement_index, row.constraints)
                }
                _ => return None,
            };
            if constraints.is_empty() {
                return None;
            }
            let contexts = facts
                .flow
                .semantic_constraint_contexts(constraints)
                .collect::<Vec<_>>();
            if contexts.is_empty() {
                return None;
            }
            let evidence = CaseEvidence::prepare(
                program,
                &facts.semantic,
                &contexts,
                machine.symbol,
                state.symbol,
                statement,
            )?;
            Some(Self { evidence })
        }

        pub(super) fn apply_claims(self, places: &mut [LinearPlace]) {
            self.apply(places, None);
        }

        pub(super) fn apply(
            self,
            places: &mut [LinearPlace],
            mut remaining: Option<&mut RemainingCustody>,
        ) {
            if places.is_empty() && remaining.is_none() {
                return;
            }
            self.evidence.for_each_inactive_owned_case(|excluded| {
                exclude_case(excluded, places, remaining.as_deref_mut());
            });
        }

        /*
            This step's exclusions, carried forward from the previous step's list.
            The evidence reads no custody, so the parallel preparation of the
            permission walk computes it ahead of the sequential custody replay that
            applies it. Consecutive steps of a large state usually exclude the same
            cases, so a list equal to the `carried` one is shared instead of copying
            every place again; only a step that changes the list allocates a new
            one, copying the unchanged prefix and then its own exclusions.
        */
        pub(super) fn inactive_owned_cases_after(
            self,
            carried: Option<&Arc<[validation::CanonicalPlace]>>,
        ) -> Arc<[validation::CanonicalPlace]> {
            let previous = carried.map_or(&[][..], |previous| &previous[..]);
            let mut matched = 0;
            let mut changed: Option<Vec<validation::CanonicalPlace>> = None;
            self.evidence.for_each_inactive_owned_case(|excluded| {
                if let Some(cases) = &mut changed {
                    cases.push(excluded.clone());
                } else if previous.get(matched) == Some(excluded) {
                    matched += 1;
                } else {
                    let mut cases = Vec::with_capacity(previous.len().max(matched + 1));
                    cases.extend_from_slice(&previous[..matched]);
                    cases.push(excluded.clone());
                    changed = Some(cases);
                }
            });
            match (changed, carried) {
                (Some(cases), _) => cases.into(),
                (None, Some(carried)) if matched == carried.len() => Arc::clone(carried),
                (None, _) => previous[..matched].into(),
            }
        }
    }

    /* Retire one inactive owned case namespace from claim and cleanup custody. */
    pub(super) fn exclude_case(
        excluded: &validation::CanonicalPlace,
        places: &mut [LinearPlace],
        remaining: Option<&mut RemainingCustody>,
    ) {
        for place in places.iter_mut().filter(|place| {
            place.root == excluded.root && place.path.starts_with(&excluded.segments)
        }) {
            place.live = false;
            place.case_excluded = true;
        }
        if let Some(remaining) = remaining {
            remaining.exclude_absent_case(excluded);
        }
    }
}
mod returned_claims {
    /*
        Track which resources a returned value owns. Every declared resource must
        be accounted for on every possible return, and one live resource cannot fill
        two fields that exist at the same time.

        Start from the completed value and its actual consuming events. Keep the
        caller's claim identities; the callee's original declarations cannot replace
        them. Ordinary affine storage moves are checked separately: they neither
        create linear resources nor prove that a case payload is absent.
    */
    use super::claim_outcomes::InactiveClaimQueries;
    use super::claim_outcomes::claim_paths_are_case_alternatives;
    use super::linear_obligations::LinearPlace;
    use super::linear_validation::permission_event_statement_index;
    use crate::flow::{CapturedValueChoice, SelectionCondition, SelectionConditions};
    use checked_trees::{
        FlowOperandCompletion, FlowOperandFact, FlowPermissionEvaluation, FlowStateOutgoingFact,
    };
    use facts::PlaceSegment;
    use language_semantics::{PermissionAccess, PermissionClaimIdentity, PermissionEventKind};
    use std::collections::{HashMap, HashSet};

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(crate) enum ReturnedClaimSource {
        Live(PermissionClaimIdentity),
        Inactive,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(crate) struct ReturnedClaim {
        pub path: Vec<PlaceSegment>,
        pub source: ReturnedClaimSource,
        pub condition: SelectionCondition,
    }

    /*
        Operand completion precedes its enclosing store. Only the receiving cut
        may read the current destination after its owned frontier is established.
    */
    #[derive(Clone, Copy, Debug)]
    pub(crate) enum CompletedClaimLocation {
        Expression,
        ReceivingStorage,
    }

    /*
        One authentic consumer occurrence. The completed operand has its own
        evaluation prefix; the enclosing exit or invocation only names its owner.
    */
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub(crate) struct ClaimConsumption {
        pub machine: symbols::SymbolHandle,
        pub state: symbols::SymbolHandle,
        pub statement_index: usize,
        pub evaluation: FlowPermissionEvaluation,
        pub source: language_semantics::PermissionEventSource,
    }

    impl ClaimConsumption {
        /*
            A construction receives its children during expression evaluation. The
            invocation consumes only the completed anonymous owner afterwards.
        */
        fn for_construction_child(self) -> Self {
            Self {
                source: language_semantics::PermissionEventSource::Statement {
                    statement_index: self.statement_index,
                },
                ..self
            }
        }

        fn consumes(
            self,
            operand: &FlowOperandFact,
            event: &checked_trees::FlowPermissionEventFact,
        ) -> bool {
            event.machine_symbol == self.machine
                && event.state_symbol == self.state
                && event.evaluation == self.evaluation
                && event.expression == operand.expression
                && event.source == self.source
                && event.kind == PermissionEventKind::Transfer
                && event.multiplicity == language_semantics::Multiplicity::Linear
                && event.access == PermissionAccess::Owned
        }

        fn for_operand(self, operand: &FlowOperandFact) -> Self {
            if operand.statement_index == self.statement_index {
                self
            } else {
                Self {
                    statement_index: operand.statement_index,
                    evaluation: FlowPermissionEvaluation::Statement,
                    source: language_semantics::PermissionEventSource::Statement {
                        statement_index: operand.statement_index,
                    },
                    ..self
                }
            }
        }
    }

    /*
        Every declared live result path must have one exact consuming occurrence.
        Missing captures and ambiguous events cannot silently remove an exit or a
        child. Named transfers have no local result capture; their caller consumes
        the selected target's complete summary instead.
    */
    pub(crate) fn current_returned_claims(
        program: &dyn ::validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        outgoing: &FlowStateOutgoingFact,
        conditions: &mut SelectionConditions,
    ) -> Result<Vec<ReturnedClaim>, &'static str> {
        if !outgoing.returned_value.is_valid() {
            return Err("outgoing edge has no completed value capture");
        }
        let operand = facts.flow.control.operands.get(outgoing.returned_value);
        let consumer = ClaimConsumption {
            machine: outgoing.machine_symbol,
            state: outgoing.state_symbol,
            statement_index: outgoing.statement_index,
            evaluation: if outgoing.transition_target.is_valid() {
                FlowPermissionEvaluation::TransitionTarget(outgoing.transition_target)
            } else {
                FlowPermissionEvaluation::Statement
            },
            source: language_semantics::PermissionEventSource::Statement {
                statement_index: outgoing.statement_index,
            },
        };
        captured_claims(
            program,
            facts,
            consumer,
            operand,
            ClaimObservation::Consumed,
            None,
            &mut InactiveClaimQueries::new(program, facts, consumer.state),
            conditions,
        )
    }

    pub(crate) fn consumed_operand_claims(
        program: &dyn ::validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        consumer: ClaimConsumption,
        operand: arena::Handle<FlowOperandFact>,
        conditions: &mut SelectionConditions,
    ) -> Result<Vec<ReturnedClaim>, &'static str> {
        if !facts.flow.control.operands.is_valid(operand) {
            return Err("consumer has no completed operand");
        }
        let operand = facts.flow.control.operands.get(operand);
        if operand.statement_index != consumer.statement_index {
            return Err("consumer and operand have different evaluation coordinates");
        }
        captured_claims(
            program,
            facts,
            consumer,
            operand,
            ClaimObservation::Consumed,
            None,
            &mut InactiveClaimQueries::new(program, facts, consumer.state),
            conditions,
        )
    }

    /*
        Ownership before publication and custody after consumption are distinct
        observations of the same retained completion tree.
    */
    #[derive(Clone, Copy)]
    enum ClaimObservation<'a> {
        Consumed,
        Completed {
            places: &'a [LinearPlace],
            events: &'a [checked_trees::FlowPermissionEventFact],
            parent: typed_trees::expression::ExpressionHandle,
        },
    }

    impl ClaimObservation<'_> {
        fn for_child(self, parent: typed_trees::expression::ExpressionHandle) -> Self {
            match self {
                Self::Consumed => self,
                Self::Completed { places, events, .. } => Self::Completed {
                    places,
                    events,
                    parent,
                },
            }
        }
    }

    pub(crate) fn completed_owned_claims(
        program: &dyn validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        value: arena::Handle<FlowOperandFact>,
        location: CompletedClaimLocation,
        places: &[LinearPlace],
        evaluation_events: &[checked_trees::FlowPermissionEventFact],
        conditions: &mut SelectionConditions,
    ) -> Result<Vec<ReturnedClaim>, &'static str> {
        if !facts.flow.control.operands.is_valid(value) {
            return Err("completed owner has no exact value capture");
        }
        let operand = facts.flow.control.operands.get(value);
        let destination = match location {
            CompletedClaimLocation::Expression => None,
            CompletedClaimLocation::ReceivingStorage => {
                receiving_destination(program, facts, value)?
            }
        };
        let owner = facts
            .flow
            .control
            .state_by_symbol(operand.state_symbol)
            .map(|state| state.machine_symbol)
            .ok_or("completed owner has no state flow")?;
        let consumer = ClaimConsumption {
            machine: owner,
            state: operand.state_symbol,
            statement_index: operand.statement_index,
            evaluation: FlowPermissionEvaluation::Statement,
            source: language_semantics::PermissionEventSource::Statement {
                statement_index: operand.statement_index,
            },
        };
        captured_claims(
            program,
            facts,
            consumer,
            operand,
            ClaimObservation::Completed {
                places,
                events: evaluation_events,
                parent: Default::default(),
            },
            destination.as_ref(),
            &mut InactiveClaimQueries::new(program, facts, consumer.state),
            conditions,
        )
    }

    /*
        A retained store handle and destination identify the receiving occurrence.
        A malformed actual store withholds custody; a non-store completion keeps its
        expression coordinate. The live frontier still supplies every identity.
    */
    fn receiving_destination(
        program: &dyn validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        value: arena::Handle<FlowOperandFact>,
    ) -> Result<Option<validation::CanonicalPlace>, &'static str> {
        let capture = facts.flow.control.operands.get(value);
        let owner = program
            .machine_holding_state(capture.state_symbol)
            .ok_or("completed store has no declared machine owner")?;
        let state = facts
            .flow
            .control
            .state(owner.symbol, capture.state_symbol)
            .ok_or("completed store has no state flow")?;
        let Some(statement) = facts.flow.state_statement(state, capture.statement_index) else {
            return Ok(None);
        };
        if statement.stored_value != value {
            return Ok(None);
        }
        if !facts.semantic.places.is_valid(statement.stored_destination) {
            return Err("completed store has no exact receiving destination");
        }
        crate::flow::canonical_place_from_semantic_place(
            program,
            facts.semantic,
            facts.semantic.places.get(statement.stored_destination),
        )
        .map(Some)
        .ok_or("completed store destination has no canonical identity")
    }

    fn captured_claims(
        program: &dyn ::validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        consumer: ClaimConsumption,
        operand: &FlowOperandFact,
        observation: ClaimObservation<'_>,
        receiving: Option<&validation::CanonicalPlace>,
        inactive: &mut InactiveClaimQueries<'_>,
        conditions: &mut SelectionConditions,
    ) -> Result<Vec<ReturnedClaim>, &'static str> {
        if !operand.type_reference.is_valid() || operand.state_symbol != consumer.state {
            return Err("returned capture does not belong to this typed outgoing occurrence");
        }
        let frontier = super::linear_claim_frontier(program, operand.type_reference);
        if frontier.is_empty() {
            return Ok(Vec::new());
        }
        if let FlowOperandCompletion::Selected(alternatives) = operand.completion {
            let rows = facts
                .flow
                .control
                .operand_alternatives
                .span(alternatives)
                .filter(|rows| !rows.is_empty())
                .ok_or("selected return has no completed alternatives")?;
            let mut claims = Vec::new();
            for (index, alternative) in rows.iter().enumerate() {
                if !alternative.source_arm.is_valid()
                    || !facts.flow.control.operands.is_valid(alternative.value)
                {
                    return Err("selected return omits a typed arm completion");
                }
                let choice = conditions
                    .choice(
                        facts.flow.control,
                        CapturedValueChoice {
                            selection: alternatives,
                            alternative: alternatives.handle_at(
                                u32::try_from(index)
                                    .map_err(|_| "selection arm exceeds arena coordinates")?,
                            ),
                        },
                    )
                    .ok_or("selected return has an invalid choice coordinate")?;
                for mut claim in captured_claims(
                    program,
                    facts,
                    consumer,
                    facts.flow.control.operands.get(alternative.value),
                    observation,
                    receiving,
                    inactive,
                    conditions,
                )? {
                    claim.condition = conditions.all(choice, claim.condition);
                    claims.push(claim);
                }
            }
            check_frontier(program, facts, operand, &claims, conditions)?;
            return Ok(claims);
        }
        let consumer = consumer.for_operand(operand);
        let evaluation = consumer.evaluation;
        let mut claims = Vec::new();
        let transferred_place = if matches!(observation, ClaimObservation::Consumed) {
            crate::flow::owned_operand_transfer_place(
                facts,
                consumer.machine,
                consumer.state,
                operand.expression,
                evaluation,
                consumer.source,
            )?
        } else {
            None
        };
        let has_carrier_transfer = transferred_place.is_some();
        let mut source_place = receiving.cloned().or(transferred_place).or_else(|| {
            validation::ownership_place_from_expression_in_state(
                program,
                operand.state_symbol,
                operand.statement_index,
                operand.expression,
            )
        });
        if matches!(observation, ClaimObservation::Consumed) {
            let anonymous = facts::PlaceRoot::Expression(operand.expression);
            let consumed_owner = operand.expression.is_valid()
                && facts
                    .flow
                    .ownership
                    .with_state_permissions(consumer.state, |handles| {
                        handles.iter().copied().any(|handle| {
                            let event = facts.flow.ownership.permissions.get(handle);
                            consumer.consumes(operand, event) && event.root == anonymous
                        })
                    });
            if consumed_owner {
                if let Some(place) = &source_place
                    && place.root != anonymous
                    && facts
                        .flow
                        .ownership
                        .with_state_permissions(consumer.state, |handles| {
                            handles.iter().copied().any(|handle| {
                                let event = facts.flow.ownership.permissions.get(handle);
                                consumer.consumes(operand, event)
                                    && event.root == place.root
                                    && facts
                                        .flow
                                        .ownership
                                        .segments
                                        .span_or_empty(event.segments)
                                        .starts_with(&place.segments)
                            })
                        })
                {
                    return Err("completed operand has competing consuming owners");
                }
                source_place = Some(validation::CanonicalPlace {
                    root: anonymous,
                    segments: Vec::new(),
                });
            }
        }
        let mut unresolved_transfers: Vec<Vec<PlaceSegment>> = Vec::new();
        if let Some(place) = &source_place {
            match observation {
                ClaimObservation::Consumed => {
                    let state_permissions = facts
                        .flow
                        .ownership
                        .with_state_permissions(consumer.state, |handles| handles.to_vec());
                    for handle in state_permissions {
                        let event = facts.flow.ownership.permissions.get(handle);
                        if !(consumer.consumes(operand, event) && event.root == place.root) {
                            continue;
                        }
                        let path = facts.flow.ownership.segments.span_or_empty(event.segments);
                        let Some(suffix) = path.strip_prefix(place.segments.as_slice()) else {
                            continue;
                        };
                        let source = if event.obligation_live {
                            if event.claim_identity == PermissionClaimIdentity::Unknown {
                                if unresolved_transfers
                                    .iter()
                                    .any(|path| path.as_slice() == suffix)
                                {
                                    return Err(
                                        "completed owner has duplicate unresolved claim transfers",
                                    );
                                }
                                unresolved_transfers.push(suffix.to_vec());
                                continue;
                            }
                            ReturnedClaimSource::Live(event.claim_identity)
                        } else {
                            ReturnedClaimSource::Inactive
                        };
                        claims.push(ReturnedClaim {
                            path: suffix.to_vec(),
                            source,
                            condition: conditions.always(),
                        });
                    }
                }
                ClaimObservation::Completed {
                    places,
                    events,
                    parent,
                } => {
                    if parent.is_valid() {
                        for event in events.iter().filter(|event| {
                            event.state_symbol == operand.state_symbol
                                && event.expression == operand.expression
                                && event.kind == PermissionEventKind::Transfer
                                && event.multiplicity == language_semantics::Multiplicity::Linear
                                && event.access == PermissionAccess::Owned
                                && event.root == place.root
                                && event.source
                                    == language_semantics::PermissionEventSource::Statement {
                                        statement_index: operand.statement_index,
                                    }
                        }) {
                            let path = facts.flow.ownership.segments.span_or_empty(event.segments);
                            let Some(suffix) = path.strip_prefix(place.segments.as_slice()) else {
                                continue;
                            };
                            let source = if event.obligation_live {
                                if event.claim_identity == PermissionClaimIdentity::Unknown {
                                    return Err("completed child transfer has no exact identity");
                                }
                                ReturnedClaimSource::Live(event.claim_identity)
                            } else {
                                ReturnedClaimSource::Inactive
                            };
                            claims.push(ReturnedClaim {
                                path: suffix.to_vec(),
                                source,
                                condition: conditions.always(),
                            });
                        }
                    }
                    if claims.is_empty() {
                        for current in places.iter().filter(|current| {
                            current.root == place.root
                                && current.multiplicity == language_semantics::Multiplicity::Linear
                                && current.ever_established
                                && !current.zero_image
                                && (current.live || current.case_excluded)
                        }) {
                            let Some(path) = current.path.strip_prefix(place.segments.as_slice())
                            else {
                                continue;
                            };
                            let source = if current.case_excluded {
                                ReturnedClaimSource::Inactive
                            } else {
                                let identity = current
                                    .claim_identity
                                    .filter(|identity| {
                                        *identity != PermissionClaimIdentity::Unknown
                                    })
                                    .ok_or("completed live owner has no exact claim identity")?;
                                ReturnedClaimSource::Live(identity)
                            };
                            claims.push(ReturnedClaim {
                                path: path.to_vec(),
                                source,
                                condition: conditions.always(),
                            });
                        }
                    }
                }
            }
        }
        if receiving.is_none() {
            let transferred_paths = claims
                .iter()
                .map(|claim| claim.path.clone())
                .collect::<Vec<_>>();
            for child in facts
                .flow
                .control
                .operand_children
                .span_or_empty(operand.constructed_children())
            {
                let destination = facts
                    .flow
                    .control
                    .operand_child_paths
                    .span(child.path)
                    .ok_or("constructed child has no typed destination path")?;
                if !frontier.iter().any(|expected| {
                    expected.path.starts_with(destination)
                        && !transferred_paths.contains(&expected.path)
                }) {
                    continue;
                }
                if !facts.flow.control.operands.is_valid(child.value) {
                    return Err("construction omits its completed child");
                }
                let child_operand = facts.flow.control.operands.get(child.value);
                if child.parent != operand.expression
                    || child_operand.state_symbol != operand.state_symbol
                    || child_operand.statement_index != operand.statement_index
                {
                    return Err("child capture belongs to another evaluation occurrence");
                }
                for mut claim in captured_claims(
                    program,
                    facts,
                    consumer.for_construction_child(),
                    child_operand,
                    observation.for_child(operand.expression),
                    None,
                    inactive,
                    conditions,
                )? {
                    let mut path = destination.to_vec();
                    path.append(&mut claim.path);
                    claim.path = path;
                    if transferred_paths.contains(&claim.path)
                        || ((!transferred_paths.is_empty() || !unresolved_transfers.is_empty())
                            && !unresolved_transfers.contains(&claim.path)
                            && claim.source != ReturnedClaimSource::Inactive)
                    {
                        continue;
                    }
                    claims.push(claim);
                }
            }
        }
        let mut represented_paths: HashSet<_> =
            claims.iter().map(|claim| claim.path.clone()).collect();
        if matches!(observation, ClaimObservation::Consumed)
            && frontier
                .iter()
                .any(|expected| !represented_paths.contains(&expected.path))
        {
            let completed = completed_call_claims(program, facts, consumer, operand, conditions)?;
            if claims.is_empty() {
                claims = completed;
                represented_paths.extend(claims.iter().map(|claim| claim.path.clone()));
            } else {
                for claim in completed {
                    if claim.source == ReturnedClaimSource::Inactive
                        && represented_paths.insert(claim.path.clone())
                    {
                        claims.push(claim);
                    }
                }
            }
        }
        for expected in &frontier {
            if represented_paths.contains(&expected.path) {
                continue;
            }
            if receiving.is_none()
                && expected.conditional
                && crate::flow::literal_value_path_is_inactive(
                    program,
                    operand.expression,
                    operand.type_reference,
                    &expected.path,
                )
            {
                represented_paths.insert(expected.path.clone());
                claims.push(ReturnedClaim {
                    path: expected.path.clone(),
                    source: ReturnedClaimSource::Inactive,
                    condition: conditions.always(),
                });
                continue;
            }
            if receiving.is_none()
                && expected.conditional
                && let Some(mut place) = source_place.clone()
                && let Some(state) = program.state_by_symbol(operand.state_symbol)
            {
                place.segments.extend_from_slice(&expected.path);
                if inactive
                    .preserved(state, operand.statement_index, &place)
                    .is_some()
                {
                    represented_paths.insert(expected.path.clone());
                    claims.push(ReturnedClaim {
                        path: expected.path.clone(),
                        source: ReturnedClaimSource::Inactive,
                        condition: conditions.always(),
                    });
                    continue;
                }
            }
            if matches!(observation, ClaimObservation::Consumed)
                && let Some(stored) = stored_claims(
                    program,
                    facts,
                    consumer,
                    operand,
                    &expected.path,
                    source_place.as_ref(),
                    has_carrier_transfer,
                    evaluation,
                    inactive,
                    conditions,
                )?
            {
                represented_paths.extend(stored.iter().map(|claim| claim.path.clone()));
                claims.extend(stored);
            }
        }
        check_frontier(program, facts, operand, &claims, conditions)?;
        Ok(claims)
    }

    /*
        An owned transfer retains its completed RHS even when that value was not
        copyable. This is claim transport, not the scalar content observation query:
        a write below a claim leaves its identity intact while independently
        retiring the producer's content theorem. A replacement at/above the claim
        stops this trace. No declaration initializer is replayed.
    */
    fn stored_claims(
        program: &dyn validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        consumer: ClaimConsumption,
        operand: &FlowOperandFact,
        path: &[PlaceSegment],
        source_place: Option<&validation::CanonicalPlace>,
        has_carrier_transfer: bool,
        evaluation: FlowPermissionEvaluation,
        inactive: &mut InactiveClaimQueries<'_>,
        conditions: &mut SelectionConditions,
    ) -> Result<Option<Vec<ReturnedClaim>>, &'static str> {
        let Some(mut source) = source_place.cloned() else {
            return Ok(None);
        };
        source.segments.extend_from_slice(path);
        if !matches!(source.root, facts::PlaceRoot::Symbol(_)) {
            return Ok(None);
        }
        let consumed =
            facts
                .flow
                .ownership
                .with_state_permissions(operand.state_symbol, |handles| {
                    handles.iter().copied().any(|handle| {
                        let event = facts.flow.ownership.permissions.get(handle);
                        event.machine_symbol == consumer.machine
                            && event.state_symbol == operand.state_symbol
                            && event.evaluation == evaluation
                            && event.expression == operand.expression
                            && event.source == consumer.source
                            && event.kind == PermissionEventKind::Transfer
                            && event.access == PermissionAccess::Owned
                            && event.obligation_live
                            && event.root == source.root
                            && facts.flow.ownership.segments.span_or_empty(event.segments)
                                == source.segments
                    })
                });
        if !consumed && !has_carrier_transfer {
            return Ok(None);
        }
        if !crate::flow::binding_origin_is_stable(
            program,
            facts.flow.control,
            operand.entry_origin_writes,
            operand.entry_origin_unknown_write,
            &source,
        ) {
            return Ok(None);
        }
        let state = program
            .state_by_symbol(operand.state_symbol)
            .ok_or("stored claim has no state owner")?;
        let flow = facts
            .flow
            .control
            .state(consumer.machine, operand.state_symbol)
            .ok_or("stored claim has no state flow")?;
        let order = program
            .statement_table
            .admitted_execution_order(state.statement_nodes);
        let end = order
            .position(operand.statement_index)
            .or_else(|| {
                (operand.statement_index
                    == crate::flow::state_statements(program, state.statement_nodes).len())
                .then_some(order.indices().len())
            })
            .ok_or("stored claim has no admitted execution coordinate")?;
        for &index in order.indices()[..end].iter().rev() {
            let statement = facts
                .flow
                .state_statement(flow, index)
                .ok_or("stored claim trace omits an executed statement")?;
            if facts.semantic.places.is_valid(statement.stored_destination)
                && facts.flow.control.operands.is_valid(statement.stored_value)
            {
                let destination = crate::flow::canonical_place_from_semantic_place(
                    program,
                    facts.semantic,
                    facts.semantic.places.get(statement.stored_destination),
                )
                .ok_or("stored claim destination has no canonical identity")?;
                if destination.root == source.root
                    && source.segments.starts_with(&destination.segments)
                {
                    let saved = facts.flow.control.operands.get(statement.stored_value);
                    if saved.state_symbol != operand.state_symbol
                        || saved.statement_index != index
                        || !saved.expression.is_valid()
                    {
                        return Ok(None);
                    }
                    let relative = &source.segments[destination.segments.len()..];
                    let claims = captured_claims(
                        program,
                        facts,
                        consumer.for_operand(saved),
                        saved,
                        ClaimObservation::Consumed,
                        None,
                        inactive,
                        conditions,
                    )?;
                    let mut selected = Vec::new();
                    for mut claim in claims.into_iter().filter(|claim| {
                        claim.path == relative
                            && (consumed || claim.source == ReturnedClaimSource::Inactive)
                    }) {
                        claim.path = path.to_vec();
                        selected.push(claim);
                    }
                    return Ok((!selected.is_empty()).then_some(selected));
                }
            }
            if !crate::flow::binding_origin_is_stable(
                program,
                facts.flow.control,
                statement.completed_writes,
                statement.completed_unknown_write,
                &source,
            ) {
                return Ok(None);
            }
        }
        Ok(None)
    }

    fn check_frontier(
        program: &dyn ::validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        operand: &FlowOperandFact,
        claims: &[ReturnedClaim],
        conditions: &mut SelectionConditions,
    ) -> Result<(), &'static str> {
        let frontier = super::linear_claim_frontier(program, operand.type_reference);
        let mut by_path: HashMap<&[PlaceSegment], Vec<&ReturnedClaim>> = HashMap::new();
        let mut by_identity: HashMap<PermissionClaimIdentity, Vec<usize>> = HashMap::new();
        for (index, claim) in claims.iter().enumerate() {
            by_path.entry(&claim.path).or_default().push(claim);
            if let ReturnedClaimSource::Live(identity) = claim.source {
                by_identity.entry(identity).or_default().push(index);
            }
        }
        for expected in &frontier {
            let matching = by_path
                .get(expected.path.as_slice())
                .map(Vec::as_slice)
                .unwrap_or_default();
            let mut coverage = conditions.never();
            for (index, claim) in matching.iter().enumerate() {
                if claim.source == ReturnedClaimSource::Inactive && !expected.conditional {
                    return Err("unconditional returned claim was recorded as inactive");
                }
                for previous in &matching[..index] {
                    let overlap = conditions.all(previous.condition, claim.condition);
                    if conditions.possible(facts.flow.control, overlap) {
                        return Err("returned claim path has overlapping consuming occurrences");
                    }
                }
                coverage = conditions.any(coverage, claim.condition);
            }
            if !conditions.covers(facts.flow.control, coverage) {
                trace_missing_frontier(program, facts, operand, &expected.path, claims);
                return Err("returned capture omits a declared claim path on an alternative");
            }
        }
        for (index, claim) in claims.iter().enumerate() {
            let ReturnedClaimSource::Live(identity) = claim.source else {
                continue;
            };
            let matching = &by_identity[&identity];
            let later = matching.partition_point(|other| *other <= index);
            for other_index in &matching[later..] {
                let other = &claims[*other_index];
                if claim.path != other.path
                    && claim.source == other.source
                    && !claim_paths_are_case_alternatives(&claim.path, &other.path)
                {
                    let overlap = conditions.all(claim.condition, other.condition);
                    if conditions.possible(facts.flow.control, overlap) {
                        return Err("one live claim supplies simultaneous returned paths");
                    }
                }
            }
        }
        let declared_paths: HashSet<_> = frontier
            .iter()
            .map(|expected| expected.path.as_slice())
            .collect();
        if claims
            .iter()
            .any(|claim| !declared_paths.contains(claim.path.as_slice()))
        {
            return Err("returned permission frontier contains an undeclared claim path");
        }
        Ok(())
    }

    /*
        A fresh result is already owned at its exact completion. Selection does not
        synthesize a transfer event for that temporary. Join its actual invocation
        and receiving establishment, retaining the caller's current identity.
    */
    fn completed_call_claims(
        program: &dyn ::validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        consumer: ClaimConsumption,
        operand: &FlowOperandFact,
        conditions: &SelectionConditions,
    ) -> Result<Vec<ReturnedClaim>, &'static str> {
        let Some(state_flow) = facts.flow.control.state_by_symbol(operand.state_symbol) else {
            return Err("completed result has no state flow");
        };
        let mut calls = crate::flow::call_flows(facts.flow.control, state_flow.calls)
            .iter()
            .filter(|call| {
                call.statement_index == operand.statement_index
                    && call.authored_expression == operand.expression
            });
        let Some(call) = calls.next() else {
            return Ok(Vec::new());
        };
        if calls.next().is_some() {
            return Err("completed result has ambiguous invocation identity");
        }
        let machine = program
            .machine_by_symbol(consumer.machine)
            .ok_or("result owner machine is absent")?;
        let state = program
            .state_by_symbol(operand.state_symbol)
            .ok_or("result owner state is absent")?;
        let evaluation = crate::flow::call_permission_evaluation(
            program,
            machine,
            state,
            call.statement_index,
            call.call_ordinal,
        );
        if evaluation == FlowPermissionEvaluation::Unknown {
            return Err("completed invocation has no permission evaluation");
        }
        let mut claims = Vec::new();
        let state_permissions = facts
            .flow
            .ownership
            .with_state_permissions(operand.state_symbol, |handles| handles.to_vec());
        for handle in state_permissions {
            let event = facts.flow.ownership.permissions.get(handle);
            if event.machine_symbol != consumer.machine
                || event.state_symbol != operand.state_symbol
                || event.evaluation != evaluation
                || event.kind != PermissionEventKind::Establish
                || event.multiplicity != language_semantics::Multiplicity::Linear
                || event.access != PermissionAccess::Owned
                || (event.obligation_live
                    && event.claim_identity == PermissionClaimIdentity::Unknown)
            {
                continue;
            }
            let Some((statement, expression, path)) =
                validation::receiving_value(program, &facts.flow.ownership.segments, event)
            else {
                continue;
            };
            if statement != call.statement_index || expression != call.authored_expression {
                continue;
            }
            if let language_semantics::PermissionEventSource::Call {
                call_ordinal,
                target_symbol,
                ..
            } = event.source
                && (call_ordinal != call.call_ordinal || target_symbol != call.target_symbol)
            {
                continue;
            }
            let claim = ReturnedClaim {
                path: path.to_vec(),
                source: if event.obligation_live {
                    ReturnedClaimSource::Live(event.claim_identity)
                } else {
                    ReturnedClaimSource::Inactive
                },
                condition: conditions.always(),
            };
            crate::shared::collections::push_unique(&mut claims, claim);
        }
        Ok(claims)
    }

    /* Temporary bounded diagnostic. Root removes this with its call before landing. */
    fn trace_missing_frontier(
        program: &dyn validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        operand: &FlowOperandFact,
        path: &[PlaceSegment],
        claims: &[ReturnedClaim],
    ) {
        let Ok(filter) = std::env::var("OMEGA_TRACE_CLAIM_FRONTIER") else {
            return;
        };
        let Some(machine) = program.machine_holding_state(operand.state_symbol) else {
            return;
        };
        if filter != "*" && filter != program.symbols.name(machine.symbol) {
            return;
        }
        static EMITTED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        if EMITTED.fetch_add(1, std::sync::atomic::Ordering::Relaxed) >= 48 {
            return;
        }
        let ownership = &facts.flow.ownership;
        let state = facts
            .flow
            .control
            .state(machine.symbol, operand.state_symbol);
        ownership.walk_guard.note(operand.state_symbol);
        let events = ownership
            .permissions
            .iter()
            .filter(|&(_, event)| {
                event.state_symbol == operand.state_symbol
                    && permission_event_statement_index(event.source)
                        == Some(operand.statement_index)
            })
            .map(|(_, event)| (event, ownership.segments.span_or_empty(event.segments)))
            .collect::<Vec<_>>();
        let transfers = state
            .map(|state| {
                ownership
                    .storage_transfers
                    .span_or_empty(state.storage_transfers)
            })
            .unwrap_or(&[])
            .iter()
            .filter_map(|event| {
                let statement = match event.source {
                    checked_trees::FlowStorageTransferSource::Statement { statement_index }
                    | checked_trees::FlowStorageTransferSource::Call {
                        statement_index, ..
                    } => statement_index,
                };
                (statement == operand.statement_index)
                    .then(|| (event, ownership.segments.span_or_empty(event.segments)))
            })
            .collect::<Vec<_>>();
        let source = validation::ownership_place_from_expression_in_state(
            program,
            operand.state_symbol,
            operand.statement_index,
            operand.expression,
        );
        let prefix_events = ownership
            .permissions
            .iter()
            .filter(|&(_, event)| {
                event.machine_symbol == machine.symbol
                    && event.state_symbol == operand.state_symbol
                    && (event.root == facts::PlaceRoot::Expression(operand.expression)
                        || source
                            .as_ref()
                            .is_some_and(|source| source.root == event.root)
                        || transfers
                            .iter()
                            .any(|(transfer, _)| transfer.root == event.root))
                    && permission_event_statement_index(event.source)
                        .is_some_and(|statement| statement <= operand.statement_index)
            })
            .map(|(_, event)| (event, ownership.segments.span_or_empty(event.segments)))
            .collect::<Vec<_>>();
        eprintln!(
            "CLAIM_SOURCE_PREFIX expression={:?} source={source:?} events={prefix_events:?}",
            operand.expression
        );
        let children = facts
            .flow
            .control
            .operand_children
            .span_or_empty(operand.constructed_children())
            .iter()
            .map(|child| {
                (
                    child,
                    facts
                        .flow
                        .control
                        .operand_child_paths
                        .span_or_empty(child.path),
                    facts.flow.control.operands.get(child.value),
                )
            })
            .collect::<Vec<_>>();
        eprintln!(
            "CLAIM_FRONTIER machine={} state={} missing={path:?} operand={operand:?} syntax={:?} claims={claims:?} events={events:?} transfers={transfers:?} children={children:?}",
            program.symbols.name(machine.symbol),
            program.symbols.name(operand.state_symbol),
            operand
                .expression
                .is_valid()
                .then(|| program.expression_table.expression(operand.expression))
        );
    }
}
pub(crate) use linear_obligations::LinearPlace;
pub(crate) use returned_claims::{
    ClaimConsumption, ReturnedClaim, ReturnedClaimSource, consumed_operand_claims,
    current_returned_claims,
};
pub(crate) use returned_claims::{CompletedClaimLocation, completed_owned_claims};
mod selected_cleanup {
    /*
        Candidate deaths retain exact regions of an established occurrence across
        selected transfers. Ordinary availability stays conservative; the retained
        candidates never enter the unconditional permission ledger.
    */
    use super::cleanup_dispositions::{
        CustodyCut, RemainingCustody, StorageBindingChange, StorageEstablishment, TransferCustody,
    };
    use super::linear_obligations::LinearPlace;
    use crate::checks::ownership_occurrences::EvaluationBranch;
    use crate::checks::ownership_occurrences::StatementOccurrence;
    use crate::checks::ownership_occurrences::routes_compatible;
    use crate::checks::places::place_or_root_in;
    use crate::flow::canonical_place_segments_may_overlap;
    use crate::flow::state_statements;
    use checked_trees::{
        ContextualCleanupFacts, FlowClaimJoinExit, FlowOwnershipFacts, FlowPermissionEventFact,
    };
    use facts::{PlaceRoot, PlaceSegment};
    use language_semantics::{PermissionAccess, PermissionEventKind, PermissionEventSource};

    #[derive(Clone, Default)]
    pub(super) struct SelectedCleanup {
        owners: Vec<Owner>,
    }

    #[derive(Clone)]
    struct Owner {
        selection: arena::Handle<checked_trees::FlowOwnedSelectionReceipt>,
        root: PlaceRoot,
        path: Vec<PlaceSegment>,
        reference: typed_trees::types::TypeReferenceHandle,
        origin: FlowPermissionEventFact,
        places: Vec<LinearPlace>,
        remaining: RemainingCustody,
        /*
            A receipt-backed candidate can outlive the named home's occupant.
            Keep the exact capture cut and subsequent physical write barriers.
        */
        source_bindings: Vec<SourceBinding>,
    }

    #[derive(Clone)]
    struct SourceBinding {
        statement: usize,
        evaluation: checked_trees::FlowPermissionEvaluation,
        cut: CustodyCut,
        route: Vec<EvaluationBranch>,
        detached_paths: Vec<Vec<PlaceSegment>>,
        unknown_write: bool,
    }

    impl SourceBinding {
        fn follows_capture(
            &self,
            source: PermissionEventSource,
            evaluation: checked_trees::FlowPermissionEvaluation,
            cut: CustodyCut,
        ) -> bool {
            let statement = match source {
                PermissionEventSource::Statement { statement_index }
                | PermissionEventSource::Call {
                    statement_index, ..
                } => statement_index,
                _ => return false,
            };
            statement != self.statement || evaluation != self.evaluation || cut >= self.cut
        }

        fn observe_write(&mut self, root: PlaceRoot, write: &validation::CanonicalPlace) {
            if !write.root.is_valid() {
                self.unknown_write = true;
            } else if write.root == root && !self.detached_paths.contains(&write.segments) {
                self.detached_paths.push(write.segments.clone());
            }
        }

        fn carries_path(
            &self,
            program: &dyn validation::CheckingInput,
            path: &[PlaceSegment],
        ) -> bool {
            !self.unknown_write
                && !self
                    .detached_paths
                    .iter()
                    .any(|detached| canonical_place_segments_may_overlap(program, detached, path))
        }
    }

    impl SelectedCleanup {
        pub(super) fn is_empty(&self) -> bool {
            self.owners.is_empty()
        }

        /*
            A completed selection transports one candidate; its exact pre-transfer
            complements remain owed. Capture every actual source cut, normalizing
            only paths this receipt transfers, never statement-entry custody.
        */
        pub(super) fn observe_selection(
            &mut self,
            program: &dyn validation::CheckingInput,
            selection: arena::Handle<checked_trees::FlowOwnedSelectionReceipt>,
            ownership: &FlowOwnershipFacts,
            transfers: &[TransferCustody],
            observations: &[super::linear_validation::ObservedCustody],
            events: &[FlowPermissionEventFact],
            occurrences: &[StatementOccurrence],
        ) -> Result<(), diagnostics::Diagnostic> {
            let receipt = ownership.owned_selections.get(selection);
            for source in ownership.selection_sources.span_or_empty(receipt.sources) {
                if !source.retains_storage {
                    continue;
                }
                let root = PlaceRoot::Symbol(source.symbol);
                let reference = validation::canonical_place_type_reference(
                    program,
                    receipt.state,
                    receipt.statement_ordinal as usize,
                    &validation::CanonicalPlace {
                        root,
                        segments: Vec::new(),
                    },
                )
                .ok_or_else(|| {
                    diagnostics::Diagnostic::error(
                        "selected complement has no exact established source type",
                    )
                })?;
                let selected_transfers = ownership
                    .selection_transfers
                    .span_or_empty(receipt.transfers)
                    .iter()
                    .filter(|transfer| {
                        transfer.source.is_valid()
                            && ownership.selection_sources.get(transfer.source).symbol
                                == source.symbol
                    })
                    .collect::<Vec<_>>();
                let paths = selected_transfers
                    .iter()
                    .map(|transfer| ownership.segments.span_or_empty(transfer.path).to_vec())
                    .collect::<Vec<_>>();
                for transfer in &selected_transfers {
                    let source_place =
                        place_or_root_in(&ownership.segments, transfer.root, transfer.path);
                    let (places, remaining, cut) =
                        super::owned_selection::evaluated_source_custody(
                            ownership,
                            transfer.expression,
                            &source_place,
                            receipt.evaluation,
                            transfers,
                            observations,
                            events,
                        )?;
                    let established = places
                        .iter()
                        .filter(|place| place.root == root)
                        .cloned()
                        .collect::<Vec<_>>();
                    let captured_remaining = remaining.clone();
                    let source_binding = SourceBinding {
                        statement: receipt.statement_ordinal as usize,
                        evaluation: receipt.evaluation,
                        cut,
                        route: occurrences
                            .get(cut.occurrence_ordinal)
                            .ok_or_else(|| {
                                diagnostics::Diagnostic::error(
                                    "selected source cut has no evaluated occurrence",
                                )
                            })?
                            .route
                            .clone(),
                        detached_paths: Vec::new(),
                        unknown_write: false,
                    };
                    let same = self.owners.iter().position(|owner| {
                        owner.selection == selection
                            && owner.root == root
                            && owner.path.is_empty()
                            && owner.places.len() == established.len()
                            && owner.places.iter().zip(&established).all(|(left, right)| {
                                let mut left = left.clone();
                                let mut right = right.clone();
                                if paths.iter().any(|path| left.path.starts_with(path)) {
                                    left.live = false;
                                    right.live = false;
                                }
                                left == right
                            })
                            && owner.remaining.same_storage_after_transfers(
                                &captured_remaining,
                                root,
                                &paths,
                            )
                    });
                    if let Some(index) = same {
                        let bindings = &mut self.owners[index].source_bindings;
                        if !bindings.iter().any(|binding| {
                            binding.statement == source_binding.statement
                                && binding.evaluation == source_binding.evaluation
                                && binding.cut == source_binding.cut
                                && binding.route == source_binding.route
                        }) {
                            bindings.push(source_binding);
                        }
                        continue;
                    }
                    self.owners.push(Owner {
                        selection,
                        root,
                        path: Vec::new(),
                        reference,
                        origin: FlowPermissionEventFact {
                            machine_symbol: receipt.machine,
                            state_symbol: receipt.state,
                            source: PermissionEventSource::Statement {
                                statement_index: receipt.statement_ordinal as usize,
                            },
                            expression: transfer.expression,
                            evaluation: receipt.evaluation,
                            kind: PermissionEventKind::Transfer,
                            multiplicity: language_semantics::Multiplicity::Affine,
                            access: PermissionAccess::Owned,
                            provenance: transfer.provenance,
                            claim_identity: transfer.claim_identity,
                            root,
                            ..Default::default()
                        },
                        places: established,
                        remaining: captured_remaining,
                        source_bindings: vec![source_binding],
                    });
                }
            }
            Ok(())
        }

        pub(super) fn observe(
            &mut self,
            program: &dyn validation::CheckingInput,
            selected: &[typed_trees::expression::ExpressionHandle],
            transfers: &[TransferCustody],
            routes: &super::selected_evaluation::ExpressionRoutes,
            calls: &[checked_trees::FlowCallFact],
            events: &[FlowPermissionEventFact],
            occurrences: &[StatementOccurrence],
            establishments: &[StorageEstablishment],
            binding_changes: &[StorageBindingChange],
            ownership: &mut FlowOwnershipFacts,
            output: &mut ContextualCleanupFacts,
        ) -> Result<(), diagnostics::Diagnostic> {
            if self.owners.is_empty() && selected.is_empty() && events.is_empty() {
                return Ok(());
            }
            let mut next_binding_change = 0;
            for (ordinal, event) in events.iter().enumerate() {
                while binding_changes
                    .get(next_binding_change)
                    .is_some_and(|change| change.cut.permission_prefix <= ordinal)
                {
                    self.observe_binding_change(&binding_changes[next_binding_change]);
                    next_binding_change += 1;
                }
                if event.access != PermissionAccess::Owned {
                    continue;
                }
                let path = ownership.segments.span_or_empty(event.segments).to_vec();
                for establishment in establishments
                    .iter()
                    .filter(|entry| entry.event_ordinal == ordinal)
                {
                    self.retire_transferred(routes, calls, &events[..ordinal], ownership)?;
                    let destination = &establishment.destination;
                    for owner in &mut self.owners {
                        for binding in &mut owner.source_bindings {
                            binding.observe_write(owner.root, destination);
                        }
                    }
                    let mut retired = Vec::new();
                    for (index, owner) in self.owners.iter().enumerate() {
                        if owner.selection.is_valid() {
                            continue;
                        }
                        if owner.root != destination.root {
                            continue;
                        }
                        if owner.path.starts_with(&destination.segments) {
                            owner.publish(
                                program,
                                event.clone(),
                                FlowClaimJoinExit::default(),
                                ownership,
                                output,
                            )?;
                            retired.push(index);
                        } else if destination.segments.starts_with(&owner.path) {
                            return Err(unsupported(
                                program,
                                event,
                                "a child replacement cannot restore a conditionally transferred containing owner",
                            ));
                        }
                    }
                    for index in retired.into_iter().rev() {
                        self.owners.remove(index);
                    }
                }
                let contained_transfer = events[..ordinal].iter().any(|prior| {
                    prior.root == event.root
                        && prior.expression == event.expression
                        && prior.source == event.source
                        && prior.evaluation == event.evaluation
                        && prior.kind == event.kind
                        && prior.access == PermissionAccess::Owned
                        && prior.multiplicity == language_semantics::Multiplicity::Affine
                        && matches!(
                            prior.kind,
                            PermissionEventKind::Transfer | PermissionEventKind::Consume
                        )
                        && path.starts_with(ownership.segments.span_or_empty(prior.segments))
                });
                if !matches!(
                    event.kind,
                    PermissionEventKind::Transfer | PermissionEventKind::Consume
                ) {
                    continue;
                }
                if !selected.contains(&event.expression) {
                    self.owners.retain(|owner| {
                        owner.selection.is_valid()
                            || owner.root != event.root
                            || !owner.path.starts_with(&path)
                    });
                    for owner in self
                        .owners
                        .iter_mut()
                        .filter(|owner| owner.root == event.root)
                    {
                        let same_source = if owner.selection.is_valid() {
                            let mut snapshots = transfers
                                .iter()
                                .filter(|snapshot| snapshot.events.contains(&ordinal));
                            if let (Some(snapshot), None) = (snapshots.next(), snapshots.next()) {
                                occurrences
                                    .get(snapshot.occurrence_ordinal)
                                    .is_some_and(|occurrence| occurrence.route.is_empty())
                                    && !owner.source_bindings.is_empty()
                                    && owner.source_bindings.iter().all(|binding| {
                                        binding.follows_capture(
                                            event.source,
                                            event.evaluation,
                                            CustodyCut {
                                                occurrence_ordinal: snapshot.occurrence_ordinal,
                                                permission_prefix: ordinal,
                                            },
                                        ) && binding.carries_path(program, &path)
                                    })
                            } else {
                                false
                            }
                        } else {
                            true
                        };
                        if same_source {
                            owner.remaining.transfer_storage(event.root, &path);
                        }
                    }
                    continue;
                }
                if contained_transfer {
                    continue;
                }
                if event.root.as_symbol().is_none() {
                    continue;
                }
                let transfer = transfers
                    .iter()
                    .find(|transfer| transfer.events.contains(&ordinal))
                    .ok_or_else(|| {
                        unsupported(
                            program,
                            event,
                            "selected transfer has no producer custody snapshot",
                        )
                    })?;
                let before_transfer = &transfer.remaining;
                let statement = match event.source {
                    PermissionEventSource::Statement { statement_index }
                    | PermissionEventSource::Call {
                        statement_index, ..
                    } => statement_index,
                    _ => 0,
                };
                let Some(reference) = validation::canonical_place_type_reference(
                    program,
                    event.state_symbol,
                    statement,
                    &validation::CanonicalPlace {
                        root: event.root,
                        segments: path.clone(),
                    },
                ) else {
                    return Err(unsupported(
                        program,
                        event,
                        "selected owner region has no exact declared type",
                    ));
                };
                if self.owners.iter().any(|owner| {
                    if owner.root != event.root {
                        return false;
                    }
                    if !owner.selection.is_valid() {
                        return owner.path == path;
                    }
                    if !path.starts_with(&owner.path) {
                        return false;
                    }
                    let receipt = ownership.owned_selections.get(owner.selection);
                    receipt.state == event.state_symbol
                        && receipt.evaluation == event.evaluation
                        && event.source
                            == PermissionEventSource::Statement {
                                statement_index: receipt.statement_ordinal as usize,
                            }
                        && ownership
                            .selection_transfers
                            .span_or_empty(receipt.transfers)
                            .iter()
                            .any(|transfer| {
                                transfer.root == event.root
                                    && transfer.expression == event.expression
                                    && ownership.segments.span_or_empty(transfer.path) == path
                            })
                }) {
                    continue;
                }
                let established = transfer
                    .places
                    .iter()
                    .filter(|place| place.root == event.root)
                    .cloned()
                    .collect::<Vec<_>>();
                let reserved = before_transfer.has_storage_frontier(event.root);
                if (established.iter().any(|place| place.ever_established) || reserved)
                    && !before_transfer.is_present(event.root, &path)
                {
                    let message = "owned region is unavailable at this selected transfer: it was already transferred or has not been established";
                    let Some((machine, state)) = program
                        .machine_by_symbol(event.machine_symbol)
                        .zip(program.state_by_symbol(event.state_symbol))
                    else {
                        return Err(unsupported(program, event, message));
                    };
                    let spelling =
                        super::borrowed_windows::place_spelling(program, event.root, &path);
                    let reason = if reserved {
                        format!(
                            "cannot use `{spelling}`: this selected region was transferred or has not been established"
                        )
                    } else {
                        format!(
                            "cannot use `{spelling}` after `{spelling}` was transferred or consumed"
                        )
                    };
                    return Err(diagnostics::Diagnostic::error(format!(
                        "{reason} in machine `{}` state `{}`",
                        machine.name.as_str(),
                        state.name.as_str(),
                    ))
                    .with_source_span(source_gap(program, event, message).source_span));
                }
                if established.is_empty()
                    || !(established.iter().any(|place| place.ever_established) || reserved)
                    || !before_transfer.is_present(event.root, &path)
                {
                    return Err(unsupported(
                        program,
                        event,
                        "selected owner region has no exact established occurrence custody",
                    ));
                }
                self.owners.push(Owner {
                    selection: Default::default(),
                    root: event.root,
                    path,
                    reference,
                    origin: event.clone(),
                    places: established,
                    remaining: before_transfer.clone(),
                    source_bindings: Vec::new(),
                });
            }
            for change in &binding_changes[next_binding_change..] {
                self.observe_binding_change(change);
            }
            self.retire_transferred(routes, calls, events, ownership)
        }

        fn observe_binding_change(&mut self, change: &StorageBindingChange) {
            for owner in &mut self.owners {
                for binding in &mut owner.source_bindings {
                    if !binding.follows_capture(change.source, change.evaluation, change.cut) {
                        continue;
                    }
                    let same_evaluation = matches!(change.source,
                        PermissionEventSource::Statement { statement_index }
                            | PermissionEventSource::Call { statement_index, .. }
                            if statement_index == binding.statement)
                        && change.evaluation == binding.evaluation;
                    if same_evaluation && !routes_compatible(&binding.route, &change.route) {
                        continue;
                    }
                    let Some(writes) = &change.writes else {
                        binding.unknown_write = true;
                        continue;
                    };
                    for write in writes {
                        binding.observe_write(owner.root, write);
                    }
                }
            }
        }

        /*
            A candidate represents custody retained by some predecessor, not an
            obligation to invent a skipped predecessor. Remove it only when this
            evaluation consumed its whole region on every route. This may-presence
            join is deliberately distinct from ordinary must-have availability;
            partial child moves cannot prove absence of their enclosing owner.
        */
        fn retire_transferred(
            &mut self,
            routes: &super::selected_evaluation::ExpressionRoutes,
            calls: &[checked_trees::FlowCallFact],
            events: &[FlowPermissionEventFact],
            ownership: &FlowOwnershipFacts,
        ) -> Result<(), diagnostics::Diagnostic> {
            if self.owners.is_empty() {
                return Ok(());
            }
            let mut present = vec![true; self.owners.len()];
            let mut selection = super::selected_evaluation::SelectedEvaluation::new();
            let join = |_branch, current: &mut Vec<bool>, sibling: &Vec<bool>| {
                for (current, sibling) in current.iter_mut().zip(sibling) {
                    *current |= *sibling;
                }
                Ok(())
            };
            for event in events
                .iter()
                .filter(|event| event.access == PermissionAccess::Owned)
            {
                selection.enter(
                    routes.event(event, calls),
                    &mut present,
                    |_, _| Ok(()),
                    join,
                )?;
                let path = ownership.segments.span_or_empty(event.segments);
                for (owner, present) in self.owners.iter().zip(&mut present) {
                    if !owner.selection.is_valid()
                        && owner.root == event.root
                        && owner.path.starts_with(path)
                    {
                        match event.kind {
                            PermissionEventKind::Transfer => *present = false,
                            PermissionEventKind::Establish | PermissionEventKind::Restore => {
                                *present = true;
                            }
                            PermissionEventKind::Consume | PermissionEventKind::AffineDrop => {}
                        }
                    }
                }
            }
            selection.enter(&[], &mut present, |_, _| Ok(()), join)?;
            let mut presence = present.into_iter();
            self.owners.retain(|_| presence.next().unwrap_or(true));
            Ok(())
        }

        pub(super) fn finish(
            &self,
            program: &dyn validation::CheckingInput,
            state: &typed_trees::state::State,
            exit: FlowClaimJoinExit,
            ordinary: &[FlowPermissionEventFact],
            remaining: &mut RemainingCustody,
            ownership: &mut FlowOwnershipFacts,
            output: &mut ContextualCleanupFacts,
        ) -> Result<(), diagnostics::Diagnostic> {
            let mut deaths = Vec::new();
            for event in ordinary {
                let path = ownership.segments.span_or_empty(event.segments);
                let regions = remaining.disposal_paths(program, event.root, path);
                if regions.is_empty() {
                    deaths.push((event.clone(), None));
                    continue;
                }
                for region in regions {
                    let mut death = event.clone();
                    death.segments = ownership.segments.insert_many(region);
                    deaths.push((death, None));
                }
            }
            for (index, owner) in self.owners.iter().enumerate() {
                let captured_home = owner.selection.is_valid()
                    && program.type_multiplicity(
                        ownership
                            .owned_selections
                            .get(owner.selection)
                            .type_reference,
                    ) != language_semantics::Multiplicity::Linear;
                if !captured_home && remaining.definitely_transferred(owner.root, &owner.path) {
                    continue;
                }
                let mut template = owner.origin.clone();
                template.source = PermissionEventSource::StateExit;
                template.expression = Default::default();
                template.evaluation =
                    super::linear_validation::exit_evaluation(program, state, exit);
                for candidate in owner.events(program, template, ownership)? {
                    ownership
                        .selected_cleanup_permissions
                        .append(candidate.clone());
                    deaths.push((candidate, Some(index)));
                }
            }
            let mut roots = program
                .state_parameters(state)
                .iter()
                .map(|parameter| PlaceRoot::Symbol(parameter.symbol))
                .collect::<Vec<_>>();
            roots.extend(
                state_statements(program, state.statement_nodes)
                    .iter()
                    .filter_map(|statement| {
                        statement
                            .as_local_data()
                            .map(|local| PlaceRoot::Symbol(local.symbol))
                    }),
            );
            let establishment_orders = roots
                .iter()
                .map(|root| {
                    let mut order = Vec::<Vec<PlaceSegment>>::new();
                    for owner in self.owners.iter().filter(|owner| owner.root == *root) {
                        for path in owner.remaining.established_paths(*root) {
                            if !order.contains(path) {
                                order.push(path.clone());
                            }
                        }
                    }
                    for path in remaining.established_paths(*root) {
                        if !order.contains(path) {
                            order.push(path.clone());
                        }
                    }
                    order
                })
                .collect::<Vec<_>>();
            deaths.sort_by_key(|(event, _)| {
                let root_position = roots.iter().position(|root| *root == event.root);
                let path = ownership.segments.span_or_empty(event.segments);
                let establishment = root_position
                    .and_then(|position| {
                        establishment_orders[position]
                            .iter()
                            .position(|established| path.starts_with(established))
                    })
                    .map_or(0, |position| position + 1);
                (
                    std::cmp::Reverse(root_position),
                    std::cmp::Reverse(establishment),
                )
            });
            let execution_order = program
                .statement_table
                .admitted_execution_order(state.statement_nodes);
            let establishment_ordered_roots = roots
                .iter()
                .filter(|root| {
                    self.owners.iter().any(|owner| {
                        if owner.root != **root {
                            return false;
                        }
                        let captured_at = match owner.origin.source {
                            PermissionEventSource::Statement { statement_index }
                            | PermissionEventSource::Call {
                                statement_index, ..
                            } => statement_index,
                            _ => 0,
                        };
                        let receiving_writes = output
                            .storage_establishments
                            .iter()
                            .filter_map(|(_, write)| {
                                let statement = match write.source {
                                    PermissionEventSource::Statement { statement_index }
                                    | PermissionEventSource::Call {
                                        statement_index, ..
                                    } => statement_index,
                                    _ => return None,
                                };
                                let path =
                                    output.segments.span_or_empty(write.destination.segments);
                                if write.machine == owner.origin.machine_symbol
                                    && write.state == owner.origin.state_symbol
                                    && write.destination.root == **root
                                    && execution_order.at_or_before(captured_at, statement)
                                    && execution_order.at_or_before(statement, exit.statement_index)
                                    && remaining.is_present(**root, path)
                                {
                                    Some(path.to_vec())
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>();
                        !owner.remaining.completes_with(
                            program,
                            **root,
                            remaining.established_paths(**root),
                            &receiving_writes,
                        )
                    })
                })
                .copied()
                .collect::<Vec<_>>();
            let mut candidate_remaining = self
                .owners
                .iter()
                .map(|owner| owner.remaining.clone())
                .collect::<Vec<_>>();
            for (event, candidate) in deaths {
                if let Some(index) = candidate {
                    let first = output.dispositions.len();
                    candidate_remaining[index].observe(
                        program,
                        &[event],
                        &ownership.segments,
                        output,
                        exit,
                        &[],
                        true,
                        &establishment_ordered_roots,
                    );
                    let handles = output
                        .dispositions
                        .iter()
                        .skip(first)
                        .map(|(handle, _)| handle)
                        .collect::<Vec<_>>();
                    for handle in handles {
                        output.dispositions.get_mut(handle).selection =
                            self.owners[index].selection;
                    }
                } else {
                    remaining.observe(
                        program,
                        &[event],
                        &ownership.segments,
                        output,
                        exit,
                        &[],
                        false,
                        &establishment_ordered_roots,
                    );
                }
            }
            Ok(())
        }
    }

    impl Owner {
        fn events(
            &self,
            program: &dyn validation::CheckingInput,
            template: FlowPermissionEventFact,
            ownership: &mut FlowOwnershipFacts,
        ) -> Result<Vec<FlowPermissionEventFact>, diagnostics::Diagnostic> {
            let mut events = Vec::new();
            super::linear_validation::append_owned_region_cleanup_permission_events(
                program,
                self.reference,
                self.root,
                &self.path,
                &self.places,
                &self.remaining,
                template,
                &mut ownership.segments,
                &mut events,
            );
            if events.is_empty() {
                let missing_disposition = self.places.iter().any(|place| {
                    if place.root != self.root
                        || !place.live
                        || place.multiplicity != language_semantics::Multiplicity::Linear
                        || !place.path.starts_with(&self.path)
                    {
                        return false;
                    }
                    validation::project_type_reference_from_segments(
                        program,
                        self.reference,
                        &place.path[self.path.len()..],
                    )
                    .is_some_and(|reference| {
                        let reference = validation::peel_declared_constraints(program, reference);
                        program
                            .closed_data_definition(reference)
                            .is_some_and(|data| {
                                data.properties.multiplicity
                                    == language_semantics::Multiplicity::Linear
                            })
                            && validation::nominal_cleanup_machine_for_type(program, reference, &[])
                                .is_none()
                    })
                });
                let message = "cannot automatically dispose retained selected owner region: every live linear obligation requires an eligible disposition";
                if missing_disposition {
                    return Err(diagnostics::Diagnostic::error(message)
                        .with_source_span(source_gap(program, &self.origin, message).source_span));
                }
                return Err(unsupported(
                    program,
                    &self.origin,
                    "selected owner cleanup has no exact supported region disposition",
                ));
            }
            Ok(events)
        }

        fn publish(
            &self,
            program: &dyn validation::CheckingInput,
            template: FlowPermissionEventFact,
            exit: FlowClaimJoinExit,
            ownership: &mut FlowOwnershipFacts,
            output: &mut ContextualCleanupFacts,
        ) -> Result<(), diagnostics::Diagnostic> {
            let events = self.events(program, template, ownership)?;
            for event in &events {
                ownership.selected_cleanup_permissions.append(event.clone());
            }
            let first = output.dispositions.len();
            self.remaining.clone().observe(
                program,
                &events,
                &ownership.segments,
                output,
                exit,
                &[],
                true,
                &[],
            );
            let handles = output
                .dispositions
                .iter()
                .skip(first)
                .map(|(handle, _)| handle)
                .collect::<Vec<_>>();
            for handle in handles {
                output.dispositions.get_mut(handle).selection = self.selection;
            }
            Ok(())
        }
    }

    fn source_gap(
        program: &dyn validation::CheckingInput,
        event: &FlowPermissionEventFact,
        message: &'static str,
    ) -> checked_trees::CheckedUnitPlanGap {
        use crate::execution::terminal_unit::plan_gaps::{GapSite, plan_gap};
        let site = match event.source {
            language_semantics::PermissionEventSource::Statement { statement_index }
            | language_semantics::PermissionEventSource::Call {
                statement_index, ..
            } => u32::try_from(statement_index)
                .ok()
                .map(|index| GapSite::Statement {
                    state: event.state_symbol,
                    index,
                }),
            _ => None,
        }
        .unwrap_or(GapSite::State(event.state_symbol));
        plan_gap(
            program,
            event.machine_symbol,
            site,
            checked_trees::CheckedUnitPlanGapReason::Unsupported(std::borrow::Cow::Borrowed(
                message,
            )),
        )
    }

    fn unsupported(
        program: &dyn validation::CheckingInput,
        event: &FlowPermissionEventFact,
        message: &'static str,
    ) -> diagnostics::Diagnostic {
        let gap = source_gap(program, event, message);
        diagnostics::Diagnostic::incomplete(
            message,
            gap.coverage.exceeded(gap.source_span.span.start as u64),
        )
        .with_source_span(gap.source_span)
    }
}
mod selected_evaluation {

    /*
        Scratch custody follows the expression occurrence schedule. Each decision
        keeps one entry and one completed sibling; reconvergence joins immediately,
        so nested selections never enumerate combinations of execution paths.
        Production and replay use the same routes, but replay consumes recorded
        identities rather than minting replacement claims.
    */
    use crate::checks::ownership_occurrences::StatementOccurrence;
    use crate::checks::ownership_occurrences::{
        EvaluationBranch, StatementStep, statement_steps_at_evaluation,
    };
    use checked_trees::{
        CheckedOperatorFacts, FlowCallFact, FlowPermissionEvaluation, FlowPermissionEventFact,
    };
    use diagnostics::Diagnostic;
    use language_semantics::{Multiplicity, PermissionEventSource};
    use typed_trees::expression::ExpressionHandle;

    mod owned_cases {
        //! Keep branch cleanup tied to the source value and storage version selected.

        use super::place_bindings::SelectionSite;
        use crate::checks::multiplicity::cleanup_dispositions::RemainingCustody;
        use crate::checks::ownership_occurrences::{EvaluationBranch, StatementOccurrence};
        use arena::HandleSpan;
        use checked_trees::{FlowControlFacts, FlowOwnedCaseSelection};
        use std::collections::{BTreeMap, BTreeSet};
        use std::sync::Arc;

        type ArmKey = (u32, u32, u32, u32);

        struct PreparedCases {
            paths: Vec<(symbols::SymbolHandle, validation::CanonicalPlace)>,
            by_variant: BTreeMap<(u32, u32), usize>,
        }

        struct PreparedOwner {
            root: facts::PlaceRoot,
            path: HandleSpan<facts::PlaceSegment>,
            cases: Arc<PreparedCases>,
        }

        struct Selection {
            row: FlowOwnedCaseSelection,
            cases: Arc<PreparedCases>,
            selected: usize,
        }

        pub(super) struct OwnedCaseSelections {
            arms: BTreeMap<ArmKey, Option<Selection>>,
        }

        impl OwnedCaseSelections {
            pub(super) fn new(
                program: &dyn validation::CheckingInput,
                control: &FlowControlFacts,
                site: SelectionSite,
                occurrences: &[StatementOccurrence],
            ) -> Self {
                let mut arms = BTreeMap::new();
                let Some(state) = control
                    .unique_state_by_symbol(site.state)
                    .filter(|state| state.machine_symbol == site.machine)
                else {
                    return Self { arms };
                };
                let Some(statements) = control.statements.span(state.statements) else {
                    return Self { arms };
                };
                // The state records execution order, not source ordinals. Locate the
                // unique original statement before reading its published selections.
                let mut matching = state
                    .statements
                    .handles()
                    .zip(statements)
                    .filter(|(_, statement)| statement.statement_index == site.statement);
                let Some((statement_handle, statement)) = matching.next() else {
                    return Self { arms };
                };
                if matching.next().is_some() {
                    return Self { arms };
                }
                let Some(selections) = control
                    .owned_case_selections
                    .span(statement.owned_case_selections)
                else {
                    return Self { arms };
                };
                let visited = occurrences
                    .iter()
                    .flat_map(|occurrence| &occurrence.route)
                    .filter(|branch| {
                        branch.expression().is_valid() && branch.source_arm().is_valid()
                    })
                    .map(|branch| {
                        let expression = branch.expression();
                        (expression.arena_index(), expression.generation())
                    })
                    .collect::<BTreeSet<_>>();
                // Group only this statement's actual rows once. The stable sort keeps
                // their producing order within each expression, as the old grouping did.
                let mut rows = statement
                    .owned_case_selections
                    .handles()
                    .zip(selections)
                    .filter_map(|(handle, row)| {
                        (row.statement == statement_handle
                            && visited.contains(&(
                                row.expression.arena_index(),
                                row.expression.generation(),
                            )))
                        .then_some(handle)
                    })
                    .collect::<Vec<_>>();
                rows.sort_by_key(|handle| {
                    let row = control.owned_case_selections.get(*handle);
                    (row.expression.arena_index(), row.expression.generation())
                });
                let mut owner: Option<PreparedOwner> = None;
                let mut current_expression = None;
                for handle in rows {
                    let row = *control.owned_case_selections.get(handle);
                    if row.machine_symbol != site.machine
                        || row.state_symbol != site.state
                        || row.statement_index != site.statement
                        || row.evaluation != site.evaluation
                    {
                        continue;
                    }
                    let expression = row.expression;
                    let expression_key = (expression.arena_index(), expression.generation());
                    if current_expression != Some(expression_key) {
                        owner = None;
                        current_expression = Some(expression_key);
                    }
                    let key = (
                        expression.arena_index(),
                        expression.generation(),
                        row.source_arm.arena_index(),
                        row.source_arm.generation(),
                    );
                    let selected = (|| {
                        if !control.operands.is_valid(row.subject)
                            || !control.storage_snapshots.is_valid(row.captured_storage)
                            || !control.storage_snapshots.is_valid(row.selected_storage)
                        {
                            return None;
                        }
                        let subject = control.operands.get(row.subject);
                        if subject.state_symbol != site.state
                            || subject.statement_index != site.statement
                            || subject.evaluation != site.evaluation
                        {
                            return None;
                        }
                        let cases = match &owner {
                            Some(prepared)
                                if prepared.root == row.root && prepared.path == row.path =>
                            {
                                Arc::clone(&prepared.cases)
                            }
                            _ => {
                                let path = control.operand_referent_segments.span(row.path)?;
                                let place = validation::CanonicalPlace {
                                    root: row.root,
                                    segments: path.to_vec(),
                                };
                                let paths = RemainingCustody::owned_case_paths(
                                    program,
                                    site.state,
                                    site.statement,
                                    &place,
                                )?;
                                let by_variant = paths
                                    .iter()
                                    .enumerate()
                                    .map(|(ordinal, (variant, _))| {
                                        ((variant.arena_index(), variant.generation()), ordinal)
                                    })
                                    .collect();
                                let cases = Arc::new(PreparedCases { paths, by_variant });
                                owner = Some(PreparedOwner {
                                    root: row.root,
                                    path: row.path,
                                    cases: Arc::clone(&cases),
                                });
                                cases
                            }
                        };
                        let selected = *cases
                            .by_variant
                            .get(&(row.variant.arena_index(), row.variant.generation()))?;
                        Some(Selection {
                            row,
                            cases,
                            selected,
                        })
                    })();
                    arms.entry(key)
                        .and_modify(|previous: &mut Option<Selection>| {
                            let identical = previous.as_ref().zip(selected.as_ref()).is_some_and(
                                |(left, right)| {
                                    left.row == right.row
                                        && left.selected == right.selected
                                        && Arc::ptr_eq(&left.cases, &right.cases)
                                },
                            );
                            if !identical {
                                *previous = None;
                            }
                        })
                        .or_insert(selected);
                }
                Self { arms }
            }

            pub(super) fn observe(
                &self,
                branch: EvaluationBranch,
                remaining: &mut RemainingCustody,
            ) {
                let expression = branch.expression();
                let arm = branch.source_arm();
                let key = (
                    expression.arena_index(),
                    expression.generation(),
                    arm.arena_index(),
                    arm.generation(),
                );
                let Some(Some(selection)) = self.arms.get(&key) else {
                    return;
                };
                if branch.selected() {
                    for (ordinal, (_, path)) in selection.cases.paths.iter().enumerate() {
                        if ordinal != selection.selected {
                            remaining.exclude_absent_case(path);
                        }
                    }
                } else if selection.row.miss_excludes_case {
                    // Only a total case test can show that this payload is absent.
                    // A failed guard still leaves that case's cleanup obligations live.
                    remaining.exclude_absent_case(&selection.cases.paths[selection.selected].1);
                }
            }
        }
    }
    pub(super) mod place_bindings {

        /*
            A selected mutation leaves live storage, independently of the expression's
            returned value. Bind that storage's arriving obligations by exact place.
            Production allocates destination names; replay authenticates both original
            arrivals. Neither path equates their lineage or repairs an absent owner.
        */
        use super::{SelectedEvaluation, join_places};
        use crate::checks::multiplicity::cleanup_dispositions::RemainingCustody;
        use crate::checks::multiplicity::linear_obligations::{
            ClaimIdentityAllocator, LinearPlace,
        };
        use crate::checks::ownership_occurrences::EvaluationBranch;
        use checked_trees::{
            FlowOwnershipFacts, FlowPermissionEvaluation, FlowSelectionPlaceArrival,
            FlowSelectionPlaceBinding,
        };
        use diagnostics::Diagnostic;
        use language_semantics::{
            PermissionClaimIdentity, PermissionEventSource, PermissionProvenance,
        };
        use symbols::SymbolHandle;

        #[derive(Clone, Copy)]
        pub(in crate::checks::multiplicity) struct SelectionSite {
            pub machine: SymbolHandle,
            pub state: SymbolHandle,
            pub statement: usize,
            pub evaluation: FlowPermissionEvaluation,
        }

        #[derive(Clone, Copy)]
        pub(in crate::checks::multiplicity) struct SelectionEntry<'routes> {
            pub site: SelectionSite,
            pub routes: &'routes super::ExpressionRoutes,
        }

        fn arrival(place: &LinearPlace) -> Result<FlowSelectionPlaceArrival, Diagnostic> {
            let (Some(claim_identity), Some(provenance)) = (place.claim_identity, place.provenance)
            else {
                return Err(Diagnostic::error(format!(
                    "selected live place `{}` has no exact arriving claim",
                    place.name
                )));
            };
            if claim_identity == PermissionClaimIdentity::Unknown
                || provenance == PermissionProvenance::Unknown
            {
                return Err(Diagnostic::error(
                    "selected storage cannot bind an unknown arriving claim",
                ));
            }
            Ok(FlowSelectionPlaceArrival {
                claim_identity,
                provenance,
            })
        }

        fn alternatives(
            branch: EvaluationBranch,
            current: &LinearPlace,
            sibling: &LinearPlace,
        ) -> Result<(FlowSelectionPlaceArrival, FlowSelectionPlaceArrival), Diagnostic> {
            let current = arrival(current)?;
            let sibling = arrival(sibling)?;
            Ok(if branch.selected() {
                (current, sibling)
            } else {
                (sibling, current)
            })
        }

        pub(in crate::checks::multiplicity) fn enter_produced(
            selected: &mut SelectedEvaluation<(Vec<LinearPlace>, RemainingCustody)>,
            route: &[EvaluationBranch],
            current: &mut (Vec<LinearPlace>, RemainingCustody),
            entry: SelectionEntry<'_>,
            ownership: &mut FlowOwnershipFacts,
            identities: &mut ClaimIdentityAllocator,
            mut leave: impl FnMut(
                EvaluationBranch,
                &mut (Vec<LinearPlace>, RemainingCustody),
                &mut FlowOwnershipFacts,
            ) -> Result<(), Diagnostic>,
        ) -> Result<(), Diagnostic> {
            let site = entry.site;
            // Cleanup may use ownership's segment arena at each departing scope.
            // Stage join bindings until that ordered walk completes so both
            // operations have one visible mutable owner rather than aliased callbacks.
            let mut bindings = Vec::new();
            selected.enter_with(
                route,
                current,
                |branch, current| {
                    entry.routes.observe_owned_case(branch, &mut current.1);
                    Ok(())
                },
                |branch, current| leave(branch, current, ownership),
                |branch, current, sibling| {
                    join_places(&mut current.0, &sibling.0, |place, other| {
                        let (selected, remaining) = alternatives(branch, place, other)?;
                        let source = PermissionEventSource::Statement {
                            statement_index: site.statement,
                        };
                        let claim_identity = identities.mint(site.machine, site.state, source);
                        let PermissionClaimIdentity::Established { ordinal, .. } = claim_identity
                        else {
                            return Err(Diagnostic::error(
                                "selected storage binding has no destination name",
                            ));
                        };
                        let destination = FlowSelectionPlaceArrival {
                            claim_identity,
                            provenance: PermissionProvenance::Joined {
                                machine_symbol: site.machine,
                                state_symbol: site.state,
                                source,
                                ordinal,
                            },
                        };
                        bindings.push((
                            place.path.clone(),
                            FlowSelectionPlaceBinding {
                                machine: site.machine,
                                state: site.state,
                                statement_ordinal: site.statement,
                                evaluation: site.evaluation,
                                expression: branch.expression(),
                                decision: branch.decision(),
                                root: place.root,
                                path: Default::default(),
                                selected,
                                remaining,
                                destination,
                            },
                        ));
                        Ok(destination)
                    })?;
                    current.1.join(&sibling.1);
                    Ok(())
                },
            )?;
            for (path, mut binding) in bindings {
                binding.path = ownership.segments.insert_many(path);
                ownership.selection_place_bindings.insert(binding);
            }
            Ok(())
        }

        pub(in crate::checks::multiplicity) fn join_recorded_places(
            ownership: &FlowOwnershipFacts,
            site: SelectionSite,
            branch: EvaluationBranch,
            current: &mut [LinearPlace],
            sibling: &[LinearPlace],
        ) -> Result<(), Diagnostic> {
            join_places(current, sibling, |place, other| {
                let (selected, remaining) = alternatives(branch, place, other)?;
                let mut candidates =
                    ownership
                        .selection_place_bindings
                        .iter()
                        .filter(|(_, binding)| {
                            binding.machine == site.machine
                                && binding.state == site.state
                                && binding.statement_ordinal == site.statement
                                && binding.evaluation == site.evaluation
                                && binding.expression == branch.expression()
                                && binding.decision == branch.decision()
                                && binding.root == place.root
                                && ownership.segments.span_or_empty(binding.path) == place.path
                        });
                let Some((_, binding)) = candidates.next() else {
                    return Err(Diagnostic::error(format!(
                        "selected live place `{}` has no recorded destination binding",
                        place.name
                    )));
                };
                if candidates.next().is_some()
                    || binding.selected != selected
                    || binding.remaining != remaining
                    || !destination_matches_owner(binding)
                {
                    return Err(Diagnostic::error(format!(
                        "selected storage binding for `{}` disagrees with its exact arrivals",
                        place.name
                    )));
                }
                Ok(binding.destination)
            })
        }

        fn destination_matches_owner(binding: &FlowSelectionPlaceBinding) -> bool {
            if binding.selected == binding.remaining {
                return false;
            }
            let PermissionClaimIdentity::Established {
                machine_symbol,
                state_symbol,
                source,
                ordinal,
            } = binding.destination.claim_identity
            else {
                return false;
            };
            machine_symbol == binding.machine
                && state_symbol == binding.state
                && source
                    == PermissionEventSource::Statement {
                        statement_index: binding.statement_ordinal,
                    }
                && binding.destination.provenance
                    == PermissionProvenance::Joined {
                        machine_symbol,
                        state_symbol,
                        source,
                        ordinal,
                    }
                && binding.destination.claim_identity != binding.selected.claim_identity
                && binding.destination.claim_identity != binding.remaining.claim_identity
        }

        /*
            Replay of the selected paths authenticates the arrivals. Later transfers
            must still resolve the exact destination, not merely its statement number.
        */
        pub(in crate::checks::multiplicity) fn claim_origin(
            ownership: &FlowOwnershipFacts,
            identity: PermissionClaimIdentity,
            provenance: PermissionProvenance,
        ) -> bool {
            let mut candidates =
                ownership
                    .selection_place_bindings
                    .iter()
                    .filter(|(_, binding)| {
                        binding.selected != binding.remaining
                            && binding.destination.claim_identity == identity
                            && binding.destination.provenance == provenance
                    });
            candidates
                .next()
                .is_some_and(|(_, binding)| destination_matches_owner(binding))
                && candidates.next().is_none()
        }
    }

    pub(crate) struct ExpressionRoutes {
        completed: Vec<(ExpressionHandle, Vec<EvaluationBranch>)>,
        owned_cases: owned_cases::OwnedCaseSelections,
    }

    impl ExpressionRoutes {
        pub(super) fn from_steps(
            program: &dyn validation::CheckingInput,
            occurrences: &[StatementOccurrence],
            control: &checked_trees::FlowControlFacts,
            site: place_bindings::SelectionSite,
        ) -> Self {
            let completed = occurrences
                .iter()
                .filter_map(|occurrence| match occurrence.step {
                    StatementStep::Complete(expression) => {
                        Some((expression, occurrence.route.clone()))
                    }
                    _ => None,
                })
                .collect();
            Self {
                completed,
                owned_cases: owned_cases::OwnedCaseSelections::new(
                    program,
                    control,
                    site,
                    occurrences,
                ),
            }
        }

        pub(super) fn new(
            program: &dyn validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            statement_index: usize,
            evaluation: FlowPermissionEvaluation,
            operators: &CheckedOperatorFacts,
            control: &checked_trees::FlowControlFacts,
        ) -> Self {
            let statement =
                &program.statement_table.statements(state.statement_nodes)[statement_index];
            let occurrences = statement_steps_at_evaluation(
                program,
                machine,
                state,
                statement_index,
                statement,
                &[],
                operators,
                Some(evaluation),
            );
            Self::from_steps(
                program,
                &occurrences,
                control,
                place_bindings::SelectionSite {
                    machine: machine.symbol,
                    state: state.symbol,
                    statement: statement_index,
                    evaluation,
                },
            )
        }

        pub(super) fn observe_owned_case(
            &self,
            branch: EvaluationBranch,
            remaining: &mut super::cleanup_dispositions::RemainingCustody,
        ) {
            self.owned_cases.observe(branch, remaining);
        }

        pub(super) fn expression(&self, expression: ExpressionHandle) -> &[EvaluationBranch] {
            self.completed
                .iter()
                .find(|(candidate, _)| *candidate == expression)
                .map_or(&[], |(_, route)| route)
        }

        pub(super) fn event(
            &self,
            event: &FlowPermissionEventFact,
            calls: &[FlowCallFact],
        ) -> &[EvaluationBranch] {
            if event.expression.is_valid() {
                return self.expression(event.expression);
            }
            if let PermissionEventSource::Call {
                statement_index,
                call_ordinal,
                target_symbol,
            } = event.source
                && let Some(call) = calls.iter().find(|call| {
                    call.statement_index == statement_index
                        && call.call_ordinal == call_ordinal
                        && call.target_symbol == target_symbol
                })
            {
                return self.expression(call.authored_expression);
            }
            &[]
        }
    }

    /*
        Branch scratch belongs to the common occurrence schedule. Permission
        production and borrow replay use the same immediate reconvergence rule.
    */
    pub(super) use crate::checks::ownership_occurrences::SelectedEvaluation;

    pub(super) fn join_places(
        current: &mut [super::linear_obligations::LinearPlace],
        sibling: &[super::linear_obligations::LinearPlace],
        mut bind: impl FnMut(
            &super::linear_obligations::LinearPlace,
            &super::linear_obligations::LinearPlace,
        ) -> Result<checked_trees::FlowSelectionPlaceArrival, Diagnostic>,
    ) -> Result<(), Diagnostic> {
        if current.len() != sibling.len() {
            return Err(Diagnostic::error(
                "expression selection has different storage rosters",
            ));
        }
        for (place, other) in current.iter_mut().zip(sibling) {
            if place.root != other.root
                || place.path != other.path
                || place.multiplicity != other.multiplicity
            {
                return Err(Diagnostic::error(
                    "expression selection has incompatible typed places",
                ));
            }
            if place.multiplicity == Multiplicity::Linear
                && (place.live || other.live)
                && (place.ever_established != other.ever_established
                    || place.claim_identity != other.claim_identity
                    || place.provenance != other.provenance)
            {
                if !(place.live
                    && other.live
                    && place.ever_established
                    && other.ever_established
                    && !place.case_excluded
                    && !other.case_excluded)
                {
                    return Err(Diagnostic::error(format!(
                        "expression selection has incompatible live linear custody for `{}`",
                        place.name
                    )));
                }
                let destination = bind(place, other)?;
                place.claim_identity = Some(destination.claim_identity);
                place.provenance = Some(destination.provenance);
            }
            place.live &= other.live;
            place.ever_established &= other.ever_established;
            place.case_excluded &= other.case_excluded;
        }
        Ok(())
    }
}
pub(crate) use claim_outcomes::ClaimOutcomePreparation;
pub(crate) use owned_transfer_paths::publish_owned_transfer_paths;
pub(crate) use permission_events::{PermissionWalkMemory, WalkInputs, prepare_permission_events};
mod projected_affine {
    /*
        A projected affine move transfers a subtree, not its still-live root.
    */
    use crate::checks::multiplicity::linear_obligations::LinearPlace;
    use crate::checks::multiplicity::linear_obligations::type_reference_is_reference;
    use crate::checks::multiplicity::linear_validation::event_statement_index;
    use crate::checks::multiplicity::linear_validation::permission_source;
    use crate::checks::type_multiplicity;
    use checked_trees::FlowPermissionEventFact;
    use language_semantics::Multiplicity;
    use language_semantics::PermissionAccess;
    use language_semantics::PermissionClaimIdentity;
    use language_semantics::PermissionEventKind;
    use language_semantics::PermissionProvenance;
    use symbols::SymbolHandle;
    use typed_trees::types::TypeReferenceNode;

    #[allow(clippy::too_many_arguments)]
    pub(super) fn append_transfer(
        program: &dyn ::validation::CheckingInput,
        _facts: checked_trees::CheckFactsRef<'_>,
        machine: SymbolHandle,
        state: SymbolHandle,
        event: &crate::flow::DiscoveredMoveEvent,
        path: &[facts::PlaceSegment],
        places: &[LinearPlace],
        remaining: &super::cleanup_dispositions::RemainingCustody,
        permissions: &mut Vec<FlowPermissionEventFact>,
    ) -> Result<(), diagnostics::Diagnostic> {
        let Some(statement_index) = event_statement_index(event.source) else {
            return Ok(());
        };
        if path.is_empty()
            || !path.iter().all(crate::flow::fixed_selection_segment)
            || program.state_by_symbol(state).is_none()
        {
            return Ok(());
        }
        let owners = places
            .iter()
            .filter(|place| place.root == event.root)
            .collect::<Vec<_>>();
        if owners.iter().any(|owner| owner.path == path)
            || !(owners.iter().any(|owner| owner.ever_established)
                || remaining.has_storage_frontier(event.root))
        {
            return Ok(());
        }
        let provenance = owners
            .iter()
            .find(|owner| owner.path.is_empty() && owner.multiplicity == Multiplicity::Affine)
            .and_then(|owner| owner.provenance)
            .unwrap_or(PermissionProvenance::Unknown);
        let root = validation::CanonicalPlace {
            root: event.root,
            segments: Vec::new(),
        };
        let projected = validation::CanonicalPlace {
            root: event.root,
            segments: path.to_vec(),
        };
        let affine = |place: &validation::CanonicalPlace| {
            validation::canonical_place_type_reference(program, state, statement_index, place)
                .is_some_and(|mut type_reference| {
                    while let TypeReferenceNode::Constrained { base_type, .. } =
                        program.type_reference_table.type_reference(type_reference)
                    {
                        type_reference = *base_type;
                    }
                    matches!(
                        program.type_reference_table.type_reference(type_reference),
                        TypeReferenceNode::Named { .. }
                            | TypeReferenceNode::Generic { .. }
                            | TypeReferenceNode::FixedArray { .. }
                    ) && type_multiplicity(program, type_reference) == Multiplicity::Affine
                })
        };
        if !affine(&root)
            || !affine(&projected)
            || program.state_by_symbol(state).is_none_or(|declaration| {
                is_borrowed_place_transfer(program, machine, declaration, event, path)
            })
        {
            return Ok(());
        }
        if !remaining.is_present(event.root, path) {
            if remaining.crosses_inactive_case(event.root, path) {
                return Ok(());
            }
            if let Some(consumed) = remaining.consumed_place(event.root, path)
                && let Some(machine) = program.machine_by_symbol(machine)
                && let Some(state) = program.state_by_symbol(state)
            {
                let spelling = super::borrowed_windows::place_spelling(program, event.root, path);
                let consumed_spelling = super::borrowed_windows::place_spelling(
                    program,
                    consumed.root,
                    &consumed.segments,
                );
                return Err(diagnostics::Diagnostic::error(
                    super::consumed_owners::consumed_use_message(
                        machine.name.as_str(),
                        state.name.as_str(),
                        statement_index,
                        &spelling,
                        &consumed_spelling,
                        None,
                    ),
                ));
            }
            return Err(diagnostics::Diagnostic::error(format!(
                "projected owned storage is not established in machine `{}` state `{}` at statement {}",
                program.symbols.name(machine),
                program.symbols.name(state),
                statement_index,
            )));
        }
        permissions.push(FlowPermissionEventFact {
            expression: event.expression,
            evaluation: checked_trees::FlowPermissionEvaluation::Statement,
            machine_symbol: machine,
            state_symbol: state,
            source: permission_source(event.source),
            kind: PermissionEventKind::Transfer,
            multiplicity: Multiplicity::Affine,
            access: PermissionAccess::Owned,
            claim_identity: PermissionClaimIdentity::Unknown,
            provenance,
            root: event.root,
            segments: event.segments,
            obligation_live: false,
        });
        Ok(())
    }

    /*
        Copying an unrestricted value observes its source without detaching storage.
        Affine reference carriers still transfer their descriptor custody, separately
        from whether their referent is owned by this place.
    */
    pub(super) fn transfers_value_custody(
        program: &dyn ::validation::CheckingInput,
        state: SymbolHandle,
        statement_index: usize,
        event: &crate::flow::DiscoveredMoveEvent,
        path: &[facts::PlaceSegment],
    ) -> bool {
        selected_value_type(program, state, statement_index, event, path).is_some_and(|reference| {
            program.stored_value_multiplicity(reference) != Multiplicity::Unrestricted
        })
    }

    /*
        Loan exclusion and borrowed-window opening distinguish an owned value from
        a reference descriptor. Moving the descriptor does not detach its referent.
    */
    pub(in crate::checks) fn transfers_owned_storage(
        program: &dyn ::validation::CheckingInput,
        state: SymbolHandle,
        statement_index: usize,
        event: &crate::flow::DiscoveredMoveEvent,
        path: &[facts::PlaceSegment],
    ) -> bool {
        selected_value_type(program, state, statement_index, event, path).is_some_and(|reference| {
            type_multiplicity(program, reference) != Multiplicity::Unrestricted
                && !type_reference_is_reference(program, reference)
        })
    }

    fn selected_value_type(
        program: &dyn ::validation::CheckingInput,
        state: SymbolHandle,
        statement_index: usize,
        event: &crate::flow::DiscoveredMoveEvent,
        path: &[facts::PlaceSegment],
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        event
            .value_type
            .is_valid()
            .then_some(event.value_type)
            .or_else(|| {
                validation::canonical_place_type_reference(
                    program,
                    state,
                    statement_index,
                    &validation::CanonicalPlace {
                        root: event.root,
                        segments: path.to_vec(),
                    },
                )
            })
    }

    /*
        Projection identifies a subtree, but grants no ownership of a borrowed
        referent. Pattern payloads and ordinary fields obey the same reference-prefix
        checks; moving a reference carrier itself does not move its referent.
    */
    pub(super) fn is_borrowed_place_transfer(
        program: &dyn ::validation::CheckingInput,
        _machine: SymbolHandle,
        state: &typed_trees::state::State,
        event: &crate::flow::DiscoveredMoveEvent,
        path: &[facts::PlaceSegment],
    ) -> bool {
        let statement_index = event_statement_index(event.source).unwrap_or(0);
        if !transfers_owned_storage(program, state.symbol, statement_index, event, path) {
            return false;
        }
        has_reference_prefix(
            program,
            state.symbol,
            statement_index,
            &validation::CanonicalPlace {
                root: event.root,
                segments: path.to_vec(),
            },
        )
    }

    /*
        A reference prefix selects storage owned elsewhere, including the exact
        referent at an empty root path. The selected value's carrier is classified
        separately so forwarding a descriptor never becomes referent ownership.
    */
    pub(super) fn has_reference_prefix(
        program: &dyn ::validation::CheckingInput,
        state: SymbolHandle,
        statement_index: usize,
        place: &validation::CanonicalPlace,
    ) -> bool {
        (0..=place.segments.len()).any(|length| {
            let prefix = validation::CanonicalPlace {
                root: place.root,
                segments: place.segments[..length].to_vec(),
            };
            validation::canonical_place_type_reference(program, state, statement_index, &prefix)
                .is_some_and(|reference| type_reference_is_reference(program, reference))
        })
    }
}
mod projected_replacements {
    /*
        Replacement of a captured owned subtree after RHS evaluation has joined.

        Source multiplicity and stored descriptor cleanup are independent: copying
        a shared reference creates no affine source claim, but replacing its slot
        still ends that exact runtime carrier. Publish its current custody here,
        beside ordinary affine replacement, rather than reviving an entry loan or
        guessing which RHS spelling last wrote the slot. Cleanup consumers reuse the
        captured destination and retain the containing home.
    */
    use super::cleanup_dispositions::RemainingCustody;
    use super::linear_obligations::LinearPlace;
    use super::linear_obligations::type_reference_is_reference;
    use crate::checks::type_multiplicity;
    use crate::published::places::place_at;
    use checked_trees::{
        ContextualCleanupDisposition, ContextualCleanupFacts, FlowPermissionEventFact,
    };
    use language_semantics::{
        Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
        PermissionProvenance,
    };
    use symbols::SymbolHandle;
    use typed_trees::types::TypeReferenceNode;

    /*
        A projected store replaces one owned subtree, independently of
        whether that subtree was extracted earlier. It does not establish or
        consume the containing root's linear/affine claim.
    */
    #[allow(clippy::too_many_arguments)]
    pub(super) fn append_replacement(
        program: &dyn ::validation::CheckingInput,
        facts: &mut checked_trees::CheckFactsWalk<'_>,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement_index: usize,
        statement: &typed_trees::statement::StatementNode,
        places: &[LinearPlace],
        remaining: &super::cleanup_dispositions::RemainingCustody,
        permissions: &mut Vec<FlowPermissionEventFact>,
        cleanup: &mut ContextualCleanupFacts,
    ) -> Result<Vec<super::cleanup_dispositions::StorageEstablishment>, diagnostics::Diagnostic>
    {
        let Some(assignment) = statement.as_assignment() else {
            return Ok(Vec::new());
        };
        let Some(place) = place_at(program, state, statement_index, assignment.target) else {
            return Ok(Vec::new());
        };
        let destinations = if let Some(indexed) =
            program.expression_table.indexed_node(assignment.target)
            && program.expression_table.range_node(indexed.index).is_some()
        {
            let (Some(machine_definition), Some(state_definition)) = (
                program.machine_by_symbol(machine),
                program.state_by_symbol(state),
            ) else {
                return Ok(Vec::new());
            };
            let Some(collection) = place_at(program, state, statement_index, indexed.collection)
            else {
                return Ok(Vec::new());
            };
            let Some(collection_type) = validation::canonical_place_type_reference(
                program,
                state,
                statement_index,
                &collection,
            )
            .and_then(|reference| validation::unwrapped_type_reference(program, reference)) else {
                return Ok(Vec::new());
            };
            let TypeReferenceNode::FixedArray {
                element_type,
                length: typed_trees::types::FixedArrayLength::Literal(length),
            } = program.type_reference_table.type_reference(collection_type)
            else {
                return Ok(Vec::new());
            };
            let endpoint = *element_type;
            let descriptor =
                validation::reference_result_custody::stored_loan_parts(program, endpoint)
                    .is_some();
            let frontier = remaining.has_storage_frontier(collection.root);
            let has_owner = places.iter().any(|owner| {
                owner.root == collection.root
                    && owner.path.is_empty()
                    && (owner.live || frontier)
                    && owner.multiplicity == Multiplicity::Affine
            });
            let affine =
                type_multiplicity(program, endpoint) == Multiplicity::Affine && !descriptor;
            if !descriptor
                && ((!has_owner && !frontier)
                    || type_reference_is_reference(program, endpoint)
                    || type_multiplicity(program, endpoint) == Multiplicity::Linear
                    || (!affine && !frontier))
            {
                return Ok(Vec::new());
            }
            let mut bounds = validation::ImmutableBoundLookup::new(program);
            let Some(shape) = validation::array_window_shape(
                program,
                machine_definition,
                state_definition,
                assignment.target,
                &mut bounds,
            ) else {
                return Ok(Vec::new());
            };
            let (Some(start), Some(end)) = (shape.start, shape.end) else {
                return Ok(Vec::new());
            };
            if start > end || end > *length {
                return Ok(Vec::new());
            }
            (start..end)
                .map(|index| {
                    let mut segments = collection.segments.clone();
                    segments.push(facts::PlaceSegment::FixedIndex { index });
                    validation::CanonicalPlace {
                        root: collection.root,
                        segments,
                    }
                })
                .collect()
        } else {
            vec![place]
        };
        Ok(destinations
            .into_iter()
            .map(|place| {
                append_place_replacement(
                    program,
                    facts,
                    machine,
                    state,
                    statement_index,
                    place,
                    places,
                    remaining,
                    permissions,
                    cleanup,
                )
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect())
    }

    #[allow(clippy::too_many_arguments)]
    fn append_place_replacement(
        program: &dyn ::validation::CheckingInput,
        facts: &mut checked_trees::CheckFactsWalk<'_>,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement_index: usize,
        place: validation::CanonicalPlace,
        places: &[LinearPlace],
        remaining: &super::cleanup_dispositions::RemainingCustody,
        permissions: &mut Vec<FlowPermissionEventFact>,
        cleanup: &mut ContextualCleanupFacts,
    ) -> Result<Option<super::cleanup_dispositions::StorageEstablishment>, diagnostics::Diagnostic>
    {
        let Some(symbol) = place.root.as_symbol() else {
            return Ok(None);
        };
        let owner = places.iter().find(|owner| {
            owner.root == facts::PlaceRoot::Symbol(symbol)
                && owner.path.is_empty()
                && (owner.live || remaining.has_storage_frontier(place.root))
                && owner.multiplicity == Multiplicity::Affine
        });
        let Some(endpoint) =
            validation::canonical_place_type_reference(program, state, statement_index, &place)
        else {
            return Ok(None);
        };
        let descriptor =
            validation::reference_result_custody::stored_loan_parts(program, endpoint).is_some();
        let affine = type_multiplicity(program, endpoint) == Multiplicity::Affine && !descriptor;
        if descriptor && place.segments.is_empty() {
            return Ok(None);
        }
        if !descriptor
            && ((owner.is_none() && !remaining.has_storage_frontier(place.root))
                || type_reference_is_reference(program, endpoint)
                || type_multiplicity(program, endpoint) == Multiplicity::Linear)
        {
            return Ok(None);
        }
        if !affine && !descriptor && !remaining.has_storage_frontier(place.root) {
            return Ok(None);
        }
        for length in 0..place.segments.len() {
            let prefix = validation::CanonicalPlace {
                root: place.root,
                segments: place.segments[..length].to_vec(),
            };
            let Some(reference) = validation::canonical_place_type_reference(
                program,
                state,
                statement_index,
                &prefix,
            ) else {
                return Ok(None);
            };
            if type_reference_is_reference(program, reference) {
                return Ok(None);
            }
        }
        if descriptor
            && append_descriptor_death(
                program,
                facts,
                cleanup,
                machine,
                state,
                statement_index,
                &place,
                endpoint,
                remaining,
            )
            .is_none()
        {
            return Ok(None);
        }
        let discarded = if affine {
            remaining.disposal_paths(program, place.root, &place.segments)
        } else {
            Vec::new()
        };
        let source = language_semantics::PermissionEventSource::Statement { statement_index };
        let segments = facts
            .flow
            .ownership
            .segments
            .insert_many(place.segments.iter().copied());
        for path in discarded {
            let discarded = validation::CanonicalPlace {
                root: place.root,
                segments: path,
            };
            let Some(reference) = validation::canonical_place_type_reference(
                program,
                state,
                statement_index,
                &discarded,
            ) else {
                return Err(diagnostics::Diagnostic::error(
                    "replacement cleanup has no exact discarded type",
                ));
            };
            if !super::linear_validation::append_owned_region_cleanup_permission_events(
                program,
                reference,
                discarded.root,
                &discarded.segments,
                places,
                remaining,
                FlowPermissionEventFact {
                    expression: Default::default(),
                    evaluation: checked_trees::FlowPermissionEvaluation::Statement,
                    machine_symbol: machine,
                    state_symbol: state,
                    source,
                    kind: PermissionEventKind::AffineDrop,
                    multiplicity: Multiplicity::Affine,
                    access: PermissionAccess::Owned,
                    claim_identity: PermissionClaimIdentity::Unknown,
                    provenance: PermissionProvenance::Unknown,
                    root: place.root,
                    segments: Default::default(),
                    obligation_live: false,
                },
                &mut facts.flow.ownership.segments,
                permissions,
            ) {
                return Err(diagnostics::Diagnostic::error(
                    "replacement cannot dispose its remaining owned region",
                ));
            }
        }
        let establishment = super::cleanup_dispositions::StorageEstablishment {
            event_ordinal: permissions.len(),
            prior_value_transferred: remaining.definitely_transferred(place.root, &place.segments),
            destination: place,
        };
        permissions.push(FlowPermissionEventFact {
            expression: Default::default(),
            evaluation: checked_trees::FlowPermissionEvaluation::Statement,
            machine_symbol: machine,
            state_symbol: state,
            source,
            kind: PermissionEventKind::Establish,
            multiplicity: if affine {
                Multiplicity::Affine
            } else {
                Multiplicity::Unrestricted
            },
            access: PermissionAccess::Owned,
            claim_identity: PermissionClaimIdentity::Unknown,
            provenance: PermissionProvenance::Established {
                machine_symbol: machine,
                state_symbol: state,
                source,
            },
            root: establishment.destination.root,
            segments,
            obligation_live: false,
        });
        Ok(Some(establishment))
    }

    /*
        A stored loan's death ends a carrier, not an affine source obligation.
        The current custody is already joined after all RHS calls. In particular,
        an exclusive call may have replaced the ingress descriptor; its old loan
        identity is neither needed nor valid authority for this slot's current death.
    */
    #[allow(clippy::too_many_arguments)]
    fn append_descriptor_death(
        program: &dyn validation::CheckingInput,
        facts: &mut checked_trees::CheckFactsWalk<'_>,
        cleanup: &mut ContextualCleanupFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement_index: usize,
        place: &validation::CanonicalPlace,
        endpoint: typed_trees::types::TypeReferenceHandle,
        remaining: &RemainingCustody,
    ) -> Option<()> {
        let declaration = program.state_by_symbol(state)?;
        let symbol = place.root.as_symbol()?;
        if let Some(parameter) = program.state_parameter_by_symbol(declaration, symbol) {
            if !validation::reference_result_custody::stored_carrier_ingress_is_complete(
                program,
                facts.view(),
                machine,
                state,
                symbol,
                parameter.type_reference,
            ) {
                return None;
            }
        } else {
            let (ordinal, local) = program.state_local_declaration(declaration, symbol)?;
            if ordinal >= statement_index
                || (!local.initial_value.is_valid() && !remaining.has_storage_frontier(place.root))
            {
                return None;
            }
        }
        let assignment = program
            .statement_table
            .statement_at(declaration, statement_index)?
            .as_assignment()?;
        let identity = validation::stored_structural_type_identity(program, endpoint, &[], &[]);
        if !identity.is_valid()
            || identity
                != validation::stored_structural_type_identity(
                    program,
                    assignment.receiving_type,
                    &[],
                    &[],
                )
        {
            return None;
        }
        if !remaining
            .removed_paths(program, place.root, &place.segments)
            .is_empty()
        {
            return None;
        }
        let mut disposition = ContextualCleanupDisposition {
            machine,
            state,
            source: language_semantics::PermissionEventSource::Statement { statement_index },
            root: place.root,
            disposed_type: endpoint,
            path: facts
                .flow
                .ownership
                .segments
                .insert_many(place.segments.iter().copied()),
            ..Default::default()
        };
        {
            let evidence =
                crate::flow::CleanupCaseEvidence::new(program, facts.view(), &disposition)?;
            if evidence.reachability(&place.segments)?
                != validation::CleanupCaseReachability::Selected
            {
                return None;
            }
            for excluded in remaining.excluded_paths(place.root) {
                if place.segments.starts_with(excluded) || excluded.starts_with(&place.segments) {
                    return None;
                }
                if crate::checks::storage_path_compatibility(
                    program,
                    crate::checks::StorageSelectorFacts {
                        flow: facts.flow.view(),
                        semantic: facts.semantic,
                    },
                    &place.segments,
                    excluded,
                )
                .0 && !facts
                    .flow
                    .control
                    .state(machine, state)
                    .zip(evidence.contexts().as_ref())
                    .is_some_and(|(state, contexts)| {
                        crate::checks::captured_storage_paths_are_disjoint(
                            program,
                            facts.view(),
                            state,
                            contexts,
                            &place.segments,
                            excluded,
                        )
                    })
                {
                    return None;
                }
            }
        }
        if !remaining.is_present(place.root, &place.segments) {
            return remaining
                .is_construction_vacancy(place.root, &place.segments)
                .then_some(());
        }
        let exclusions = remaining
            .removed_regions(program, place.root, &[])
            .into_iter()
            .map(
                |(path, cause)| checked_trees::ContextualCleanupExcludedRegion {
                    root: place.root,
                    segments: cleanup.segments.insert_many(path.iter().copied()),
                    cause,
                },
            )
            .collect::<Vec<_>>();
        let inactive_cases = remaining
            .inactive_case_paths(place.root, &[])
            .into_iter()
            .map(|path| checked_trees::ContextualCleanupExcludedRegion {
                root: place.root,
                segments: cleanup.segments.insert_many(path.iter().copied()),
                cause: checked_trees::ContextualCleanupExclusionCause::InactiveCase,
            })
            .collect::<Vec<_>>();
        disposition.exclusions = cleanup.exclusions.insert_many(exclusions);
        disposition.inactive_cases = cleanup.exclusions.insert_many(inactive_cases);
        cleanup.dispositions.append(disposition);
        Some(())
    }
}
mod temporary_results {
    /*
        Completed anonymous owners retain their exact source custody through calls,
        borrowed observations and constructed children. A partial transfer keeps its
        remaining partition until the actual normal consumer continuation; every
        remaining linear claim needs its owner-authorized disposition. Borrowing
        lends access without transferring that owner or shortening its lifetime.
    */
    use crate::checks::multiplicity::claim_outcomes::claim_paths_are_case_alternatives;
    use crate::checks::multiplicity::linear_claim_frontier;
    use crate::checks::multiplicity::linear_validation::{
        event_statement_index, permission_source,
    };
    use arena::HandleSpan;
    use checked_trees::CheckFacts;
    use checked_trees::FlowPermissionEventFact;
    use diagnostics::Diagnostic;
    use language_semantics::Multiplicity;
    use language_semantics::PermissionAccess;
    use language_semantics::PermissionClaimIdentity;
    use language_semantics::PermissionEventKind;
    use language_semantics::PermissionProvenance;
    use symbols::SymbolHandle;

    mod borrowed_owners {
        /*
            Borrowed access to an anonymous owner ends before its call cleanup.
            Every contained linear obligation needs an authorized disposition on normal
            completion, including obligations outside a borrowed projection.
        */
        use super::super::owned_cleanup::claim_receipts;
        use super::owners::{TemporaryConsumption, TemporaryOwner};
        use super::{
            FlowPermissionEventFact, HandleSpan, Multiplicity, PermissionAccess,
            PermissionClaimIdentity, PermissionEventKind, PermissionProvenance,
        };
        use diagnostics::Diagnostic;
        use language_semantics::PermissionEventSource;

        pub(in crate::checks::multiplicity) fn append_borrow(
            program: &dyn ::validation::CheckingInput,
            facts: &mut checked_trees::CheckFactsWalk<'_>,
            machine: symbols::SymbolHandle,
            state: symbols::SymbolHandle,
            consumer: PermissionEventSource,
            evaluation: checked_trees::FlowPermissionEvaluation,
            owners: &[TemporaryOwner],
            places: &mut [crate::checks::multiplicity::linear_obligations::LinearPlace],
            permissions: &mut Vec<FlowPermissionEventFact>,
            diagnostics: &mut Vec<Diagnostic>,
            dispositions: &mut checked_trees::ContextualCleanupFacts,
        ) -> Vec<facts::PlaceRoot> {
            let mut retired = Vec::new();
            for (owner, access, borrowed_path) in owners
                .iter()
                .filter_map(|owner| match &owner.consumption {
                    TemporaryConsumption::Borrow {
                        access,
                        borrowed_path,
                    } => Some((owner, *access, borrowed_path)),
                    TemporaryConsumption::OwnedTransfer
                    | TemporaryConsumption::OwnedProjection { .. } => None,
                })
                .filter(|(owner, _, _)| {
                    owner.consumer == consumer && owner.evaluation == evaluation
                })
            {
                let root = facts::PlaceRoot::Expression(owner.expression);
                if claim_receipts(program, owner.type_reference, root, &[], places, &[], &[])
                    .is_none()
                {
                    let boundary = match owner.consumer {
                        PermissionEventSource::Call {
                            target_symbol,
                            statement_index,
                            ..
                        } => format!(
                            "call `{}` at statement {}",
                            program.symbols.name(target_symbol),
                            statement_index
                        ),
                        PermissionEventSource::Statement { statement_index } => {
                            format!("value completion at statement {}", statement_index)
                        }
                        _ => continue,
                    };
                    diagnostics.push(Diagnostic::error(format!(
                        "cannot automatically dispose borrowed temporary `{}` after {}: every contained linear obligation needs its owner-authorized terminal disposition",
                        program.normalized_type_identity_text(owner.type_reference), boundary,
                    )));
                    continue;
                }
                if validation::reference_result_custody::binding_multiplicity(
                    program,
                    owner.type_reference,
                ) == Multiplicity::Unrestricted
                {
                    continue;
                }
                if !places
                    .iter()
                    .any(|place| place.root == root && place.ever_established)
                {
                    diagnostics.push(Diagnostic::error(
                        "unimplemented: conditional temporary owner cleanup needs selected establishment custody",
                    ));
                    continue;
                }
                dispositions
                    .dispositions
                    .append(checked_trees::ContextualCleanupDisposition {
                        selection: Default::default(),
                        establishment_ordered: false,
                        zero_image: false,
                        machine,
                        state,
                        source: owner.consumer,
                        exit: checked_trees::FlowClaimJoinExit::default(),
                        completion: owner.completion,
                        completion_constraints: owner.completion_constraints,
                        root,
                        disposed_type: owner.type_reference,
                        path: HandleSpan::empty(),
                        exclusions: HandleSpan::empty(),
                        definite_exclusions: HandleSpan::empty(),
                        inactive_cases: HandleSpan::empty(),
                    });
                permissions.push(FlowPermissionEventFact {
                    expression: owner.completion,
                    evaluation,
                    machine_symbol: machine,
                    state_symbol: state,
                    source: owner.consumer,
                    kind: PermissionEventKind::Establish,
                    multiplicity: Multiplicity::Unrestricted,
                    access,
                    claim_identity: PermissionClaimIdentity::Unknown,
                    provenance: PermissionProvenance::Established {
                        machine_symbol: machine,
                        state_symbol: state,
                        source: owner.producer,
                    },
                    root,
                    segments: facts
                        .flow
                        .ownership
                        .segments
                        .insert_many(borrowed_path.iter().copied()),
                    obligation_live: false,
                });
                for place in places
                    .iter_mut()
                    .filter(|place| place.root == root && place.live)
                {
                    permissions.push(FlowPermissionEventFact {
                        expression: owner.completion,
                        evaluation,
                        machine_symbol: machine,
                        state_symbol: state,
                        source: owner.consumer,
                        kind: if place.multiplicity == Multiplicity::Linear {
                            PermissionEventKind::Consume
                        } else {
                            PermissionEventKind::AffineDrop
                        },
                        multiplicity: place.multiplicity,
                        access: PermissionAccess::Owned,
                        claim_identity: place
                            .claim_identity
                            .unwrap_or(PermissionClaimIdentity::Unknown),
                        provenance: place.provenance.unwrap_or(PermissionProvenance::Unknown),
                        root,
                        segments: facts
                            .flow
                            .ownership
                            .segments
                            .insert_many(place.path.iter().copied()),
                        obligation_live: false,
                    });
                    place.live = false;
                }
                retired.push(root);
            }
            retired
        }
    }
    mod owned_transfers {
        /*
            A consuming call or construction receives a completed anonymous owner.
            Its producer established the actual claim frontier before this operation.
            Calls transfer it whole; transparent constructors retain each child's
            identity and provenance at its typed receiving path. Neither operation
            settles a child by inventing a replacement claim at its destination.
        */
        use super::owners::{TemporaryConsumption, TemporaryDestination, TemporaryOwner};
        use super::{
            FlowPermissionEventFact, Multiplicity, PermissionAccess, PermissionClaimIdentity,
            PermissionEventKind, PermissionProvenance,
        };
        use language_semantics::PermissionEventSource;

        #[allow(clippy::too_many_arguments)]
        pub(in crate::checks::multiplicity) fn append_owned_transfer(
            facts: &mut checked_trees::CheckFactsWalk<'_>,
            machine: symbols::SymbolHandle,
            state: symbols::SymbolHandle,
            consumer: PermissionEventSource,
            evaluation: checked_trees::FlowPermissionEvaluation,
            completed: typed_trees::expression::ExpressionHandle,
            owners: &[TemporaryOwner],
            written_targets: &mut [(
                super::super::linear_obligations::WrittenLinearTarget,
                PermissionEventSource,
            )],
            places: &mut [crate::checks::multiplicity::linear_obligations::LinearPlace],
            remaining: &mut super::super::cleanup_dispositions::RemainingCustody,
            permissions: &mut Vec<FlowPermissionEventFact>,
            conditional_carrier_transfers: &mut Vec<usize>,
        ) -> Result<(), diagnostics::Diagnostic> {
            for owner in owners.iter().filter(|owner| {
                matches!(owner.consumption, TemporaryConsumption::OwnedTransfer)
                    && owner.consumer == consumer
                    && owner.evaluation == evaluation
                    && match &owner.destination {
                        TemporaryDestination::CallArgument => {
                            matches!(consumer, PermissionEventSource::Call { .. })
                        }
                        TemporaryDestination::ConstructedChild { expression, .. } => {
                            *expression == completed
                        }
                    }
            }) {
                let root = facts::PlaceRoot::Expression(owner.expression);
                let incoming = places
                    .iter()
                    .filter(|place| place.root == root)
                    .collect::<Vec<_>>();
                let completed_value = incoming
                    .iter()
                    .any(|place| place.ever_established && (place.live || place.case_excluded));
                if matches!(
                    owner.destination,
                    TemporaryDestination::ConstructedChild { .. }
                ) && (incoming.is_empty()
                    || incoming.iter().any(|place| !place.ever_established)
                    || !completed_value
                    || incoming.iter().any(|place| {
                        place.live
                            && ((place.multiplicity == Multiplicity::Linear
                                && place.claim_identity.is_none())
                                || place.provenance.is_none())
                    }))
                {
                    return Err(diagnostics::Diagnostic::error(
                        "constructed child has no unique completed incoming custody",
                    ));
                }
                for index in 0..places.len() {
                    if places[index].root != root || !places[index].live {
                        continue;
                    }
                    if let TemporaryDestination::ConstructedChild {
                        expression, path, ..
                    } = &owner.destination
                    {
                        let mut destination = path.clone();
                        destination.extend_from_slice(&places[index].path);
                        let nominal = written_targets.iter().any(|(target, _)| {
                            target.value == *expression
                                && places[target.place_index].multiplicity == Multiplicity::Linear
                                && places[target.place_index].path == target.destination_path
                        });
                        let mut receivers = 0;
                        for (target, _) in written_targets
                            .iter_mut()
                            .filter(|(target, _)| target.value == *expression)
                        {
                            if places[target.place_index]
                                .path
                                .strip_prefix(target.destination_path.as_slice())
                                == Some(destination.as_slice())
                            {
                                receivers += 1;
                                target.claim_identity = places[index].claim_identity;
                                target.provenance = places[index].provenance;
                                target.obligation_live =
                                    places[index].multiplicity == Multiplicity::Linear;
                            }
                        }
                        if places[index].multiplicity == Multiplicity::Linear
                            && !nominal
                            && receivers != 1
                        {
                            return Err(diagnostics::Diagnostic::error(
                                "constructed child claim has no unique typed receiving path",
                            ));
                        }
                    }
                }
                append_root_transfer(
                    facts,
                    machine,
                    state,
                    owner.consumer,
                    evaluation,
                    owner.expression,
                    places,
                    permissions,
                    conditional_carrier_transfers,
                );
                if completed_value {
                    remaining.transfer_storage(root, &[]);
                }
            }
            Ok(())
        }

        #[allow(clippy::too_many_arguments)]
        fn append_root_transfer(
            facts: &mut checked_trees::CheckFactsWalk<'_>,
            machine: symbols::SymbolHandle,
            state: symbols::SymbolHandle,
            consumer: PermissionEventSource,
            evaluation: checked_trees::FlowPermissionEvaluation,
            expression: typed_trees::expression::ExpressionHandle,
            places: &mut [crate::checks::multiplicity::linear_obligations::LinearPlace],
            permissions: &mut Vec<FlowPermissionEventFact>,
            conditional_carrier_transfers: &mut Vec<usize>,
        ) {
            let root = facts::PlaceRoot::Expression(expression);
            for place in places
                .iter_mut()
                .filter(|place| place.root == root && place.live)
            {
                super::super::linear_validation::record_conditional_carrier_transfer(
                    place,
                    &[],
                    permissions.len(),
                    conditional_carrier_transfers,
                );
                permissions.push(FlowPermissionEventFact {
                    expression,
                    evaluation,
                    machine_symbol: machine,
                    state_symbol: state,
                    source: consumer,
                    kind: PermissionEventKind::Transfer,
                    multiplicity: place.multiplicity,
                    access: PermissionAccess::Owned,
                    claim_identity: place
                        .claim_identity
                        .unwrap_or(PermissionClaimIdentity::Unknown),
                    provenance: place.provenance.unwrap_or(PermissionProvenance::Unknown),
                    root,
                    segments: facts
                        .flow
                        .ownership
                        .segments
                        .insert_many(place.path.iter().copied()),
                    obligation_live: place.multiplicity == Multiplicity::Linear,
                });
                place.live = false;
            }
        }

        #[derive(Clone, Copy, PartialEq, Eq)]
        enum ReceivingStorage {
            Place(facts::PlaceHandle),
            EvaluatedTarget(arena::Handle<checked_trees::FlowOperandFact>),
        }

        fn receiving_storage(
            facts: checked_trees::CheckFactsRef<'_>,
            row: &checked_trees::FlowStatementFact,
            state: symbols::SymbolHandle,
            statement: usize,
            evaluation: checked_trees::FlowPermissionEvaluation,
        ) -> Option<ReceivingStorage> {
            if facts.semantic.places.is_valid(row.stored_destination) {
                return Some(ReceivingStorage::Place(row.stored_destination));
            }
            if !facts.flow.control.operands.is_valid(row.store_target) {
                return None;
            }
            let target = facts.flow.control.operands.get(row.store_target);
            if target.state_symbol != state
                || target.statement_index != statement
                || target.evaluation != evaluation
                || !target.expression.is_valid()
            {
                return None;
            }
            let referents = facts
                .flow
                .control
                .operand_referents
                .span(target.referents)?;
            (!referents.is_empty()
                && referents.iter().all(|referent| {
                    referent.root.is_valid()
                        && facts
                            .flow
                            .control
                            .operand_referent_segments
                            .span(referent.segments)
                            .is_some()
                }))
            .then_some(ReceivingStorage::EvaluatedTarget(row.store_target))
        }

        /*
            A retained store receives the actual completed value, including when its
            destination is borrowed storage with no caller-owned claim slot. Named
            receiving production already owns its writes; this settles only a separate
            live expression home, without deriving any claim from receiver annotations.
        */
        #[allow(clippy::too_many_arguments)]
        pub(in crate::checks::multiplicity) fn append_stored_transfer(
            facts: &mut checked_trees::CheckFactsWalk<'_>,
            machine: symbols::SymbolHandle,
            state: symbols::SymbolHandle,
            statement: usize,
            evaluation: checked_trees::FlowPermissionEvaluation,
            places: &mut [crate::checks::multiplicity::linear_obligations::LinearPlace],
            remaining: &mut super::super::cleanup_dispositions::RemainingCustody,
            permissions: &mut Vec<FlowPermissionEventFact>,
            conditional_carrier_transfers: &mut Vec<usize>,
        ) -> Result<(), diagnostics::Diagnostic> {
            let mut sources = Vec::new();
            for scope in facts
                .flow
                .control
                .machine_states(machine)
                .into_iter()
                .filter(|scope| scope.state_symbol == state)
            {
                for row in facts
                    .flow
                    .control
                    .statements
                    .span_or_empty(scope.statements)
                    .iter()
                    .filter(|row| {
                        row.statement_index == statement
                            && facts.flow.control.operands.is_valid(row.stored_value)
                    })
                {
                    // An evaluated destination can select one of several places.
                    // Receiving the RHS does not require publishing a value-map
                    // entry for every possible destination.
                    let Some(destination) =
                        receiving_storage(facts.view(), row, state, statement, evaluation)
                    else {
                        continue;
                    };
                    let value = facts.flow.control.operands.get(row.stored_value);
                    if value.state_symbol != state || value.statement_index != statement {
                        return Err(diagnostics::Diagnostic::error(
                            "stored owner has a different completion coordinate",
                        ));
                    }
                    let source = (value.expression, destination);
                    crate::shared::collections::push_unique(&mut sources, source);
                }
            }
            if sources.len() > 1 {
                return Err(diagnostics::Diagnostic::error(
                    "stored owner has ambiguous receiving storage",
                ));
            }
            for (expression, _) in sources {
                let root = facts::PlaceRoot::Expression(expression);
                let live = places.iter().any(|place| place.root == root && place.live);
                let inactive = places.iter().any(|place| {
                    place.root == root && place.ever_established && place.case_excluded
                }) && permissions.iter().any(|event| {
                    event.root == root
                        && event.machine_symbol == machine
                        && event.state_symbol == state
                        && event.evaluation == evaluation
                        && event.kind == PermissionEventKind::Establish
                        && matches!(event.source,
                            PermissionEventSource::Statement { statement_index }
                            | PermissionEventSource::Call { statement_index, .. }
                            if statement_index == statement)
                });
                if !live && !inactive {
                    continue;
                }
                if !remaining.is_present(root, &[]) {
                    if !live {
                        continue;
                    }
                    return Err(diagnostics::Diagnostic::error(
                        "stored owner has no complete source custody",
                    ));
                }
                append_root_transfer(
                    facts,
                    machine,
                    state,
                    PermissionEventSource::Statement {
                        statement_index: statement,
                    },
                    evaluation,
                    expression,
                    places,
                    permissions,
                    conditional_carrier_transfers,
                );
                remaining.transfer_storage(root, &[]);
            }
            Ok(())
        }
    }
    mod owners {
        /*
            The anonymous owners one statement's consumers receive. A producer completes
            before its consumer: a construction establishes its value, and a call
            result is established by its call's completion. One source evaluation
            inventory supplies named/anonymous permission production and the exact loan
            continuation; executable admission consumes that published roster.
        */
        use super::super::borrowed_windows::StorageWalk;
        use super::super::borrowed_windows::authored_route_exclusive;
        use super::super::projected_affine::has_reference_prefix;
        use super::CheckFacts;
        use crate::checks::multiplicity::linear_obligations::type_reference_is_reference;
        use crate::checks::type_carries_linear_obligation;
        use crate::flow::indexed_statements;
        use crate::published::places::place_at;
        use language_semantics::PermissionEventSource;

        /* How the consuming operation uses the anonymous owner. */
        #[derive(Clone)]
        pub(crate) enum TemporaryConsumption {
            /*
                An evaluated loan retains the whole producing owner, even when its
                operand selects a child; the owner dies after the call.
            */
            Borrow {
                access: language_semantics::PermissionAccess,
                borrowed_path: Vec<facts::PlaceSegment>,
            },
            /*
                The call receives the completed owner whole, with every claim its
                producer established or its operands moved into it.
            */
            OwnedTransfer,
            /*
                The call receives one owned subtree; its normal continuation disposes
                the original owner's untouched complement.
            */
            OwnedProjection {
                path: Vec<facts::PlaceSegment>,
            },
        }

        #[derive(Clone)]
        pub(crate) enum TemporaryDestination {
            CallArgument,
            ConstructedChild {
                production: arena::HandleSpan<checked_trees::FlowOperandChild>,
                expression: typed_trees::expression::ExpressionHandle,
                path: Vec<facts::PlaceSegment>,
            },
        }

        #[derive(Clone)]
        pub(crate) struct TemporaryOwner {
            pub(crate) expression: typed_trees::expression::ExpressionHandle,
            pub(crate) type_reference: typed_trees::types::TypeReferenceHandle,
            pub(crate) consumption: TemporaryConsumption,
            pub(crate) evaluation: checked_trees::FlowPermissionEvaluation,
            pub(crate) producer: PermissionEventSource,
            pub(crate) consumer: PermissionEventSource,
            /*
                A noncall observation can be the last user of returned backing. Zero
                retains the exact call continuation named by `consumer`.
            */
            pub(crate) completion: typed_trees::expression::ExpressionHandle,
            pub(crate) destination: TemporaryDestination,
            pub(crate) completion_constraints:
                Option<arena::HandleSpan<checked_trees::FlowConstraintRef>>,
            completion_ordinal: usize,
        }

        /*
            The claim-bearing type of a by-value operand an anonymous producer
            completes. Existing owned storage keeps its claim identities; an exclusive
            borrowed extraction establishes machine-local custody, because the caller's
            claims stay suspended with its loan. Canonical storage, not source spelling,
            distinguishes those producers from an ordinary owned transfer.
        */
        pub(crate) fn owned_temporary_type(
            program: &dyn ::validation::CheckingInput,
            state_symbol: symbols::SymbolHandle,
            statement_index: usize,
            parameter: &typed_trees::signature::StateParameter,
            argument: typed_trees::expression::ExpressionHandle,
        ) -> Option<typed_trees::types::TypeReferenceHandle> {
            if parameter.is_const
                || parameter.relevance.is_erased()
                || type_reference_is_reference(program, parameter.type_reference)
            {
                return None;
            }
            owned_value_type(program, state_symbol, statement_index, argument)
        }

        fn owned_value_type(
            program: &dyn ::validation::CheckingInput,
            state_symbol: symbols::SymbolHandle,
            statement_index: usize,
            argument: typed_trees::expression::ExpressionHandle,
        ) -> Option<typed_trees::types::TypeReferenceHandle> {
            let place = place_at(program, state_symbol, statement_index, argument)?;
            let anonymous_value =
                place.root == facts::PlaceRoot::Expression(argument) && place.segments.is_empty();
            let borrowed_value =
                has_reference_prefix(program, state_symbol, statement_index, &place)
                    && validation::find_state(program, state_symbol).is_some_and(
                        |(machine, state)| {
                            let statements =
                                program.statement_table.statements(state.statement_nodes);
                            let walk = StorageWalk::new(program, machine, state, statements);
                            authored_route_exclusive(
                                &walk,
                                statement_index,
                                place.root,
                                &place.segments,
                            )
                        },
                    );
            if !anonymous_value && !borrowed_value {
                return None;
            }
            let type_reference = validation::canonical_place_type_reference(
                program,
                state_symbol,
                statement_index,
                &place,
            )?;
            let type_reference = if borrowed_value {
                validation::assignment_value_type(program, type_reference)
            } else {
                type_reference
            };
            (type_carries_linear_obligation(program, type_reference)
                || (anonymous_value
                    && validation::reference_result_custody::binding_multiplicity(
                        program,
                        type_reference,
                    ) == language_semantics::Multiplicity::Affine))
                .then_some(type_reference)
        }

        pub(crate) fn temporary_owners(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            calls: &[checked_trees::FlowCallFact],
        ) -> Result<Vec<TemporaryOwner>, diagnostics::Diagnostic> {
            let completed = indexed_statements(program, state.statement_nodes)
                .map(|(statement_index, statement)| {
                    crate::checks::ownership_occurrences::statement_steps(
                        program,
                        machine,
                        state,
                        statement_index,
                        statement,
                        &[],
                        &facts.operators,
                    )
                    .into_iter()
                    .filter_map(|occurrence| match occurrence.step {
                        crate::checks::ownership_occurrences::StatementStep::Complete(
                            expression,
                        ) => Some((expression, occurrence.route)),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let mut owners = Vec::new();
            for consumer in calls {
                if facts.flow.control.is_retired(state.symbol, consumer)
                    || (consumer.authored_expression.is_valid()
                        && !completed
                            .get(consumer.statement_index)
                            .is_some_and(|expressions| {
                                expressions.iter().any(|(expression, _)| {
                                    *expression == consumer.authored_expression
                                })
                            }))
                {
                    continue;
                }
                let Some(site) = validation::find_call_site(
                    program,
                    machine.symbol,
                    state.symbol,
                    consumer.statement_index,
                    consumer.call_ordinal,
                ) else {
                    continue;
                };
                let Some(parameters) =
                    validation::call_target_parameters(program, consumer.target_symbol)
                else {
                    continue;
                };
                let arguments = validation::call_site_argument_expressions(program, &site);
                if parameters.len() != arguments.len() {
                    continue;
                }
                for (parameter, argument) in parameters.iter().zip(arguments) {
                    let (expression, type_reference, consumption) = if let Some(type_reference) =
                        owned_temporary_type(
                            program,
                            state.symbol,
                            consumer.statement_index,
                            parameter,
                            *argument,
                        ) {
                        (
                            *argument,
                            type_reference,
                            TemporaryConsumption::OwnedTransfer,
                        )
                    } else if !site.is_internal_state_transfer(program, machine.symbol)
                        && let Some((expression, type_reference, path)) =
                            owned_projection(program, state.symbol, consumer, parameter, *argument)
                    {
                        (
                            expression,
                            type_reference,
                            TemporaryConsumption::OwnedProjection { path },
                        )
                    } else {
                        if parameter.is_const
                            || parameter.is_mutable
                            || parameter.relevance.is_erased()
                        {
                            continue;
                        }
                        let descriptor_access =
                            validation::slice_view_shell(program, parameter.type_reference, &[])
                                .and_then(|shell| match shell.access {
                                    language_semantics::ReferenceAccess::Shared => {
                                        Some(language_semantics::PermissionAccess::Shared)
                                    }
                                    language_semantics::ReferenceAccess::Mutable => {
                                        Some(language_semantics::PermissionAccess::Exclusive)
                                    }
                                    _ => None,
                                });
                        let (borrowed, access) = if let Some(access) = descriptor_access {
                            (*argument, access)
                        } else if let Some((borrowed, _)) =
                            crate::published::semantic::calls::shared_argument_referent(
                                program,
                                *argument,
                                parameter.type_reference,
                            )
                        {
                            (borrowed, language_semantics::PermissionAccess::Shared)
                        } else {
                            continue;
                        };
                        let Some(place) =
                            place_at(program, state.symbol, consumer.statement_index, borrowed)
                        else {
                            continue;
                        };
                        let Some(expression) = place.root.as_expression() else {
                            continue;
                        };
                        let root = validation::CanonicalPlace {
                            root: place.root,
                            segments: Vec::new(),
                        };
                        let Some(type_reference) = validation::canonical_place_type_reference(
                            program,
                            state.symbol,
                            consumer.statement_index,
                            &root,
                        ) else {
                            continue;
                        };
                        (
                            expression,
                            type_reference,
                            TemporaryConsumption::Borrow {
                                access,
                                borrowed_path: place.segments,
                            },
                        )
                    };
                    let evaluation = crate::flow::call_permission_evaluation(
                        program,
                        machine,
                        state,
                        consumer.statement_index,
                        consumer.call_ordinal,
                    );
                    if evaluation == checked_trees::FlowPermissionEvaluation::Unknown {
                        continue;
                    }
                    let Some(producer) = producer_source(
                        program,
                        machine,
                        state,
                        calls,
                        consumer.statement_index,
                        evaluation,
                        expression,
                    ) else {
                        continue;
                    };
                    let mut completions = completed
                        .get(consumer.statement_index)
                        .into_iter()
                        .flatten()
                        .enumerate()
                        .filter(|(_, (completed, _))| *completed == expression);
                    let Some((completion_ordinal, _)) = completions.next() else {
                        continue;
                    };
                    if completions.next().is_some() {
                        continue;
                    }
                    owners.push(TemporaryOwner {
                        expression,
                        type_reference,
                        consumption,
                        evaluation,
                        producer,
                        consumer: PermissionEventSource::Call {
                            statement_index: consumer.statement_index,
                            call_ordinal: consumer.call_ordinal,
                            target_symbol: consumer.target_symbol,
                        },
                        destination: TemporaryDestination::CallArgument,
                        completion_ordinal,
                        completion: typed_trees::expression::ExpressionHandle::invalid(),
                        completion_constraints: None,
                    });
                }
            }
            for owner in &mut owners {
                if !matches!(owner.consumption, TemporaryConsumption::Borrow { .. }) {
                    continue;
                }
                let PermissionEventSource::Call {
                    statement_index, ..
                } = owner.consumer
                else {
                    continue;
                };
                let Some(completions) = completed.get(statement_index) else {
                    continue;
                };
                let completion_at = |call: &checked_trees::FlowCallFact| {
                    if !call.authored_expression.is_valid() {
                        return Some((completions.len(), &[][..]));
                    }
                    let mut matching = completions
                        .iter()
                        .enumerate()
                        .filter(|(_, (expression, _))| *expression == call.authored_expression);
                    let (ordinal, (_, route)) = matching.next()?;
                    matching
                        .next()
                        .is_none()
                        .then_some((ordinal, route.as_slice()))
                };
                let mut original = calls.iter().filter(|call| {
                    owner.consumer
                        == PermissionEventSource::Call {
                            statement_index: call.statement_index,
                            call_ordinal: call.call_ordinal,
                            target_symbol: call.target_symbol,
                        }
                });
                let Some(original_call) = original.next() else {
                    continue;
                };
                if original.next().is_some() {
                    continue;
                }
                let Some((mut last_completion, original_route)) = completion_at(original_call)
                else {
                    continue;
                };
                for consumer in calls
                    .iter()
                    .filter(|call| call.statement_index == statement_index)
                {
                    if facts.flow.control.is_retired(state.symbol, consumer) {
                        continue;
                    }
                    if crate::flow::call_permission_evaluation(
                        program,
                        machine,
                        state,
                        statement_index,
                        consumer.call_ordinal,
                    ) != owner.evaluation
                    {
                        continue;
                    }
                    let Some((completion, route)) = completion_at(consumer) else {
                        continue;
                    };
                    if completion < last_completion
                        || !route
                            .iter()
                            .all(|decision| original_route.contains(decision))
                    {
                        continue;
                    }
                    let mut retains_owner = false;
                    for operand in consumer.argument_operands().handles() {
                        let capture = facts.flow.control.operands.get(operand);
                        let roots = crate::flow::captured_lifetime_roots(program, &facts.flow.control, capture)
                            .ok_or_else(|| diagnostics::Diagnostic::error(format!(
                                "temporary backing lifetime coverage is unavailable for an operand of `{}` in state `{}` at statement {}",
                                program.symbols.name(consumer.target_symbol), state.name, statement_index,
                                )).with_source_span(program.expression_table.source_span(capture.expression)))?;
                        retains_owner |=
                            roots.contains(&facts::PlaceRoot::Expression(owner.expression));
                    }
                    if retains_owner {
                        owner.consumer = PermissionEventSource::Call {
                            statement_index,
                            call_ordinal: consumer.call_ordinal,
                            target_symbol: consumer.target_symbol,
                        };
                        last_completion = completion;
                    }
                }
                let Some(flow_state) = facts.flow.control.state(machine.symbol, state.symbol)
                else {
                    continue;
                };
                let Some(statement) = facts.flow.state_statement(flow_state, statement_index)
                else {
                    continue;
                };
                for observation in facts
                    .flow
                    .control
                    .value_observations
                    .span_or_empty(statement.value_observations)
                    .iter()
                    .filter(|observation| observation.evaluation == owner.evaluation)
                {
                    let mut matching = completions
                        .iter()
                        .enumerate()
                        .filter(|(_, (expression, _))| *expression == observation.expression);
                    let Some((completion, (_, route))) = matching.next() else {
                        continue;
                    };
                    if matching.next().is_some()
                        || completion < last_completion
                        || !route
                            .iter()
                            .all(|decision| original_route.contains(decision))
                    {
                        continue;
                    }
                    let capture = facts.flow.control.operands.get(observation.receiver);
                    let roots = crate::flow::captured_observation_roots(program, &facts.flow.control, capture)
                        .ok_or_else(|| diagnostics::Diagnostic::error(format!(
                            "temporary backing observation coverage is unavailable for an observed receiver in state `{}` at statement {}",
                            state.name, statement_index,
                        )).with_source_span(program.expression_table.source_span(observation.expression)))?;
                    if roots.contains(&facts::PlaceRoot::Expression(owner.expression)) {
                        if facts
                            .flow
                            .control
                            .value_observations
                            .span_or_empty(statement.value_observations)
                            .iter()
                            .filter(|candidate| {
                                candidate.expression == observation.expression
                                    && candidate.evaluation == owner.evaluation
                            })
                            .count()
                            != 1
                        {
                            return Err(diagnostics::Diagnostic::error(format!(
                                "temporary owner has ambiguous value completion in state `{}` at statement {}",
                                state.name, statement_index,
                            )).with_source_span(program.expression_table.source_span(observation.expression)));
                        }
                        owner.consumer = PermissionEventSource::Statement { statement_index };
                        owner.completion = observation.expression;
                        owner.completion_constraints = Some(observation.constraints);
                        last_completion = completion;
                    }
                }
            }
            append_constructed_owners(
                program,
                facts,
                machine,
                state,
                calls,
                &completed,
                &mut owners,
            );
            owners.sort_by_key(|owner| owner.completion_ordinal);
            Ok(owners)
        }

        /*
            Simultaneous children bind actual anonymous owners to their typed receiving
            paths. Alternatives are not children: the selection's guarded receipt owns
            those transfers. Both kinds use the same temporary-owner transfer ledger.
        */
        #[allow(clippy::too_many_arguments)]
        fn append_constructed_owners(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            calls: &[checked_trees::FlowCallFact],
            completed: &[Vec<(
                typed_trees::expression::ExpressionHandle,
                Vec<crate::checks::ownership_occurrences::EvaluationBranch>,
            )>],
            owners: &mut Vec<TemporaryOwner>,
        ) {
            use crate::checks::multiplicity::permission_schedule::{PermissionStep, walk};
            use crate::checks::ownership_occurrences::{
                StatementStep, statement_steps_at_evaluation,
            };
            let retained =
                retained_operand_productions(facts, machine.symbol, state.symbol, calls, completed);
            walk(program, state, calls, (), |step, _| {
                let PermissionStep::Evaluate {
                    statement,
                    evaluation,
                } = step
                else {
                    return;
                };
                let node = &program.statement_table.statements(state.statement_nodes)[statement];
                let occurrences = statement_steps_at_evaluation(
                    program,
                    machine,
                    state,
                    statement,
                    node,
                    &[],
                    &facts.operators,
                    Some(evaluation),
                );
                for (_, parent) in retained.iter().copied().map(|handle| (handle, facts.flow.control.operands.get(handle))).filter(|(_, operand)| {
                    operand.state_symbol == state.symbol
                        && operand.statement_index == statement
                        && occurrences.iter().any(|occurrence| {
                            matches!(occurrence.step, StatementStep::Complete(expression) if expression == operand.expression)
                        })
                }) {
                    let production = parent.constructed_children();
                    for child in facts.flow.control.operand_children.span_or_empty(production) {
                        if !child.type_reference.is_valid()
                            || type_reference_is_reference(program, child.type_reference)
                        {
                            continue;
                        }
                        let value = facts.flow.control.operands.get(child.value);
                        let Some(reference) = owned_value_type(program, state.symbol, statement, value.expression) else {
                            continue;
                        };
                        let Some(completion_ordinal) = completed.get(statement)
                            .and_then(|values| values.iter().position(|(expression, _)| *expression == value.expression))
                        else {
                            continue;
                        };
                        if !occurrences.iter().any(|occurrence| {
                            matches!(occurrence.step, StatementStep::Complete(expression) if expression == value.expression)
                        }) {
                            continue;
                        }
                        let path = facts.flow.control.operand_child_paths.span_or_empty(child.path);
                        if owners.iter().any(|owner| {
                            owner.expression == value.expression && owner.evaluation == evaluation
                                && matches!(&owner.destination, TemporaryDestination::ConstructedChild { production: existing_production, path: existing, .. }
                                    if *existing_production == production && existing == path)
                        }) {
                            continue;
                        }
                        let Some(producer) = producer_source(
                            program, machine, state, calls, statement, evaluation, value.expression,
                        ) else {
                            continue;
                        };
                        owners.push(TemporaryOwner {
                            expression: value.expression,
                            type_reference: reference,
                            consumption: TemporaryConsumption::OwnedTransfer,
                            evaluation,
                            producer,
                            consumer: PermissionEventSource::Statement { statement_index: statement },
                            destination: TemporaryDestination::ConstructedChild {
                                production,
                                expression: parent.expression,
                                path: path.to_vec(),
                            },
                            completion_ordinal,
                            completion: typed_trees::expression::ExpressionHandle::invalid(),
                            completion_constraints: None,
                        });
                    }
                }
            });
        }

        /*
            Walk only value graphs retained by this state's actual consumers. In
            particular a contextual borrow keeps its completed owned production through
            retention.stored_value; allocated child rows alone are not a parent receipt.
            Handles deduplicate shared captures without conflating repeated expressions.
        */
        fn retained_operand_productions(
            facts: &CheckFacts,
            machine: symbols::SymbolHandle,
            state: symbols::SymbolHandle,
            calls: &[checked_trees::FlowCallFact],
            completed: &[Vec<(
                typed_trees::expression::ExpressionHandle,
                Vec<crate::checks::ownership_occurrences::EvaluationBranch>,
            )>],
        ) -> Vec<arena::Handle<checked_trees::FlowOperandFact>> {
            let control = &facts.flow.control;
            let Some(scope) = control.state(machine, state) else {
                return Vec::new();
            };
            let mut pending = calls
                .iter()
                .filter(|call| {
                    !control.is_retired(state, call)
                        && (!call.authored_expression.is_valid()
                            || completed.get(call.statement_index).is_some_and(|rows| {
                                rows.iter()
                                    .any(|(expression, _)| *expression == call.authored_expression)
                            }))
                })
                .flat_map(|call| call.operands.handles())
                .collect::<Vec<_>>();
            for statement in control.statements.span_or_empty(scope.statements) {
                pending.extend([statement.stored_value, statement.store_target]);
                pending.extend(
                    control
                        .value_observations
                        .span_or_empty(statement.value_observations)
                        .iter()
                        .map(|row| row.receiver),
                );
            }
            pending.extend(
                control
                    .outgoing
                    .span_or_empty(scope.outgoing)
                    .iter()
                    .map(|row| row.returned_value),
            );
            for (_, invocation) in control.operator_invocations.iter() {
                let selected_use = if facts.operators.uses.is_valid(invocation.operator_use) {
                    let usage = facts.operators.uses.get(invocation.operator_use);
                    Some((usage.origin, usage.expression))
                } else if facts.operators.named_uses.is_valid(invocation.named_use) {
                    let usage = facts.operators.named_uses.get(invocation.named_use);
                    Some((usage.origin, usage.expression))
                } else {
                    None
                };
                let Some((
                    checked_trees::CheckedValueOrigin::StateStatement {
                        machine_symbol,
                        state_symbol,
                        statement_index,
                        ..
                    },
                    expression,
                )) = selected_use
                else {
                    continue;
                };
                if machine_symbol != machine
                    || state_symbol != state
                    || !completed.get(statement_index).is_some_and(|rows| {
                        rows.iter().any(|(completed, _)| *completed == expression)
                    })
                {
                    continue;
                }
                pending.extend(invocation.operands.handles());
            }
            let mut retained = Vec::new();
            let mut retentions = Vec::new();
            let mut visited_retentions = Vec::new();
            while let Some(handle) = pending.pop() {
                if !control.operands.is_valid(handle) || retained.contains(&handle) {
                    continue;
                }
                let operand = control.operands.get(handle);
                if operand.state_symbol != state {
                    continue;
                }
                retained.push(handle);
                pending.push(operand.backing);
                match operand.completion {
                    checked_trees::FlowOperandCompletion::Constructed(children) => pending.extend(
                        control
                            .operand_children
                            .span_or_empty(children)
                            .iter()
                            .map(|child| child.value),
                    ),
                    checked_trees::FlowOperandCompletion::Selected(alternatives) => pending.extend(
                        control
                            .operand_alternatives
                            .span_or_empty(alternatives)
                            .iter()
                            .map(|arm| arm.value),
                    ),
                    checked_trees::FlowOperandCompletion::Copied { copy, .. } => {
                        let copy = control.copied_values.get(copy);
                        pending.push(copy.source);
                        retentions.extend(
                            control
                                .captured_reference_leaves
                                .span_or_empty(copy.references)
                                .iter()
                                .map(|leaf| leaf.retention),
                        );
                    }
                    checked_trees::FlowOperandCompletion::Observation
                    | checked_trees::FlowOperandCompletion::IncomingContents { .. }
                    | checked_trees::FlowOperandCompletion::InvocationResult { .. } => {}
                }
                retentions.push(operand.retention);
                while let Some(retention) = retentions.pop() {
                    if !control.operand_retentions.is_valid(retention)
                        || visited_retentions.contains(&retention)
                    {
                        continue;
                    }
                    visited_retentions.push(retention);
                    let profile = control.operand_retentions.get(retention);
                    pending.push(profile.stored_value);
                    retentions.extend(
                        control
                            .operand_retained_leaves
                            .span_or_empty(profile.leaves)
                            .iter()
                            .map(|leaf| leaf.retention),
                    );
                }
            }
            retained
        }

        #[allow(clippy::too_many_arguments)]
        fn producer_source(
            program: &dyn ::validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            calls: &[checked_trees::FlowCallFact],
            statement: usize,
            evaluation: checked_trees::FlowPermissionEvaluation,
            expression: typed_trees::expression::ExpressionHandle,
        ) -> Option<PermissionEventSource> {
            if program.expression_table.call_node(expression).is_none() {
                return Some(PermissionEventSource::Statement {
                    statement_index: statement,
                });
            }
            let mut producers = calls.iter().filter(|producer| {
                producer.statement_index == statement && producer.authored_expression == expression
            });
            let producer = producers.next()?;
            if producers.next().is_some()
                || crate::flow::call_permission_evaluation(
                    program,
                    machine,
                    state,
                    statement,
                    producer.call_ordinal,
                ) != evaluation
            {
                return None;
            }
            Some(PermissionEventSource::Call {
                statement_index: statement,
                call_ordinal: producer.call_ordinal,
                target_symbol: producer.target_symbol,
            })
        }

        fn owned_projection(
            program: &dyn validation::CheckingInput,
            state: symbols::SymbolHandle,
            consumer: &checked_trees::FlowCallFact,
            parameter: &typed_trees::signature::StateParameter,
            argument: typed_trees::expression::ExpressionHandle,
        ) -> Option<(
            typed_trees::expression::ExpressionHandle,
            typed_trees::types::TypeReferenceHandle,
            Vec<facts::PlaceSegment>,
        )> {
            if parameter.is_const
                || parameter.relevance.is_erased()
                || type_reference_is_reference(program, parameter.type_reference)
            {
                return None;
            }
            let place = place_at(program, state, consumer.statement_index, argument)?;
            let expression = place.root.as_expression()?;
            if place.segments.is_empty() {
                return None;
            }
            let endpoint = validation::canonical_place_type_reference(
                program,
                state,
                consumer.statement_index,
                &place,
            )?;
            if program.type_multiplicity(endpoint) == language_semantics::Multiplicity::Unrestricted
                || type_reference_is_reference(program, endpoint)
                || has_reference_prefix(program, state, consumer.statement_index, &place)
            {
                return None;
            }
            let reference =
                projectable_owner(program, state, consumer.statement_index, expression)?;
            Some((expression, reference, place.segments))
        }

        /*
            The type of an anonymous affine owner whose untouched complement may die
            after one subtree moves out. Its completing consumer must separately prove
            authorized disposal of every claim left under the original owner.
        */
        pub(in crate::checks::multiplicity) fn projectable_owner(
            program: &dyn validation::CheckingInput,
            state: symbols::SymbolHandle,
            statement_index: usize,
            expression: typed_trees::expression::ExpressionHandle,
        ) -> Option<typed_trees::types::TypeReferenceHandle> {
            let reference = validation::canonical_place_type_reference(
                program,
                state,
                statement_index,
                &validation::CanonicalPlace {
                    root: facts::PlaceRoot::Expression(expression),
                    segments: Vec::new(),
                },
            )?;
            (program.type_multiplicity(reference) == language_semantics::Multiplicity::Affine)
                .then_some(reference)
        }
    }
    mod projected {
        /*
            Call continuations retire the complement of an already transferred subtree.
            Establishment and transfer belong to the shared occurrence ledger, not this
            cleanup. The original owner, rather than the projected argument, dies here.
        */
        use super::owners::{TemporaryConsumption, TemporaryOwner};
        use super::{
            FlowPermissionEventFact, Multiplicity, PermissionAccess, PermissionClaimIdentity,
            PermissionEventKind, PermissionProvenance, SymbolHandle,
        };
        use language_semantics::PermissionEventSource;

        #[allow(clippy::too_many_arguments)]
        pub(in crate::checks::multiplicity) fn append_cleanup(
            program: &dyn validation::CheckingInput,
            facts: &mut checked_trees::CheckFactsWalk<'_>,
            machine: SymbolHandle,
            state: SymbolHandle,
            consumer: PermissionEventSource,
            evaluation: checked_trees::FlowPermissionEvaluation,
            owners: &[TemporaryOwner],
            places: &mut [super::super::linear_obligations::LinearPlace],
            remaining: &super::super::cleanup_dispositions::RemainingCustody,
            permissions: &mut Vec<FlowPermissionEventFact>,
        ) -> Result<Vec<facts::PlaceRoot>, diagnostics::Diagnostic> {
            let mut retired = Vec::new();
            for owner in owners
                .iter()
                .rev()
                .filter(|owner| owner.consumer == consumer && owner.evaluation == evaluation)
            {
                let TemporaryConsumption::OwnedProjection { path } = &owner.consumption else {
                    continue;
                };
                if !matches!(consumer, PermissionEventSource::Call { .. }) {
                    continue;
                }
                retired.push(retire_owner(
                    program,
                    facts.flow.ownership,
                    machine,
                    state,
                    ProjectedConsumer {
                        source: consumer,
                        evaluation,
                        producer: PermissionProvenance::Established {
                            machine_symbol: machine,
                            state_symbol: state,
                            source: owner.producer,
                        },
                    },
                    owner.expression,
                    path,
                    places,
                    remaining,
                    permissions,
                )?);
            }
            Ok(retired)
        }

        /* The completed consumer of one projected move out of an anonymous owner. */
        pub(in crate::checks::multiplicity) struct ProjectedConsumer {
            pub(in crate::checks::multiplicity) source: PermissionEventSource,
            pub(in crate::checks::multiplicity) evaluation: checked_trees::FlowPermissionEvaluation,
            /* The establishment of the anonymous owner whose complement dies here. */
            pub(in crate::checks::multiplicity) producer: PermissionProvenance,
        }

        /*
            Retire an anonymous owner whose selected subtree has already transferred:
            its untouched complement dies at the consumer that completed the move,
            whether that consumer is a call continuation or the statement whose value
            received the subtree. Nothing can name the owner afterwards.
        */
        #[allow(clippy::too_many_arguments)]
        pub(in crate::checks::multiplicity) fn retire_owner(
            program: &dyn validation::CheckingInput,
            ownership: &mut checked_trees::FlowOwnershipFacts,
            machine: SymbolHandle,
            state: SymbolHandle,
            consumer: ProjectedConsumer,
            expression: typed_trees::expression::ExpressionHandle,
            path: &[facts::PlaceSegment],
            places: &mut [super::super::linear_obligations::LinearPlace],
            remaining: &super::super::cleanup_dispositions::RemainingCustody,
            permissions: &mut Vec<FlowPermissionEventFact>,
        ) -> Result<facts::PlaceRoot, diagnostics::Diagnostic> {
            let root = facts::PlaceRoot::Expression(expression);
            let statement = match consumer.source {
                PermissionEventSource::Call {
                    statement_index, ..
                }
                | PermissionEventSource::Statement { statement_index } => statement_index,
                _ => {
                    return Err(diagnostics::Diagnostic::error(
                        "projected temporary cleanup has no completing call or statement",
                    ));
                }
            };
            let reference = super::owners::projectable_owner(program, state, statement, expression)
                .ok_or_else(|| {
                    diagnostics::Diagnostic::error(
                        "projected temporary cleanup has no exact affine owner",
                    )
                })?;
            if path.is_empty()
                || remaining.is_present(root, path)
                || !remaining.is_present(root, &[])
                || !places
                    .iter()
                    .any(|place| place.root == root && place.ever_established)
            {
                return Err(diagnostics::Diagnostic::error(
                    "projected temporary cleanup lacks its exact completed transfer and residual partition",
                ));
            }
            let start = permissions.len();
            super::super::linear_validation::append_owned_region_cleanup_permission_events(
                program,
                reference,
                root,
                &[],
                places,
                remaining,
                FlowPermissionEventFact {
                    expression,
                    evaluation: consumer.evaluation,
                    machine_symbol: machine,
                    state_symbol: state,
                    source: consumer.source,
                    kind: PermissionEventKind::AffineDrop,
                    multiplicity: Multiplicity::Affine,
                    access: PermissionAccess::Owned,
                    claim_identity: PermissionClaimIdentity::Unknown,
                    provenance: consumer.producer,
                    root,
                    segments: Default::default(),
                    obligation_live: false,
                },
                &mut ownership.segments,
                permissions,
            );
            if permissions.len() == start {
                return Err(diagnostics::Diagnostic::error(
                    "cannot automatically dispose a projected temporary: its exact residual claims need owner-authorized terminal dispositions",
                ));
            }
            for place in places.iter_mut().filter(|place| place.root == root) {
                place.live = false;
            }
            Ok(root)
        }
    }

    pub(super) use owned_transfers::{append_owned_transfer, append_stored_transfer};
    pub(crate) use owners::owned_temporary_type;
    pub(super) use owners::{TemporaryOwner, projectable_owner, temporary_owners};
    pub(super) use projected::{ProjectedConsumer, retire_owner};

    /*
        One call completion disposes temporary owners in reverse establishment
        order. Usage selects the exact whole or residual plan, not a separate
        cleanup order for borrowed versus transferred arguments.
    */
    #[allow(clippy::too_many_arguments)]
    pub(super) fn append_continuation_cleanup(
        program: &dyn validation::CheckingInput,
        facts: &mut checked_trees::CheckFactsWalk<'_>,
        machine: SymbolHandle,
        state: SymbolHandle,
        consumer: language_semantics::PermissionEventSource,
        completion: typed_trees::expression::ExpressionHandle,
        evaluation: checked_trees::FlowPermissionEvaluation,
        owners: &[TemporaryOwner],
        places: &mut [super::linear_obligations::LinearPlace],
        remaining: &super::cleanup_dispositions::RemainingCustody,
        permissions: &mut Vec<FlowPermissionEventFact>,
        dispositions: &mut checked_trees::ContextualCleanupFacts,
    ) -> Result<Vec<facts::PlaceRoot>, Diagnostic> {
        let mut retired = Vec::new();
        for owner in owners.iter().rev().filter(|owner| {
            owner.consumer == consumer
                && owner.completion == completion
                && owner.evaluation == evaluation
        }) {
            let mut diagnostics = Vec::new();
            match owner.consumption {
                owners::TemporaryConsumption::Borrow { .. } => {
                    retired.extend(borrowed_owners::append_borrow(
                        program,
                        facts,
                        machine,
                        state,
                        consumer,
                        evaluation,
                        std::slice::from_ref(owner),
                        places,
                        permissions,
                        &mut diagnostics,
                        dispositions,
                    ))
                }
                owners::TemporaryConsumption::OwnedProjection { .. } => {
                    retired.extend(projected::append_cleanup(
                        program,
                        facts,
                        machine,
                        state,
                        consumer,
                        evaluation,
                        std::slice::from_ref(owner),
                        places,
                        remaining,
                        permissions,
                    )?)
                }
                owners::TemporaryConsumption::OwnedTransfer => {}
            }
            if let Some(diagnostic) = diagnostics.into_iter().next() {
                return Err(diagnostic);
            }
        }
        Ok(retired)
    }

    /*
        A partial anonymous move needs its exact completing consumer before a
        remaining claim can be considered for owner-authorized disposal. Eligibility
        is checked only after the actual transfer, against the resulting custody.
    */
    pub(super) fn unselected_claim_diagnostic(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        event: &crate::flow::DiscoveredMoveEvent,
        path: &[facts::PlaceSegment],
        owners: &[TemporaryOwner],
    ) -> Option<Diagnostic> {
        let expression = event.root.as_expression()?;
        let consumer = permission_source(event.source);
        let mut continuations = owners.iter().filter(|owner| {
            owner.expression == expression
                && owner.consumer == consumer
                && owner.evaluation == event.evaluation
        });
        if matches!(
            consumer,
            language_semantics::PermissionEventSource::Call { .. }
        ) && let Some(owner) = continuations.next()
            && continuations.next().is_none()
            && matches!(
                &owner.consumption,
                owners::TemporaryConsumption::OwnedProjection { path: selected } if selected == path
            )
        {
            return None;
        }
        let root = validation::CanonicalPlace {
            root: event.root,
            segments: Vec::new(),
        };
        let reference = validation::canonical_place_type_reference(
            program,
            state_symbol,
            event_statement_index(event.source).unwrap_or(0),
            &root,
        )?;
        for claim in linear_claim_frontier(program, reference) {
            if claim.multiplicity != Multiplicity::Linear
                || claim.path.starts_with(path)
                || claim_paths_are_case_alternatives(&claim.path, path)
            {
                continue;
            }
            let machine_name = validation::find_state(program, state_symbol)
                .map(|(machine, _)| machine.name.as_str())
                .unwrap_or("<unknown machine>");
            let statement_index = event_statement_index(event.source).unwrap_or(0);
            return Some(Diagnostic::error(format!(
                "cannot partially move a temporary call result while an unselected linear claim remains in {machine_name}::{} at statement {statement_index}; bind the result to a local and transfer or consume every claim",
                program.symbols.name(state_symbol)
            )));
        }
        None
    }
}
mod type_multiplicity {
    /*
        The multiplicity a type carries, with generic substitutions applied, and
        whether an expression establishes a linear obligation.
    */
    use crate::checks::multiplicity::linear_obligations::LinearPlace;
    use crate::flow::expressions;
    use crate::published::places::place_at;
    use language_semantics::Multiplicity;
    use symbols::SymbolHandle;
    use typed_trees::types::TypeReferenceHandle;

    pub(crate) fn type_carries_linear_obligation(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        validation::type_carries_linear_obligation(program, type_reference)
    }

    pub(crate) fn expression_establishes_obligation(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        expression: typed_trees::expression::ExpressionHandle,
        relative_path: &[facts::PlaceSegment],
        places: &[LinearPlace],
    ) -> bool {
        if let Some(source) = place_at(program, state_symbol, statement_index, expression) {
            let mut source_path = source.segments;
            source_path.extend_from_slice(relative_path);
            if let Some(place) = places.iter().find(|place| {
                place.root == source.root && place.path == source_path && place.ever_established
            }) {
                return place.live;
            }
        }

        if let typed_trees::expression::ExpressionNode::ArrayLiteral(values) =
            program.expression_table.expression(expression)
            && let Some(facts::PlaceSegment::FixedIndex { index }) = relative_path.first()
        {
            return expressions(program)
                .expression_handles(*values)
                .get(*index)
                .is_some_and(|value| {
                    expression_establishes_obligation(
                        program,
                        state_symbol,
                        statement_index,
                        *value,
                        &relative_path[1..],
                        places,
                    )
                });
        }

        if let typed_trees::expression::ExpressionNode::StructLiteral(literal) =
            program.expression_table.expression(expression)
        {
            let mut remaining = relative_path;
            if literal.case_name.is_some() {
                let Some(facts::PlaceSegment::Case { variant }) = remaining.first() else {
                    return remaining.is_empty();
                };
                if literal_variant(program, literal).map(|candidate| candidate.symbol)
                    != Some(*variant)
                {
                    return false;
                }
                remaining = &remaining[1..];
            }
            if let Some(facts::PlaceSegment::Field { symbol }) = remaining.first() {
                let Some(field_name) = data_field_name(program, *symbol) else {
                    return false;
                };
                let Some(field) = expressions(program)
                    .struct_fields(literal.fields)
                    .iter()
                    .find(|field| field.name.as_str() == field_name)
                else {
                    return false;
                };
                return expression_establishes_obligation(
                    program,
                    state_symbol,
                    statement_index,
                    field.value,
                    &remaining[1..],
                    places,
                );
            }
            return remaining.is_empty();
        }

        if matches!(
            program.expression_table.expression(expression),
            typed_trees::expression::ExpressionNode::Name(path)
                if program.data_definitions().iter().any(|definition| {
                    program.data_members(definition).iter().any(|member| {
                        matches!(
                            member,
                            typed_trees::data::DataMember::Variant(variant)
                                if variant.symbol == path.symbol
                        )
                    })
                })
        ) {
            return relative_path.is_empty();
        }

        true
    }

    pub(crate) fn type_multiplicity(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Multiplicity {
        program.type_multiplicity(type_reference)
    }

    /*
        The access kind of a possibly-constrained reference type, walking
        `Constrained` shells to the reference node underneath. `None` when the type
        bottoms out at anything other than a reference.
    */
    pub(crate) fn reference_access(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<language_core::ReferenceAccess> {
        match program.type_reference_table.type_reference(type_reference) {
            typed_trees::types::TypeReferenceNode::Reference { access, .. } => Some(*access),
            typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                reference_access(program, *base_type)
            }
            _ => None,
        }
    }

    pub(crate) fn literal_variant<'program>(
        program: &'program dyn ::validation::CheckingInput,
        literal: &typed_trees::expression::TableStructLiteral,
    ) -> Option<&'program typed_trees::data::DataVariant> {
        let case_name = literal.case_name.as_ref()?;
        let definition = program
            .data_definitions()
            .iter()
            .find(|definition| definition.name == literal.type_name)?;
        program.data_members(definition).iter().find_map(|member| {
            let variant = member.as_variant()?;
            (variant.name == *case_name).then_some(variant)
        })
    }

    pub(crate) fn data_field_name(
        program: &dyn ::validation::CheckingInput,
        field_symbol: SymbolHandle,
    ) -> Option<&str> {
        program
            .any_field_owner(field_symbol)
            .map(|(_, field)| field.name.as_str())
    }
}

pub(crate) use linear_obligations::{check_linear_obligations, type_reference_is_reference};
pub(crate) use linear_validation::linear_claim_frontier;
pub(crate) use temporary_results::owned_temporary_type;
pub(crate) use type_multiplicity::{
    data_field_name, reference_access, type_carries_linear_obligation, type_multiplicity,
};
