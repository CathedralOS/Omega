//! Edge-owned cleanup, dynamic conformance, dynamic call and forwarded
//! dynamic plan fixtures.

use super::provider_and_call_plans::internal_call_plan;
use super::scalar_plans::scalar_mutation;
use super::{edge_id, identity, machine_id, operation_id};
use calling_conventions::{ValuePlacement, ValueShape};
use machine_code::{
    InternalCallRelocation, InternalUnitCallRecord, MachineCodeFunction, MachineCodePlan,
    ScalarCallStackEvidence, ScalarControlFlowEvidence, ScalarStackEvidence,
    ScalarStackMutationKind, SemanticCodeAttribution, SemanticCodeSite, StackAdjustmentPair,
    UnitAffineCleanupRecord, UnitCallStackEvidence, UnitParameterHomeRecord, UnitParameterRecord,
    UnitStackEvidence,
};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use target::NativeTarget;
use target_operations::{CallSiteOwner, TerminalPsiProvenance};
use terminal_psi::{
    NominalAffineCleanup, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    TerminalAffineCleanupAction,
};

pub(super) fn edge_owned_cleanup_plan() -> MachineCodePlan {
    let structural_type = StructuralTypeId::new(1).expect("type");
    let place = PlaceId::new(1).expect("place");
    let empty_shape = ValueShape::integer(0, 1);
    let empty_placement = ValuePlacement {
        shape: empty_shape,
        locations: Vec::new(),
    };
    let unit_stack = Some(UnitStackEvidence {
        frame: None,
        aarch64_return_link: None,
        stack_alignment: 16,
    });
    let empty_return = |edge| UnitAffineCleanupRecord {
        structural_types: Vec::new().into(),
        psi_edge: edge,
        locals: Vec::new(),
        actions: Vec::new(),
        code_offset: 13,
        byte_count: 1,
    };
    let x86_empty_call_bytes = vec![
        0x48, 0x83, 0xec, 0x08, 0xe8, 0, 0, 0, 0, 0x48, 0x83, 0xc4, 0x08, 0xc3,
    ];
    let stack_pair = StackAdjustmentPair {
        byte_size: 8,
        allocation_offset: 0,
        allocation_byte_count: 4,
        release_offset: 9,
        release_byte_count: 4,
    };
    let operation_call = |operation, target| InternalCallRelocation {
        owner: CallSiteOwner::Operation(operation),
        target,
        unit_stack: Some(UnitCallStackEvidence {
            outbound: Some(stack_pair),
        }),
        scalar_stack: None,
        offset: 5,
    };
    let operation_custody = |operation, target| InternalUnitCallRecord {
        source: machine_code::InternalUnitCallSource::Authored,
        owner: CallSiteOwner::Operation(operation),
        target,
        result: None,
        semantic_result: None,
        structural_result: None,
        scalar_arguments: Vec::new(),
        arguments: Vec::new(),
        claim_transfers: Vec::new(),
        operation_ordinal: 0,
        code_offset: 0,
        byte_count: 13,
    };
    MachineCodePlan {
        psi: identity(),
        target: NativeTarget::linux_x64(),
        entry: machine_id(3),
        functions: vec![
            MachineCodeFunction {
                scalar_abi: None,
                mixed_structural_scalar_abi: None,
                structural_call_scalar_return: None,
                parameter_abi: None,
                internal_unit_scalar_calls: Vec::new(),
                installed_provider_unit_scalar_calls: Vec::new(),
                dynamic_calls: Vec::new(),
                stored_dynamic_calls: Vec::new(),
                dynamic_parameter_calls: Vec::new(),
                forwarded_dynamic_parameter_calls: Vec::new(),
                forwarded_dynamic_descriptor_calls: Vec::new(),
                unit_scalar_homes: Vec::new(),
                unit_integer_constants: Vec::new(),
                unit_affine_scalar_records: Vec::new(),
                unit_structural_scalar_field_stores: Vec::new(),
                unit_write_only_primitive_stores: Vec::new(),
                scalar_structural_scalar_field_stores: Vec::new(),
                machine: machine_id(1),
                attachment: Some(structural_type),
                provenance: TerminalPsiProvenance {
                    operations: vec![operation_id(1)],
                    edges: vec![edge_id(1)],
                },
                bytes: x86_empty_call_bytes.clone(),
                x86_scalar_fma: Vec::new(),
                x86_scalar_fma_occurrences: Vec::new(),
                x86_floating_control: None,
                unit_stack,
                unit_parameter_homes: Vec::new(),
                unit_parameters: Vec::new(),
                scalar_stack: None,
                internal_calls: vec![operation_call(operation_id(1), machine_id(2))],
                foreign_calls: Vec::new(),
                internal_unit_calls: vec![operation_custody(operation_id(1), machine_id(2))],
                unit_continuations: Vec::new(),
                unit_affine_cleanup: Some(empty_return(edge_id(1))),
                semantic_code_attribution: vec![
                    SemanticCodeAttribution {
                        site: SemanticCodeSite::Operation(operation_id(1)),
                        operation_ordinal: 0,
                        code_offset: 0,
                        byte_count: 13,
                    },
                    SemanticCodeAttribution {
                        site: SemanticCodeSite::Edge(edge_id(1)),
                        operation_ordinal: 1,
                        code_offset: 13,
                        byte_count: 1,
                    },
                ],
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
                scalar_affine_cleanup: None,
                scalar_control_affine_cleanups: Vec::new(),
                scalar_structural_parameters: Vec::new(),
                scalar_structural_parameter_homes: Vec::new(),
                structural_return: None,
            },
            MachineCodeFunction {
                scalar_abi: None,
                mixed_structural_scalar_abi: None,
                structural_call_scalar_return: None,
                parameter_abi: None,
                internal_unit_scalar_calls: Vec::new(),
                installed_provider_unit_scalar_calls: Vec::new(),
                dynamic_calls: Vec::new(),
                stored_dynamic_calls: Vec::new(),
                dynamic_parameter_calls: Vec::new(),
                forwarded_dynamic_parameter_calls: Vec::new(),
                forwarded_dynamic_descriptor_calls: Vec::new(),
                unit_scalar_homes: Vec::new(),
                unit_integer_constants: Vec::new(),
                unit_affine_scalar_records: Vec::new(),
                unit_structural_scalar_field_stores: Vec::new(),
                unit_write_only_primitive_stores: Vec::new(),
                scalar_structural_scalar_field_stores: Vec::new(),
                machine: machine_id(2),
                attachment: Some(StructuralTypeId::new(2).expect("helper type")),
                provenance: TerminalPsiProvenance {
                    operations: Vec::new(),
                    edges: vec![edge_id(2)],
                },
                bytes: vec![0xc3],
                x86_scalar_fma: Vec::new(),
                x86_scalar_fma_occurrences: Vec::new(),
                x86_floating_control: None,
                unit_stack,
                unit_parameter_homes: Vec::new(),
                unit_parameters: Vec::new(),
                scalar_stack: None,
                internal_calls: Vec::new(),
                foreign_calls: Vec::new(),
                internal_unit_calls: Vec::new(),
                unit_continuations: Vec::new(),
                unit_affine_cleanup: Some(UnitAffineCleanupRecord {
                    structural_types: Vec::new().into(),
                    code_offset: 0,
                    byte_count: 1,
                    ..empty_return(edge_id(2))
                }),
                semantic_code_attribution: vec![SemanticCodeAttribution {
                    site: SemanticCodeSite::Edge(edge_id(2)),
                    operation_ordinal: 0,
                    code_offset: 0,
                    byte_count: 1,
                }],
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
                scalar_affine_cleanup: None,
                scalar_control_affine_cleanups: Vec::new(),
                scalar_structural_parameters: Vec::new(),
                scalar_structural_parameter_homes: Vec::new(),
                structural_return: None,
            },
            MachineCodeFunction {
                scalar_abi: None,
                mixed_structural_scalar_abi: None,
                structural_call_scalar_return: None,
                parameter_abi: None,
                internal_unit_scalar_calls: Vec::new(),
                installed_provider_unit_scalar_calls: Vec::new(),
                dynamic_calls: Vec::new(),
                stored_dynamic_calls: Vec::new(),
                dynamic_parameter_calls: Vec::new(),
                forwarded_dynamic_parameter_calls: Vec::new(),
                forwarded_dynamic_descriptor_calls: Vec::new(),
                unit_scalar_homes: Vec::new(),
                unit_integer_constants: Vec::new(),
                unit_affine_scalar_records: Vec::new(),
                unit_structural_scalar_field_stores: Vec::new(),
                unit_write_only_primitive_stores: Vec::new(),
                scalar_structural_scalar_field_stores: Vec::new(),
                machine: machine_id(3),
                attachment: None,
                provenance: TerminalPsiProvenance {
                    operations: Vec::new(),
                    edges: vec![edge_id(3)],
                },
                bytes: x86_empty_call_bytes,
                x86_scalar_fma: Vec::new(),
                x86_scalar_fma_occurrences: Vec::new(),
                x86_floating_control: None,
                unit_stack,
                unit_parameter_homes: vec![UnitParameterHomeRecord {
                    place,
                    structural_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    access: terminal_psi::StructuralAccess::Owned,
                    shape: empty_shape,
                    source: empty_placement.clone(),
                    location: machine_code::StructuralSourceLocation::Stack { byte_offset: 0 },
                    indirect: false,
                }],
                unit_parameters: vec![UnitParameterRecord {
                    place,
                    structural_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    shape: empty_shape,
                }],
                scalar_stack: None,
                internal_calls: vec![InternalCallRelocation {
                    owner: CallSiteOwner::CleanupAction {
                        edge: edge_id(3),
                        action_ordinal: 0,
                    },
                    target: machine_id(1),
                    unit_stack: Some(UnitCallStackEvidence {
                        outbound: Some(stack_pair),
                    }),
                    scalar_stack: None,
                    offset: 5,
                }],
                foreign_calls: Vec::new(),
                internal_unit_calls: vec![InternalUnitCallRecord {
                    source: machine_code::InternalUnitCallSource::Authored,
                    owner: CallSiteOwner::CleanupAction {
                        edge: edge_id(3),
                        action_ordinal: 0,
                    },
                    target: machine_id(1),
                    result: None,
                    semantic_result: None,
                    structural_result: None,
                    scalar_arguments: Vec::new(),
                    arguments: Vec::new(),
                    claim_transfers: Vec::new(),
                    operation_ordinal: 0,
                    code_offset: 0,
                    byte_count: 13,
                }],
                unit_continuations: Vec::new(),
                unit_affine_cleanup: Some(UnitAffineCleanupRecord {
                    structural_types: Vec::new().into(),
                    psi_edge: edge_id(3),
                    locals: Vec::new(),
                    actions: vec![TerminalAffineCleanupAction::InvokeNominal(
                        NominalAffineCleanup {
                            place,
                            structural_type,
                            cleanup_machine: machine_id(1),
                            cleanup_receiver: None,
                            requirement_obligations: Vec::new(),
                        },
                    )],
                    code_offset: 0,
                    byte_count: 14,
                }),
                semantic_code_attribution: vec![SemanticCodeAttribution {
                    site: SemanticCodeSite::Edge(edge_id(3)),
                    operation_ordinal: 0,
                    code_offset: 0,
                    byte_count: 14,
                }],
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
                scalar_affine_cleanup: None,
                scalar_control_affine_cleanups: Vec::new(),
                scalar_structural_parameters: Vec::new(),
                scalar_structural_parameter_homes: Vec::new(),
                structural_return: None,
            },
        ],
    }
}

