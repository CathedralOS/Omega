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
//! — a root that is not a mutable-borrowed parameter, a whole-root or
//! indexed/referent route, a scalar or ambiguous field, a moved type that
//! differs from the declared field, extraction over an open hole, a repair at
//! another place, a repair value that is not a whole owned value of the hole's
//! type, an open hole at an exit, or reconverging paths that disagree on the
//! open holes — is a `LoweringError::UnpinnedBorrowedStorageWindow` naming
//! the place. Terminal verification reconstructs the same debt independently
//! (`terminal-verifier/src/validation/borrowed_windows.rs`); nothing here is
//! evidence for it.
//!
//! Boundary: the checked Unit plan vocabulary
//! (`CheckedUnitEffectOperationPlan`) has no row for a move out of borrowed
//! storage or for a whole structural field store, and the checked stage omits
//! such a body at local construction, so no plan family calls these emitters
//! yet. They own the Terminal spelling; the plan rows and the checked-stage
//! producer are the missing dependencies named on BORROWED-STORAGE-RESTORATION.

use super::{
    CheckedUnitStructuralPathSegment, LoweringError, Operation, OperationKind, OperationResult,
    PlaceId, StructuralAccess, StructuralFieldId, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeId, allocate_dense, place_id,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::structural_scalar_store::lower_structural_field_path;
use crate::terminal_identities::lookup_type_id;
use checked_trees::{
    CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralResultBindingPlan,
};
use language_semantics::Multiplicity;
use terminal_psi::{StructuralArgument, StructuralOperationResult};

/// One open restoration debt: the exact hole beneath a mutable-borrowed
/// parameter root and the structural result place now holding the removed
/// value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpenBorrowedWindow {
    pub(crate) source: PlaceId,
    pub(crate) path: Vec<StructuralPathSegment>,
    pub(crate) field: StructuralFieldId,
    pub(crate) hole_type: StructuralTypeId,
    pub(crate) moved: PlaceId,
    /// `path` extended by the field's own spelled segment: the exact place
    /// as a Terminal route, for overlap checks against later routes.
    full_path: Vec<StructuralPathSegment>,
    spelling: String,
}

impl OpenBorrowedWindow {
    /// Frontier identity of the hole: the exact place (root and spelled
    /// route) plus its declared type. `moved` is the path-local result
    /// binding of that arm's extraction, so it is excluded — reconverging
    /// arms may each have moved the same field under different bindings and
    /// still carry one agreeing hole.
    fn same_hole(&self, other: &Self) -> bool {
        self.source == other.source
            && self.full_path == other.full_path
            && self.field == other.field
            && self.hole_type == other.hole_type
    }
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

/// The resolved exact place a move or repair names beneath a parameter root.
struct ResolvedWindowPlace {
    source: PlaceId,
    path: Vec<StructuralPathSegment>,
    field: StructuralFieldId,
    full_path: Vec<StructuralPathSegment>,
    hole_type: StructuralTypeId,
    spelling: String,
}

impl BorrowedWindowLedger {
    pub(crate) fn open_windows(&self) -> &[OpenBorrowedWindow] {
        &self.open
    }

