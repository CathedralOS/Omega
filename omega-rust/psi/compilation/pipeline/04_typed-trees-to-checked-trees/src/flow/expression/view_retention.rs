//! Captured lifetime dependencies are independent of view address geometry.
//! Declaration relations keep every input lifetime, including inputs a body
//! does not select. Actual operand captures substitute those lifetimes once;
//! they never turn a declaration fallback into an address or extent proof.
//! An unchanged state input already owns its admitted reference frontier.
//! Retain those leaves against the input owner even when no local store has
//! produced a capture; selecting a case payload cannot erase that dependency.
use super::Execution;
use arena::{Handle, HandleSpan};
use checked_trees::{FlowOperandFact, FlowOperandReferent, FlowOperandRetention};
use facts::{PlaceRoot, PlaceSegment};
use typed_trees::expression::ExpressionHandle;
use validation::CanonicalPlace;

impl Execution<'_, '_, '_> {
    /// Keep every completed descriptor profile at a selection. The joined
    /// roots answer lifetime-only queries; alternatives retain the actual
    /// stored contents needed by a later projection through a descriptor.
    fn captured_retention_at_path(
        &mut self,
        capture: &FlowOperandFact,
        path: &[PlaceSegment],
    ) -> Handle<FlowOperandRetention> {
        let Some(profiles) = super::super::operand_values::captured_retention_profiles_at_path(
            self.program,
            &self.context.control,
            capture,
            path,
        ) else {
            return Handle::invalid();
        };
        self.join_retention_profiles(&profiles)
    }

    fn join_retention_profiles(
        &mut self,
        profiles: &[Handle<FlowOperandRetention>],
    ) -> Handle<FlowOperandRetention> {
        if let [profile] = profiles {
            return *profile;
        }
        if profiles.is_empty() {
            return Handle::invalid();
        }
        let mut roots = Vec::new();
        let mut alternatives = Vec::new();
        for profile in profiles {
            let Some(retained) = self.context.control.retained_roots(*profile) else {
                return Handle::invalid();
            };
            for root in retained {
                if !roots.contains(root) {
                    roots.push(*root);
                }
            }
            alternatives.push(*self.context.control.operand_retentions.get(*profile));
        }
        let roots = self
            .context
            .control
            .operand_retained_roots
            .insert_many(roots);
        let alternatives = self
            .context
            .control
            .operand_retentions
            .insert_many(alternatives);
        self.context
            .control
            .operand_retentions
            .insert(FlowOperandRetention {
                roots,
                alternatives,
                ..Default::default()
            })
    }

    /// Every possible result must cover each declared lifetime leaf at its
    /// own completion. Joining dependencies never joins address geometry or
    /// turns mutually exclusive values into simultaneous product children.
    pub(super) fn capture_alternative_retention(
        &mut self,
        expression: ExpressionHandle,
        alternatives: HandleSpan<FlowOperandFact>,
    ) -> Handle<FlowOperandRetention> {
        let Some(reference) = self.context.expression_result_type_at(
            self.program,
            self.machine,
            self.state,
            expression,
        ) else {
            return Handle::invalid();
        };
        let alternatives = self
            .context
            .control
            .operands
            .span_or_empty(alternatives)
            .iter()
            .copied()
            .map(|capture| (capture, Vec::new()))
            .collect::<Vec<_>>();
        self.capture_retention_join(reference, &alternatives)
    }

    /// Join complete declared lifetime leaves across selected completed
    /// values. Branch results and runtime element choices differ in geometry,
    /// not in the requirement that every candidate cover each retained leaf.
    fn capture_retention_join(
        &mut self,
        reference: typed_trees::types::TypeReferenceHandle,
        selections: &[(FlowOperandFact, Vec<PlaceSegment>)],
    ) -> Handle<FlowOperandRetention> {
        let Some(paths) =
            validation::borrow_contracts::borrow_carrying_owner_paths(self.program, reference)
        else {
            return Handle::invalid();
        };
        if selections.is_empty() {
            return Handle::invalid();
        }
        let mut leaves = Vec::new();
        for path in paths {
            let path = crate::borrow::owner_place_segments(&path);
            let mut profiles = Vec::new();
            for (capture, prefix) in selections {
                let mut selected_path = prefix.clone();
                selected_path.extend_from_slice(&path);
                let leaf = self.captured_retention_at_path(capture, &selected_path);
                if self.context.control.retained_roots(leaf).is_none() {
                    return Handle::invalid();
                }
                if !profiles.contains(&leaf) {
                    profiles.push(leaf);
                }
            }
            let retention = self.join_retention_profiles(&profiles);
            if path.is_empty() {
                return retention;
            }
            let path = self
                .context
                .control
                .operand_retained_paths
                .insert_many(path);
            leaves.push(checked_trees::FlowOperandRetainedLeaf { path, retention });
        }
        let leaves = self
            .context
            .control
            .operand_retained_leaves
            .insert_many(leaves);
        self.context
            .control
            .operand_retentions
            .insert(FlowOperandRetention {
                roots: HandleSpan::empty(),
                alternatives: HandleSpan::empty(),
                leaves,
                stored_value: Handle::invalid(),
                stored_projection: HandleSpan::empty(),
            })
    }

    pub(super) fn append_retention(&mut self, roots: &[PlaceRoot]) -> Handle<FlowOperandRetention> {
        let roots = self
            .context
            .control
            .operand_retained_roots
            .insert_many(roots.iter().copied());
        self.context
            .control
            .operand_retentions
            .insert(FlowOperandRetention {
                roots,
                alternatives: HandleSpan::empty(),
                leaves: HandleSpan::empty(),
                stored_value: Handle::invalid(),
                stored_projection: HandleSpan::empty(),
            })
    }

    pub(super) fn retention_from_referents(
        &mut self,
        referents: HandleSpan<FlowOperandReferent>,
    ) -> Handle<FlowOperandRetention> {
        let mut roots = Vec::new();
        for referent in self
            .context
            .control
            .operand_referents
            .span_or_empty(referents)
        {
            if !roots.contains(&referent.root) {
                roots.push(referent.root);
            }
        }
        if roots.is_empty() {
            Handle::invalid()
        } else {
            self.append_retention(&roots)
        }
    }

    /// Keep the authenticated stored value and its selected region together.
    /// No child capture or geometric observation is manufactured by this link.
    fn projected_value_retention(
        &mut self,
        capture: FlowOperandFact,
        projection: &[PlaceSegment],
    ) -> Handle<FlowOperandRetention> {
        let stored_value = self.context.control.operands.insert(capture);
        let stored_projection = self
            .context
            .control
            .operand_retained_paths
            .insert_many(projection.iter().copied());
        self.context
            .control
            .operand_retentions
            .insert(FlowOperandRetention {
                roots: HandleSpan::empty(),
                alternatives: HandleSpan::empty(),
                leaves: HandleSpan::empty(),
                stored_value,
                stored_projection,
            })
    }

    fn stored_retention_at_place(
        &mut self,
        source: &CanonicalPlace,
    ) -> Option<Handle<FlowOperandRetention>> {
        let statements = super::super::reference_spans::appended_span_since(
            &self.context.control.statements,
            self.context.state_statements_start,
        );
        let capture = super::super::stored_views::capture_before_statement(
            self.program,
            self.semantic,
            &self.context.control,
            self.state,
            statements,
            self.statement_index,
            self.entry_origin_writes,
            self.entry_origin_unknown_write,
            source,
        )?;
        Some(match capture {
            super::super::stored_values::StoredOperand::Unavailable => Handle::invalid(),
            super::super::stored_values::StoredOperand::Value {
                capture,
                projection,
                ..
            } => {
                let capture = *self.context.control.operands.get(capture);
                self.captured_retention_at_path(&capture, &projection)
            }
        })
    }

    pub(super) fn stored_operand_retention(
        &mut self,
        expression: ExpressionHandle,
    ) -> Option<Handle<FlowOperandRetention>> {
        let source = validation::canonical_place_from_expression_in_state(
            self.program,
            self.state.symbol,
            self.statement_index,
            expression,
        )?;
        self.stored_retention_at_place(&source)
    }

    /// State admission establishes each reference leaf of its exact input.
    /// A whole-owner binding query ignores descendant field stores, so check
    /// each leaf's reaching value independently: replacements retain their
    /// completed capture, unknown writes stay unknown, and only an unchanged
    /// leaf retains the input owner. None supplies an address or new access.
    fn incoming_value_retention(
        &mut self,
        source: &CanonicalPlace,
    ) -> Option<Handle<FlowOperandRetention>> {
        let parameter = self
            .program
            .state_parameter_by_symbol(self.state, source.root.as_symbol()?)?;
        let paths = validation::borrow_contracts::borrow_carrying_owner_paths(
            self.program,
            parameter.type_reference,
        )?;
        let statements = super::super::reference_spans::appended_span_since(
            &self.context.control.statements,
            self.context.state_statements_start,
        );
        let mut leaves = Vec::new();
        for path in paths {
            let path = crate::borrow::owner_place_segments(&path);
            if path.len() < source.segments.len()
                || !source.segments.iter().zip(&path).all(|(left, right)| {
                    super::super::canonical_place_segments_equal(self.program, *left, *right)
                })
            {
                continue;
            }
            let leaf = CanonicalPlace {
                root: source.root,
                segments: path.clone(),
            };
            let retention = match super::super::stored_views::capture_before_statement(
                self.program,
                self.semantic,
                &self.context.control,
                self.state,
                statements,
                self.statement_index,
                self.entry_origin_writes,
                self.entry_origin_unknown_write,
                &leaf,
            ) {
                Some(super::super::stored_values::StoredOperand::Value {
                    capture,
                    projection,
                    ..
                }) => {
                    let capture = *self.context.control.operands.get(capture);
                    self.captured_retention_at_path(&capture, &projection)
                }
                Some(super::super::stored_values::StoredOperand::Unavailable) => Handle::invalid(),
                None => self.append_retention(&[source.root]),
            };
            let remaining = &path[source.segments.len()..];
            if remaining.is_empty() {
                return Some(retention);
            }
            let path = self
                .context
                .control
                .operand_retained_paths
                .insert_many(remaining.iter().copied());
            leaves.push(checked_trees::FlowOperandRetainedLeaf { path, retention });
        }
        if leaves.is_empty() {
            return None;
        }
        let leaves = self
            .context
            .control
            .operand_retained_leaves
            .insert_many(leaves);
        Some(
            self.context
                .control
                .operand_retentions
                .insert(FlowOperandRetention {
                    roots: HandleSpan::empty(),
                    alternatives: HandleSpan::empty(),
                    leaves,
                    stored_value: Handle::invalid(),
                    stored_projection: HandleSpan::empty(),
                }),
        )
    }

    pub(super) fn capture_storage_retention(
        &mut self,
        capture: &FlowOperandFact,
        value_retention: Option<Handle<FlowOperandRetention>>,
        backing: Handle<FlowOperandFact>,
    ) -> Handle<FlowOperandRetention> {
        // Storage dependency comes from the actual receiver capture even when
        // its address join is unresolved. The selected value profile below is
        // independent: neither parent contents nor later stores replace it.
        let backing_fact = if backing.is_valid() {
            if !self.context.control.operands.is_valid(backing) {
                return Handle::invalid();
            }
            Some(*self.context.control.operands.get(backing))
        } else {
            None
        };
        let storage_retention = backing_fact
            .map(|fact| fact.retention)
            .unwrap_or(capture.retention);
        let reference_free = validation::borrow_contracts::borrow_carrying_owner_paths(
            self.program,
            capture.type_reference,
        )
        .is_some_and(|paths| paths.is_empty());
        // A projected view keeps its denoted storage on the lifetime walk.
        // `retained_roots` stops at that projection, but a reference-free
        // field of the referent still occupies the same caller storage.
        // When the projection has no resolvable leaf, the view's captured
        // referents are that storage.
        let roots = if let Some(roots) = self
            .context
            .control
            .retained_roots(storage_retention)
            .map(<[_]>::to_vec)
        {
            roots
        } else if reference_free
            && let Some(fact) = backing_fact
            && self.is_view_type(fact.type_reference)
        {
            let mut roots = super::super::operand_values::captured_lifetime_roots(
                self.program,
                &self.context.control,
                &fact,
            )
            .filter(|roots| !roots.is_empty())
            .unwrap_or_default();
            if roots.is_empty() {
                for place in self.captured_referent_places(fact.referents) {
                    if place.root.is_valid() && !roots.contains(&place.root) {
                        roots.push(place.root);
                    }
                }
            }
            if roots.is_empty() {
                return Handle::invalid();
            }
            roots
        } else {
            return Handle::invalid();
        };
        if reference_free {
            // The parent supplies storage roots, never the contents of the
            // selected slot. A scalar place carries no reference leaves.
            return self.append_retention(&roots);
        }
        let mut value = *capture;
        if let Some(retention) = value_retention {
            value.retention = retention;
        }
        // A just-completed value profile is authoritative, including an
        // explicit unknown. Reaching-store lookup describes resident storage;
        // it cannot replace this evaluation's result after its own writes.
        if capture.children.is_empty()
            && value_retention.is_none()
            && let Some(source) = validation::canonical_place_from_expression_in_state(
                self.program,
                self.state.symbol,
                self.statement_index,
                capture.expression,
            )
        {
            let statements = super::super::reference_spans::appended_span_since(
                &self.context.control.statements,
                self.context.state_statements_start,
            );
            match super::super::stored_views::capture_before_statement(
                self.program,
                self.semantic,
                &self.context.control,
                self.state,
                statements,
                self.statement_index,
                self.entry_origin_writes,
                self.entry_origin_unknown_write,
                &source,
            ) {
                Some(super::super::stored_values::StoredOperand::Value {
                    capture,
                    projection,
                    ..
                }) => {
                    let capture = *self.context.control.operands.get(capture);
                    if projection.is_empty() {
                        value = capture;
                    } else {
                        value.retention = self.projected_value_retention(capture, &projection);
                    }
                }
                Some(_) => return self.append_retention(&roots),
                None => {
                    if let Some(retention) = self.incoming_value_retention(&source) {
                        value.retention = retention;
                    }
                }
            }
        }
        let stored_value = self.context.control.operands.insert(value);
        let roots = self
            .context
            .control
            .operand_retained_roots
            .insert_many(roots);
        self.context
            .control
            .operand_retentions
            .insert(FlowOperandRetention {
                roots,
                alternatives: HandleSpan::empty(),
                leaves: HandleSpan::empty(),
                stored_value,
                stored_projection: HandleSpan::empty(),
            })
    }

    pub(super) fn capture_value_retention(
        &mut self,
        capture: &FlowOperandFact,
    ) -> Handle<FlowOperandRetention> {
        let Some(paths) = validation::borrow_contracts::borrow_carrying_owner_paths(
            self.program,
            capture.type_reference,
        ) else {
            return Handle::invalid();
        };
        if paths.is_empty() {
            return Handle::invalid();
        }
        let mut value = *capture;
        if capture.children.is_empty() {
            let Some(source) = validation::canonical_place_from_expression_in_state(
                self.program,
                self.state.symbol,
                self.statement_index,
                capture.expression,
            ) else {
                return Handle::invalid();
            };
            let statements = super::super::reference_spans::appended_span_since(
                &self.context.control.statements,
                self.context.state_statements_start,
            );
            match super::super::stored_views::capture_before_statement(
                self.program,
                self.semantic,
                &self.context.control,
                self.state,
                statements,
                self.statement_index,
                self.entry_origin_writes,
                self.entry_origin_unknown_write,
                &source,
            ) {
                Some(super::super::stored_values::StoredOperand::Value {
                    capture,
                    projection,
                    ..
                }) => {
                    let capture = *self.context.control.operands.get(capture);
                    if projection.is_empty() {
                        value = capture;
                    } else {
                        value.retention = self.projected_value_retention(capture, &projection);
                    }
                }
                None => {
                    let Some(retention) = self.incoming_value_retention(&source) else {
                        return Handle::invalid();
                    };
                    value.retention = retention;
                }
                Some(super::super::stored_values::StoredOperand::Unavailable) => {
                    return Handle::invalid();
                }
            }
        }
        let Some(roots) = super::super::operand_values::captured_lifetime_roots(
            self.program,
            &self.context.control,
            &value,
        ) else {
            return Handle::invalid();
        };
        let roots = self
            .context
            .control
            .operand_retained_roots
            .insert_many(roots);
        let stored_value = self.context.control.operands.insert(value);
        self.context
            .control
            .operand_retentions
            .insert(FlowOperandRetention {
                roots,
                alternatives: HandleSpan::empty(),
                leaves: HandleSpan::empty(),
                stored_value,
                stored_projection: HandleSpan::empty(),
            })
    }

    pub(super) fn capture_member_retention(
        &mut self,
        expression: ExpressionHandle,
        receiver: &FlowOperandFact,
    ) -> Option<Handle<FlowOperandRetention>> {
        let member = self.program.expression_table.member_node(expression)?;
        let symbol = validation::effective_member_symbol(self.program, member.receiver, member);
        if !symbol.is_valid() {
            return Some(Handle::invalid());
        }
        let mut path = Vec::new();
        if let Some(variant) = facts::payload_variant_for_field(self.program, symbol) {
            path.push(PlaceSegment::Case { variant });
        }
        path.push(PlaceSegment::Field { symbol });
        let captured = self.captured_retention_at_path(receiver, &path);
        if captured.is_valid() {
            return Some(captured);
        }
        if !receiver.children.is_empty()
            || (self
                .context
                .control
                .operand_retentions
                .is_valid(receiver.retention)
                && {
                    let profile = self
                        .context
                        .control
                        .operand_retentions
                        .get(receiver.retention);
                    !profile.leaves.is_empty()
                        || !profile.alternatives.is_empty()
                        || self.context.control.operands.is_valid(profile.stored_value)
                        || self
                            .context
                            .control
                            .operand_retained_roots
                            .span_or_empty(profile.roots)
                            .iter()
                            .any(|root| root.as_expression().is_some())
                })
        {
            // An aggregate projection is a prefix of its declared leaves,
            // not itself a leaf. Retain the completed profile and selector;
            // complete leaf queries still refuse unknown coverage.
            return Some(self.projected_value_retention(*receiver, &path));
        }
        let places = self.captured_referent_places(receiver.referents);
        let mut roots = Vec::new();
        for mut place in places {
            place.segments.extend_from_slice(&path);
            let Some(retention) = self.stored_retention_at_place(&place) else {
                return None;
            };
            let Some(retained) = self.context.control.retained_roots(retention) else {
                return Some(Handle::invalid());
            };
            for root in retained {
                if !roots.contains(root) {
                    roots.push(*root);
                }
            }
        }
        (!receiver.referents.is_empty()).then(|| self.append_retention(&roots))
    }

    /// A stored input selects descriptor contents, not the address of its
    /// slot. Explicit children and reaching stores share one capture query;
    /// an unresolved newer store cannot revive an earlier initializer.
    fn returned_input_retention(
        &mut self,
        actual: &FlowOperandFact,
        source: validation::borrow_contracts::ViewReturnInputSource,
        path: &[PlaceSegment],
    ) -> Option<Vec<PlaceRoot>> {
        let stored = source == validation::borrow_contracts::ViewReturnInputSource::Stored;
        if stored && self.is_view_type(actual.type_reference) {
            if !self
                .context
                .control
                .operand_retentions
                .is_valid(actual.retention)
            {
                return None;
            }
            let storage = self
                .context
                .control
                .operand_retentions
                .get(actual.retention)
                .stored_value;
            if !self.context.control.operands.is_valid(storage) {
                return None;
            }
            let capture = *self.context.control.operands.get(storage);
            let capture = self.captured_retention_at_path(&capture, path);
            return self
                .context
                .control
                .retained_roots(capture)
                .map(<[_]>::to_vec);
        }
        if !stored || !self.is_view_type(actual.type_reference) || !path.is_empty() {
            let capture = self.captured_retention_at_path(actual, path);
            return self
                .context
                .control
                .retained_roots(capture)
                .map(<[_]>::to_vec);
        }
        None
    }

    /// Loading an indexed descriptor retains its pointee, not its array slot.
    /// A nonconstant index keeps every possible captured element lifetime.
    pub(super) fn capture_indexed_retention(
        &mut self,
        expression: ExpressionHandle,
        collection: &FlowOperandFact,
        index: &FlowOperandFact,
        selector_writes_start: usize,
    ) -> Handle<FlowOperandRetention> {
        if !validation::value_element_has_builtin_coordinate(
            self.program,
            self.machine,
            Some(self.state),
            expression,
            collection.type_reference,
        ) {
            return Handle::invalid();
        }
        let Some(later) = self.operand_writes.get(selector_writes_start..) else {
            return Handle::invalid();
        };
        let places = self.captured_referent_places(collection.referents);
        if later.iter().any(|writes| {
            let Some(writes) = writes else {
                return true;
            };
            writes.iter().any(|write| {
                places.iter().any(|place| {
                    place.root == write.root
                        && super::super::canonical_place_segments_may_overlap(
                            self.program,
                            &place.segments,
                            &write.segments,
                        )
                })
            })
        }) {
            return Handle::invalid();
        }
        let mut paths = Vec::new();
        if let Some(index) = self.captured_coordinate(index) {
            // Preserve an aggregate element's declared lifetime leaves too.
            let path = [PlaceSegment::FixedIndex { index }];
            let captured = self.captured_retention_at_path(collection, &path);
            return if captured.is_valid() {
                captured
            } else {
                self.projected_value_retention(*collection, &path)
            };
        } else {
            let mut profile = collection;
            if self
                .context
                .control
                .operand_retentions
                .is_valid(collection.retention)
            {
                let stored = self
                    .context
                    .control
                    .operand_retentions
                    .get(collection.retention)
                    .stored_value;
                if self.context.control.operands.is_valid(stored) {
                    profile = self.context.control.operands.get(stored);
                }
            }
            let Some(leaves) = validation::borrow_contracts::borrow_carrying_owner_paths(
                self.program,
                profile.type_reference,
            ) else {
                return Handle::invalid();
            };
            for leaf in leaves {
                let path = crate::borrow::owner_place_segments(&leaf);
                if let Some(PlaceSegment::FixedIndex { index }) = path.first() {
                    let path = vec![PlaceSegment::FixedIndex { index: *index }];
                    if !paths.contains(&path) {
                        paths.push(path);
                    }
                }
            }
        }
        if paths.is_empty() {
            return Handle::invalid();
        }
        let Some(reference) = self.context.expression_result_type_at(
            self.program,
            self.machine,
            self.state,
            expression,
        ) else {
            return Handle::invalid();
        };
        let selections = paths
            .into_iter()
            .map(|path| (*collection, path))
            .collect::<Vec<_>>();
        self.capture_retention_join(reference, &selections)
    }

    pub(super) fn capture_returned_retention(
        &mut self,
        expression: ExpressionHandle,
        destination: typed_trees::types::TypeReferenceHandle,
        call: &typed_trees::expression::TableCallExpression,
        inputs: &[(ExpressionHandle, FlowOperandFact)],
        retention: &mut Option<Handle<FlowOperandRetention>>,
    ) {
        let target = validation::call_target_symbol(self.program, call);
        let Some(returned) = destination
            .is_valid()
            .then_some(destination)
            .or_else(|| {
                self.context.expression_result_type_at(
                    self.program,
                    self.machine,
                    self.state,
                    expression,
                )
            })
            .or_else(|| validation::call_target_return_type(self.program, target))
        else {
            return;
        };
        if self.is_view_type(returned) {
            *retention = Some(Handle::invalid());
        }
        let Some(sources) =
            crate::borrow::call_reference_lifetime_sources(self.program, self.state.symbol, call)
        else {
            return;
        };
        let mut outputs: Vec<(Vec<PlaceSegment>, Vec<PlaceRoot>, bool)> = Vec::new();
        for source in sources {
            // The declaration supplies the lifetime relation, but the completed
            // value belongs to its contextual carrier. Generic preparation can
            // give those carriers different member symbols. Translate through
            // their declaration selectors once, just as returned loans do;
            // raw symbol equality would discard a valid leaf after storage.
            let Some(path) = crate::borrow::structural_owner_path(self.program, &source.owner_path)
                .and_then(|path| {
                    validation::reference_result_custody::loan_place_segments(
                        self.program,
                        returned,
                        &path,
                    )
                })
            else {
                *retention = Some(Handle::invalid());
                return;
            };
            let position = outputs
                .iter()
                .position(|(candidate, _, _)| *candidate == path)
                .unwrap_or_else(|| {
                    outputs.push((path.clone(), Vec::new(), true));
                    outputs.len() - 1
                });
            let Some((_, actual)) =
                inputs.get(source.parameter_index + usize::from(call.receiver.is_valid()))
            else {
                outputs[position].2 = false;
                continue;
            };
            let source_path = crate::borrow::owner_place_segments(&source.source_path);
            let Some(roots) = self.returned_input_retention(actual, source.source, &source_path)
            else {
                outputs[position].2 = false;
                continue;
            };
            for root in roots {
                if !outputs[position].1.contains(&root) {
                    outputs[position].1.push(root);
                }
            }
        }
        let mut leaves = Vec::new();
        for (path, roots, complete) in outputs {
            let captured = if complete {
                self.append_retention(&roots)
            } else {
                Handle::invalid()
            };
            if path.is_empty() {
                *retention = Some(captured);
            } else {
                let path = self
                    .context
                    .control
                    .operand_retained_paths
                    .insert_many(path);
                leaves.push(checked_trees::FlowOperandRetainedLeaf {
                    path,
                    retention: captured,
                });
            }
        }
        if !leaves.is_empty() {
            let roots = if let Some(root) = *retention {
                if !self.context.control.operand_retentions.is_valid(root) {
                    return;
                }
                self.context.control.operand_retentions.get(root).roots
            } else {
                HandleSpan::empty()
            };
            let leaves = self
                .context
                .control
                .operand_retained_leaves
                .insert_many(leaves);
            *retention = Some(self.context.control.operand_retentions.insert(
                FlowOperandRetention {
                    roots,
                    alternatives: HandleSpan::empty(),
                    leaves,
                    stored_value: Handle::invalid(),
                    stored_projection: HandleSpan::empty(),
                },
            ));
        }
    }
}
