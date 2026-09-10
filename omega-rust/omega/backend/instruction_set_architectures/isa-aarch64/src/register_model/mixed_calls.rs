//! Fixed register calls with independently allocated integer and IEEE input banks.
use super::*;

/// AAPCS64 rows ordered by GPR count (0–8), then IEEE register count (1–8).
pub fn aarch64_aapcs64_mixed_unit_call_keys() -> Vec<RegisterConstraintKey> {
    keys(800..872)
}

/// Darwin rows ordered by GPR count (0–8), then IEEE register count (1–8).
pub fn aarch64_darwin_mixed_unit_call_keys() -> Vec<RegisterConstraintKey> {
    keys(872..944)
}

/// Integer aggregate results ordered by fragment count (1–2), then the mixed
/// Unit-call input order: GPR count (0–8), IEEE register count (1–8).
/// Existing integer-only aggregate key ordinals remain unchanged.
pub fn aarch64_mixed_aggregate_call_keys(darwin: bool) -> Vec<RegisterConstraintKey> {
    let first = if darwin { 1300 } else { 1100 };
    keys(first..first + 144)
}

fn keys(variants: std::ops::Range<u16>) -> Vec<RegisterConstraintKey> {
    variants
        .map(|variant| RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant: u32::from(variant),
        })
        .collect()
}