/// A Unit caller whose scalar-parameter ABI spills both incoming registers
/// into its 16-byte frame before an argument-free internal call, then crosses
/// one continuation boundary that rebinds the first parameter to a successor
/// parameter behind a zero-byte edge cleanup, and returns under the final
/// edge cleanup.
pub(super) fn continuation_unit_call_plan() -> MachineCodePlan {
    let i32_scalar = semantic_vocabulary::ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
            .expect("i32"),
    );
    let i32_shape = ValueShape::integer(4, 4);
    let call_plan = calling_conventions::evaluate_call_plan(
        calling_conventions::CallingPolicy::native_for_target(NativeTarget::linux_x64()),
        &calling_conventions::CallSignature {
            parameters: vec![i32_shape, i32_shape],
            result: None,
        },
    )
    .expect("parameter call plan");
    let parameter_abi = machine_code::ParameterFunctionAbiRecord {
        call_plan: call_plan.clone(),
        parameters: vec![
            target_operations::ScalarAbiValue {
                value: semantic_vocabulary::ValueId::new(45).expect("parameter value"),
                scalar_type: i32_scalar,
                placement: call_plan.parameters[0].clone(),
            },
            target_operations::ScalarAbiValue {
                value: semantic_vocabulary::ValueId::new(46).expect("parameter value"),
                scalar_type: i32_scalar,
                placement: call_plan.parameters[1].clone(),
            },
        ],
        entry_register_spills: vec![
            machine_code::UnitEntryRegisterSpillRecord {
                source_value: semantic_vocabulary::ValueId::new(45).expect("parameter value"),
                parameter_index: 0,
                register: calling_conventions::MachineRegister::X86Rdi,
                byte_offset: 0,
                code_offset: 4,
                byte_count: 5,
            },
            machine_code::UnitEntryRegisterSpillRecord {
                source_value: semantic_vocabulary::ValueId::new(46).expect("parameter value"),
                parameter_index: 1,
                register: calling_conventions::MachineRegister::X86Rsi,
                byte_offset: 8,
                code_offset: 9,
                byte_count: 5,
            },
        ],
    };
    MachineCodePlan {
        psi: identity(),
        target: NativeTarget::linux_x64(),
        entry: machine_id(1),
        functions: vec![
            MachineCodeFunction {
                scalar_abi: None,
                mixed_structural_scalar_abi: None,
                structural_call_scalar_return: None,
                parameter_abi: Some(parameter_abi),
                internal_unit_scalar_calls: Vec::new(),
                installed_provider_unit_scalar_calls: Vec::new(),
                dynamic_calls: Vec::new(),
                stored_dynamic_calls: Vec::new(),
                dynamic_parameter_calls: Vec::new(),
                forwarded_dynamic_parameter_calls: Vec::new(),
                forwarded_dynamic_descriptor_calls: Vec::new(),
                unit_scalar_homes: Vec::new(),
                unit_integer_constants: Vec::new(),
                unit_affine_scalar_records: Vec::new(),
                unit_structural_scalar_field_stores: Vec::new(),
                unit_write_only_primitive_stores: Vec::new(),
                scalar_structural_scalar_field_stores: Vec::new(),
                machine: machine_id(1),
                attachment: Some(StructuralTypeId::new(1).expect("attachment type")),
                provenance: TerminalPsiProvenance {
                    operations: vec![operation_id(1)],
                    edges: vec![edge_id(9), edge_id(1)],
                },
                bytes: vec![
                    0x48, 0x83, 0xec, 0x10, // 0: sub rsp, 16 — frame allocation
                    0x48, 0x89, 0x7c, 0x24, 0x00, // 4: mov [rsp], rdi — parameter spill
                    0x48, 0x89, 0x74, 0x24, 0x08, // 9: mov [rsp+8], rsi — parameter spill
                    0x48, 0x83, 0xec, 0x08, // 14: sub rsp, 8 — call area
                    0xe8, 0, 0, 0, 0, // 18: call rel32 — relocation at 19
                    0x48, 0x83, 0xc4, 0x08, // 23: add rsp, 8 — call area release
                    0x48, 0x83, 0xc4, 0x10, // 27: add rsp, 16 — frame release
                    0xc3, // 31: ret
                ],
                x86_scalar_fma: Vec::new(),
                x86_scalar_fma_occurrences: Vec::new(),
                x86_floating_control: None,
                unit_stack: Some(UnitStackEvidence {
                    frame: Some(StackAdjustmentPair {
                        byte_size: 16,
                        allocation_offset: 0,
                        allocation_byte_count: 4,
                        release_offset: 27,
                        release_byte_count: 4,
                    }),
                    aarch64_return_link: None,
                    stack_alignment: 16,
                }),
                unit_parameter_homes: Vec::new(),
                unit_parameters: Vec::new(),
                scalar_stack: None,
                internal_calls: vec![InternalCallRelocation {
                    owner: CallSiteOwner::Operation(operation_id(1)),
                    target: machine_id(2),
                    unit_stack: Some(UnitCallStackEvidence {
                        outbound: Some(StackAdjustmentPair {
                            byte_size: 8,
                            allocation_offset: 14,
                            allocation_byte_count: 4,
                            release_offset: 23,
                            release_byte_count: 4,
                        }),
                    }),
                    scalar_stack: None,
                    offset: 19,
                }],
                foreign_calls: Vec::new(),
                internal_unit_calls: vec![InternalUnitCallRecord {
                    source: machine_code::InternalUnitCallSource::Authored,
                    owner: CallSiteOwner::Operation(operation_id(1)),
                    target: machine_id(2),
                    result: None,
                    semantic_result: None,
                    structural_result: None,
                    scalar_arguments: Vec::new(),
                    arguments: Vec::new(),
                    claim_transfers: Vec::new(),
                    operation_ordinal: 0,
                    code_offset: 14,
                    byte_count: 13,
                }],
                unit_continuations: vec![machine_code::UnitContinuationRecord {
                    operation_ordinal: 1,
                    successor_operation_ordinal: 2,
                    source_block: semantic_vocabulary::BlockId::new(1).expect("source block"),
                    target_block: semantic_vocabulary::BlockId::new(2).expect("target block"),
                    bindings: vec![abstract_operations::ValueBinding {
                        parameter: semantic_vocabulary::ValueId::new(50).expect("bound parameter"),
                        argument: semantic_vocabulary::ValueId::new(45).expect("parameter value"),
                        scalar_type: i32_scalar,
                    }],
                    cleanup: UnitAffineCleanupRecord {
                        structural_types: Vec::new().into(),
                        psi_edge: edge_id(9),
                        locals: Vec::new(),
                        actions: Vec::new(),
                        code_offset: 27,
                        byte_count: 0,
                    },
                }],
                unit_affine_cleanup: Some(UnitAffineCleanupRecord {
                    structural_types: Vec::new().into(),
                    psi_edge: edge_id(1),
                    locals: Vec::new(),
                    actions: Vec::new(),
                    code_offset: 27,
                    byte_count: 5,
                }),
                semantic_code_attribution: vec![
                    SemanticCodeAttribution {
                        site: SemanticCodeSite::Operation(operation_id(1)),
                        operation_ordinal: 0,
                        code_offset: 14,
                        byte_count: 13,
                    },
                    SemanticCodeAttribution {
                        site: SemanticCodeSite::Edge(edge_id(9)),
                        operation_ordinal: 1,
                        code_offset: 27,
                        byte_count: 0,
                    },
                    SemanticCodeAttribution {
                        site: SemanticCodeSite::Edge(edge_id(1)),
                        operation_ordinal: 2,
                        code_offset: 27,
                        byte_count: 5,
                    },
                ],
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
                scalar_affine_cleanup: None,
                scalar_control_affine_cleanups: Vec::new(),
                scalar_structural_parameters: Vec::new(),
                scalar_structural_parameter_homes: Vec::new(),
                structural_return: None,
            },
            MachineCodeFunction {
                scalar_abi: None,
                mixed_structural_scalar_abi: None,
                structural_call_scalar_return: None,
                parameter_abi: None,
                internal_unit_scalar_calls: Vec::new(),
                installed_provider_unit_scalar_calls: Vec::new(),
                dynamic_calls: Vec::new(),
                stored_dynamic_calls: Vec::new(),
                dynamic_parameter_calls: Vec::new(),
                forwarded_dynamic_parameter_calls: Vec::new(),
                forwarded_dynamic_descriptor_calls: Vec::new(),
                unit_scalar_homes: Vec::new(),
                unit_integer_constants: Vec::new(),
                unit_affine_scalar_records: Vec::new(),
                unit_structural_scalar_field_stores: Vec::new(),
                unit_write_only_primitive_stores: Vec::new(),
                scalar_structural_scalar_field_stores: Vec::new(),
                machine: machine_id(2),
                attachment: Some(StructuralTypeId::new(2).expect("attachment type")),
                provenance: TerminalPsiProvenance {
                    operations: Vec::new(),
                    edges: vec![edge_id(2)],
                },
                bytes: vec![0xc3],
                x86_scalar_fma: Vec::new(),
                x86_scalar_fma_occurrences: Vec::new(),
                x86_floating_control: None,
                unit_stack: Some(UnitStackEvidence {
                    frame: None,
                    aarch64_return_link: None,
                    stack_alignment: 16,
                }),
                unit_parameter_homes: Vec::new(),
                unit_parameters: Vec::new(),
                scalar_stack: None,
                internal_calls: Vec::new(),
                foreign_calls: Vec::new(),
                internal_unit_calls: Vec::new(),
                unit_continuations: Vec::new(),
                unit_affine_cleanup: Some(UnitAffineCleanupRecord {
                    structural_types: Vec::new().into(),
                    psi_edge: edge_id(2),
                    locals: Vec::new(),
                    actions: Vec::new(),
                    code_offset: 0,
                    byte_count: 1,
                }),
                semantic_code_attribution: vec![SemanticCodeAttribution {
                    site: SemanticCodeSite::Edge(edge_id(2)),
                    operation_ordinal: 0,
                    code_offset: 0,
                    byte_count: 1,
                }],
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
                scalar_affine_cleanup: None,
                scalar_control_affine_cleanups: Vec::new(),
                scalar_structural_parameters: Vec::new(),
                scalar_structural_parameter_homes: Vec::new(),
                structural_return: None,
            },
        ],
    }
}

