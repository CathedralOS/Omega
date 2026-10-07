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
//! - The call context's shared claim namespace grows with completed scalar
//!   and Unit producers. A composed state borrows its published machine-wide
//!   roster. Nested operands do not start another identity namespace.
//!
//! Private declarations share one machine-owned roster in both routes. A
//! constructor can produce literals as well as temporary values: splitting
//! those rosters gives their literal allocators overlapping ordinals.
//!
//! Every other check either route applied runs for both. In particular a
//! borrowed-window move or repair must name a machine-parameter root the
//! sequence still holds, and a repair value must be a whole owned value.
//! Owned local rebinding instead retains the produced value's home after the
//! old occurrence's disposal. It creates neither an extraction nor borrowed
//! restoration debt; later local reads and cleanup resolve the updated home.
//! A completed result can occupy an operation result or a continuation's block
//! parameter. The latter rejoins the registered binding's exact place and type;
//! its source construction need not manufacture another operation result.
//! What the routes still admit differently is decided before emission, by
//! each route's admission, not by a narrower copy of the emitter.

use super::argument_evaluation::Evaluation;
use super::bodies::UnitPlans;
use super::primitive_locals::PrimitiveLocal;
use super::signatures::MachineSignature;
use super::view_ranges::ViewRangeSite;
use super::{parameters, reference_results};
use crate::emission::borrowed_window::BorrowedWindowLedger;
use crate::emission::indexed_write::IndexedWriteTarget;
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::expression_preparation::bindings::structural_paths::lower_structural_path;
use crate::scalar_graph::scalar_call_closure::callee::PreparedScalarCallee;
use crate::unit::{
    BoundaryMachineId, CheckedTrees, CheckedUnitEffectOperationPlan, LoweringError, Operation,
    OperationKind, OperationResult, PlaceId, ScalarType, ServiceId, ServiceReachId,
    StructuralAccess, StructuralArgument, StructuralDomainId, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeDeclaration, StructuralTypeId,
    ValueDeclaration, allocate_dense, lookup_type_id, place_id, terminal_scalar_type, unsupported,
};
use checked_trees::CheckedUnitStructuralArgumentPlan;
use semantic_vocabulary::{BlockId, OperationId};

mod atomic_access;
mod boundary_calls;
mod byte_operands;
mod calls;
mod dynamic_calls;
mod structural_results;
pub(super) use structural_results::StructuralResults;
mod locals;
mod stores;
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
    pub(super) private_places: &'f mut Vec<StructuralPlaceDeclaration>,
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
    /// The ordinary machine's trivial affine locals; a composed state has none.
    pub(super) local_places: &'f [StructuralPlaceDeclaration],
}

pub(crate) use crate::emission::claim_bindings::claim_established_here;

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
                | CheckedUnitEffectOperationPlan::RebindStructuralLocal { .. }
                | CheckedUnitEffectOperationPlan::ContinuationCleanup { .. }
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
                value_role,
                destination,
                path,
                value,
            } => self.write_only_primitive_store(
                *statement_index,
                *value_role,
                destination,
                path,
                value,
            ),
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
            CheckedUnitEffectOperationPlan::RebindStructuralLocal {
                statement_index,
                destination,
                value,
            } => self.rebind_structural_local(*statement_index, destination, value),
            CheckedUnitEffectOperationPlan::ContinuationCleanup {
                cleanup_actions, ..
            } => self.continuation_cleanup(cleanup_actions),
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
                let earlier = self.results.earlier(
                    std::slice::from_ref(source),
                    self.operations,
                    self.evaluation,
                )?;
                let primitive_home = self.evaluation.immutable_local_borrow_home(
                    self.checked,
                    self.state,
                    source.structural_storage_root(),
                    self.values,
                    self.structural_types.declarations(),
                    self.next_place,
                    self.operations,
                    self.private_places,
                )?;
                let mut locals = self.evaluation.structural_locals.clone();
                locals.extend(primitive_home);
                parameters::lower_reference_source(
                    source,
                    self.structural_types.declarations(),
                    self.parameters,
                    self.caller.local_places,
                    &earlier,
                    self.primitive_locals,
                    &locals,
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
        // Descriptor cleanup follows its emitted transfers and releases. A
        // source local alone does not imply the original holder survives.
        self.results.push(declaration, false);
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
        let (_, authored) = crate::declarations::state_declaration(self.checked, self.state)?;
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
        let place = self.evaluation.immutable_primitive_home(
            parameter.symbol,
            value,
            structural_type,
            self.structural_types.declarations(),
            self.next_place,
            self.operations,
            self.private_places,
        )?;
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
            binding_ordinal,
            path,
            ..
        } = operation
        else {
            unreachable!("dispatched release_reference")
        };
        let source = self
            .results
            .continuation_place(*binding_ordinal, self.operations)?;
        self.push_unit(OperationKind::ReleaseReference {
            source: self.evaluation.current_structural_place(source),
            path: lower_structural_path(path)?,
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
}

/// The structural type's declaration in `declarations`, or `context` as
/// the rejection when it is absent.
pub(super) fn structural_type_declaration<'d>(
    declarations: &'d [StructuralTypeDeclaration],
    id: StructuralTypeId,
    context: &'static str,
) -> Result<&'d StructuralTypeDeclaration, LoweringError> {
    declarations
        .iter()
        .find(|declaration| declaration.id == id)
        .ok_or(LoweringError::Unsupported(context))
}

