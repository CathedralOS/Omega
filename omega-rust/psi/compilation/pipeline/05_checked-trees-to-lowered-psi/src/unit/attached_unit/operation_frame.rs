//! One emitter for the Unit effect operations every attached body lowers the
//! same way: port writes, primitive, indexed, scalar-field and byte-sequence
//! stores, scalar locals, view-subslice locals, borrowed-storage windows,
//! call-continuation cleanup, structural-value construction, and every call
//! shape (`calls`: internal Unit and structural calls, member calls of a
//! construction and scalar calls; `boundary_calls`: the three boundary call
//! shapes). The ordinary machine (`ordinary_machine::MachineEmission`) and a
//! composed-graph state (`composed_control/emission.rs::emit_call_operations`)
//! each build an `OperationFrame` over their per-state environment and hand it
//! the checked operation; `OperationFrame::emit` and `OperationFrame::emit_call`
//! dispatch to the one method for that kind, which delegates the lowering
//! itself to the shared `crate::emission::*` and `ordinary_calls` helpers.
//!
//! A frame borrows the parameters, structural type roster, evaluation, value
//! namespace, identity counters, operation buffer and scalar-call context of
//! the body it emits into. The scalar-call context's obligation counter is the
//! body's only call-obligation counter while a frame is open, so helpers that
//! take a bare `&mut u64` and helpers that take the context advance the same
//! identity space.
//!
//! Operand evaluation stays with each route's value and storage environment,
//! but both consume `argument_schedule` to interleave nested structural
//! producers with scalar operands at their authored positions. A composed
//! state's `literal_arguments` materializes literals and consumes any already
//! staged operands without evaluating them again. Both hand the completed
//! operands to `emit_call` as `CallInputs`; everything from
//! there on (target lookup, transfer validation, claim custody, requirement
//! obligations, crash substitution, result publication) is one code path.
//!
//! The two routes keep genuinely different representations, and each is
//! explicit here rather than hidden in parallel copies of each operation:
//!
//! - `StructuralResults`: the ordinary machine keeps a dense roster indexed by
//!   authored binding ordinal, with each row's return-discard custody; a
//!   composed state registers completed results in the operation buffer and a
//!   machine-wide place catalog. `StructuralResults::earlier` gives both the
//!   dense `(declaration, discard)` view call operands resolve against.
//! - `StructuralTypeRoster`: a body that owns its type roster may add the
//!   generated literal-view carrier; a composed callee borrowing the closure's
//!   published roster may not.
//! - `PrivatePlaces`: the ordinary machine keeps byte-sequence literal places
//!   and construction temporaries in two rosters; a composed body keeps both
//!   in its one private temporary roster.
//! - `ClaimBindings`: the ordinary machine's claim table grows when a linear
//!   boundary result mints a caller binding; a composed state reads the
//!   machine-wide table and cannot mint.
//!
//! Every other check either route applied runs for both. In particular a
//! borrowed-window move or repair must name a machine-parameter root the
//! sequence still holds (Terminal verification anchors windows on machine
//! parameters only), and a repair value must be a whole owned result binding.
//! A completed result can occupy an operation result or a continuation's block
//! parameter. The latter rejoins the registered binding's exact place and type;
//! its source construction need not manufacture another operation result.
//! What the routes still admit differently is decided before emission, by
//! each route's admission, not by a narrower copy of the emitter.

use super::argument_evaluation::Evaluation;
use super::bodies::UnitPlans;
use super::primitive_locals::{self, PrimitiveLocal};
use super::signatures::MachineSignature;
use super::view_ranges::ViewRangeSite;
use super::{parameters, reference_results};
use crate::emission::borrowed_window::{
    BorrowedWindowLedger, BorrowedWindowRepairValue, WindowEvaluation,
};
use crate::emission::indexed_write::{IndexedWriteLeaf, IndexedWriteTarget};
use crate::emission::operation_emission::buffer::{OperationBuffer, StructuralBinding};
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::expression_preparation::bindings::ScalarBindings;
use crate::expression_preparation::bindings::structural_paths::lower_structural_path;
use crate::scalar_graph::scalar_call_closure::callee::PreparedScalarCallee;
use crate::unit::{
    BoundaryMachineId, CheckedScalarExpressionRole, CheckedTrees, CheckedUnitEffectOperationPlan,
    ClaimId, LoweringError, Operation, OperationKind, OperationResult, PermissionClaimIdentity,
    PlaceId, ScalarType, ServiceId, ServiceReachId, ServiceReachSummary, StructuralAccess,
    StructuralArgument, StructuralDomainId, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeDeclaration, StructuralTypeId,
    ValueDeclaration, allocate_dense, claim_id, lookup_service_id, lookup_type_id, place_id,
    terminal_scalar_type, unsupported,
};
use checked_trees::{CheckedComposedUnitControlStatePlan, CheckedUnitStructuralArgumentPlan};
use std::borrow::Cow;

mod atomic_access;
mod boundary_calls;
mod calls;
pub(super) use calls::CallInputs;

/// One lowered boundary declaration a call can name.
#[derive(Debug, Clone)]
pub(crate) struct BoundaryParameters {
    /// The checked boundary machine calls name.
    pub(crate) source: symbols::SymbolHandle,
    /// The published boundary identity.
    pub(crate) id: BoundaryMachineId,
    pub(crate) structural: Vec<StructuralParameterDeclaration>,
    /// Dense scalar parameter types, in the declaration's scalar lane.
    pub(crate) scalar: Vec<ScalarType>,
    /// The declaration's `scalar_requires` rows; each call owes one
    /// obligation per row.
    pub(crate) requirement_count: usize,
}

/// The per-state environment one operation emits into. Each route builds a
/// frame per operation; the frame only borrows, so the route keeps ownership
/// of its namespaces, counters and buffer between operations.
pub(super) struct OperationFrame<'f, 'c> {
    pub(super) checked: &'f CheckedTrees,
    pub(super) machine: symbols::SymbolHandle,
    pub(super) state: symbols::SymbolHandle,
    /// The machine's scalar completion binding, whose initializer lowers in
    /// the `Return` role. A composed state completes without one.
    pub(super) scalar_result: Option<&'f checked_trees::CheckedUnitScalarResultBindingPlan>,
    /// Scalar formals ahead of the dense local namespace.
    pub(super) scalar_parameter_count: usize,
    /// Values the operation's authored source can name; staged argument
    /// temporaries past this count are private to their call.
    pub(super) source_value_count: usize,
    pub(super) parameters: &'f [StructuralParameterDeclaration],
    pub(super) structural_types: StructuralTypeRoster<'f>,
    pub(super) type_ids: &'f [(String, StructuralTypeId)],
    pub(super) service_ids: &'f [(ServiceReachId, ServiceId)],
    /// Primitive referents the route has established so far; each route
    /// keeps its own roster of `establish_primitive_local` results.
    pub(super) primitive_locals: &'f [PrimitiveLocal],
    pub(super) results: StructuralResults<'f>,
    pub(super) private_places: PrivatePlaces<'f>,
    /// Open borrowed-storage windows on the current straight-line sequence.
    pub(super) windows: &'f mut BorrowedWindowLedger,
    pub(super) evaluation: &'f mut Evaluation,
    pub(super) values: &'f mut Vec<ValueDeclaration>,
    pub(super) next_place: &'f mut u64,
    pub(super) next_value: &'f mut u64,
    pub(super) next_block: &'f mut u64,
    pub(super) next_edge: &'f mut u64,
    pub(super) calls: &'f mut CallEmissionContext<'c>,
    pub(super) operations: &'f mut OperationBuffer,
    pub(super) callees: Callees<'f>,
    pub(super) caller: CallerCustody<'f>,
}

/// The closure-wide targets a call resolves against. An ordinary machine and
/// a composed graph emitted in the same Unit closure name the same
/// signatures, boundaries and prepared scalar callees; a standalone composed
/// catalog (a dynamic continuation) supplies the subset its leaves can call.
#[derive(Clone, Copy)]
pub(super) struct Callees<'f> {
    /// The checked bodies and boundary plans the closure was admitted from.
    pub(super) plans: UnitPlans<'f>,
    /// Every Unit body's allocated formals, claims and `requires` roster;
    /// `MachineSignature::call_target` is what a call substitutes into.
    pub(super) signatures: &'f [MachineSignature],
    pub(super) boundaries: &'f [BoundaryParameters],
    pub(super) domain_ids: &'f [crate::terminal_identities::StructuralDomainBinding],
    /// The closure's Unit bodies: a scalar call may name one that completes
    /// with a scalar instead of a prepared scalar callee.
    pub(super) closure: &'f [symbols::SymbolHandle],
    pub(super) prepared_scalar_machines: &'f [PreparedScalarCallee<'f>],
}

/// What the calling body itself contributes to a call's custody.
pub(super) struct CallerCustody<'f> {
    /// The caller's erased scalar formals; erased actuals lower over them.
    pub(super) erased_scalar_parameters: &'f [ValueDeclaration],
    /// The caller's erased proof-only roster; `Formal` actuals resolve in it.
    pub(super) erased_proof_parameters: &'f [checked_trees::CheckedErasedProofParameterPlan],
    /// The claims the emitting body (ordinary) or state (composed) holds on
    /// entry. A boundary call expects one receipt per claim its parameter
    /// arguments carry.
    pub(super) entry_claims: &'f [checked_trees::CheckedUnitEntryClaimPlan],
    pub(super) claims: ClaimBindings<'f>,
    /// The ordinary machine's trivial affine locals; a composed state has none.
    pub(super) local_places: &'f [StructuralPlaceDeclaration],
}

/// The caller's claim namespace.
pub(super) enum ClaimBindings<'f> {
    /// The ordinary machine's table: its lowered entry claims plus every
    /// caller-local binding a linear boundary result mints for a claim
    /// established at its own binding statement. `next_claim` is the dense
    /// tail past the entry claims.
    Growable {
        bindings: &'f mut Vec<(PermissionClaimIdentity, ClaimId)>,
        next_claim: &'f mut u64,
    },
    /// A composed state's machine-wide table. The graph publishes its claim
    /// roster before any state emits, so a state cannot mint a binding.
    Fixed(&'f [(PermissionClaimIdentity, ClaimId)]),
}

impl ClaimBindings<'_> {
    pub(super) fn bindings(&self) -> &[(PermissionClaimIdentity, ClaimId)] {
        match self {
            Self::Growable { bindings, .. } => bindings,
            Self::Fixed(bindings) => bindings,
        }
    }

    /// Mint the caller binding for a claim this call's own statement
    /// established.
    fn mint(&mut self, identity: PermissionClaimIdentity) -> Result<ClaimId, LoweringError> {
        let Self::Growable {
            bindings,
            next_claim,
        } = self
        else {
            return unsupported("composed state cannot mint a boundary result claim binding");
        };
        let claim = claim_id(allocate_dense(next_claim)?);
        bindings.push((identity, claim));
        Ok(claim)
    }
}

/// The private places a body declares beside its authored results.
pub(super) enum PrivatePlaces<'f> {
    /// The ordinary machine keeps byte-sequence literal places (preallocated
    /// per call argument, then store literals) apart from the temporaries a
    /// structural-value construction declares.
    Split {
        literals: &'f mut Vec<StructuralPlaceDeclaration>,
        temporaries: &'f mut Vec<StructuralPlaceDeclaration>,
    },
    /// A composed body keeps literals, construction and join temporaries in
    /// one roster. A literal's ordinal counts only the literals in it.
    Shared(&'f mut Vec<StructuralPlaceDeclaration>),
}

