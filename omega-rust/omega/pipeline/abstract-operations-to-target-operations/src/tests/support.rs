//! Shared scalar fixture projections used across lowering test families.

use super::*;

/// Exercise retained expression-family validators without changing production routing.
pub(super) fn lower_legacy_scalar_fixture(
    source: &AbstractOperationPlan,
    target: NativeTarget,
) -> Result<target_operations::TargetOperationPlan, LoweringError> {
    assert!(source.boundary_machines.is_empty());
    let functions = source
        .functions
        .iter()
        .map(|function| (function.machine, function))
        .collect();
    let types = crate::lowering::StructuralTypeLookupForTests::new(&source.structural_types);
    let functions = source
        .functions
        .iter()
        .map(|function| {
            crate::lowering::lower_scalar_function_for_tests(
                function,
                function
                    .result
                    .scalar()
                    .expect("legacy scalar validator fixture"),
                target,
                &functions,
                &types,
                &BTreeMap::new(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(target_operations::TargetOperationPlan {
        psi: source.psi,
        entry: source.entry,
        target,
        functions,
    })
}

pub(crate) fn identity() -> TerminalPsiIdentity {
    TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::CURRENT,
        program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
    }
}

pub(crate) fn scalar_result(function: &AbstractFunction) -> AbstractResult {
    function.result.scalar().expect("fixture is scalar")
}

pub(crate) fn scalar_result_mut(function: &mut AbstractFunction) -> &mut AbstractResult {
    let AbstractFunctionResult::Scalar(result) = &mut function.result else {
        panic!("fixture is scalar")
    };
    result
}
