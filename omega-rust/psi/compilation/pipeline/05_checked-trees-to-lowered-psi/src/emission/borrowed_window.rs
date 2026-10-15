//! Detached structural storage: `MoveStructuralField` removes a child from
//! owned or exclusive borrowed storage; `StoreStructuralField` reseats the
//! same exact place. Detached bytes and claims belong to the new value, not
//! to an alias that a later store can overwrite. Owned partial roots retain
//! their checked residual custody; only borrowed roots owe restoration.
//! An intact freely discardable value needs no extraction. Its completed
//! replacement uses the same store with ReplaceDiscardable disposition and
//! write-only authority; receiving checks still enforce exact type, resource
//! freedom and validity. A reference descriptor owns its pointer, not the
//! selected backing, so this distinction follows every Referent crossing.
//!
//! The source checker (`typed-trees-to-checked-trees`
//! `checks/multiplicity/borrowed_windows.rs`) admits a consuming move through
//! an exclusive chain only when a same-typed store reseats the exact place
//! before every non-crash exit. This emitter spells that window in Terminal:
//! the moved place becomes a `MoveStructuralField` whose structural result
//! carries the removed value, and the restoring assignment becomes a
//! `StoreStructuralField` naming the same root, carrier path and field. The
//! caller's storage is never replaced by a staged copy: the hole stays in the
//! borrowed root and the repair writes back into it.
//!
//! Checked stores name their opening extraction's result coordinate. The
//! emitter resolves that coordinate to the stable extracted-place identity;
//! it does not compare selector operand families to rediscover storage
//! equality. A value read and a pre-RHS assignment capture can select the
//! same element through different operands. The source checker owns that
//! spatial judgment; the ledger owns sequencing and the emitted operation
//! pair. Detached value custody remains independent of the debt reference.
//! A pre-RHS capture can spell that storage through a private borrowed block
//! parameter. Its shortened physical path and root type drive emitted access;
//! the original evaluated path still selects memberships and identifies debt.
//! Neither coordinate can substitute for the other or replay its selectors.
//!
//! The ledger fails closed. Every shape the checked facts do not pin exactly
//! — a root without owned or exclusive mutable custody, a whole-root route
//! beneath anything but an exclusive parameter loan, an unsupported projection,
//! a scalar or ambiguous field, a moved type that
//! differs from the declared field, extraction over an open hole, a repair at
//! another place, a repair value that is not a whole owned value of the hole's
//! type, or an open borrowed hole at a sequence exit — is a
//! `LoweringError::UnpinnedBorrowedStorageWindow` naming
//! the place.
//!
//! A returned record reference retains its carrier place and an explicit
//! `Referent` crossing in both the emitted path and ledger key. Geometry walks
//! the referent; custody never changes to ownership of the referent. The same
//! declaration walk serves scalar stores and structural restoration windows.
//!
//! An exclusive `&mut [T]` parameter owns none of its elements and has no
//! static extent. A window on one of its elements keeps the view's own loan as
//! root and selects the element with a `RuntimeIndex` (a literal selector
//! included), owed against an `ElementViewLength` observation emitted directly
//! before each move and store, as element-view scalar stores do. The ledger
//! pairs the hole by its opening extraction, not by fresh selector ids.
//!
//! A whole-root route under an exclusive `&mut` parameter loan is a root
//! window: `MoveStructuralField`/`StoreStructuralField` with no field and an
//! empty path, the whole-value assignment `referent = value` through the
//! loan. Its hole is the whole referent, so it overlaps every other place
//! beneath that root.
//!
//! Caller: `unit/attached_unit/operation_frame.rs`, the operation emitter
//! both the ordinary single-state machine (a local move-out/restore) and the
//! composed state graph (a structural call result replacing a borrowed field)
//! use. Each route drives one ledger per straight-line operation sequence and
//! requires it closed before that sequence's exits.

use super::{
    CheckedUnitStructuralPathSegment, LoweringError, OperationKind, OperationResult, PlaceId,
    StructuralAccess, StructuralFieldId, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeId, allocate_dense, place_id,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::runtime_elements::{self, ProjectionStep};
use crate::emission::structural_scalar_store::{WindowLeaf, lower_window_path};
use crate::lowering_error::unsupported;
use crate::terminal_identities::lookup_type_id;
use checked_trees::{
    CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralResultBindingPlan,
};
use language_semantics::Multiplicity;
use terminal_psi::{StructuralArgument, StructuralOperationResult};

/// One open restoration debt: the exact hole beneath an owned or exclusive
/// storage root. The removed value retains separate ordinary result custody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpenBorrowedWindow {
    /// Stable identity of the extraction, independent of the detached value's
    /// subsequent transfer or disposal and of fresh selector evaluation ids.
    opening: PlaceId,
    pub(crate) source: PlaceId,
    pub(crate) path: Vec<StructuralPathSegment>,
    /// A field extending `path`, or `None` when `path` names the whole hole.
    pub(crate) field: Option<StructuralFieldId>,
    pub(crate) hole_type: StructuralTypeId,
    /// The checked route is retained for overlap diagnostics, not for pairing
    /// a restoration: its selectors name evaluation roles, not storage ids.
    checked_path: Vec<CheckedUnitStructuralPathSegment>,
    spelling: String,
    restoration_required: bool,
}

/// The restoring value: an already-owned whole structural place of the hole's
/// exact declared type. The removed value itself or any other whole owned
/// value of that type qualifies; both values' obligations stay with their
/// own custody rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BorrowedWindowRepairValue {
    pub(crate) place: PlaceId,
    pub(crate) structural_type: StructuralTypeId,
}