impl PrivatePlaces<'_> {
    /// The roster a store's byte-sequence literal place joins.
    fn literals(&mut self) -> &mut Vec<StructuralPlaceDeclaration> {
        match self {
            Self::Split { literals, .. } => literals,
            Self::Shared(places) => places,
        }
    }

    /// The roster a structural-value construction's temporaries join.
    fn temporaries(&mut self) -> &mut Vec<StructuralPlaceDeclaration> {
        match self {
            Self::Split { temporaries, .. } => temporaries,
            Self::Shared(places) => places,
        }
    }
}

/// The structural type roster a frame reads.
pub(super) enum StructuralTypeRoster<'f> {
    /// A roster the emitting body owns; it may grow generated carriers.
    Owned(&'f mut Vec<StructuralTypeDeclaration>),
    /// The closure's published roster, borrowed by a shared callee.
    Published(&'f [StructuralTypeDeclaration]),
}

impl StructuralTypeRoster<'_> {
    fn declarations(&self) -> &[StructuralTypeDeclaration] {
        match self {
            Self::Owned(types) => types,
            Self::Published(types) => types,
        }
    }
}

/// Where the emitting body keeps its completed structural results.
pub(super) enum StructuralResults<'f> {
    /// The ordinary machine's roster: one row per authored result binding in
    /// ordinal order, with whether the machine's return discards it.
    Dense(&'f mut Vec<(StructuralPlaceDeclaration, bool)>),
    /// A composed state: each binding is registered in the operation buffer,
    /// and every result place joins the machine-wide place catalog.
    StateGraph {
        state: &'f CheckedComposedUnitControlStatePlan,
        places: &'f mut Vec<StructuralPlaceDeclaration>,
    },
}

impl StructuralResults<'_> {
    /// The place a call continuation discards for `binding_ordinal`.
    fn continuation_place(
        &self,
        binding_ordinal: u32,
        operations: &OperationBuffer,
    ) -> Result<PlaceId, LoweringError> {
        match self {
            Self::Dense(roster) => {
                let (place, discard_on_return) =
                    roster
                        .get(binding_ordinal as usize)
                        .ok_or(LoweringError::Unsupported(
                            "call continuation result has not been produced",
                        ))?;
                if *discard_on_return {
                    return unsupported("call continuation cleanup has conflicting custody");
                }
                if !matches!(place.kind, StructuralPlaceKind::OperationResult { .. }) {
                    return unsupported("call continuation cleanup has no operation result");
                }
                Ok(place.id)
            }
            Self::StateGraph { state, .. } => {
                let binding = super::composed_control::state_graph_result(
                    state,
                    binding_ordinal,
                    operations,
                )?;
                Ok(binding.operation_result()?.place)
            }
        }
    }

    /// The completed whole value a borrowed-window repair stores back.
    fn repair_value(
        &self,
        binding_ordinal: u32,
        operations: &OperationBuffer,
    ) -> Result<BorrowedWindowRepairValue, LoweringError> {
        match self {
            Self::Dense(roster) => {
                let (declaration, _) =
                    roster
                        .get(binding_ordinal as usize)
                        .ok_or(LoweringError::Unsupported(
                            "borrowed-window repair binding is absent",
                        ))?;
                let structural_type = match declaration.kind {
                    StructuralPlaceKind::OperationResult {
                        structural_type, ..
                    } => structural_type,
                    StructuralPlaceKind::BlockParameter { .. } => {
                        // A construction or selection can finish at a join,
                        // not at its last operation. That completed binding
                        // already owns the join's type and custody; rejoin it
                        // to the exact declared place rather than requiring a
                        // second operation to manufacture the same value.
                        let mut bindings = operations
                            .structural_values
                            .iter()
                            .filter(|(ordinal, _)| *ordinal == binding_ordinal);
                        let (_, binding) = bindings.next().ok_or(LoweringError::Unsupported(
                            "borrowed-window repair join has no completed binding",
                        ))?;
                        let result = binding.operation_result()?;
                        if bindings.next().is_some() || result.place != declaration.id {
                            return unsupported(
                                "borrowed-window repair join substituted its completed place",
                            );
                        }
                        result.structural_type
                    }
                    _ => {
                        return unsupported(
                            "borrowed-window repair binding is not a completed value",
                        );
                    }
                };
                Ok(BorrowedWindowRepairValue {
                    place: declaration.id,
                    structural_type,
                })
            }
            Self::StateGraph { state, .. } => {
                let binding = super::composed_control::state_graph_result(
                    state,
                    binding_ordinal,
                    operations,
                )?;
                let produced = binding.operation_result()?;
                Ok(BorrowedWindowRepairValue {
                    place: produced.place,
                    structural_type: produced.structural_type,
                })
            }
        }
    }

    /// A dense roster admits only the next ordinal; a composed state rejects
    /// a repeated binding when it registers the result.
    fn require_next(
        &self,
        result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
        drifted: &'static str,
    ) -> Result<(), LoweringError> {
        match self {
            Self::Dense(roster) if result.binding_ordinal as usize != roster.len() => {
                unsupported(drifted)
            }
            _ => Ok(()),
        }
    }

    /// The completed results a call's operands resolve by binding ordinal,
    /// as the dense `(declaration, discard)` roster transfer validation and
    /// argument lowering index.
    ///
    /// A composed state's binding ordinals belong to the source state, while
    /// its place catalog spans the whole emitted machine, so the view rejoins
    /// the operation buffer's registry (one row per ordinal, in order) to the
    /// one catalog declaration its producing operation established. Graph
    /// states own each result's cleanup per edge (`result_custody`), so no
    /// row is marked for a return discard here. A call naming no completed
    /// result reads no row, and the view stays empty rather than failing on
    /// a registry it never consults.
    fn earlier(
        &self,
        arguments: &[CheckedUnitStructuralArgumentPlan],
        operations: &OperationBuffer,
    ) -> Result<Cow<'_, [(StructuralPlaceDeclaration, bool)]>, LoweringError> {
        let places = match self {
            Self::Dense(roster) => return Ok(Cow::Borrowed(roster.as_slice())),
            Self::StateGraph { places, .. } => places,
        };
        if !arguments.iter().any(|argument| {
            argument
                .source_structural_result_binding_ordinal()
                .is_some()
        }) {
            return Ok(Cow::Owned(Vec::new()));
        }
        operations
            .structural_values
            .iter()
            .enumerate()
            .map(|(binding_position, (ordinal, result))| {
                if *ordinal as usize != binding_position {
                    return unsupported(
                        "composed call result binding namespace is stale or duplicated",
                    );
                }
                match result {
                    StructuralBinding::ByteSequenceLiteral { place, structural_type, declaration_ordinal } => {
                        let declaration = StructuralPlaceDeclaration { id: *place, kind: StructuralPlaceKind::ByteSequenceLiteral { structural_type: *structural_type, declaration_ordinal: *declaration_ordinal } };
                        let mut producers = operations.iter().filter(|operation| matches!(operation.kind,
                            OperationKind::EstablishByteSequenceLiteral { destination, .. } if destination == declaration.id));
                        let producer = producers.next().ok_or(LoweringError::Unsupported("literal binding has no establishment"))?;
                        if producers.next().is_some() || producer.result != OperationResult::Unit {
                            return unsupported("literal binding repeats or changes its establishment");
                        }
                        Ok((declaration, false))
                    }
                    StructuralBinding::OperationResult(result) => {
                        let mut declarations = places.iter().filter(|place| place.id == result.place);
                        let declaration = declarations.next().ok_or(LoweringError::Unsupported(
                            "composed call completed result has no place declaration",
                        ))?;
                        if declarations.next().is_some()
                            || !matches!(declaration.kind,
                            StructuralPlaceKind::OperationResult { structural_type, producer }
                                if structural_type == result.structural_type
                                    && operations.operations.iter().any(|candidate| {
                                        candidate.id == producer
                                            && candidate.result.structural() == Some(result)
                                    }))
                        {
                            return unsupported("composed call result declaration differs from its operation");
                        }
                        Ok((*declaration, false))
                    }
                }
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Cow::Owned)
    }

    /// Add one completed result's declaration: the dense row with its
    /// return-discard custody, or the machine-wide composed catalog.
    fn push(&mut self, declaration: StructuralPlaceDeclaration, discard_on_return: bool) {
        match self {
            Self::Dense(roster) => roster.push((declaration, discard_on_return)),
            Self::StateGraph { places, .. } => places.push(declaration),
        }
    }
}