/// A Unit caller forwarding its one zero-byte owned structural parameter to a
/// mixed-ABI callee whose scalar result the caller then returns: the exact
/// retained shape `structural_call_scalar_return` custody requires — one
/// Operation-owned argument-bearing call joined to a matching scalar return
/// and an empty return-edge cleanup.
pub(super) fn structural_call_scalar_return_plan() -> MachineCodePlan {
    let i32_scalar = semantic_vocabulary::ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
            .expect("i32"),
    );
    let structural_type = StructuralTypeId::new(1).expect("type");
    let caller_place = PlaceId::new(1).expect("caller place");
    let callee_place = PlaceId::new(2).expect("callee place");
    let empty_shape = ValueShape::integer(0, 1);
    let empty_placement = ValuePlacement {
        shape: empty_shape,
        locations: Vec::new(),
    };
    let call_plan = calling_conventions::evaluate_call_plan(
        calling_conventions::CallingPolicy::native_for_target(NativeTarget::linux_x64()),
        &calling_conventions::CallSignature {
            parameters: vec![empty_shape],
            result: Some(ValueShape::integer(4, 4)),
        },
    )
    .expect("mixed call plan");
    let result_value = semantic_vocabulary::ValueId::new(80).expect("result value");
    let stack_pair = StackAdjustmentPair {
        byte_size: 8,
        allocation_offset: 0,
        allocation_byte_count: 4,
        release_offset: 9,
        release_byte_count: 4,
    };
    MachineCodePlan {
        psi: identity(),
        target: NativeTarget::linux_x64(),
        entry: machine_id(1),
        functions: vec![
            MachineCodeFunction {
                scalar_abi: None,
                mixed_structural_scalar_abi: None,
                structural_call_scalar_return: Some(
                    machine_code::StructuralCallScalarReturnEvidence {
                        psi_edge: edge_id(1),
                        psi_operation: operation_id(1),
                        source_value: result_value,
                        scalar_type: i32_scalar,
                        callee: machine_id(2),
                    },
                ),
                parameter_abi: None,
                internal_unit_scalar_calls: Vec::new(),
                installed_provider_unit_scalar_calls: Vec::new(),
                dynamic_calls: Vec::new(),
                stored_dynamic_calls: Vec::new(),
                dynamic_parameter_calls: Vec::new(),
                forwarded_dynamic_parameter_calls: Vec::new(),
                forwarded_dynamic_descriptor_calls: Vec::new(),
                unit_scalar_homes: Vec::new(),
                unit_integer_constants: Vec::new(),
                unit_affine_scalar_records: Vec::new(),
                unit_structural_scalar_field_stores: Vec::new(),
                unit_write_only_primitive_stores: Vec::new(),
                scalar_structural_scalar_field_stores: Vec::new(),
                machine: machine_id(1),
                attachment: Some(structural_type),
                provenance: TerminalPsiProvenance {
                    operations: vec![operation_id(1)],
                    edges: vec![edge_id(1)],
                },
                bytes: vec![
                    0x48, 0x83, 0xec, 0x08, // 0: sub rsp, 8 — outbound call area
                    0xe8, 0, 0, 0, 0, // 4: call rel32 — relocation at 5
                    0x48, 0x83, 0xc4, 0x08, // 9: add rsp, 8 — call area release
                    0xc3, // 13: ret
                ],
                x86_scalar_fma: Vec::new(),
                x86_scalar_fma_occurrences: Vec::new(),
                x86_floating_control: None,
                unit_stack: Some(UnitStackEvidence {
                    frame: None,
                    aarch64_return_link: None,
                    stack_alignment: 16,
                }),
                unit_parameter_homes: vec![UnitParameterHomeRecord {
                    place: caller_place,
                    structural_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    shape: empty_shape,
                    source: empty_placement.clone(),
                    location: machine_code::StructuralSourceLocation::Stack { byte_offset: 0 },
                    indirect: false,
                }],
                unit_parameters: vec![UnitParameterRecord {
                    place: caller_place,
                    structural_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    shape: empty_shape,
                }],
                scalar_stack: None,
                internal_calls: vec![InternalCallRelocation {
                    owner: CallSiteOwner::Operation(operation_id(1)),
                    target: machine_id(2),
                    unit_stack: Some(UnitCallStackEvidence {
                        outbound: Some(stack_pair),
                    }),
                    scalar_stack: None,
                    offset: 5,
                }],
                foreign_calls: Vec::new(),
                internal_unit_calls: vec![InternalUnitCallRecord {
                    source: machine_code::InternalUnitCallSource::Authored,
                    owner: CallSiteOwner::Operation(operation_id(1)),
                    target: machine_id(2),
                    result: Some(i32_scalar),
                    semantic_result: Some(abstract_operations::AbstractResult {
                        value: result_value,
                        scalar_type: i32_scalar,
                    }),
                    structural_result: None,
                    scalar_arguments: Vec::new(),
                    arguments: vec![machine_code::InternalUnitCallArgumentRecord {
                        place: caller_place,
                        access: StructuralAccess::Owned,
                        path: Vec::new(),
                        root_structural_type: structural_type,
                        structural_type,
                        shape: empty_shape,
                        source_byte_offset: 0,
                        source_location: machine_code::StructuralSourceLocation::Stack {
                            byte_offset: 0,
                        },
                        call_stack_bytes: 8,
                        fixed_array_length: None,
                        element_stride: None,
                        source: machine_code::InternalUnitStructuralArgumentSourceRecord::Placement(
                            empty_placement.clone(),
                        ),
                        destination: call_plan.parameters[0].clone(),
                        code_offset: 4,
                        byte_count: 0,
                        bytes: Vec::new(),
                    }],
                    claim_transfers: Vec::new(),
                    operation_ordinal: 0,
                    code_offset: 0,
                    byte_count: 13,
                }],
                unit_continuations: Vec::new(),
                unit_affine_cleanup: Some(UnitAffineCleanupRecord {
                    structural_types: Vec::new().into(),
                    psi_edge: edge_id(1),
                    locals: Vec::new(),
                    actions: Vec::new(),
                    code_offset: 13,
                    byte_count: 1,
                }),
                semantic_code_attribution: vec![
                    SemanticCodeAttribution {
                        site: SemanticCodeSite::Operation(operation_id(1)),
                        operation_ordinal: 0,
                        code_offset: 0,
                        byte_count: 13,
                    },
                    SemanticCodeAttribution {
                        site: SemanticCodeSite::Edge(edge_id(1)),
                        operation_ordinal: 1,
                        code_offset: 13,
                        byte_count: 1,
                    },
                ],
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
                scalar_affine_cleanup: None,
                scalar_control_affine_cleanups: Vec::new(),
                scalar_structural_parameters: Vec::new(),
                scalar_structural_parameter_homes: Vec::new(),
                structural_return: None,
            },
            MachineCodeFunction {
                scalar_abi: None,
                mixed_structural_scalar_abi: Some(
                    target_operations::MixedStructuralScalarFunctionAbi {
                        scalar_parameters: Vec::new(),
                        structural_parameters: vec![target_operations::TargetStructuralParameter {
                            place: callee_place,
                            structural_type,
                            multiplicity: StructuralMultiplicity::Affine,
                            access: StructuralAccess::Owned,
                            projected_qualifications: Vec::new(),
                            shape: empty_shape,
                            placement: empty_placement.clone(),
                        }],
                        result: target_operations::ScalarAbiValue {
                            value: semantic_vocabulary::ValueId::new(90).expect("callee result"),
                            scalar_type: i32_scalar,
                            placement: call_plan.result.clone().expect("mixed result"),
                        },
                        call_plan: call_plan.clone(),
                    },
                ),
                structural_call_scalar_return: None,
                parameter_abi: None,
                internal_unit_scalar_calls: Vec::new(),
                installed_provider_unit_scalar_calls: Vec::new(),
                dynamic_calls: Vec::new(),
                stored_dynamic_calls: Vec::new(),
                dynamic_parameter_calls: Vec::new(),
                forwarded_dynamic_parameter_calls: Vec::new(),
                forwarded_dynamic_descriptor_calls: Vec::new(),
                unit_scalar_homes: Vec::new(),
                unit_integer_constants: Vec::new(),
                unit_affine_scalar_records: Vec::new(),
                unit_structural_scalar_field_stores: Vec::new(),
                unit_write_only_primitive_stores: Vec::new(),
                scalar_structural_scalar_field_stores: Vec::new(),
                machine: machine_id(2),
                attachment: None,
                provenance: TerminalPsiProvenance {
                    operations: Vec::new(),
                    edges: vec![edge_id(2)],
                },
                bytes: vec![0xc3],
                x86_scalar_fma: Vec::new(),
                x86_scalar_fma_occurrences: Vec::new(),
                x86_floating_control: None,
                unit_stack: None,
                unit_parameter_homes: Vec::new(),
                unit_parameters: Vec::new(),
                scalar_stack: Some(ScalarStackEvidence {
                    mutations: Vec::new(),
                    control_flow: ScalarControlFlowEvidence::Linear,
                    stack_alignment: 16,
                    cleanup_preservation: None,
                }),
                internal_calls: Vec::new(),
                foreign_calls: Vec::new(),
                internal_unit_calls: Vec::new(),
                unit_continuations: Vec::new(),
                unit_affine_cleanup: None,
                semantic_code_attribution: Vec::new(),
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
                scalar_affine_cleanup: None,
                scalar_control_affine_cleanups: Vec::new(),
                scalar_structural_parameters: vec![UnitParameterRecord {
                    place: callee_place,
                    structural_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    shape: empty_shape,
                }],
                scalar_structural_parameter_homes: vec![UnitParameterHomeRecord {
                    place: callee_place,
                    structural_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    shape: empty_shape,
                    source: empty_placement,
                    location: machine_code::StructuralSourceLocation::Stack { byte_offset: 0 },
                    indirect: false,
                }],
                structural_return: None,
            },
        ],
    }
}

