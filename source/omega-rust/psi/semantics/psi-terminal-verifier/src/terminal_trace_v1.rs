//! Verifier-owned reconstruction of the first bounded D39 observation profile.

use psi_core::{BlockId, EdgeId, MachineId, OperationId};
use psi_terminal::{
    OperationKind, StructuralAccess, TerminalMachineResult, TerminalModule,
    TerminalObservationSchema, TerminalTraceCrashSiteRow, TerminalTraceResultSchema,
    TerminalTraceRootRow, TerminalTraceScalarSchema, TerminalTraceStructuralSchema,
    TerminalTraceV1Rows, TerminalTraceValueComparison, Terminator,
};

use crate::{ModuleError, validate_module_representation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsupportedTerminalTraceV1Classification {
    BoundaryCall,
    PortWrite,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalTraceV1ReconstructionError {
    InvalidModule(ModuleError),
    MissingEntry(MachineId),
    UnsupportedExternalEvent {
        machine: MachineId,
        block: BlockId,
        operation: OperationId,
        classification: UnsupportedTerminalTraceV1Classification,
    },
    DuplicateCrashSite {
        machine: MachineId,
        block: BlockId,
        edge: EdgeId,
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
                comparison: exact,
            })
        }
    };

    let mut crash_sites = Vec::new();
    for machine in &module.machines {
        for block in &machine.blocks {
            for operation in &block.operations {
                match classify_operation(&operation.kind) {
                    OperationClassification::Internal => {}
                    OperationClassification::Unsupported(classification) => {
                        return Err(
                            TerminalTraceV1ReconstructionError::UnsupportedExternalEvent {
                                machine: machine.id,
                                block: block.id,
                                operation: operation.id,
                                classification,
                            },
                        );
                    }
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

    Ok(TerminalTraceV1Rows {
        root: TerminalTraceRootRow {
            entry: entry.id,
            scalar_inputs,
            structural_inputs,
            result,
        },
        crash_sites,
    })
}

fn crash_site_key(row: &TerminalTraceCrashSiteRow) -> (MachineId, BlockId, EdgeId) {
    (row.machine, row.block, row.edge)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperationClassification {
    Internal,
    Unsupported(UnsupportedTerminalTraceV1Classification),
}

/// This intentionally has no wildcard arm. Adding a Terminal operation cannot
/// silently inherit an observation classification.
fn classify_operation(kind: &OperationKind) -> OperationClassification {
    match kind {
        OperationKind::BoundaryCall { .. } => OperationClassification::Unsupported(
            UnsupportedTerminalTraceV1Classification::BoundaryCall,
        ),
        OperationKind::PortWrite { .. } => OperationClassification::Unsupported(
            UnsupportedTerminalTraceV1Classification::PortWrite,
        ),
        OperationKind::WriteOnlyPrimitiveStore { .. }
        | OperationKind::EstablishPayloadlessCase { .. }
        | OperationKind::EstablishByteSequenceLiteral { .. }
        | OperationKind::EstablishTrivialAffineLocal { .. }
        | OperationKind::Call { .. }
        | OperationKind::CallUnit { .. }
        | OperationKind::CallStructuralScalar { .. }
        | OperationKind::CallStructural { .. }
        | OperationKind::IntegerConstant { .. }
        | OperationKind::BooleanConstant { .. }
        | OperationKind::BooleanStructuralField { .. }
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
        | OperationKind::SaturatingIntegerMultiply { .. } => OperationClassification::Internal,
    }
}

#[cfg(test)]
mod tests {
    use psi_core::{
        BlockId, BoundaryMachineId, ContractId, EdgeId, MachineId, ScalarType, ServiceId, ValueId,
    };
    use psi_terminal::{
        Block, CrashCause, CrashRouteBucket, CrashRouteGuard, MachineContract, OperationKind,
        TerminalMachine, TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
        VocabularyMarker,
    };

    use super::*;

    fn id<T>(raw: u64, make: impl FnOnce(u64) -> Option<T>) -> T {
        make(raw).expect("nonzero test identity")
    }

    fn module(terminator: Terminator, crash_routes: Vec<CrashRouteBucket>) -> TerminalModule {
        let machine = id(1, MachineId::new);
        let block = id(1, BlockId::new);
        TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine,
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            placed_view_inputs: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            closed_conformance_applications: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines: vec![TerminalMachine {
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
                    id: block,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator,
                }],
                contract: MachineContract {
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
            id: id(2, ValueId::new),
            scalar_type: ScalarType::Boolean,
        };
        let result = ValueDeclaration {
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
    fn external_event_operations_have_explicit_rejecting_classifications() {
        let boundary = OperationKind::BoundaryCall {
            boundary: id(1, BoundaryMachineId::new),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
            requirement_obligations: Vec::new(),
        };
        assert_eq!(
            classify_operation(&boundary),
            OperationClassification::Unsupported(
                UnsupportedTerminalTraceV1Classification::BoundaryCall,
            ),
        );

        let port = OperationKind::PortWrite {
            service: id(1, ServiceId::new),
            port: 0x3f8,
            value: 1,
        };
        assert_eq!(
            classify_operation(&port),
            OperationClassification::Unsupported(
                UnsupportedTerminalTraceV1Classification::PortWrite,
            ),
        );

        assert_eq!(
            classify_operation(&OperationKind::BooleanConstant { value: true }),
            OperationClassification::Internal,
        );
    }
}