/// Open restoration debts on the current emission path, in opening order.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct BorrowedWindowLedger {
    open: Vec<OpenBorrowedWindow>,
    assignment: Option<CapturedAssignmentDestination>,
}

/// The current assignment's operands, evaluated before its RHS. The source
/// checker pins the storage route against overlapping mutation; lowering
/// additionally retains its home and selector values instead of replaying
/// those reads when old-value cleanup or the final store executes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CapturedAssignmentDestination {
    statement_index: u32,
    destination_expression: typed_trees::expression::ExpressionHandle,
    destination: CheckedUnitStructuralArgumentPlan,
    source: PlaceId,
    root_type: StructuralTypeId,
    path: Vec<StructuralPathSegment>,
    original_source: PlaceId,
    original_path: Vec<StructuralPathSegment>,
}

/// Physical emission and checked storage identity share evaluated selectors,
/// but a captured descriptor prefix gives them different roots and paths.
struct EvaluatedStoragePlace {
    source: PlaceId,
    root_type: StructuralTypeId,
    path: Vec<StructuralPathSegment>,
    field: Option<StructuralFieldId>,
    original_source: PlaceId,
    original_path: Vec<StructuralPathSegment>,
}

/// An indexed store consumes the descriptor and coordinate retained by the
/// target's capture, rather than reconstructing either after the RHS.
pub(crate) struct CapturedIndexedDestination {
    pub(crate) source: PlaceId,
    pub(crate) path: Vec<StructuralPathSegment>,
    pub(crate) coordinate: semantic_vocabulary::ValueId,
}

/// The resolved exact place a move or repair names beneath a window root:
/// the root's declared type, the carrier's projection steps (a runtime element
/// evaluates when the operation emits), the checked route for diagnostics,
/// the leaf's field declaration id (`None` for a root window), and the hole's
/// declared type.
struct ResolvedStoragePlace {
    source: PlaceId,
    root_type: StructuralTypeId,
    root_access: StructuralAccess,
    root_multiplicity: StructuralMultiplicity,
    steps: Vec<ProjectionStep>,
    checked_path: Vec<CheckedUnitStructuralPathSegment>,
    field: Option<terminal_psi::StructuralFieldDeclaration>,
    hole_type: StructuralTypeId,
    spelling: String,
    restoration_required: bool,
    qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
    projected_qualifications: Vec<terminal_psi::StructuralPathQualification>,
}

impl ResolvedStoragePlace {
    /// Opaque predicates on an ancestor cannot be preserved by replacing only
    /// a descendant. Exact hole memberships move with the hole, while sibling
    /// memberships are untouched. Geometry alone never establishes a domain.
    fn selected_memberships(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        carrier_path: &[StructuralPathSegment],
    ) -> Result<
        (
            Vec<semantic_vocabulary::StructuralDomainId>,
            Vec<terminal_psi::StructuralPathQualification>,
        ),
        LoweringError,
    > {
        let mut path = carrier_path.to_vec();
        if self.field.is_some() {
            let Some(CheckedUnitStructuralPathSegment::Field(identity)) = self.checked_path.last()
            else {
                return Err(unpinned(
                    &self.spelling,
                    "the qualified hole lost its field identity",
                ));
            };
            path.push(StructuralPathSegment::Field(
                identities.intern(&identity.spelling()),
            ));
        }
        terminal_psi::StructuralPathQualification::select_restorable(
            &self.qualifications,
            &self.projected_qualifications,
            &path,
        )
        .ok_or_else(|| {
            unpinned(
                &self.spelling,
                "field replacement cannot restore an ancestor's qualification",
            )
        })
    }
}