/// The owned argument a field store's repair window names: the carrier
/// path extended by the stored field under the value's declared type
/// identity.
fn field_window_place(
    source: checked_trees::CheckedUnitStructuralArgumentSourcePlan,
    carrier_path: &[checked_trees::CheckedUnitStructuralPathSegment],
    field_identity: &str,
    type_identity: String,
) -> CheckedUnitStructuralArgumentPlan {
    let mut path = carrier_path.to_vec();
    path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
        field_identity.to_owned(),
    ));
    CheckedUnitStructuralArgumentPlan {
        source,
        path,
        type_identity,
        access: checked_trees::CheckedStructuralAccess::Owned,
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

/// Mint the place and pushed operation for one structural result: every
/// `Structural` result payload shares the same empty-establishment,
/// empty-claim literal, varying only in type, multiplicity, and
/// qualifications.
pub(super) fn allocate_structural_result(
    operations: &mut OperationBuffer,
    next_place: &mut u64,
    structural_type: StructuralTypeId,
    multiplicity: terminal_psi::StructuralMultiplicity,
    qualifications: Vec<StructuralDomainId>,
    kind: OperationKind,
) -> Result<(PlaceId, OperationId), LoweringError> {
    let place = place_id(allocate_dense(next_place)?);
    let producer = operations.allocate();
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: producer,
        result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
            qualification_establishments: Vec::new(),
            place,
            structural_type,
            multiplicity,
            qualifications,
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind,
    });
    Ok((place, producer))
}

/// The id of the operation just pushed, or `context` as the rejection.
pub(super) fn last_operation_id(
    operations: &mut OperationBuffer,
    context: &'static str,
) -> Result<OperationId, LoweringError> {
    operations
        .operations
        .last()
        .map(|operation| operation.id)
        .ok_or(LoweringError::Unsupported(context))
}

/// The `OperationResult` place declaration pairing
/// `allocate_structural_result`.
pub(super) fn operation_result_declaration(
    id: PlaceId,
    producer: OperationId,
    structural_type: StructuralTypeId,
) -> StructuralPlaceDeclaration {
    StructuralPlaceDeclaration {
        id,
        kind: StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        },
    }
}

/// The `BlockParameter` place declaration a join's `block` reads at
/// `position`.
pub(super) fn block_parameter_declaration(
    id: PlaceId,
    block: BlockId,
    position: u32,
) -> StructuralPlaceDeclaration {
    StructuralPlaceDeclaration {
        id,
        kind: StructuralPlaceKind::BlockParameter { block, position },
    }
}

/// The `StructuralParameterDeclaration` every join and continuation
/// parameter shares: caller fields alongside the same empty qualification
/// roster.
pub(super) fn structural_parameter_declaration(
    place: PlaceId,
    position: u32,
    structural_type: StructuralTypeId,
    multiplicity: terminal_psi::StructuralMultiplicity,
    access: StructuralAccess,
) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place,
        position,
        structural_type,
        multiplicity,
        access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}
