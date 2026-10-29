//! The selected normalized-foreign-call record both instruction sets' template
//! tests admit, the case those tests derive from it, and the callback roster
//! they wire into it.
//!
//! The record carries physical call inputs only; source ownership and
//! declarations stay the template tests' independent responsibility. Only a
//! target's own facts differ between instruction sets — which target, how many
//! parameters its integer registers take before the first floating one, the
//! aggregate its fourth result shape names, and the locator family its import is
//! spelled in — so those are arguments and the construction is stated once.
//!
//! The encoded effects a roster row charges, the callback destination
//! vocabulary naming where a materialized address lands, and the byte counts
//! that vocabulary's materialization groups charge stay with the instruction
//! set; [`case`] and [`callback_fixture`] take them as arguments so the
//! scaffolding around them is stated once.

use calling_conventions::{
    CallSignature, CallbackBinderRequirement, CallbackMaterialization,
    CallbackMaterializationContext, CallbackRequirementId, CallingPolicy, LayoutPlanId,
    LayoutSlotId, MachineRegister, NativeCallbackDemand, NativeParameterApplication,
    NativeParameterId, NativePlace, ValueLocation, ValueShape,
};
use function_identity::MachineFunctionIdentity;
use register_model::{
    RegisterInstructionConstraint, RegisterOperandAccess, RegisterViewId,
    ValidatedPhysicalRegisterModel,
};
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects, SelectedInstructionId,
    SelectedInstructionKind, SelectedNormalizedForeignCall,
};
use semantic_vocabulary::{
    BoundaryMachineId, IeeeFloatFormat, IntegerSign, IntegerType, OperationId, PlaceId, ScalarType,
    StructuralTypeId, ValueId,
};
use target::{
    ForeignLocatorCandidate, NativeTarget, NormalizedForeignLocator, TargetProfile,
    normalize_foreign_locator,
};
use target_operations::{
    NormalizedForeignCallBinding, NormalizedForeignCallMechanism, ProviderExecutionBinding,
    ProviderPlanReportIdentity, TargetBoundaryResult, TargetNativeCallbackArgument,
    TargetStructuralHomeLayout, TargetStructuralHomeOrigin, TargetStructuralHomeRequirement,
    TargetUnitScalarArgumentSource, TargetUnitScalarCallArgument, TargetUnitScalarHomeRequirement,
};

/// The PE import the Windows fixture names.
pub fn pe_locator() -> NormalizedForeignLocator {
    normalize_foreign_locator(
        ForeignLocatorCandidate::PeByName {
            library: b"KERNEL32.dll".to_vec(),
            export: b"Leaf".to_vec(),
        },
        TargetProfile::WindowsX64,
    )
    .unwrap()
}

/// The symbol-versioned ELF import the Linux fixture names.
pub fn elf_locator(profile: TargetProfile) -> NormalizedForeignLocator {
    normalize_foreign_locator(
        ForeignLocatorCandidate::ElfVersioned {
            object: b"libc.so.6".to_vec(),
            symbol: b"leaf".to_vec(),
            version: b"GLIBC_2.0".to_vec(),
        },
        profile,
    )
    .unwrap()
}

/// The Mach-O dylib import the Darwin fixture names.
pub fn mach_o_locator() -> NormalizedForeignLocator {
    normalize_foreign_locator(
        ForeignLocatorCandidate::MachODylibSymbol {
            install_name: b"/usr/lib/libSystem.B.dylib".to_vec(),
            symbol: b"_leaf".to_vec(),
        },
        TargetProfile::MacosArm64,
    )
    .unwrap()
}

