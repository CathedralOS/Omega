//! The selected normalized-foreign-call record both instruction sets' template
//! tests admit.
//!
//! The record carries physical call inputs only; source ownership and
//! declarations stay the template tests' independent responsibility. Only a
//! target's own facts differ between instruction sets — which target, how many
//! parameters its integer registers take before the first floating one, the
//! aggregate its fourth result shape names, and the locator family its import is
//! spelled in — so those are arguments and the construction is stated once.

use calling_conventions::{CallSignature, CallingPolicy, ValueShape};
use selected_instructions::{SelectedInstructionId, SelectedNormalizedForeignCall};
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
    ProviderPlanReportIdentity, TargetBoundaryResult, TargetStructuralHomeLayout,
    TargetStructuralHomeOrigin, TargetStructuralHomeRequirement, TargetUnitScalarArgumentSource,
    TargetUnitScalarCallArgument, TargetUnitScalarHomeRequirement,
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