    /// Emit `MoveStructuralField` for `moved`, the checked owned place a
    /// consuming move takes out of a mutable-borrowed parameter, binding the
    /// removed value as `result`. Returns the structural result place.
    pub(crate) fn emit_move(
        &mut self,
        moved: &CheckedUnitStructuralArgumentPlan,
        result: &CheckedUnitStructuralResultBindingPlan,
        parameters: &[StructuralParameterDeclaration],
        structural_types: &[StructuralTypeDeclaration],
        type_ids: &[(String, StructuralTypeId)],
        next_place: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Result<PlaceId, LoweringError> {
        let place = resolve_window_place(moved, parameters, structural_types, type_ids)?;
        if lookup_type_id(type_ids, &result.type_identity)? != place.hole_type {
            return Err(unpinned(
                &place.spelling,
                "the moved binding's type differs from the declared field type",
            ));
        }
        if let Some(absent) = self.open.iter().find(|absent| {
            absent.source == place.source && paths_overlap(&absent.full_path, &place.full_path)
        }) {
            return Err(unpinned(
                &place.spelling,
                if absent.full_path == place.full_path {
                    "the place is already absent: repeated extraction of one open window"
                } else {
                    "the place overlaps an open window: extraction through absent storage"
                },
            ));
        }
        let moved_place = place_id(allocate_dense(next_place)?);
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
                path: place.path.clone(),
                field: place.field,
            },
        });
        self.open.push(OpenBorrowedWindow {
            source: place.source,
            path: place.path,
            field: place.field,
            hole_type: place.hole_type,
            moved: moved_place,
            full_path: place.full_path,
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
        operations: &mut OperationBuffer,
    ) -> Result<(), LoweringError> {
        let place = resolve_window_place(destination, parameters, structural_types, type_ids)?;
        let Some(index) = self.open.iter().position(|absent| {
            absent.source == place.source && absent.full_path == place.full_path
        }) else {
            let reason = if self.open.iter().any(|absent| {
                absent.source == place.source && paths_overlap(&absent.full_path, &place.full_path)
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
        let id = operations.allocate();
        operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id,
            result: OperationResult::Unit,
            kind: OperationKind::StoreStructuralField {
                destination: place.source,
                path: place.path,
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

    /// Reconverging paths must agree on the exact open holes; one repaired
    /// path does not repair another. The frontier is hole identity — root,
    /// spelled route, declared type — so sibling paths that each opened the
    /// same place agree even though their `moved` result bindings are
    /// arm-local: on the joined edge the shared hole is one open window,
    /// and `emit_store` never consults `moved` when reseating it.
    pub(crate) fn require_same_frontier(&self, other: &Self) -> Result<(), LoweringError> {
        let disagreeing = self
            .open
            .iter()
            .find(|absent| !other.open.iter().any(|open| open.same_hole(absent)))
            .or_else(|| {
                other
                    .open
                    .iter()
                    .find(|absent| !self.open.iter().any(|open| open.same_hole(absent)))
            });
        match disagreeing {
            None => Ok(()),
            Some(absent) => Err(unpinned(
                &absent.spelling,
                "reconverging paths disagree on the open window",
            )),
        }
    }

    /// The ledger a reconverged block continues with: the incoming
    /// frontiers must agree on the exact open holes, so the shared holes
    /// collapse to one open window each. Frontier identity excludes the
    /// arm-local `moved` binding, and `emit_store` never consults `moved`
    /// when reseating, so the joined frontier keeps this side's rows
    /// verbatim. A driver joining more than two edges folds this pairwise.
    pub(crate) fn joined(&self, other: &Self) -> Result<Self, LoweringError> {
        self.require_same_frontier(other)?;
        Ok(self.clone())
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
/// an ancestor of it.
fn paths_overlap(first: &[StructuralPathSegment], second: &[StructuralPathSegment]) -> bool {
    first.starts_with(second) || second.starts_with(first)
}

fn resolve_window_place(
    plan: &CheckedUnitStructuralArgumentPlan,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
    type_ids: &[(String, StructuralTypeId)],
) -> Result<ResolvedWindowPlace, LoweringError> {
    let spelling = spell_place(plan, parameters);
    let CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } = plan.source else {
        return Err(unpinned(
            &spelling,
            "borrowed-storage windows open only beneath a mutable-borrowed parameter root",
        ));
    };
    let parameter = parameters
        .iter()
        .find(|parameter| parameter.position == parameter_index)
        .ok_or_else(|| unpinned(&spelling, "the parameter root is not declared"))?;
    if parameter.access != StructuralAccess::MutableBorrow {
        return Err(unpinned(
            &spelling,
            "the root is not an exclusive `&mut` borrow: shared and write-only loans admit \
             no extraction and owned roots use ordinary partial moves",
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
            "the root carries qualifications or linear custody the window does not restore",
        ));
    }
    if plan.access != CheckedStructuralAccess::Owned {
        return Err(unpinned(
            &spelling,
            "the place is not taken as an owned value",
        ));
    }
    let Some((CheckedUnitStructuralPathSegment::Field(field_identity), carrier_path)) =
        plan.path.split_last()
    else {
        return Err(unpinned(
            &spelling,
            "the route is not an exact field beneath the root: whole-root, indexed and \
             referent routes are not pinned windows",
        ));
    };
    if !carrier_path
        .iter()
        .all(|segment| matches!(segment, CheckedUnitStructuralPathSegment::Field(_)))
    {
        return Err(unpinned(
            &spelling,
            "the route crosses an index or referent: only exact field chains are pinned",
        ));
    }
    let (path, field) = lower_structural_field_path(
        parameter.structural_type,
        carrier_path,
        field_identity,
        structural_types,
    )
    .map_err(|_| {
        unpinned(
            &spelling,
            "the field chain does not resolve through the declared records",
        )
    })?;
    let StructuralFieldType::Structural(hole_type) = field.field_type else {
        return Err(unpinned(
            &spelling,
            "the field is not a structural subtree: scalar and byte fields open no window",
        ));
    };
    if lookup_type_id(type_ids, &plan.type_identity)? != hole_type {
        return Err(unpinned(
            &spelling,
            "the checked place type differs from the declared field type",
        ));
    }
    let mut full_path = path.clone();
    full_path.push(StructuralPathSegment::Field(field.identity.clone()));
    Ok(ResolvedWindowPlace {
        source: parameter.place,
        path,
        field: field.id,
        full_path,
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
        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
            match parameters
                .iter()
                .find(|parameter| parameter.position == parameter_index)
            {
                Some(parameter) if parameter.is_self => "self".to_owned(),
                _ => format!("parameter#{parameter_index}"),
            }
        }
        CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { .. }
        | CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. } => "local".to_owned(),
        CheckedUnitStructuralArgumentSourcePlan::TrivialAffineLocal {
            declaration_ordinal,
        } => format!("local#{declaration_ordinal}"),
        CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
            format!("result#{binding_ordinal}")
        }
        CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { .. } => "literal".to_owned(),
        CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
            parameter_index, ..
        } => format!("parameter#{parameter_index}[..]"),
    };
    for segment in &plan.path {
        match segment {
            CheckedUnitStructuralPathSegment::Field(identity) => {
                spelling.push('.');
                spelling.push_str(identity);
            }
            CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                spelling.push_str(&format!("[{index}]"));
            }
            CheckedUnitStructuralPathSegment::Referent => spelling.push_str(".*"),
        }
    }
    spelling
}

#[cfg(test)]
mod tests {
    //! The emitter is exercised on Terminal declarations directly, and the
    //! emitted pair is replayed through `terminal_verifier::validate_module`
    //! so the spelling is the one independent verification reconstructs.

    use super::{BorrowedWindowLedger, BorrowedWindowRepairValue};
    use crate::emission::operation_emission::buffer::OperationBuffer;
    use crate::lowering_error::LoweringError;
    use checked_trees::{
        CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
        CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
        CheckedUnitStructuralResultBindingPlan,
    };
    use language_semantics::Multiplicity;
    use semantic_vocabulary::{
        BlockId, ContractId, EdgeId, MachineId, OperationId, PlaceId, ScalarType,
        StructuralFieldId, StructuralPlaceKind, StructuralTypeId,
    };
    use terminal_psi::{
        BindingRelevance, Block, MachineContract, OperationKind, OperationResult, StructuralAccess,
        StructuralArgument, StructuralFieldDeclaration, StructuralFieldType,
        StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
        StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
        TerminalMachine, TerminalMachineResult, TerminalModule, Terminator, VocabularyMarker,
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

    struct Emission {
        ledger: BorrowedWindowLedger,
        operations: OperationBuffer,
        next_place: u64,
    }

    impl Emission {
        fn new() -> Self {
            Self {
                ledger: BorrowedWindowLedger::default(),
                operations: OperationBuffer::new(0),
                // Place 1 is the parameter root; results follow it.
                next_place: 2,
            }
        }

        fn emit_move(
            &mut self,
            parameters: &[StructuralParameterDeclaration],
            moved: &CheckedUnitStructuralArgumentPlan,
        ) -> Result<PlaceId, LoweringError> {
            self.ledger.emit_move(
                moved,
                &cell_binding(),
                parameters,
                &structural_types(),
                &type_ids(),
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
            self.ledger.emit_store(
                destination,
                value,
                parameters,
                &structural_types(),
                &type_ids(),
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
    fn move_then_restore_spells_the_pair_on_one_exact_place_and_verifies() {
        let receiver = envelope_receiver();
        let mut emission = Emission::new();
        let moved = emission
            .emit_move(&receiver, &checked_place(&fields(&["left"]), CELL))
            .expect("the move opens a window");
        assert_eq!(emission.ledger.open_windows().len(), 1);
        assert_eq!(emission.ledger.open_windows()[0].moved, moved);
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
                    place: moved,
                    structural_type: cell(),
                },
            )
            .expect("the store closes the window");
        assert!(emission.ledger.open_windows().is_empty());
        emission
            .ledger
            .require_closed()
            .expect("closed at the exit");

        let kinds = emission
            .operations
            .operations
            .iter()
            .map(|operation| operation.kind.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            vec![
                OperationKind::MoveStructuralField {
                    source: id(1, PlaceId::new),
                    path: Vec::new(),
                    field: left(),
                },
                OperationKind::StoreStructuralField {
                    destination: id(1, PlaceId::new),
                    path: Vec::new(),
                    field: left(),
                    value: StructuralArgument {
                        place: moved,
                        path: Vec::new(),
                        access: StructuralAccess::Owned,
                    },
                },
            ]
        );
        assert_eq!(
            emission.operations.operations[0].id,
            id(1, OperationId::new)
        );
        let OperationResult::Structural(result) = &emission.operations.operations[0].result else {
            panic!("the move binds a structural result");
        };
        assert_eq!(result.structural_type, cell());
        assert_eq!(result.multiplicity, StructuralMultiplicity::Unrestricted);
        terminal_verifier::validate_module(&module(&emission, &receiver))
            .expect("the emitted pair is the window Terminal verification reconstructs");
    }

    #[test]
    fn a_nested_field_chain_lowers_to_the_carrier_path_and_verifies() {
        let receiver = vec![parameter(crate_type(), StructuralAccess::MutableBorrow)];
        let mut emission = Emission::new();
        let moved = emission
            .emit_move(
                &receiver,
                &checked_place(&fields(&["inner", "right"]), CELL),
            )
            .expect("the nested move opens a window");
        emission
            .emit_store(
                &receiver,
                &checked_place(&fields(&["inner", "right"]), CELL),
                BorrowedWindowRepairValue {
                    place: moved,
                    structural_type: cell(),
                },
            )
            .expect("the nested store closes the window");
        let carrier = vec![StructuralPathSegment::Field("inner".into())];
        assert!(matches!(
            &emission.operations.operations[0].kind,
            OperationKind::MoveStructuralField { source, path, field }
                if *source == id(1, PlaceId::new) && *path == carrier && *field == right()
        ));
        assert!(matches!(
            &emission.operations.operations[1].kind,
            OperationKind::StoreStructuralField { destination, path, field, .. }
                if *destination == id(1, PlaceId::new) && *path == carrier && *field == right()
        ));
        terminal_verifier::validate_module(&module(&emission, &receiver))
            .expect("nested pair verifies");
    }

    #[test]
    fn a_different_whole_owned_value_of_the_hole_type_restores() {
        let receiver = envelope_receiver();
        let mut emission = Emission::new();
        emission
            .emit_move(&receiver, &checked_place(&fields(&["left"]), CELL))
            .expect("open");
        let replacement = id(7, PlaceId::new);
        emission
            .emit_store(
                &receiver,
                &checked_place(&fields(&["left"]), CELL),
                BorrowedWindowRepairValue {
                    place: replacement,
                    structural_type: cell(),
                },
            )
            .expect("any whole owned Cell restores the hole");
        assert!(matches!(
            &emission.operations.operations[1].kind,
            OperationKind::StoreStructuralField { value, .. } if value.place == replacement
        ));
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
            "mutable-borrowed parameter root",
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
        assert_unpinned(
            emission
                .emit_move(&receiver, &checked_place(&[], ENVELOPE))
                .unwrap_err(),
            "self",
            "whole-root",
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
        let binding_mismatch = emission.ledger.emit_move(
            &checked_place(&fields(&["left"]), CELL),
            &CheckedUnitStructuralResultBindingPlan {
                type_identity: ENVELOPE.into(),
                ..cell_binding()
            },
            &receiver,
            &structural_types(),
            &type_ids(),
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
            .ledger
            .emit_move(
                &ancestor,
                &CheckedUnitStructuralResultBindingPlan {
                    type_identity: ENVELOPE.into(),
                    ..cell_binding()
                },
                &crate_receiver,
                &structural_types(),
                &type_ids(),
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
    fn reconverging_paths_must_agree_on_the_open_window() {
        let receiver = envelope_receiver();
        let mut repaired = Emission::new();
        let moved = repaired
            .emit_move(&receiver, &checked_place(&fields(&["left"]), CELL))
            .expect("open");
        let mut unrepaired = repaired.ledger.clone();
        repaired
            .emit_store(
                &receiver,
                &checked_place(&fields(&["left"]), CELL),
                BorrowedWindowRepairValue {
                    place: moved,
                    structural_type: cell(),
                },
            )
            .expect("close on one path");
        assert_unpinned(
            repaired
                .ledger
                .require_same_frontier(&unrepaired)
                .unwrap_err(),
            "self.left",
            "reconverging paths disagree",
        );
        assert_unpinned(
            unrepaired
                .require_same_frontier(&repaired.ledger)
                .unwrap_err(),
            "self.left",
            "reconverging paths disagree",
        );
        unrepaired
            .emit_store(
                &checked_place(&fields(&["left"]), CELL),
                BorrowedWindowRepairValue {
                    place: moved,
                    structural_type: cell(),
                },
                &receiver,
                &structural_types(),
                &type_ids(),
                &mut OperationBuffer::new(0),
            )
            .expect("close on the other path");
        repaired
            .ledger
            .require_same_frontier(&unrepaired)
            .expect("both paths closed");
    }

    #[test]
    fn sibling_paths_opening_the_same_hole_agree_at_the_join() {
        let receiver = envelope_receiver();
        let mut left_arm = Emission::new();
        let left_moved = left_arm
            .emit_move(&receiver, &checked_place(&fields(&["left"]), CELL))
            .expect("open on the first path");
        let mut right_arm = Emission::new();
        right_arm.next_place = 3;
        let right_moved = right_arm
            .emit_move(&receiver, &checked_place(&fields(&["left"]), CELL))
            .expect("open on the second path");
        // Each arm bound its own result place for the removed value; the
        // frontier is the shared hole, not the per-arm binding.
        assert_ne!(left_moved, right_moved);
        left_arm
            .ledger
            .require_same_frontier(&right_arm.ledger)
            .expect("same hole opened on both paths agrees");
        right_arm
            .ledger
            .require_same_frontier(&left_arm.ledger)
            .expect("agreement is symmetric");

        // A hole opened on one arm only still disagrees with a path that
        // never opened it, and with a path that opened a different hole.
        let mut untouched = Emission::new();
        assert_unpinned(
            left_arm
                .ledger
                .require_same_frontier(&untouched.ledger)
                .unwrap_err(),
            "self.left",
            "reconverging paths disagree",
        );
        untouched
            .emit_move(&receiver, &checked_place(&fields(&["right"]), CELL))
            .expect("open the sibling field instead");
        assert_unpinned(
            left_arm
                .ledger
                .require_same_frontier(&untouched.ledger)
                .unwrap_err(),
            "self.left",
            "reconverging paths disagree",
        );
    }

    #[test]
    fn a_joined_frontier_collapses_arm_local_bindings_to_one_open_window() {
        let receiver = envelope_receiver();
        let mut left_arm = Emission::new();
        left_arm
            .emit_move(&receiver, &checked_place(&fields(&["left"]), CELL))
            .expect("open on the first path");
        let mut right_arm = Emission::new();
        right_arm.next_place = 3;
        right_arm
            .emit_move(&receiver, &checked_place(&fields(&["left"]), CELL))
            .expect("open on the second path");

        let mut joined = left_arm
            .ledger
            .joined(&right_arm.ledger)
            .expect("same hole opened on both paths joins");
        assert_eq!(joined.open_windows().len(), 1);

        // The hole stays open on the joined path: extracting it again
        // still rejects, and reseating names the exact place — neither
        // arm-local `moved` binding is consulted.
        let mut extraction = joined.clone();
        // The overlap refusal fires before the place counter is read.
        let mut next_place = 8;
        assert_unpinned(
            extraction
                .emit_move(
                    &checked_place(&fields(&["left"]), CELL),
                    &cell_binding(),
                    &receiver,
                    &structural_types(),
                    &type_ids(),
                    &mut next_place,
                    &mut OperationBuffer::new(0),
                )
                .unwrap_err(),
            "self.left",
            "already absent",
        );
        joined
            .emit_store(
                &checked_place(&fields(&["left"]), CELL),
                BorrowedWindowRepairValue {
                    place: joined.open_windows()[0].moved,
                    structural_type: cell(),
                },
                &receiver,
                &structural_types(),
                &type_ids(),
                &mut OperationBuffer::new(0),
            )
            .expect("the joined frontier's shared hole reseats");
        joined
            .require_closed()
            .expect("closed once the shared hole is reseated");
    }

    #[test]
    fn a_joined_frontier_rejects_when_paths_disagree() {
        let receiver = envelope_receiver();
        let mut opened = Emission::new();
        opened
            .emit_move(&receiver, &checked_place(&fields(&["left"]), CELL))
            .expect("open");
        let closed = Emission::new();
        assert_unpinned(
            opened.ledger.joined(&closed.ledger).unwrap_err(),
            "self.left",
            "reconverging paths disagree",
        );
        assert_unpinned(
            closed.ledger.joined(&opened.ledger).unwrap_err(),
            "self.left",
            "reconverging paths disagree",
        );
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
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .expect("tokenize");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .expect("resolve");
        let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type");
        let checked = typed_trees_to_checked_trees::lower_typed_trees(
            typed,
            &typed_trees_to_checked_trees::CheckingRequest::settled(),
        )
        .expect("the borrowed window checks");
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
        let lowered =
            crate::lower_machine(&checked, "Main::main").expect("lowers through the ledger");
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

    /// A route spelled through a `&mut` alias local names the same storage
    /// hole as the owner path: the checker keys the window on the resolved
    /// place, so `let r = &mut self; let x = r.f; r.f = move x` plans and
    /// emits the same Move/Store pair as the direct spelling.
    #[test]
    fn the_reborrow_alias_route_routes_move_out_and_restore_through_the_ledger() {
        let source = r#"
            data Inventory {
                slots: i32;
            }

            data Main {
                inventory: Inventory;
            }

            machine Main::main(&mut self) {
                let view: &mut Main = &mut self;
                let replacement: Inventory = view.inventory;
                view.inventory = move replacement;
            }
        "#;
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .expect("tokenize");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .expect("resolve");
        let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type");
        let checked = typed_trees_to_checked_trees::lower_typed_trees(
            typed,
            &typed_trees_to_checked_trees::CheckingRequest::settled(),
        )
        .expect("the borrowed window checks");
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str().ends_with("main"))
            .expect("Main::main");
        let plans = &checked.facts.flow.terminal_unit_effects;
        let plan = plans
            .for_machine(machine.symbol)
            .expect("the reborrowed move-out/restore pair plans");
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
        let lowered =
            crate::lower_machine(&checked, "Main::main").expect("lowers through the ledger");
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