/// The edge-owned cleanup caller extended with one byte-exact rebound dynamic
/// call so object construction materializes an ordinary dynamic conformance
/// table with one resolved and one unresolved slot.
pub(super) fn dynamic_conformance_table_plan() -> MachineCodePlan {
    let mut plan = edge_owned_cleanup_plan();
    let owner = machine_id(3);
    let operation = operation_id(4);
    let place = PlaceId::new(1).expect("dynamic source place");
    let structural_type = StructuralTypeId::new(1).expect("structural type");
    let empty_shape = calling_conventions::ValueShape::integer(0, 1);
    let empty_placement = calling_conventions::ValuePlacement {
        shape: empty_shape,
        locations: Vec::new(),
    };
    // The two-word descriptor travels as one indirect pointer argument.
    let descriptor_placement = calling_conventions::ValuePlacement {
        shape: calling_conventions::ValueShape::integer(16, 8),
        locations: vec![calling_conventions::ValueLocation::Indirect {
            pointer: calling_conventions::IndirectPointerLocation::Register(
                calling_conventions::MachineRegister::X86Rdi,
            ),
            copy_stack_byte_offset: None,
            byte_size: 16,
            alignment: 8,
        }],
    };
    let mut application = terminal_psi::ClosedConformanceApplication {
        owner,
        declaration_identity: "closed.decl".to_string(),
        telescope: Vec::new(),
        subject_identity: None,
        trait_identity: "closed.trait".to_string(),
        trait_lifetime_arguments: Vec::new(),
        trait_arguments: Vec::new(),
        realization_callables: vec![terminal_psi::ClosedConformanceRealizationCallable {
            source_callable_identity: "closed.callable.a".to_string(),
            machine: machine_id(2),
            result: terminal_psi::ClosedConformanceCallableResult::Unit,
        }],
        rows: vec![
            terminal_psi::ClosedConformanceRow {
                declaring_trait_identity: "closed.trait".to_string(),
                public_requirement_identity: "closed.req.a".to_string(),
                requirement_identity: "closed.req.a.impl".to_string(),
                realization_identity: "closed.real.a".to_string(),
                realization_callable_identity: Some("closed.callable.a".to_string()),
            },
            terminal_psi::ClosedConformanceRow {
                declaring_trait_identity: "closed.trait".to_string(),
                public_requirement_identity: "closed.req.b".to_string(),
                requirement_identity: "closed.req.b.impl".to_string(),
                realization_identity: "closed.real.b".to_string(),
                realization_callable_identity: None,
            },
        ],
        report_fingerprint: 0,
        commitment: terminal_psi::ClosedConformanceApplicationCommitment::default(),
    };
    application.report_fingerprint =
        terminal_psi::closed_conformance_application_report_fingerprint(&application);
    application.commitment = terminal_psi::closed_conformance_application_commitment(&application);
    assert_ne!(application.report_fingerprint, 0);
    assert!(!application.commitment.is_zero());
    let selection_source = terminal_psi::StructuralArgument {
        place,
        path: Vec::new(),
        access: StructuralAccess::SharedBorrow,
    };
    let initial = terminal_psi::TerminalDynamicConformanceSelection {
        owner,
        ordinal: 0,
        source: selection_source.clone(),
        conformance_application_report_fingerprint: application.report_fingerprint,
        conformance_application_commitment: application.commitment,
    };
    let rebound = terminal_psi::TerminalDynamicConformanceSelection {
        ordinal: 1,
        ..initial.clone()
    };
    let instance_source = target_operations::TargetStructuralArgument {
        place,
        access: StructuralAccess::SharedBorrow,
        path: Vec::new(),
        root_structural_type: structural_type,
        structural_type,
        shape: empty_shape,
        source_byte_offset: 0,
        fixed_array_length: None,
        element_stride: None,
        source: target_operations::TargetStructuralArgumentSource::Placement(
            empty_placement.clone(),
        ),
        destination: descriptor_placement.clone(),
    };
    let dynamic_call = machine_code::DynamicCallRecord {
        psi_operation: operation,
        dynamic_dispatch: abstract_operations::AbstractReboundDynamicDispatch {
            initial,
            rebound,
            descriptor: terminal_psi::TerminalReboundDynamicDescriptor {
                owner,
                ordinal: 0,
                initial_selection_ordinal: 0,
                rebound_selection_ordinal: 1,
            },
            initial_application: application.clone(),
            application,
            dispatch: terminal_psi::TerminalIndirectDynamicDispatch {
                owner,
                operation,
                descriptor_ordinal: 0,
                declaring_trait_identity: "closed.trait".to_string(),
                public_requirement_identity: "closed.req.a".to_string(),
                family_tuple: Vec::new(),
                requirement_identity: "closed.req.a.impl".to_string(),
                realization_identity: "closed.real.a".to_string(),
                realization_callable_identity: "closed.callable.a".to_string(),
                realization: machine_id(2),
            },
        },
        call_plan: calling_conventions::CallPlan {
            policy: calling_conventions::CallingPolicy::SystemVAMD64,
            parameters: vec![descriptor_placement.clone()],
            result: None,
            callback_materializations: Vec::new(),
            ordinary_clobbers: calling_conventions::RegisterSet::new([]),
            stack_alignment: 16,
            shadow_bytes: 0,
            entry_control: calling_conventions::EntryControl::CallReturn,
        },
        result: None,
        descriptor_abi: machine_code::DynamicTraitDescriptorAbiRecord {
            instance_byte_offset: 0,
            table_byte_offset: 8,
            word_byte_size: 8,
            total_byte_size: 16,
            byte_alignment: 8,
        },
        descriptor_home_byte_offset: 0,
        initial_instance: machine_code::DynamicInstanceMaterializationRecord {
            selection_ordinal: 0,
            source: instance_source.clone(),
            source_home_byte_offset: 0,
            source_home_indirect: false,
            code_offset: 4,
            byte_count: 9,
        },
        table_address: machine_code::DynamicTableAddressMaterialization {
            code_offset: 22,
            byte_count: 12,
            encoding: machine_code::DynamicTableAddressEncoding::X86_64Relative32 {
                relocation_offset: 25,
            },
        },
        rebound_instance: machine_code::DynamicInstanceMaterializationRecord {
            selection_ordinal: 1,
            source: instance_source,
            source_home_byte_offset: 0,
            source_home_indirect: false,
            code_offset: 13,
            byte_count: 9,
        },
        argument: machine_code::InternalUnitCallArgumentRecord {
            place,
            access: StructuralAccess::SharedBorrow,
            path: Vec::new(),
            root_structural_type: structural_type,
            structural_type,
            shape: empty_shape,
            source_byte_offset: 0,
            source_location: machine_code::StructuralSourceLocation::Stack { byte_offset: 0 },
            call_stack_bytes: 24,
            fixed_array_length: None,
            element_stride: None,
            source: machine_code::InternalUnitStructuralArgumentSourceRecord::Placement(
                empty_placement,
            ),
            destination: descriptor_placement,
            code_offset: 38,
            byte_count: 5,
            bytes: vec![0x48, 0x8b, 0x7c, 0x24, 0x18],
        },
        selected_table_byte_offset: 0,
        indirect_call_offset: 51,
        indirect_call_byte_count: 3,
        unit_stack: UnitCallStackEvidence {
            outbound: Some(StackAdjustmentPair {
                byte_size: 24,
                allocation_offset: 34,
                allocation_byte_count: 4,
                release_offset: 54,
                release_byte_count: 4,
            }),
        },
        operation_ordinal: 0,
        code_offset: 0,
        byte_count: 58,
    };

    let caller = &mut plan.functions[2];
    caller.attachment = Some(structural_type);
    caller.provenance.operations = vec![operation];
    caller.bytes = vec![
        // sub rsp, 16 — function frame holding the dynamic descriptor home.
        0x48, 0x83, 0xec, 0x10, //
        // lea r11, [rsp]; mov [rsp], r11 — initial instance word.
        0x4c, 0x8d, 0x1c, 0x24, 0x4c, 0x89, 0x5c, 0x24, 0x00, //
        // lea r11, [rsp]; mov [rsp], r11 — rebound instance word.
        0x4c, 0x8d, 0x1c, 0x24, 0x4c, 0x89, 0x5c, 0x24, 0x00, //
        // lea r10, [rip+rel32]; mov [rsp+8], r10 — table address word.
        0x4c, 0x8d, 0x15, 0x00, 0x00, 0x00, 0x00, 0x4c, 0x89, 0x54, 0x24, 0x08, //
        // sub rsp, 24 — outbound call frame.
        0x48, 0x83, 0xec, 0x18, //
        // mov rdi, [rsp+24] — argument descriptor pointer.
        0x48, 0x8b, 0x7c, 0x24, 0x18, //
        // mov r11, [rsp+32]; mov r11, [r11]; call r11.
        0x4c, 0x8b, 0x5c, 0x24, 0x20, 0x4d, 0x8b, 0x1b, 0x41, 0xff, 0xd3, //
        // add rsp, 24 — outbound release completes the operation span.
        0x48, 0x83, 0xc4, 0x18, //
        // Original edge-owned cleanup tail, shifted by 58 bytes.
        0x48, 0x83, 0xec, 0x08, 0xe8, 0x00, 0x00, 0x00, 0x00, 0x48, 0x83, 0xc4, 0x08, //
        // add rsp, 16; ret — frame release and return close the edge span.
        0x48, 0x83, 0xc4, 0x10, 0xc3,
    ];
    caller.unit_stack = Some(UnitStackEvidence {
        frame: Some(StackAdjustmentPair {
            byte_size: 16,
            allocation_offset: 0,
            allocation_byte_count: 4,
            release_offset: 71,
            release_byte_count: 4,
        }),
        aarch64_return_link: None,
        stack_alignment: 16,
    });
    caller.dynamic_calls = vec![dynamic_call];
    caller.internal_calls[0].offset = 63;
    caller.internal_calls[0].unit_stack = Some(UnitCallStackEvidence {
        outbound: Some(StackAdjustmentPair {
            byte_size: 8,
            allocation_offset: 58,
            allocation_byte_count: 4,
            release_offset: 67,
            release_byte_count: 4,
        }),
    });
    caller.internal_unit_calls[0].code_offset = 58;
    caller.internal_unit_calls[0].operation_ordinal = 1;
    let cleanup = caller
        .unit_affine_cleanup
        .as_mut()
        .expect("Unit cleanup fixture");
    cleanup.code_offset = 58;
    cleanup.byte_count = 18;
    caller.semantic_code_attribution = vec![
        SemanticCodeAttribution {
            site: SemanticCodeSite::Operation(operation),
            operation_ordinal: 0,
            code_offset: 0,
            byte_count: 58,
        },
        SemanticCodeAttribution {
            site: SemanticCodeSite::Edge(edge_id(3)),
            operation_ordinal: 1,
            code_offset: 58,
            byte_count: 18,
        },
    ];
    plan
}

