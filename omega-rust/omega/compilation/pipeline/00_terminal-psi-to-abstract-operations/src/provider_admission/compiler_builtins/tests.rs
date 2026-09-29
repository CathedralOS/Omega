//! Target-catalog admission of selected float arithmetic against the exact
//! Terminal boundary signature.
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

#[test]
fn every_form_realizes_on_every_native_target_for_its_exact_signature() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ] {
        for realization in forms() {
            let scalar = ScalarType::IeeeFloat(realization.format);
            let exact = boundary(vec![scalar, scalar], BoundaryMachineResult::Scalar(scalar));
            assert_eq!(
                float_arithmetic_realization(realization, target, &exact),
                Ok(realization)
            );
        }
    }
}

#[test]
fn a_substituted_format_or_shape_cannot_realize_the_boundary() {
    let target = target::NativeTarget::macos_arm64();
    let binary32 = ScalarType::IeeeFloat(IeeeFloatFormat::Binary32);
    let binary64 = ScalarType::IeeeFloat(IeeeFloatFormat::Binary64);
    let integer = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap());
    for realization in forms() {
        let (own, other) = match realization.format {
            IeeeFloatFormat::Binary32 => (binary32, binary64),
            IeeeFloatFormat::Binary64 => (binary64, binary32),
        };
        for substituted in [
            // A row selected for the other format.
            boundary(vec![other, other], BoundaryMachineResult::Scalar(other)),
            // Mixed operand or result formats.
            boundary(vec![own, other], BoundaryMachineResult::Scalar(own)),
            boundary(vec![own, own], BoundaryMachineResult::Scalar(other)),
            // Another operand count, an integer lane, or no scalar result.
            boundary(vec![own], BoundaryMachineResult::Scalar(own)),
            boundary(vec![own, own, own], BoundaryMachineResult::Scalar(own)),
            boundary(vec![own, integer], BoundaryMachineResult::Scalar(own)),
            boundary(vec![own, own], BoundaryMachineResult::Unit),
        ] {
            assert_eq!(
                float_arithmetic_realization(realization, target, &substituted),
                Err("does not match its Terminal boundary signature".to_owned()),
                "{realization:?} {substituted:?}"
            );
        }
    }
}
