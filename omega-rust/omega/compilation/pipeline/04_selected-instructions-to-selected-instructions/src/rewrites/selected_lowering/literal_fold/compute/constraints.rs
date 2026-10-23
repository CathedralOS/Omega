//! Producer selection of admitted immediate-form constraints.

use register_model::{
    RegisterConstraintKey, RegisterInstructionConstraint, RegisterOperandAccess,
    TargetRegisterEnvironmentConstraintKeys, ValidatedRegisterConstraintCatalog,
};
use selected_instructions::{
    MachineEffectDeclaration, SelectedInstructionKind, ValidatedMachineEffectCatalog,
};

use crate::rewrites::enabled_pair_rules;
use crate::rewrites::operand_probes::effect_declaration;
use crate::{
    LiteralFoldError, LiteralFoldPolicy, PairResultDisposition, SelectedInstructionPairRule,
};

/// One policy-enabled catalog row bound to its constraint-catalog row and to
/// the rewritten form's machine-effect declaration.
pub(super) struct AdmittedPair<'a> {
    pub(super) rule: SelectedInstructionPairRule,
    pub(super) row: &'a RegisterInstructionConstraint,
    /// The rewritten form's declaration in the bound effect catalog, already
    /// admitted against the pair's declared machine-effect surface.
    pub(super) declaration: &'a MachineEffectDeclaration,
}

pub(super) struct AdmittedPairs<'a> {
    pairs: Vec<AdmittedPair<'a>>,
    /// The bound catalog the per-candidate producer and consumer
    /// declarations resolve against.
    pub(super) catalog: &'a ValidatedMachineEffectCatalog,
}

impl<'a> AdmittedPairs<'a> {
    /// The enabled pair admitting `kind` as its consumer with the literal
    /// victim at operand `victim_operand` and the folded literal inside the
    /// pair's declared immediate bound. One family may declare disjoint
    /// operand grammars for the same consumer kind — the exact-add selection
    /// admits the literal at either `Use` position — so a kind match alone
    /// does not identify the grammar; and disjoint families may admit the
    /// same kind at the same position under disjoint literal bounds — the
    /// bitwise-and selections admit the zero literal (annihilator) and the
    /// all-ones literal (identity) — so the literal's value completes the
    /// key.
    pub(super) fn for_consumer(
        &self,
        kind: SelectedInstructionKind,
        victim_operand: u16,
        literal: u64,
    ) -> Option<&AdmittedPair<'a>> {
        let mut matches = self.pairs.iter().filter(|pair| {
            pair.rule.matches_consumer(kind)
                && pair.rule.victim_operand() == victim_operand
                && pair.rule.admits_immediate(literal)
        });
        let pair = matches.next()?;
        // The enabled rules must partition each consumer grammar on the
        // literal's value: a second admitting pair is a catalog defect, so
        // admission refuses rather than silently preferring catalog order.
        if matches.next().is_some() {
            return None;
        }
        Some(pair)
    }

    /// Any enabled pair admitting `kind` as its consumer with the literal
    /// victim at operand `victim_operand`, whatever literal bound the pair
    /// declares. Admission-time error reporting distinguishes an admitted
    /// operand position whose literal lies outside every enabled bound
    /// from a position no enabled grammar covers at all.
    pub(super) fn for_position(
        &self,
        kind: SelectedInstructionKind,
        victim_operand: u16,
    ) -> Option<&AdmittedPair<'a>> {
        self.pairs.iter().find(|pair| {
            pair.rule.matches_consumer(kind) && pair.rule.victim_operand() == victim_operand
        })
    }

    /// The enabled pair rewriting `kind` into the form `key` names. Apply
    /// time binds through the recorded constraint key rather than the folded
    /// operand position: every grammar a kind admits rewrites through the
    /// same bound row.
    pub(super) fn for_consumer_row(
        &self,
        kind: SelectedInstructionKind,
        key: RegisterConstraintKey,
    ) -> Option<&AdmittedPair<'a>> {
        self.pairs
            .iter()
            .find(|pair| pair.rule.matches_consumer(kind) && pair.row.key == key)
    }
}

pub(super) fn select_admitted_pairs<'a>(
    constraints: &'a ValidatedRegisterConstraintCatalog,
    keys: &TargetRegisterEnvironmentConstraintKeys,
    policy: LiteralFoldPolicy,
    catalog: &'a ValidatedMachineEffectCatalog,
) -> Result<AdmittedPairs<'a>, LiteralFoldError> {
    let find = |key| {
        register_model::find_register_constraint(&constraints.catalog().constraints, key)
            .ok_or(LiteralFoldError::ImmediateConstraintMismatch)
    };
    let mut pairs = Vec::new();
    for rule in enabled_pair_rules(policy) {
        let key = rule
            .immediate_constraint_key(keys)
            .ok_or(LiteralFoldError::ImmediateConstraintMismatch)?;
        let row = find(key)?;
        validate_immediate_row(rule, row)?;
        // The bound effect catalog must declare the rewritten form under the
        // row's constraint key, and the declaration must satisfy the pair's
        // declared machine-effect surface: no implicit unit uses or clobbers
        // beyond the declared result channel and no memory, trap, stack, or
        // control-flow traffic.
        let declaration = effect_declaration(catalog, rule.rewritten(), row.key)
            .ok_or(LiteralFoldError::EffectCatalogMismatch)?;
        if !rule.machine_effects().admits_rewritten(declaration) {
            return Err(LiteralFoldError::EffectCatalogMismatch);
        }
        pairs.push(AdmittedPair {
            rule,
            row,
            declaration,
        });
    }
    Ok(AdmittedPairs { pairs, catalog })
}

/// Admit the rewritten row the pair's declared axes describe: the row
/// binds the surviving `Use` operand under a surviving-`Use` grammar and
/// writes the scalar `Def` result under a scalar result channel, each at
/// its own canonical index in one register class, and its unit traffic
/// satisfies the rule's declared unit-effect surface — no implicit uses,
/// clobbers, or operand unit bindings beyond the declared channel.
fn validate_immediate_row(
    rule: SelectedInstructionPairRule,
    row: &RegisterInstructionConstraint,
) -> Result<(), LiteralFoldError> {
    let unit_effects = rule.unit_effects();
    let scalar_result = rule.result() == PairResultDisposition::ScalarRegister;
    let (binds_surviving, binds_result) = rule
        .rewritten_row_roles()
        .ok_or(LiteralFoldError::ImmediateConstraintMismatch)?;
    let survives = usize::from(binds_surviving);
    let operands = row.operands.as_slice();
    let well_formed = operands.len() == survives + usize::from(binds_result)
        // Each operand sits at its own canonical index; the leading operand
        // is the surviving `Use` channel and the trailing one the `Def`
        // result.
        && operands.iter().enumerate().all(|(index, operand)| {
            usize::from(operand.operand) == index
                && operand.access
                    == if index < survives {
                        RegisterOperandAccess::Use
                    } else {
                        RegisterOperandAccess::Def
                    }
                && unit_effects.admits_operand(operand)
        })
        // A scalar result channel declares no implicit definitions; the
        // unit result channel publishes them in place of a `Def` — the
        // target condition state a compare writes.
        && row.implicit_defs.is_empty() == scalar_result
        // A surviving `Use` and its `Def` result share one register class.
        && operands.windows(2).all(|pair| pair[0].class == pair[1].class)
        && unit_effects.admits_row_units(row);
    if !well_formed {
        return Err(LiteralFoldError::ImmediateConstraintMismatch);
    }
    Ok(())
}
