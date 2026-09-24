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
//! Operand evaluation stays with each route, because the routes schedule it
//! differently: the ordinary machine stages nested argument groups ahead of
//! their call (`argument_schedule`), and a composed state evaluates each
//! call's operands and literals just before it (`literal_arguments`). Both
//! hand the completed operands to `emit_call` as `CallInputs`; everything from
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
//! What the routes still admit differently is decided before emission, by
//! each route's admission, not by a narrower copy of the emitter.

use super::argument_evaluation::Evaluation;
use super::bodies::UnitPlans;
use super::primitive_locals::{self, PrimitiveLocal};
use super::signatures::MachineSignature;
use super::view_ranges::{ViewRangeSite, ViewRangeSource};
use crate::emission::borrowed_window::{BorrowedWindowLedger, BorrowedWindowRepairValue};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::operation_emission::view_subslice::ViewFamily;
use crate::expression_preparation::bindings::ScalarBindings;
use crate::expression_preparation::bindings::structural_paths::lower_structural_path;
use crate::expression_preparation::bindings::view_locals;
use crate::scalar_graph::scalar_call_closure::callee::PreparedScalarCallee;
use crate::unit::{
    BoundaryMachineId, CheckedScalarExpressionRole, CheckedTrees, CheckedUnitEffectOperationPlan,
    ClaimId, LoweringError, Operation, OperationKind, OperationResult, PermissionClaimIdentity,
    PlaceId, ScalarType, SemanticDomainId, ServiceId, ServiceReachId, ServiceReachSummary,
    StructuralDomainId, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeDeclaration, StructuralTypeId, ValueDeclaration,
    allocate_dense, claim_id, lookup_service_id, lookup_type_id, place_id, terminal_scalar_type,
    unsupported,
};
use checked_trees::{CheckedComposedUnitControlStatePlan, CheckedUnitStructuralArgumentPlan};
use std::borrow::Cow;

mod boundary_calls;
mod calls;
pub(super) use calls::CallInputs;

