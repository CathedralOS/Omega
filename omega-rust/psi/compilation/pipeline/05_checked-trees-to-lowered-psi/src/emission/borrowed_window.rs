//! Borrowed-storage restoration windows: `MoveStructuralField` at the move
//! out of exclusive borrowed storage and `StoreStructuralField` at the
//! restoring assignment on the same exact place.
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
//! The ledger fails closed. Every shape the checked facts do not pin exactly
//! — a root without owned or exclusive mutable custody, a whole-root route
//! beneath anything but an exclusive parameter loan, an unsupported projection,
//! a scalar or ambiguous field, a moved type that
//! differs from the declared field, extraction over an open hole, a repair at
//! another place, a repair value that is not a whole owned value of the hole's
//! type, or an open hole at a sequence exit — is a
//! `LoweringError::UnpinnedBorrowedStorageWindow` naming
//! the place.
//!
//! A returned record reference retains its carrier place and an explicit
//! `Referent` crossing in both the emitted path and ledger key. Geometry walks
//! the referent; custody never changes to ownership of the referent. The same
//! declaration walk serves scalar stores and structural restoration windows.
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
    CheckedUnitStructuralPathSegment, LoweringError, Operation, OperationKind, OperationResult,
    PlaceId, StructuralAccess, StructuralFieldId, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeId, allocate_dense, place_id,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::runtime_elements::{self, ProjectionStep};
use crate::emission::structural_scalar_store::{WindowLeaf, lower_window_path};
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
    pub(crate) source: PlaceId,
    pub(crate) path: Vec<StructuralPathSegment>,
    /// A field extending `path`, or `None` when `path` names the whole hole.
    pub(crate) field: Option<StructuralFieldId>,
    pub(crate) hole_type: StructuralTypeId,
    /// The checked route the authored statement spelled, leaf included: hole
    /// identity is compared at this coordinate — Terminal runtime-element
    /// segments evaluate fresh `ValueId`s per operation, so equal places
    /// share the checked path, not the emitted one.
    checked_path: Vec<CheckedUnitStructuralPathSegment>,
    spelling: String,
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
}

/// The resolved exact place a move or repair names beneath a window root:
/// the carrier's projection steps (a runtime element evaluates when the
/// operation emits), the checked route as the hole's key, the leaf's field
/// declaration id (`None` for a root window), and the hole's declared type.
struct ResolvedWindowPlace {
    source: PlaceId,
    steps: Vec<ProjectionStep>,
    checked_path: Vec<CheckedUnitStructuralPathSegment>,
    field: Option<StructuralFieldId>,
    hole_type: StructuralTypeId,
    spelling: String,
    restoration_required: bool,
}

