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
