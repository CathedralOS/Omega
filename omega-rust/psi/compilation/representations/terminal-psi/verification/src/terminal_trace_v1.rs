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
