//! Producer selection of admitted immediate-form constraints.

use register_model::{
    RegisterConstraintKey, RegisterInstructionConstraint, RegisterOperandAccess,
    TargetRegisterEnvironmentConstraintKeys, ValidatedRegisterConstraintCatalog,
};
use selected_instructions::{
    MachineEffectDeclaration, MachineSemanticKind, SelectedInstructionKind,
    ValidatedMachineEffectCatalog,
};

use crate::{
    LiteralFoldError, LiteralFoldPolicy, PairOperandShape, PairResultDisposition,
    SelectedInstructionPairRule, enabled_pair_rules,
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

    /// Whether any enabled pair admits `kind` as its consumer, whatever
    /// operand position its grammar folds. Admission-time error reporting
    /// distinguishes an unadmitted kind from an admitted kind whose literal
    /// sits at a position no grammar covers.
    pub(super) fn admits_consumer_kind(&self, kind: SelectedInstructionKind) -> bool {
        self.pairs
            .iter()
            .any(|pair| pair.rule.matches_consumer(kind))
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

/// The single catalog declaration for `semantic` bound to `constraint`, or
/// none when the catalog does not declare exactly one such form.
pub(super) fn effect_declaration(
    catalog: &ValidatedMachineEffectCatalog,
    semantic: MachineSemanticKind,
    constraint: RegisterConstraintKey,
) -> Option<&MachineEffectDeclaration> {
    let mut matches = catalog.catalog().declarations.iter().filter(|declaration| {
        declaration.semantic == semantic && declaration.constraint == constraint
    });
    let declaration = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(declaration)
}

pub(super) fn select_admitted_pairs<'a>(
    constraints: &'a ValidatedRegisterConstraintCatalog,
    keys: &TargetRegisterEnvironmentConstraintKeys,
    policy: LiteralFoldPolicy,
    catalog: &'a ValidatedMachineEffectCatalog,
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

/// Admit the rewritten row whose operand shape matches the rule's declared
/// operand grammar and result channel — a surviving `Use` operand plus scalar
/// `Def`, a lone `Def` for the constant folds, or implicit physical-unit
/// defs — and whose unit traffic satisfies the rule's declared unit-effect
/// surface: no implicit uses, clobbers, or operand unit bindings beyond it.
fn validate_immediate_row(
    rule: SelectedInstructionPairRule,
    row: &RegisterInstructionConstraint,
) -> Result<(), LiteralFoldError> {
    let unit_effects = rule.unit_effects();
    let clean = |operands: &[&register_model::RegisterOperandConstraint]| {
        operands
            .iter()
            .all(|operand| unit_effects.admits_operand(operand))
            && unit_effects.admits_row_units(row)
    };
    match (rule.operand_shape(), rule.result(), row.operands.as_slice()) {
        // Scalar-result form: `result = surviving <op> immediate`, or the
        // auxiliary-`Use` and scratch-defs grammars' copy rewrite — the
        // surviving `Use` binds operand 0 and the `Def` result binds
        // operand 1 whichever consumer grammar the literal occupied.
        (
            PairOperandShape::BinaryRightLiteral
            | PairOperandShape::BinaryLeftLiteral
            | PairOperandShape::BinaryRightLiteralAuxiliaryUses
            | PairOperandShape::BinaryRightLiteralAuxiliaryUsesOrScratchDefs
            | PairOperandShape::BinaryRightLiteralScratchDefs
            | PairOperandShape::BinaryLeftLiteralScratchDefs,
            PairResultDisposition::ScalarRegister,
            [left, result],
        ) => {
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
        (PairOperandShape::BinaryRightLiteral, PairResultDisposition::ImplicitUnits, [left]) => {
            if left.operand != 0
                || left.access != RegisterOperandAccess::Use
                || !clean(&[left])
                || row.implicit_defs.is_empty()
            {
                return Err(LiteralFoldError::ImmediateConstraintMismatch);
            }
        }
        // Constant-materialization form: `result = materialize folded` carries
        // no register input; its single `Def` operand is the only output. The
        // constant-result grammars rewrite into the same row: its folded
        // result is a `MaterializeI64` constant binding no `Use` operand,
        // whichever `Use` position the folded literal occupied.
        (
            PairOperandShape::UnaryLiteral
            | PairOperandShape::BinaryRightLiteralConstantResult
            | PairOperandShape::BinaryLeftLiteralConstantResult
            | PairOperandShape::BinaryLeftLiteralConstantResultAuxiliaryUses,
            PairResultDisposition::ScalarRegister,
            [result],
        ) => {
            if result.operand != 0
                || result.access != RegisterOperandAccess::Def
                || !clean(&[result])
                || !row.implicit_defs.is_empty()
            {
                return Err(LiteralFoldError::ImmediateConstraintMismatch);
            }
        }
        _ => return Err(LiteralFoldError::ImmediateConstraintMismatch),
    }
    Ok(())
}
