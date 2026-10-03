//! Verifier-owned reconstruction of the first bounded D39 observation profile.

use semantic_vocabulary::{
    BlockId, EdgeId, IntegerSign, IntegerType, MachineId, OperationId, ScalarType,
};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, CrashCause, OperationKind, StructuralAccess,
    TerminalMachineResult, TerminalModule, TerminalObservationSchema,
    TerminalOperationCrashDenotation, TerminalTraceBoundaryCrashSiteRow, TerminalTraceCrashSiteRow,
    TerminalTraceOperationCrashSiteRow, TerminalTraceOrdinaryEventKind,
    TerminalTraceOrdinaryEventRow, TerminalTraceResultSchema, TerminalTraceRootRow,
    TerminalTraceScalarSchema, TerminalTraceStructuralSchema, TerminalTraceV1Rows,
    TerminalTraceValueComparison, Terminator,
};

use crate::{ModuleError, validate_module_representation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalTraceV1OperationClassification {
    Internal,
    /// An internal operation that owns an operation-level crash site.
    TrappingPrimitive,
    BoundaryCall,
    PortWrite,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalTraceV1ReconstructionError {
    InvalidModule(ModuleError),
    MissingEntry(MachineId),
    DuplicateCrashSite {
        machine: MachineId,
        block: BlockId,
        edge: EdgeId,
    },
    DuplicateBoundaryCrashSite {
        machine: MachineId,
        block: BlockId,
        operation: OperationId,
        cause: CrashCause,
    },
    DuplicateOrdinaryEventSite {
        machine: MachineId,
        block: BlockId,
        operation: OperationId,
    },
    DuplicateOperationCrashSite {
        machine: MachineId,
        block: BlockId,
        operation: OperationId,
    },
}

impl std::fmt::Display for TerminalTraceV1ReconstructionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TerminalTraceV1ReconstructionError {}

impl From<ModuleError> for TerminalTraceV1ReconstructionError {
    fn from(error: ModuleError) -> Self {
        Self::InvalidModule(error)
    }
}

/// Reconstruct the consumer-selected known row set from exact Terminal
/// semantics. The canonical codec owner separately binds these rows to the
/// exact module identity; no producer-supplied row or weakening flag enters
/// this API.
pub fn reconstruct_terminal_observation_profile_rows(
    schema: TerminalObservationSchema,
    module: &TerminalModule,
) -> Result<TerminalTraceV1Rows, TerminalTraceV1ReconstructionError> {
    match schema {
        TerminalObservationSchema::TerminalTraceV1 => reconstruct_terminal_trace_v1_rows(module),
    }
}

pub fn reconstruct_terminal_trace_v1_rows(
    module: &TerminalModule,
) -> Result<TerminalTraceV1Rows, TerminalTraceV1ReconstructionError> {
    validate_module_representation(module)?;

    // V1 terminator crash rows identify edges only. An operation-level boundary
    // crash must not disappear from a supposedly complete observer roster, so
    // each declared route rides on its own call-site row below rather than
    // inventing an edge for the invocation.
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .ok_or(TerminalTraceV1ReconstructionError::MissingEntry(
            module.entry,
        ))?;

    let exact = TerminalTraceValueComparison::ExactSemanticValue;
    let scalar_inputs = entry
        .parameters
        .iter()
        .map(|parameter| TerminalTraceScalarSchema {
            scalar_type: parameter.scalar_type,
            comparison: exact,
        })
        .collect();
    let structural_inputs = entry
        .structural_parameters
        .iter()
        .map(|parameter| TerminalTraceStructuralSchema {
            structural_type: parameter.structural_type,
            multiplicity: parameter.multiplicity,
            access: parameter.access,
            qualifications: parameter.qualifications.clone(),
            projected_qualifications: parameter.projected_qualifications.clone(),
            comparison: exact,
        })
        .collect();
    let result = match &entry.result {
        TerminalMachineResult::Unit => TerminalTraceResultSchema::Unit,
        TerminalMachineResult::Scalar(result) => {
            TerminalTraceResultSchema::Scalar(TerminalTraceScalarSchema {
                scalar_type: result.scalar_type,
                comparison: exact,
            })
        }
        TerminalMachineResult::Structural(result) => {
            TerminalTraceResultSchema::Structural(TerminalTraceStructuralSchema {
                structural_type: result.structural_type,
                multiplicity: result.multiplicity,
                access: StructuralAccess::Owned,
                qualifications: result.qualifications.clone(),
                projected_qualifications: Vec::new(),
                comparison: exact,
            })
        }
    };

    let mut crash_sites = Vec::new();
    let mut boundary_crash_sites = Vec::new();
    let mut operation_crash_sites = Vec::new();
    let mut ordinary_events = Vec::new();
    for machine in &module.machines {
        let mut value_types = None;
        for block in &machine.blocks {
            for operation in &block.operations {
                match classify_operation(&operation.kind) {
                    TerminalTraceV1OperationClassification::Internal => {}
                    TerminalTraceV1OperationClassification::TrappingPrimitive => {
                        // The operation is its own site: no edge or boundary
                        // identity is fabricated. The denotation is
                        // reconstructed from the validated value types, not
                        // read from a producer row.
                        let value_types = value_types.get_or_insert_with(|| {
                            crate::validation::machine_value_types(machine)
                                .collect::<std::collections::BTreeMap<_, _>>()
                        });
                        let denotation = match operation.kind {
                            OperationKind::FloatResultPolicy { .. } => {
                                TerminalOperationCrashDenotation::FloatNonFinite {
                                    format: crate::validation::float_result_policy::format(
                                        operation,
                                    )?,
                                }
                            }
                            _ => {
                                let integer = crate::validation::trapping_integer::trapping_integer_denotation(
                                    operation,
                                    value_types,
                                )?;
                                TerminalOperationCrashDenotation::Integer {
                                    primitive: integer.primitive,
                                    result_type: integer.result_type,
                                    operand_type: integer.operand_type,
                                }
                            }
                        };
                        operation_crash_sites.push(TerminalTraceOperationCrashSiteRow {
                            machine: machine.id,
                            block: block.id,
                            operation: operation.id,
                            cause: CrashCause::Trap,
                            denotation,
                        });
                    }
                    TerminalTraceV1OperationClassification::BoundaryCall => {
                        let declaration = boundary_declaration(module, operation);
                        ordinary_events.push(reconstruct_boundary_call_event(
                            machine.id,
                            block.id,
                            operation,
                            declaration,
                        ));
                        // The declared ceiling is observed at every call to it:
                        // one row per (call site, cause bucket), retaining the
                        // declaration's own guards for a later per-invocation
                        // substitution join.
                        for route in &declaration.crash_routes {
                            boundary_crash_sites.push(TerminalTraceBoundaryCrashSiteRow {
                                machine: machine.id,
                                block: block.id,
                                operation: operation.id,
                                boundary: declaration.id,
                                boundary_identity: declaration.identity.clone(),
                                route: route.clone(),
                            });
                        }
                    }
                    TerminalTraceV1OperationClassification::PortWrite => ordinary_events.push(
                        reconstruct_port_write_event(module, machine.id, block.id, operation),
                    ),
                }
            }
            if let Terminator::Crash { edge, cause, .. } = block.terminator {
                crash_sites.push(TerminalTraceCrashSiteRow {
                    machine: machine.id,
                    block: block.id,
                    edge,
                    cause,
                });
            }
        }
    }
    crash_sites.sort_unstable_by_key(crash_site_key);
    if let Some(rows) = crash_sites
        .windows(2)
        .find(|rows| crash_site_key(&rows[0]) == crash_site_key(&rows[1]))
    {
        let duplicate = rows[0];
        return Err(TerminalTraceV1ReconstructionError::DuplicateCrashSite {
            machine: duplicate.machine,
            block: duplicate.block,
            edge: duplicate.edge,
        });
    }
    boundary_crash_sites.sort_unstable_by_key(boundary_crash_site_key);
    if let Some(rows) = boundary_crash_sites
        .windows(2)
        .find(|rows| boundary_crash_site_key(&rows[0]) == boundary_crash_site_key(&rows[1]))
    {
        let duplicate = &rows[0];
        return Err(
            TerminalTraceV1ReconstructionError::DuplicateBoundaryCrashSite {
                machine: duplicate.machine,
                block: duplicate.block,
                operation: duplicate.operation,
                cause: duplicate.route.cause,
            },
        );
    }
    operation_crash_sites.sort_unstable_by_key(operation_crash_site_key);
    if let Some(rows) = operation_crash_sites
        .windows(2)
        .find(|rows| operation_crash_site_key(&rows[0]) == operation_crash_site_key(&rows[1]))
    {
        let duplicate = rows[0];
        return Err(
            TerminalTraceV1ReconstructionError::DuplicateOperationCrashSite {
                machine: duplicate.machine,
                block: duplicate.block,
                operation: duplicate.operation,
            },
        );
    }
    ordinary_events.sort_unstable_by_key(ordinary_event_site_key);
    if let Some(rows) = ordinary_events
        .windows(2)
        .find(|rows| ordinary_event_site_key(&rows[0]) == ordinary_event_site_key(&rows[1]))
    {
        let duplicate = &rows[0];
        return Err(
            TerminalTraceV1ReconstructionError::DuplicateOrdinaryEventSite {
                machine: duplicate.machine,
                block: duplicate.block,
                operation: duplicate.operation,
            },
        );
    }

    Ok(TerminalTraceV1Rows {
        root: TerminalTraceRootRow {
            entry: entry.id,
            scalar_inputs,
            structural_inputs,
            result,
        },
        crash_sites,
        boundary_crash_sites,
        operation_crash_sites,
        ordinary_events,
    })
}

fn operation_crash_site_key(
    row: &TerminalTraceOperationCrashSiteRow,
) -> (MachineId, BlockId, OperationId) {
    (row.machine, row.block, row.operation)
}

fn crash_site_key(row: &TerminalTraceCrashSiteRow) -> (MachineId, BlockId, EdgeId) {
    (row.machine, row.block, row.edge)
}

fn boundary_crash_site_key(
    row: &TerminalTraceBoundaryCrashSiteRow,
) -> (MachineId, BlockId, OperationId, CrashCause) {
    (row.machine, row.block, row.operation, row.route.cause)
}

fn ordinary_event_site_key(
    row: &TerminalTraceOrdinaryEventRow,
) -> (MachineId, BlockId, OperationId) {
    (row.machine, row.block, row.operation)
}

/// This intentionally has no wildcard arm. Adding a Terminal operation cannot
/// silently inherit an observation classification.
fn classify_operation(kind: &OperationKind) -> TerminalTraceV1OperationClassification {
    match kind {
        OperationKind::BoundaryCall { .. } => TerminalTraceV1OperationClassification::BoundaryCall,
        OperationKind::PortWrite { .. } => TerminalTraceV1OperationClassification::PortWrite,
        OperationKind::TrappingInteger { .. } => {
            TerminalTraceV1OperationClassification::TrappingPrimitive
        }
        OperationKind::FloatResultPolicy { policy } => {
            if policy.can_trap() {
                TerminalTraceV1OperationClassification::TrappingPrimitive
            } else {
                TerminalTraceV1OperationClassification::Internal
            }
        }
        // An atomic event is program-internal memory traffic: the v1 profile
        // observes boundary and port effects, not memory-order events, and a
        // serial trace has no second participant that could observe one.
        OperationKind::AtomicAccess { .. } => TerminalTraceV1OperationClassification::Internal,
        OperationKind::WriteOnlyPrimitiveStore { .. }
        | OperationKind::EstablishReference { .. }
        | OperationKind::ReleaseReference { .. }
        | OperationKind::EstablishPrimitiveLocal { .. }
        | OperationKind::PrimitiveScalarRead { .. }
        | OperationKind::StructuralScalarFieldStore { .. }
        | OperationKind::ByteSequenceStore { .. }
        | OperationKind::StructuralByteSequenceFieldStore { .. }
        | OperationKind::StructuralByteViewFieldStore { .. }
        | OperationKind::StructuralByteSequenceFieldLength { .. }
        | OperationKind::StructuralByteSequenceFieldByteStore { .. }
        | OperationKind::StructuralByteSequenceFieldRead { .. }
        | OperationKind::EstablishScalarCase { .. }
        | OperationKind::EstablishStructuralCase { .. }
        | OperationKind::EstablishScalarArray { .. }
        | OperationKind::EstablishZeroedStructural { .. }
        | OperationKind::EstablishStructuralLocal { .. }
        | OperationKind::EstablishByteSequenceConcatenation { .. }
        | OperationKind::EstablishByteSequenceLiteral { .. }
        | OperationKind::ByteSequenceLength { .. }
        | OperationKind::ByteSequenceRead { .. }
        | OperationKind::ByteSequenceWrite { .. }
        | OperationKind::ByteSequenceSubslice { .. }
        | OperationKind::ByteSequencePredicate { .. }
        | OperationKind::ByteSequenceEqual { .. }
        | OperationKind::EstablishElementView { .. }
        | OperationKind::EstablishMutableElementView { .. }
        | OperationKind::ElementViewLength { .. }
        | OperationKind::ElementViewRead { .. }
        | OperationKind::ElementViewWrite { .. }
        | OperationKind::ElementViewSubslice { .. }
        | OperationKind::EstablishTrivialAffineLocal { .. }
        | OperationKind::EstablishStructuralArray { .. }
        | OperationKind::EstablishRecord { .. }
        | OperationKind::StoreDynamicDescriptor { .. }
        | OperationKind::Call { .. }
        | OperationKind::CallUnit { .. }
        | OperationKind::CallStructuralScalar { .. }
        | OperationKind::CallDynamicScalar { .. }
        | OperationKind::CallDynamicParameterScalar { .. }
        | OperationKind::CallDynamicUnit { .. }
        | OperationKind::CallDynamicParameterUnit { .. }
        | OperationKind::CallStructural { .. }
        | OperationKind::CallStructuralWithScalarArguments { .. }
        | OperationKind::IntegerConstant { .. }
        | OperationKind::BooleanConstant { .. }
        | OperationKind::IeeeFloatConstant { .. }
        | OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. }
        | OperationKind::IeeeFloatCompare { .. }
        | OperationKind::ScalarRepresentationView { .. }
        | OperationKind::BooleanStructuralField { .. }
        | OperationKind::StructuralCaseMembership { .. }
        | OperationKind::StructuralLeafCopy { .. }
        | OperationKind::StructuralCaseLeafCopy { .. }
        | OperationKind::IntegerStructuralField { .. }
        | OperationKind::MoveStructuralField { .. }
        | OperationKind::StoreStructuralField { .. }
        | OperationKind::BooleanNot { .. }
        | OperationKind::BooleanEqual { .. }
        | OperationKind::IntegerEqual { .. }
        | OperationKind::IntegerLessThan { .. }
        | OperationKind::IntegerLessOrEqual { .. }
        | OperationKind::IntegerBitwiseNot { .. }
        | OperationKind::IntegerWiden { .. }
        | OperationKind::IntegerExactCast { .. }
        | OperationKind::IntegerBitwiseAnd { .. }
        | OperationKind::IntegerBitwiseOr { .. }
        | OperationKind::IntegerBitwiseXor { .. }
        | OperationKind::WrappingIntegerShiftLeft { .. }
        | OperationKind::WrappingIntegerShiftRight { .. }
        | OperationKind::ExactIntegerShiftLeft { .. }
        | OperationKind::ExactIntegerShiftRight { .. }
        | OperationKind::ExactIntegerAdd { .. }
        | OperationKind::ExactIntegerSubtract { .. }
        | OperationKind::ExactIntegerMultiply { .. }
        | OperationKind::ExactIntegerDivide { .. }
        | OperationKind::ExactIntegerRemainder { .. }
        | OperationKind::WrappingIntegerDivide { .. }
        | OperationKind::WrappingIntegerRemainder { .. }
        | OperationKind::SaturatingIntegerDivide { .. }
        | OperationKind::SaturatingIntegerRemainder { .. }
        | OperationKind::WrappingIntegerAdd { .. }
        | OperationKind::SaturatingIntegerAdd { .. }
        | OperationKind::WrappingIntegerSubtract { .. }
        | OperationKind::SaturatingIntegerSubtract { .. }
        | OperationKind::WrappingIntegerMultiply { .. }
        | OperationKind::SaturatingIntegerMultiply { .. } => {
            TerminalTraceV1OperationClassification::Internal
        }
    }
}

fn boundary_declaration<'a>(
    module: &'a TerminalModule,
    operation: &terminal_psi::Operation,
) -> &'a BoundaryMachineDeclaration {
    let OperationKind::BoundaryCall { boundary, .. } = &operation.kind else {
        unreachable!("classification selected a boundary call")
    };
    module
        .boundary_machines
        .iter()
        .find(|declaration| declaration.id == *boundary)
        .expect("module validation established the boundary declaration")
}

