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
//! the place. Terminal verification reconstructs the same debt independently
//! (`terminal-verifier/src/validation/borrowed_windows.rs`); nothing here is
//! evidence for it.
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
}

impl BorrowedWindowLedger {
    #[cfg(test)]
    pub(crate) fn open_windows(&self) -> &[OpenBorrowedWindow] {
        &self.open
    }

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
        let path = runtime_elements::complete_path(place.steps, indexes, evaluation.calls)?;
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
        let Some(index) = self.open.iter().position(|absent| {
            absent.source == place.source && absent.checked_path == place.checked_path
        }) else {
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
        };
        if value.structural_type != self.open[index].hole_type {
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
        let path = runtime_elements::complete_path(place.steps, indexes, evaluation.calls)?;
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
        self.open.remove(index);
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
    let spelling = spell_place(plan, parameters);
    let (source, root_type) = match plan.source {
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
            (parameter.place, parameter.structural_type)
        }
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
            // A window uses the same exclusive destination as an ordinary
            // field store. Referent traversal below retains the loan carrier;
            // accepting a local never substitutes its payload for its storage.
            super::store_destination::unqualified_local_store_root(
                symbol, locals, operations,
            ).map_err(|_| unpinned(
                &spelling,
                "the local root is not an established structural local with unqualified exclusive custody",
            ))?
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
    })
}

