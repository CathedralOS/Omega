//! One emitter for the Unit effect operations every attached body lowers the
//! same way: port writes, stores (`stores`: one lowering for every store
//! leaf), scalar locals, view-subslice locals, borrowed-storage windows,
//! call-continuation cleanup, structural-value construction, and every call
//! shape (`calls`: internal Unit and structural calls, member calls of a
//! construction and scalar calls; `boundary_calls`: the three boundary call
//! shapes). The ordinary machine (`ordinary_machine::MachineEmission`) and a
//! composed-graph state (`composed_control/emission.rs::emit_call_operations`)
//! each build an `OperationFrame` over their per-state environment and hand it
//! the checked operation; `OperationFrame::emit` and `OperationFrame::emit_call`
//! dispatch to the one method for that kind, which delegates the lowering
//! itself to the shared `crate::emission::*` and `ordinary_calls` helpers.
//! Assignment destination capture runs before those RHS operations. The shared
//! ledger retains its home and completed selectors for cleanup and stores, so
//! a later call cannot redirect an earlier selected assignment by changing an
//! index. Capturing coordinates introduces no loan or ownership transfer.
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
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::expression_preparation::bindings::structural_paths::lower_structural_path;
use crate::unit::{
    BoundaryMachineId, CheckedTrees, CheckedUnitEffectOperationPlan, LoweringError, Operation,
    OperationKind, OperationResult, PlaceId, ScalarType, ServiceId, ServiceReachId,
    StructuralAccess, StructuralArgument, StructuralDomainId, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeDeclaration, StructuralTypeId,
    ValueDeclaration, allocate_dense, lookup_type_id, place_id, unsupported,
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
mod loan_completions;
mod locals;
mod selection_retirement;
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
    pub(super) parameters: std::borrow::Cow<'f, [StructuralParameterDeclaration]>,
    pub(super) structural_types: StructuralTypeRoster<'f>,
    pub(super) type_ids: &'f [(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
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
                | CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { .. }
                )
                | CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::Structural { .. }
                )
                | CheckedUnitEffectOperationPlan::Store(_)
                | CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::Scalar { .. }
                )
                | CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::ScalarArray { .. }
                )
                | checked_trees::structural_field_write_pattern!()
                | CheckedUnitEffectOperationPlan::ContinuationCleanup { .. }
                | checked_trees::byte_sequence_establishment_pattern!()
                | checked_trees::reference_hold_pattern!()
                | CheckedUnitEffectOperationPlan::AtomicFence { .. }
                | CheckedUnitEffectOperationPlan::AtomicAccess(_)
        )
    }

    /// Emit one operation `lowers` admits.
    pub(super) fn emit(
        mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<(), LoweringError> {
        match operation {
            CheckedUnitEffectOperationPlan::CaptureAssignmentDestination {
                statement_index,
                destination,
            } => self.windows.capture_assignment_destination(
                identities,
                destination,
                &self.parameters,
                self.structural_types.declarations(),
                self.type_ids,
                &mut crate::emission::borrowed_window::WindowEvaluation {
                    checked: self.checked,
                    machine: self.machine,
                    state: self.state,
                    statement_index: *statement_index,
                    evaluation: self.evaluation,
                    source_value_count: self.source_value_count,
                    calls: self.calls,
                },
                self.values,
                self.next_value,
                self.next_block,
                self.next_edge,
                self.operations,
            ),
            CheckedUnitEffectOperationPlan::PortWrite {
                service_reach,
                port,
                value,
                ..
            } => self.port_write(service_reach, *port, *value),
            CheckedUnitEffectOperationPlan::Store(store) => self.store(identities, store),
            CheckedUnitEffectOperationPlan::EstablishLocal(binding) => {
                self.establish_local(identities, operation, binding)
            }
            CheckedUnitEffectOperationPlan::MoveStructuralField {
                result,
                source,
                expression,
            } => self.move_structural_field(identities, result, source, *expression),
            CheckedUnitEffectOperationPlan::ContinuationCleanup {
                boundary,
                cleanup_actions,
                selected_regions,
            } => self.continuation_cleanup(identities, boundary, cleanup_actions, selected_regions),
            CheckedUnitEffectOperationPlan::EstablishViewSubslice { .. } => {
                self.establish_view_subslice(identities, operation)
            }
            CheckedUnitEffectOperationPlan::EstablishReference {
                formation_expression,
                result,
                source,
                storage,
            } => self.establish_reference(
                identities,
                *formation_expression,
                result,
                source,
                *storage,
            ),
            CheckedUnitEffectOperationPlan::ReleaseReference { .. } => {
                self.release_reference(identities, operation)
            }
            CheckedUnitEffectOperationPlan::AtomicFence { ordering, .. } => {
                self.atomic_fence(*ordering)
            }
            CheckedUnitEffectOperationPlan::AtomicAccess(access) => {
                self.atomic_access(identities, access)
            }
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
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,

        formation_expression: checked_trees::expression::ExpressionHandle,
        result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
        source: &CheckedUnitStructuralArgumentPlan,
        storage: checked_trees::CheckedReferenceStorage,
    ) -> Result<(), LoweringError> {
        self.results
            .require_next(result, "reference result binding is not dense")?;
        let argument =
            if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::ScalarParameter {
                parameter_index,
            } = source.source
            {
                self.primitive_parameter_source(identities, parameter_index, source)?
            } else {
                let earlier = self.results.earlier(
                    std::slice::from_ref(source),
                    self.operations,
                    self.evaluation,
                )?;
                self.evaluation.prepare_immutable_local_borrow(
                    self.checked,
                    self.state,
                    source.structural_storage_root(),
                    self.values,
                    self.structural_types.declarations(),
                    self.next_place,
                    self.operations,
                    self.private_places,
                )?;
                let mut selectors = parameters::mint_runtime_index_selectors(
                    identities,
                    std::slice::from_ref(source),
                    &self.evaluation.structural_fields,
                    &self.evaluation.contextual_bindings(),
                    self.values,
                    self.next_value,
                    self.operations,
                )?
                .into_iter();
                parameters::lower_reference_source(
                    identities,
                    self.checked,
                    source,
                    self.structural_types.declarations(),
                    &self.parameters,
                    self.caller.local_places,
                    &earlier,
                    self.primitive_locals,
                    &self.evaluation.structural_locals,
                    Some(parameters::ReferenceSourceEvaluation {
                        values: self.values,
                        selectors: &mut selectors,
                        calls: self.calls,
                        next_value: self.next_value,
                        operations: self.operations,
                    }),
                )?
            };
        let (storage, value) =
            reference_results::publish_storage(storage, self.next_value, self.operations)?;
        if let Some(value) = value {
            self.values.push(value);
        }
        let declaration = reference_results::emit(
            reference_results::Formation {
                checked: self.checked,
                state: self.state,
                statement_index: result.statement_index as usize,
                expression: formation_expression,
            },
            result,
            argument,
            storage,
            self.type_ids,
            self.structural_types.declarations(),
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
            result,
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
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,

        parameter_index: u32,
        source: &CheckedUnitStructuralArgumentPlan,
    ) -> Result<StructuralArgument, LoweringError> {
        self.evaluation.prepare_borrowed_argument(
            identities,
            self.checked,
            self.state,
            source,
            self.values,
            self.structural_types.declarations(),
            self.next_place,
            self.operations,
            self.private_places,
        )?;
        let structural_type = lookup_type_id(self.type_ids, &source.type_identity)?;
        let scalar_type = self
            .structural_types
            .declarations()
            .iter()
            .find_map(|actual| {
                if actual.id == structural_type
                    && let terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar) = actual.shape
                {
                    Some(scalar)
                } else {
                    None
                }
            })
            .ok_or(LoweringError::Unsupported(
                "primitive reference ingress lost its declared view",
            ))?;
        let place = self
            .evaluation
            .scalar_bindings
            .scalar_parameter_home(parameter_index, scalar_type)?
            .ok_or(LoweringError::Unsupported(
                "primitive reference ingress lost its original home",
            ))?;
        Ok(
            crate::expression_preparation::bindings::structural_paths::structural_argument(
                place,
                Vec::new(),
                StructuralAccess::SharedBorrow,
            ),
        )
    }

    /// End the exact current carrier through the shared storage namespace.
    /// Replaced locals resolve their receiving home, not their birth result.
    fn release_reference(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,

        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<(), LoweringError> {
        let CheckedUnitEffectOperationPlan::ReleaseReference {
            source,
            path,
            select_evaluated_storage,
            selection_complements,
            ..
        } = operation
        else {
            unreachable!("dispatched release_reference")
        };
        let storage = self
            .evaluation
            .cleanup_storage(source, &self.parameters, |binding| {
                self.results.cleanup_value(binding, self.operations)
            })?;
        let suffix = lower_structural_path(identities, path)?;
        let types = self.structural_types.declarations();
        let leaf = terminal_semantics::runtime_structural_path_tip(
            types.iter(),
            storage.selected_type,
            &suffix,
        )
        .ok_or(LoweringError::Unsupported(
            "loan retirement lost its carrier",
        ))?;
        let source = storage.place;
        let mut path = storage.path;
        path.extend(suffix);
        match &leaf.shape {
            terminal_psi::StructuralTypeShape::Reference { .. } => {
                if *select_evaluated_storage {
                    self.evaluation.continue_selected_reference_release(
                        identities,
                        self.checked,
                        terminal_psi::StructuralArgument {
                            place: source,
                            path: path.clone(),
                            access: StructuralAccess::Owned,
                        },
                        self.values,
                        self.next_value,
                        self.next_block,
                        self.next_edge,
                        self.operations,
                    )?;
                } else {
                    self.push_unit(OperationKind::ReleaseReference {
                        source: self.evaluation.current_structural_place(source),
                        path: path.clone(),
                    });
                }
            }
            terminal_psi::StructuralTypeShape::ElementView { .. }
            | terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::byte_sequence_view_carrier_pattern!(),
            ) => {
                // Fixed and runtime-length descriptors retire through ordinary carrier cleanup.
                // Their captured loan boundary does not change their geometry
                // into a stored reference or consume their borrowed backing.
                self.evaluation.continue_cleanup_regions(
                    identities,
                    self.checked,
                    vec![crate::unit::unit_cleanup::CleanupRegion {
                        selected_region: select_evaluated_storage.then(|| {
                            terminal_psi::StructuralArgument {
                                place: source,
                                path: path.clone(),
                                access: StructuralAccess::Owned,
                            }
                        }),
                        conditions: Vec::new(),
                        action: if path.is_empty() {
                            self.evaluation
                                .descriptor_retirement(source, self.operations)?
                        } else {
                            terminal_psi::TerminalAffineCleanupAction::DiscardResidual(
                                terminal_psi::StructuralAffineDiscard {
                                    place: source,
                                    path: path.clone(),
                                    structural_type: leaf.id,
                                },
                            )
                        },
                    }],
                    self.values,
                    self.next_value,
                    self.next_block,
                    self.next_edge,
                    self.operations,
                )?;
            }
            _ => return unsupported("loan retirement requires a descriptor carrier"),
        }
        self.retire_selection_complements(identities, selection_complements)?;
        if path.is_empty() {
            // A whole descriptor release ends this physical owner. Keeping it
            // in the selection frontier would move a dead parent on the next
            // private edge; projected releases retain the owner's siblings.
            self.evaluation.retire_structural_home(source);
        }
        Ok(())
    }

    /// Mint runtime index selectors for `arguments`, then lower the
    /// structural argument roster against the caller's completed and
    /// declared places.
    pub(super) fn lower_runtime_structural_arguments(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        arguments: &[CheckedUnitStructuralArgumentPlan],
        earlier: &[(Option<StructuralPlaceDeclaration>, bool)],
        byte_places: &[PlaceId],
        evaluated_selectors: Option<&[ValueDeclaration]>,
    ) -> Result<Vec<StructuralArgument>, LoweringError> {
        let mut selector_values =
            parameters::call_index_selectors(arguments, evaluated_selectors)?.into_iter();
        parameters::lower_structural_arguments(
            identities,
            self.checked,
            arguments,
            self.structural_types.declarations(),
            &self.parameters,
            &[],
            earlier,
            &self.operations.structural_values.clone(),
            byte_places,
            self.primitive_locals,
            &self.evaluation.structural_locals,
            Some(&self.evaluation.scalar_bindings),
            Some(self.values),
            &mut selector_values,
            Some(&mut *self.calls),
            Some((self.next_value, self.operations)),
        )
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

/// The owned argument a store's repair window names: the store's path below
/// the window root, under the hole's declared type identity.
fn window_place(
    source: checked_trees::CheckedUnitStructuralArgumentSourcePlan,
    path: &[checked_trees::CheckedUnitStructuralPathSegment],
    type_identity: typed_trees::type_identity::TypeIdentityId,
) -> CheckedUnitStructuralArgumentPlan {
    CheckedUnitStructuralArgumentPlan {
        source,
        path: path.to_vec(),
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