impl OperationFrame<'_, '_> {
    /// Whether `operation` emits through `emit`. Every other operation a
    /// composed state carries is a call, which needs its completed operands
    /// and emits through `emit_call`.
    pub(super) fn lowers(operation: &CheckedUnitEffectOperationPlan) -> bool {
        matches!(
            operation,
            CheckedUnitEffectOperationPlan::PortWrite { .. }
                | CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
                | CheckedUnitEffectOperationPlan::EstablishStructuralLocal { .. }
                | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
                | CheckedUnitEffectOperationPlan::RepresentationViewStore(_)
                | CheckedUnitEffectOperationPlan::ByteSequenceStore(_)
                | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
                | CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
                | CheckedUnitEffectOperationPlan::ElementViewWrite(_)
                | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
                | CheckedUnitEffectOperationPlan::StructuralCaseFieldStore(_)
                | CheckedUnitEffectOperationPlan::StructuralCaseLiteralFieldStore(_)
                | CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. }
                | CheckedUnitEffectOperationPlan::EstablishScalarArray { .. }
                | CheckedUnitEffectOperationPlan::MoveStructuralField { .. }
                | CheckedUnitEffectOperationPlan::StoreStructuralField { .. }
                | CheckedUnitEffectOperationPlan::CallContinuationCleanup { .. }
                | CheckedUnitEffectOperationPlan::EstablishByteSequence { .. }
                | CheckedUnitEffectOperationPlan::EstablishViewSubslice { .. }
                | CheckedUnitEffectOperationPlan::EstablishReference { .. }
                | CheckedUnitEffectOperationPlan::ReleaseReference { .. }
                | CheckedUnitEffectOperationPlan::AtomicAccess(_)
        )
    }

    /// Emit one operation `lowers` admits.
    pub(super) fn emit(
        mut self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<(), LoweringError> {
        match operation {
            CheckedUnitEffectOperationPlan::PortWrite {
                service_reach,
                port,
                value,
                ..
            } => self.port_write(service_reach, *port, *value),
            CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                statement_index,
                destination,
                path,
                value,
            } => self.write_only_primitive_store(*statement_index, destination, path, value),
            CheckedUnitEffectOperationPlan::RepresentationViewStore(write) => {
                self.representation_view_store(write)
            }
            CheckedUnitEffectOperationPlan::ByteSequenceStore(store) => {
                self.structural_byte_sequence_field_store(store)
            }
            CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(store) => {
                self.indexed_write(IndexedWriteTarget::ByteField(store))
            }
            CheckedUnitEffectOperationPlan::ByteSequenceWrite(write) => {
                let target = match write.destination {
                    checked_trees::CheckedStoreRoot::Parameter { .. } => {
                        IndexedWriteTarget::ByteView(write)
                    }
                    checked_trees::CheckedStoreRoot::Local { symbol } => {
                        let (argument, produced) =
                            crate::emission::store_destination::local_store_root(
                                symbol,
                                &self.evaluation.structural_locals,
                                self.operations,
                            )?;
                        if argument.access != StructuralAccess::Owned {
                            return unsupported("byte write local is not an owned home");
                        }
                        IndexedWriteTarget::OwnedBytes {
                            write,
                            place: argument.place,
                            structural_type: produced.structural_type,
                        }
                    }
                };
                self.indexed_write(target)
            }
            CheckedUnitEffectOperationPlan::ElementViewWrite(write) => {
                self.element_view_write(write)
            }
            CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) => {
                self.structural_scalar_field_store(store)
            }
            CheckedUnitEffectOperationPlan::StructuralCaseFieldStore(store) => {
                self.structural_case_field_store(store)
            }
            CheckedUnitEffectOperationPlan::StructuralCaseLiteralFieldStore(store) => {
                self.structural_case_literal_field_store(store)
            }
            CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, value } => {
                self.establish_scalar_local(result, value)
            }
            CheckedUnitEffectOperationPlan::EstablishScalarArray { .. } => {
                self.establish_scalar_array(operation)
            }
            CheckedUnitEffectOperationPlan::MoveStructuralField { result, source } => {
                self.move_structural_field(result, source)
            }
            CheckedUnitEffectOperationPlan::StoreStructuralField {
                statement_index,
                destination,
                value,
            } => self.store_structural_field(*statement_index, destination, value),
            CheckedUnitEffectOperationPlan::CallContinuationCleanup {
                affine_discards, ..
            } => self.call_continuation_cleanup(affine_discards),
            CheckedUnitEffectOperationPlan::EstablishByteSequence { .. } => {
                self.establish_byte_sequence(operation)
            }
            CheckedUnitEffectOperationPlan::EstablishViewSubslice { .. } => {
                self.establish_view_subslice(operation)
            }
            CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. } => {
                self.establish_structural_value(operation)
            }
            CheckedUnitEffectOperationPlan::EstablishStructuralLocal { .. } => {
                self.establish_structural_local(operation)
            }
            CheckedUnitEffectOperationPlan::EstablishReference { result, source } => {
                self.establish_reference(result, source)
            }
            CheckedUnitEffectOperationPlan::ReleaseReference { .. } => {
                self.release_reference(operation)
            }
            CheckedUnitEffectOperationPlan::AtomicAccess(access) => self.atomic_access(access),
            _ => unsupported("operation frame received a call operation"),
        }
    }

    /// Establish a reference permission carrier for the composed route, the
    /// same `reference_results::emit` the ordinary route drives; the binding
    /// joins the machine-wide place catalog and the operation registry so
    /// later arguments, the matching release and the state-graph result view
    /// all rejoin it by ordinal.
    fn establish_reference(
        &mut self,
        result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
        source: &CheckedUnitStructuralArgumentPlan,
    ) -> Result<(), LoweringError> {
        self.results
            .require_next(result, "reference result binding is not dense")?;
        let argument =
            if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::ScalarParameter {
                parameter_index,
            } = source.source
            {
                self.primitive_parameter_source(parameter_index, source)?
            } else {
                let earlier = self
                    .results
                    .earlier(std::slice::from_ref(source), self.operations)?;
                parameters::lower_reference_source(
                    source,
                    self.structural_types.declarations(),
                    self.parameters,
                    self.caller.local_places,
                    &earlier,
                    self.primitive_locals,
                )?
            };
        let declaration = reference_results::emit(
            result,
            argument,
            self.type_ids,
            self.next_place,
            self.operations,
        )?;
        let Some(terminal_psi::OperationResult::Structural(produced)) = self
            .operations
            .operations
            .last()
            .map(|operation| operation.result.clone())
        else {
            return unsupported("reference establishment produced no structural result");
        };
        let (_, authored) = crate::expression_preparation::source_custody::authored_state(
            self.checked,
            self.state,
        )?;
        let local = matches!(
            self.checked
                .statement_table
                .statements(authored.statement_nodes)
                .get(result.statement_index as usize),
            Some(checked_trees::statement::StatementNode::LocalData(_))
        );
        if local {
            reference_results::validate_local_establishment(
                self.checked,
                self.state,
                result,
                source,
            )?;
        }
        self.results.push(declaration, local);
        self.evaluation.establish_structural_binding(
            self.checked,
            self.state,
            result,
            true,
            produced.into(),
            self.structural_types.declarations(),
            self.operations,
        )
    }

    /// Give one immutable scalar ingress its original addressable home. Every
    /// loan of that parameter rejoins this place, including differently typed
    /// views; no loan copies a previously established referent.
    fn primitive_parameter_source(
        &mut self,
        parameter_index: u32,
        source: &CheckedUnitStructuralArgumentPlan,
    ) -> Result<StructuralArgument, LoweringError> {
        let (_, authored) = crate::expression_preparation::source_custody::authored_state(
            self.checked,
            self.state,
        )?;
        let parameter = self
            .checked
            .state_parameters(authored)
            .iter()
            .filter(|parameter| {
                !parameter.is_const
                    && !parameter.relevance.is_erased()
                    && self
                        .checked
                        .primitive_type_reference(parameter.type_reference)
                        .is_some()
            })
            .nth(parameter_index as usize)
            .ok_or(LoweringError::Unsupported(
                "primitive reference ingress is absent",
            ))?;
        let scalar_type = terminal_scalar_type(
            self.checked
                .primitive_type_reference(parameter.type_reference)
                .ok_or(LoweringError::Unsupported(
                    "primitive reference ingress lost its carrier",
                ))?,
        )?;
        if parameter.is_mutable
            || !parameter.symbol.is_valid()
            || !source.path.is_empty()
            || source.access != checked_trees::CheckedStructuralAccess::SharedBorrow
            || self
                .checked
                .normalized_type_identity(parameter.type_reference)
                .as_str()
                != source.type_identity
        {
            return unsupported("primitive reference ingress changed its source custody");
        }
        let mut homes = self
            .evaluation
            .primitive_storage
            .iter()
            .filter(|row| row.0 == parameter.symbol);
        if let Some((_, place, existing_type)) = homes.next() {
            if homes.next().is_some() || *existing_type != scalar_type {
                return unsupported("primitive ingress has ambiguous storage");
            }
            return Ok(StructuralArgument {
                place: *place,
                path: Vec::new(),
                access: StructuralAccess::SharedBorrow,
            });
        }
        let position = match &self.evaluation.scalar_bindings {
            Some(bindings) => bindings.immutable_position(parameter_index as usize)?,
            None => parameter_index as usize,
        };
        let value = self.values.get(position).ok_or(LoweringError::Unsupported(
            "primitive ingress value is absent",
        ))?;
        let structural_type = lookup_type_id(self.type_ids, &source.type_identity)?;
        if value.scalar_type != scalar_type
            || !self
                .structural_types
                .declarations()
                .iter()
                .any(|declaration| {
                    declaration.id == structural_type
                        && declaration.shape
                            == terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar_type)
                })
        {
            return unsupported("primitive ingress value and storage carrier disagree");
        }
        let producer = self.operations.allocate();
        let place = crate::unit::place_id(crate::unit::allocate_dense(self.next_place)?);
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: producer,
            result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                place,
                structural_type,
                multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
                qualification_establishments: Vec::new(),
            }),
            kind: OperationKind::EstablishPrimitiveLocal { value: value.id },
        });
        self.private_places
            .temporaries()
            .push(StructuralPlaceDeclaration {
                id: place,
                kind: StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                },
            });
        self.evaluation
            .primitive_storage
            .push((parameter.symbol, place, scalar_type));
        Ok(StructuralArgument {
            place,
            path: Vec::new(),
            access: StructuralAccess::SharedBorrow,
        })
    }

    /// End the established reference carrier: its place rejoins through the
    /// same binding-ordinal view a call continuation uses.
    fn release_reference(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<(), LoweringError> {
        let CheckedUnitEffectOperationPlan::ReleaseReference {
            binding_ordinal, ..
        } = operation
        else {
            unreachable!("dispatched release_reference")
        };
        let source = self
            .results
            .continuation_place(*binding_ordinal, self.operations)?;
        self.push_unit(OperationKind::ReleaseReference {
            source: self.evaluation.current_structural_place(source),
        });
        Ok(())
    }

    /// Append one Unit-result operation at the next identity.
    fn push_unit(&mut self, kind: OperationKind) {
        let id = self.operations.allocate();
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id,
            result: OperationResult::Unit,
            kind,
        });
    }

    fn port_write(
        &mut self,
        service_reach: &ServiceReachSummary,
        port: u16,
        value: u8,
    ) -> Result<(), LoweringError> {
        let checked = self.checked;
        let rows = &checked.facts.service_reaches.rows;
        let [port_service] = rows.services(service_reach.direct) else {
            return unsupported(
                "port output does not carry the unique exact checked PortIo service",
            );
        };
        if !rows
            .services(service_reach.transitive)
            .contains(port_service)
        {
            return unsupported(
                "port output does not carry the unique exact checked PortIo service",
            );
        }
        // `CheckedUnitEffectOperationPlan::PortWrite` is minted only for the
        // exact checked asm-port-out builtin. Its singleton direct row is
        // therefore the symbol-backed PortIo authority; no spelling lookup is
        // repeated here.
        let service = lookup_service_id(self.service_ids, *port_service)?;
        self.push_unit(OperationKind::PortWrite {
            service,
            port,
            value,
        });
        Ok(())
    }

    /// A primitive store names an exclusive borrowed parameter projection or
    /// an established primitive local.
    fn write_only_primitive_store(
        &mut self,
        statement_index: u32,
        destination: &checked_trees::CheckedStoreRoot,
        path: &[checked_trees::CheckedUnitStructuralPathSegment],
        value: &checked_trees::CheckedCallScalarArgument,
    ) -> Result<(), LoweringError> {
        if let Some(loan) =
            crate::scalar_graph::scalar_computations::fields::loaned_primitive_store(
                self.checked,
                self.machine,
                self.state,
                destination,
                path,
                self.structural_types.declarations(),
            )?
        {
            let projection = loan
                .byte_overlay
                .as_ref()
                .ok_or(LoweringError::Unsupported(
                    "primitive recast store lost its byte projection",
                ))?;
            // Capture the destination before evaluating the RHS, just as for
            // field stores. A recast loan never invents another owned home.
            let destination = self.prepare_loaned_byte_store(statement_index, &loan)?;
            // Source custody expands the loan to its captured prefix before
            // matching the assignment's value role, independently of layout.
            let mut authored_path = loan.path.clone();
            authored_path.extend_from_slice(path);
            let role = crate::emission::primitive_store::value_role(
                self.checked,
                self.state,
                statement_index,
                &authored_path,
            )?;
            let value = self.evaluation.source_value(
                self.checked,
                self.machine,
                self.state,
                statement_index,
                role,
                value,
                self.source_value_count,
                self.values,
                self.next_value,
                self.next_block,
                self.next_edge,
                self.operations,
                self.calls,
            )?;
            if value.scalar_type != projection.scalar_type || !value.qualifications.is_empty() {
                return unsupported("primitive store RHS differs from its destination carrier");
            }
            for kind in destination.emit(
                value,
                projection.stored_type,
                projection.offset,
                self.next_value,
                self.operations,
                self.calls,
            )? {
                self.push_unit(kind);
            }
            return Ok(());
        }
        let destination = match destination {
            checked_trees::CheckedStoreRoot::Parameter { position } => {
                // The root names the authored parameter position;
                // `structural_parameters` pairs each checked plan's authored
                // position with the declaration that position lowered into.
                let (_, parameter) = self
                    .evaluation
                    .structural_parameters
                    .iter()
                    .find(|(authored, _)| authored == position)
                    .ok_or(LoweringError::Unsupported(
                        "primitive store parameter is absent",
                    ))?;
                crate::emission::primitive_store::parameter_destination(
                    parameter,
                    path,
                    self.structural_types.declarations(),
                )?
            }
            checked_trees::CheckedStoreRoot::Local { symbol } => {
                if path.is_empty() {
                    let local = primitive_locals::find(self.primitive_locals, *symbol)?;
                    crate::emission::primitive_store::Destination {
                        place: local.declaration.id,
                        path: Vec::new(),
                        scalar_type: local.scalar_type,
                    }
                } else {
                    // A projected store writes through an established
                    // structural local's owned place: the declared carrier
                    // supplies the walk, the binding table supplies storage.
                    let source = self
                        .evaluation
                        .structural_locals
                        .iter()
                        .find(|local| local.symbol == *symbol)
                        .map(|local| local.argument.clone())
                        .ok_or(LoweringError::Unsupported(
                            "primitive store lost its structural local destination",
                        ))?;
                    if source.access != StructuralAccess::Owned || !source.path.is_empty() {
                        return unsupported("primitive store local is not an owned whole place");
                    }
                    let (_, authored) =
                        crate::expression_preparation::source_custody::authored_state(
                            self.checked,
                            self.state,
                        )?;
                    let local = crate::emission::structural_scalar_store_source::local_destination(
                        self.checked,
                        authored,
                        statement_index,
                        *symbol,
                    )?;
                    let identity = self.checked.normalized_type_identity(local.type_reference);
                    let declaration = self
                        .structural_types
                        .declarations()
                        .iter()
                        .find(|declaration| declaration.identity == identity.as_str())
                        .ok_or(LoweringError::Unsupported(
                            "primitive store local lost its declared carrier",
                        ))?;
                    let (steps, scalar_type) =
                        crate::emission::primitive_store::lower_destination_path(
                            declaration.id,
                            path,
                            self.structural_types.declarations(),
                        )?;
                    crate::emission::primitive_store::Destination {
                        place: source.place,
                        path: steps,
                        scalar_type,
                    }
                }
            }
        };
        let role = crate::emission::primitive_store::value_role(
            self.checked,
            self.state,
            statement_index,
            path,
        )?;
        let kind = crate::emission::primitive_store::emit_assignment(
            self.checked,
            self.machine,
            self.state,
            statement_index,
            destination,
            role,
            value,
            self.evaluation,
            self.source_value_count,
            self.values,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
            self.calls,
        )?;
        self.push_unit(kind);
        Ok(())
    }

    fn structural_byte_sequence_field_store(
        &mut self,
        store: &checked_trees::CheckedByteSequenceStorePlan,
    ) -> Result<(), LoweringError> {
        use crate::emission::structural_byte_sequence_store::StoreRoot;
        let root = match store.root {
            checked_trees::CheckedStoreRoot::Parameter { position } => {
                let (_, parameter) = self
                    .evaluation
                    .structural_parameters
                    .iter()
                    .find(|(authored, _)| *authored == position)
                    .ok_or(LoweringError::Unsupported(
                        "byte-field store destination is absent",
                    ))?;
                StoreRoot::Parameter(parameter)
            }
            checked_trees::CheckedStoreRoot::Local { symbol } => {
                let (place, structural_type) = self.local_store_root(symbol)?;
                if self
                    .structural_types
                    .declarations()
                    .iter()
                    .any(|declaration| {
                        declaration.id == structural_type
                            && matches!(
                                declaration.shape,
                                terminal_psi::StructuralTypeShape::Record { .. }
                            )
                    })
                {
                    StoreRoot::Owned {
                        place,
                        structural_type,
                    }
                } else {
                    StoreRoot::Reference {
                        place,
                        structural_type,
                    }
                }
            }
        };
        // Only a roster this body owns may gain the generated literal-view
        // carrier; the store below rejects a published roster without it.
        if let StructuralTypeRoster::Owned(types) = &mut self.structural_types {
            crate::emission::structural_byte_sequence_store::prepare_types(&store.value, types)?;
        }
        if let checked_trees::CheckedByteSequenceStoreDestination::ViewField {
            carrier_path,
            field_identity,
        } = &store.destination
        {
            let mut destination =
                crate::emission::structural_byte_sequence_store::prepare_view_field_store(
                    store,
                    &root,
                    carrier_path,
                    field_identity,
                    self.structural_types.declarations(),
                )?;
            let indexes = crate::emission::runtime_elements::evaluate_selectors(
                &destination.steps,
                self.checked,
                self.machine,
                self.state,
                store.statement_index,
                self.evaluation,
                self.source_value_count,
                self.values,
                self.next_value,
                self.next_block,
                self.next_edge,
                self.operations,
                self.calls,
            )?;
            self.observe_store_element_extent(
                destination.destination,
                destination.structural_type,
                &destination.steps,
            )?;
            let path = crate::emission::runtime_elements::complete_path(
                std::mem::take(&mut destination.steps),
                indexes,
                self.calls,
            )?;
            let kind = crate::emission::structural_byte_sequence_store::emit_view_field_store(
                store,
                destination,
                path,
                self.structural_types.declarations(),
                self.private_places.literals(),
                self.next_place,
                self.operations,
            )?;
            self.push_unit(kind);
            return Ok(());
        }
        let kind = crate::emission::structural_byte_sequence_store::emit_content_store(
            store,
            root,
            &self.evaluation.structural_parameters,
            &self.evaluation.structural_locals,
            self.structural_types.declarations(),
            self.private_places.literals(),
            self.next_place,
            self.next_value,
            &mut self.calls.next_obligation_identity,
            self.operations,
        )?;
        self.push_unit(kind);
        Ok(())
    }

    /// The scalar namespace an assignment's index and value resolve in: the
    /// composed state's own namespace when the evaluation carries one, and
    /// otherwise the ordinary dense source prefix over primitive storage,
    /// structural parameters, and established structural locals.
    fn dense_assignment_namespace(&self) -> ScalarBindings {
        ScalarBindings::new(self.values.iter().map(|value| value.scalar_type))
            .with_primitive_storage(&self.evaluation.primitive_storage)
            .with_view_locals(&self.evaluation.view_locals)
            .with_byte_record_loans(&self.evaluation.byte_record_loans)
            .with_element_views(&self.evaluation.element_views)
            .with_structural_parameters(&self.evaluation.structural_parameters)
            .with_structural_locals(&self.evaluation.structural_locals)
    }

    /// One runtime-indexed write: the index, then the value, lower in the
    /// assignment's scalar namespace, and `indexed_write::emit` checks the
    /// leaf's destination and emits the coordinate, length and write. Only a
    /// byte field's dense namespace carries the resolved structural field and
    /// case observations. A byte leaf's authored root resolves in
    /// `structural_parameters`, the authored-to-lowered pairing the field,
    /// case-field and element-view stores use, not in `parameters`, whose
    /// positions are dense on the ordinary route.
    fn indexed_write(&mut self, target: IndexedWriteTarget<'_>) -> Result<(), LoweringError> {
        let leaf = target.leaf();
        let dense;
        let bindings = match &self.evaluation.scalar_bindings {
            Some(bindings) => {
                // State scalar positions are stable, but earlier operations
                // may have established owned or borrowed storage since that
                // namespace was created. Join the current local homes before
                // lowering either indexed operand.
                dense = bindings
                    .clone()
                    .with_structural_locals(&self.evaluation.structural_locals)
                    .with_view_locals(&self.evaluation.view_locals)
                    .with_byte_record_loans(&self.evaluation.byte_record_loans);
                &dense
            }
            None => {
                let namespace = self.dense_assignment_namespace();
                dense = match leaf {
                    IndexedWriteLeaf::ByteField(_) => namespace
                        .with_resolved_structural_observations(
                            &self.evaluation.structural_fields,
                            &self.evaluation.structural_cases,
                        ),
                    IndexedWriteLeaf::ByteView(_) | IndexedWriteLeaf::ElementView(_) => namespace,
                };
                &dense
            }
        };
        let index = bindings.expression_at(
            self.checked,
            self.state,
            leaf.statement_index(),
            CheckedScalarExpressionRole::AssignmentIndex { depth: 0 },
        )?;
        let value = leaf.lower_value(bindings, self.checked, self.state, self.values)?;
        let kind = crate::emission::indexed_write::emit(
            target,
            &self.evaluation.structural_parameters,
            self.structural_types.declarations(),
            &index,
            &value,
            self.values,
            self.next_value,
            &mut self.calls.next_obligation_identity,
            self.operations,
        )?;
        self.push_unit(kind);
        Ok(())
    }

    /// A store through a recast loan local emits the value once, then one
    /// primitive store per emitted footprint byte — or one whole store for
    /// a same-width leaf — in the dense scalar namespace the indexed-write
    /// family uses.
    fn representation_view_store(
        &mut self,
        write: &checked_trees::CheckedRepresentationViewStorePlan,
    ) -> Result<(), LoweringError> {
        for kind in crate::emission::representation_view_store::emit(
            write,
            self.checked,
            self.machine,
            self.state,
            &mut *self.evaluation,
            self.source_value_count,
            self.values,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
            self.calls,
            self.structural_types.declarations(),
        )? {
            self.push_unit(kind);
        }
        Ok(())
    }

    /// Resolve a structural-local store destination to the local's live
    /// storage and type. Scalar, byte and sum fields require identical root
    /// authority; only the replacement's value contract differs.
    fn local_store_root(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Result<(PlaceId, StructuralTypeId), LoweringError> {
        crate::emission::store_destination::unqualified_local_store_root(
            symbol,
            &self.evaluation.structural_locals,
            self.operations,
        )
    }

    fn element_view_write(
        &mut self,
        write: &checked_trees::CheckedElementViewWritePlan,
    ) -> Result<(), LoweringError> {
        // A `&mut`-borrowed parameter names its destination place directly; a
        // locally bound `&mut` view names the home place its establishment
        // produced, resolved through the structural-local roster like a whole
        // structural store's local root.
        let (destination_place, destination_type) = match write.destination {
            checked_trees::CheckedStoreRoot::Parameter { position } => {
                let (_, destination) = self
                    .evaluation
                    .structural_parameters
                    .iter()
                    .find(|(authored, _)| *authored == position)
                    .ok_or(LoweringError::Unsupported(
                        "element-view write names an unknown parameter",
                    ))?;
                if destination.access != StructuralAccess::MutableBorrow
                    || destination.multiplicity
                        != terminal_psi::StructuralMultiplicity::Unrestricted
                    || !destination.qualifications.is_empty()
                    || !destination.projected_qualifications.is_empty()
                {
                    return unsupported("element-view write parameter lost mutable custody");
                }
                (destination.place, destination.structural_type)
            }
            checked_trees::CheckedStoreRoot::Local { symbol } => self.local_store_root(symbol)?,
        };
        self.indexed_write(IndexedWriteTarget::ElementView {
            write,
            place: destination_place,
            structural_type: destination_type,
        })
    }

    /// Field and element stores both retain a recast loan's original backing.
    /// This prepares selectors only; callers evaluate their ordinary RHS next.
    fn prepare_loaned_byte_store(
        &mut self,
        statement_index: u32,
        loan: &crate::scalar_graph::scalar_computations::fields::LoanedStore,
    ) -> Result<crate::emission::representation_view_store::ByteStoreDestination, LoweringError>
    {
        use crate::scalar_graph::scalar_computations::fields::LoanedSource;
        let (place, root_type) = match loan.source {
            LoanedSource::Parameter(position) => {
                let (_, parameter) = self
                    .evaluation
                    .structural_parameters
                    .get(position as usize)
                    .ok_or(LoweringError::Unsupported(
                        "record store lost its backing parameter",
                    ))?;
                crate::emission::structural_scalar_store::validate_root_access(
                    parameter,
                    crate::emission::structural_scalar_store::StoreAccessPolicy::Exclusive,
                )?;
                (parameter.place, parameter.structural_type)
            }
            LoanedSource::Local(symbol) => self.local_store_root(symbol)?,
        };
        let captured_index = if loan.byte_overlay.is_some()
            && matches!(loan.path.last(), Some(checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(_)))
        {
            let bindings = self.dense_assignment_namespace();
            let coordinate = bindings.byte_record_coordinate(loan.symbol)?;
            Some(crate::emission::operation_emission::emit_direct_expression(
                &coordinate, self.values, self.next_value, self.operations,
            ))
        } else {
            None
        };
        let mut destination = crate::emission::representation_view_store::prepare_byte_destination(
            place,
            root_type,
            &loan.path,
            captured_index,
            self.checked,
            self.machine,
            self.state,
            statement_index,
            self.evaluation,
            self.source_value_count,
            self.values,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
            self.calls,
            self.structural_types.declarations(),
        )?;
        if let Some(projection) = &loan.byte_overlay {
            let steps = projection
                .indices
                .iter()
                .map(|index| crate::emission::runtime_elements::selector_step(&index.selector))
                .collect::<Vec<_>>();
            let indexes = crate::emission::runtime_elements::evaluate_selectors(
                &steps,
                self.checked,
                self.machine,
                self.state,
                statement_index,
                self.evaluation,
                self.source_value_count,
                self.values,
                self.next_value,
                self.next_block,
                self.next_edge,
                self.operations,
                self.calls,
            )?;
            if indexes.len() != projection.indices.len() {
                return unsupported("record byte store lost a declared array coordinate");
            }
            for (index, dimension) in indexes.into_iter().zip(&projection.indices) {
                destination.project_index(
                    index,
                    dimension.stride,
                    dimension.extent,
                    self.next_value,
                    self.operations,
                )?;
            }
        }
        Ok(destination)
    }

    fn structural_scalar_field_store(
        &mut self,
        store: &checked_trees::CheckedStructuralScalarFieldStorePlan,
    ) -> Result<(), LoweringError> {
        let loan = crate::scalar_graph::scalar_computations::fields::loaned_store(
            self.checked,
            self.machine,
            self.state,
            store,
            self.structural_types.declarations(),
        )?;
        let mut rejoined;
        let store = if let Some(loan) = loan {
            use crate::scalar_graph::scalar_computations::fields::LoanedSource;
            let destination = match loan.source {
                LoanedSource::Parameter(position) => {
                    let (authored, _) = self
                        .evaluation
                        .structural_parameters
                        .get(position as usize)
                        .ok_or(LoweringError::Unsupported(
                            "record store lost its captured parameter",
                        ))?;
                    checked_trees::CheckedStoreRoot::Parameter {
                        position: *authored,
                    }
                }
                LoanedSource::Local(symbol) => checked_trees::CheckedStoreRoot::Local { symbol },
            };
            if let Some(projection) = &loan.byte_overlay {
                let destination = self.prepare_loaned_byte_store(store.statement_index, &loan)?;
                let value = self.scalar_field_store_value(store)?;
                for kind in destination.emit(
                    value,
                    projection.stored_type,
                    projection.offset,
                    self.next_value,
                    self.operations,
                    self.calls,
                )? {
                    self.push_unit(kind);
                }
                return Ok(());
            }
            rejoined = store.clone();
            rejoined.destination = destination;
            rejoined.carrier_path = loan.path;
            &rejoined
        } else {
            store
        };
        // A parameter destination roots the store at its borrowed place; an
        // owned structural local roots it at the local's live home place,
        // resolved through the structural-local roster.
        let (destination_place, destination_type, lowered) = match store.destination {
            checked_trees::CheckedStoreRoot::Parameter { position } => {
                // The destination names the authored parameter position;
                // `structural_parameters` pairs each checked plan's authored
                // position with the declaration that position lowered into.
                let (_, destination) = self
                    .evaluation
                    .structural_parameters
                    .iter()
                    .find(|(authored, _)| *authored == position)
                    .ok_or(LoweringError::Unsupported(
                        "structural scalar store names an unknown parameter",
                    ))?;
                let lowered =
                    crate::emission::structural_scalar_store::lower_structural_scalar_store_place(
                        store,
                        store.statement_index,
                        destination,
                        self.structural_types.declarations(),
                        crate::emission::structural_scalar_store::StoreAccessPolicy::Exclusive,
                    )?;
                (destination.place, destination.structural_type, lowered)
            }
            checked_trees::CheckedStoreRoot::Local { symbol } => {
                let (place, root_type) = self.local_store_root(symbol)?;
                let lowered =
                    crate::emission::structural_scalar_store::lower_structural_scalar_store_local(
                        store,
                        root_type,
                        self.structural_types.declarations(),
                    )?;
                (place, root_type, lowered)
            }
        };
        // The carrier's runtime elements are evaluated before the value.
        let indexes = crate::emission::runtime_elements::evaluate_selectors(
            &lowered.steps,
            self.checked,
            self.machine,
            self.state,
            store.statement_index,
            self.evaluation,
            self.source_value_count,
            self.values,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
            self.calls,
        )?;
        let value = self.scalar_field_store_value(store)?;
        if value.scalar_type != lowered.scalar_type {
            return unsupported("structural scalar store RHS differs from its field type");
        }
        self.observe_store_element_extent(destination_place, destination_type, &lowered.steps)?;
        let kind = lowered.into_operation(destination_place, indexes, value.id, self.calls)?;
        self.push_unit(kind);
        Ok(())
    }

    /// A descriptor's live extent owns runtime bounds through either a scalar
    /// field store or a stored-view replacement. References keep their own
    /// authority; following the type here only finds the extent representation.
    fn observe_store_element_extent(
        &mut self,
        destination: semantic_vocabulary::PlaceId,
        mut structural_type: StructuralTypeId,
        steps: &[crate::emission::runtime_elements::ProjectionStep],
    ) -> Result<(), LoweringError> {
        use crate::emission::runtime_elements::ProjectionStep;
        let declarations = self.structural_types.declarations();
        if let Some(terminal_psi::StructuralTypeShape::Reference { referent, .. }) = declarations
            .iter()
            .find(|declaration| declaration.id == structural_type)
            .map(|declaration| &declaration.shape)
        {
            structural_type = *referent;
        }
        let is_view = declarations.iter().any(|declaration| {
            declaration.id == structural_type
                && matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::ElementView { .. }
                )
        });
        if !is_view
            || !steps
                .iter()
                .any(|step| !matches!(step, ProjectionStep::Static(_)))
        {
            return Ok(());
        }
        let length = crate::terminal_identities::value_id(allocate_dense(self.next_value)?);
        let observation = self.operations.allocate();
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: observation,
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: length,
                scalar_type: terminal_scalar_type(crate::unit::PrimitiveType::U64)?,
            }),
            kind: OperationKind::ElementViewLength {
                source: destination,
            },
        });
        Ok(())
    }

    fn scalar_field_store_value(
        &mut self,
        store: &checked_trees::CheckedStructuralScalarFieldStorePlan,
    ) -> Result<ValueDeclaration, LoweringError> {
        self.evaluation.field_assignment_value(
            self.checked,
            self.machine,
            self.state,
            store.statement_index,
            &store.value,
            store.primitive_type,
            Some(&store.field_identity),
            self.values,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
            self.calls,
        )
    }

    /// Map an authored store root to the window argument's structural lane.
    /// Composed source-reader declarations keep authored `position` values;
    /// roster order, not that field, is the checked dense parameter namespace.
    fn window_destination(
        &self,
        destination: checked_trees::CheckedStoreRoot,
    ) -> Result<
        (
            StructuralTypeId,
            checked_trees::CheckedUnitStructuralArgumentSourcePlan,
        ),
        LoweringError,
    > {
        use checked_trees::CheckedUnitStructuralArgumentSourcePlan;
        match destination {
            checked_trees::CheckedStoreRoot::Parameter { position } => {
                let (_, authored) = self
                    .evaluation
                    .structural_parameters
                    .iter()
                    .find(|(source_position, _)| *source_position == position)
                    .ok_or(LoweringError::Unsupported(
                        "window store names an unknown authored parameter",
                    ))?;
                let (dense, parameter) = self
                    .parameters
                    .iter()
                    .enumerate()
                    .find(|(_, parameter)| parameter.place == authored.place)
                    .ok_or(LoweringError::Unsupported(
                        "window store parameter has no retained slot",
                    ))?;
                Ok((
                    parameter.structural_type,
                    CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index: u32::try_from(dense).map_err(|_| {
                            LoweringError::Unsupported(
                                "window parameter index exceeds its namespace",
                            )
                        })?,
                    },
                ))
            }
            checked_trees::CheckedStoreRoot::Local { symbol } => {
                let (_, structural_type) = self.local_store_root(symbol)?;
                Ok((
                    structural_type,
                    CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
                ))
            }
        }
    }

    /// Replace one unrestricted sum field beneath a mutable-borrowed
    /// parameter. The checked plan supplies the authored value as a parameter
    /// projection an owned or shared place still holds: leaf-copy it into a
    /// fresh owned place, vacate the authored field through the window
    /// ledger, restore the hole with the copy, and retire the moved-out leaf
    /// trivially on the following edge.
    fn structural_case_field_store(
        &mut self,
        store: &checked_trees::CheckedStructuralCaseFieldStorePlan,
    ) -> Result<(), LoweringError> {
        // A parameter destination roots the window at its borrowed place; an
        // owned structural local roots it at the local's live home place.
        // The window pair below writes through either root identically.
        let (root_type, source) = self.window_destination(store.destination)?;
        let (_, field) = crate::emission::structural_scalar_store::lower_structural_field_path(
            root_type,
            &store.carrier_path,
            &store.field_identity,
            self.structural_types.declarations(),
        )?;
        let terminal_psi::StructuralFieldType::Structural(hole_type) = field.field_type else {
            return unsupported("structural case field store names a non-structural field");
        };
        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
            parameter_index: value_index,
        } = store.value.source
        else {
            return unsupported("structural case field store value is not a parameter");
        };
        if !store.value.path.iter().all(|segment| {
            matches!(
                segment,
                checked_trees::CheckedUnitStructuralPathSegment::Field(_)
            )
        }) {
            return unsupported("structural case field store value path leaves record fields");
        }
        let value_root =
            self.parameters
                .get(value_index as usize)
                .ok_or(LoweringError::Unsupported(
                    "structural case field store value names an unknown parameter",
                ))?;
        let source_place = self.evaluation.current_structural_place(value_root.place);
        // The copy reads the source where it lives: a shared loan or an owned
        // parameter both admit the leaf copy without moving anything.
        let copied_place = place_id(allocate_dense(self.next_place)?);
        let copy = self.operations.allocate();
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: copy,
            result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place: copied_place,
                structural_type: hole_type,
                multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::StructuralLeafCopy {
                source: source_place,
                path: lower_structural_path(&store.value.path)?,
            },
        });
        self.private_places
            .literals()
            .push(StructuralPlaceDeclaration {
                id: copied_place,
                kind: StructuralPlaceKind::OperationResult {
                    producer: copy,
                    structural_type: hole_type,
                },
            });
        let mut field_path = store.carrier_path.clone();
        field_path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
            store.field_identity.clone(),
        ));
        let window_place = CheckedUnitStructuralArgumentPlan {
            source,
            path: field_path,
            type_identity: store.value.type_identity.clone(),
            access: checked_trees::CheckedStructuralAccess::Owned,
        };
        require_parameter_window_root(self.parameters, &window_place, self.evaluation)?;
        let mut evaluation = WindowEvaluation {
            checked: self.checked,
            machine: self.machine,
            state: self.state,
            statement_index: store.statement_index,
            evaluation: &mut *self.evaluation,
            source_value_count: self.source_value_count,
            calls: &mut *self.calls,
        };
        let moved = self.windows.emit_move(
            &window_place,
            &checked_trees::CheckedUnitStructuralResultBindingPlan {
                statement_index: store.statement_index,
                binding_ordinal: 0,
                type_identity: store.value.type_identity.clone(),
                multiplicity: language_semantics::Multiplicity::Unrestricted,
            },
            self.parameters,
            self.structural_types.declarations(),
            self.type_ids,
            &mut evaluation,
            self.values,
            self.next_place,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
        )?;
        // The moved-out leaf is an internal temporary, not an authored
        // result — declare it as a private temporary so the verifier finds
        // its producing operation, then the cleanup edge retires it.
        let move_id = self
            .operations
            .operations
            .last()
            .ok_or(LoweringError::Unsupported(
                "structural case field store move emitted no operation",
            ))?
            .id;
        self.private_places
            .literals()
            .push(StructuralPlaceDeclaration {
                id: moved,
                kind: StructuralPlaceKind::OperationResult {
                    producer: move_id,
                    structural_type: hole_type,
                },
            });
        // The moved-out leaf is `Unrestricted` by the plan's own contract, so
        // no custody action retires it: edge discards admit affine places
        // only, and an unrestricted owned place may simply be left behind.
        self.windows.emit_store(
            &window_place,
            BorrowedWindowRepairValue {
                place: copied_place,
                structural_type: hole_type,
            },
            self.parameters,
            self.structural_types.declarations(),
            self.type_ids,
            &mut evaluation,
            self.values,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
        )
    }

    /// Replace one sum field with an authored case literal: the store itself
    /// establishes the construction. Every scalar operand evaluates in the
    /// literal's `StructuralValueField` namespace, an unnamed common field
    /// takes its declared zero, and the established place repairs the vacated
    /// field through the same move/store window pair a whole-place store
    /// emits — the hole's custody story is identical; only the producer
    /// differs.
    fn structural_case_literal_field_store(
        &mut self,
        store: &checked_trees::CheckedStructuralCaseLiteralFieldStorePlan,
    ) -> Result<(), LoweringError> {
        let (root_type, source) = self.window_destination(store.destination)?;
        let (_, field) = crate::emission::structural_scalar_store::lower_structural_field_path(
            root_type,
            &store.carrier_path,
            &store.field_identity,
            self.structural_types.declarations(),
        )?;
        let terminal_psi::StructuralFieldType::Structural(hole_type) = field.field_type else {
            return unsupported("structural case literal field store names a non-structural field");
        };
        let declaration = self
            .structural_types
            .declarations()
            .iter()
            .find(|declaration| declaration.id == hole_type)
            .ok_or(LoweringError::Unsupported(
                "structural case literal field store type missing",
            ))?;
        let (common, cases) = match &declaration.shape {
            terminal_psi::StructuralTypeShape::Sum { cases } => (Vec::new(), cases.clone()),
            terminal_psi::StructuralTypeShape::Mixed { fields, cases } => (
                fields
                    .iter()
                    .filter(|field| {
                        !matches!(
                            field.field_type,
                            terminal_psi::StructuralFieldType::Erased { .. }
                        )
                    })
                    .cloned()
                    .collect::<Vec<_>>(),
                cases.clone(),
            ),
            _ => return unsupported("case literal field store has a non-sum shape"),
        };
        let case = cases
            .iter()
            .find(|case| case.identity == store.case_identity)
            .ok_or(LoweringError::Unsupported(
                "case literal field store case declaration missing",
            ))?;
        // A mixed shape's establishment roster is its runtime common fields in
        // declaration order, then the selected case's payload.
        let shape_fields = common
            .iter()
            .chain(case.fields.iter().filter(|field| {
                !matches!(
                    field.field_type,
                    terminal_psi::StructuralFieldType::Erased { .. }
                )
            }))
            .cloned()
            .collect::<Vec<_>>();
        // Case construction uses the same current assignment namespace as
        // ordinary field stores. A single-state body has no preinstalled
        // state namespace, including when this case has no scalar payload.
        let bindings = self.evaluation.scalar_bindings.clone().unwrap_or_else(|| {
            self.dense_assignment_namespace()
                .with_resolved_structural_observations(
                    &self.evaluation.structural_fields,
                    &self.evaluation.structural_cases,
                )
        });
        let mut initialized = Vec::new();
        for field_plan in &store.fields {
            let (position, target) = shape_fields
                .iter()
                .enumerate()
                .find(|(_, candidate)| candidate.identity == field_plan.field_identity)
                .ok_or(LoweringError::Unsupported(
                    "case literal field store field identity missing",
                ))?;
            if !matches!(
                target.field_type,
                terminal_psi::StructuralFieldType::Scalar(_)
                    | terminal_psi::StructuralFieldType::IeeeFloat(_)
                    | terminal_psi::StructuralFieldType::BoundedInteger(_)
            ) {
                return unsupported("case literal operand requires a scalar carrier");
            }
            let expression = bindings.expression_at(
                self.checked,
                self.state,
                store.statement_index,
                CheckedScalarExpressionRole::StructuralValueField {
                    expression: store.literal,
                    field_ordinal: field_plan.field_ordinal,
                },
            )?;
            if target.field_type.scalar_type() != Some(expression.scalar_type())
                || crate::emission::expression_validation::direct_expression_contains_short_circuit(
                    &expression,
                )
            {
                return unsupported(
                    "case literal operand needs an exact branch-free scalar operand",
                );
            }
            crate::emission::expression_validation::validate_direct_parameter_types(
                &expression,
                &self
                    .values
                    .iter()
                    .map(|value| value.scalar_type)
                    .collect::<Vec<_>>(),
            )?;
            let value = crate::emission::operation_emission::expressions::emit_direct_expression(
                &expression,
                self.values,
                self.next_value,
                self.operations,
            );
            // The verifier reconstructs the declared range against the final
            // SSA initializer; a bounded field requests that proof.
            let range_obligation = if matches!(
                target.field_type,
                terminal_psi::StructuralFieldType::BoundedInteger(_)
            ) {
                Some(crate::terminal_identities::obligation_id(allocate_dense(
                    &mut self.calls.next_obligation_identity,
                )?))
            } else {
                None
            };
            initialized.push((
                position,
                terminal_psi::RecordFieldInitializer {
                    field: target.id,
                    value: terminal_psi::RecordFieldValue::Scalar {
                        value,
                        range_obligation,
                    },
                },
            ));
        }
        // Case construction zero-initializes every common field the literal
        // leaves unnamed.
        for (position, target) in common.iter().enumerate() {
            if initialized
                .iter()
                .any(|(initialized, _)| *initialized == position)
            {
                continue;
            }
            let scalar_type = target
                .field_type
                .scalar_type()
                .ok_or(LoweringError::Unsupported(
                    "an unnamed common field has no scalar zero",
                ))?;
            let zero = crate::emission::operation_emission::expressions::emit_scalar_leaf(
                crate::unit::attached_unit::structural_values::record::zero_constant(scalar_type),
                scalar_type,
                self.next_value,
                self.operations,
            );
            let range_obligation = if matches!(
                target.field_type,
                terminal_psi::StructuralFieldType::BoundedInteger(_)
            ) {
                Some(crate::terminal_identities::obligation_id(allocate_dense(
                    &mut self.calls.next_obligation_identity,
                )?))
            } else {
                None
            };
            initialized.push((
                position,
                terminal_psi::RecordFieldInitializer {
                    field: target.id,
                    value: terminal_psi::RecordFieldValue::Scalar {
                        value: zero,
                        range_obligation,
                    },
                },
            ));
        }
        initialized.sort_by_key(|(position, _)| *position);
        let copied_place = place_id(allocate_dense(self.next_place)?);
        let establishment = self.operations.allocate();
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: establishment,
            result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place: copied_place,
                structural_type: hole_type,
                multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishStructuralCase {
                result_case: case.id,
                fields: initialized.into_iter().map(|(_, field)| field).collect(),
            },
        });
        self.private_places
            .literals()
            .push(StructuralPlaceDeclaration {
                id: copied_place,
                kind: StructuralPlaceKind::OperationResult {
                    producer: establishment,
                    structural_type: hole_type,
                },
            });
        let mut field_path = store.carrier_path.clone();
        field_path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
            store.field_identity.clone(),
        ));
        let window_place = CheckedUnitStructuralArgumentPlan {
            source,
            path: field_path,
            type_identity: store.type_identity.clone(),
            access: checked_trees::CheckedStructuralAccess::Owned,
        };
        require_parameter_window_root(self.parameters, &window_place, self.evaluation)?;
        let mut evaluation = WindowEvaluation {
            checked: self.checked,
            machine: self.machine,
            state: self.state,
            statement_index: store.statement_index,
            evaluation: &mut *self.evaluation,
            source_value_count: self.source_value_count,
            calls: &mut *self.calls,
        };
        let moved = self.windows.emit_move(
            &window_place,
            &checked_trees::CheckedUnitStructuralResultBindingPlan {
                statement_index: store.statement_index,
                binding_ordinal: 0,
                type_identity: store.type_identity.clone(),
                multiplicity: language_semantics::Multiplicity::Unrestricted,
            },
            self.parameters,
            self.structural_types.declarations(),
            self.type_ids,
            &mut evaluation,
            self.values,
            self.next_place,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
        )?;
        let move_id = self
            .operations
            .operations
            .last()
            .ok_or(LoweringError::Unsupported(
                "structural case literal field store move emitted no operation",
            ))?
            .id;
        self.private_places
            .literals()
            .push(StructuralPlaceDeclaration {
                id: moved,
                kind: StructuralPlaceKind::OperationResult {
                    producer: move_id,
                    structural_type: hole_type,
                },
            });
        // The moved-out leaf is `Unrestricted` by the plan's own contract, so
        // no custody action retires it.
        self.windows.emit_store(
            &window_place,
            BorrowedWindowRepairValue {
                place: copied_place,
                structural_type: hole_type,
            },
            self.parameters,
            self.structural_types.declarations(),
            self.type_ids,
            &mut evaluation,
            self.values,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
        )
    }

    /// Bind one scalar local. A composed state names it in its own scalar
    /// namespace at its dense position; without that namespace the source
    /// bindings are the dense prefix of formals followed by locals in ordinal
    /// order, so the ordinal must name the next position.
    fn establish_scalar_local(
        &mut self,
        result: &checked_trees::CheckedUnitScalarResultBindingPlan,
        value: &checked_trees::CheckedCallScalarArgument,
    ) -> Result<(), LoweringError> {
        if self.evaluation.scalar_bindings.is_none()
            && usize::try_from(result.binding_ordinal)
                .ok()
                .and_then(|ordinal| ordinal.checked_add(self.scalar_parameter_count))
                != Some(self.values.len())
        {
            return unsupported("Unit scalar expression local binding drifted from source order");
        }
        // The source value is keyed by the local's authored ordinal, not by
        // the dense binding ordinal the plan assigns it.
        let role = if self.scalar_result == Some(result) {
            CheckedScalarExpressionRole::Return
        } else {
            CheckedScalarExpressionRole::LocalInitializer {
                binding_ordinal:
                    crate::expression_preparation::source_custody::source_local_ordinal(
                        self.checked,
                        self.state,
                        result.statement_index,
                    )?,
            }
        };
        let lowered = self.evaluation.source_value(
            self.checked,
            self.machine,
            self.state,
            result.statement_index,
            role,
            value,
            self.source_value_count,
            self.values,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
            self.calls,
        )?;
        if lowered.scalar_type != terminal_scalar_type(result.primitive_type)? {
            return unsupported("Unit scalar expression local type disagrees with its binding");
        }
        let lowered = self.refine_scalar_local_value(result.statement_index, lowered)?;
        if let Some(bindings) = self.evaluation.scalar_bindings.as_mut() {
            bindings.append(
                checked_trees::CheckedScalarBindingDestination::Immutable,
                lowered.scalar_type,
                self.values.len(),
            )?;
        }
        self.values.push(lowered);
        Ok(())
    }

    /// Establish the declared predicate on a completed local value before
    /// publishing its source binding. The continuation forwards the existing
    /// namespace and this value together, regardless of its producing operation.
    fn refine_scalar_local_value(
        &mut self,
        statement_index: u32,
        value: ValueDeclaration,
    ) -> Result<ValueDeclaration, LoweringError> {
        let (_, state) = crate::expression_preparation::source_custody::authored_state(
            self.checked,
            self.state,
        )?;
        let Some(checked_trees::statement::StatementNode::LocalData(local)) = self
            .checked
            .statement_table
            .statements(state.statement_nodes)
            .get(statement_index as usize)
        else {
            return Ok(value);
        };
        if local.type_is_inferred {
            return Ok(value);
        }
        let primitive_type = self
            .checked
            .primitive_type_reference(local.type_reference)
            .ok_or(LoweringError::Unsupported(
                "scalar local lost its primitive type",
            ))?;
        if terminal_scalar_type(primitive_type)? != value.scalar_type {
            return unsupported("completed scalar local disagrees with its declared type");
        }
        // Numeric predicates do not stand in for declared membership:
        // retain the catalog's independent rejection of unsupported theories.
        crate::expression_preparation::qualifications::type_atoms(
            self.checked,
            local.type_reference,
        )?;
        let predicates = validation::scalar_type_refinement_predicates(
            &self.checked.typed,
            local.type_reference,
            checked_trees::CheckedScalarExpression::Parameter {
                position: self.values.len(),
                primitive_type,
            },
        )
        .ok_or(LoweringError::Unsupported(
            "Unit local refinement has no predicate owner",
        ))?;
        if predicates.is_empty() {
            return Ok(value);
        }
        if local.is_mutable {
            return unsupported(
                "mutable scalar value refinements require invariant-window custody",
            );
        }
        let machine = self
            .calls
            .machine_ids
            .iter()
            .find(|(symbol, _)| *symbol == self.machine)
            .map(|(_, identity)| *identity)
            .ok_or(LoweringError::Unsupported(
                "local refinement lost its machine identity",
            ))?;
        self.values.push(value);
        self.evaluation.require_value_refinements(
            machine,
            predicates,
            self.values,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
            self.calls,
        )?;
        self.values.pop().ok_or(LoweringError::Unsupported(
            "local refinement continuation lost its completed value",
        ))
    }

    /// One `let`-bound primitive array literal: stage each element operand as
    /// a private value tail, emit the constructor's structural result into
    /// the body's own binding namespace, then release the tail. A
    /// statement-source construction additionally binds the declared local.
    fn establish_scalar_array(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<(), LoweringError> {
        let CheckedUnitEffectOperationPlan::EstablishScalarArray {
            source,
            result,
            elements,
        } = operation
        else {
            return unsupported("array producer missing");
        };
        self.results
            .require_next(result, "array result binding is not dense")?;
        let leaf_start = self.values.len();
        for (ordinal, element) in elements.iter().enumerate() {
            let element_ordinal = u32::try_from(ordinal)
                .map_err(|_| LoweringError::Unsupported("array element ordinal exceeds u32"))?;
            let value = self.evaluation.source_value(
                self.checked,
                self.machine,
                self.state,
                result.statement_index,
                CheckedScalarExpressionRole::ArrayElement {
                    source: *source,
                    element_ordinal,
                },
                element,
                self.source_value_count,
                self.values,
                self.next_value,
                self.next_block,
                self.next_edge,
                self.operations,
                self.calls,
            )?;
            self.values.push(value);
        }
        let declaration = super::scalar_arrays::emit(
            result,
            &self.values[leaf_start..],
            self.type_ids,
            self.next_place,
            self.operations,
        )?;
        self.values.truncate(leaf_start);
        if let StructuralResults::StateGraph { places, state } = &mut self.results {
            places.push(declaration);
            let producer = self
                .operations
                .operations
                .last()
                .ok_or(LoweringError::Unsupported(
                    "array establishment emitted no operation",
                ))?;
            let OperationResult::Structural(produced) = &producer.result else {
                return unsupported("array establishment produced no structural value");
            };
            let produced = produced.clone();
            self.evaluation.establish_structural_result(
                self.checked,
                state.state,
                result,
                *source == checked_trees::CheckedArrayConstructionSource::Statement,
                produced,
                self.structural_types.declarations(),
                self.operations,
            )?;
        } else {
            self.results.push(declaration, false);
            // The dense route registers no result row, so it binds the
            // statement's local here; the state-graph row's own registration
            // already named it.
            if *source == checked_trees::CheckedArrayConstructionSource::Statement {
                super::structural_values::bind_local(
                    self.checked,
                    self.state,
                    operation,
                    declaration,
                    &mut self.evaluation.structural_locals,
                )?;
            }
        }
        Ok(())
    }

    /// Move one whole structural field out of borrowed storage into the
    /// authored result binding. The opened window stays in the ledger until
    /// the matching `StoreStructuralField`; each route requires the ledger
    /// closed before its sequence's exits.
    fn move_structural_field(
        &mut self,
        result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
        source: &CheckedUnitStructuralArgumentPlan,
    ) -> Result<(), LoweringError> {
        self.results
            .require_next(result, "borrowed-window move result binding is not dense")?;
        require_parameter_window_root(self.parameters, source, self.evaluation)?;
        let mut evaluation = WindowEvaluation {
            checked: self.checked,
            machine: self.machine,
            state: self.state,
            statement_index: result.statement_index,
            evaluation: &mut *self.evaluation,
            source_value_count: self.source_value_count,
            calls: &mut *self.calls,
        };
        let moved = self.windows.emit_move(
            source,
            result,
            self.parameters,
            self.structural_types.declarations(),
            self.type_ids,
            &mut evaluation,
            self.values,
            self.next_place,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
        )?;
        let producer = self
            .operations
            .operations
            .last()
            .ok_or(LoweringError::Unsupported(
                "borrowed-window move emitted no operation",
            ))?;
        let OperationResult::Structural(produced) = &producer.result else {
            return unsupported("borrowed-window move established no structural value");
        };
        let declaration = StructuralPlaceDeclaration {
            id: moved,
            kind: StructuralPlaceKind::OperationResult {
                producer: producer.id,
                structural_type: produced.structural_type,
            },
        };
        let produced = produced.clone();
        match &mut self.results {
            StructuralResults::Dense(roster) => roster.push((declaration, false)),
            // The composed namespace resolves later operands through the
            // operation buffer's registry, so the moved value registers there.
            StructuralResults::StateGraph { places, .. } => {
                places.push(declaration);
                self.evaluation.establish_structural_result(
                    self.checked,
                    self.state,
                    result,
                    true,
                    produced,
                    self.structural_types.declarations(),
                    self.operations,
                )?;
            }
        }
        Ok(())
    }

    /// Establish the authored carrier at its declaration, then register the
    /// actual backing in the local observation namespaces. A shared view
    /// borrows immutable literal storage; a bounded owner constructs fresh
    /// storage with its own live length and declared qualifications.
    fn establish_byte_sequence(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<(), LoweringError> {
        let CheckedUnitEffectOperationPlan::EstablishByteSequence {
            result,
            value,
            qualifications,
        } = operation
        else {
            return unsupported("literal local has no producer");
        };
        self.results
            .require_next(result, "literal local binding is not dense")?;
        super::byte_locals::validate(self.checked, self.state, operation)?;
        let structural_type = lookup_type_id(self.type_ids, &result.type_identity)?;
        let mut qualifications = qualifications
            .iter()
            .map(|domain| {
                crate::terminal_identities::lookup_domain_id_for_carrier(
                    self.callees.domain_ids,
                    *domain,
                    structural_type,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        qualifications.sort();
        qualifications.dedup();
        if self
            .structural_types
            .declarations()
            .iter()
            .any(|declaration| {
                declaration.id == structural_type
                    && matches!(
                        declaration.shape,
                        terminal_psi::StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                        )
                    )
            })
        {
            return self.establish_owned_byte_sequence(
                result,
                value,
                structural_type,
                qualifications,
            );
        }
        if !self
            .structural_types
            .declarations()
            .iter()
            .any(|declaration| {
                declaration.id == structural_type
                    && matches!(
                        declaration.shape,
                        terminal_psi::StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                        )
                    )
            })
        {
            return unsupported("literal local carrier is not a borrowed byte view");
        }
        let bytes = value.literal_bytes().ok_or(LoweringError::Unsupported(
            "borrowed byte local has no literal storage",
        ))?;
        let destination = place_id(allocate_dense(self.next_place)?);
        let declaration_ordinal =
            crate::emission::next_byte_sequence_literal_ordinal(self.private_places.literals())?;
        let declaration = StructuralPlaceDeclaration {
            id: destination,
            kind: StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal,
                structural_type,
            },
        };
        self.private_places.literals().push(declaration);
        let id = self.operations.allocate();
        self.operations.push(Operation {
            id,
            static_reach_binding: None,
            suspension_crossing: None,
            result: OperationResult::Unit,
            kind: OperationKind::EstablishByteSequenceLiteral {
                destination,
                bytes: bytes.to_vec(),
                qualifications,
            },
        });
        // The literal roster owns its declaration. Dense bindings index that
        // same place; a graph's registry retains its explicit literal kind.
        if let StructuralResults::Dense(roster) = &mut self.results {
            roster.push((declaration, false));
        }
        self.evaluation.establish_structural_binding(
            self.checked,
            self.state,
            result,
            true,
            StructuralBinding::ByteSequenceLiteral {
                place: destination,
                structural_type,
                declaration_ordinal,
            },
            self.structural_types.declarations(),
            self.operations,
        )
    }

    /// Bind an owned bounded byte local to fresh backing copied from its
    /// initializer. The concatenation result is the local's own place: later
    /// loans, lengths and reads name it exactly as they name any owned result.
    fn establish_owned_byte_sequence(
        &mut self,
        result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
        value: &checked_trees::CheckedByteSequenceValuePlan,
        structural_type: semantic_vocabulary::StructuralTypeId,
        qualifications: Vec<StructuralDomainId>,
    ) -> Result<(), LoweringError> {
        // Only a roster this body owns may gain the generated literal-view
        // carrier; a published roster must already hold it.
        if let StructuralTypeRoster::Owned(types) = &mut self.structural_types {
            crate::emission::structural_byte_sequence_store::prepare_types(value, types)?;
        }
        let (declaration, produced) =
            crate::emission::structural_byte_sequence_store::emit_owned_value(
                value,
                structural_type,
                qualifications,
                &self.evaluation.structural_parameters,
                &self.evaluation.structural_locals,
                self.structural_types.declarations(),
                self.private_places.literals(),
                self.next_place,
                self.next_value,
                &mut self.calls.next_obligation_identity,
                self.operations,
            )?;
        match &mut self.results {
            StructuralResults::Dense(roster) => roster.push((declaration, false)),
            StructuralResults::StateGraph { places, .. } => places.push(declaration),
        }
        self.evaluation.establish_structural_result(
            self.checked,
            self.state,
            result,
            true,
            produced,
            self.structural_types.declarations(),
            self.operations,
        )
    }

    /// Bind one immutable view local to an exclusive range. The range
    /// replays at its `LocalBinding` site and emits through the same replay
    /// and Terminal operation a call argument or an edge transfer uses. The
    /// published place joins the local namespace exactly as an `as_slice`
    /// view local's does, so later edges, calls, lengths and element reads
    /// resolve either kind of view local the same way.
    ///
    /// A range over array backing has no view to narrow yet. It first
    /// establishes a whole element view of that collection
    /// under a shared loan -- the same `EstablishElementView` an `as_slice`
    /// local emits -- into a private temporary, and narrows that. The
    /// verifier then relates the whole view's length to the array's declared
    /// extent and checks the range against it like any other subslice.
    fn establish_view_subslice(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<(), LoweringError> {
        let CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, source } = operation
        else {
            return unsupported("view subslice binding has no producer");
        };
        self.results
            .require_next(result, "view subslice result binding is not dense")?;
        let binding_site = super::view_ranges::binding_site(self.checked, self.state, operation)?;
        let (expression, start, end) = match &source.source {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
                root: _,
                expression,
                start,
                end,
            } => (*expression, start, end),
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
                root: _,
                expression,
                start,
                end,
            } => (*expression, start, end),
            _ => return unsupported("view subslice binding lost its range source"),
        };

        // Endpoints read the body's scalar namespace, including the view
        // locals and parameters a `len` endpoint may observe.
        let bindings = self
            .evaluation
            .scalar_bindings
            .clone()
            .unwrap_or_else(|| {
                self.dense_assignment_namespace()
                    .with_structural_observations(self.structural_types.declarations())
            })
            .with_structural_locals(&self.evaluation.structural_locals)
            .with_view_locals(&self.evaluation.view_locals)
            .with_byte_record_loans(&self.evaluation.byte_record_loans);
        let destination = place_id(allocate_dense(self.next_place)?);
        let site = ViewRangeSite {
            state: self.state,
            statement: result.statement_index,
            site: binding_site,
            expression,
            retained: Some((start, end)),
        };
        let result_type = lookup_type_id(self.type_ids, &result.type_identity)?;
        let storage = super::view_ranges::storage(
            self.checked,
            self.state,
            result.statement_index,
            source,
            self.structural_types.declarations(),
        )?;
        let whole = storage
            .requires_establishment()
            .then(|| allocate_dense(self.next_place).map(place_id))
            .transpose()?;
        let (range_source, establishment) =
            storage.bind(self.checked, self.state, &bindings, whole)?;
        if let Some(establishment) = establishment {
            let declaration = super::view_ranges::establish(&establishment, self.operations);
            self.private_places.temporaries().push(declaration);
        }
        let declaration = super::view_ranges::emit(
            self.checked,
            site,
            range_source,
            &source.path,
            result_type,
            destination,
            &bindings,
            self.values,
            self.next_value,
            self.operations,
        )?;
        let produced = self
            .operations
            .operations
            .last()
            .and_then(|producer| producer.result.structural())
            .filter(|produced| produced.place == destination)
            .cloned()
            .ok_or(LoweringError::Unsupported(
                "view subslice established no structural value",
            ))?;
        match &mut self.results {
            StructuralResults::Dense(roster) => roster.push((declaration, false)),
            StructuralResults::StateGraph { places, .. } => places.push(declaration),
        }
        self.evaluation.establish_structural_result(
            self.checked,
            self.state,
            result,
            true,
            produced,
            self.structural_types.declarations(),
            self.operations,
        )
    }

    /// Establish one declared structural local's home as an operation result
    /// place. Member stores and borrows rejoin the local through the binding
    /// the result registration publishes; the home itself has no contents
    /// until member operations establish them.
    fn establish_structural_local(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<(), LoweringError> {
        let CheckedUnitEffectOperationPlan::EstablishStructuralLocal {
            result,
            discard_result_on_return,
            ..
        } = operation
        else {
            return unsupported("structural local establishment has no result");
        };
        self.results
            .require_next(result, "structural local result binding is not dense")?;
        let structural_type = lookup_type_id(self.type_ids, &result.type_identity)?;
        let multiplicity = match result.multiplicity {
            language_semantics::Multiplicity::Affine => {
                terminal_psi::StructuralMultiplicity::Affine
            }
            language_semantics::Multiplicity::Unrestricted => {
                terminal_psi::StructuralMultiplicity::Unrestricted
            }
            language_semantics::Multiplicity::Linear => {
                terminal_psi::StructuralMultiplicity::Linear
            }
        };
        let home = place_id(allocate_dense(self.next_place)?);
        let producer = self.operations.allocate();
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: producer,
            result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place: home,
                structural_type,
                multiplicity,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishStructuralLocal {},
        });
        let declaration = StructuralPlaceDeclaration {
            id: home,
            kind: StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            },
        };
        self.results.push(declaration, *discard_result_on_return);
        let produced = self
            .operations
            .operations
            .last()
            .and_then(|produced| produced.result.structural().cloned())
            .ok_or(LoweringError::Unsupported(
                "structural local established no result",
            ))?;
        self.evaluation.establish_structural_result(
            self.checked,
            self.state,
            result,
            true,
            produced,
            self.structural_types.declarations(),
            self.operations,
        )
    }

    /// Store a whole owned result back into its open window. The ledger
    /// closes the hole; the repaired place reads through any selection that
    /// transported the result.
    fn store_structural_field(
        &mut self,
        statement_index: u32,
        destination: &CheckedUnitStructuralArgumentPlan,
        value: &CheckedUnitStructuralArgumentPlan,
    ) -> Result<(), LoweringError> {
        if !value.path.is_empty() || value.access != checked_trees::CheckedStructuralAccess::Owned {
            return unsupported("borrowed-window repair value is not a whole owned result");
        }
        let repair = match value.source {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal,
            } => {
                let repair = self
                    .results
                    .repair_value(binding_ordinal, self.operations)?;
                BorrowedWindowRepairValue {
                    place: self.evaluation.current_structural_place(repair.place),
                    structural_type: repair.structural_type,
                }
            }
            // An owned parameter's own storage supplies the repair whole.
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index,
            } => {
                let parameter = self.parameters.get(parameter_index as usize);
                let Some(parameter) = parameter else {
                    return unsupported("borrowed-window repair value's parameter is not declared");
                };
                if parameter.access != terminal_psi::StructuralAccess::Owned {
                    return unsupported("borrowed-window repair value is not an owned parameter");
                }
                BorrowedWindowRepairValue {
                    place: parameter.place,
                    structural_type: parameter.structural_type,
                }
            }
            // An owned local's established home supplies the repair whole.
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                let local = self
                    .evaluation
                    .structural_locals
                    .iter()
                    .find(|local| local.symbol == symbol);
                let Some(crate::expression_preparation::bindings::StructuralLocalBinding {
                    argument,
                    ..
                }) = local
                else {
                    return unsupported("borrowed-window repair value's local is not established");
                };
                BorrowedWindowRepairValue {
                    place: argument.place,
                    structural_type: lookup_type_id(self.type_ids, &value.type_identity)?,
                }
            }
            _ => {
                return unsupported("borrowed-window repair value is not a whole owned result");
            }
        };
        require_parameter_window_root(self.parameters, destination, self.evaluation)?;
        let mut evaluation = WindowEvaluation {
            checked: self.checked,
            machine: self.machine,
            state: self.state,
            statement_index,
            evaluation: &mut *self.evaluation,
            source_value_count: self.source_value_count,
            calls: &mut *self.calls,
        };
        self.windows.emit_store(
            destination,
            repair,
            self.parameters,
            self.structural_types.declarations(),
            self.type_ids,
            &mut evaluation,
            self.values,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
        )
    }

    /// Commit the dying affine results on a completed call's normal
    /// continuation while its evaluated scalar bindings stay live.
    fn call_continuation_cleanup(
        &mut self,
        affine_discards: &[checked_trees::CheckedUnitPartialAffineDiscardPlan],
    ) -> Result<(), LoweringError> {
        let mut discards = Vec::new();
        let mut residuals = Vec::new();
        for discard in affine_discards {
            let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal,
            } = discard.source
            else {
                return unsupported("call continuation cleanup requires a result binding");
            };
            let place = self
                .results
                .continuation_place(binding_ordinal, self.operations)?;
            if discard.path.is_empty() {
                discards.push(place);
            } else {
                residuals.push(terminal_psi::StructuralAffineDiscard {
                    place,
                    path: lower_structural_path(&discard.path)?,
                    structural_type: lookup_type_id(self.type_ids, &discard.type_identity)?,
                });
            }
        }
        self.evaluation
            .continue_values(
                discards,
                residuals,
                self.values,
                self.next_value,
                self.next_block,
                self.next_edge,
                self.operations,
            )
            .map(|_| ())
    }
}

