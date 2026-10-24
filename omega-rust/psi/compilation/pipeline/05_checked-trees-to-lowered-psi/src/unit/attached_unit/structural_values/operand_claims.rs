//! Capture a completed operand's custody before evaluating its later siblings.
//! Fresh producers supply their concrete result roster; an existing home uses
//! the source checker's exact consuming occurrence, never its birth declaration.
//! Constructor parents rebase these receipts without minting child claims.

use super::super::{LoweringError, StructuralMultiplicity, unsupported};
use super::emission;

pub(super) struct CompletedOperand {
    pub(super) argument: terminal_psi::StructuralArgument,
    pub(super) claims: Vec<terminal_psi::StructuralResultClaimBinding>,
}

impl CompletedOperand {
    fn checked(
        argument: terminal_psi::StructuralArgument,
        claims: Vec<terminal_psi::StructuralResultClaimBinding>,
    ) -> Result<Self, LoweringError> {
        if claims.iter().enumerate().any(|(index, claim)| {
            claims[..index]
                .iter()
                .any(|prior| prior.claim == claim.claim || prior.path == claim.path)
        }) {
            return unsupported("completed operand repeats a claim or path");
        }
        Ok(Self { argument, claims })
    }
}

impl emission::Emission<'_, '_, '_> {
    pub(super) fn capture_operand(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        argument: terminal_psi::StructuralArgument,
        expression: typed_trees::expression::ExpressionHandle,
        first_new_place: u64,
    ) -> Result<CompletedOperand, LoweringError> {
        let completed_claims = if self.multiplicity == StructuralMultiplicity::Unrestricted {
            None
        } else {
            self.evaluation.current_claims.source(&argument)?
        };
        let claims = if self.multiplicity == StructuralMultiplicity::Unrestricted {
            Vec::new()
        } else if argument.place.get() < first_new_place {
            crate::emission::claim_bindings::lower_owned_operand_claims(
                identities,
                self.checked,
                self.machine,
                self.state,
                self.statement,
                expression,
                None,
                None,
                self.calls.claims.bindings(),
                completed_claims.as_deref(),
                self.structural_type,
                self.structural_types,
            )?
        } else {
            completed_claims.ok_or(LoweringError::Unsupported(
                "fresh constructor operand has no published custody",
            ))?
        };
        CompletedOperand::checked(argument, claims)
    }

}