/// One selected normalized-foreign-call record for `target`, carrying
/// `input_layout` parameters and `result`'s shaped result. `integer_registers`
/// is the target's integer-argument register count and `aggregate` the shape
/// its fourth result names.
pub fn record(
    target: NativeTarget,
    integer_registers: usize,
    aggregate: ValueShape,
    locator: NormalizedForeignLocator,
    input_layout: usize,
    result: usize,
) -> SelectedNormalizedForeignCall {
    let (general, floating) = if input_layout < integer_registers {
        (input_layout, 0)
    } else {
        (
            (input_layout - integer_registers) / 8,
            (input_layout - integer_registers) % 8 + 1,
        )
    };
    let parameters = std::iter::repeat_n(ValueShape::integer(8, 8), general)
        .chain(std::iter::repeat_n(ValueShape::float(8), floating))
        .collect::<Vec<_>>();
    let result_shape = match result {
        0 => None,
        1 => Some(ValueShape::integer(8, 8)),
        2 => Some(ValueShape::float(8)),
        3 => Some(ValueShape::integer(16, 8)),
        4 => Some(aggregate),
        5 => Some(ValueShape::integer(24, 8)),
        _ => panic!("test result"),
    };
    let boundary_entry_plan = calling_conventions::evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters,
            result: result_shape,
        },
    )
    .unwrap()
    .plan()
    .clone();
    let commitment = task_plans::SameStackProviderPlanCommitment::from_digest([0x42; 32]);
    let identities =
        terminal_psi::TerminalIdentityTable::from(vec![String::new(), "Foreign::leaf".to_owned()]);
    let requirement_index = identities
        .index_of("Foreign::leaf")
        .expect("test requirement identity must intern");
    let same_stack_contribution = task_plans::admit_same_stack_contribution(
        task_plans::SameStackContributionAdmissionCandidate {
            provider_plan_report_identity: 0xA1,
            provider_plan_commitment: commitment,
            requirement_identity: requirement_index,
            receipt: task_plans::SameStackContributionAdmissionReceiptId::from_normalized_identity(
                0xA2,
            )
            .unwrap(),
            bytes: 64,
            alignment: 16,
        },
        0xA1,
        commitment,
        requirement_index,
    )
    .unwrap();
    let operation = OperationId::new(7).unwrap();
    let scalar_type = |floating| {
        if floating {
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary64)
        } else {
            ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap())
        }
    };
    let scalar_arguments = boundary_entry_plan
        .call
        .parameters
        .iter()
        .enumerate()
        .map(|(position, placement)| TargetUnitScalarCallArgument {
            parameter_index: position as u32,
            source: TargetUnitScalarArgumentSource::Parameter {
                parameter_index: position as u32,
                source_value: ValueId::new(100 + position as u64).unwrap(),
                scalar_type: scalar_type(position >= general),
            },
            placement: placement.clone(),
        })
        .collect();
    let completion = match result_shape {
        None => TargetBoundaryResult::Unit,
        Some(shape) if result <= 2 => {
            TargetBoundaryResult::Scalar(TargetUnitScalarHomeRequirement {
                defining_operation: operation,
                source_value: ValueId::new(8).unwrap(),
                scalar_type: scalar_type(result == 2),
                shape,
            })
        }
        Some(shape) => TargetBoundaryResult::Structural(TargetStructuralHomeRequirement {
            origin: TargetStructuralHomeOrigin::OperationResult {
                operation,
                result: terminal_psi::StructuralOperationResult {
                    place: PlaceId::new(9).unwrap(),
                    structural_type: StructuralTypeId::new(2).unwrap(),
                    multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    qualification_establishments: Vec::new(),
                    claims: Vec::new(),
                },
            },
            layout: TargetStructuralHomeLayout::Aggregate(ValueShape::integer(
                shape.byte_size,
                shape.alignment,
            )),
        }),
    };
    SelectedNormalizedForeignCall {
        instruction: SelectedInstructionId(4),
        invocation_site: operation.into(),
        call: legalized_operations::LegalizedNormalizedForeignCall {
            boundary: BoundaryMachineId::new(3).unwrap(),
            provider_execution: ProviderExecutionBinding::from_execution_record(
                ProviderPlanReportIdentity::new(0xA1).unwrap(),
                1,
                2,
                3,
                4,
            )
            .unwrap(),
            binding: NormalizedForeignCallBinding {
                boundary_entry_plan,
                mechanism: NormalizedForeignCallMechanism::Import {
                    locator,
                    same_stack_contribution,
                },
            },
            scalar_arguments,
            structural_arguments: Vec::new(),
            structural_indices: Vec::new(),
            result: completion,
            callbacks: Vec::new(),
        },
        effect: optimization_unit::EffectLink {
            input: 1,
            output: 2,
        },
        ownership: Vec::new(),
    }
}