impl BorrowedWindowLedger {
    pub(crate) fn capture_assignment_destination(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        destination: &CheckedUnitStructuralArgumentPlan,
        destination_expression: typed_trees::expression::ExpressionHandle,
        parameters: &[StructuralParameterDeclaration],
        structural_types: &[StructuralTypeDeclaration],
        type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
        evaluation: &mut WindowEvaluation<'_, '_>,
        values: &mut Vec<terminal_psi::ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        next_place: &mut u64,
        places: &mut Vec<terminal_psi::StructuralPlaceDeclaration>,
        operations: &mut OperationBuffer,
    ) -> Result<(), LoweringError> {
        let place = resolve_storage_place(
            identities,
            destination,
            StructuralAccess::WriteOnlyBorrow,
            parameters,
            evaluation.evaluation,
            operations,
            structural_types,
            type_ids,
        )?;
        let indexes = runtime_elements::evaluate_selectors(
            identities,
            &place.steps,
            evaluation.checked,
            evaluation.machine,
            evaluation.state,
            evaluation.statement_index,
            evaluation.evaluation,
            evaluation.source_value_count,
            values,
            next_value,
            next_block,
            next_edge,
            operations,
            evaluation.calls,
        )?;
        let mut path =
            runtime_elements::complete_path(place.steps, indexes, evaluation.calls, operations)?;
        if place.field.is_some() {
            let Some(CheckedUnitStructuralPathSegment::Field(identity)) = destination.path.last()
            else {
                return unsupported("captured field destination lost its declaration identity");
            };
            path.push(StructuralPathSegment::Field(
                identities.intern(&identity.spelling()),
            ));
        }
        // Retain authority already held at this endpoint. In particular a
        // write-only source can never acquire read or disposal authority,
        // while readable storage must retain the authority checked cleanup
        // needs after its RHS has completed.
        let access = if terminal_semantics::reference_source_projection_accessible(
            structural_types.iter(),
            place.root_type,
            place.root_multiplicity,
            place.root_access,
            &path,
            StructuralAccess::MutableBorrow,
        ) {
            StructuralAccess::MutableBorrow
        } else {
            StructuralAccess::WriteOnlyBorrow
        };
        // Inline fields and fixed-array coordinates already name stable
        // storage under their original PlaceId. Only descriptor crossings can
        // redirect that path after the RHS; those require a physical endpoint.
        let capture_boundary = path
            .iter()
            .enumerate()
            .filter_map(|(position, segment)| {
                if *segment == StructuralPathSegment::Referent {
                    return Some(position + 1);
                }
                (matches!(
                    segment,
                    StructuralPathSegment::FixedIndex(_)
                        | StructuralPathSegment::RuntimeIndex { .. }
                ) && terminal_semantics::runtime_structural_path_tip(
                    structural_types.iter(),
                    place.root_type,
                    &path[..position],
                )
                .is_some_and(|declaration| {
                    matches!(
                        declaration.shape,
                        terminal_psi::StructuralTypeShape::ElementView { .. }
                            | terminal_psi::StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                                    | terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                            )
                    )
                }))
                .then_some(position)
            })
            .next_back();
        let original_path = path.clone();
        let (source, root_type, path) = if let Some(capture_boundary) = capture_boundary {
            // Freeze every descriptor-dependent part of the address. The
            // suffix consists only of inline offsets and already evaluated
            // coordinates; retaining it preserves real field declarations and
            // needs neither another descriptor load nor selector evaluation.
            // A reference captures its loaded address. A view parameter
            // captures its descriptor words before selecting the element.
            let retained_path = path.split_off(capture_boundary);
            let capture_type = terminal_semantics::runtime_structural_path_tip(
                structural_types.iter(),
                place.root_type,
                &path,
            )
            .ok_or(LoweringError::Unsupported(
                "captured destination lost its backing declaration",
            ))?
            .id;
            let (qualifications, projected_qualifications) =
                terminal_psi::StructuralPathQualification::select(
                    &place.qualifications,
                    &place.projected_qualifications,
                    &path,
                );
            let source = super::assignment_destination::capture(
                place.source,
                path,
                capture_type,
                access,
                qualifications,
                projected_qualifications,
                evaluation.evaluation,
                values,
                next_value,
                next_block,
                next_edge,
                next_place,
                places,
                operations,
            )?;
            (source, capture_type, retained_path)
        } else {
            (place.source, place.root_type, path)
        };
        self.assignment = Some(CapturedAssignmentDestination {
            statement_index: evaluation.statement_index,
            destination_expression,
            destination: destination.clone(),
            source,
            root_type,
            path,
            original_source: place.source,
            original_path,
        });
        Ok(())
    }

    fn assignment_destination(
        &self,
        statement_index: u32,
        source: &CheckedUnitStructuralArgumentSourcePlan,
        path: &[CheckedUnitStructuralPathSegment],
    ) -> Option<&CapturedAssignmentDestination> {
        self.assignment.as_ref().filter(|capture| {
            capture.statement_index == statement_index
                && &capture.destination.source == source
                && path.starts_with(&capture.destination.path)
        })
    }

    /// An emitter-owned displacement has no authored value expression. Associate
    /// it with this exact captured target; zero still means no capture, never
    /// permission to reuse another occurrence with the same storage spelling.
    pub(crate) fn captured_displacement_expression(
        &self,
        statement_index: u32,
        source: &CheckedUnitStructuralArgumentSourcePlan,
        path: &[CheckedUnitStructuralPathSegment],
    ) -> typed_trees::expression::ExpressionHandle {
        self.assignment_destination(statement_index, source, path)
            .map_or_else(
                typed_trees::expression::ExpressionHandle::invalid,
                |capture| capture.destination_expression,
            )
    }

    /// Return the retained target or a fixed descendant selected by the
    /// replacement's decomposition. Descendant cleanup paths are declaration
    /// paths; only the assignment prefix owns runtime selector evaluation.
    pub(crate) fn captured_assignment_place(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        statement_index: u32,
        source: &CheckedUnitStructuralArgumentSourcePlan,
        path: &[CheckedUnitStructuralPathSegment],
    ) -> Result<Option<(PlaceId, Vec<StructuralPathSegment>)>, LoweringError> {
        let Some(capture) = self.assignment_destination(statement_index, source, path) else {
            return Ok(None);
        };
        let mut retained = capture.path.clone();
        retained.extend(
            crate::expression_preparation::bindings::structural_paths::lower_structural_path(
                identities,
                &path[capture.destination.path.len()..],
            )?,
        );
        Ok(Some((capture.source, retained)))
    }

    pub(crate) fn captured_indexed_destination(
        &self,
        statement_index: u32,
        source: &CheckedUnitStructuralArgumentSourcePlan,
        descriptor_path: &[CheckedUnitStructuralPathSegment],
    ) -> Result<Option<CapturedIndexedDestination>, LoweringError> {
        let Some(capture) = &self.assignment else {
            return Ok(None);
        };
        if capture.statement_index != statement_index || &capture.destination.source != source {
            return Ok(None);
        }
        let Some(suffix) = capture.destination.path.strip_prefix(descriptor_path) else {
            return Ok(None);
        };
        if !matches!(
            suffix.first(),
            Some(
                CheckedUnitStructuralPathSegment::FixedIndex(_)
                    | CheckedUnitStructuralPathSegment::RuntimeIndex(_)
                    | CheckedUnitStructuralPathSegment::ComputedIndex(_)
            )
        ) {
            return Ok(None);
        }
        // After the last descriptor boundary each checked segment contributes
        // exactly one retained inline segment. A prefix above that boundary is
        // not recoverable from the captured address and cannot name this view.
        let prefix_length =
            capture
                .path
                .len()
                .checked_sub(suffix.len())
                .ok_or(LoweringError::Unsupported(
                    "indexed store descriptor precedes its captured backing",
                ))?;
        let Some(StructuralPathSegment::RuntimeIndex { index, .. }) =
            capture.path.get(prefix_length)
        else {
            return unsupported("indexed store lost its captured runtime coordinate");
        };
        Ok(Some(CapturedIndexedDestination {
            source: capture.source,
            path: capture.path[..prefix_length].to_vec(),
            coordinate: *index,
        }))
    }

    fn evaluated_place(
        capture: Option<&CapturedAssignmentDestination>,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        plan: &CheckedUnitStructuralArgumentPlan,
        place: &ResolvedStoragePlace,
        evaluation: &mut WindowEvaluation<'_, '_>,
        values: &mut Vec<terminal_psi::ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Result<EvaluatedStoragePlace, LoweringError> {
        if let Some(capture) = capture {
            let descendants =
                crate::expression_preparation::bindings::structural_paths::lower_structural_path(
                    identities,
                    &plan.path[capture.destination.path.len()..],
                )?;
            let mut path = capture.path.clone();
            path.extend_from_slice(&descendants);
            let mut original_path = capture.original_path.clone();
            original_path.extend(descendants);
            let field = place.field.as_ref().and_then(|field| {
                matches!(path.last(), Some(StructuralPathSegment::Field(_))).then_some(field.id)
            });
            if field.is_some() {
                path.pop();
            }
            if place.field.is_some() {
                original_path.pop();
            }
            return Ok(EvaluatedStoragePlace {
                source: capture.source,
                root_type: capture.root_type,
                path,
                field,
                original_source: capture.original_source,
                original_path,
            });
        }
        let indexes = runtime_elements::evaluate_selectors(
            identities,
            &place.steps,
            evaluation.checked,
            evaluation.machine,
            evaluation.state,
            evaluation.statement_index,
            evaluation.evaluation,
            evaluation.source_value_count,
            values,
            next_value,
            next_block,
            next_edge,
            operations,
            evaluation.calls,
        )?;
        let path = runtime_elements::complete_path(
            place.steps.clone(),
            indexes,
            evaluation.calls,
            operations,
        )?;
        Ok(EvaluatedStoragePlace {
            source: place.source,
            root_type: place.root_type,
            original_path: path.clone(),
            path,
            field: place.field.as_ref().map(|field| field.id),
            original_source: place.source,
        })
    }

    /// Emit `MoveStructuralField` for `moved`, the checked owned place a
    /// consuming move takes out of exclusive storage, binding the
    /// removed value as `result`. Returns the structural result place.
    pub(crate) fn emit_move(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        moved: &CheckedUnitStructuralArgumentPlan,
        expression: typed_trees::expression::ExpressionHandle,
        result: &CheckedUnitStructuralResultBindingPlan,
        result_claims: Vec<terminal_psi::StructuralResultClaimBinding>,
        parameters: &[StructuralParameterDeclaration],
        structural_types: &[StructuralTypeDeclaration],
        type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
        evaluation: &mut WindowEvaluation<'_, '_>,
        values: &mut Vec<terminal_psi::ValueDeclaration>,
        next_place: &mut u64,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Result<PlaceId, LoweringError> {
        let place = resolve_window_place(
            identities,
            moved,
            StructuralAccess::MutableBorrow,
            parameters,
            evaluation.evaluation,
            operations,
            structural_types,
            type_ids,
        )?;
        if lookup_type_id(type_ids, &result.type_identity)? != place.hole_type {
            return Err(unpinned(
                &place.spelling,
                "the moved binding's type differs from the declared field type",
            ));
        }
        if place.steps.is_empty() && place.field.is_none() && !place.restoration_required {
            return Err(unpinned(
                &place.spelling,
                "an owned whole local transfers without a restoration window",
            ));
        }
        // Only displacement belongs to the assignment target occurrence.
        // Matching a source/path is not enough: a genuine RHS extraction reads
        // its own later source, even when it spells that same storage route.
        let capture = self
            .assignment_destination(evaluation.statement_index, &moved.source, &moved.path)
            .filter(|capture| {
                expression.is_valid() && capture.destination_expression == expression
            });
        let evaluated = Self::evaluated_place(
            capture, identities, moved, &place, evaluation, values, next_value, next_block,
            next_edge, operations,
        )?;
        let source = evaluated.source;
        let path = evaluated.path;
        let field = evaluated.field;
        let (qualifications, projected_qualifications) =
            place.selected_memberships(identities, &evaluated.original_path)?;
        if let Some(absent) = self.open.iter().find(|absent| {
            absent.source == evaluated.original_source
                && paths_overlap(&absent.checked_path, &place.checked_path)
        }) {
            return Err(unpinned(
                &place.spelling,
                if absent.checked_path == place.checked_path {
                    "the place is already absent: repeated extraction of one open window"
                } else {
                    "the place overlaps an open window: extraction through absent storage"
                },
            ));
        }
        runtime_elements::observe_element_extent(
            source,
            evaluated.root_type,
            &path,
            structural_types,
            next_value,
            operations,
        )?;
        let moved_place = place_id(allocate_dense(next_place)?);
        // The RHS can observe storage again after capture cleared the cache.
        // Both spellings now denote the modified subtree; neither retains a
        // live descriptor observation across its extraction or replacement.
        operations
            .invalidate_field_lengths_under(evaluated.original_source, &evaluated.original_path);
        operations.invalidate_field_lengths_under(source, &path);
        operations.push_leaf(
            OperationResult::Structural(StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place: moved_place,
                structural_type: place.hole_type,
                multiplicity: terminal_multiplicity(result.multiplicity),
                qualifications,
                projected_qualifications,
                claims: result_claims,
            }),
            OperationKind::MoveStructuralField {
                source,
                path: path.clone(),
                field,
            },
        );
        self.open.push(OpenBorrowedWindow {
            opening: moved_place,
            source: evaluated.original_source,
            path: evaluated.original_path,
            field: place.field.as_ref().map(|field| field.id),
            hole_type: place.hole_type,
            checked_path: place.checked_path,
            spelling: place.spelling,
            restoration_required: place.restoration_required,
        });
        Ok(moved_place)
    }

    /// Install a whole completed value at the checked destination. An opening
    /// names exact restoration debt; without one, intact borrowed storage
    /// requires freely discardable replacement custody. Both retain the destination
    /// captured before the RHS and its exact declared type.
    pub(crate) fn emit_store(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        destination: &CheckedUnitStructuralArgumentPlan,
        opening: Option<PlaceId>,
        value: BorrowedWindowRepairValue,
        parameters: &[StructuralParameterDeclaration],
        structural_types: &[StructuralTypeDeclaration],
        type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
        evaluation: &mut WindowEvaluation<'_, '_>,
        values: &mut Vec<terminal_psi::ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Result<(), LoweringError> {
        let place = resolve_window_place(
            identities,
            destination,
            StructuralAccess::WriteOnlyBorrow,
            parameters,
            evaluation.evaluation,
            operations,
            structural_types,
            type_ids,
        )?;
        let capture = self.assignment_destination(
            evaluation.statement_index,
            &destination.source,
            &destination.path,
        );
        let evaluated = Self::evaluated_place(
            capture,
            identities,
            destination,
            &place,
            evaluation,
            values,
            next_value,
            next_block,
            next_edge,
            operations,
        )?;
        let source = evaluated.source;
        let path = evaluated.path;
        let field = evaluated.field;
        let index = opening.and_then(|opening| {
            self.open
                .iter()
                .position(|absent| absent.opening == opening)
        });
        if opening.is_some() && index.is_none() {
            return Err(unpinned(
                &place.spelling,
                "the restoration names no open extraction",
            ));
        }
        if let Some(index) = index {
            let absent = &self.open[index];
            if absent.source != evaluated.original_source
                || absent.field != place.field.as_ref().map(|field| field.id)
                || absent.hole_type != place.hole_type
            {
                return Err(unpinned(
                    &place.spelling,
                    "the restoration names a different storage or type than the extraction opened",
                ));
            }
        }
        let multiplicity = parameters
            .iter()
            .find(|parameter| parameter.place == value.place)
            .map(|parameter| parameter.multiplicity)
            .or_else(|| {
                operations.operations.iter().find_map(|operation| {
                    operation
                        .result
                        .structural()
                        .filter(|result| result.place == value.place)
                        .map(|result| result.multiplicity)
                })
            })
            .or_else(|| {
                // A selection or dispatch completes at a join's block
                // parameter, not at an operation result: the completed
                // binding's registry still names that place's custody.
                operations
                    .structural_values
                    .iter()
                    .find_map(|(_, binding)| {
                        binding
                            .operation_result()
                            .ok()
                            .filter(|result| result.place == value.place)
                            .map(|result| result.multiplicity)
                    })
            });
        let discardable = multiplicity == Some(StructuralMultiplicity::Unrestricted)
            || multiplicity == Some(StructuralMultiplicity::Affine)
                && structural_types.iter().any(|declaration| {
                    declaration.id == value.structural_type
                        && (declaration.no_code_affine_cleanup || declaration.descriptor_cleanup)
                });
        if index.is_none()
            && ((place.restoration_required && !discardable)
                || self.open.iter().any(|absent| {
                    absent.source == evaluated.original_source
                        && paths_overlap(&absent.checked_path, &place.checked_path)
                }))
        {
            let reason = if self.open.iter().any(|absent| {
                absent.source == evaluated.original_source
                    && paths_overlap(&absent.checked_path, &place.checked_path)
            }) {
                "the store overlaps an open window without naming its exact place"
            } else {
                "the store names a place with no open window: a replacement of established \
                 storage is not a restoration"
            };
            return Err(unpinned(&place.spelling, reason));
        }
        if value.structural_type != place.hole_type {
            return Err(unpinned(
                &place.spelling,
                "the repair value's type differs from the type moved out of the place",
            ));
        }
        let (required, required_projected) =
            place.selected_memberships(identities, &evaluated.original_path)?;
        let (offered, offered_projected) = parameters
            .iter()
            .filter(|parameter| parameter.place == value.place)
            .map(|parameter| {
                (
                    &parameter.qualifications,
                    &parameter.projected_qualifications,
                )
            })
            .chain(operations.operations.iter().filter_map(|operation| {
                operation
                    .result
                    .structural()
                    .filter(|result| result.place == value.place)
                    .map(|result| (&result.qualifications, &result.projected_qualifications))
            }))
            .chain(
                operations
                    .structural_values
                    .iter()
                    .filter_map(|(_, binding)| {
                        binding
                            .operation_result()
                            .ok()
                            .filter(|result| result.place == value.place)
                            .map(|result| {
                                (&result.qualifications, &result.projected_qualifications)
                            })
                    }),
            )
            .next()
            .ok_or_else(|| {
                unpinned(
                    &place.spelling,
                    "the repair value has no declared membership source",
                )
            })?;
        if !required.iter().all(|domain| offered.contains(domain))
            || !required_projected
                .iter()
                .all(|row| offered_projected.contains(row))
        {
            return Err(unpinned(
                &place.spelling,
                "the repair value does not restore the hole's exact qualifications",
            ));
        }
        runtime_elements::observe_element_extent(
            source,
            evaluated.root_type,
            &path,
            structural_types,
            next_value,
            operations,
        )?;
        operations
            .invalidate_field_lengths_under(evaluated.original_source, &evaluated.original_path);
        operations.invalidate_field_lengths_under(source, &path);
        operations.push_leaf(
            OperationResult::Unit,
            OperationKind::StoreStructuralField {
                disposition: if index.is_none() && place.restoration_required {
                    terminal_psi::StructuralStoreDisposition::ReplaceDiscardable
                } else {
                    terminal_psi::StructuralStoreDisposition::Vacant
                },
                destination: source,
                path,
                field,
                value: StructuralArgument {
                    place: value.place,
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                },
            },
        );
        if let Some(index) = index {
            self.open.remove(index);
        }
        Ok(())
    }

    /// Every non-crash exit (return, transition edge, or end of body) must
    /// find no open window on its path.
    pub(crate) fn require_closed(&self) -> Result<(), LoweringError> {
        match self.open.iter().find(|absent| absent.restoration_required) {
            None => Ok(()),
            Some(absent) => Err(unpinned(
                &absent.spelling,
                "the place is still absent at an exit: missing restoration on this path",
            )),
        }
    }
}

fn unpinned(place: &str, reason: &'static str) -> LoweringError {
    LoweringError::UnpinnedBorrowedStorageWindow {
        place: place.to_owned(),
        reason,
    }
}

pub(crate) fn terminal_multiplicity(multiplicity: Multiplicity) -> StructuralMultiplicity {
    match multiplicity {
        Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
        Multiplicity::Affine => StructuralMultiplicity::Affine,
        Multiplicity::Linear => StructuralMultiplicity::Linear,
    }
}

/// The multiplicity a declared-only local's zeroed whole shape carries. A
/// `[linear]` type's zero image owns only storage: implicit zero-filling
/// establishes no linear obligation (wiki/spec/language/ownership.md,
/// "Establishment and erasure"); the later assignment establishes the owed
/// value and its consumer discharges it. Terminal's zeroed establishment
/// therefore admits only non-linear results, and the zero image of a linear
/// carrier is presented as affine: disposable storage with no debt, exactly
/// as an affine carrier's zero image.
pub(crate) fn zeroed_image_multiplicity(multiplicity: Multiplicity) -> StructuralMultiplicity {
    zeroed_image(terminal_multiplicity(multiplicity))
}

/// The same rule over a carrier already lowered to its Terminal multiplicity:
/// both lanes mint a declared-only local's zero image through this one
/// mapping.
pub(crate) fn zeroed_image(multiplicity: StructuralMultiplicity) -> StructuralMultiplicity {
    match multiplicity {
        StructuralMultiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
        StructuralMultiplicity::Affine | StructuralMultiplicity::Linear => {
            StructuralMultiplicity::Affine
        }
    }
}

/// Two exact places beneath one root overlap when either names the other or
/// an ancestor of it. The comparison runs on the checked route: distinct
/// runtime selectors keep the authority the source checker admitted — a
/// selector equality question belongs to the value proof, not the ledger.
fn paths_overlap(
    first: &[CheckedUnitStructuralPathSegment],
    second: &[CheckedUnitStructuralPathSegment],
) -> bool {
    first.starts_with(second) || second.starts_with(first)
}

/// The frame pieces a window with runtime elements evaluates its selectors
/// through: the checked facts that retained each `AssignmentIndex`
/// coordinate, and the same scalar evaluation surface the store's own value
/// completes on.
pub(crate) struct WindowEvaluation<'a, 'calls> {
    pub(crate) checked: &'a checked_trees::CheckedTrees,
    pub(crate) machine: symbols::SymbolHandle,
    pub(crate) state: symbols::SymbolHandle,
    pub(crate) statement_index: u32,
    pub(crate) evaluation: &'a mut crate::unit::attached_unit::argument_evaluation::Evaluation,
    pub(crate) source_value_count: usize,
    pub(crate) calls: &'a mut CallEmissionContext<'calls>,
}

fn resolve_window_place(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    plan: &CheckedUnitStructuralArgumentPlan,
    required_access: StructuralAccess,
    parameters: &[StructuralParameterDeclaration],
    evaluation: &crate::unit::attached_unit::argument_evaluation::Evaluation,
    operations: &OperationBuffer,
    structural_types: &[StructuralTypeDeclaration],
    type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
) -> Result<ResolvedStoragePlace, LoweringError> {
    let place = resolve_storage_place(
        identities,
        plan,
        required_access,
        parameters,
        evaluation,
        operations,
        structural_types,
        type_ids,
    )?;
    if place
        .field
        .as_ref()
        .is_some_and(|field| field.field_type.as_structural().is_none())
    {
        return Err(unpinned(
            &place.spelling,
            "the field is not a structural subtree: scalar and byte fields open no window",
        ));
    }
    Ok(place)
}

/// Resolve storage geometry independently of consuming its contents. An
/// assignment may capture a primitive endpoint although it opens no owned
/// structural extraction window.
fn resolve_storage_place(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    plan: &CheckedUnitStructuralArgumentPlan,
    required_access: StructuralAccess,
    parameters: &[StructuralParameterDeclaration],
    evaluation: &crate::unit::attached_unit::argument_evaluation::Evaluation,
    operations: &OperationBuffer,
    structural_types: &[StructuralTypeDeclaration],
    type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
) -> Result<ResolvedStoragePlace, LoweringError> {
    let spelling = spell_place(plan);
    let (
        source,
        root_type,
        root_access,
        root_multiplicity,
        restoration_required,
        qualifications,
        projected_qualifications,
    ) = match plan.source {
        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
            // Plan coordinates index the retained structural roster. A
            // composed declaration's `position` belongs to source readers
            // and may include intervening scalar formals.
            let parameter = parameters
                .get(parameter_index as usize)
                .ok_or_else(|| unpinned(&spelling, "the parameter root is not declared"))?;
            if !required_access.admits(parameter.access) {
                return Err(unpinned(
                    &spelling,
                    "the root is not an exclusive `&mut` borrow: shared and write-only loans \
                     admit no extraction and owned roots use ordinary partial moves",
                ));
            }
            if parameter.access == StructuralAccess::MutableBorrow
                && !matches!(
                    parameter.multiplicity,
                    terminal_psi::nonlinear_multiplicity_pair_pattern!()
                )
            {
                return Err(unpinned(
                    &spelling,
                    "the root carries linear custody the window does not restore",
                ));
            }
            (
                parameter.place,
                parameter.structural_type,
                parameter.access,
                parameter.multiplicity,
                parameter.access != StructuralAccess::Owned,
                parameter.qualifications.clone(),
                parameter.projected_qualifications.clone(),
            )
        }
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
            // A window uses the same exclusive destination as an ordinary
            // field store. Referent traversal below retains the loan carrier;
            // accepting a local never substitutes its payload for its storage.
            let root = super::store_destination::local_store_root(
                symbol,
                &evaluation.structural_locals,
                evaluation.structural_root_parameters(),
                operations,
            )
            .map_err(|error| match error {
                LoweringError::Unsupported(reason) => unpinned(&spelling, reason),
                error => error,
            })?;
            (
                root.argument.place,
                root.structural_type,
                root.argument.access,
                root.multiplicity,
                root.argument.access != StructuralAccess::Owned,
                root.qualifications.to_vec(),
                root.projected_qualifications.to_vec(),
            )
        }
        _ => {
            return Err(unpinned(
                &spelling,
                "borrowed-storage windows require a mutable-borrowed parameter or an \
                 established owned or exclusive structural local",
            ));
        }
    };
    if plan.access != CheckedStructuralAccess::Owned {
        return Err(unpinned(
            &spelling,
            "the place is not taken as an owned value",
        ));
    }
    // A whole-root route replaces the referent of an exclusive parameter
    // loan: the hole is the loan's whole referent, of the parameter's own
    // declared type. An owned local's whole value moves by ordinary
    // transfer, never through a window.
    if plan.path.is_empty()
        && matches!(
            plan.source,
            CheckedUnitStructuralArgumentSourcePlan::Parameter { .. }
        )
    {
        if lookup_type_id(type_ids, &plan.type_identity)? != root_type {
            return Err(unpinned(
                &spelling,
                "the checked place type differs from the loan's referent type",
            ));
        }
        return Ok(ResolvedStoragePlace {
            source,
            root_type,
            root_access,
            root_multiplicity,
            steps: Vec::new(),
            checked_path: Vec::new(),
            field: None,
            hole_type: root_type,
            spelling,
            restoration_required,
            qualifications,
            projected_qualifications,
        });
    }
    let (steps, leaf, borrowed_backing) =
        lower_window_path(identities, root_type, &plan.path, structural_types).map_err(
            |error| match error {
                LoweringError::Unsupported(reason) => unpinned(&spelling, reason),
                other => other,
            },
        )?;
    // Owning a reference/view descriptor never owns its backing storage.
    let restoration_required = restoration_required || borrowed_backing;
    let checked_leaf_type = lookup_type_id(type_ids, &plan.type_identity)?;
    let (field, hole_type) = match leaf {
        WindowLeaf::Field(field) => {
            let Some(hole_type) = field.field_type.as_structural().or_else(|| {
                if let Some(carrier) = field.field_type.byte_sequence_carrier() {
                    // The checked destination already names its exact leaf
                    // type. Equal backing geometry elsewhere in the roster
                    // cannot select or make that declaration ambiguous.
                    let declaration =
                        terminal_psi::find_structural_type(structural_types, checked_leaf_type)?;
                    let terminal_psi::StructuralTypeShape::ByteSequence(declared) =
                        declaration.shape
                    else {
                        return None;
                    };
                    return terminal_semantics::byte_carrier_geometry_matches(carrier, declared)
                        .then_some(checked_leaf_type);
                }
                field
                    .field_type
                    .leaf_declaration(structural_types.iter())
                    .map(|row| row.id)
            }) else {
                return Err(unpinned(
                    &spelling,
                    "the field has no exact declared storage endpoint",
                ));
            };
            (Some(field.clone()), hole_type)
        }
        WindowLeaf::Subtree { structural_type } => (None, structural_type),
    };
    if checked_leaf_type != hole_type {
        return Err(unpinned(
            &spelling,
            "the checked place type differs from the declared leaf type",
        ));
    }
    let mut checked_path = plan.path.clone();
    if matches!(
        steps.first(),
        Some(ProjectionStep::Static(StructuralPathSegment::Referent))
    ) {
        checked_path.insert(0, CheckedUnitStructuralPathSegment::Referent);
    }
    Ok(ResolvedStoragePlace {
        source,
        root_type,
        root_access,
        root_multiplicity,
        steps,
        checked_path,
        field,
        hole_type,
        spelling,
        restoration_required,
        qualifications,
        projected_qualifications,
    })
}

