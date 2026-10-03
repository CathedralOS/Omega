//! Target-catalog admission of selected float operations against the exact
//! Terminal boundary signature of each operation and result format.
use super::{NearestIeeeFloatArithmeticRealization, float_arithmetic_realization};
use semantic_vocabulary::{
    BoundaryMachineId, IeeeFloatFormat, IntegerSign, IntegerType, ScalarType,
};
use target_operations::IeeeFloatArithmeticOperation;
use terminal_psi::{BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind};

fn boundary(
    parameters: Vec<ScalarType>,
    result: BoundaryMachineResult,
) -> BoundaryMachineDeclaration {
    BoundaryMachineDeclaration {
        unconditional_termination: false,
        completion: terminal_psi::BoundaryCompletion::NormalReturn,
        kind: terminal_psi::BoundaryRequirementKind::Trait,
        scalar_ensures: Vec::new(),
        scalar_requires: Vec::new(),
        fixed_service_reach: Vec::new(),
        id: BoundaryMachineId::new(1).unwrap(),
        identity: "Float::add(f64,f64)->f64".into(),
        attachment: None,
        parameter_order: parameters
            .iter()
            .map(|_| BoundaryParameterKind::Scalar)
            .collect(),
        scalar_parameters: parameters,
        structural_parameters: Vec::new(),
        result,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
        crash_routes: Vec::new(),
    }
}

fn forms() -> impl Iterator<Item = NearestIeeeFloatArithmeticRealization> {
    IeeeFloatArithmeticOperation::ALL
        .into_iter()
        .flat_map(|operation| {
            [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64]
                .map(|format| NearestIeeeFloatArithmeticRealization { operation, format })
        })
}

/// The exact boundary signature of one realization: its operation's operand
/// count and operand type, and one result of its format.
fn exact_signature(realization: NearestIeeeFloatArithmeticRealization) -> Vec<ScalarType> {
    vec![
        realization.operation.operand_type(realization.format);
        realization.operation.operand_count()
    ]
}

#[test]
fn float_format_proposals_keep_boolean_results_and_conversion_direction() {
    use IeeeFloatFormat::{Binary32, Binary64};
    for (input, output, expected) in [
        (Binary32, ScalarType::Boolean, Binary32),
        (Binary64, ScalarType::Boolean, Binary64),
        (Binary32, ScalarType::IeeeFloat(Binary64), Binary64),
        (Binary64, ScalarType::IeeeFloat(Binary32), Binary32),
    ] {
        let declaration = boundary(
            vec![ScalarType::IeeeFloat(input)],
            BoundaryMachineResult::Scalar(output),
        );
        let identity = declaration.identity.clone();
        assert_eq!(
            super::boundary_float_formats(&[declaration]).get(&identity),
            Some(&expected)
        );
    }
}

#[test]
fn every_form_realizes_only_on_its_supported_targets_for_its_exact_signature() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ] {
        for realization in forms() {
            let scalar = realization.operation.result_type(realization.format);
            let exact = boundary(
                exact_signature(realization),
                BoundaryMachineResult::Scalar(scalar),
            );
            if realization.operation == IeeeFloatArithmeticOperation::FusedMultiplyAdd
                && target.architecture == target::Architecture::X86_64
            {
                assert!(float_arithmetic_realization(realization, target, &exact).is_err());
            } else {
                assert_eq!(
                    float_arithmetic_realization(realization, target, &exact),
                    Ok(realization)
                );
            }
        }
    }
    assert_eq!(forms().count(), 114);
}

