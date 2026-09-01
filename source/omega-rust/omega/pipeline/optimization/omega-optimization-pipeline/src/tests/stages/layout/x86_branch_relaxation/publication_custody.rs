//! Authenticated direct-receipt, rel8-manifest, and exit-custody corruption coverage.

use omega_optimization_core::OptimizationSelectionIdentity;

use crate::tests::*;

#[test]
fn every_direct_function_relative_receipt_field_rejects() {
    let donor = super::fixture::alternate_direct_realization();
    for field in [
        FunctionRelativeLayoutPublicationCustodyFieldForTest::Source,
        FunctionRelativeLayoutPublicationCustodyFieldForTest::Machine,
        FunctionRelativeLayoutPublicationCustodyFieldForTest::Relaxation,
        FunctionRelativeLayoutPublicationCustodyFieldForTest::ExitContract,
        FunctionRelativeLayoutPublicationCustodyFieldForTest::Realization,
    ] {
        let mut realization = super::fixture::direct_realization();
        realization.corrupt_publication_custody_for_test(field, &donor);
        assert_eq!(
            validate_function_relative_layout_optimization_realization_custody(&realization),
            Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch),
            "the public validator must reject the {field:?} receipt substitution",
        );
    }
}

#[test]
fn every_rel8_specific_manifest_boundary_rejects_after_reauthentication() {
    for field in ManifestField::ALL {
        let mut realization = super::fixture::direct_realization();
        let record = realization.manifest_mut().record_mut();
        field.corrupt(record);
        record.identity = record.recomputed_identity();
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(&record.encode()),
            Ok(record.clone()),
            "the {field:?} mutation must retain a valid manifest envelope",
        );
        assert_eq!(
            validate_function_relative_layout_optimization_realization_custody(&realization),
            Err(FunctionRelativeOptimizationRealizationError::RootMismatch),
            "independent replay must reject the authenticated {field:?} mutation",
        );
    }
}

#[test]
fn every_rel8_specific_exit_boundary_rejects_after_reauthentication() {
    for boundary in [
        Rel8ExitBoundaryForTest::ResolvedLayout,
        Rel8ExitBoundaryForTest::LayoutCustody,
    ] {
        let mut realization = super::fixture::direct_realization();
        realization
            .exit_contract_mut()
            .corrupt_rel8_boundary_and_reauthenticate_for_test(boundary);
        let exit_identity = realization.exit_contract().identity();
        let record = realization.manifest_mut().record_mut();
        record.whole_function_exit_contract = exit_identity;
        record.identity = record.recomputed_identity();
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(&record.encode()),
            Ok(record.clone()),
        );
        assert_eq!(
            validate_function_relative_layout_optimization_realization_custody(&realization),
            Err(FunctionRelativeOptimizationRealizationError::ExitContract(
                WholeFunctionExitContractError::ArtifactMismatch,
            )),
            "independent replay must reject the authenticated exit boundary mutation",
        );
    }
}

#[derive(Debug, Clone, Copy)]
enum ManifestField {
    PhaseSelections,
    BaselineLayout,
    ResolvedLayout,
    Relaxation,
    ExitContract,
}

impl ManifestField {
    const ALL: [Self; 5] = [
        Self::PhaseSelections,
        Self::BaselineLayout,
        Self::ResolvedLayout,
        Self::Relaxation,
        Self::ExitContract,
    ];

    fn corrupt(self, record: &mut FunctionRelativeOptimizationRealizationManifest) {
        match self {
            Self::PhaseSelections => {
                record.function_relative_layout_selections =
                    OptimizationSelectionIdentity::from_bytes([0xc1; 32]);
            }
            Self::BaselineLayout => {
                record.baseline_resolved_layout =
                    ResolvedSelectedFormLayoutIdentity::from_bytes([0xc2; 32]);
            }
            Self::ResolvedLayout => {
                record.resolved_layout = ResolvedSelectedFormLayoutIdentity::from_bytes([0xc3; 32]);
            }
            Self::Relaxation => {
                record.x86_branch_relaxation =
                    Some(X86BranchRelaxationIdentity::from_bytes([0xc4; 32]));
            }
            Self::ExitContract => {
                record.whole_function_exit_contract =
                    WholeFunctionExitContractIdentity::from_bytes([0xc5; 32]);
            }
        }
    }
}
