//! Addresses are formed from completed operands, not the final selector
//! variables. A bound can change while a later bound evaluates; each coordinate
//! therefore reads its own capture. Stored descriptors retain these addresses
//! independently of later writes to their backing contents.

use super::super::CapturedReferencePlace;
use super::super::observations::completed_copy_read_place;
use super::super::observations::descriptor_observation_namespace;
use super::super::operand_values::captured_operand_reference_places_with_formation;
use super::super::operand_values::finite_projection_paths;
use super::super::project_captured_view_place;
use super::super::reference_places::ReferenceResultSourceKind;
use super::super::reference_places::call_result_captured_sources;
use super::super::reference_places::call_result_leaf_sources;
use super::super::stored_values::StoredOperand;
use super::super::stored_views::capture_at_snapshot;
use super::super::stored_views::reference_places_at_snapshot;
use super::reference_projection::ReferenceProjection;
use crate::published::places::place_at;
use typed_trees::signature::StateParameter;
use typed_trees::types::TypeReferenceHandle;
use validation::borrow_contracts::borrow_carrying_owner_paths;
use validation::completed_storage::captured_retention_profiles_at_path;
use validation::reference_result_custody::loan_place_segments;
/// Ephemeral results of the shared immutable completed-storage query.
/// This is neither a second store inventory nor permission to load a pointer.
pub(super) enum StoredReferenceContents {
    Unknown,
    Incoming,
    /// A literal or other immutable image. It has no caller-local address, and
    /// that absence is the complete backing, not an unresolved pointer.
    Image,
    Captured {
        referents: HandleSpan<FlowOperandReferent>,
    },
}
use super::Execution;
use arena::{Handle, HandleSpan};
use checked_trees::{
    FlowCallFact, FlowCapturedReferenceLeaf as ReferenceLeaf, FlowCaseAccessFact,
    FlowConstraintKind, FlowConstraintRef, FlowOperandAddressProjection, FlowOperandCompletion,
    FlowOperandFact, FlowOperandReferent, FlowOperandRetainedSupplier, FlowReferentFormation,
    FlowReferentFormationEvidence as FormationEvidence,
};
use facts::{
    PlaceRoot, PlaceSegment, ScalarValue, payload_variant_for_field, project_captured_view_segments,
};
use typed_trees::expression::{ExpressionHandle, TableCallExpression};
use validation::{
    CanonicalPlace, canonical_place_type_reference, captured_integer_value,
    declared_place_type_raw, effective_member_symbol, has_builtin_bound_expression_meaning,
    indexed_operation_has_builtin_meaning, owned_projection_type_reference, reference_shell,
};