// The call's completion receipts are settlement custody validated by
// `validate_module`, not runtime values: the observation schema describes the
// values a semantic trace compares, so the replayed row carries only the
// declared argument and result schemas. Receipts have no channel to leak into.
fn reconstruct_boundary_call_event(
    machine: MachineId,
    block: BlockId,
    operation: &terminal_psi::Operation,
    declaration: &BoundaryMachineDeclaration,
) -> TerminalTraceOrdinaryEventRow {
    let boundary = declaration.id;
    let exact = TerminalTraceValueComparison::ExactSemanticValue;
    TerminalTraceOrdinaryEventRow {
        machine,
        block,
        operation: operation.id,
        kind: TerminalTraceOrdinaryEventKind::BoundaryCall {
            boundary,
            boundary_identity: declaration.identity.clone(),
        },
        scalar_arguments: declaration
            .scalar_parameters
            .iter()
            .map(|scalar_type| TerminalTraceScalarSchema {
                scalar_type: *scalar_type,
                comparison: exact,
            })
            .collect(),
        structural_arguments: declaration
            .structural_parameters
            .iter()
            .map(|parameter| TerminalTraceStructuralSchema {
                structural_type: parameter.structural_type,
                multiplicity: parameter.multiplicity,
                access: parameter.access,
                qualifications: parameter.qualifications.clone(),
                projected_qualifications: parameter.projected_qualifications.clone(),
                comparison: exact,
            })
            .collect(),
        result: match &declaration.result {
            BoundaryMachineResult::Unit => TerminalTraceResultSchema::Unit,
            BoundaryMachineResult::Scalar(scalar_type) => {
                TerminalTraceResultSchema::Scalar(TerminalTraceScalarSchema {
                    scalar_type: *scalar_type,
                    comparison: exact,
                })
            }
            BoundaryMachineResult::Structural(result) => {
                TerminalTraceResultSchema::Structural(TerminalTraceStructuralSchema {
                    structural_type: result.structural_type,
                    multiplicity: result.multiplicity,
                    access: StructuralAccess::Owned,
                    qualifications: result.qualifications.clone(),
                    projected_qualifications: Vec::new(),
                    comparison: exact,
                })
            }
        },
    }
}