/// One scalar Terminal function whose body performs a single indirect dispatch
/// through an existential descriptor parameter: the descriptor's table word
/// arrives in `rsi`, so `call qword ptr [rsi]` selects requirement slot zero.
/// The record carries the complete semantic, ABI, register, and byte custody
/// that object replay must re-derive from the emitted bytes alone.
pub(super) fn dynamic_parameter_call_plan() -> MachineCodePlan {
    let target = NativeTarget::linux_x64();
    let mut plan = internal_call_plan(target);
    let caller = &mut plan.functions[1];
    caller.bytes = vec![
        0x50, // push rax — outbound call frame keeps the call site 16-aligned
        0xff, 0x96, 0, 0, 0, 0,    // call qword ptr [rsi] — descriptor table slot zero
        0x58, // pop rax
        0xc3, // ret
    ];
    caller.internal_calls = Vec::new();
    caller.scalar_stack = Some(ScalarStackEvidence {
        mutations: vec![
            scalar_mutation(0, 1, ScalarStackMutationKind::X86Push),
            scalar_mutation(7, 1, ScalarStackMutationKind::X86Pop),
        ],
        control_flow: ScalarControlFlowEvidence::Linear,
        stack_alignment: 16,
        cleanup_preservation: None,
    });
    let pointer = ValueShape::integer(8, 8);
    let policy = calling_conventions::CallingPolicy::native_for_target(target);
    let function_call_plan = calling_conventions::evaluate_call_plan(
        policy,
        &calling_conventions::CallSignature {
            parameters: vec![pointer, pointer],
            result: None,
        },
    )
    .expect("descriptor-parameter entry ABI");
    let dispatch_call_plan = calling_conventions::evaluate_call_plan(
        policy,
        &calling_conventions::CallSignature {
            parameters: vec![pointer],
            result: None,
        },
    )
    .expect("erased adapter ABI");
    let requirement = terminal_psi::TerminalDynamicRequirement {
        slot: 0,
        declaring_trait_identity: "dyn.trait".to_string(),
        public_requirement_identity: "dyn.req".to_string(),
        family_tuple: Vec::new(),
        result: terminal_psi::ClosedConformanceCallableResult::Unit,
    };
    caller.dynamic_parameter_calls = vec![machine_code::DynamicParameterCallRecord {
        psi_edge: edge_id(2),
        psi_operation: operation_id(2),
        source_value: None,
        scalar_type: None,
        parameter: terminal_psi::TerminalDynamicDescriptorParameter {
            owner: machine_id(2),
            ordinal: 0,
            source_position: 0,
            trait_identity: "dyn.trait".to_string(),
            access: StructuralAccess::SharedBorrow,
            requirements: vec![requirement.clone()],
        },
        requirement,
        function_call_plan,
        dispatch_call_plan,
        instance: calling_conventions::MachineRegister::X86Rdi,
        table: calling_conventions::MachineRegister::X86Rsi,
        table_slot_byte_offset: 0,
        mechanism: machine_code::DynamicParameterCallMechanismRecord::X86MemoryIndirect {
            table: calling_conventions::MachineRegister::X86Rsi,
        },
        indirect_call_offset: 1,
        indirect_call_byte_count: 6,
        call_stack: ScalarCallStackEvidence {
            outbound: None,
            aarch64_return_link: None,
        },
        operation_ordinal: 0,
        code_offset: 0,
        byte_count: 8,
    }];
    plan
}