/// The authored-facing name of a checked place for diagnostics: the receiver
/// or parameter position, then each spelled segment.
fn spell_place(
    plan: &CheckedUnitStructuralArgumentPlan,
    parameters: &[StructuralParameterDeclaration],
) -> String {
    let mut spelling = match plan.source {
        CheckedUnitStructuralArgumentSourcePlan::ScalarParameter { parameter_index } => {
            format!("scalar parameter#{parameter_index}")
        }
        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
            match parameters.get(parameter_index as usize) {
                Some(parameter) if parameter.is_self => "self".to_owned(),
                _ => format!("parameter#{parameter_index}"),
            }
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

#[cfg(test)]
mod tests {
    //! The emitter is exercised on Terminal declarations directly, and the
    //! emitted pair is replayed through `terminal_verifier::validate_module`
    //! so the spelling is the one independent verification reconstructs.

    use super::{BorrowedWindowLedger, BorrowedWindowRepairValue, WindowEvaluation};
    use crate::TerminalMachineSelection;
    use crate::emission::operation_emission::buffer::OperationBuffer;
    use crate::emission::operation_emission::calls::CallEmissionContext;
    use crate::lowering_error::LoweringError;
    use crate::unit::attached_unit::argument_evaluation::Evaluation;
    use checked_trees::{
        CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
        CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
        CheckedUnitStructuralResultBindingPlan,
    };
    use language_semantics::Multiplicity;
    use semantic_vocabulary::{
        BlockId, ContractId, EdgeId, MachineId, PlaceId, ScalarType, StructuralFieldId,
        StructuralPlaceKind, StructuralTypeId,
    };
    use terminal_psi::{
        BindingRelevance, Block, MachineContract, OperationKind, OperationResult, StructuralAccess,
        StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
        StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeDeclaration,
        StructuralTypeShape, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
        VocabularyMarker,
    };

    fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
        constructor(raw).expect("nonzero fixture identity")
    }

    const CELL: &str = "Cell";
    const ENVELOPE: &str = "Envelope";
    const CRATE: &str = "Crate";

    fn cell() -> StructuralTypeId {
        id(1, StructuralTypeId::new)
    }

    fn envelope() -> StructuralTypeId {
        id(2, StructuralTypeId::new)
    }

    fn crate_type() -> StructuralTypeId {
        id(3, StructuralTypeId::new)
    }

    fn flag() -> StructuralFieldId {
        id(1, StructuralFieldId::new)
    }

    fn left() -> StructuralFieldId {
        id(1, StructuralFieldId::new)
    }

    fn right() -> StructuralFieldId {
        id(2, StructuralFieldId::new)
    }

    fn count() -> StructuralFieldId {
        id(3, StructuralFieldId::new)
    }

    fn inner() -> StructuralFieldId {
        id(1, StructuralFieldId::new)
    }

    fn field(
        id: StructuralFieldId,
        identity: &str,
        field_type: StructuralFieldType,
    ) -> StructuralFieldDeclaration {
        StructuralFieldDeclaration {
            id,
            identity: identity.into(),
            relevance: BindingRelevance::Relevant,
            field_type,
        }
    }

    /// `Cell { flag: bool }`, `Envelope { left: Cell, right: Cell, count: bool }`,
    /// `Crate { inner: Envelope }`.
    fn structural_types() -> Vec<StructuralTypeDeclaration> {
        vec![
            StructuralTypeDeclaration {
                id: cell(),
                identity: CELL.into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![field(
                        flag(),
                        "flag",
                        StructuralFieldType::Scalar(ScalarType::Boolean),
                    )],
                },
            },
            StructuralTypeDeclaration {
                id: envelope(),
                identity: ENVELOPE.into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![
                        field(left(), "left", StructuralFieldType::Structural(cell())),
                        field(right(), "right", StructuralFieldType::Structural(cell())),
                        field(
                            count(),
                            "count",
                            StructuralFieldType::Scalar(ScalarType::Boolean),
                        ),
                    ],
                },
            },
            StructuralTypeDeclaration {
                id: crate_type(),
                identity: CRATE.into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![field(
                        inner(),
                        "inner",
                        StructuralFieldType::Structural(envelope()),
                    )],
                },
            },
        ]
    }

    fn type_ids() -> Vec<(String, StructuralTypeId)> {
        vec![
            (CELL.into(), cell()),
            (ENVELOPE.into(), envelope()),
            (CRATE.into(), crate_type()),
        ]
    }

    fn parameter(
        structural_type: StructuralTypeId,
        access: StructuralAccess,
    ) -> StructuralParameterDeclaration {
        StructuralParameterDeclaration {
            place: id(1, PlaceId::new),
            position: 0,
            is_self: true,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }
    }

    fn envelope_receiver() -> Vec<StructuralParameterDeclaration> {
        vec![parameter(envelope(), StructuralAccess::MutableBorrow)]
    }

    fn checked_place(
        path: &[CheckedUnitStructuralPathSegment],
        type_identity: &str,
    ) -> CheckedUnitStructuralArgumentPlan {
        CheckedUnitStructuralArgumentPlan {
            source: CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 },
            path: path.to_vec(),
            type_identity: type_identity.into(),
            access: CheckedStructuralAccess::Owned,
        }
    }

    fn fields(identities: &[&str]) -> Vec<CheckedUnitStructuralPathSegment> {
        identities
            .iter()
            .map(|identity| CheckedUnitStructuralPathSegment::Field((*identity).into()))
            .collect()
    }

    fn cell_binding() -> CheckedUnitStructuralResultBindingPlan {
        CheckedUnitStructuralResultBindingPlan {
            statement_index: 0,
            binding_ordinal: 0,
            type_identity: CELL.into(),
            multiplicity: Multiplicity::Unrestricted,
        }
    }

    /// The checked/scalar evaluation surface `emit_move`/`emit_store` need
    /// to resolve runtime element selectors; the fixtures' paths are all
    /// field-only, so this harness is never read.
    struct WindowHarness {
        checked: checked_trees::CheckedTrees,
        calls: CallEmissionContext<'static>,
        evaluation: Evaluation,
        values: Vec<terminal_psi::ValueDeclaration>,
        next_value: u64,
        next_block: u64,
        next_edge: u64,
    }

    impl WindowHarness {
        fn new() -> Self {
            // Identities are ZII: zero is absence, so block ids start at 1.
            let mut next_block = 1;
            Self {
                checked: checked_trees::CheckedTrees::default(),
                calls: CallEmissionContext {
                    machine_ids: &[],
                    requirement_counts: &[],
                    boundaries: &[],
                    next_obligation_identity: 0,
                    obligation_limit: 0,
                },
                evaluation: Evaluation::new(&mut next_block).expect("empty evaluation frame"),
                values: Vec::new(),
                next_value: 0,
                next_block,
                next_edge: 0,
            }
        }

        fn emit_move(
            &mut self,
            ledger: &mut BorrowedWindowLedger,
            moved: &CheckedUnitStructuralArgumentPlan,
            result: &CheckedUnitStructuralResultBindingPlan,
            parameters: &[StructuralParameterDeclaration],
            next_place: &mut u64,
            operations: &mut OperationBuffer,
        ) -> Result<PlaceId, LoweringError> {
            let Self {
                checked,
                calls,
                evaluation,
                values,
                next_value,
                next_block,
                next_edge,
            } = self;
            ledger.emit_move(
                moved,
                result,
                parameters,
                &structural_types(),
                &type_ids(),
                &mut WindowEvaluation {
                    checked,
                    machine: symbols::SymbolHandle::invalid(),
                    state: symbols::SymbolHandle::invalid(),
                    statement_index: 0,
                    evaluation,
                    source_value_count: 0,
                    calls,
                },
                values,
                next_place,
                next_value,
                next_block,
                next_edge,
                operations,
            )
        }

        fn emit_store(
            &mut self,
            ledger: &mut BorrowedWindowLedger,
            destination: &CheckedUnitStructuralArgumentPlan,
            value: BorrowedWindowRepairValue,
            parameters: &[StructuralParameterDeclaration],
            operations: &mut OperationBuffer,
        ) -> Result<(), LoweringError> {
            let Self {
                checked,
                calls,
                evaluation,
                values,
                next_value,
                next_block,
                next_edge,
            } = self;
            ledger.emit_store(
                destination,
                value,
                parameters,
                &structural_types(),
                &type_ids(),
                &mut WindowEvaluation {
                    checked,
                    machine: symbols::SymbolHandle::invalid(),
                    state: symbols::SymbolHandle::invalid(),
                    statement_index: 0,
                    evaluation,
                    source_value_count: 0,
                    calls,
                },
                values,
                next_value,
                next_block,
                next_edge,
                operations,
            )
        }
    }

    struct Emission {
        ledger: BorrowedWindowLedger,
        operations: OperationBuffer,
        next_place: u64,
        harness: WindowHarness,
    }

    impl Emission {
        fn new() -> Self {
            Self {
                ledger: BorrowedWindowLedger::default(),
                operations: OperationBuffer::new(0),
                // Place 1 is the parameter root; results follow it.
                next_place: 2,
                harness: WindowHarness::new(),
            }
        }

        fn emit_move(
            &mut self,
            parameters: &[StructuralParameterDeclaration],
            moved: &CheckedUnitStructuralArgumentPlan,
        ) -> Result<PlaceId, LoweringError> {
            self.harness.emit_move(
                &mut self.ledger,
                moved,
                &cell_binding(),
                parameters,
                &mut self.next_place,
                &mut self.operations,
            )
        }

        fn emit_store(
            &mut self,
            parameters: &[StructuralParameterDeclaration],
            destination: &CheckedUnitStructuralArgumentPlan,
            value: BorrowedWindowRepairValue,
        ) -> Result<(), LoweringError> {
            self.harness.emit_store(
                &mut self.ledger,
                destination,
                value,
                parameters,
                &mut self.operations,
            )
        }
    }

    fn assert_unpinned(error: LoweringError, place: &str, reason_fragment: &str) {
        let LoweringError::UnpinnedBorrowedStorageWindow {
            place: named,
            reason,
        } = error
        else {
            panic!("expected an unpinned-window refusal, got {error:?}");
        };
        assert_eq!(named, place);
        assert!(
            reason.contains(reason_fragment),
            "reason `{reason}` does not mention `{reason_fragment}`"
        );
    }

    /// A Terminal module whose single unattached machine borrows the
    /// `parameters` (declared as plain positional parameters: an `is_self`
    /// row requires an attachment) and runs `operations` before a Unit
    /// return, with every structural result place declared.
    fn module(
        emission: &Emission,
        parameters: &[StructuralParameterDeclaration],
    ) -> TerminalModule {
        let structural_parameters = parameters
            .iter()
            .map(|parameter| StructuralParameterDeclaration {
                is_self: false,
                ..parameter.clone()
            })
            .collect::<Vec<_>>();
        let mut structural_places = structural_parameters
            .iter()
            .map(|parameter| StructuralPlaceDeclaration {
                id: parameter.place,
                kind: StructuralPlaceKind::Parameter {
                    position: parameter.position,
                    is_self: false,
                },
            })
            .collect::<Vec<_>>();
        for operation in &emission.operations.operations {
            if let OperationResult::Structural(result) = &operation.result {
                structural_places.push(StructuralPlaceDeclaration {
                    id: result.place,
                    kind: StructuralPlaceKind::OperationResult {
                        producer: operation.id,
                        structural_type: result.structural_type,
                    },
                });
            }
        }
        TerminalModule {
            scalar_qualifications: Default::default(),
            scalar_block_invariants: Vec::new(),
            operation_crash_contracts: Vec::new(),
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: id(1, MachineId::new),
            structural_types: structural_types(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            placed_view_inputs: Vec::new(),
            reborrow_root_handoffs: Vec::new(),
            reborrow_restored_call_uses: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            proof_recursive_components: Vec::new(),
            closed_conformance_applications: Vec::new(),
            dynamic_dispatch: Default::default(),
            suspension_call_plan_count: 0,
            suspension_call_sites: Vec::new(),
            suspension_call_plans: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines: vec![TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: id(1, MachineId::new),
                attachment: None,
                parameters: Vec::new(),
                structural_parameters,
                ranked_scc: None,
                result: TerminalMachineResult::Unit,
                structural_places,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: id(1, BlockId::new),
                blocks: vec![Block {
                    erased_proof_formals: Vec::new(),
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: id(1, BlockId::new),
                    parameters: Vec::new(),
                    operations: emission.operations.operations.clone(),
                    terminator: Terminator::ReturnUnit {
                        edge: id(1, EdgeId::new),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: MachineContract {
                    erased_proof_formals: Vec::new(),
                    erased_scalar_formals: Vec::new(),
                    id: id(1, ContractId::new),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        }
    }

    #[test]
    fn a_disjoint_sibling_window_stays_independent() {
        let receiver = envelope_receiver();
        let mut emission = Emission::new();
        let left_value = emission
            .emit_move(&receiver, &checked_place(&fields(&["left"]), CELL))
            .expect("open left");
        let right_value = emission
            .emit_move(&receiver, &checked_place(&fields(&["right"]), CELL))
            .expect("open the disjoint right sibling");
        assert_eq!(emission.ledger.open_windows().len(), 2);
        emission
            .emit_store(
                &receiver,
                &checked_place(&fields(&["right"]), CELL),
                BorrowedWindowRepairValue {
                    place: right_value,
                    structural_type: cell(),
                },
            )
            .expect("close right first");
        assert_unpinned(
            emission.ledger.require_closed().unwrap_err(),
            "self.left",
            "missing restoration",
        );
        emission
            .emit_store(
                &receiver,
                &checked_place(&fields(&["left"]), CELL),
                BorrowedWindowRepairValue {
                    place: left_value,
                    structural_type: cell(),
                },
            )
            .expect("close left");
        emission.ledger.require_closed().expect("both closed");
        terminal_verifier::validate_module(&module(&emission, &receiver))
            .expect("two windows verify");
    }

    #[test]
    fn roots_that_are_not_exclusive_parameter_borrows_reject() {
        let mut emission = Emission::new();
        for access in [
            StructuralAccess::Owned,
            StructuralAccess::SharedBorrow,
            StructuralAccess::WriteOnlyBorrow,
        ] {
            let error = emission
                .emit_move(
                    &[parameter(envelope(), access)],
                    &checked_place(&fields(&["left"]), CELL),
                )
                .unwrap_err();
            assert_unpinned(error, "self.left", "not an exclusive `&mut` borrow");
        }
        let local = CheckedUnitStructuralArgumentPlan {
            source: CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                symbol: symbols::SymbolHandle::invalid(),
            },
            ..checked_place(&fields(&["left"]), CELL)
        };
        assert_unpinned(
            emission
                .emit_move(&envelope_receiver(), &local)
                .unwrap_err(),
            "local.left",
            "not an established structural local",
        );
        let undeclared = CheckedUnitStructuralArgumentPlan {
            source: CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 3 },
            ..checked_place(&fields(&["left"]), CELL)
        };
        assert_unpinned(
            emission
                .emit_move(&envelope_receiver(), &undeclared)
                .unwrap_err(),
            "parameter#3.left",
            "not declared",
        );
        assert!(emission.operations.operations.is_empty());
        assert!(emission.ledger.open_windows().is_empty());
    }

    #[test]
    fn routes_the_checked_facts_do_not_pin_reject() {
        let receiver = envelope_receiver();
        let mut emission = Emission::new();
        // The whole root resolves to the loan's referent, so a binding of
        // another type cannot take it.
        assert_unpinned(
            emission
                .emit_move(&receiver, &checked_place(&[], ENVELOPE))
                .unwrap_err(),
            "self",
            "moved binding's type differs",
        );
        let indexed = vec![
            CheckedUnitStructuralPathSegment::FixedIndex(0),
            CheckedUnitStructuralPathSegment::Field("left".into()),
        ];
        assert_unpinned(
            emission
                .emit_move(&receiver, &checked_place(&indexed, CELL))
                .unwrap_err(),
            "self[0].left",
            "index or referent",
        );
        let referent = vec![
            CheckedUnitStructuralPathSegment::Referent,
            CheckedUnitStructuralPathSegment::Field("left".into()),
        ];
        assert_unpinned(
            emission
                .emit_move(&receiver, &checked_place(&referent, CELL))
                .unwrap_err(),
            "self.*.left",
            "index or referent",
        );
        assert_unpinned(
            emission
                .emit_move(&receiver, &checked_place(&fields(&["count"]), CELL))
                .unwrap_err(),
            "self.count",
            "not a structural subtree",
        );
        assert_unpinned(
            emission
                .emit_move(&receiver, &checked_place(&fields(&["absent"]), CELL))
                .unwrap_err(),
            "self.absent",
            "does not resolve",
        );
        assert_unpinned(
            emission
                .emit_move(&receiver, &checked_place(&fields(&["left"]), ENVELOPE))
                .unwrap_err(),
            "self.left",
            "checked place type differs",
        );
        let shared_take = CheckedUnitStructuralArgumentPlan {
            access: CheckedStructuralAccess::SharedBorrow,
            ..checked_place(&fields(&["left"]), CELL)
        };
        assert_unpinned(
            emission.emit_move(&receiver, &shared_take).unwrap_err(),
            "self.left",
            "not taken as an owned value",
        );
        let mut qualified = envelope_receiver();
        qualified[0].multiplicity = StructuralMultiplicity::Linear;
        assert_unpinned(
            emission
                .emit_move(&qualified, &checked_place(&fields(&["left"]), CELL))
                .unwrap_err(),
            "self.left",
            "linear custody",
        );
        let binding_mismatch = emission.harness.emit_move(
            &mut emission.ledger,
            &checked_place(&fields(&["left"]), CELL),
            &CheckedUnitStructuralResultBindingPlan {
                type_identity: ENVELOPE.into(),
                ..cell_binding()
            },
            &receiver,
            &mut emission.next_place,
            &mut emission.operations,
        );
        assert_unpinned(
            binding_mismatch.unwrap_err(),
            "self.left",
            "moved binding's type differs",
        );
        assert!(emission.operations.operations.is_empty());
    }

    /// Whole-value assignment through `&mut` is a root window: no field, an
    /// empty path, the hole the loan's whole referent. It overlaps every
    /// place beneath the root until the repair.
    #[test]
    fn a_root_window_vacates_and_reseats_the_whole_referent_of_its_loan() {
        let receiver = envelope_receiver();
        let mut emission = Emission::new();
        let envelope_binding = CheckedUnitStructuralResultBindingPlan {
            type_identity: ENVELOPE.into(),
            ..cell_binding()
        };
        let displaced = emission
            .harness
            .emit_move(
                &mut emission.ledger,
                &checked_place(&[], ENVELOPE),
                &envelope_binding,
                &receiver,
                &mut emission.next_place,
                &mut emission.operations,
            )
            .expect("the loan's whole referent opens a root window");
        assert!(matches!(
            &emission.operations.operations[0].kind,
            OperationKind::MoveStructuralField { path, field: None, .. } if path.is_empty()
        ));
        assert_unpinned(
            emission
                .emit_move(&receiver, &checked_place(&fields(&["left"]), CELL))
                .unwrap_err(),
            "self.left",
            "overlaps an open window",
        );
        emission
            .emit_store(
                &receiver,
                &checked_place(&[], ENVELOPE),
                BorrowedWindowRepairValue {
                    place: displaced,
                    structural_type: envelope(),
                },
            )
            .expect("the repair reseats the whole referent");
        emission
            .ledger
            .require_closed()
            .expect("root window closed");
        assert!(matches!(
            &emission.operations.operations[1].kind,
            OperationKind::StoreStructuralField { path, field: None, .. } if path.is_empty()
        ));
        terminal_verifier::validate_module(&module(&emission, &receiver))
            .expect("the root window pair verifies");
    }

    #[test]
    fn extraction_over_an_open_window_rejects() {
        let crate_receiver = vec![parameter(crate_type(), StructuralAccess::MutableBorrow)];
        let mut emission = Emission::new();
        emission
            .emit_move(
                &crate_receiver,
                &checked_place(&fields(&["inner", "left"]), CELL),
            )
            .expect("open inner.left");
        assert_unpinned(
            emission
                .emit_move(
                    &crate_receiver,
                    &checked_place(&fields(&["inner", "left"]), CELL),
                )
                .unwrap_err(),
            "self.inner.left",
            "repeated extraction",
        );
        let ancestor = CheckedUnitStructuralArgumentPlan {
            type_identity: ENVELOPE.into(),
            ..checked_place(&fields(&["inner"]), CELL)
        };
        let error = emission
            .harness
            .emit_move(
                &mut emission.ledger,
                &ancestor,
                &CheckedUnitStructuralResultBindingPlan {
                    type_identity: ENVELOPE.into(),
                    ..cell_binding()
                },
                &crate_receiver,
                &mut emission.next_place,
                &mut emission.operations,
            )
            .unwrap_err();
        assert_unpinned(error, "self.inner", "overlaps an open window");
        assert_eq!(emission.operations.operations.len(), 1);
    }

    #[test]
    fn a_store_that_does_not_name_the_open_hole_exactly_rejects() {
        let receiver = envelope_receiver();
        let mut emission = Emission::new();
        let value = BorrowedWindowRepairValue {
            place: id(9, PlaceId::new),
            structural_type: cell(),
        };
        assert_unpinned(
            emission
                .emit_store(&receiver, &checked_place(&fields(&["left"]), CELL), value)
                .unwrap_err(),
            "self.left",
            "no open window",
        );
        let moved = emission
            .emit_move(&receiver, &checked_place(&fields(&["left"]), CELL))
            .expect("open left");
        assert_unpinned(
            emission
                .emit_store(&receiver, &checked_place(&fields(&["right"]), CELL), value)
                .unwrap_err(),
            "self.right",
            "no open window",
        );
        let crate_receiver = vec![parameter(crate_type(), StructuralAccess::MutableBorrow)];
        let mut nested = Emission::new();
        nested
            .emit_move(
                &crate_receiver,
                &checked_place(&fields(&["inner", "left"]), CELL),
            )
            .expect("open inner.left");
        let whole_inner = CheckedUnitStructuralArgumentPlan {
            type_identity: ENVELOPE.into(),
            ..checked_place(&fields(&["inner"]), CELL)
        };
        assert_unpinned(
            nested
                .emit_store(
                    &crate_receiver,
                    &whole_inner,
                    BorrowedWindowRepairValue {
                        place: id(9, PlaceId::new),
                        structural_type: envelope(),
                    },
                )
                .unwrap_err(),
            "self.inner",
            "without naming its exact place",
        );
        assert_unpinned(
            emission
                .emit_store(
                    &receiver,
                    &checked_place(&fields(&["left"]), CELL),
                    BorrowedWindowRepairValue {
                        place: moved,
                        structural_type: envelope(),
                    },
                )
                .unwrap_err(),
            "self.left",
            "repair value's type differs",
        );
        assert_eq!(emission.ledger.open_windows().len(), 1);
        assert_eq!(emission.operations.operations.len(), 1);
    }

    #[test]
    fn a_move_without_its_store_is_rejected_by_terminal_verification_too() {
        let receiver = envelope_receiver();
        let mut emission = Emission::new();
        emission
            .emit_move(&receiver, &checked_place(&fields(&["left"]), CELL))
            .expect("open");
        // The ledger refuses the exit; independent verification refuses the
        // same module without consulting the ledger.
        assert!(emission.ledger.require_closed().is_err());
        assert!(terminal_verifier::validate_module(&module(&emission, &receiver)).is_err());
    }

    #[test]
    fn transformed_owned_values_repair_holes_without_displacing_absent_content() {
        for repair in [
            "self.inventory = move taken.bump();",
            "let updated: Inventory = taken.bump(); self.inventory = move updated;",
        ] {
            let source = format!(
                "data Inventory {{ slots: i32; }}
                 machine Inventory::bump(self) -> Inventory {{
                     Inventory {{ slots: ((self.slots as i32 in Wrapping) + 1) as i32 }}
                 }}
                 data Main {{ inventory: Inventory; sibling: i32; }}
                 machine Main::replace(&mut self) {{
                     let taken: Inventory = self.inventory;
                     self.sibling = 9;
                     {repair}
                 }}"
            );
            let checked = crate::front_end::checked_program(&source);
            let lowered =
                crate::lower_machine(&checked, TerminalMachineSelection::Name("Main::replace"))
                    .expect("ordinary call custody and exact-place restoration compose");
            let profile = proof_admission::AdmissionProfile::default();
            terminal_verifier::verify_module(
                &lowered.semantic_module,
                &lowered.proof_bundle,
                &profile,
            )
            .expect("transformed replacement independently verifies");
            let moves = lowered
                .semantic_module
                .machines
                .iter()
                .flat_map(|machine| &machine.blocks)
                .flat_map(|block| &block.operations)
                .filter(|operation| {
                    matches!(operation.kind, OperationKind::MoveStructuralField { .. })
                })
                .count();
            assert_eq!(
                moves, 1,
                "repair cannot extract absent content a second time"
            );
            let mut unclosed = lowered.semantic_module.clone();
            for block in unclosed
                .machines
                .iter_mut()
                .flat_map(|machine| &mut machine.blocks)
            {
                block.operations.retain(|operation| {
                    !matches!(operation.kind, OperationKind::StoreStructuralField { .. })
                });
            }
            assert!(
                terminal_verifier::verify_module(&unclosed, &lowered.proof_bundle, &profile,)
                    .is_err(),
                "consuming the detached value does not repair its original place"
            );
            let missing_repair = source.replace(repair, "");
            assert!(crate::front_end::checked_program_result(&missing_repair).is_err());
            let unchecked_arithmetic = source.replace(
                "((self.slots as i32 in Wrapping) + 1) as i32",
                "self.slots + 1",
            );
            let checked = crate::front_end::checked_program(&unchecked_arithmetic);
            assert!(matches!(
                crate::lower_machine(&checked, TerminalMachineSelection::Name("Main::replace")),
                Err(crate::LoweringError::OperationProofUnavailable(_))
            ));
        }
    }

    /// The guide canary's body (`pass/ownership/move_keyword_field_assignment`)
    /// plans as a borrowed-window pair: the checked move-out/restore rows route
    /// through `BorrowedWindowLedger`, the emitted machine carries the Move and
    /// Store operation pair, and independent module verification accepts the
    /// closure.
    #[test]
    fn the_guide_canary_body_routes_move_out_and_restore_through_the_ledger() {
        let source = r#"
            data Inventory {
                slots: i32;
            }

            data Main {
                inventory: Inventory;
            }

            machine Main::main(&mut self) {
                let replacement: Inventory = self.inventory;
                self.inventory = move replacement;
            }
        "#;
        let checked = crate::front_end::checked_program(source);
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str().ends_with("main"))
            .expect("Main::main");
        let plans = &checked.facts.flow.terminal_unit_effects;
        let plan = plans
            .for_machine(machine.symbol)
            .expect("the move-out/restore pair plans");
        assert!(plan.operations.iter().any(|operation| {
            matches!(
                operation,
                checked_trees::CheckedUnitEffectOperationPlan::MoveStructuralField { .. }
            )
        }));
        assert!(plan.operations.iter().any(|operation| {
            matches!(
                operation,
                checked_trees::CheckedUnitEffectOperationPlan::StoreStructuralField { .. }
            )
        }));
        let lowered = crate::lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
            .expect("lowers through the ledger");
        let emitted_kinds: Vec<&OperationKind> = lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|terminal| &terminal.blocks)
            .flat_map(|block| &block.operations)
            .map(|operation| &operation.kind)
            .collect();
        assert!(
            emitted_kinds
                .iter()
                .any(|kind| { matches!(kind, OperationKind::MoveStructuralField { .. }) })
        );
        assert!(
            emitted_kinds
                .iter()
                .any(|kind| { matches!(kind, OperationKind::StoreStructuralField { .. }) })
        );
        terminal_verifier::validate_module(&lowered.semantic_module)
            .expect("the emitted module verifies");
    }
}
