//! Substitutable-field inventory for the standalone TerminalTraceV1
//! observation profile.
//!
//! The family's declared legs are driven through the shared substitution
//! matrix by `trace_profile_custody.rs`; keeping the inventory in its own
//! file names the boundary between what is declared substitutable and the
//! driver that exercises every leg. A leg is one authored substitution of a
//! `TerminalTraceV1Profile` field that still canonicalizes under
//! `encode_terminal_trace_v1_profile`: the retained module identity, the
//! root row's entry machine, scalar and structural input schemas (roster
//! shape, type, integer carrier, multiplicity, access, and qualification
//! strands), the result schema in each variant, the sorted crash-site and
//! boundary-crash rows (free coordinates, causes, identities, and roster
//! shape), and the ordinary-event rows (free coordinates, kind recasts and
//! identities, argument schemas, result schema, and roster shape). Closed
//! tags, zero and order-breaking identities, count lies, empty route
//! buckets, roster-order violations, and the producer-side encode
//! rejections stay authored in `trace_profile_custody.rs` as decode- and
//! encode-level rejections.
//!
//! Every declared leg must keep the substituted profile canonically
//! decodable while the module-bound replay — the verifier's own
//! reconstruction of the same canonical module — rejects it with
//! `ProfileMismatch`, asserted inside the matrix's checker.

use mutation_matrix::{MutationOutcome, custody_field_inventory};
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};
use terminal_codec::TerminalTraceV1ProfileAcceptanceError;
use terminal_psi::{
    CrashCause, SemanticFingerprint, StructuralAccess, StructuralMultiplicity,
    StructuralPathSegment, TerminalTraceCrashSiteRow, TerminalTraceOrdinaryEventKind,
    TerminalTraceResultSchema, TerminalTraceScalarSchema, TerminalTraceStructuralSchema,
    TerminalTraceV1Profile, TerminalTraceValueComparison,
};

use super::super::{block_id, edge_id, machine_id, operation_id};
use super::{boundary_machine_id, service_id, structural_domain_id, structural_type_id, u8_type};

custody_field_inventory! {
    /// One substitutable field of the TerminalTraceV1 profile; each leg
    /// decodes canonically and is rejected by the module-bound acceptance
    /// replay.
    pub enum TraceProfileCustodyFieldForTest {
        // --- envelope ---
        ModuleIdentitySubstituted,
        // --- root row ---
        RootEntryToModuleMachine,
        RootEntryForeign,
        RootScalarInputDropped,
        RootScalarInputInserted,
        RootScalarInputTypeSubstituted,
        RootScalarIntegerWidthSubstituted,
        RootScalarIntegerSignSubstituted,
        RootStructuralInputDropped,
        RootStructuralInputInserted,
        RootStructuralInputTypeSubstituted,
        RootStructuralInputMultiplicitySubstituted,
        RootStructuralInputAccessSubstituted,
        RootStructuralQualificationSubstituted,
        ProjectedPathFieldRenamed,
        ProjectedQualificationDomainSubstituted,
        // --- root result ---
        RootResultUnit,
        RootResultScalarRetyped,
        RootResultStructural,
        RootResultIntegerWidthSubstituted,
        // --- crash roster ---
        CrashRosterCleared,
        CrashRowDropped,
        CrashRowInserted,
        CrashMachineSubstituted,
        CrashBlockSubstituted,
        CrashEdgeSubstituted,
        CrashCauseSubstituted,
        SecondCrashMachineSubstituted,
        SecondCrashBlockSubstituted,
        SecondCrashEdgeSubstituted,
        SecondCrashCauseSubstituted,
        // --- boundary crash roster ---
        BoundaryCrashRowDropped,
        BoundaryCrashRowOnForeignOperation,
        BoundaryCrashMachineSubstituted,
        BoundaryCrashBlockSubstituted,
        BoundaryCrashOperationSubstituted,
        BoundaryCrashBoundarySubstituted,
        BoundaryCrashIdentitySubstituted,
        BoundaryCrashIdentityLookalike,
        // --- ordinary event roster ---
        EventRowDropped,
        EventRowInserted,
        EventOperationForeign,
        EventBlockForeign,
        EventMachineForeign,
        PortWriteRecastAsBoundaryCall,
        PortWriteRecastToBoundaryIdentity,
        BoundaryCallRecastAsPortWrite,
        EventServiceForeign,
        EventBoundaryForeign,
        EventServiceIdentitySubstituted,
        PortWriteServiceIdentityLookalike,
        EventScalarArgumentDropped,
        EventScalarArgumentInserted,
        EventScalarIntegerWidthSubstituted,
        EventScalarIntegerSignSubstituted,
        BoundaryCallScalarArgumentDropped,
        BoundaryCallScalarTypeSubstituted,
        BoundaryCallStructuralArgumentDropped,
        BoundaryCallStructuralTypeSubstituted,
        BoundaryCallStructuralMultiplicitySubstituted,
        BoundaryCallStructuralAccessSubstituted,
        BoundaryCallQualificationSubstituted,
        BoundaryCallQualificationCleared,
        PortWriteResultSubstituted,
        BoundaryCallResultUnit,
        BoundaryCallResultTypeSubstituted,
        BoundaryCallResultAccessSubstituted,
        BoundaryCallResultMultiplicitySubstituted,
    }
}