/// The edge-owned cleanup caller extended with one stored-descriptor
/// establishment and one later dispatch through the same aggregate slot. The
/// frame holds the descriptor pair at offset zero and the signed i32 result
/// home at offset sixteen, so every retained field below is re-derived from
/// these bytes during installation.
pub(super) fn stored_dynamic_call_plan() -> MachineCodePlan {
    let mut plan = edge_owned_cleanup_plan();
    let owner = machine_id(3);
    let establishment_operation = operation_id(4);
    let call_operation = operation_id(5);
    let place = PlaceId::new(1).expect("dynamic source place");
    let structural_type = StructuralTypeId::new(1).expect("structural type");
    let i32_type = semantic_vocabulary::ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
            .expect("i32 scalar type"),
    );
    let result_shape = ValueShape::integer(4, 4);
    let empty_shape = ValueShape::integer(0, 1);
    let empty_placement = ValuePlacement {
        shape: empty_shape,
        locations: Vec::new(),
    };
    let descriptor_placement = ValuePlacement {
        shape: ValueShape::integer(16, 8),
        locations: vec![calling_conventions::ValueLocation::Indirect {
            pointer: calling_conventions::IndirectPointerLocation::Register(
                calling_conventions::MachineRegister::X86Rdi,
            ),
            copy_stack_byte_offset: None,
            byte_size: 16,
            alignment: 8,
        }],
    };
    let result_placement = ValuePlacement {
        shape: result_shape,
        locations: vec![calling_conventions::ValueLocation::Register {
            register: calling_conventions::MachineRegister::X86Rax,
            value_byte_offset: 0,
            byte_size: 4,
        }],
    };
    let mut application = terminal_psi::ClosedConformanceApplication {
        owner,
        declaration_identity: "closed.decl".to_string(),
        telescope: Vec::new(),
        subject_identity: None,
        trait_identity: "closed.trait".to_string(),
        trait_lifetime_arguments: Vec::new(),
        trait_arguments: Vec::new(),
        realization_callables: vec![terminal_psi::ClosedConformanceRealizationCallable {
            source_callable_identity: "closed.callable.a".to_string(),
            machine: machine_id(2),
            result: terminal_psi::ClosedConformanceCallableResult::I32,
        }],
        rows: vec![
            terminal_psi::ClosedConformanceRow {
                declaring_trait_identity: "closed.trait".to_string(),
                public_requirement_identity: "closed.req.a".to_string(),
                requirement_identity: "closed.req.a.impl".to_string(),
                realization_identity: "closed.real.a".to_string(),
                realization_callable_identity: Some("closed.callable.a".to_string()),
            },
            terminal_psi::ClosedConformanceRow {
                declaring_trait_identity: "closed.trait".to_string(),
                public_requirement_identity: "closed.req.b".to_string(),
                requirement_identity: "closed.req.b.impl".to_string(),
                realization_identity: "closed.real.b".to_string(),
                realization_callable_identity: None,
            },
        ],
        report_fingerprint: 0,
        commitment: terminal_psi::ClosedConformanceApplicationCommitment::default(),
    };
    application.report_fingerprint =
        terminal_psi::closed_conformance_application_report_fingerprint(&application);
    application.commitment = terminal_psi::closed_conformance_application_commitment(&application);
    let stored = abstract_operations::AbstractStoredDynamicDescriptor {
        selection: terminal_psi::TerminalDynamicConformanceSelection {
            owner,
            ordinal: 0,
            source: StructuralArgument {
                place,
                path: Vec::new(),
                access: StructuralAccess::SharedBorrow,
            },
            conformance_application_report_fingerprint: application.report_fingerprint,
            conformance_application_commitment: application.commitment,
        },
        descriptor: terminal_psi::TerminalStoredDynamicDescriptor {
            owner,
            ordinal: 0,
            establishment_operation,
            selection_ordinal: 0,
            aggregate_type_identity: "closed.aggregate".to_string(),
            field_identity: "closed.field".to_string(),
        },
        application,
    };
    let instance_source = target_operations::TargetStructuralArgument {
        place,
        access: StructuralAccess::SharedBorrow,
        path: Vec::new(),
        root_structural_type: structural_type,
        structural_type,
        shape: empty_shape,
        source_byte_offset: 0,
        fixed_array_length: None,
        element_stride: None,
        source: target_operations::TargetStructuralArgumentSource::Placement(
            empty_placement.clone(),
        ),
        destination: descriptor_placement.clone(),
    };
    let result_home = machine_code::UnitScalarHomeRecord {
        defining_operation: call_operation,
        source_value: semantic_vocabulary::ValueId::new(80).expect("stored result value"),
        scalar_type: i32_type,
        shape: result_shape,
        byte_offset: 16,
    };
    let stored_call = machine_code::StoredDynamicCallRecord {
        establishment: machine_code::StoredDynamicDescriptorMaterializationRecord {
            psi_operation: establishment_operation,
            stored: stored.clone(),
            descriptor_abi: machine_code::DynamicTraitDescriptorAbiRecord {
                instance_byte_offset: 0,
                table_byte_offset: 8,
                word_byte_size: 8,
                total_byte_size: 16,
                byte_alignment: 8,
            },
            descriptor_home_byte_offset: 0,
            instance: machine_code::DynamicInstanceMaterializationRecord {
                selection_ordinal: 0,
                source: instance_source,
                source_home_byte_offset: 0,
                source_home_indirect: false,
                code_offset: 4,
                byte_count: 9,
            },
            table_address: machine_code::DynamicTableAddressMaterialization {
                code_offset: 13,
                byte_count: 12,
                encoding: machine_code::DynamicTableAddressEncoding::X86_64Relative32 {
                    relocation_offset: 16,
                },
            },
            operation_ordinal: 0,
            code_offset: 0,
            byte_count: 25,
        },
        psi_operation: call_operation,
        dynamic_dispatch: abstract_operations::AbstractStoredDynamicDispatch {
            stored,
            dispatch: terminal_psi::TerminalStoredDynamicDispatch {
                owner,
                operation: call_operation,
                descriptor_ordinal: 0,
                declaring_trait_identity: "closed.trait".to_string(),
                public_requirement_identity: "closed.req.a".to_string(),
                family_tuple: Vec::new(),
                requirement_identity: "closed.req.a.impl".to_string(),
                realization_identity: "closed.real.a".to_string(),
                realization_callable_identity: "closed.callable.a".to_string(),
                realization: machine_id(2),
            },
        },
        call_plan: calling_conventions::CallPlan {
            policy: calling_conventions::CallingPolicy::SystemVAMD64,
            parameters: vec![descriptor_placement.clone()],
            result: Some(result_placement.clone()),
            callback_materializations: Vec::new(),
            ordinary_clobbers: calling_conventions::RegisterSet::new([]),
            stack_alignment: 16,
            shadow_bytes: 0,
            entry_control: calling_conventions::EntryControl::CallReturn,
        },
        result: machine_code::InternalUnitScalarCallResultRecord {
            home: result_home,
            source: result_placement,
            code_offset: 49,
            byte_count: 8,
        },
        argument: machine_code::InternalUnitCallArgumentRecord {
            place,
            access: StructuralAccess::SharedBorrow,
            path: Vec::new(),
            root_structural_type: structural_type,
            structural_type,
            shape: empty_shape,
            source_byte_offset: 0,
            source_location: machine_code::StructuralSourceLocation::Stack { byte_offset: 0 },
            call_stack_bytes: 24,
            fixed_array_length: None,
            element_stride: None,
            source: machine_code::InternalUnitStructuralArgumentSourceRecord::Placement(
                empty_placement,
            ),
            destination: descriptor_placement,
            code_offset: 29,
            byte_count: 5,
            bytes: vec![0x48, 0x8b, 0x7c, 0x24, 0x18],
        },
        selected_table_byte_offset: 0,
        indirect_call_offset: 42,
        indirect_call_byte_count: 3,
        unit_stack: UnitCallStackEvidence {
            outbound: Some(StackAdjustmentPair {
                byte_size: 24,
                allocation_offset: 25,
                allocation_byte_count: 4,
                release_offset: 45,
                release_byte_count: 4,
            }),
        },
        operation_ordinal: 1,
        code_offset: 25,
        byte_count: 32,
    };
    let caller = &mut plan.functions[2];
    caller.attachment = Some(structural_type);
    caller.provenance.operations = vec![establishment_operation, call_operation];
    caller.bytes = vec![
        // sub rsp, 32 — frame holding the stored descriptor and result home.
        0x48, 0x83, 0xec, 0x20, //
        // lea r11, [rsp]; mov [rsp], r11 — descriptor instance word.
        0x4c, 0x8d, 0x1c, 0x24, 0x4c, 0x89, 0x5c, 0x24, 0x00, //
        // lea r10, [rip+rel32]; mov [rsp+8], r10 — descriptor table word.
        0x4c, 0x8d, 0x15, 0x00, 0x00, 0x00, 0x00, 0x4c, 0x89, 0x54, 0x24, 0x08, //
        // sub rsp, 24 — outbound call frame.
        0x48, 0x83, 0xec, 0x18, //
        // mov rdi, [rsp+24] — descriptor instance word argument.
        0x48, 0x8b, 0x7c, 0x24, 0x18, //
        // mov r11, [rsp+32]; mov r11, [r11]; call r11 — selected slot dispatch.
        0x4c, 0x8b, 0x5c, 0x24, 0x20, 0x4d, 0x8b, 0x1b, 0x41, 0xff, 0xd3, //
        // add rsp, 24 — outbound release.
        0x48, 0x83, 0xc4, 0x18, //
        // movsxd rax, eax; mov [rsp+16], rax — i32 result home store.
        0x48, 0x63, 0xc0, 0x48, 0x89, 0x44, 0x24, 0x10, //
        // Edge-owned cleanup tail: sub rsp, 8; call rel32; add rsp, 8.
        0x48, 0x83, 0xec, 0x08, 0xe8, 0x00, 0x00, 0x00, 0x00, 0x48, 0x83, 0xc4, 0x08, //
        // add rsp, 32; ret — frame release and return close the edge span.
        0x48, 0x83, 0xc4, 0x20, 0xc3,
    ];
    caller.unit_stack = Some(UnitStackEvidence {
        frame: Some(StackAdjustmentPair {
            byte_size: 32,
            allocation_offset: 0,
            allocation_byte_count: 4,
            release_offset: 70,
            release_byte_count: 4,
        }),
        aarch64_return_link: None,
        stack_alignment: 16,
    });
    caller.unit_scalar_homes = vec![result_home];
    caller.stored_dynamic_calls = vec![stored_call];
    caller.internal_calls[0].offset = 62;
    caller.internal_calls[0].unit_stack = Some(UnitCallStackEvidence {
        outbound: Some(StackAdjustmentPair {
            byte_size: 8,
            allocation_offset: 57,
            allocation_byte_count: 4,
            release_offset: 66,
            release_byte_count: 4,
        }),
    });
    caller.internal_unit_calls[0].code_offset = 57;
    caller.internal_unit_calls[0].operation_ordinal = 2;
    let cleanup = caller
        .unit_affine_cleanup
        .as_mut()
        .expect("Unit cleanup fixture");
    cleanup.code_offset = 57;
    cleanup.byte_count = 18;
    caller.semantic_code_attribution = vec![
        SemanticCodeAttribution {
            site: SemanticCodeSite::Operation(establishment_operation),
            operation_ordinal: 0,
            code_offset: 0,
            byte_count: 25,
        },
        SemanticCodeAttribution {
            site: SemanticCodeSite::Operation(call_operation),
            operation_ordinal: 1,
            code_offset: 25,
            byte_count: 32,
        },
        SemanticCodeAttribution {
            site: SemanticCodeSite::Edge(edge_id(3)),
            operation_ordinal: 2,
            code_offset: 57,
            byte_count: 18,
        },
    ];
    plan
}

