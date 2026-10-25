//! Addresses are formed from completed operands, not the final selector
//! variables. A bound can change while a later bound evaluates; each coordinate
//! therefore reads its own capture. Stored descriptors retain these addresses
//! independently of later writes to their backing contents.

use super::super::CapturedReferencePlace;
/// Ephemeral results of the shared immutable completed-storage query.
/// This is neither a second store inventory nor permission to load a pointer.
pub(super) enum StoredReferenceContents {
    Unknown,
    Incoming,
    Captured {
        referents: HandleSpan<FlowOperandReferent>,
        retention: Handle<checked_trees::FlowOperandRetention>,
    },
}
use super::Execution;
use arena::{Handle, HandleSpan};
use checked_trees::{FlowConstraintKind, FlowOperandFact, FlowOperandReferent};
use facts::{PlaceRoot, PlaceSegment, ScalarValue};
use typed_trees::expression::ExpressionHandle;
use validation::CanonicalPlace;

impl Execution<'_, '_, '_> {
    /// Observe the stored descriptors carried by one completed owned value.
    /// This is the same inventory used by selected payload copies; physical
    /// addresses are supplied by the evaluator, not recovered from an initializer.
    pub(super) fn capture_owned_reference_leaves(
        &mut self,
        owner: arena::Handle<FlowOperandFact>,
        places: &[CapturedReferencePlace],
    ) -> HandleSpan<checked_trees::FlowCapturedReferenceLeaf> {
        let capture = *self.context.control.operands.get(owner);
        let Some(paths) = validation::borrow_contracts::borrow_carrying_owner_paths(
            self.program,
            capture.type_reference,
        ) else {
            return HandleSpan::empty();
        };
        let mut leaves = Vec::new();
        for path in paths {
            let path = crate::borrow::owner_place_segments(&path);
            let Some(reference) = validation::owned_projection_type_reference(
                self.program,
                capture.type_reference,
                &path,
            ) else {
                return HandleSpan::empty();
            };
            if !self.is_view_type(reference) {
                return HandleSpan::empty();
            }
            // Keep the supplying evaluations at this same immutable cut.
            // Formation inventory and lifetime roots never manufacture these
            // links; unknown or mixed incoming stores publish no partial roster.
            let suppliers = places
                .iter()
                .map(|place| {
                    let place = place.clone().projected(&path)?;
                    let stored = super::super::stored_views::capture_at_snapshot(
                        self.program,
                        &self.context.control,
                        self.state,
                        self.storage_snapshot,
                        &place.place,
                    )?;
                    let captures = stored.captures()?;
                    (!captures.is_empty()).then_some(captures)
                })
                .collect::<Option<Vec<_>>>();
            let mut immutable_image = suppliers.as_ref().is_some_and(|rows| !rows.is_empty());
            if let Some(suppliers) = &suppliers {
                for selected in suppliers.iter().flatten() {
                    let value = self.context.control.operands.get(selected.capture);
                    let profiles =
                        validation::completed_storage::captured_retention_profiles_at_path(
                            self.program,
                            &self.context.control,
                            value,
                            &selected.projection,
                        );
                    immutable_image &= profiles.is_some_and(|profiles| {
                        !profiles.is_empty()
                            && profiles.iter().all(|profile| {
                                self.context.control.operand_retentions.is_valid(*profile)
                                    && self
                                        .context
                                        .control
                                        .operand_retentions
                                        .get(*profile)
                                        .immutable_image
                                    && self
                                        .context
                                        .control
                                        .retained_roots(*profile)
                                        .is_some_and(|roots| roots.is_empty())
                            })
                    });
                }
            }
            let mut origins = Vec::new();
            let mut known = true;
            let mut lifetime_known = true;
            let mut retained_roots = Vec::new();
            for place in places {
                let source = self
                    .append_captured_references(std::slice::from_ref(place))
                    .handles()
                    .next()
                    .unwrap_or_default();
                let Some(place) = place.clone().projected(&path) else {
                    known = false;
                    break;
                };
                let lifetime = self
                    .stored_retention_at_place(&place.place)
                    .unwrap_or_else(|| self.captured_retention_at_path(capture, &path));
                if let Some(roots) = self.context.control.retained_roots(lifetime) {
                    for root in roots {
                        if !retained_roots.contains(root) {
                            retained_roots.push(*root);
                        }
                    }
                } else {
                    lifetime_known = false;
                }
                let Some(contents) = self.stored_reference_contents_at_place(&place.place) else {
                    known = false;
                    break;
                };
                let alternatives = contents;
                for contents in alternatives {
                    match contents {
                        StoredReferenceContents::Captured { referents, .. } => {
                            let resolved = self.captured_reference_places(referents);
                            if resolved.is_empty() {
                                known = false;
                                break;
                            }
                            origins.extend(resolved);
                        }
                        // A missing local store is not an address. Only an actual
                        // incoming descriptor occurrence has a symbolic contents
                        // namespace; an unresolved stored descriptor above remains
                        // authoritative unknown, including after replacement.
                        StoredReferenceContents::Incoming => {
                            // Conditional inventory is not a descriptor read. Keep
                            // its owner path as an obligation for actual selection.
                            let segments = self
                                .context
                                .control
                                .operand_referent_segments
                                .insert_many(place.place.segments.iter().copied());
                            let path = self
                                .context
                                .control
                                .operand_child_paths
                                .insert_many(path.iter().copied());
                            let formation = self.context.control.referent_formations.append(
                                checked_trees::FlowReferentFormation {
                                    source,
                                    source_access: capture.case_access,
                                    owner,
                                    path,
                                    root: place.place.root,
                                    segments,
                                },
                            );
                            origins.push(CapturedReferencePlace {
                                place: place.place.clone(),
                                formation,
                            });
                        }
                        StoredReferenceContents::Unknown => {
                            known = false;
                            break;
                        }
                    }
                }
                if !known {
                    break;
                }
            }
            let referents = if known {
                self.append_captured_references(&origins)
            } else {
                HandleSpan::empty()
            };
            let retention = if lifetime_known && !places.is_empty() {
                self.append_retention(&retained_roots)
            } else {
                arena::Handle::invalid()
            };
            if self.context.control.operand_retentions.is_valid(retention) {
                let suppliers = suppliers
                    .map(|rows| {
                        let rows = rows
                            .into_iter()
                            .flatten()
                            .map(|selected| checked_trees::FlowOperandRetainedSupplier {
                                operand: selected.capture,
                                projection: self
                                    .context
                                    .control
                                    .operand_retained_paths
                                    .insert_many(selected.projection),
                            })
                            .collect::<Vec<_>>();
                        self.context
                            .control
                            .operand_retained_suppliers
                            .insert_many(rows)
                    })
                    .unwrap_or_default();
                let profile = self.context.control.operand_retentions.get_mut(retention);
                profile.suppliers = suppliers;
                profile.immutable_image = immutable_image;
            }
            let path = self.context.control.operand_child_paths.insert_many(path);
            leaves.push(checked_trees::FlowCapturedReferenceLeaf {
                path,
                type_reference: reference,
                referents,
                retention,
                constraints: capture.constraints,
            });
        }
        self.context
            .control
            .captured_reference_leaves
            .insert_many(leaves)
    }

    /// Instantiate returned descriptor leaves at invocation entry. The body
    /// relation may name a descriptor saved before a later callee replacement;
    /// querying after those effects would substitute the wrong occurrence.
    pub(super) fn capture_call_reference_leaves(
        &mut self,
        expression: ExpressionHandle,
        destination: typed_trees::types::TypeReferenceHandle,
        call: &typed_trees::expression::TableCallExpression,
        inputs: &[(ExpressionHandle, FlowOperandFact)],
        constraints: HandleSpan<checked_trees::FlowConstraintRef>,
    ) -> Option<HandleSpan<checked_trees::FlowCapturedReferenceLeaf>> {
        let returned = self.constructed_value_type(expression, destination)?;
        if self.is_view_type(returned) {
            return None;
        }
        let Some(paths) =
            validation::borrow_contracts::borrow_carrying_owner_paths(self.program, returned)
        else {
            return Some(HandleSpan::empty());
        };
        if paths.is_empty() {
            return None;
        }
        let Some((_, state)) = validation::find_state(self.program, call.target_symbol) else {
            if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                eprintln!(
                    "CHECKSITE returned-capture-refusal stage=target-state machine={:?} state={:?} statement={} call={:?} target={:?} inputs={}",
                    self.machine.symbol,
                    self.state.symbol,
                    self.statement_index,
                    expression,
                    call.target_symbol,
                    inputs.len(),
                );
            }
            return Some(HandleSpan::empty());
        };
        let parameters = self.program.state_parameters(state);
        let Some(indices) = self.captured_argument_indices(
            parameters,
            inputs
                .iter()
                .skip(usize::from(call.receiver.is_valid()))
                .map(|(_, value)| value),
        ) else {
            if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                eprintln!(
                    "CHECKSITE returned-capture-refusal stage=input-arity machine={:?} state={:?} statement={} call={:?} target={:?} formals={} inputs={} receiver={}",
                    self.machine.symbol,
                    self.state.symbol,
                    self.statement_index,
                    expression,
                    call.target_symbol,
                    parameters.len(),
                    inputs.len(),
                    call.receiver.is_valid(),
                );
            }
            return Some(HandleSpan::empty());
        };
        let machine_symbol = self.machine.symbol;
        let state_symbol = self.state.symbol;
        let statement_index = self.statement_index;
        let trace_refusal = |stage: &str, path: &[PlaceSegment], source_count: Option<usize>| {
            if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_none() {
                return;
            }
            eprintln!(
                "CHECKSITE returned-capture-refusal stage={} machine={:?} state={:?} statement={} call={:?} target={:?} formals={} formal_prefix={:?} inputs={} input_prefix={:?} index_count={} indices={:?} path_count={} path={:?} source_count={:?}",
                stage,
                machine_symbol,
                state_symbol,
                statement_index,
                expression,
                call.target_symbol,
                parameters.len(),
                parameters
                    .iter()
                    .take(4)
                    .map(|parameter| (parameter.symbol, parameter.type_reference))
                    .collect::<Vec<_>>(),
                inputs.len(),
                inputs
                    .iter()
                    .take(4)
                    .map(|(expression, value)| (
                        *expression,
                        value.expression,
                        value.state_symbol,
                        value.statement_index,
                        value.type_reference,
                        value.completion,
                        value.storage_snapshot,
                        value.constraints
                    ))
                    .collect::<Vec<_>>(),
                indices.len(),
                &indices[..indices.len().min(8)],
                path.len(),
                &path[..path.len().min(16)],
                source_count,
            );
        };
        let Some(sources) = super::super::reference_places::call_result_captured_sources(
            self.program,
            call,
            self.context.call_frames,
            &indices,
        ) else {
            trace_refusal("source-relation", &[], None);
            return Some(HandleSpan::empty());
        };
        let mut leaves = Vec::new();
        for path in &paths {
            let path = crate::borrow::owner_place_segments(path);
            let Some(reference) =
                validation::owned_projection_type_reference(self.program, returned, &path)
            else {
                return Some(HandleSpan::empty());
            };
            if !self.is_view_type(reference) {
                return Some(HandleSpan::empty());
            }
            let mut places = Vec::new();
            let mut complete = true;
            let mut matched = false;
            let mut refusal_stage = "no-matching-source";
            for source in &sources {
                let Some(output) = validation::reference_result_custody::loan_place_segments(
                    self.program,
                    returned,
                    &source.result_path,
                ) else {
                    return Some(HandleSpan::empty());
                };
                let suffix = match source.kind {
                    super::super::reference_places::ReferenceResultSourceKind::Reference => {
                        if output != path {
                            continue;
                        }
                        Vec::new()
                    }
                    super::super::reference_places::ReferenceResultSourceKind::Value {
                        type_reference,
                    } => {
                        let Some(suffix) = path.strip_prefix(output.as_slice()) else {
                            continue;
                        };
                        // Rebase nominal selectors through each instantiated
                        // type. The subtree describes slots; the completed
                        // actual below must supply their present descriptors.
                        let Some(suffix) = crate::borrow::structural_owner_path(
                            self.program,
                            &crate::borrow::owner_path_from_place_segments(self.program, suffix),
                        ) else {
                            return Some(HandleSpan::empty());
                        };
                        let projected = validation::reference_result_custody::loan_place_segments(
                            self.program,
                            type_reference,
                            &suffix,
                        )
                        .and_then(|path| {
                            validation::owned_projection_type_reference(
                                self.program,
                                type_reference,
                                &path,
                            )
                        });
                        if projected.is_none_or(|reference| !self.is_view_type(reference)) {
                            return Some(HandleSpan::empty());
                        }
                        suffix
                    }
                };
                matched = true;
                let Some(actual) = inputs
                    .get(source.argument_index + usize::from(call.receiver.is_valid()))
                    .map(|(_, actual)| actual)
                else {
                    refusal_stage = "actual-roster";
                    complete = false;
                    break;
                };
                let selected = crate::borrow::structural_owner_path(
                    self.program,
                    &crate::borrow::owner_path_from_place_segments(self.program, &source.segments),
                )
                .and_then(|mut path| {
                    path.extend_from_slice(&suffix);
                    validation::reference_result_custody::loan_place_segments(
                        self.program,
                        actual.type_reference,
                        &path,
                    )
                });
                let Some(selected) = selected else {
                    refusal_stage = "selected-path";
                    complete = false;
                    break;
                };
                let Some(projected) = self.capture_reference_projection(
                    actual,
                    &selected,
                    super::reference_projection::ReferenceProjection::Value,
                ) else {
                    refusal_stage = "captured-projection";
                    complete = false;
                    break;
                };
                if projected.places.is_empty() {
                    refusal_stage = "empty-pointee";
                    complete = false;
                    break;
                }
                for place in projected.places {
                    if !places.contains(&place) {
                        places.push(place);
                    }
                }
            }
            let referents = if complete && matched {
                self.append_captured_references(&places)
            } else {
                trace_refusal(refusal_stage, &path, Some(sources.len()));
                if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                    for source in sources.iter().take(4) {
                        eprintln!(
                            "CHECKSITE returned-capture-source source_count={} value={} argument_index={} parameter_type={:?} source_path_count={} source_path={:?} result_path_count={} result_path={:?}",
                            sources.len(),
                            matches!(
                                source.kind,
                                super::super::reference_places::ReferenceResultSourceKind::Value { .. }
                            ),
                            source.argument_index,
                            source.parameter_type,
                            source.segments.len(),
                            &source.segments[..source.segments.len().min(16)],
                            source.result_path.len(),
                            &source.result_path[..source.result_path.len().min(16)],
                        );
                    }
                }
                HandleSpan::empty()
            };
            let path = self.context.control.operand_child_paths.insert_many(path);
            leaves.push(checked_trees::FlowCapturedReferenceLeaf {
                path,
                type_reference: reference,
                referents,
                retention: arena::Handle::invalid(),
                constraints,
            });
        }
        Some(
            self.context
                .control
                .captured_reference_leaves
                .insert_many(leaves),
        )
    }

    /// Address capture precedes invocation effects; lifetime publication uses
    /// the result's complete declared profile, independently of that geometry.
    pub(super) fn publish_returned_reference_leaves(
        &mut self,
        capture: &FlowOperandFact,
        references: HandleSpan<checked_trees::FlowCapturedReferenceLeaf>,
    ) -> HandleSpan<checked_trees::FlowCapturedReferenceLeaf> {
        let mut leaves = self
            .context
            .control
            .captured_reference_leaves
            .span_or_empty(references)
            .to_vec();
        for leaf in &mut leaves {
            let path = self
                .context
                .control
                .operand_child_paths
                .span_or_empty(leaf.path)
                .to_vec();
            leaf.retention = self.captured_retention_at_path(*(capture), &path);
            leaf.constraints = capture.constraints;
        }
        self.context
            .control
            .captured_reference_leaves
            .insert_many(leaves)
    }

    pub(super) fn is_incoming_descriptor_place(&self, place: &CanonicalPlace) -> bool {
        if place.root.as_symbol().is_some_and(|symbol| {
            self.program
                .state_parameter_by_symbol(self.state, symbol)
                .is_some()
        }) {
            // The leaf schema was projected from this parameter's actual
            // declared carrier above. This names its incoming value only;
            // independent loan checking still owns every access permission.
            return true;
        }
        let Some((_, state)) = self
            .borrow
            .first_state_borrow(self.machine.symbol, self.state.symbol)
        else {
            return false;
        };
        self.borrow
            .loans
            .span_or_empty(state.loans)
            .iter()
            .any(|loan| {
                if !self
                    .borrow
                    .incoming_references
                    .is_valid(loan.incoming_reference)
                {
                    return false;
                }
                let incoming = self.borrow.incoming_references.get(loan.incoming_reference);
                let root = incoming.place_root();
                let Some(owner) = self.borrow.owner_segments.span(incoming.owner_path) else {
                    return false;
                };
                incoming.machine_symbol == self.machine.symbol
                    && incoming.state_symbol == self.state.symbol
                    && root == place.root
                    && owner.len() == place.segments.len()
                    && owner.iter().zip(&place.segments).all(|(owner, segment)| {
                        crate::borrow::owner_segment_matches_place_segment(
                            self.program,
                            owner,
                            segment,
                        ) == Some(true)
                    })
            })
    }

    /// Project an owned field from its receiver's completed address. Reading a
    /// view field instead loads its stored descriptor; the address of that
    /// descriptor is not the address it denotes.
    pub(super) fn capture_member_referents(
        &mut self,
        expression: ExpressionHandle,
        receiver: &FlowOperandFact,
        constraints: HandleSpan<checked_trees::FlowConstraintRef>,
        case_access: &mut arena::Handle<checked_trees::FlowCaseAccessFact>,
    ) -> HandleSpan<FlowOperandReferent> {
        let is_view = expression != self.reference_slot_target
            && self
                .context
                .expression_result_type_at(self.program, self.machine, self.state, expression)
                .is_some_and(|reference| self.is_view_type(reference));
        let Some(member) = self.program.expression_table.member_node(expression) else {
            return HandleSpan::empty();
        };
        let symbol = validation::effective_member_symbol(self.program, member.receiver, member);
        if !symbol.is_valid() {
            return HandleSpan::empty();
        }
        let mut path = Vec::new();
        if let Some(variant) = facts::payload_variant_for_field(self.program, symbol) {
            path.push(PlaceSegment::Case { variant });
        }
        path.push(PlaceSegment::Field { symbol });
        let places = self
            .captured_reference_places(receiver.referents)
            .into_iter()
            .map(|place| place.projected(&path))
            .collect::<Option<Vec<_>>>();
        let Some(places) = places else {
            return HandleSpan::empty();
        };
        // The actual receiver names the descriptor carrier. Loading its saved
        // pointee must not replace this independent payload-formation obligation.
        let source_places = self.append_captured_references(&places);
        let contexts = self
            .context
            .contexts
            .semantic_constraint_contexts(constraints)
            .collect::<Vec<_>>();
        let case_places = super::super::observations::descriptor_observation_namespace(
            self.program,
            &self.context.control,
            receiver,
            &contexts,
        )
        .and_then(|namespace| super::super::project_captured_view_place(namespace, &path))
        .map(|namespace| self.append_referent_places(&[namespace]))
        .unwrap_or(source_places);
        *case_access = self.record_case_access_places(expression, constraints, case_places);
        if is_view {
            if let Some(captured) = self.captured_projection_referents(receiver, &path) {
                return captured;
            }
            let value = self.context.control.operands.insert(*receiver);
            if let Some(selected) = super::super::stored_views::reference_places_at_snapshot(
                self.program,
                &self.context.control,
                value,
                &path,
                self.storage_snapshot,
                |place| self.is_incoming_descriptor_place(place),
            ) {
                return self.append_captured_references(&selected);
            }
            self.load_reference_places(places)
        } else {
            self.append_captured_references(&places)
        }
    }

    /// A completed owned value keeps its own descriptor leaves. A storage
    /// address instead loads the current descriptor after selector effects.
    /// Missing captured leaves are authoritative unknown, never a slot address.
    fn captured_projection_referents(
        &mut self,
        receiver: &FlowOperandFact,
        path: &[PlaceSegment],
    ) -> Option<HandleSpan<FlowOperandReferent>> {
        if self.is_view_type(receiver.type_reference)
            || (matches!(
                receiver.completion,
                checked_trees::FlowOperandCompletion::Observation
            ) && receiver.reference_leaves.is_empty())
        {
            return None;
        }
        let places =
            super::super::operand_values::captured_operand_reference_places_with_formation(
                self.program,
                &self.context.control,
                receiver,
                receiver.type_reference,
                path,
            )
            .unwrap_or_default();
        Some(self.append_captured_references(&places))
    }

    fn load_reference_places(
        &mut self,
        sources: Vec<CapturedReferencePlace>,
    ) -> HandleSpan<FlowOperandReferent> {
        let mut places = Vec::new();
        for source in sources {
            let Some(contents) = self.stored_reference_contents_at_place(&source.place) else {
                return HandleSpan::empty();
            };
            let alternatives = contents;
            for contents in alternatives {
                match contents {
                    StoredReferenceContents::Captured { referents, .. } => {
                        let captured = self.captured_reference_places(referents);
                        if captured.is_empty() {
                            return HandleSpan::empty();
                        }
                        places.extend(captured);
                    }
                    StoredReferenceContents::Incoming => places.push(source.clone()),
                    StoredReferenceContents::Unknown => {
                        return HandleSpan::empty();
                    }
                }
            }
        }
        self.append_captured_references(&places)
    }

    /// A checked return route substitutes the tuple that actually ran. Its
    /// reference arguments already retain their exact evaluated addresses,
    /// including anonymous owners and selectors evaluated before later inputs.
    /// Re-reading their source syntax would lose or replace those addresses.
    /// Lifetime retention and live loan authority remain independent checks.
    pub(super) fn capture_returned_referents(
        &mut self,
        expression: ExpressionHandle,
        call: &typed_trees::expression::TableCallExpression,
        inputs: &[(ExpressionHandle, FlowOperandFact)],
        referents: &mut Option<HandleSpan<FlowOperandReferent>>,
    ) {
        let Some(reference) = self.context.expression_result_type_at(
            self.program,
            self.machine,
            self.state,
            expression,
        ) else {
            return;
        };
        if !self.is_view_type(reference) {
            return;
        }
        // A produced unknown result is authoritative, just as an unresolved
        // saved descriptor is. No initializer supplies a replacement address.
        *referents = Some(HandleSpan::empty());
        let Some(sources) = super::super::reference_places::call_result_leaf_sources(
            self.program,
            call,
            self.context.call_frames,
        ) else {
            return;
        };
        let mut origins = Vec::new();
        for source in sources {
            if !source.result_path.is_empty() {
                return;
            }
            let Some((_, actual)) =
                inputs.get(source.argument_index + usize::from(call.receiver.is_valid()))
            else {
                return;
            };
            let Some(projected) = self.capture_reference_projection(
                actual,
                &source.segments,
                super::reference_projection::ReferenceProjection::Value,
            ) else {
                return;
            };
            let places = projected.places;
            for place in places {
                if !origins.contains(&place) {
                    origins.push(place);
                }
            }
        }
        *referents = Some(self.append_captured_references(&origins));
    }

    /// Read the completed value installed by a reaching store. A matching
    /// unresolved capture or an intervening replacement is authoritative:
    /// neither may fall back to the declaration's initializer.
    pub(super) fn stored_view_referents(
        &mut self,
        expression: ExpressionHandle,
    ) -> Option<HandleSpan<FlowOperandReferent>> {
        let source = validation::canonical_place_from_expression_in_state(
            self.program,
            self.state.symbol,
            self.statement_index,
            expression,
        )?;
        self.stored_view_referents_at_place(&source)
    }

    /// Resolve the actual stored descriptor through the current immutable cut.
    /// Expression and typed projection callers share this one association owner.
    pub(super) fn stored_view_referents_at_place(
        &mut self,
        source: &CanonicalPlace,
    ) -> Option<HandleSpan<FlowOperandReferent>> {
        let contents = self.stored_reference_contents_at_place(source)?;
        let mut places = Vec::new();
        for alternative in contents {
            match alternative {
                StoredReferenceContents::Incoming => {
                    places.push(CapturedReferencePlace::unformed(source.clone()))
                }
                StoredReferenceContents::Captured { referents, .. } => {
                    let captured = self.captured_reference_places(referents);
                    if captured.is_empty() {
                        return Some(HandleSpan::empty());
                    }
                    places.extend(captured);
                }
                StoredReferenceContents::Unknown => return Some(HandleSpan::empty()),
            }
        }
        Some(self.append_captured_references(&places))
    }

    /// Snapshot storage is the sole association owner. Unknown/malformed
    /// predecessors remain unknown; only an actual ingress keeps its namespace.
    pub(super) fn stored_reference_contents_at_place(
        &mut self,
        source: &CanonicalPlace,
    ) -> Option<Vec<StoredReferenceContents>> {
        if source
            .segments
            .iter()
            .any(|segment| matches!(segment, PlaceSegment::Index { .. }))
        {
            let root = CanonicalPlace {
                root: source.root,
                segments: Vec::new(),
            };
            let reference = validation::canonical_place_type_reference(
                self.program,
                self.state.symbol,
                self.statement_index,
                &root,
            )?;
            let paths = super::super::operand_values::finite_projection_paths(
                self.program,
                reference,
                &source.segments,
            )?;
            let mut alternatives = Vec::new();
            for segments in paths {
                alternatives.extend(self.stored_reference_contents_at_place(&CanonicalPlace {
                    root: source.root,
                    segments,
                })?);
            }
            return (!alternatives.is_empty()).then_some(alternatives);
        }
        let Some(stored) = super::super::stored_views::capture_at_snapshot(
            self.program,
            &self.context.control,
            self.state,
            self.storage_snapshot,
            source,
        ) else {
            return self
                .is_incoming_descriptor_place(source)
                .then_some(vec![StoredReferenceContents::Incoming]);
        };
        let mut pending = vec![stored];
        let mut alternatives = Vec::new();
        while let Some(stored) = pending.pop() {
            match stored {
                super::super::stored_values::StoredOperand::Unavailable => {
                    return Some(vec![StoredReferenceContents::Unknown]);
                }
                super::super::stored_values::StoredOperand::Incoming(place) => {
                    if place != *source || !self.is_incoming_descriptor_place(&place) {
                        return Some(vec![StoredReferenceContents::Unknown]);
                    }
                    alternatives.push(StoredReferenceContents::Incoming);
                }
                super::super::stored_values::StoredOperand::Alternatives(values) => {
                    pending.extend(values.into_iter().rev());
                }
                super::super::stored_values::StoredOperand::Value {
                    capture,
                    projection,
                    ..
                } => {
                    let places = super::super::stored_views::reference_places_at_snapshot(
                        self.program,
                        &self.context.control,
                        capture,
                        &projection,
                        self.storage_snapshot,
                        |place| self.is_incoming_descriptor_place(place),
                    )
                    .unwrap_or_default();
                    let referents = self.append_captured_references(&places);
                    let capture = *self.context.control.operands.get(capture);
                    let retention = self.captured_retention_at_path(capture, &projection);
                    alternatives.push(StoredReferenceContents::Captured {
                        referents,
                        retention,
                    });
                }
            }
        }
        (!alternatives.is_empty()).then_some(alternatives)
    }

    pub(super) fn capture_indexed_referents(
        &mut self,
        expression: ExpressionHandle,
        collection: &FlowOperandFact,
        start: &FlowOperandFact,
        end: Option<&FlowOperandFact>,
    ) -> HandleSpan<FlowOperandReferent> {
        if !validation::indexed_operation_has_builtin_meaning(
            self.program,
            self.machine,
            Some(self.state),
            expression,
            Some(collection.type_reference),
        ) || [Some(start), end].into_iter().flatten().any(|bound| {
            bound.expression.is_valid()
                && !validation::has_builtin_bound_expression_meaning(
                    self.program,
                    self.machine,
                    Some(self.state),
                    bound.expression,
                )
        }) {
            return HandleSpan::empty();
        }
        let indexed = self.program.expression_table.indexed_node(expression);
        let Some(indexed) = indexed else {
            return HandleSpan::empty();
        };
        let mut places = self.captured_collection_places(collection);
        let geometry = if let Some(end) = end {
            let Some(range) = self.program.expression_table.range_node(indexed.index) else {
                return HandleSpan::empty();
            };
            let start_coordinate = if start.expression.is_valid() {
                self.captured_coordinate(start)
            } else {
                Some(0)
            };
            // The whole interval denotes the captured collection itself,
            // even when its runtime extent remains symbolic. This equality
            // uses selected builtin meaning, not a declaration's initializer.
            if start_coordinate == Some(0) && !end.expression.is_valid() && !range.end_inclusive {
                return self.append_captured_references(&places);
            }
            let end_coordinate = if end.expression.is_valid() {
                self.captured_coordinate(end).and_then(|end| {
                    if range.end_inclusive {
                        end.checked_add(1)
                    } else {
                        Some(end)
                    }
                })
            } else {
                let mut extent = None;
                for place in &places {
                    let Some(current) = crate::checks::collection_extents::at_formed_place(
                        self.program,
                        self.state.symbol,
                        self.statement_index,
                        &place.place,
                    ) else {
                        return HandleSpan::empty();
                    };
                    if extent.is_some_and(|previous| previous != current) {
                        return HandleSpan::empty();
                    }
                    extent = Some(current);
                }
                extent
            };
            match (start_coordinate, end_coordinate) {
                (Some(start), Some(end)) if start <= end => PlaceSegment::FixedRange { start, end },
                // The captured descriptor still denotes a window when its
                // bounds are unknown. A point would consume this dimension
                // and move every later selector onto the wrong nested array.
                _ => PlaceSegment::OpaqueRange,
            }
        } else {
            match self.captured_coordinate(start) {
                Some(index) => PlaceSegment::FixedIndex { index },
                None => PlaceSegment::Index {
                    expression: indexed.index,
                },
            }
        };
        // Element selection may read a stored reference descriptor. A range
        // instead forms a view of the collection and keeps its window address.
        let reads_descriptor = end.is_none()
            && expression != self.reference_slot_target
            && self
                .context
                .expression_result_type_at(self.program, self.machine, self.state, expression)
                .is_some_and(|reference| self.is_view_type(reference));
        if reads_descriptor
            && let Some(referents) = self.captured_projection_referents(collection, &[geometry])
        {
            return referents;
        }
        for place in &mut places {
            let Some(segments) =
                facts::project_captured_view_segments(&place.place.segments, &[geometry])
            else {
                return HandleSpan::empty();
            };
            place.place.segments = segments;
        }
        if reads_descriptor {
            self.load_reference_places(places)
        } else {
            self.append_captured_references(&places)
        }
    }

    /// Both descriptor stores and returned values consume this exact completed
    /// argument roster. Formal carrier checks decide eligible integer lanes;
    /// captured_coordinate reads their immutable observations, not current homes.
    pub(super) fn captured_argument_indices<'inputs>(
        &self,
        parameters: &[typed_trees::signature::StateParameter],
        inputs: impl ExactSizeIterator<Item = &'inputs FlowOperandFact>,
    ) -> Option<Vec<Option<usize>>> {
        if parameters.len() != inputs.len() {
            return None;
        }
        Some(
            parameters
                .iter()
                .zip(inputs)
                .map(|(parameter, input)| {
                    if validation::reference_shell(self.program, parameter.type_reference).is_none()
                        && self
                            .program
                            .primitive_type_reference(parameter.type_reference)
                            .is_some_and(|primitive| primitive.is_fixed_integer())
                    {
                        self.captured_coordinate(input)
                    } else {
                        None
                    }
                })
                .collect(),
        )
    }

    pub(super) fn captured_coordinate(&self, operand: &FlowOperandFact) -> Option<usize> {
        validation::captured_integer_value(
            self.program,
            self.machine,
            self.state,
            operand.expression,
            |expression, place| {
                // Authored literals belong to the arithmetic engine. Only
                // actual storage observations can bind its captured places.
                if !matches!(place.root, PlaceRoot::Symbol(_)) {
                    return None;
                }
                let place = super::super::observations::completed_copy_read_place(
                    self.program,
                    &self.context.control,
                    self.context.active_completed_copies,
                    self.state.symbol,
                    self.statement_index,
                    expression,
                    place,
                )?;
                let value = crate::values::scalar_value_at_place(
                    self.program,
                    self.semantic,
                    self.context
                        .contexts
                        .constraint_refs
                        .span_or_empty(operand.constraints)
                        .iter()
                        .filter_map(|reference| {
                            let FlowConstraintKind::SemanticContext { context } = reference.kind
                            else {
                                return None;
                            };
                            Some(self.semantic.contexts.get(context))
                        }),
                    &place,
                )?;
                match value {
                    ScalarValue::Integer(value) => Some(value),
                    _ => None,
                }
            },
        )?
        .to_u64()
        .and_then(|value| usize::try_from(value).ok())
    }

    fn captured_collection_places(&self, operand: &FlowOperandFact) -> Vec<CapturedReferencePlace> {
        // Collection operands are evaluated as places. An empty capture is
        // authoritative, including for owned fields behind an unresolved view.
        self.captured_reference_places(operand.referents)
    }

    pub(super) fn captured_reference_places(
        &self,
        referents: HandleSpan<FlowOperandReferent>,
    ) -> Vec<CapturedReferencePlace> {
        self.context
            .control
            .operand_referents
            .span_or_empty(referents)
            .iter()
            .map(|referent| CapturedReferencePlace::from_referent(&self.context.control, referent))
            .collect::<Option<Vec<_>>>()
            .unwrap_or_default()
    }

    pub(super) fn append_captured_references(
        &mut self,
        places: &[CapturedReferencePlace],
    ) -> HandleSpan<FlowOperandReferent> {
        let mut rows = Vec::new();
        let mut seen = Vec::new();
        for reference in places {
            if seen.contains(&reference) {
                continue;
            }
            seen.push(reference);
            let segments = self
                .context
                .control
                .operand_referent_segments
                .insert_many(reference.place.segments.iter().copied());
            rows.push(FlowOperandReferent {
                root: reference.place.root,
                segments,
                formation: reference.formation,
            });
        }
        self.context.control.operand_referents.insert_many(rows)
    }

    /// A real descriptor load completes its pending owner-case obligation at
    /// this read. Its endpoint becomes immutable provenance; later pointee
    /// projections still consume current contents and current case evidence.
    pub(super) fn complete_reference_formation(
        &mut self,
        expression: ExpressionHandle,
        constraints: HandleSpan<checked_trees::FlowConstraintRef>,
        referents: HandleSpan<FlowOperandReferent>,
    ) -> HandleSpan<FlowOperandReferent> {
        let access = self.record_case_access_places(expression, constraints, referents);
        if !access.is_valid() {
            return referents;
        }
        let mut completed = Vec::new();
        for source in referents.handles() {
            let Some(mut reference) = CapturedReferencePlace::from_referent(
                &self.context.control,
                self.context.control.operand_referents.get(source),
            ) else {
                return HandleSpan::empty();
            };
            let segments = self
                .context
                .control
                .operand_referent_segments
                .insert_many(reference.place.segments.iter().copied());
            reference.formation = self.context.control.referent_formations.append(
                checked_trees::FlowReferentFormation {
                    source,
                    source_access: access,
                    owner: arena::Handle::invalid(),
                    path: HandleSpan::empty(),
                    root: reference.place.root,
                    segments,
                },
            );
            completed.push(reference);
        }
        self.append_captured_references(&completed)
    }

    pub(super) fn captured_referent_places(
        &self,
        referents: HandleSpan<FlowOperandReferent>,
    ) -> Vec<CanonicalPlace> {
        self.context
            .control
            .operand_referents
            .span_or_empty(referents)
            .iter()
            .map(|referent| CanonicalPlace {
                root: referent.root,
                segments: self
                    .context
                    .control
                    .operand_referent_segments
                    .span_or_empty(referent.segments)
                    .to_vec(),
            })
            .collect()
    }

    pub(super) fn append_referent_places(
        &mut self,
        places: &[CanonicalPlace],
    ) -> HandleSpan<FlowOperandReferent> {
        let rows = places
            .iter()
            .map(|place| FlowOperandReferent {
                formation: arena::Handle::invalid(),
                root: place.root,
                segments: self
                    .context
                    .control
                    .operand_referent_segments
                    .insert_many(place.segments.iter().copied()),
            })
            .collect::<Vec<_>>();
        self.context.control.operand_referents.insert_many(rows)
    }
}
