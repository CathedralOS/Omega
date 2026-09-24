//! Whole-function exit contract: evidence that every exit, call and
//! preserved-register effect of each function matches its target contract.
//!
//! Function realization stages the contract through
//! [`stage_whole_function_exit_contract_for_layout`] (`layout_optimization`):
//! it replays the layout phase, produces the contract for the layout that
//! phase selected (`compute`), and admits it only through the independent
//! record replay (`validation`). Producer and replay share the target catalogs
//! and effect predicates in `validation_rules`, not each other's algorithms.
//! `stage` holds the entrances fixed to one layout form: the baseline layout
//! with a frame, and the layout after x86 branch relaxation. `identity` names
//! the canonical contract encoder.

mod compute;
mod error;
mod identity;
mod layout_optimization;
mod stage;
mod validation;
mod validation_rules;

pub use error::WholeFunctionExitContractError;
pub(crate) use layout_optimization::validate_exit_record_for_replayed_layout;
pub use layout_optimization::{
    stage_whole_function_exit_contract_for_layout, validate_whole_function_exit_contract_for_layout,
};
#[cfg(any(test, feature = "test-support"))]
use machine_code::{ResolvedSelectedFormLayoutIdentity, X86BranchRelaxationIdentity};
pub use machine_code::{
    WholeFunctionEntryAssumption, WholeFunctionExitContract, WholeFunctionExitContractIdentity,
    WholeFunctionExitEvidence, WholeFunctionExitLayoutCustody, WholeFunctionExitPolicy,
    WholeFunctionFrameDisposition, WholeFunctionHardeningPolicy, WholeFunctionReturnEvidence,
    WholeFunctionReturnMechanism, WholeFunctionReturnValueEvidence,
};
pub use stage::{
    stage_whole_function_exit_contract_after_x86_branch_relaxation,
    stage_whole_function_exit_contract_with_frame,
    validate_whole_function_exit_contract_after_x86_branch_relaxation,
    validate_whole_function_exit_contract_with_frame,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedWholeFunctionExitContract {
    contract: std::sync::Arc<WholeFunctionExitContract>,
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
        contract.identity = self::identity::contract_identity(contract);
    }
}

#[cfg(any(test, feature = "test-support"))]
#[derive(Debug, Clone, Copy)]
pub enum Rel8ExitBoundaryForTest {
    ResolvedLayout,
    LayoutCustody,
}
