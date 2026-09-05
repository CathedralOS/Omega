//! Admitted exit contract. Raw records and identity belong to machine-code data.

#[cfg(any(test, feature = "test-support"))]
use machine_code::{ResolvedSelectedFormLayoutIdentity, X86BranchRelaxationIdentity};
pub use machine_code::{
    WholeFunctionEntryAssumption, WholeFunctionExitContract, WholeFunctionExitContractIdentity,
    WholeFunctionExitEvidence, WholeFunctionExitLayoutCustody, WholeFunctionExitPolicy,
    WholeFunctionFrameDisposition, WholeFunctionHardeningPolicy, WholeFunctionReturnEvidence,
    WholeFunctionReturnMechanism, WholeFunctionReturnValueEvidence,
    WholeFunctionStructuralUnitCallEvidence, WholeFunctionStructuralUnitExitEvidence,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedWholeFunctionExitContract {
    pub(super) contract: std::sync::Arc<WholeFunctionExitContract>,
}

impl ValidatedWholeFunctionExitContract {
    pub fn contract(&self) -> &WholeFunctionExitContract {
        &self.contract
    }

    /// Share the original immutable record without granting admission.
    pub fn shared_contract(&self) -> std::sync::Arc<WholeFunctionExitContract> {
        std::sync::Arc::clone(&self.contract)
    }

    pub fn identity(&self) -> WholeFunctionExitContractIdentity {
        self.contract.identity
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn contract_mut(&mut self) -> &mut WholeFunctionExitContract {
        std::sync::Arc::make_mut(&mut self.contract)
    }

    /// Test-only rel8 custody mutation with a valid enclosing identity. This
    /// grants no production construction, validation, or publication authority.
    #[cfg(any(test, feature = "test-support"))]
    pub fn corrupt_rel8_boundary_and_reauthenticate_for_test(
        &mut self,
        boundary: Rel8ExitBoundaryForTest,
    ) {
        let contract = std::sync::Arc::make_mut(&mut self.contract);
        match boundary {
            Rel8ExitBoundaryForTest::LayoutCustody => {
                contract.layout_custody =
                    WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 {
                        relaxation: X86BranchRelaxationIdentity::from_bytes([0xb1; 32]),
                    };
            }
            Rel8ExitBoundaryForTest::ResolvedLayout => {
                contract.resolved_layout =
                    ResolvedSelectedFormLayoutIdentity::from_bytes([0xb2; 32]);
            }
        }
        contract.identity = super::identity::contract_identity(contract);
    }
}

#[cfg(any(test, feature = "test-support"))]
#[derive(Debug, Clone, Copy)]
pub enum Rel8ExitBoundaryForTest {
    ResolvedLayout,
    LayoutCustody,
}
