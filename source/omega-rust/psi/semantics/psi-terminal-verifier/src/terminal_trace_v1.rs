//! Verifier-owned reconstruction of the first bounded D39 observation profile.

use psi_core::{BlockId, EdgeId, IntegerSign, IntegerType, MachineId, OperationId, ScalarType};
use psi_terminal::{
    OperationKind, StructuralAccess, TerminalMachineResult, TerminalModule,
    TerminalObservationSchema, TerminalTraceCrashSiteRow, TerminalTraceOrdinaryEventKind,
    TerminalTraceOrdinaryEventRow, TerminalTraceResultSchema, TerminalTraceRootRow,
    TerminalTraceScalarSchema, TerminalTraceStructuralSchema, TerminalTraceV1Rows,
    TerminalTraceValueComparison, Terminator,
};

use crate::{ModuleError, validate_module_representation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalTraceV1OperationClassification {
    Internal,
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
    DuplicateOrdinaryEventSite {
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
    let mut ordinary_events = Vec::new();
    for machine in &module.machines {
        for block in &machine.blocks {
            for operation in &block.operations {
                match classify_operation(&operation.kind) {
                    TerminalTraceV1OperationClassification::Internal => {}
                    TerminalTraceV1OperationClassification::BoundaryCall => ordinary_events.push(
                        reconstruct_boundary_call_event(module, machine.id, block.id, operation),
                    ),
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
        ordinary_events,
    })
}

fn crash_site_key(row: &TerminalTraceCrashSiteRow) -> (MachineId, BlockId, EdgeId) {
    (row.machine, row.block, row.edge)
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
        OperationKind::WriteOnlyPrimitiveStore { .. }
        | OperationKind::StructuralScalarFieldStore { .. }
        | OperationKind::EstablishPayloadlessCase { .. }
        | OperationKind::EstablishByteSequenceLiteral { .. }
        | OperationKind::EstablishTrivialAffineLocal { .. }
        | OperationKind::EstablishAffineScalarRecord { .. }
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
        | OperationKind::BooleanStructuralField { .. }
        | OperationKind::IntegerStructuralField { .. }
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

fn reconstruct_boundary_call_event(
    module: &TerminalModule,
    machine: MachineId,
    block: BlockId,
    operation: &psi_terminal::Operation,
) -> TerminalTraceOrdinaryEventRow {
    let OperationKind::BoundaryCall { boundary, .. } = &operation.kind else {
        unreachable!("classification selected a boundary call")
    };
    let declaration = module
        .boundary_machines
        .iter()
        .find(|declaration| declaration.id == *boundary)
        .expect("module validation established the boundary declaration");
    let exact = TerminalTraceValueComparison::ExactSemanticValue;
    TerminalTraceOrdinaryEventRow {
        machine,
        block,
        operation: operation.id,
        kind: TerminalTraceOrdinaryEventKind::BoundaryCall {
            boundary: *boundary,
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
        result: declaration
            .result
            .map_or(TerminalTraceResultSchema::Unit, |scalar_type| {
                TerminalTraceResultSchema::Scalar(TerminalTraceScalarSchema {
                    scalar_type,
                    comparison: exact,
                })
            }),
    }
}

fn reconstruct_port_write_event(
    module: &TerminalModule,
    machine: MachineId,
    block: BlockId,
    operation: &psi_terminal::Operation,
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
    use psi_core::{
        BlockId, BoundaryMachineId, ContractId, EdgeId, MachineId, PlaceId, ScalarType, ServiceId,
        StructuralPlaceKind, StructuralTypeId, ValueId,
    };
    use psi_terminal::{
        Block, BoundaryMachineDeclaration, CrashCause, CrashRouteBucket, CrashRouteGuard,
        MachineContract, Operation, OperationKind, OperationResult, ServiceDeclaration,
        StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
        StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
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
    fn external_event_operations_have_explicit_observable_classifications() {
        let boundary = OperationKind::BoundaryCall {
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
            id: boundary,
            identity: "Console::publish".into(),
            attachment: None,
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
            result: Some(u8_type),
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        }];
        module.machines[0].parameters = vec![
            ValueDeclaration {
                id: argument,
                scalar_type: ScalarType::Boolean,
            },
            ValueDeclaration {
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
                id: id(2, OperationId::new),
                result: OperationResult::Scalar(ValueDeclaration {
                    id: id(3, ValueId::new),
                    scalar_type: u8_type,
                }),
                kind: OperationKind::BoundaryCall {
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