#[test]
fn a_substituted_format_or_shape_cannot_realize_the_boundary() {
    let target = target::NativeTarget::macos_arm64();
    let binary32 = ScalarType::IeeeFloat(IeeeFloatFormat::Binary32);
    let binary64 = ScalarType::IeeeFloat(IeeeFloatFormat::Binary64);
    let signed64 = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap());
    let unsigned32 = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap());
    let signed16 = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 16).unwrap());
    for realization in forms() {
        let own = realization.operation.result_type(realization.format);
        let other = match realization.format {
            IeeeFloatFormat::Binary32 => binary64,
            IeeeFloatFormat::Binary64 => binary32,
        };
        let exact = exact_signature(realization);
        let operand = exact[0];
        let mut substituted = vec![
            // A result of the other format, or no scalar result.
            boundary(exact.clone(), BoundaryMachineResult::Scalar(other)),
            boundary(exact.clone(), BoundaryMachineResult::Unit),
            // One operand fewer or more.
            boundary(exact[1..].to_vec(), BoundaryMachineResult::Scalar(own)),
            boundary(
                [exact.as_slice(), &[operand]].concat(),
                BoundaryMachineResult::Scalar(own),
            ),
        ];
        // Every operand replaced by another float format or integer carrier.
        for replacement in [binary32, binary64, signed64, unsigned32, signed16] {
            if replacement != operand {
                substituted.push(boundary(
                    vec![replacement; exact.len()],
                    BoundaryMachineResult::Scalar(own),
                ));
            }
        }
        // Substitute each position independently, including a ternary addend.
        for position in 0..exact.len() {
            let mut mixed = exact.clone();
            mixed[position] = if operand == binary32 {
                binary64
            } else {
                binary32
            };
            substituted.push(boundary(mixed, BoundaryMachineResult::Scalar(own)));
        }
        for substituted in substituted {
            assert_eq!(
                float_arithmetic_realization(realization, target, &substituted),
                Err("does not match its Terminal boundary signature".to_owned()),
                "{realization:?} {substituted:?}"
            );
        }
    }
}

#[test]
fn boundary_completion_requires_exact_schema_and_closed_execution_agreement() {
    use semantic_vocabulary::{BoundaryCompletion, ExternalCompletionKind};
    use target_operations::CompilerBuiltinExecution;
    let mut declaration = boundary(Vec::new(), BoundaryMachineResult::Unit);
    let mut plan = effects::provider_plan::ProviderPlan::default();
    plan.schema
        .methods
        .push(effects::provider_plan::ServiceMethod {
            requirement_identity: declaration.identity.clone(),
            completion: BoundaryCompletion::NormalReturn,
            ..Default::default()
        });
    let matches = |declaration: &BoundaryMachineDeclaration,
                   plan: &effects::provider_plan::ProviderPlan| {
        super::boundary_contract_matches_selected_schema(declaration, plan, &declaration.identity)
    };
    assert!(matches(&declaration, &plan));
    declaration.unconditional_termination = true;
    assert!(
        !matches(&declaration, &plan),
        "NoGuarantee cannot authenticate finite cleanup"
    );
    plan.schema.methods[0].terminates_guarantee = true;
    assert!(matches(&declaration, &plan));
    plan.schema.methods[0].termination_premises.push(
        effects::provider_plan::ServiceProgressPremise {
            profile: "WeakFair".into(),
            subject: effects::provider_plan::ServiceProgressSubject::ProviderReceiver,
            subject_projections: Vec::new(),
            establishment_routes: Vec::new(),
        },
    );
    assert!(
        !matches(&declaration, &plan),
        "an undisclosed progress subject cannot become an unconditional guarantee"
    );
    declaration.unconditional_termination = false;
    assert!(matches(&declaration, &plan));
    plan.schema.methods[0].termination_premises.clear();
    assert!(
        !matches(&declaration, &plan),
        "the retained projection must stay exact"
    );
    plan.schema.methods[0].terminates_guarantee = false;
    assert_ne!(
        declaration.completion,
        super::compiler_builtin_completion(CompilerBuiltinExecution::HostedExitProcessI32)
    );
    declaration.completion = BoundaryCompletion::External(ExternalCompletionKind::ProcessExit);
    assert!(!matches(&declaration, &plan));
    plan.schema.methods[0].completion = declaration.completion;
    assert!(matches(&declaration, &plan));
    assert_eq!(
        declaration.completion,
        super::compiler_builtin_completion(CompilerBuiltinExecution::HostedExitProcessI32)
    );
    assert_ne!(
        declaration.completion,
        super::compiler_builtin_completion(CompilerBuiltinExecution::HostedWriteByteI32)
    );
    plan.schema.methods[0]
        .requirement_identity
        .push_str("::other");
    assert!(!matches(&declaration, &plan));
}