impl BorrowedWindowLedger {
    /// Emit `MoveStructuralField` for `moved`, the checked owned place a
    /// consuming move takes out of exclusive storage, binding the
    /// removed value as `result`. Returns the structural result place.
    pub(crate) fn emit_move(
        &mut self,
        moved: &CheckedUnitStructuralArgumentPlan,
        result: &CheckedUnitStructuralResultBindingPlan,
        parameters: &[StructuralParameterDeclaration],
        structural_types: &[StructuralTypeDeclaration],
        type_ids: &[(String, StructuralTypeId)],
        evaluation: &mut WindowEvaluation<'_, '_>,
        values: &mut Vec<terminal_psi::ValueDeclaration>,
        next_place: &mut u64,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Result<PlaceId, LoweringError> {
        let place = resolve_window_place(
            moved,
            parameters,
            &evaluation.evaluation.structural_locals,
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
        let indexes = runtime_elements::evaluate_selectors(
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
        let path =
            runtime_elements::complete_path(place.steps, indexes, evaluation.calls, operations)?;
        if let Some(absent) = self.open.iter().find(|absent| {
            absent.source == place.source
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
        let moved_place = place_id(allocate_dense(next_place)?);
        operations.invalidate_field_lengths_under(place.source, &path);
        let id = operations.allocate();
        operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id,
            result: OperationResult::Structural(StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place: moved_place,
                structural_type: place.hole_type,
                multiplicity: terminal_multiplicity(result.multiplicity),
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::MoveStructuralField {
                source: place.source,
                path: path.clone(),
                field: place.field,
            },
        });
        self.open.push(OpenBorrowedWindow {
            source: place.source,
            path,
            field: place.field,
            hole_type: place.hole_type,
            checked_path: place.checked_path,
            spelling: place.spelling,
        });
        Ok(moved_place)
    }

    /// Emit `StoreStructuralField` reseating the open window at
    /// `destination`, the checked place the restoring assignment names, with
    /// `value`. The destination must be an open hole exactly; the value must
    /// be a whole owned place of the hole's declared type.
    pub(crate) fn emit_store(
        &mut self,
        destination: &CheckedUnitStructuralArgumentPlan,
        value: BorrowedWindowRepairValue,
        parameters: &[StructuralParameterDeclaration],
        structural_types: &[StructuralTypeDeclaration],
        type_ids: &[(String, StructuralTypeId)],
        evaluation: &mut WindowEvaluation<'_, '_>,
        values: &mut Vec<terminal_psi::ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Result<(), LoweringError> {
        let place = resolve_window_place(
            destination,
            parameters,
            &evaluation.evaluation.structural_locals,
            operations,
            structural_types,
            type_ids,
        )?;
        let index = self.open.iter().position(|absent| {
            absent.source == place.source && absent.checked_path == place.checked_path
        });
        if index.is_none()
            && (place.restoration_required
                || self.open.iter().any(|absent| {
                    absent.source == place.source
                        && paths_overlap(&absent.checked_path, &place.checked_path)
                }))
        {
            let reason = if self.open.iter().any(|absent| {
                absent.source == place.source
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
        let indexes = runtime_elements::evaluate_selectors(
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
        let path =
            runtime_elements::complete_path(place.steps, indexes, evaluation.calls, operations)?;
        operations.invalidate_field_lengths_under(place.source, &path);
        let id = operations.allocate();
        operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id,
            result: OperationResult::Unit,
            kind: OperationKind::StoreStructuralField {
                destination: place.source,
                path,
                field: place.field,
                value: StructuralArgument {
                    place: value.place,
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                },
            },
        });
        if let Some(index) = index {
            self.open.remove(index);
        }
        Ok(())
    }

    /// Every non-crash exit (return, transition edge, or end of body) must
    /// find no open window on its path.
    pub(crate) fn require_closed(&self) -> Result<(), LoweringError> {
        match self.open.first() {
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

fn terminal_multiplicity(multiplicity: Multiplicity) -> StructuralMultiplicity {
    match multiplicity {
        Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
        Multiplicity::Affine => StructuralMultiplicity::Affine,
        Multiplicity::Linear => StructuralMultiplicity::Linear,
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
    plan: &CheckedUnitStructuralArgumentPlan,
    parameters: &[StructuralParameterDeclaration],
    locals: &[crate::expression_preparation::bindings::StructuralLocalBinding],
    operations: &OperationBuffer,
    structural_types: &[StructuralTypeDeclaration],
    type_ids: &[(String, StructuralTypeId)],
) -> Result<ResolvedWindowPlace, LoweringError> {
    let spelling = spell_place(plan);
    let (source, root_type, restoration_required) = match plan.source {
        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
            // Plan coordinates index the retained structural roster. A
            // composed declaration's `position` belongs to source readers
            // and may include intervening scalar formals.
            let parameter = parameters
                .get(parameter_index as usize)
                .ok_or_else(|| unpinned(&spelling, "the parameter root is not declared"))?;
            if parameter.access != StructuralAccess::MutableBorrow {
                return Err(unpinned(
                    &spelling,
                    "the root is not an exclusive `&mut` borrow: shared and write-only loans \
                     admit no extraction and owned roots use ordinary partial moves",
                ));
            }
            if !matches!(
                parameter.multiplicity,
                StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
            ) || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
            {
                return Err(unpinned(
                    &spelling,
                    "the root carries qualifications or linear custody the window does not \
                     restore",
                ));
            }
            (parameter.place, parameter.structural_type, true)
        }
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
            // A window uses the same exclusive destination as an ordinary
            // field store. Referent traversal below retains the loan carrier;
            // accepting a local never substitutes its payload for its storage.
            let (source, root_type) = super::store_destination::unqualified_local_store_root(
                symbol, locals, operations,
            ).map_err(|_| unpinned(
                &spelling,
                "the local root is not an established structural local with unqualified exclusive custody",
            ))?;
            let (argument, _) =
                super::store_destination::local_store_root(symbol, locals, operations)?;
            (
                source,
                root_type,
                argument.access != StructuralAccess::Owned,
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
    if plan.path.is_empty() {
        if !matches!(
            plan.source,
            CheckedUnitStructuralArgumentSourcePlan::Parameter { .. }
        ) {
            return Err(unpinned(
                &spelling,
                "whole-root routes open a window only beneath an exclusive `&mut` parameter loan",
            ));
        }
        if lookup_type_id(type_ids, &plan.type_identity)? != root_type {
            return Err(unpinned(
                &spelling,
                "the checked place type differs from the loan's referent type",
            ));
        }
        return Ok(ResolvedWindowPlace {
            source,
            steps: Vec::new(),
            checked_path: Vec::new(),
            field: None,
            hole_type: root_type,
            spelling,
            restoration_required,
        });
    }
    let (steps, leaf) = lower_window_path(root_type, &plan.path, structural_types).map_err(|_| {
        let reason = if plan.path.iter().any(|segment| {
            !matches!(segment, CheckedUnitStructuralPathSegment::Field(_))
        }) {
            "index or referent routes do not resolve through the declared records and              fixed arrays"
        } else {
            "the route does not resolve through the declared records and fixed arrays"
        };
        unpinned(&spelling, reason)
    })?;
    let (field, hole_type) = match leaf {
        WindowLeaf::Field(field) => {
            let StructuralFieldType::Structural(hole_type) = field.field_type else {
                return Err(unpinned(
                    &spelling,
                    "the field is not a structural subtree: scalar and byte fields open no \
                     window",
                ));
            };
            (Some(field.id), hole_type)
        }
        WindowLeaf::Element { element_type } => (None, element_type),
    };
    if lookup_type_id(type_ids, &plan.type_identity)? != hole_type {
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
    Ok(ResolvedWindowPlace {
        source,
        steps,
        checked_path,
        field,
        hole_type,
        spelling,
        restoration_required,
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
        | CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. } => "local".to_owned(),
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
            CheckedUnitStructuralPathSegment::Case(identity) => {
                spelling.push_str(".[");
                spelling.push_str(identity);
                spelling.push(']');
            }
            CheckedUnitStructuralPathSegment::Field(identity) => {
                spelling.push('.');
                spelling.push_str(identity);
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
                            ) => identity.clone(),
                            checked_trees::CheckedStructuralPredicatePathSegment::Case(case) => {
                                format!("as {case}")
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