/// Everything a template test hands its encoder, in the order both instruction
/// sets' test modules already destructure: the validated physical model, the
/// selected kind and alternative, the roster row's operand columns, the effects
/// that row's call declaration charges, and the record they were derived from.
pub type Case = (
    ValidatedPhysicalRegisterModel,
    SelectedInstructionKind,
    MachineAlternativeKey,
    Vec<RegisterViewId>,
    Vec<RegisterOperandAccess>,
    MachineEncodedEffects,
    SelectedNormalizedForeignCall,
);

/// Derive one encoder-test case from `record`.
///
/// The kind, the alternative, the roster row the record evaluates to, that
/// row's operand columns, and the record itself are target-independent; only
/// the row lookup (`constraint`) and the encoded effects of the row under
/// `physical` (`effects`) belong to the instruction set.
pub fn case(
    physical: ValidatedPhysicalRegisterModel,
    record: SelectedNormalizedForeignCall,
    constraint: impl Fn(&SelectedNormalizedForeignCall) -> Option<RegisterInstructionConstraint>,
    effects: impl Fn(
        &RegisterInstructionConstraint,
        &ValidatedPhysicalRegisterModel,
    ) -> MachineEncodedEffects,
) -> Case {
    let kind = SelectedInstructionKind::NormalizedForeignCall {
        boundary: BoundaryMachineId::new(3).unwrap(),
        ordinal: 2,
    };
    let alternative = MachineAlternativeKey {
        family: MachineAlternativeFamily::NormalizedForeignCall,
        variant: 0,
    };
    let row = constraint(&record).expect("fixture record must evaluate its roster row");
    let (operands, accesses) = operand_columns(&row);
    let effects = effects(&row, &physical);
    (
        physical,
        kind,
        alternative,
        operands,
        accesses,
        effects,
        record,
    )
}

/// The roster row's operand view and access columns, in roster order.
pub fn operand_columns(
    row: &RegisterInstructionConstraint,
) -> (Vec<RegisterViewId>, Vec<RegisterOperandAccess>) {
    let views = row
        .operands
        .iter()
        .map(|operand| {
            operand
                .fixed_view
                .expect("fixture row pins every operand view")
        })
        .collect();
    let accesses = row.operands.iter().map(|operand| operand.access).collect();
    (views, accesses)
}

/// The materialization byte counts one instruction set charges for its two
/// callback destination forms: a register and a record field.
#[derive(Debug, Clone, Copy)]
pub struct CallbackByteCounts {
    pub register: u16,
    pub field: u16,
}

/// One wired private callback destination, named the way every instruction set
/// can read it: the register its address materializes in and, for a record
/// field, the byte offset of the private slot inside that record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallbackPlacement {
    pub register: MachineRegister,
    pub field_byte_offset: Option<usize>,
}

/// The wired callback fixture: the record that carries the callbacks, each
/// destination in the caller's own vocabulary, and the materialization byte
/// count the caller's table charges for it — all in roster order.
pub struct CallbackFixture<Destination> {
    pub record: SelectedNormalizedForeignCall,
    pub destinations: Vec<Destination>,
    pub byte_counts: Vec<u16>,
}