/// Apply one declared substitution to the profile.
///
/// Every leg uses a fixed alternate rather than donor values: the foreign
/// donor profile was reconstructed for a different module, so its rows name
/// that module's coordinates and cannot supply this fixture's rooted,
/// join-free field values.
#[allow(clippy::too_many_lines)]
pub(crate) fn substitute_trace_profile_for_test(
    record: &mut TerminalTraceV1Profile,
    field: TraceProfileCustodyFieldForTest,
    donor: &TerminalTraceV1Profile,
) {
    let _ = donor;
    use TraceProfileCustodyFieldForTest as Leg;
    match field {
        Leg::ModuleIdentitySubstituted => {
            record.module_identity.program_fingerprint = SemanticFingerprint::from_bytes([7; 32]);
        }

        // --- root row ---
        Leg::RootEntryToModuleMachine => record.root.entry = machine_id(21),
        Leg::RootEntryForeign => record.root.entry = machine_id(7),
        Leg::RootScalarInputDropped => {
            record.root.scalar_inputs.pop();
        }
        Leg::RootScalarInputInserted => {
            record.root.scalar_inputs.push(TerminalTraceScalarSchema {
                scalar_type: u8_type(),
                comparison: TerminalTraceValueComparison::ExactSemanticValue,
            });
        }
        Leg::RootScalarInputTypeSubstituted => {
            record.root.scalar_inputs[0].scalar_type = u8_type();
        }
        Leg::RootScalarIntegerWidthSubstituted => {
            record.root.scalar_inputs[1].scalar_type = ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 test type"),
            );
        }
        Leg::RootScalarIntegerSignSubstituted => {
            record.root.scalar_inputs[1].scalar_type = ScalarType::Integer(
                IntegerType::new(IntegerSign::Signed, 8).expect("i8 test type"),
            );
        }
        Leg::RootStructuralInputDropped => {
            record.root.structural_inputs.pop();
        }
        Leg::RootStructuralInputInserted => {
            record
                .root
                .structural_inputs
                .push(TerminalTraceStructuralSchema {
                    structural_type: structural_type_id(1),
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: StructuralAccess::Owned,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    comparison: TerminalTraceValueComparison::ExactSemanticValue,
                });
        }
        Leg::RootStructuralInputTypeSubstituted => {
            record.root.structural_inputs[0].structural_type = structural_type_id(9);
        }
        Leg::RootStructuralInputMultiplicitySubstituted => {
            record.root.structural_inputs[0].multiplicity = StructuralMultiplicity::Linear;
        }
        Leg::RootStructuralInputAccessSubstituted => {
            record.root.structural_inputs[0].access = StructuralAccess::WriteOnlyBorrow;
        }
        Leg::RootStructuralQualificationSubstituted => {
            record.root.structural_inputs[0].qualifications[0] = structural_domain_id(9);
        }
        Leg::ProjectedPathFieldRenamed => {
            record.root.structural_inputs[1].projected_qualifications[0].path[0] =
                StructuralPathSegment::Field("Region".into());
        }
        Leg::ProjectedQualificationDomainSubstituted => {
            record.root.structural_inputs[1].projected_qualifications[0].domain =
                structural_domain_id(9);
        }

        // --- root result ---
        Leg::RootResultUnit => record.root.result = TerminalTraceResultSchema::Unit,
        Leg::RootResultScalarRetyped => {
            record.root.result = TerminalTraceResultSchema::Scalar(TerminalTraceScalarSchema {
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 16).expect("u16 test type"),
                ),
                comparison: TerminalTraceValueComparison::ExactSemanticValue,
            });
        }
        Leg::RootResultStructural => {
            let schema = record.root.structural_inputs[0].clone();
            record.root.result = TerminalTraceResultSchema::Structural(schema);
        }
        Leg::RootResultIntegerWidthSubstituted => {
            let TerminalTraceResultSchema::Scalar(schema) = &mut record.root.result else {
                unreachable!("the fixture's root result is a scalar schema")
            };
            schema.scalar_type = ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 test type"),
            );
        }

        // --- crash roster ---
        Leg::CrashRosterCleared => record.crash_sites.clear(),
        Leg::CrashRowDropped => {
            record.crash_sites.remove(0);
        }
        Leg::CrashRowInserted => record.crash_sites.push(TerminalTraceCrashSiteRow {
            machine: machine_id(51),
            block: block_id(51),
            edge: edge_id(51),
            cause: CrashCause::Abort,
        }),
        Leg::CrashMachineSubstituted => record.crash_sites[0].machine = machine_id(31),
        Leg::CrashBlockSubstituted => record.crash_sites[0].block = block_id(31),
        Leg::CrashEdgeSubstituted => record.crash_sites[0].edge = edge_id(31),
        Leg::CrashCauseSubstituted => record.crash_sites[0].cause = CrashCause::Abort,
        Leg::SecondCrashMachineSubstituted => record.crash_sites[1].machine = machine_id(61),
        Leg::SecondCrashBlockSubstituted => record.crash_sites[1].block = block_id(61),
        Leg::SecondCrashEdgeSubstituted => record.crash_sites[1].edge = edge_id(61),
        Leg::SecondCrashCauseSubstituted => record.crash_sites[1].cause = CrashCause::Trap,

        // --- boundary crash roster ---
        Leg::BoundaryCrashRowDropped => {
            record.boundary_crash_sites.remove(0);
        }
        Leg::BoundaryCrashRowOnForeignOperation => {
            let mut extra = record.boundary_crash_sites[1].clone();
            extra.operation = operation_id(9);
            record.boundary_crash_sites.push(extra);
        }
        Leg::BoundaryCrashMachineSubstituted => {
            record.boundary_crash_sites[0].machine = machine_id(5);
        }
        Leg::BoundaryCrashBlockSubstituted => {
            record.boundary_crash_sites[0].block = block_id(5);
        }
        Leg::BoundaryCrashOperationSubstituted => {
            record.boundary_crash_sites[0].operation = operation_id(1);
        }
        Leg::BoundaryCrashBoundarySubstituted => {
            record.boundary_crash_sites[0].boundary = boundary_machine_id(31);
        }
        Leg::BoundaryCrashIdentitySubstituted => {
            record.boundary_crash_sites[0].boundary_identity = "Xonsole::publish".into();
        }
        Leg::BoundaryCrashIdentityLookalike => {
            record.boundary_crash_sites[0].boundary_identity = "Console::lookalike".into();
        }

        // --- ordinary event roster ---
        Leg::EventRowDropped => {
            record.ordinary_events.remove(0);
        }
        Leg::EventRowInserted => {
            let mut extra = record.ordinary_events[1].clone();
            extra.operation = operation_id(9);
            record.ordinary_events.push(extra);
        }
        Leg::EventOperationForeign => record.ordinary_events[1].operation = operation_id(9),
        Leg::EventBlockForeign => record.ordinary_events[1].block = block_id(31),
        Leg::EventMachineForeign => record.ordinary_events[1].machine = machine_id(31),
        Leg::PortWriteRecastAsBoundaryCall => {
            record.ordinary_events[0].kind = TerminalTraceOrdinaryEventKind::BoundaryCall {
                boundary: boundary_machine_id(1),
                boundary_identity: "PortIo::write-byte".into(),
            };
        }
        Leg::PortWriteRecastToBoundaryIdentity => {
            record.ordinary_events[0].kind = TerminalTraceOrdinaryEventKind::BoundaryCall {
                boundary: boundary_machine_id(1),
                boundary_identity: "Console::publish".into(),
            };
        }
        Leg::BoundaryCallRecastAsPortWrite => {
            record.ordinary_events[1].kind = TerminalTraceOrdinaryEventKind::PortWrite {
                service: service_id(1),
                service_identity: "Console::publish".into(),
            };
        }
        Leg::EventServiceForeign => {
            let TerminalTraceOrdinaryEventKind::PortWrite { service, .. } =
                &mut record.ordinary_events[0].kind
            else {
                unreachable!("the first event is a port write")
            };
            *service = service_id(9);
        }
        Leg::EventBoundaryForeign => {
            let TerminalTraceOrdinaryEventKind::BoundaryCall { boundary, .. } =
                &mut record.ordinary_events[1].kind
            else {
                unreachable!("the second event is a boundary call")
            };
            *boundary = boundary_machine_id(9);
        }
        Leg::EventServiceIdentitySubstituted => {
            let TerminalTraceOrdinaryEventKind::PortWrite {
                service_identity, ..
            } = &mut record.ordinary_events[0].kind
            else {
                unreachable!("the first event is a port write")
            };
            *service_identity = "XortIo::write-byte".into();
        }
        Leg::PortWriteServiceIdentityLookalike => {
            let TerminalTraceOrdinaryEventKind::PortWrite {
                service_identity, ..
            } = &mut record.ordinary_events[0].kind
            else {
                unreachable!("the first event is a port write")
            };
            *service_identity = "PortIo::lookalike".into();
        }
        Leg::EventScalarArgumentDropped => {
            record.ordinary_events[0].scalar_arguments.pop();
        }
        Leg::EventScalarArgumentInserted => {
            let inserted = record.ordinary_events[0].scalar_arguments[0];
            record.ordinary_events[0].scalar_arguments.push(inserted);
        }
        Leg::EventScalarIntegerWidthSubstituted => {
            record.ordinary_events[0].scalar_arguments[0].scalar_type = ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 32).expect("u32 test type"),
            );
        }
        Leg::EventScalarIntegerSignSubstituted => {
            record.ordinary_events[0].scalar_arguments[0].scalar_type = ScalarType::Integer(
                IntegerType::new(IntegerSign::Signed, 16).expect("i16 test type"),
            );
        }
        Leg::BoundaryCallScalarArgumentDropped => {
            record.ordinary_events[1].scalar_arguments.pop();
        }
        Leg::BoundaryCallScalarTypeSubstituted => {
            record.ordinary_events[1].scalar_arguments[0].scalar_type = u8_type();
        }
        Leg::BoundaryCallStructuralArgumentDropped => {
            record.ordinary_events[1].structural_arguments.pop();
        }
        Leg::BoundaryCallStructuralTypeSubstituted => {
            record.ordinary_events[1].structural_arguments[0].structural_type =
                structural_type_id(9);
        }
        Leg::BoundaryCallStructuralMultiplicitySubstituted => {
            record.ordinary_events[1].structural_arguments[0].multiplicity =
                StructuralMultiplicity::Affine;
        }
        Leg::BoundaryCallStructuralAccessSubstituted => {
            record.ordinary_events[1].structural_arguments[0].access =
                StructuralAccess::MutableBorrow;
        }
        Leg::BoundaryCallQualificationSubstituted => {
            record.ordinary_events[1].structural_arguments[0].qualifications[0] =
                structural_domain_id(9);
        }
        Leg::BoundaryCallQualificationCleared => {
            record.ordinary_events[1].structural_arguments[0]
                .qualifications
                .clear();
        }
        Leg::PortWriteResultSubstituted => {
            record.ordinary_events[0].result =
                TerminalTraceResultSchema::Scalar(record.ordinary_events[0].scalar_arguments[0]);
        }
        Leg::BoundaryCallResultUnit => {
            record.ordinary_events[1].result = TerminalTraceResultSchema::Unit;
        }
        Leg::BoundaryCallResultTypeSubstituted => {
            let TerminalTraceResultSchema::Structural(result) =
                &mut record.ordinary_events[1].result
            else {
                unreachable!("the boundary call result is a structural schema")
            };
            result.structural_type = structural_type_id(9);
        }
        Leg::BoundaryCallResultAccessSubstituted => {
            let TerminalTraceResultSchema::Structural(result) =
                &mut record.ordinary_events[1].result
            else {
                unreachable!("the boundary call result is a structural schema")
            };
            result.access = StructuralAccess::MutableBorrow;
        }
        Leg::BoundaryCallResultMultiplicitySubstituted => {
            let TerminalTraceResultSchema::Structural(result) =
                &mut record.ordinary_events[1].result
            else {
                unreachable!("the boundary call result is a structural schema")
            };
            result.multiplicity = StructuralMultiplicity::Linear;
        }
    }
}

/// Name the verdict a declared leg must produce at the module-bound replay:
/// every representable substitution diverges from the verifier-owned
/// reconstruction of the same canonical module, so acceptance always
/// rejects with `ProfileMismatch`. Canonical decoding and the encode
/// round-trip are asserted inside the matrix's checker.
pub(crate) fn trace_profile_custody_outcome(
    _field: TraceProfileCustodyFieldForTest,
) -> MutationOutcome<TerminalTraceV1ProfileAcceptanceError> {
    MutationOutcome::ExactError(TerminalTraceV1ProfileAcceptanceError::ProfileMismatch)
}
