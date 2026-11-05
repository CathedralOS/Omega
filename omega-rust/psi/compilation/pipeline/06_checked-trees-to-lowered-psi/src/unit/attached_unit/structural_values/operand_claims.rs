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
    fn from_completed(
        argument: terminal_psi::StructuralArgument,
        claims: Option<Vec<terminal_psi::StructuralResultClaimBinding>>,
    ) -> Result<Self, LoweringError> {
        Self::checked(
            argument,
            claims.ok_or(LoweringError::Unsupported(
                "completed operand has no published custody",
            ))?,
        )
    }

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
    /// An already completed binding carries its current claim names, including
    /// a known empty roster. It needs no second source-expression consumption.
    pub(super) fn completed_operand(
        &self,
        argument: terminal_psi::StructuralArgument,
    ) -> Result<CompletedOperand, LoweringError> {
        let claims = self.evaluation.current_claims.source(&argument)?;
        CompletedOperand::from_completed(argument, claims)
    }

    pub(super) fn capture_operand(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        argument: terminal_psi::StructuralArgument,
        expression: typed_trees::expression::ExpressionHandle,
        first_new_place: u64,
    ) -> Result<CompletedOperand, LoweringError> {
        let argument =
            self.refresh_memberships(identities, argument, expression, first_new_place)?;
        self.capture_operand_claims(identities, argument, expression, first_new_place)
    }

    pub(super) fn capture_operand_claims(
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
        if self.multiplicity != StructuralMultiplicity::Unrestricted
            && argument.place.get() >= first_new_place
        {
            return CompletedOperand::from_completed(argument, completed_claims);
        }
        let claims = if self.multiplicity == StructuralMultiplicity::Unrestricted {
            Vec::new()
        } else {
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
        };
        CompletedOperand::checked(argument, claims)
    }
}