fn reconstruct_port_write_event(
    module: &TerminalModule,
    machine: MachineId,
    block: BlockId,
    operation: &terminal_psi::Operation,
) -> TerminalTraceOrdinaryEventRow {
    let OperationKind::PortWrite { service, .. } = operation.kind else {
        unreachable!("classification selected a port write")
    };
    let declaration = module
        .services
        .iter()
        .find(|declaration| declaration.id == service)
        .expect("module validation established the service declaration");
    let exact = TerminalTraceValueComparison::ExactSemanticValue;
    TerminalTraceOrdinaryEventRow {
        machine,
        block,
        operation: operation.id,
        kind: TerminalTraceOrdinaryEventKind::PortWrite {
            service,
            service_identity: declaration.identity.clone(),
        },
        scalar_arguments: [16, 8]
            .map(|bits| TerminalTraceScalarSchema {
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, bits)
                        .expect("u16 and u8 are valid Terminal integer types"),
                ),
                comparison: exact,
            })
            .into(),
        structural_arguments: Vec::new(),
        result: TerminalTraceResultSchema::Unit,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        IntegerSign, IntegerType, OperationId, StructuralAccess, TerminalTraceCrashSiteRow,
        TerminalTraceOrdinaryEventKind, TerminalTraceResultSchema, TerminalTraceScalarSchema,
        TerminalTraceV1OperationClassification, TerminalTraceValueComparison, classify_operation,
        reconstruct_terminal_trace_v1_rows,
    };
    use semantic_vocabulary::{
        BlockId, BoundaryMachineId, ContractId, EdgeId, MachineId, PlaceId, ScalarType, ServiceId,
        StructuralPlaceKind, StructuralTypeId, ValueId,
    };
    use terminal_psi::{
        Block, BoundaryMachineDeclaration, CrashCause, CrashRouteBucket, CrashRouteGuard,
        MachineContract, Operation, OperationKind, OperationResult, ServiceDeclaration,
        StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
        StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
        TerminalMachine, TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
        VocabularyMarker,
    };

    fn id<T>(raw: u64, make: impl FnOnce(u64) -> Option<T>) -> T {
        make(raw).expect("nonzero test identity")
    }

    fn module(terminator: Terminator, crash_routes: Vec<CrashRouteBucket>) -> TerminalModule {
        let machine = id(1, MachineId::new);
        let block = id(1, BlockId::new);
        TerminalModule {
            scalar_qualifications: Default::default(),
            scalar_block_invariants: Vec::new(),
            operation_crash_contracts: Vec::new(),
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine,
            structural_types: Vec::new(),
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
                id: machine,
                attachment: None,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                ranked_scc: None,
                result: TerminalMachineResult::Unit,
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block,
                blocks: vec![Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    id: block,
                    structural_parameters: Vec::new(),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator,
                }],
                contract: MachineContract {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    id: id(1, ContractId::new),
                    crash_routes,
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        }
    }

    #[test]
    fn reconstructs_mandatory_root_and_exact_crash_site() {
        let cause = CrashCause::Abort;
        let edge = id(1, EdgeId::new);
        let module = module(
            Terminator::Crash {
                edge,
                cause,
                site_guard: Vec::new(),
                frontier_lower_bound: Vec::new(),
            },
            vec![CrashRouteBucket {
                cause,
                alternatives: vec![CrashRouteGuard::Truth],
            }],
        );
        let rows = reconstruct_terminal_trace_v1_rows(&module)
            .expect("valid internal crash module reconstructs");

        assert_eq!(rows.root.entry, module.entry);
        assert_eq!(rows.root.result, TerminalTraceResultSchema::Unit);
        assert_eq!(
            rows.crash_sites,
            [TerminalTraceCrashSiteRow {
                machine: module.entry,
                block: id(1, BlockId::new),
                edge,
                cause,
            }],
        );
    }

    #[test]
    fn reconstructs_ordered_scalar_input_and_exact_result_schema() {
        let parameter = ValueDeclaration {
            qualifications: Default::default(),
            id: id(2, ValueId::new),
            scalar_type: ScalarType::Boolean,
        };
        let result = ValueDeclaration {
            qualifications: Default::default(),
            id: id(3, ValueId::new),
            scalar_type: ScalarType::Boolean,
        };
        let mut module = module(
            Terminator::Return {
                edge: id(2, EdgeId::new),
                value: parameter.id,
                cleanup_actions: Vec::new(),
            },
            Vec::new(),
        );
        module.machines[0].parameters = vec![parameter];
        module.machines[0].result = TerminalMachineResult::Scalar(result);

        let rows =
            reconstruct_terminal_trace_v1_rows(&module).expect("scalar root schema reconstructs");
        let exact = TerminalTraceValueComparison::ExactSemanticValue;
        assert_eq!(
            rows.root.scalar_inputs,
            [TerminalTraceScalarSchema {
                scalar_type: ScalarType::Boolean,
                comparison: exact,
            }],
        );
        assert_eq!(
            rows.root.result,
            TerminalTraceResultSchema::Scalar(TerminalTraceScalarSchema {
                scalar_type: ScalarType::Boolean,
                comparison: exact,
            }),
        );
    }

    #[test]
    fn external_event_operations_have_explicit_observable_classifications() {
        let boundary = OperationKind::BoundaryCall {
            requirement_obligations: Vec::new(),
            boundary: id(1, BoundaryMachineId::new),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
        };
        assert_eq!(
            classify_operation(&boundary),
            TerminalTraceV1OperationClassification::BoundaryCall,
        );

        let port = OperationKind::PortWrite {
            service: id(1, ServiceId::new),
            port: 0x3f8,
            value: 1,
        };
        assert_eq!(
            classify_operation(&port),
            TerminalTraceV1OperationClassification::PortWrite,
        );

        assert_eq!(
            classify_operation(&OperationKind::BooleanConstant { value: true }),
            TerminalTraceV1OperationClassification::Internal,
        );
    }

    fn float_policy_module(
        format: semantic_vocabulary::IeeeFloatFormat,
        policy: terminal_psi::FloatResultPolicy,
    ) -> TerminalModule {
        let mut module = module(
            Terminator::ReturnUnit {
                edge: id(1, EdgeId::new),
                trivial_affine_discards: Vec::new(),
            },
            if policy.can_trap() {
                vec![CrashRouteBucket {
                    cause: CrashCause::Trap,
                    alternatives: vec![CrashRouteGuard::Truth],
                }]
            } else {
                Vec::new()
            },
        );
        let declaration = |ordinal| ValueDeclaration {
            id: id(ordinal, ValueId::new),
            scalar_type: ScalarType::IeeeFloat(format),
            qualifications: Default::default(),
        };
        module.machines[0].parameters = (1..=3).map(declaration).collect();
        module.machines[0].blocks[0].operations = vec![Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(1, OperationId::new),
            result: OperationResult::Scalar(declaration(4)),
            kind: OperationKind::FloatResultPolicy { policy },
        }];
        module
    }

    #[test]
    fn float_result_policy_validates_payloads_and_owns_only_its_trap_site() {
        use semantic_vocabulary::IeeeFloatFormat;
        use terminal_psi::{
            FloatResultPolicy, FloatSaturationRule, TerminalOperationCrashDenotation,
        };
        let policies = [
            FloatResultPolicy::Trapping {
                raw_result: id(3, ValueId::new),
            },
            FloatResultPolicy::Saturating {
                raw_result: id(3, ValueId::new),
                left: id(1, ValueId::new),
                right: id(2, ValueId::new),
                rule: FloatSaturationRule::FiniteOperandOverflow,
            },
            FloatResultPolicy::Saturating {
                raw_result: id(3, ValueId::new),
                left: id(1, ValueId::new),
                right: id(2, ValueId::new),
                rule: FloatSaturationRule::FiniteDivisionOverflow,
            },
        ];
        for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
            for policy in policies {
                let module = float_policy_module(format, policy);
                // The result is deliberately discarded. Trapping remains an
                // observable operation even with no later scalar consumer.
                let rows = reconstruct_terminal_trace_v1_rows(&module).unwrap();
                assert!(rows.crash_sites.is_empty());
                assert!(rows.boundary_crash_sites.is_empty());
                assert!(rows.ordinary_events.is_empty());
                assert_eq!(
                    rows.operation_crash_sites.len(),
                    usize::from(policy.can_trap())
                );
                if policy.can_trap() {
                    let row = rows.operation_crash_sites[0];
                    assert_eq!(
                        (row.machine, row.block, row.operation),
                        (module.entry, id(1, BlockId::new), id(1, OperationId::new))
                    );
                    assert_eq!(row.cause, CrashCause::Trap);
                    assert_eq!(
                        row.denotation,
                        TerminalOperationCrashDenotation::FloatNonFinite { format }
                    );
                    let mut missing_ceiling = module.clone();
                    missing_ceiling.machines[0].contract.crash_routes.clear();
                    assert!(matches!(
                        crate::validate_module_representation(&missing_ceiling),
                        Err(crate::ModuleError::FloatResultPolicyCrashUncovered { .. })
                    ));
                }
                for operand in policy.operands() {
                    let mut wrong_type = module.clone();
                    wrong_type.machines[0].parameters[(operand.get() - 1) as usize].scalar_type =
                        ScalarType::Boolean;
                    assert!(
                        matches!(crate::validate_module_representation(&wrong_type), Err(crate::ModuleError::FloatResultPolicyOperandTypeMismatch { operand: rejected, .. }) if rejected == operand)
                    );

                    let mut wrong_format = module.clone();
                    wrong_format.machines[0].parameters[(operand.get() - 1) as usize].scalar_type =
                        ScalarType::IeeeFloat(match format {
                            IeeeFloatFormat::Binary32 => IeeeFloatFormat::Binary64,
                            IeeeFloatFormat::Binary64 => IeeeFloatFormat::Binary32,
                        });
                    assert!(
                        matches!(crate::validate_module_representation(&wrong_format), Err(crate::ModuleError::FloatResultPolicyOperandTypeMismatch { operand: rejected, .. }) if rejected == operand)
                    );

                    let mut undefined = module.clone();
                    undefined.machines[0]
                        .parameters
                        .retain(|parameter| parameter.id != operand);
                    assert!(crate::validate_module_representation(&undefined).is_err());
                }
                let mut wrong_result = module;
                wrong_result.machines[0].blocks[0].operations[0].result =
                    OperationResult::Scalar(ValueDeclaration {
                        id: id(4, ValueId::new),
                        scalar_type: ScalarType::Boolean,
                        qualifications: Default::default(),
                    });
                assert!(matches!(
                    crate::validate_module_representation(&wrong_result),
                    Err(crate::ModuleError::FloatResultPolicyRequiresFloatResult(_))
                ));
            }
        }
    }

    #[test]
    fn float_result_policy_crash_coverage_cannot_use_its_own_normal_return() {
        use semantic_vocabulary::{IeeeFloatFormat, Proposition, ScalarTerm};
        use terminal_psi::{CrashPredicateTerm, FloatResultPolicy};
        let mut module = float_policy_module(
            IeeeFloatFormat::Binary64,
            FloatResultPolicy::Trapping {
                raw_result: id(3, ValueId::new),
            },
        );
        let flag = id(5, ValueId::new);
        module.machines[0].parameters.push(ValueDeclaration {
            id: flag,
            scalar_type: ScalarType::Boolean,
            qualifications: Default::default(),
        });
        let guard = Proposition::Equal(
            ScalarTerm::value(flag, ScalarType::Boolean),
            ScalarTerm::Boolean(true),
        );
        module.machines[0].contract.crash_routes[0].alternatives =
            vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
                guard.clone(),
            ))];
        let questions = crate::reconstruct_crash_obligations(&module).unwrap();
        assert_eq!(questions.len(), 1);
        let crate::CrashObligationQuestion::Site { guards, paths } = &questions[0].question else {
            panic!("policy owns an operation crash site");
        };
        assert_eq!(guards, std::slice::from_ref(&guard));
        assert_eq!(paths.len(), 1);
        let operation = &module.machines[0].blocks[0].operations[0];
        let failure = crate::validation::float_result_policy::failure_condition(
            operation,
            id(3, ValueId::new),
        )
        .unwrap();
        assert!(paths[0].contains(&failure));
        assert!(
            !paths[0].contains(&guard),
            "published guard is a goal, never an axiom"
        );
        for normal_fact in crate::validation::float_result_policy::normal_return_facts(
            operation,
            id(3, ValueId::new),
        )
        .unwrap()
        {
            assert!(
                !paths[0].contains(&normal_fact),
                "normal continuation cannot justify its own trap"
            );
        }
    }

    #[test]
    fn reconstructs_complete_canonically_ordered_ordinary_event_roster() {
        let mut module = module(
            Terminator::ReturnUnit {
                edge: id(2, EdgeId::new),
                trivial_affine_discards: Vec::new(),
            },
            Vec::new(),
        );
        let service = id(1, ServiceId::new);
        let boundary = id(1, BoundaryMachineId::new);
        let argument = id(1, ValueId::new);
        let first_type = id(1, StructuralTypeId::new);
        let second_type = id(2, StructuralTypeId::new);
        let first_place = id(1, PlaceId::new);
        let second_place = id(2, PlaceId::new);
        let u8_type =
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 test type"));
        module.structural_types = vec![
            StructuralTypeDeclaration {
                id: first_type,
                identity: "Console::Message".into(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
            StructuralTypeDeclaration {
                id: second_type,
                identity: "Console::Context".into(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
        ];
        module.services = vec![ServiceDeclaration {
            id: service,
            identity: "PortIo::write-byte".into(),
            parents: Vec::new(),
        }];
        module.root_service_reach.concrete = vec![service];
        module.boundary_machines = vec![BoundaryMachineDeclaration {
            unconditional_termination: false,
            completion: terminal_psi::BoundaryCompletion::NormalReturn,
            kind: terminal_psi::BoundaryRequirementKind::Trait,
            scalar_ensures: Vec::new(),
            scalar_requires: Vec::new(),
            fixed_service_reach: Vec::new(),
            crash_routes: Vec::new(),
            id: boundary,
            identity: "Console::publish".into(),
            attachment: None,
            parameter_order: vec![
                terminal_psi::BoundaryParameterKind::Scalar,
                terminal_psi::BoundaryParameterKind::Scalar,
                terminal_psi::BoundaryParameterKind::Structural,
                terminal_psi::BoundaryParameterKind::Structural,
            ],
            scalar_parameters: vec![ScalarType::Boolean, u8_type],
            structural_parameters: vec![
                StructuralParameterDeclaration {
                    place: id(3, PlaceId::new),
                    position: 0,
                    is_self: false,
                    structural_type: first_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: StructuralAccess::Owned,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                },
                StructuralParameterDeclaration {
                    place: id(4, PlaceId::new),
                    position: 1,
                    is_self: false,
                    structural_type: second_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: StructuralAccess::SharedBorrow,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                },
            ],
            result: terminal_psi::BoundaryMachineResult::Scalar(u8_type),
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        }];
        module.machines[0].parameters = vec![
            ValueDeclaration {
                qualifications: Default::default(),
                id: argument,
                scalar_type: ScalarType::Boolean,
            },
            ValueDeclaration {
                qualifications: Default::default(),
                id: id(2, ValueId::new),
                scalar_type: u8_type,
            },
        ];
        module.machines[0].structural_parameters = vec![
            StructuralParameterDeclaration {
                place: first_place,
                position: 0,
                is_self: false,
                structural_type: first_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::Owned,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            },
            StructuralParameterDeclaration {
                place: second_place,
                position: 1,
                is_self: false,
                structural_type: second_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            },
        ];
        module.machines[0].structural_places = vec![
            StructuralPlaceDeclaration {
                id: first_place,
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            },
            StructuralPlaceDeclaration {
                id: second_place,
                kind: StructuralPlaceKind::Parameter {
                    position: 1,
                    is_self: false,
                },
            },
        ];
        module.machines[0].published_service_ceiling = vec![service];
        module.machines[0].blocks[0].operations = vec![
            Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: id(2, OperationId::new),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: id(3, ValueId::new),
                    scalar_type: u8_type,
                }),
                kind: OperationKind::BoundaryCall {
                    requirement_obligations: Vec::new(),
                    boundary,
                    arguments: vec![argument, id(2, ValueId::new)],
                    structural_arguments: vec![
                        StructuralArgument {
                            place: first_place,
                            path: Vec::new(),
                            access: StructuralAccess::Owned,
                        },
                        StructuralArgument {
                            place: second_place,
                            path: Vec::new(),
                            access: StructuralAccess::SharedBorrow,
                        },
                    ],
                    completion_receipts: Vec::new(),
                },
            },
            Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: id(1, OperationId::new),
                result: OperationResult::Unit,
                kind: OperationKind::PortWrite {
                    service,
                    port: 0x3f8,
                    value: b'X',
                },
            },
        ];

        let rows = reconstruct_terminal_trace_v1_rows(&module)
            .expect("valid external-event module reconstructs");
        assert_eq!(
            rows.ordinary_events
                .iter()
                .map(|row| row.operation)
                .collect::<Vec<_>>(),
            [id(1, OperationId::new), id(2, OperationId::new)],
        );
        let [port, call] = rows.ordinary_events.as_slice() else {
            panic!("both ordinary events must be present")
        };
        assert_eq!(
            port.kind,
            TerminalTraceOrdinaryEventKind::PortWrite {
                service,
                service_identity: "PortIo::write-byte".into(),
            },
        );
        assert_eq!(
            port.scalar_arguments
                .iter()
                .map(|schema| schema.scalar_type)
                .collect::<Vec<_>>(),
            [
                ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 16).unwrap()),
                u8_type,
            ],
        );
        assert_eq!(
            call.kind,
            TerminalTraceOrdinaryEventKind::BoundaryCall {
                boundary,
                boundary_identity: "Console::publish".into(),
            },
        );
        assert_eq!(
            call.scalar_arguments
                .iter()
                .map(|schema| schema.scalar_type)
                .collect::<Vec<_>>(),
            [ScalarType::Boolean, u8_type],
        );
        assert_eq!(
            call.structural_arguments
                .iter()
                .map(|schema| (schema.structural_type, schema.access))
                .collect::<Vec<_>>(),
            [
                (first_type, StructuralAccess::Owned),
                (second_type, StructuralAccess::SharedBorrow),
            ],
        );
        assert_eq!(
            call.result,
            TerminalTraceResultSchema::Scalar(TerminalTraceScalarSchema {
                scalar_type: u8_type,
                comparison: TerminalTraceValueComparison::ExactSemanticValue,
            }),
        );
    }
}