impl Execution<'_, '_, '_> {
    /// A returned value acquires its permission at this exact completion,
    /// before an enclosing call can consume it. Its ordinary lifetime ledger
    /// retains every reference and input lifetime when the result is saved.
    pub(super) fn retain_invocation_permissions(
        &mut self,
        invocation: Handle<FlowCallFact>,
        capture: &FlowOperandFact,
    ) {
        if !self.context.control.calls.is_valid(invocation) {
            return;
        }
        let actual = self.context.control.calls.get(invocation);
        let Some(call) = self.borrow.call_at(
            self.machine.symbol,
            self.state.symbol,
            actual.statement_index,
            actual.call_ordinal,
        ) else {
            return;
        };
        let source = self.borrow.calls.get(call);
        if actual.authored_expression != capture.expression
            || source.authored_expression != capture.expression
            || source.target_symbol != actual.target_symbol
            || actual.statement_index != capture.statement_index
        {
            return;
        }
        let loans = source
            .completed_reference_loans
            .handles()
            .filter(|loan| {
                let row = self.borrow.loans.get(*loan);
                self.borrow.is_call_result_loan(row)
                    && row.established_by_call == call
                    && row.formation_expression == capture.expression
                    && self
                        .context
                        .contexts
                        .constraint_refs
                        .span_or_empty(capture.constraints)
                        .iter()
                        .any(|reference| {
                            matches!(reference.kind,
                        FlowConstraintKind::BorrowLoan { loan: active } if active == *loan)
                        })
            })
            .collect::<Vec<_>>();
        if loans.is_empty() {
            return;
        }
        let formation_value = self.context.control.operands.append(*capture);
        for loan in loans {
            self.context.borrow_lifetimes.activations.append(
                checked_trees::FlowBorrowActivationFact {
                    source: checked_trees::FlowInvalidationSource::Statement {
                        statement_index: self.statement_index,
                    },
                    loan,
                    formation_value,
                },
            );
        }
    }
    /// Bind a prepared pointer resource to the actual completed slot read.
    /// The access certificate precedes activation: this child permission can
    /// never establish the read that supplies it. Owned copies and selections
    /// retain their earlier producers rather than receiving a new namespace.
    pub(super) fn retain_descriptor_permission(
        &mut self,
        capture: &mut FlowOperandFact,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) {
        let Some((_, state)) = self
            .borrow
            .first_state_borrow(self.machine.symbol, self.state.symbol)
        else {
            return;
        };
        let mut resources = self.borrow.state_loans(state).filter(|(_, loan)| {
            loan.statement_index == self.statement_index
                && loan.formation_expression == capture.expression
                && crate::borrow::is_descriptor_load_observation(self.borrow, loan)
        });
        let Some((handle, loan)) = resources.next() else {
            return;
        };
        if resources.next().is_some() {
            return;
        }
        if capture.descriptor_permission != checked_trees::FlowDescriptorPermission::Unavailable
            || !matches!(
                capture.completion,
                checked_trees::FlowOperandCompletion::Observation
            )
            || validation::reference_shell(self.program, capture.type_reference).is_none()
            || !self
                .context
                .control
                .case_accesses
                .is_valid(capture.case_access)
        {
            return;
        }
        let access = self.context.control.case_accesses.get(capture.case_access);
        if access.machine_symbol != self.machine.symbol
            || access.state_symbol != self.state.symbol
            || access.statement_index != self.statement_index
            || access.expression != capture.expression
            || access.evaluation != self.evaluation
            || access.places.is_empty()
        {
            return;
        }
        // Preserve produced copies and current stored suppliers. An abstract
        // incoming slot is the one operation needing a new opaque child. Its
        // receiver and selectors have already run at this storage snapshot.
        if !self.context.control.operands.is_valid(capture.backing) {
            return;
        }
        let receiver = self.context.control.operands.get(capture.backing);
        let Some(source) = place_at(
            self.program,
            self.state.symbol,
            self.statement_index,
            capture.expression,
        ) else {
            return;
        };
        let Some(parent) = place_at(
            self.program,
            self.state.symbol,
            self.statement_index,
            receiver.expression,
        ) else {
            return;
        };
        if source.root != parent.root || !source.segments.starts_with(&parent.segments) {
            return;
        }
        let projection = &source.segments[parent.segments.len()..];
        let incoming = validation::completed_storage::reference_sources_at_path(
            self.program,
            &self.context.control,
            capture.backing,
            projection,
            capture.storage_snapshot,
        );
        fn complete_incoming(
            program: &dyn validation::CheckingInput,
            state: &typed_trees::state::State,
            source: &validation::completed_storage::StoredOperand,
        ) -> bool {
            use validation::completed_storage::StoredOperand;
            match source {
                StoredOperand::Incoming(place) => place.root.as_symbol().is_some_and(|symbol| {
                    program.state_parameter_by_symbol(state, symbol).is_some()
                }),
                StoredOperand::Alternatives(alternatives) => {
                    !alternatives.is_empty()
                        && alternatives
                            .iter()
                            .all(|source| complete_incoming(program, state, source))
                }
                StoredOperand::Value { .. } | StoredOperand::Unavailable => false,
            }
        }
        if !complete_incoming(self.program, self.state, &incoming) {
            return;
        }
        if !self
            .context
            .control
            .operand_retentions
            .is_valid(capture.retention)
        {
            return;
        }
        let observations = self
            .context
            .control
            .statements
            .get(self.statement_fact)
            .value_observations;
        let mut reads = observations.handles().filter(|handle| {
            let read = self.context.control.value_observations.get(*handle);
            read.expression == capture.expression
                && read.evaluation == self.evaluation
                && read.receiver == capture.backing
                && read.constraints == access.constraints
                && !read.places.is_empty()
        });
        let Some(read) = reads.next() else {
            return;
        };
        if reads.next().is_some() {
            return;
        }
        // A prepared row is not an entry hypothesis. Only the real completed
        // value carries its lifetime; an unavailable source keeps refusing.
        capture.descriptor_permission =
            checked_trees::FlowDescriptorPermission::Loaded { loan: handle, read };
        capture.referents = self.append_referent_places(&[validation::CanonicalPlace {
            root: loan.root,
            segments: Vec::new(),
        }]);
        let formation_value = self.context.control.operands.append(*capture);
        self.context
            .borrow_lifetimes
            .activations
            .append(checked_trees::FlowBorrowActivationFact {
                source: checked_trees::FlowInvalidationSource::Statement {
                    statement_index: self.statement_index,
                },
                loan: handle,
                formation_value,
            });
        crate::flow::reference_spans::append_constraint_ref(
            &mut self.context.contexts.constraint_refs,
            constraints,
            checked_trees::FlowConstraintKind::BorrowLoan { loan: handle },
        );
        capture.constraints = *constraints;
    }

    /// Observe the stored descriptors carried by one completed owned value.
    /// This is the same inventory used by selected payload copies; physical
    /// addresses are supplied by the evaluator, not recovered from an initializer.
    pub(super) fn capture_owned_reference_leaves(
        &mut self,
        owner: Handle<FlowOperandFact>,
        places: &[CapturedReferencePlace],
    ) -> HandleSpan<ReferenceLeaf> {
        let capture = *self.context.control.operands.get(owner);
        let Some(paths) = borrow_carrying_owner_paths(self.program, capture.type_reference) else {
            return HandleSpan::empty();
        };
        let mut leaves = Vec::new();
        for path in paths {
            let path = crate::borrow::owner_place_segments(&path);
            let Some(reference) =
                owned_projection_type_reference(self.program, capture.type_reference, &path)
            else {
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
                    let stored = capture_at_snapshot(
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
                    let profiles = captured_retention_profiles_at_path(
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
            let mut saw_image = false;
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
                                FlowReferentFormation {
                                    source,
                                    evidence: FormationEvidence::Access(capture.case_access),
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
                        StoredReferenceContents::Image => saw_image = true,
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
                Handle::invalid()
            };
            if self.context.control.operand_retentions.is_valid(retention) {
                let suppliers = suppliers
                    .map(|rows| {
                        let rows = rows
                            .into_iter()
                            .flatten()
                            .map(|selected| FlowOperandRetainedSupplier {
                                contract: None,
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
                profile.immutable_image =
                    immutable_image || (saw_image && origins.is_empty() && known);
            }
            let path = self.context.control.operand_child_paths.insert_many(path);
            leaves.push(ReferenceLeaf {
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
        destination: TypeReferenceHandle,
        call: &TableCallExpression,
        inputs: &[(ExpressionHandle, FlowOperandFact)],
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> Option<HandleSpan<ReferenceLeaf>> {
        let returned = self.constructed_value_type(expression, destination)?;
        if self.is_view_type(returned) {
            return None;
        }
        let Some(paths) = borrow_carrying_owner_paths(self.program, returned) else {
            return Some(HandleSpan::empty());
        };
        if paths.is_empty() {
            return None;
        }
        let entered_target = validation::call_target_entry_symbol(
            self.program,
            validation::call_target_symbol(self.program, call),
        );
        let Some(parameters) = validation::find_state(self.program, entered_target)
            .map(|(_, state)| self.program.state_parameters(state))
            .or_else(|| {
                validation::selected_requirement_signature(self.program, entered_target)
                    .map(|(_, signature)| self.program.state_signature_parameters(signature))
            })
        else {
            return Some(HandleSpan::empty());
        };
        let Some(indices) = self.captured_argument_indices(
            parameters,
            inputs
                .iter()
                .skip(usize::from(call.receiver.is_valid()))
                .map(|(_, value)| value),
        ) else {
            return Some(HandleSpan::empty());
        };
        let Some(sources) =
            call_result_captured_sources(self.program, call, self.context.call_frames, &indices)
        else {
            return Some(HandleSpan::empty());
        };
        let mut leaves = Vec::new();
        for path in &paths {
            let path = crate::borrow::owner_place_segments(path);
            let Some(reference) = owned_projection_type_reference(self.program, returned, &path)
            else {
                return Some(HandleSpan::empty());
            };
            if !self.is_view_type(reference) {
                return Some(HandleSpan::empty());
            }
            let mut places = Vec::new();
            let mut complete = true;
            let mut matched = false;
            for source in &sources {
                let Some(output) = loan_place_segments(self.program, returned, &source.result_path)
                else {
                    return Some(HandleSpan::empty());
                };
                let suffix = match source.kind {
                    ReferenceResultSourceKind::Reference => {
                        if output != path {
                            continue;
                        }
                        Vec::new()
                    }
                    ReferenceResultSourceKind::Value { type_reference } => {
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
                        let projected = loan_place_segments(self.program, type_reference, &suffix)
                            .and_then(|path| {
                                owned_projection_type_reference(self.program, type_reference, &path)
                            });
                        if projected.is_none_or(|reference| !self.is_view_type(reference)) {
                            return Some(HandleSpan::empty());
                        }
                        suffix
                    }
                };
                matched = true;
                match source.backing {
                    super::super::reference_places::ReferenceResultBacking::CallerArgument {
                        argument_index,
                        ..
                    } => {
                        let Some(actual) = inputs
                            .get(argument_index + usize::from(call.receiver.is_valid()))
                            .map(|(_, actual)| actual)
                        else {
                            complete = false;
                            break;
                        };
                        let selected = crate::borrow::structural_owner_path(
                            self.program,
                            &crate::borrow::owner_path_from_place_segments(
                                self.program,
                                &source.segments,
                            ),
                        )
                        .and_then(|mut path| {
                            path.extend_from_slice(&suffix);
                            loan_place_segments(self.program, actual.type_reference, &path)
                        });
                        let Some(selected) = selected else {
                            complete = false;
                            break;
                        };
                        let Some(projected) = self.capture_reference_projection(
                            actual,
                            &selected,
                            ReferenceProjection::Value,
                        ) else {
                            complete = false;
                            break;
                        };
                        if projected.places.is_empty() {
                            complete = false;
                            break;
                        }
                        for place in projected.places {
                            crate::shared::collections::push_unique(&mut places, place);
                        }
                    }
                    super::super::reference_places::ReferenceResultBacking::InvocationResult(
                        home,
                    ) => {
                        // The completed invocation owns this referent: its
                        // identity is the call site itself, not a formal's
                        // storage. The leaf path replays beneath the opaque
                        // referent it names.
                        let selected = crate::borrow::structural_owner_path(
                            self.program,
                            &crate::borrow::owner_path_from_place_segments(
                                self.program,
                                &source.segments,
                            ),
                        )
                        .and_then(|mut path| {
                            path.extend_from_slice(&suffix);
                            loan_place_segments(self.program, home.referent, &path)
                        });
                        let Some(segments) = selected else {
                            complete = false;
                            break;
                        };
                        crate::shared::collections::push_unique(
                            &mut places,
                            CapturedReferencePlace::unformed(CanonicalPlace {
                                root: PlaceRoot::Expression(expression),
                                segments,
                            }),
                        );
                    }
                }
            }
            let referents = if complete && matched {
                self.append_captured_references(&places)
            } else {
                HandleSpan::empty()
            };
            let path = self.context.control.operand_child_paths.insert_many(path);
            leaves.push(ReferenceLeaf {
                path,
                type_reference: reference,
                referents,
                retention: Handle::invalid(),
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
        references: HandleSpan<ReferenceLeaf>,
    ) -> HandleSpan<ReferenceLeaf> {
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
        constraints: HandleSpan<FlowConstraintRef>,
        case_access: &mut Handle<FlowCaseAccessFact>,
        possible_storage: &mut Option<HandleSpan<FlowOperandReferent>>,
        address_projection: &mut Option<FlowOperandAddressProjection>,
    ) -> HandleSpan<FlowOperandReferent> {
        let is_view = expression != self.reference_slot_target
            && self
                .context
                .expression_result_type_at(self.program, self.machine, self.state, expression)
                .is_some_and(|reference| self.is_view_type(reference));
        let Some(member) = self.program.expression_table.member_node(expression) else {
            return HandleSpan::empty();
        };
        let symbol = effective_member_symbol(self.program, member.receiver, member);
        if !symbol.is_valid() {
            return HandleSpan::empty();
        }
        let mut path = Vec::new();
        if let Some(variant) = payload_variant_for_field(self.program, symbol) {
            path.push(PlaceSegment::Case { variant });
        }
        path.push(PlaceSegment::Field { symbol });
        // The field is relative to the evaluated receiver, even when a returned
        // view has no absolute address in the source place namespace.
        *address_projection = Some(FlowOperandAddressProjection::Place(
            self.context
                .control
                .operand_child_paths
                .insert_many(path.iter().copied()),
        ));
        let carrier = reference_shell(self.program, receiver.type_reference)
            .map_or(receiver.type_reference, |shell| shell.referee);
        if !is_view && owned_projection_type_reference(self.program, carrier, &path).is_some() {
            *possible_storage = self.project_possible_storage(receiver, &path);
        }

        // A recast's fields describe its new interpretation. Appending them
        // to the original storage path could make an overlapping write look
        // disjoint. Keep the relative projection above, but let the retained
        // enclosure bound the write when no translated address is available.
        let places = if !self.backing_allows_projection(receiver) {
            Some(Vec::new())
        } else {
            self.captured_reference_places(receiver.referents)
                .into_iter()
                .map(|place| place.projected(&path))
                .collect::<Option<Vec<_>>>()
        };
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
        let case_places = descriptor_observation_namespace(
            self.program,
            &self.context.control,
            receiver,
            &contexts,
        )
        .and_then(|namespace| project_captured_view_place(namespace, &path))
        .map(|namespace| self.append_referent_places(&[namespace]))
        .unwrap_or(source_places);
        *case_access = self.record_case_access_places(expression, constraints, case_places);
        if is_view {
            if let Some(captured) = self.captured_projection_referents(receiver, &path) {
                return captured;
            }
            let value = self.context.control.operands.insert(*receiver);
            if let Some(selected) = reference_places_at_snapshot(
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
            || (matches!(receiver.completion, FlowOperandCompletion::Observation)
                && receiver.reference_leaves.is_empty())
        {
            return None;
        }
        let places = captured_operand_reference_places_with_formation(
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
                    StoredReferenceContents::Image => {}
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
        call: &TableCallExpression,
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
        let entered_target = validation::call_target_entry_symbol(
            self.program,
            validation::call_target_symbol(self.program, call),
        );
        let Some(parameters) = validation::find_state(self.program, entered_target)
            .map(|(_, state)| self.program.state_parameters(state))
            .or_else(|| {
                validation::selected_requirement_signature(self.program, entered_target)
                    .map(|(_, signature)| self.program.state_signature_parameters(signature))
            })
        else {
            return;
        };
        let Some(indices) = self.captured_argument_indices(
            parameters,
            inputs
                .iter()
                .skip(usize::from(call.receiver.is_valid()))
                .map(|(_, value)| value),
        ) else {
            return;
        };
        let Some(sources) =
            call_result_leaf_sources(self.program, call, self.context.call_frames, &indices)
        else {
            return;
        };
        let mut origins = Vec::new();
        for source in sources {
            if !source.result_path.is_empty() {
                return;
            }
            match source.backing {
                super::super::reference_places::ReferenceResultBacking::CallerArgument {
                    argument_index,
                    ..
                } => {
                    let Some((_, actual)) =
                        inputs.get(argument_index + usize::from(call.receiver.is_valid()))
                    else {
                        return;
                    };
                    let Some(projected) = self.capture_reference_projection(
                        actual,
                        &source.segments,
                        ReferenceProjection::Value,
                    ) else {
                        return;
                    };
                    let places = projected.places;
                    for place in places {
                        crate::shared::collections::push_unique(&mut origins, place);
                    }
                }
                super::super::reference_places::ReferenceResultBacking::InvocationResult(_) => {
                    // An opaque result referent names its producing call
                    // site; the leaf path already replays beneath its declared
                    // referent, so no argument operand is substituted.
                    crate::shared::collections::push_unique(
                        &mut origins,
                        CapturedReferencePlace::unformed(CanonicalPlace {
                            root: PlaceRoot::Expression(expression),
                            segments: source.segments,
                        }),
                    );
                }
            }
        }
        *referents = Some(self.append_captured_references(&origins));
    }

    /// Normal return completes the descriptor formation established in the
    /// checked callee. The pre-call capture supplies possible backing, not a
    /// requirement that the caller select every mutually exclusive origin.
    pub(super) fn complete_call_reference_formations(
        &mut self,
        call: Handle<FlowCallFact>,
        referents: &mut Option<HandleSpan<FlowOperandReferent>>,
        leaves: &mut Option<HandleSpan<ReferenceLeaf>>,
    ) {
        if !self.context.control.calls.is_valid(call) {
            return;
        }
        let mut returned_origins = Vec::new();
        if let Some(referents) = referents {
            *referents = self.returned_descriptor_formations(call, *referents);
            returned_origins.extend_from_slice(
                self.context
                    .control
                    .operand_referents
                    .span_or_empty(*referents),
            );
        }
        if let Some(leaves) = leaves {
            let mut returned = self
                .context
                .control
                .captured_reference_leaves
                .span_or_empty(*leaves)
                .to_vec();
            for leaf in &mut returned {
                leaf.referents = self.returned_descriptor_formations(call, leaf.referents);
                returned_origins.extend_from_slice(
                    self.context
                        .control
                        .operand_referents
                        .span_or_empty(leaf.referents),
                );
            }
            *leaves = self
                .context
                .control
                .captured_reference_leaves
                .insert_many(returned);
        }
        let returned_origins = self
            .context
            .control
            .operand_referents
            .insert_many(returned_origins);
        self.context
            .control
            .calls
            .get_mut(call)
            .normal_return_referents = returned_origins;
    }

    fn returned_descriptor_formations(
        &mut self,
        call: Handle<FlowCallFact>,
        referents: HandleSpan<FlowOperandReferent>,
    ) -> HandleSpan<FlowOperandReferent> {
        let mut returned = Vec::new();
        for source in referents.handles() {
            let mut referent = *self.context.control.operand_referents.get(source);
            referent.formation =
                self.context
                    .control
                    .referent_formations
                    .append(FlowReferentFormation {
                        source,
                        evidence: FormationEvidence::Return(call),
                        root: referent.root,
                        segments: referent.segments,
                        ..Default::default()
                    });
            returned.push(referent);
        }
        self.context.control.operand_referents.insert_many(returned)
    }

    /// Every covered arm must supply its completed descriptor. Keep formation
    /// identity with the address; equal paths can have different load obligations.
    pub(super) fn capture_alternative_referents(
        &mut self,
        alternatives: HandleSpan<checked_trees::FlowOperandAlternative>,
    ) -> HandleSpan<FlowOperandReferent> {
        let Some(alternatives) = self.context.control.operand_alternatives.span(alternatives)
        else {
            return HandleSpan::empty();
        };
        if alternatives.is_empty() {
            return HandleSpan::empty();
        }
        let mut places = Vec::new();
        for alternative in alternatives {
            if !alternative.source_arm.is_valid()
                || !self.context.control.operands.is_valid(alternative.value)
            {
                return HandleSpan::empty();
            }
            let value = self.context.control.operands.get(alternative.value);
            let captured = self.captured_reference_places(value.referents);
            if captured.is_empty() {
                return HandleSpan::empty();
            }
            places.extend(captured);
        }
        self.append_captured_references(&places)
    }

    /// Read the completed value installed by a reaching store. A matching
    /// unresolved capture or an intervening replacement is authoritative:
    /// neither may fall back to the declaration's initializer.
    pub(super) fn stored_view_referents(
        &mut self,
        expression: ExpressionHandle,
    ) -> Option<HandleSpan<FlowOperandReferent>> {
        let source = place_at(
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
                StoredReferenceContents::Image => {}
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
            let reference = canonical_place_type_reference(
                self.program,
                self.state.symbol,
                self.statement_index,
                &root,
            )?;
            let paths = finite_projection_paths(self.program, reference, &source.segments)?;
            let mut alternatives = Vec::new();
            for segments in paths {
                alternatives.extend(self.stored_reference_contents_at_place(&CanonicalPlace {
                    root: source.root,
                    segments,
                })?);
            }
            return (!alternatives.is_empty()).then_some(alternatives);
        }
        let Some(stored) = capture_at_snapshot(
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
                StoredOperand::Unavailable => {
                    return Some(vec![StoredReferenceContents::Unknown]);
                }
                StoredOperand::Incoming(place) => {
                    if place != *source || !self.is_incoming_descriptor_place(&place) {
                        return Some(vec![StoredReferenceContents::Unknown]);
                    }
                    alternatives.push(StoredReferenceContents::Incoming);
                }
                StoredOperand::Alternatives(values) => {
                    pending.extend(values.into_iter().rev());
                }
                StoredOperand::Value {
                    capture,
                    projection,
                    ..
                } => {
                    let places = reference_places_at_snapshot(
                        self.program,
                        &self.context.control,
                        capture,
                        &projection,
                        self.storage_snapshot,
                        |place| self.is_incoming_descriptor_place(place),
                    )
                    .unwrap_or_default();
                    if places.is_empty()
                        && projection_is_immutable_image(
                            self.program,
                            &self.context.control,
                            capture,
                            &projection,
                        )
                    {
                        alternatives.push(StoredReferenceContents::Image);
                    } else {
                        let referents = self.append_captured_references(&places);
                        alternatives.push(StoredReferenceContents::Captured { referents });
                    }
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
        possible_storage: &mut Option<HandleSpan<FlowOperandReferent>>,
        address_projection: &mut Option<FlowOperandAddressProjection>,
    ) -> HandleSpan<FlowOperandReferent> {
        if !indexed_operation_has_builtin_meaning(
            self.program,
            self.machine,
            Some(self.state),
            expression,
            Some(collection.type_reference),
        ) || [Some(start), end].into_iter().flatten().any(|bound| {
            bound.expression.is_valid()
                && !has_builtin_bound_expression_meaning(
                    self.program,
                    self.machine,
                    Some(self.state),
                    bound.expression,
                )
        }) {
            return HandleSpan::empty();
        }
        // A selector is an independently completed value even when builtin
        // indexing has no selected operator row to retain its actuals. Keep
        // that exact epoch for later storage identity and custody judgments.
        for operand in [Some(start), end].into_iter().flatten() {
            if operand.expression.is_valid() {
                self.context.control.operands.insert(*operand);
            }
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
                *address_projection = Some(FlowOperandAddressProjection::View(HandleSpan::empty()));
                *possible_storage = self.project_possible_storage(collection, &[]);
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
        let segments = self
            .context
            .control
            .operand_child_paths
            .insert_many([geometry]);
        *address_projection = Some(if end.is_some() {
            FlowOperandAddressProjection::View(segments)
        } else {
            FlowOperandAddressProjection::Place(segments)
        });
        // Element selection may read a stored reference descriptor. A range
        // instead forms a view of the collection and keeps its window address.
        let reads_descriptor = end.is_none()
            && expression != self.reference_slot_target
            && self
                .context
                .expression_result_type_at(self.program, self.machine, self.state, expression)
                .is_some_and(|reference| self.is_view_type(reference));
        // A descriptor slot's geometry is not the stored pointer's pointee.
        // Owned element selection and range formation retain the collection's
        // aligned may-write footprint independently of exact referents below.
        if !reads_descriptor {
            *possible_storage = self.project_possible_storage(collection, &[geometry]);
        }
        if reads_descriptor
            && let Some(referents) = self.captured_projection_referents(collection, &[geometry])
        {
            return referents;
        }
        for place in &mut places {
            let Some(segments) = project_captured_view_segments(&place.place.segments, &[geometry])
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
        parameters: &[StateParameter],
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
                    if reference_shell(self.program, parameter.type_reference).is_none()
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
        captured_integer_value(
            self.program,
            self.machine,
            self.state,
            operand.expression,
            |expression, place| {
                // Authored literals belong to the arithmetic engine. A
                // typed completed expression can also carry an observation
                // (for example its call result); its own captured constraints
                // supply that value, never a replay of its producer.
                if !matches!(place.root, PlaceRoot::Symbol(_))
                    && place
                        .root
                        .as_expression()
                        .and_then(|expression| {
                            declared_place_type_raw(
                                self.program,
                                self.machine,
                                Some(self.state),
                                expression,
                            )
                        })
                        .is_none()
                {
                    return None;
                }
                let place = completed_copy_read_place(
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
        if !self.backing_allows_projection(operand) {
            Vec::new()
        } else {
            self.captured_reference_places(operand.referents)
        }
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
        constraints: HandleSpan<FlowConstraintRef>,
        referents: HandleSpan<FlowOperandReferent>,
        source_access: arena::Handle<checked_trees::FlowCaseAccessFact>,
    ) -> HandleSpan<FlowOperandReferent> {
        // The descriptor's actual load already owns its carrier-case
        // obligation. A returned slice can retain its complete input lifetime
        // while its exact pointee window has no canonical place. Do not create
        // another empty case obligation about that unavailable geometry. The
        // original nonempty source access remains independently checked, and
        // the empty referent roster still supplies no address or pointee fact.
        // Reuse only the token produced by this exact evaluation, never an
        // earlier read of a same-spelled descriptor or its old case epoch.
        if referents.is_empty() && self.context.control.case_accesses.is_valid(source_access) {
            let access = self.context.control.case_accesses.get(source_access);
            if access.machine_symbol == self.machine.symbol
                && access.state_symbol == self.state.symbol
                && access.statement_index == self.statement_index
                && access.expression == expression
                && access.evaluation == self.evaluation
                && access.constraints == constraints
                && !access.places.is_empty()
            {
                return referents;
            }
        }
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
            reference.formation =
                self.context
                    .control
                    .referent_formations
                    .append(FlowReferentFormation {
                        source,
                        evidence: FormationEvidence::Access(access),
                        owner: Handle::invalid(),
                        path: HandleSpan::empty(),
                        root: reference.place.root,
                        segments,
                    });
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
                formation: Handle::invalid(),
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

/// An immutable image projected through a completed constructor has no local
/// address. Empty referents are that image, not a missing supplier.
fn projection_is_immutable_image(
    program: &dyn validation::CheckingInput,
    control: &checked_trees::FlowControlFacts,
    capture: Handle<checked_trees::FlowOperandFact>,
    projection: &[PlaceSegment],
) -> bool {
    if !control.operands.is_valid(capture) {
        return false;
    }
    let operand = control.operands.get(capture);
    if projection.is_empty() {
        return operand.referents.is_empty()
            && control.operand_retentions.is_valid(operand.retention)
            && control
                .operand_retentions
                .get(operand.retention)
                .immutable_image;
    }
    let Some((child, _, consumed)) = validation::completed_storage::captured_operand_child_at_path(
        program,
        control,
        operand,
        operand.type_reference,
        projection,
    ) else {
        return false;
    };
    projection_is_immutable_image(program, control, child, &projection[consumed..])
}
