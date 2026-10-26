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
        if argument.place.get() >= first_new_place {
            return self.published_operand(argument);
        }
        let claims = if self.multiplicity == StructuralMultiplicity::Unrestricted {
            Vec::new()
        } else {
            super::super::claims::lower_owned_operand_claims(
                identities,
                self.checked,
                self.machine,
                self.state,
                self.statement,
                expression,
                None,
                None,
                self.calls.claims.bindings(),
                self.structural_type,
                self.structural_types,
            )?
        };
        CompletedOperand::checked(argument, claims)
    }

    /// An actual producer or completed join publishes its current result
    /// roster. This entrance requires no source-expression reconstruction.
    pub(super) fn published_operand(
        &self,
        argument: terminal_psi::StructuralArgument,
    ) -> Result<CompletedOperand, LoweringError> {
        let claims = if self.multiplicity == StructuralMultiplicity::Unrestricted {
            Vec::new()
        } else {
            let claims = self
                .operations
                .structural_values
                .iter()
                .find(|(_, binding)| binding.place() == argument.place)
                .map(|(_, binding)| binding.claims())
                .or_else(|| {
                    self.operations
                        .operations
                        .iter()
                        .filter_map(|operation| operation.result.structural())
                        .find(|result| result.place == argument.place)
                        .map(|result| result.claims.as_slice())
                })
                .or_else(|| {
                    self.arrival_claims
                        .iter()
                        .find(|(place, _)| *place == argument.place)
                        .map(|(_, claims)| claims.as_slice())
                })
                .ok_or(LoweringError::Unsupported(
                    "fresh constructor operand has no published custody",
                ))?;
            let mut selected = Vec::new();
            for claim in claims {
                if let Some(path) = claim.path.strip_prefix(argument.path.as_slice()) {
                    selected.push(terminal_psi::StructuralResultClaimBinding {
                        claim: claim.claim,
                        path: path.to_vec(),
                    });
                }
            }
            selected
        };
        CompletedOperand::checked(argument, claims)
    }
}