/// The authored-facing name of a checked place, followed by its path segments.
fn spell_place(plan: &CheckedUnitStructuralArgumentPlan) -> String {
    let mut spelling = match plan.source {
        CheckedUnitStructuralArgumentSourcePlan::ScalarParameter { parameter_index } => {
            format!("scalar parameter#{parameter_index}")
        }
        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
            format!("parameter#{parameter_index}")
        }
        CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { .. }
        | CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
        | CheckedUnitStructuralArgumentSourcePlan::StructuralLocalBorrow { .. } => {
            "local".to_owned()
        }
        CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { .. } => {
            "computed expression".to_owned()
        }
        CheckedUnitStructuralArgumentSourcePlan::TrivialAffineLocal {
            declaration_ordinal,
        } => format!("local#{declaration_ordinal}"),
        CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
            format!("result#{binding_ordinal}")
        }
        CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { .. } => "literal".to_owned(),
        CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
            root:
                checked_trees::CheckedStorageRoot::Parameter {
                    index: parameter_index,
                },
            ..
        }
        | CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
            root:
                checked_trees::CheckedStorageRoot::Parameter {
                    index: parameter_index,
                },
            ..
        } => format!("parameter#{parameter_index}[..]"),
        CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
            root: checked_trees::CheckedStorageRoot::ViewLocal { .. },
            ..
        }
        | CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
            root: checked_trees::CheckedStorageRoot::ViewLocal { .. },
            ..
        } => "local[..]".to_owned(),
        CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
            root: checked_trees::CheckedStorageRoot::OwnedLocal { .. },
            ..
        }
        | CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
            root: checked_trees::CheckedStorageRoot::OwnedLocal { .. },
            ..
        } => "local[..]".to_owned(),
    };
    for segment in &plan.path {
        match segment {
            CheckedUnitStructuralPathSegment::RepresentationView { type_identity, .. } => {
                spelling.push_str(&format!(".[view {type_identity:?}]"));
            }
            CheckedUnitStructuralPathSegment::Case(identity) => {
                spelling.push_str(".[");
                spelling.push_str(&identity.spelling());
                spelling.push(']');
            }
            CheckedUnitStructuralPathSegment::Field(identity) => {
                spelling.push('.');
                spelling.push_str(&identity.spelling());
            }
            CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                spelling.push_str(&format!("[{index}]"));
            }
            CheckedUnitStructuralPathSegment::Referent => spelling.push_str(".*"),
            CheckedUnitStructuralPathSegment::FixedByteRange { start, end } => {
                spelling.push_str(&format!("[{start}..{end}]"));
            }
            CheckedUnitStructuralPathSegment::ComputedIndex(root) => {
                spelling.push_str(&format!("[computed#{root:?}]"));
            }
            CheckedUnitStructuralPathSegment::RuntimeIndex(
                checked_trees::CheckedRuntimeIndex::StorageRead { symbol, .. },
            ) => spelling.push_str(&format!("[storage#{symbol:?}]")),
            CheckedUnitStructuralPathSegment::RuntimeIndex(
                checked_trees::CheckedRuntimeIndex::Parameter { position },
            ) => {
                spelling.push_str(&format!("[selector#{position}]"));
            }
            CheckedUnitStructuralPathSegment::RuntimeIndex(
                checked_trees::CheckedRuntimeIndex::AssignmentIndex { depth },
            ) => spelling.push_str(&format!("[index#{depth}]")),
            CheckedUnitStructuralPathSegment::RuntimeIndex(
                checked_trees::CheckedRuntimeIndex::Local { position },
            ) => spelling.push_str(&format!("[local#{position}]")),
            CheckedUnitStructuralPathSegment::RuntimeIndex(
                checked_trees::CheckedRuntimeIndex::StructuralField(field),
            ) => {
                spelling.push_str(&format!(
                    "[field#{}:{}]",
                    field.parameter_position,
                    field
                        .path
                        .iter()
                        .map(|segment| match segment {
                            checked_trees::CheckedStructuralPredicatePathSegment::Field(
                                identity,
                            ) => identity.spelling(),
                            checked_trees::CheckedStructuralPredicatePathSegment::Case(case) => {
                                format!("as {}", case.spelling())
                            }
                            checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(
                                index,
                            ) => index.to_string(),
                            checked_trees::CheckedStructuralPredicatePathSegment::RuntimeIndex(
                                _,
                            ) => "rt".to_string(),
                        })
                        .collect::<Vec<_>>()
                        .join(".")
                ));
            }
        }
    }
    spelling
}
