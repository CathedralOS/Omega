use super::{BoundaryNominalIdentity, TerminalBoundaryApplicationDemands};
use semantic_vocabulary::OperationId;

/// Closed semantic role selected for one exact D29 application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryApplicationRealizationRole {
    NongenericCheckedBody,
    SpecializedCheckedBody,
    ExactCompilerIntrinsic,
}

/// Role-specific semantic realization retained beside a Terminal demand.
///
/// The variants deliberately do not share optional fields. Constructing this
/// data grants no execution or external admission authority; the compiler
/// product owner derives and validates each published row from retained
/// selected-plan and checked/admitted custody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryApplicationRealization {
    NongenericCheckedBody {
        realization_machine: BoundaryNominalIdentity,
        realization_state: BoundaryNominalIdentity,
        realization_contract_commitment: [u8; 32],
    },
    SpecializedCheckedBody {
        realization_template: BoundaryNominalIdentity,
        realization_machine: BoundaryNominalIdentity,
        realization_state: BoundaryNominalIdentity,
        specialization_commitment: [u8; 32],
        realization_contract_commitment: [u8; 32],
    },
    ExactCompilerIntrinsic {
        execution: effects::CompilerIntrinsicExecutionIdentity,
    },
}

impl BoundaryApplicationRealization {
    pub const fn role(&self) -> BoundaryApplicationRealizationRole {
        match self {
            Self::NongenericCheckedBody { .. } => {
                BoundaryApplicationRealizationRole::NongenericCheckedBody
            }
            Self::SpecializedCheckedBody { .. } => {
                BoundaryApplicationRealizationRole::SpecializedCheckedBody
            }
            Self::ExactCompilerIntrinsic { .. } => {
                BoundaryApplicationRealizationRole::ExactCompilerIntrinsic
            }
        }
    }

    fn has_strong_commitments(&self) -> bool {
        match self {
            Self::NongenericCheckedBody {
                realization_contract_commitment,
                ..
            } => *realization_contract_commitment != [0; 32],
            Self::SpecializedCheckedBody {
                specialization_commitment,
                realization_contract_commitment,
                ..
            } => {
                *specialization_commitment != [0; 32] && *realization_contract_commitment != [0; 32]
            }
            Self::ExactCompilerIntrinsic { .. } => true,
        }
    }
}

/// Exact selected-plan and role-specific realization companion for one
/// source-free Terminal demand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryApplicationRealizationCompanion {
    terminal_operation: OperationId,
    selected_plan_digest: [u8; 32],
    realization: BoundaryApplicationRealization,
}

impl BoundaryApplicationRealizationCompanion {
    pub fn new(
        terminal_operation: OperationId,
        selected_plan_digest: [u8; 32],
        realization: BoundaryApplicationRealization,
    ) -> Result<Self, &'static str> {
        if selected_plan_digest == [0; 32] {
            return Err("boundary application realization has an empty selected-plan digest");
        }
        if !realization.has_strong_commitments() {
            return Err("boundary application realization has an empty role commitment");
        }
        Ok(Self {
            terminal_operation,
            selected_plan_digest,
            realization,
        })
    }

    pub const fn terminal_operation(&self) -> OperationId {
        self.terminal_operation
    }

    pub const fn selected_plan_digest(&self) -> &[u8; 32] {
        &self.selected_plan_digest
    }

    pub const fn role(&self) -> BoundaryApplicationRealizationRole {
        self.realization.role()
    }

    pub const fn realization(&self) -> &BoundaryApplicationRealization {
        &self.realization
    }
}

/// Complete role-specific D29 companion for one Terminal semantic product.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalBoundaryApplicationRealizations {
    terminal_psi: terminal_psi::TerminalPsiIdentity,
    rows: Vec<BoundaryApplicationRealizationCompanion>,
}

impl TerminalBoundaryApplicationRealizations {
    pub fn new(
        demands: &TerminalBoundaryApplicationDemands,
        mut rows: Vec<BoundaryApplicationRealizationCompanion>,
    ) -> Result<Self, &'static str> {
        rows.sort_by_key(|row| row.terminal_operation().get());
        if rows.len() != demands.rows().len()
            || !rows
                .iter()
                .zip(demands.rows())
                .all(|(row, demand)| row.terminal_operation() == demand.terminal_operation())
        {
            return Err(
                "boundary application realizations do not cover the exact Terminal demand set",
            );
        }
        Ok(Self {
            terminal_psi: demands.terminal_psi(),
            rows,
        })
    }

    pub fn validate_for_demands(
        &self,
        demands: &TerminalBoundaryApplicationDemands,
    ) -> Result<(), &'static str> {
        if self.terminal_psi != demands.terminal_psi()
            || self.rows.len() != demands.rows().len()
            || !self
                .rows
                .iter()
                .zip(demands.rows())
                .all(|(row, demand)| row.terminal_operation() == demand.terminal_operation())
        {
            return Err("boundary application realizations belong to different Terminal demands");
        }
        Ok(())
    }

    pub fn rows(&self) -> &[BoundaryApplicationRealizationCompanion] {
        &self.rows
    }
}

#[cfg(test)]
mod tests {
    use super::{BoundaryApplicationRealization, BoundaryApplicationRealizationCompanion};
    use crate::BoundaryNominalIdentity;
    use semantic_vocabulary::OperationId;

    #[test]
    fn checked_roles_require_strong_plan_and_contract_commitments() {
        let identity = || BoundaryNominalIdentity::new("nominal".to_owned()).unwrap();
        let operation = OperationId::new(1).unwrap();
        assert!(
            BoundaryApplicationRealizationCompanion::new(
                operation,
                [0; 32],
                BoundaryApplicationRealization::NongenericCheckedBody {
                    realization_machine: identity(),
                    realization_state: identity(),
                    realization_contract_commitment: [1; 32],
                },
            )
            .is_err()
        );
        assert!(
            BoundaryApplicationRealizationCompanion::new(
                operation,
                [1; 32],
                BoundaryApplicationRealization::SpecializedCheckedBody {
                    realization_template: identity(),
                    realization_machine: identity(),
                    realization_state: identity(),
                    specialization_commitment: [0; 32],
                    realization_contract_commitment: [1; 32],
                },
            )
            .is_err()
        );
    }
}