/// One lowered boundary declaration a call can name: its checked source, the
/// published boundary identity, its structural formals and its dense scalar
/// parameter types.
pub(crate) type BoundaryParameters = (
    symbols::SymbolHandle,
    BoundaryMachineId,
    Vec<StructuralParameterDeclaration>,
    Vec<ScalarType>,
);

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
    /// Established primitive referents; a composed state establishes none.
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
    pub(super) domain_ids: &'f [(SemanticDomainId, StructuralDomainId)],
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
                Ok(place.id)
            }
            Self::StateGraph { state, .. } => {
                super::composed_control::state_graph_result(state, binding_ordinal, operations)
                    .map(|produced| produced.place)
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
                let StructuralPlaceKind::OperationResult {
                    structural_type, ..
                } = declaration.kind
                else {
                    return unsupported(
                        "borrowed-window repair binding is not an operation result",
                    );
                };
                Ok(BorrowedWindowRepairValue {
                    place: declaration.id,
                    structural_type,
                })
            }
            Self::StateGraph { state, .. } => {
                super::composed_control::state_graph_result(state, binding_ordinal, operations).map(
                    |produced| BorrowedWindowRepairValue {
                        place: produced.place,
                        structural_type: produced.structural_type,
                    },
                )
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
                    return unsupported(
                        "composed call result declaration differs from its operation",
                    );
                }
                Ok((*declaration, false))
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
                | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
                | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(_)
                | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
                | CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
                | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
                | CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. }
                | CheckedUnitEffectOperationPlan::MoveStructuralField { .. }
                | CheckedUnitEffectOperationPlan::StoreStructuralField { .. }
                | CheckedUnitEffectOperationPlan::CallContinuationCleanup { .. }
                | CheckedUnitEffectOperationPlan::EstablishViewSubslice { .. }
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
            CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(store) => {
                self.structural_byte_sequence_field_store(store)
            }
            CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(store) => {
                self.structural_byte_sequence_field_byte_store(store)
            }
            CheckedUnitEffectOperationPlan::ByteSequenceWrite(write) => {
                self.byte_sequence_write(write)
            }
            CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) => {
                self.structural_scalar_field_store(store)
            }
            CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, value } => {
                self.establish_scalar_local(result, value)
            }
            CheckedUnitEffectOperationPlan::MoveStructuralField { result, source } => {
                self.move_structural_field(result, source)
            }
            CheckedUnitEffectOperationPlan::StoreStructuralField {
                destination, value, ..
            } => self.store_structural_field(destination, value),
            CheckedUnitEffectOperationPlan::CallContinuationCleanup {
                affine_discards, ..
            } => self.call_continuation_cleanup(affine_discards),
            CheckedUnitEffectOperationPlan::EstablishViewSubslice { .. } => {
                self.establish_view_subslice(operation)
            }
            CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. } => {
                self.establish_structural_value(operation)
            }
            _ => unsupported("operation frame received a call operation"),
        }
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
        destination: &checked_trees::CheckedPrimitiveStoreDestination,
        path: &[checked_trees::CheckedUnitStructuralPathSegment],
        value: &checked_trees::CheckedCallScalarArgument,
    ) -> Result<(), LoweringError> {
        let destination = match destination {
            checked_trees::CheckedPrimitiveStoreDestination::Parameter { parameter_index } => {
                let parameter = self.parameters.get(*parameter_index as usize).ok_or(
                    LoweringError::Unsupported("primitive store parameter is absent"),
                )?;
                crate::emission::primitive_store::parameter_destination(
                    parameter,
                    path,
                    self.structural_types.declarations(),
                )?
            }
            checked_trees::CheckedPrimitiveStoreDestination::Local { symbol } => {
                if !path.is_empty() {
                    return unsupported("primitive local store has a projected destination");
                }
                let local = primitive_locals::find(self.primitive_locals, *symbol)?;
                crate::emission::primitive_store::Destination {
                    place: local.declaration.id,
                    path: Vec::new(),
                    scalar_type: local.scalar_type,
                }
            }
        };
        let kind = crate::emission::primitive_store::emit_assignment(
            self.checked,
            self.machine,
            self.state,
            statement_index,
            destination,
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
        store: &checked_trees::CheckedStructuralByteSequenceFieldStorePlan,
    ) -> Result<(), LoweringError> {
        // Only a roster this body owns may gain the generated literal-view
        // carrier; the store below rejects a published roster without it.
        if let StructuralTypeRoster::Owned(types) = &mut self.structural_types {
            crate::emission::structural_byte_sequence_store::literal_view_type(types)?;
        }
        let kind = crate::emission::structural_byte_sequence_store::emit(
            store,
            self.parameters,
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
    /// otherwise the ordinary dense source prefix over primitive storage and
    /// structural parameters.
    fn dense_assignment_namespace(&self) -> ScalarBindings {
        ScalarBindings::new(self.values.len())
            .with_primitive_storage(&self.evaluation.primitive_storage)
            .with_view_locals(&self.evaluation.view_locals)
            .with_element_views(&self.evaluation.element_views)
            .with_structural_parameters(&self.evaluation.structural_parameters)
    }

    fn structural_byte_sequence_field_byte_store(
        &mut self,
        store: &checked_trees::CheckedStructuralByteSequenceFieldByteStorePlan,
    ) -> Result<(), LoweringError> {
        let dense;
        let bindings = match &self.evaluation.scalar_bindings {
            Some(bindings) => bindings,
            None => {
                dense = self
                    .dense_assignment_namespace()
                    .with_resolved_structural_observations(
                        &self.evaluation.structural_fields,
                        &self.evaluation.structural_cases,
                    );
                &dense
            }
        };
        let index = bindings.expression_at(
            self.checked,
            self.state,
            store.statement_index,
            CheckedScalarExpressionRole::AssignmentIndex,
        )?;
        let value = crate::emission::byte_store_scalar_value(
            bindings,
            self.checked,
            self.state,
            store.statement_index,
            &store.value,
            self.values,
        )?;
        let kind = crate::emission::structural_byte_sequence_index_store::emit(
            store,
            self.parameters,
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

    fn byte_sequence_write(
        &mut self,
        write: &checked_trees::CheckedByteSequenceWritePlan,
    ) -> Result<(), LoweringError> {
        let dense;
        let bindings = match &self.evaluation.scalar_bindings {
            Some(bindings) => bindings,
            None => {
                dense = self.dense_assignment_namespace();
                &dense
            }
        };
        let index = bindings.expression_at(
            self.checked,
            self.state,
            write.statement_index,
            CheckedScalarExpressionRole::AssignmentIndex,
        )?;
        let value = crate::emission::byte_store_scalar_value(
            bindings,
            self.checked,
            self.state,
            write.statement_index,
            &write.value,
            self.values,
        )?;
        let kind = crate::emission::byte_sequence_write::emit(
            write,
            self.parameters,
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

    fn structural_scalar_field_store(
        &mut self,
        store: &checked_trees::CheckedStructuralScalarFieldStorePlan,
    ) -> Result<(), LoweringError> {
        let destination = self
            .parameters
            .iter()
            .find(|parameter| Some(parameter.position) == store.destination.parameter_position())
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
        let value = self.evaluation.field_assignment_value(
            self.checked,
            self.machine,
            self.state,
            store,
            self.values,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
            self.calls,
        )?;
        if value.scalar_type != lowered.scalar_type {
            return unsupported("structural scalar store RHS differs from its field type");
        }
        let kind = lowered.into_operation(
            destination.place,
            value.id,
            &mut self.calls.next_obligation_identity,
        )?;
        self.push_unit(kind);
        Ok(())
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
        let role = if self.scalar_result == Some(result) {
            CheckedScalarExpressionRole::Return
        } else {
            CheckedScalarExpressionRole::LocalInitializer {
                binding_ordinal: result.binding_ordinal,
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
        let moved = self.windows.emit_move(
            source,
            result,
            self.parameters,
            self.structural_types.declarations(),
            self.type_ids,
            self.next_place,
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
                    produced,
                    self.structural_types.declarations(),
                    self.operations,
                )?;
            }
        }
        Ok(())
    }

    /// Bind one immutable view local to an exclusive range of an established
    /// view: a whole view parameter or an earlier view local. The range
    /// replays at its `LocalBinding` site and emits through the same replay
    /// and Terminal operation a call argument or an edge transfer uses. The
    /// published place joins the local namespace exactly as an `as_slice`
    /// view local's does, so later edges, calls, lengths and element reads
    /// resolve either kind of view local the same way.
    ///
    /// A range over a fixed-array field (`self.items[a..b]`) has no view to
    /// narrow yet. It first establishes a whole element view of the field
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
        super::view_ranges::binding_local(self.checked, self.state, operation)?;
        let (root, family, expression, start, end) = match &source.source {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
                root,
                expression,
                start,
                end,
            } => (*root, ViewFamily::Bytes, *expression, start, end),
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
                root,
                expression,
                start,
                end,
            } => (*root, ViewFamily::Elements, *expression, start, end),
            _ => return unsupported("view subslice binding lost its range source"),
        };
        let field_range = !source.path.is_empty();
        let range_source = match root {
            checked_trees::CheckedStorageRoot::Parameter { index } if field_range => {
                self.establish_whole_field_view(index, source, &result.type_identity, family)?
            }
            checked_trees::CheckedStorageRoot::Parameter { index } => {
                let parameter =
                    self.parameters
                        .get(index as usize)
                        .ok_or(LoweringError::Unsupported(
                            "view subslice source parameter is absent",
                        ))?;
                if parameter.access != terminal_psi::StructuralAccess::SharedBorrow
                    || parameter.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                    || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty()
                {
                    return unsupported("view subslice source parameter is not an immutable view");
                }
                let (_, authored) = crate::expression_preparation::source_custody::authored_state(
                    self.checked,
                    self.state,
                )?;
                let symbol = self
                    .checked
                    .state_parameters(authored)
                    .get(parameter.position as usize)
                    .ok_or(LoweringError::Unsupported(
                        "view subslice source has no authored parameter",
                    ))?
                    .symbol;
                ViewRangeSource {
                    symbol,
                    place: self.evaluation.current_structural_place(parameter.place),
                    structural_type: parameter.structural_type,
                    family,
                }
            }
            checked_trees::CheckedStorageRoot::ViewLocal { symbol } => {
                let local = view_locals::resolve(&self.evaluation.view_locals, symbol)?;
                if (family == ViewFamily::Bytes)
                    != (local.carrier == view_locals::ViewCarrier::Bytes)
                {
                    return unsupported("view subslice source local changed its view family");
                }
                ViewRangeSource {
                    symbol,
                    place: self.evaluation.current_structural_place(local.place),
                    structural_type: local.structural_type,
                    family,
                }
            }
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
            .with_view_locals(&self.evaluation.view_locals);
        let destination = place_id(allocate_dense(self.next_place)?);
        let site = ViewRangeSite {
            state: self.state,
            statement: result.statement_index,
            site: checked_trees::CheckedSubsliceSite::LocalBinding,
            expression,
            retained: Some((start, end)),
        };
        let result_type = lookup_type_id(self.type_ids, &result.type_identity)?;
        let declaration = if field_range {
            super::view_ranges::emit_over_field(
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
            )?
        } else {
            super::view_ranges::emit(
                self.checked,
                site,
                range_source,
                result_type,
                destination,
                &bindings,
                self.values,
                self.next_value,
                self.operations,
            )?
        };
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
            produced,
            self.structural_types.declarations(),
            self.operations,
        )
    }

    /// Establish a whole element view of the fixed-array field `source.path`
    /// below structural parameter `index`, as the source a field range
    /// narrows. The view reads the array through a shared loan, so any
    /// readable root lends it; the temporary is private to this binding.
    fn establish_whole_field_view(
        &mut self,
        index: u32,
        source: &CheckedUnitStructuralArgumentPlan,
        type_identity: &str,
        family: ViewFamily,
    ) -> Result<ViewRangeSource, LoweringError> {
        let parameter = self
            .parameters
            .get(index as usize)
            .ok_or(LoweringError::Unsupported(
                "view subslice field owner parameter is absent",
            ))?;
        if family != ViewFamily::Elements
            || source.access != checked_trees::CheckedStructuralAccess::SharedBorrow
            || !matches!(
                parameter.access,
                terminal_psi::StructuralAccess::SharedBorrow
                    | terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::Owned
            )
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
        {
            return unsupported("view subslice field owner cannot lend a shared element view");
        }
        let (_, authored) = crate::expression_preparation::source_custody::authored_state(
            self.checked,
            self.state,
        )?;
        let symbol = self
            .checked
            .state_parameters(authored)
            .get(parameter.position as usize)
            .ok_or(LoweringError::Unsupported(
                "view subslice field owner has no authored parameter",
            ))?
            .symbol;
        let structural_type = lookup_type_id(self.type_ids, type_identity)?;
        let element = self
            .structural_types
            .declarations()
            .iter()
            .find_map(|declaration| match declaration.shape {
                terminal_psi::StructuralTypeShape::ElementView { element }
                    if declaration.id == structural_type =>
                {
                    Some(element)
                }
                _ => None,
            })
            .ok_or(LoweringError::Unsupported(
                "view subslice field range lost its element view type",
            ))?;
        let whole = place_id(allocate_dense(self.next_place)?);
        let producer = self.operations.allocate();
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: producer,
            result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place: whole,
                structural_type,
                multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishElementView {
                destination: whole,
                source: terminal_psi::StructuralArgument {
                    place: self.evaluation.current_structural_place(parameter.place),
                    path: lower_structural_path(&source.path)?,
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                },
                element,
            },
        });
        self.private_places
            .temporaries()
            .push(StructuralPlaceDeclaration {
                id: whole,
                kind: StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                },
            });
        Ok(ViewRangeSource {
            symbol,
            place: whole,
            structural_type,
            family,
        })
    }

    /// Store a whole owned result back into its open window. The ledger
    /// closes the hole; the repaired place reads through any selection that
    /// transported the result.
    fn store_structural_field(
        &mut self,
        destination: &CheckedUnitStructuralArgumentPlan,
        value: &CheckedUnitStructuralArgumentPlan,
    ) -> Result<(), LoweringError> {
        let (Some(binding_ordinal), true, checked_trees::CheckedStructuralAccess::Owned) = (
            value.source_structural_result_binding_ordinal(),
            value.path.is_empty(),
            value.access,
        ) else {
            return unsupported("borrowed-window repair value is not a whole owned result");
        };
        let repair = self
            .results
            .repair_value(binding_ordinal, self.operations)?;
        let repair = BorrowedWindowRepairValue {
            place: self.evaluation.current_structural_place(repair.place),
            structural_type: repair.structural_type,
        };
        require_parameter_window_root(self.parameters, destination, self.evaluation)?;
        self.windows.emit_store(
            destination,
            repair,
            self.parameters,
            self.structural_types.declarations(),
            self.type_ids,
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
        self.evaluation.cleanup_continuation(
            discards,
            residuals,
            self.values,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
        )
    }
}

/// The exclusive parameter root a borrowed-storage window names, as the
/// sequence currently holds it. An owned selection that transported the root
/// to a join parameter leaves no machine-parameter root for the window, and
/// Terminal verification anchors windows on machine parameters only.
fn require_parameter_window_root(
    parameters: &[StructuralParameterDeclaration],
    place: &CheckedUnitStructuralArgumentPlan,
    evaluation: &Evaluation,
) -> Result<(), LoweringError> {
    let transported = place.source_parameter_index().is_some_and(|position| {
        parameters.iter().any(|parameter| {
            parameter.position == position
                && evaluation.current_structural_place(parameter.place) != parameter.place
        })
    });
    if transported {
        return unsupported("borrowed-window root was transported to a join parameter");
    }
    Ok(())
}