pub(super) fn append_constraints(
    constraints: &mut Vec<RegisterInstructionConstraint>,
    model: &ValidatedPhysicalRegisterModel,
) {
    for (base_key, keys) in [
        (
            aarch64_aapcs64_register_unit_call_keys()[0],
            aarch64_aapcs64_mixed_unit_call_keys(),
        ),
        (
            aarch64_darwin_register_unit_call_keys()[0],
            aarch64_darwin_mixed_unit_call_keys(),
        ),
    ] {
        let Some(base) = constraints.iter().find(|row| row.key == base_key).cloned() else {
            continue;
        };
        let mut keys = keys.into_iter();
        for general_count in 0..=8 {
            for float_count in 1..=8 {
                let mut row = base.clone();
                let Some(key) = keys.next() else { return };
                row.key = key;
                let Some(operands) = (0..general_count)
                    .map(|position| format!("x{position}"))
                    .chain((0..float_count).map(|position| format!("d{position}")))
                    .enumerate()
                    .map(|(operand, name)| {
                        let view = model.model().view_named(&name)?;
                        Some(RegisterOperandConstraint {
                            operand: operand as u16,
                            access: RegisterOperandAccess::Use,
                            class: view.class,
                            fixed_view: Some(view.id),
                            tied_to: None,
                            early_clobber: false,
                        })
                    })
                    .collect::<Option<Vec<_>>>()
                else {
                    return;
                };
                row.operands = operands;
                constraints.push(row);
            }
        }
    }
    for darwin in [false, true] {
        let unit_keys = if darwin {
            aarch64_darwin_mixed_unit_call_keys()
        } else {
            aarch64_aapcs64_mixed_unit_call_keys()
        };
        for (ordinal, key) in aarch64_mixed_aggregate_call_keys(darwin)
            .into_iter()
            .enumerate()
        {
            let Some(mut row) = constraints
                .iter()
                .find(|row| row.key == unit_keys[ordinal % 72])
                .cloned()
            else {
                return;
            };
            row.key = key;
            let result_count = ordinal / 72 + 1;
            for name in ["x0", "x1"].into_iter().take(result_count) {
                let Some(view) = model.model().view_named(name) else {
                    return;
                };
                row.operands.push(RegisterOperandConstraint {
                    operand: row.operands.len() as u16,
                    access: RegisterOperandAccess::Def,
                    class: view.class,
                    fixed_view: Some(view.id),
                    tied_to: None,
                    early_clobber: false,
                });
                // A returned fragment is a known definition, not an unknown
                // clobber. All other ABI effects remain those of the Unit row.
                row.clobbers.retain(|unit| !view.write_units.contains(unit));
            }
            constraints.push(row);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use register_model::validate_physical_register_model;

    #[test]
    fn mixed_aggregate_rows_preserve_banks_and_define_only_result_fragments() {
        let model = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let catalog = aarch64_register_constraint_catalog(&model);
        for darwin in [false, true] {
            let unit_keys = if darwin {
                aarch64_darwin_mixed_unit_call_keys()
            } else {
                aarch64_aapcs64_mixed_unit_call_keys()
            };
            let old_keys = aarch64_register_aggregate_call_keys(darwin);
            for (ordinal, key) in aarch64_mixed_aggregate_call_keys(darwin)
                .into_iter()
                .enumerate()
            {
                assert!(!old_keys.contains(&key));
                let fragments = ordinal / 72 + 1;
                let general_count = ordinal % 72 / 8;
                let float_count = ordinal % 8 + 1;
                let arity = general_count + float_count;
                let row = catalog
                    .constraints
                    .iter()
                    .find(|row| row.key == key)
                    .unwrap();
                let unit = catalog
                    .constraints
                    .iter()
                    .find(|row| row.key == unit_keys[ordinal % 72])
                    .unwrap();
                assert_eq!(row.operands.len(), arity + fragments);
                let names = (0..general_count)
                    .map(|position| format!("x{position}"))
                    .chain((0..float_count).map(|position| format!("d{position}")))
                    .chain((0..fragments).map(|position| format!("x{position}")));
                for (position, (operand, name)) in row.operands.iter().zip(names).enumerate() {
                    let view = model.model().view_named(&name).unwrap();
                    assert_eq!(operand.operand, position as u16);
                    assert_eq!(operand.fixed_view, Some(view.id));
                    assert_eq!(operand.class, view.class);
                    assert_eq!(
                        operand.access,
                        if position < arity {
                            RegisterOperandAccess::Use
                        } else {
                            RegisterOperandAccess::Def
                        }
                    );
                    assert_eq!(operand.tied_to, None);
                    assert!(!operand.early_clobber);
                }
                assert_eq!(row.implicit_uses, unit.implicit_uses);
                assert_eq!(row.implicit_defs, unit.implicit_defs);
                let mut clobbers = unit.clobbers.clone();
                for name in ["x0", "x1"].into_iter().take(fragments) {
                    let view = model.model().view_named(name).unwrap();
                    clobbers.retain(|unit| !view.write_units.contains(unit));
                }
                assert_eq!(row.clobbers, clobbers);
            }
        }
        validate_aarch64_register_constraint_catalog(catalog, &model).unwrap();
    }

    #[test]
    fn mixed_aggregate_rows_reject_result_bank_access_and_clobber_mutations() {
        let model = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let catalog = aarch64_register_constraint_catalog(&model);
        for darwin in [false, true] {
            let key = aarch64_mixed_aggregate_call_keys(darwin)[143];
            for mutation in 0..4 {
                let mut changed = catalog.clone();
                let row = changed
                    .constraints
                    .iter_mut()
                    .find(|row| row.key == key)
                    .unwrap();
                match mutation {
                    0 => row.operands.last_mut().unwrap().access = RegisterOperandAccess::Use,
                    1 => {
                        let view = model.model().view_named("d1").unwrap();
                        let operand = row.operands.last_mut().unwrap();
                        operand.fixed_view = Some(view.id);
                        operand.class = view.class;
                    }
                    2 => row
                        .clobbers
                        .extend(&model.model().view_named("x1").unwrap().write_units),
                    3 => row.clobbers.retain(|unit| {
                        !model
                            .model()
                            .view_named("x2")
                            .unwrap()
                            .write_units
                            .contains(unit)
                    }),
                    _ => unreachable!(),
                }
                assert!(validate_aarch64_register_constraint_catalog(changed, &model).is_err());
            }
        }
    }
}
