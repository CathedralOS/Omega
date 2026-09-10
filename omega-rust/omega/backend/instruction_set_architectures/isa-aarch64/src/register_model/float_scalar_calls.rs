//! Ordinary scalar results use the ABI floating register without a wrapper call.
use super::*;

pub fn aarch64_float_scalar_call_keys(darwin: bool) -> Vec<RegisterConstraintKey> {
    let count = input_keys(darwin).len()
        + (if darwin {
            aarch64_darwin_mixed_unit_call_keys()
        } else {
            aarch64_aapcs64_mixed_unit_call_keys()
        })
        .len();
    (0..count)
        .map(|ordinal| RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant: (if darwin { 1800 } else { 1600 }) + ordinal as u32,
        })
        .collect()
}

pub fn aarch64_float_scalar_return_keys(darwin: bool) -> Vec<RegisterConstraintKey> {
    vec![RegisterConstraintKey {
        family: RegisterConstraintFamily::Return,
        variant: if darwin { 1601 } else { 1600 },
    }]
}

fn input_keys(darwin: bool) -> Vec<RegisterConstraintKey> {
    (if darwin {
        aarch64_darwin_register_unit_call_keys()
    } else {
        aarch64_aapcs64_register_unit_call_keys()
    })
    .into_iter()
    .chain(if darwin {
        aarch64_darwin_mixed_unit_call_keys()
    } else {
        aarch64_aapcs64_mixed_unit_call_keys()
    })
    .collect()
}

pub(super) fn append_constraints(
    constraints: &mut Vec<RegisterInstructionConstraint>,
    model: &ValidatedPhysicalRegisterModel,
) {
    let Some(result) = model.model().view_named("d0") else {
        return;
    };
    for darwin in [false, true] {
        let inputs = input_keys(darwin).into_iter().map(|key| (key, "d0")).chain(
            (if darwin {
                aarch64_darwin_mixed_unit_call_keys()
            } else {
                aarch64_aapcs64_mixed_unit_call_keys()
            })
            .into_iter()
            .map(|key| (key, "x0")),
        );
        for ((input, result_name), key) in inputs.zip(aarch64_float_scalar_call_keys(darwin)) {
            let Some(result) = model.model().view_named(result_name) else {
                return;
            };
            let Some(mut row) = constraints.iter().find(|row| row.key == input).cloned() else {
                return;
            };
            row.key = key;
            row.operands.push(RegisterOperandConstraint {
                operand: row.operands.len() as u16,
                access: RegisterOperandAccess::Def,
                class: result.class,
                fixed_view: Some(result.id),
                tied_to: None,
                early_clobber: false,
            });
            // The result Def already covers its full architectural write footprint,
            // including upper vector lanes. Retaining those units as independent
            // call clobbers would conflict with the result's own fixed ABI home.
            row.clobbers
                .retain(|unit| !result.write_units.contains(unit));
            constraints.push(row);
        }
        let base = if darwin {
            AARCH64_DARWIN_RETURN
        } else {
            AARCH64_AAPCS64_RETURN
        };
        let Some(mut row) = constraints.iter().find(|row| row.key == base).cloned() else {
            return;
        };
        row.key = aarch64_float_scalar_return_keys(darwin)[0];
        row.operands = vec![RegisterOperandConstraint {
            operand: 0,
            access: RegisterOperandAccess::Use,
            class: result.class,
            fixed_view: Some(result.id),
            tied_to: None,
            early_clobber: false,
        }];
        constraints.push(row);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floating_return_uses_ordinary_return_bytes_and_exact_home() {
        let physical =
            register_model::validate_physical_register_model(aarch64_physical_register_model())
                .unwrap();
        let home = physical.model().view_named("d0").unwrap().id;
        let wrong = physical.model().view_named("d1").unwrap().id;
        let kind = selected_instructions::SelectedInstructionKind::ReturnScalar;
        let alternative = selected_instructions::MachineAlternativeKey {
            family: selected_instructions::MachineAlternativeFamily::ReturnScalar,
            variant: 0,
        };
        let encoded = crate::encode_aarch64_selected_form(&physical, kind, alternative, &[home])
            .expect("floating return");
        crate::validate_aarch64_selected_form_encoding(
            &physical,
            kind,
            alternative,
            &[home],
            encoded.bytes(),
        )
        .unwrap();
        assert!(
            crate::validate_aarch64_selected_form_encoding(
                &physical,
                kind,
                alternative,
                &[wrong],
                encoded.bytes()
            )
            .is_err()
        );
    }

    #[test]
    fn scalar_float_roster_is_complete_and_binds_result_home_and_clobbers() {
        let physical =
            register_model::validate_physical_register_model(aarch64_physical_register_model())
                .expect("physical model");
        let catalog = aarch64_register_constraint_catalog(&physical);
        validate_aarch64_register_constraint_catalog(catalog.clone(), &physical)
            .expect("complete catalog");
        let mut keys = Vec::new();
        for darwin in [false, true] {
            keys.extend(aarch64_float_scalar_call_keys(darwin));
            keys.extend(aarch64_float_scalar_return_keys(darwin));
        }
        let count = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), count, "ABI key ranges do not overlap");
        for key in keys {
            assert_eq!(
                catalog
                    .constraints
                    .iter()
                    .filter(|row| row.key == key)
                    .count(),
                1
            );
        }
        let result = physical.model().view_named("d0").unwrap();
        let call_key = aarch64_float_scalar_call_keys(false)[0];
        let row = catalog
            .constraints
            .iter()
            .find(|row| row.key == call_key)
            .unwrap();
        assert_eq!(row.operands.last().unwrap().fixed_view, Some(result.id));
        assert!(
            result
                .write_units
                .iter()
                .all(|unit| !row.clobbers.contains(unit))
        );
        let input = catalog
            .constraints
            .iter()
            .find(|row| row.key == aarch64_aapcs64_register_unit_call_keys()[0])
            .unwrap();
        assert!(
            input
                .clobbers
                .iter()
                .all(|unit| row.clobbers.contains(unit) || result.write_units.contains(unit)),
            "every caller-save effect remains either a Def write or a clobber"
        );
        let mut wrong_home = catalog.clone();
        let row = wrong_home
            .constraints
            .iter_mut()
            .find(|row| row.key == call_key)
            .unwrap();
        row.operands.last_mut().unwrap().fixed_view =
            Some(physical.model().view_named("d1").unwrap().id);
        assert!(validate_aarch64_register_constraint_catalog(wrong_home, &physical).is_err());
        let mut wrong_clobbers = catalog;
        wrong_clobbers
            .constraints
            .iter_mut()
            .find(|row| row.key == call_key)
            .unwrap()
            .clobbers
            .clear();
        assert!(validate_aarch64_register_constraint_catalog(wrong_clobbers, &physical).is_err());
    }
}