/// The exclusive root a borrowed-storage window names, as the sequence
/// currently holds it. An owned selection that transported a parameter root
/// to a join parameter leaves that root undetermined; owned local roots have
/// no parameter to transport and pass vacuously.
fn require_parameter_window_root(
    parameters: &[StructuralParameterDeclaration],
    place: &CheckedUnitStructuralArgumentPlan,
    evaluation: &Evaluation,
) -> Result<(), LoweringError> {
    let transported = place.source_parameter_index().is_some_and(|position| {
        parameters.get(position as usize).is_some_and(|parameter| {
            evaluation.current_structural_place(parameter.place) != parameter.place
        })
    });
    if transported {
        return unsupported("borrowed-window root was transported to a join parameter");
    }
    Ok(())
}

#[cfg(test)]
mod completed_storage_tests {
    use super::{
        OperationBuffer, StructuralBinding, StructuralPlaceDeclaration, StructuralPlaceKind,
        StructuralResults, place_id,
    };

    #[test]
    fn joined_repair_requires_one_exact_completed_binding() {
        let place = place_id(1);
        let structural_type = crate::terminal_identities::structural_type_id(1);
        let declaration = StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::BlockParameter {
                block: crate::terminal_identities::block_id(1),
                position: 0,
            },
        };
        let mut declarations = vec![(declaration, false)];
        let results = StructuralResults::Dense(&mut declarations);
        let mut operations = OperationBuffer::new(0);
        assert!(results.repair_value(0, &operations).is_err());
        operations.structural_values.push((
            0,
            StructuralBinding::OperationResult(terminal_psi::StructuralOperationResult {
                place,
                structural_type,
                multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            }),
        ));
        let repair = results
            .repair_value(0, &operations)
            .expect("completed join");
        assert_eq!(repair.place, place);
        assert_eq!(repair.structural_type, structural_type);
        operations
            .structural_values
            .push(operations.structural_values[0].clone());
        assert!(
            results.repair_value(0, &operations).is_err(),
            "duplicate binding"
        );
        operations.structural_values.pop();
        let StructuralBinding::OperationResult(result) = &mut operations.structural_values[0].1
        else {
            panic!("owned binding")
        };
        result.place = place_id(2);
        assert!(
            results.repair_value(0, &operations).is_err(),
            "substituted place"
        );
    }
}