/// Wire the four-callback fixture into `record`: a private parameter slot, a
/// record field, another parameter slot, and another field of the same root
/// record, in that roster order, over the record's own evaluated
/// boundary-entry plan.
///
/// `into_destination` names each placement in the caller's vocabulary — the
/// fixture never lands an address in a memory destination, because its callback
/// roots are register resident — and `counts` is the caller's own per-form
/// materialization extent table. The two private parameter slots are
/// materialized inside the call, so the record keeps only the field root as an
/// explicit input; source custody for that root is checked upstream.
pub fn callback_fixture<Destination>(
    mut record: SelectedNormalizedForeignCall,
    into_destination: impl Fn(CallbackPlacement) -> Destination,
    counts: CallbackByteCounts,
) -> CallbackFixture<Destination> {
    let root = NativeParameterId::new(81).unwrap();
    let places = [
        NativePlace::Parameter(NativeParameterId::new(82).unwrap()),
        NativePlace::Field {
            parameter: root,
            layout: LayoutPlanId::new(83).unwrap(),
            field_path: vec![LayoutSlotId::new(1).unwrap()],
        },
        NativePlace::Parameter(NativeParameterId::new(84).unwrap()),
        NativePlace::Field {
            parameter: root,
            layout: LayoutPlanId::new(83).unwrap(),
            field_path: vec![LayoutSlotId::new(2).unwrap()],
        },
    ];
    let requirement = CallbackRequirementId::new(85).unwrap();
    let context = CallbackMaterializationContext {
        binders: (1..=4)
            .map(|identity| CallbackBinderRequirement {
                binder: calling_conventions::StaticMachineBinderId::new(identity).unwrap(),
                requirement,
            })
            .collect(),
        demands: places
            .iter()
            .cloned()
            .map(|destination| NativeCallbackDemand {
                destination,
                requirement,
            })
            .collect(),
    };
    let plan = &mut record.call.binding.boundary_entry_plan;
    plan.call.callback_materializations = places
        .iter()
        .cloned()
        .enumerate()
        .map(|(position, destination)| CallbackMaterialization {
            binder: context.binders[position].binder,
            destination,
        })
        .collect();
    let continuation = terminal_psi::TerminalPsiIdentity {
        vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
        program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([1; 32]),
    };
    let mut destinations = Vec::with_capacity(places.len());
    let mut byte_counts = Vec::with_capacity(places.len());
    for (placement_index, place) in places.into_iter().enumerate() {
        let (parameter, native_ordinal, field_byte_offset) = match place {
            NativePlace::Parameter(parameter) => (parameter, placement_index / 2 + 1, None),
            NativePlace::Field { parameter, .. } => (parameter, 0, Some((placement_index / 2) * 8)),
        };
        let placement = plan.call.parameters[native_ordinal].clone();
        let [ValueLocation::Register { register, .. }] = placement.locations.as_slice() else {
            panic!("fixture callback root must be register resident")
        };
        destinations.push(into_destination(CallbackPlacement {
            register: *register,
            field_byte_offset,
        }));
        byte_counts.push(if field_byte_offset.is_some() {
            counts.field
        } else {
            counts.register
        });
        record.call.callbacks.push(TargetNativeCallbackArgument {
            registrar_invokes_callback: true,
            terminal_operation: record
                .invocation_site
                .operation()
                .expect("callback is owned by a returning operation"),
            placement_index,
            callback_function: MachineFunctionIdentity::callback_thunk(
                continuation,
                placement_index,
            )
            .unwrap(),
            destination: place,
            field_byte_offset,
            application: NativeParameterApplication {
                parameter,
                native_ordinal: native_ordinal as u32,
                shape: placement.shape,
                placement,
            },
            registrar_boundary_entry_plan: plan.clone(),
            registrar_context: context.clone(),
            registrar_application_commitment: [0x66; 32],
        });
    }
    record.call.scalar_arguments.truncate(1);
    CallbackFixture {
        record,
        destinations,
        byte_counts,
    }
}