/// One transparent descriptor-parameter helper (machine 2) forwarding its own
/// parameter unchanged into machine 1's existential dispatch body. Both the
/// helper's forwarded-parameter custody and the callee's dispatch custody are
/// retained for installation.
pub(super) fn forwarded_dynamic_parameter_call_plan() -> MachineCodePlan {
    let target = NativeTarget::linux_x64();
    let mut plan = internal_call_plan(target);
    let i32_type = semantic_vocabulary::ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
            .expect("i32 scalar type"),
    );
    let pointer = ValueShape::integer(8, 8);
    let policy = calling_conventions::CallingPolicy::native_for_target(target);
    let call_plan = calling_conventions::evaluate_call_plan(
        policy,
        &calling_conventions::CallSignature {
            parameters: vec![pointer, pointer],
            result: Some(ValueShape::integer(4, 4)),
        },
    )
    .expect("forwarded descriptor ABI");
    let dispatch_plan = calling_conventions::evaluate_call_plan(
        policy,
        &calling_conventions::CallSignature {
            parameters: vec![pointer],
            result: Some(ValueShape::integer(4, 4)),
        },
    )
    .expect("parameter dispatch ABI");
    let requirement = terminal_psi::TerminalDynamicRequirement {
        slot: 0,
        declaring_trait_identity: "dyn.trait".to_string(),
        public_requirement_identity: "dyn.req".to_string(),
        family_tuple: Vec::new(),
        result: terminal_psi::ClosedConformanceCallableResult::I32,
    };
    // The callee owns one existential descriptor parameter and dispatches its
    // requirement slot zero through the inbound table register.
    let callee_parameter = terminal_psi::TerminalDynamicDescriptorParameter {
        owner: machine_id(1),
        ordinal: 0,
        source_position: 0,
        trait_identity: "dyn.trait".to_string(),
        access: StructuralAccess::SharedBorrow,
        requirements: vec![requirement.clone()],
    };
    let callee = &mut plan.functions[0];
    callee.bytes = vec![
        0x50, // push rax — outbound call frame keeps the call site 16-aligned
        0xff, 0x96, 0, 0, 0, 0,    // call qword ptr [rsi] — descriptor table slot zero
        0x58, // pop rax
        0xc3, // ret
    ];
    callee.scalar_stack = Some(ScalarStackEvidence {
        mutations: vec![
            scalar_mutation(0, 1, ScalarStackMutationKind::X86Push),
            scalar_mutation(7, 1, ScalarStackMutationKind::X86Pop),
        ],
        control_flow: ScalarControlFlowEvidence::Linear,
        stack_alignment: 16,
        cleanup_preservation: None,
    });
    callee.dynamic_parameter_calls = vec![machine_code::DynamicParameterCallRecord {
        psi_edge: edge_id(1),
        psi_operation: operation_id(1),
        source_value: Some(semantic_vocabulary::ValueId::new(90).expect("dispatch result value")),
        scalar_type: Some(i32_type),
        parameter: callee_parameter.clone(),
        requirement: requirement.clone(),
        function_call_plan: call_plan.clone(),
        dispatch_call_plan: dispatch_plan,
        instance: calling_conventions::MachineRegister::X86Rdi,
        table: calling_conventions::MachineRegister::X86Rsi,
        table_slot_byte_offset: 0,
        mechanism: machine_code::DynamicParameterCallMechanismRecord::X86MemoryIndirect {
            table: calling_conventions::MachineRegister::X86Rsi,
        },
        indirect_call_offset: 1,
        indirect_call_byte_count: 6,
        call_stack: ScalarCallStackEvidence {
            outbound: None,
            aarch64_return_link: None,
        },
        operation_ordinal: 0,
        code_offset: 0,
        byte_count: 8,
    }];
    // The forwarder exposes the same descriptor parameter and passes it
    // unchanged to the callee through one aligned direct call.
    let forwarder_parameter = terminal_psi::TerminalDynamicDescriptorParameter {
        owner: machine_id(2),
        ordinal: 0,
        source_position: 0,
        trait_identity: "dyn.trait".to_string(),
        access: StructuralAccess::SharedBorrow,
        requirements: vec![requirement],
    };
    let call_stack = ScalarCallStackEvidence {
        outbound: None,
        aarch64_return_link: None,
    };
    let forwarder = &mut plan.functions[1];
    forwarder.bytes = vec![
        0x50, // push rax — outbound call frame keeps the call site 16-aligned
        0xe8, 0, 0, 0, 0,    // call machine 1
        0x58, // pop rax
        0xc3, // ret
    ];
    forwarder.scalar_stack = Some(ScalarStackEvidence {
        mutations: vec![
            scalar_mutation(0, 1, ScalarStackMutationKind::X86Push),
            scalar_mutation(6, 1, ScalarStackMutationKind::X86Pop),
        ],
        control_flow: ScalarControlFlowEvidence::Linear,
        stack_alignment: 16,
        cleanup_preservation: None,
    });
    forwarder.internal_calls = vec![InternalCallRelocation {
        owner: CallSiteOwner::Operation(operation_id(2)),
        target: machine_id(1),
        unit_stack: None,
        scalar_stack: Some(call_stack),
        offset: 2,
    }];
    forwarder.forwarded_dynamic_parameter_calls =
        vec![machine_code::ForwardedDynamicParameterCallRecord {
            psi_edge: edge_id(2),
            psi_operation: operation_id(2),
            source_value: Some(semantic_vocabulary::ValueId::new(91).expect("forwarded result")),
            scalar_type: Some(i32_type),
            callee: machine_id(1),
            argument: abstract_operations::AbstractDynamicDescriptorArgument {
                argument: terminal_psi::TerminalDynamicDescriptorArgument {
                    owner: machine_id(2),
                    operation: operation_id(2),
                    parameter_ordinal: 0,
                    source: terminal_psi::TerminalDynamicDescriptorSource::Parameter { ordinal: 0 },
                },
                target: callee_parameter,
                source: abstract_operations::AbstractDynamicDescriptorSource::Parameter(
                    forwarder_parameter.clone(),
                ),
            },
            parameter: forwarder_parameter,
            function_call_plan: call_plan.clone(),
            callee_call_plan: call_plan,
            instance: calling_conventions::MachineRegister::X86Rdi,
            table: calling_conventions::MachineRegister::X86Rsi,
            instance_destination: calling_conventions::MachineRegister::X86Rdi,
            table_destination: calling_conventions::MachineRegister::X86Rsi,
            direct_call_offset: 1,
            direct_call_byte_count: 5,
            call_stack: machine_code::ForwardedDynamicParameterCallStackEvidence::Scalar(
                call_stack,
            ),
            operation_ordinal: 0,
            code_offset: 0,
            byte_count: 7,
        }];
    forwarder.semantic_code_attribution = vec![
        SemanticCodeAttribution {
            site: SemanticCodeSite::Operation(operation_id(2)),
            operation_ordinal: 0,
            code_offset: 0,
            byte_count: 7,
        },
        SemanticCodeAttribution {
            site: SemanticCodeSite::Edge(edge_id(2)),
            operation_ordinal: 1,
            code_offset: 7,
            byte_count: 1,
        },
    ];
    plan
}

