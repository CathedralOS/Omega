//! Producer selection of admitted immediate-form constraints.

use register_model::{
    RegisterInstructionConstraint, RegisterOperandAccess, TargetRegisterEnvironmentConstraintKeys,
    ValidatedRegisterConstraintCatalog,
};
use selected_instructions::SelectedInstructionKind;

use crate::{LiteralFoldError, LiteralFoldPolicy, SelectedInstructionPairRule, enabled_pair_rules};

/// One policy-enabled catalog row bound to its constraint-catalog row.
pub(super) struct AdmittedPair<'a> {
    pub(super) rule: SelectedInstructionPairRule,
    pub(super) row: &'a RegisterInstructionConstraint,
}

pub(super) struct AdmittedPairs<'a> {
    pairs: Vec<AdmittedPair<'a>>,
}

impl<'a> AdmittedPairs<'a> {
    pub(super) fn for_consumer(&self, kind: SelectedInstructionKind) -> Option<&AdmittedPair<'a>> {
        self.pairs
            .iter()
            .find(|pair| pair.rule.matches_consumer(kind))
    }
}

pub(super) fn select_admitted_pairs<'a>(
    constraints: &'a ValidatedRegisterConstraintCatalog,
    keys: &TargetRegisterEnvironmentConstraintKeys,
    policy: LiteralFoldPolicy,
) -> Result<AdmittedPairs<'a>, LiteralFoldError> {
    let find = |key| {
        constraints
            .catalog()
            .constraints
            .iter()
            .find(|row| row.key == key)
            .ok_or(LiteralFoldError::ImmediateConstraintMismatch)
    };
    let mut pairs = Vec::new();
    for rule in enabled_pair_rules(policy) {
        let key = rule
            .immediate_constraint_key(keys)
            .ok_or(LiteralFoldError::ImmediateConstraintMismatch)?;
        let row = find(key)?;
        validate_immediate_row(row)?;
        pairs.push(AdmittedPair { rule, row });
    }
    Ok(AdmittedPairs { pairs })
}

fn validate_immediate_row(row: &RegisterInstructionConstraint) -> Result<(), LiteralFoldError> {
    let clean = |operands: &[&register_model::RegisterOperandConstraint]| {
        operands.iter().all(|operand| {
            operand.fixed_view.is_none() && operand.tied_to.is_none() && !operand.early_clobber
        }) && row.implicit_uses.is_empty()
            && row.clobbers.is_empty()
    };
    match row.operands.as_slice() {
        // Scalar-result form: `result = left <op> immediate`.
        [left, result] => {
            if left.operand != 0
                || left.access != RegisterOperandAccess::Use
                || result.operand != 1
                || result.access != RegisterOperandAccess::Def
                || left.class != result.class
                || !clean(&[left, result])
                || !row.implicit_defs.is_empty()
            {
                return Err(LiteralFoldError::ImmediateConstraintMismatch);
            }
        }
        // Flag-defining form: `compare left, immediate` carries no `Def`
        // operand; its only implicit output is the target condition state.
        [left] => {
            if left.operand != 0
                || left.access != RegisterOperandAccess::Use
                || !clean(&[left])
                || row.implicit_defs.is_empty()
            {
                return Err(LiteralFoldError::ImmediateConstraintMismatch);
            }
        }
        _ => return Err(LiteralFoldError::ImmediateConstraintMismatch),
    }
    Ok(())
}
