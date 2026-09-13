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
    let [left, result] = row.operands.as_slice() else {
        return Err(LiteralFoldError::ImmediateConstraintMismatch);
    };
    if left.operand != 0
        || left.access != RegisterOperandAccess::Use
        || result.operand != 1
        || result.access != RegisterOperandAccess::Def
        || left.class != result.class
        || [left, result].iter().any(|operand| {
            operand.fixed_view.is_some() || operand.tied_to.is_some() || operand.early_clobber
        })
        || !row.implicit_uses.is_empty()
        || !row.implicit_defs.is_empty()
        || !row.clobbers.is_empty()
    {
        return Err(LiteralFoldError::ImmediateConstraintMismatch);
    }
    Ok(())
}