/// The edge-owned cleanup caller extended with one forwarded existential
/// descriptor argument whose single-row closed application emits one adapter
/// plus one forwarded table. The call itself has a Unit requirement result.
pub(super) fn forwarded_dynamic_descriptor_call_plan() -> MachineCodePlan {
    let mut plan = edge_owned_cleanup_plan();
    let owner = machine_id(3);
    let operation = operation_id(4);
    let place = PlaceId::new(1).expect("forwarded source place");
    let structural_type = StructuralTypeId::new(1).expect("structural type");
    let pointer = ValueShape::integer(8, 8);
    let policy = calling_conventions::CallingPolicy::native_for_target(NativeTarget::linux_x64());
    let call_plan = calling_conventions::evaluate_call_plan(
        policy,
        &calling_conventions::CallSignature {
            parameters: vec![pointer, pointer],
            result: None,
        },
    )
    .expect("forwarded descriptor ABI");
    let adapter_call_plan = calling_conventions::evaluate_call_plan(
        policy,
        &calling_conventions::CallSignature {
            parameters: vec![pointer],
            result: None,
        },
    )
    .expect("adapter ABI");
    let requirement = terminal_psi::TerminalDynamicRequirement {
        slot: 0,
        declaring_trait_identity: "fwd.trait".to_string(),
        public_requirement_identity: "fwd.req".to_string(),
        family_tuple: Vec::new(),
        result: terminal_psi::ClosedConformanceCallableResult::Unit,
    };
    let mut application = terminal_psi::ClosedConformanceApplication {
        owner,
        declaration_identity: "fwd.decl".to_string(),
        telescope: Vec::new(),
        subject_identity: None,
        trait_identity: "fwd.trait".to_string(),
        trait_lifetime_arguments: Vec::new(),
        trait_arguments: Vec::new(),
        realization_callables: vec![terminal_psi::ClosedConformanceRealizationCallable {
            source_callable_identity: "fwd.callable.a".to_string(),
            machine: machine_id(2),
            result: terminal_psi::ClosedConformanceCallableResult::Unit,
        }],
        rows: vec![terminal_psi::ClosedConformanceRow {
            declaring_trait_identity: "fwd.trait".to_string(),
            public_requirement_identity: "fwd.req".to_string(),
            requirement_identity: "fwd.req.impl".to_string(),
            realization_identity: "fwd.real.a".to_string(),
            realization_callable_identity: Some("fwd.callable.a".to_string()),
        }],
        report_fingerprint: 0,
        commitment: terminal_psi::ClosedConformanceApplicationCommitment::default(),
    };
    application.report_fingerprint =
        terminal_psi::closed_conformance_application_report_fingerprint(&application);
    application.commitment = terminal_psi::closed_conformance_application_commitment(&application);
    let adapter = machine_code::ForwardedDynamicDescriptorAdapterRecord {
        identity: machine_code::ForwardedDynamicDescriptorAdapterIdentity {
            application: application.commitment,
            row_index: 0,
            realization: machine_id(2),
        },
        requirement_identity: "fwd.req.impl".to_string(),
        realization_identity: "fwd.real.a".to_string(),
        realization_callable_identity: "fwd.callable.a".to_string(),
        result: terminal_psi::ClosedConformanceCallableResult::Unit,
        erased_call_plan: adapter_call_plan.clone(),
        realization_call_plan: adapter_call_plan,
        source_shape: pointer,
        bytes: vec![
            // mov rdi, [rdi] — load the shared-borrow instance word.
            0x48, 0x8b, 0x3f, //
            // sub rsp, 8; call rel32; add rsp, 8 — aligned adapter tail call.
            0x48, 0x83, 0xec, 0x08, 0xe8, 0x00, 0x00, 0x00, 0x00, 0x48, 0x83, 0xc4, 0x08, //
            0xc3,
        ],
        argument_code_offset: 0,
        argument_byte_count: 3,
        direct_call_offset: 7,
        direct_call_byte_count: 5,
        return_offset: 16,
        return_byte_count: 1,
    };
    let argument = machine_code::ForwardedDynamicDescriptorArgumentRecord {
        custody: abstract_operations::AbstractDynamicDescriptorArgument {
            argument: terminal_psi::TerminalDynamicDescriptorArgument {
                owner,
                operation,
                parameter_ordinal: 0,
                source: terminal_psi::TerminalDynamicDescriptorSource::Selection { ordinal: 0 },
            },
            target: terminal_psi::TerminalDynamicDescriptorParameter {
                owner: machine_id(1),
                ordinal: 0,
                source_position: 0,
                trait_identity: "fwd.trait".to_string(),
                access: StructuralAccess::SharedBorrow,
                requirements: vec![requirement],
            },
            source: abstract_operations::AbstractDynamicDescriptorSource::Selection {
                selection: terminal_psi::TerminalDynamicConformanceSelection {
                    owner,
                    ordinal: 0,
                    source: StructuralArgument {
                        place,
                        path: Vec::new(),
                        access: StructuralAccess::SharedBorrow,
                    },
                    conformance_application_report_fingerprint: application.report_fingerprint,
                    conformance_application_commitment: application.commitment,
                },
                application,
            },
        },
        instance: target_operations::TargetDynamicDescriptorInstanceArgument {
            place,
            access: StructuralAccess::SharedBorrow,
            path: Vec::new(),
            root_structural_type: structural_type,
            structural_type,
            shape: pointer,
            source_byte_offset: 0,
            source: ValuePlacement {
                shape: pointer,
                locations: vec![calling_conventions::ValueLocation::Stack {
                    stack_byte_offset: 0,
                    value_byte_offset: 0,
                    byte_size: 8,
                    alignment: 8,
                }],
            },
            destination: ValuePlacement {
                shape: pointer,
                locations: vec![calling_conventions::ValueLocation::Register {
                    register: calling_conventions::MachineRegister::X86Rdi,
                    value_byte_offset: 0,
                    byte_size: 8,
                }],
            },
        },
        instance_destination: calling_conventions::MachineRegister::X86Rdi,
        table_destination: calling_conventions::MachineRegister::X86Rsi,
        source_home_byte_offset: 0,
        source_home_indirect: false,
        instance_code_offset: 7,
        instance_byte_count: 4,
        table_address: machine_code::DynamicTableAddressMaterialization {
            code_offset: 0,
            byte_count: 7,
            encoding: machine_code::DynamicTableAddressEncoding::X86_64Relative32 {
                relocation_offset: 3,
            },
        },
        adapters: vec![adapter],
    };
    let caller = &mut plan.functions[2];
    caller.attachment = Some(structural_type);
    caller.provenance.operations = vec![operation];
    caller.bytes = vec![
        // lea rsi, [rip+rel32] — forwarded table address (relocation at +3).
        0x48, 0x8d, 0x35, 0x00, 0x00, 0x00, 0x00, //
        // lea rdi, [rsp] — instance pointer materialization.
        0x48, 0x8d, 0x3c, 0x24, //
        // sub rsp, 8; call rel32; add rsp, 8 — aligned direct call to the callee.
        0x48, 0x83, 0xec, 0x08, 0xe8, 0x00, 0x00, 0x00, 0x00, 0x48, 0x83, 0xc4, 0x08, //
        // Edge-owned cleanup tail: sub rsp, 8; call rel32; add rsp, 8.
        0x48, 0x83, 0xec, 0x08, 0xe8, 0x00, 0x00, 0x00, 0x00, 0x48, 0x83, 0xc4, 0x08, //
        0xc3,
    ];
    caller.internal_calls = vec![
        InternalCallRelocation {
            owner: CallSiteOwner::Operation(operation),
            target: machine_id(1),
            unit_stack: Some(UnitCallStackEvidence {
                outbound: Some(StackAdjustmentPair {
                    byte_size: 8,
                    allocation_offset: 11,
                    allocation_byte_count: 4,
                    release_offset: 20,
                    release_byte_count: 4,
                }),
            }),
            scalar_stack: None,
            offset: 16,
        },
        InternalCallRelocation {
            owner: CallSiteOwner::CleanupAction {
                edge: edge_id(3),
                action_ordinal: 0,
            },
            target: machine_id(1),
            unit_stack: Some(UnitCallStackEvidence {
                outbound: Some(StackAdjustmentPair {
                    byte_size: 8,
                    allocation_offset: 24,
                    allocation_byte_count: 4,
                    release_offset: 33,
                    release_byte_count: 4,
                }),
            }),
            scalar_stack: None,
            offset: 29,
        },
    ];
    caller.internal_unit_calls[0].code_offset = 24;
    caller.internal_unit_calls[0].operation_ordinal = 1;
    let cleanup = caller
        .unit_affine_cleanup
        .as_mut()
        .expect("Unit cleanup fixture");
    cleanup.code_offset = 24;
    cleanup.byte_count = 14;
    caller.forwarded_dynamic_descriptor_calls =
        vec![machine_code::ForwardedDynamicDescriptorCallRecord {
            psi_operation: operation,
            semantic_result: None,
            result: None,
            callee: machine_id(1),
            call_plan,
            dynamic_arguments: vec![argument],
            claim_transfers: Vec::new(),
            direct_call_offset: 15,
            direct_call_byte_count: 5,
            unit_stack: UnitCallStackEvidence {
                outbound: Some(StackAdjustmentPair {
                    byte_size: 8,
                    allocation_offset: 11,
                    allocation_byte_count: 4,
                    release_offset: 20,
                    release_byte_count: 4,
                }),
            },
            operation_ordinal: 0,
            code_offset: 0,
            byte_count: 24,
        }];
    caller.semantic_code_attribution = vec![
        SemanticCodeAttribution {
            site: SemanticCodeSite::Operation(operation),
            operation_ordinal: 0,
            code_offset: 0,
            byte_count: 24,
        },
        SemanticCodeAttribution {
            site: SemanticCodeSite::Edge(edge_id(3)),
            operation_ordinal: 1,
            code_offset: 24,
            byte_count: 14,
        },
    ];
    plan
}

pub(super) fn mixed_edge_owned_cleanup_plan() -> MachineCodePlan {
    let mut plan = edge_owned_cleanup_plan();
    let caller = &mut plan.functions[2];
    let trivial_place = PlaceId::new(2).expect("trivial place");
    let trivial_type = StructuralTypeId::new(3).expect("trivial type");
    let mut trivial_parameter = caller.unit_parameters[0];
    trivial_parameter.place = trivial_place;
    trivial_parameter.structural_type = trivial_type;
    let mut trivial_home = caller.unit_parameter_homes[0].clone();
    trivial_home.place = trivial_place;
    trivial_home.structural_type = trivial_type;
    caller.unit_parameters.push(trivial_parameter);
    caller.unit_parameter_homes.push(trivial_home);
    caller
        .unit_affine_cleanup
        .as_mut()
        .expect("Unit cleanup fixture")
        .actions
        .insert(0, TerminalAffineCleanupAction::DiscardRoot(trivial_place));
    caller.internal_calls[0].owner = CallSiteOwner::CleanupAction {
        edge: edge_id(3),
        action_ordinal: 1,
    };
    caller.internal_unit_calls[0].owner = caller.internal_calls[0].owner;
    plan
}

pub(super) fn two_call_edge_owned_cleanup_plan() -> MachineCodePlan {
    let mut plan = edge_owned_cleanup_plan();
    let second_operation = operation_id(2);
    let second_helper = machine_id(4);
    let second_edge = edge_id(4);
    let second_call_bytes = [
        0x48, 0x83, 0xec, 0x08, 0xe8, 0, 0, 0, 0, 0x48, 0x83, 0xc4, 0x08,
    ];
    let drop = &mut plan.functions[0];
    drop.bytes.splice(13..13, second_call_bytes);
    drop.provenance.operations.push(second_operation);
    drop.internal_calls.push(InternalCallRelocation {
        owner: CallSiteOwner::Operation(second_operation),
        target: second_helper,
        unit_stack: Some(UnitCallStackEvidence {
            outbound: Some(StackAdjustmentPair {
                byte_size: 8,
                allocation_offset: 13,
                allocation_byte_count: 4,
                release_offset: 22,
                release_byte_count: 4,
            }),
        }),
        scalar_stack: None,
        offset: 18,
    });
    drop.internal_unit_calls.push(InternalUnitCallRecord {
        source: machine_code::InternalUnitCallSource::Authored,
        owner: CallSiteOwner::Operation(second_operation),
        target: second_helper,
        result: None,
        semantic_result: None,
        structural_result: None,
        scalar_arguments: Vec::new(),
        arguments: Vec::new(),
        claim_transfers: Vec::new(),
        operation_ordinal: 1,
        code_offset: 13,
        byte_count: 13,
    });
    drop.semantic_code_attribution.insert(
        1,
        SemanticCodeAttribution {
            site: SemanticCodeSite::Operation(second_operation),
            operation_ordinal: 1,
            code_offset: 13,
            byte_count: 13,
        },
    );
    drop.semantic_code_attribution[2].operation_ordinal = 2;
    drop.semantic_code_attribution[2].code_offset = 26;
    let drop_return = drop.unit_affine_cleanup.as_mut().expect("drop return");
    drop_return.code_offset = 26;

    let mut helper = plan.functions[1].clone();
    helper.machine = second_helper;
    helper.attachment = Some(StructuralTypeId::new(2).expect("second helper type"));
    helper.provenance.edges = vec![second_edge];
    let helper_return = helper.unit_affine_cleanup.as_mut().expect("helper return");
    helper_return.psi_edge = second_edge;
    helper.semantic_code_attribution[0].site = SemanticCodeSite::Edge(second_edge);
    plan.functions.push(helper);
    plan
}

pub(super) fn add_empty_unit_cleanup(function: &mut MachineCodeFunction) {
    let byte_count = if function.bytes.ends_with(&0xd65f_03c0_u32.to_le_bytes()) {
        4
    } else {
        1
    };
    let code_offset = function.bytes.len() - byte_count;
    function.unit_affine_cleanup = Some(UnitAffineCleanupRecord {
        structural_types: Vec::new().into(),
        psi_edge: function.provenance.edges[0],
        locals: Vec::new(),
        actions: Vec::new(),
        code_offset,
        byte_count,
    });
    if function.semantic_code_attribution.is_empty() {
        function
            .semantic_code_attribution
            .push(SemanticCodeAttribution {
                site: SemanticCodeSite::Edge(function.provenance.edges[0]),
                operation_ordinal: 0,
                code_offset,
                byte_count,
            });
    }
}
