//! Independent ABI expectations for every mixed aggregate call row.
use super::*;
use register_model::validate_physical_register_model;

#[test]
fn mixed_aggregate_rows_preserve_banks_results_and_other_call_effects() {
    let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let catalog = x86_64_register_constraint_catalog(&model);
    validate_x86_64_register_constraint_catalog(catalog.clone(), &model).unwrap();
    let mut expected_rows = Vec::new();
    for fragments in 1..=2 {
        for general_count in 0..=6 {
            for float_count in 1..=8 {
                let names = ["rdi", "rsi", "rdx", "rcx", "r8", "r9"]
                    .into_iter()
                    .take(general_count)
                    .map(str::to_owned)
                    .chain((0..float_count).map(|position| format!("xmm{position}")))
                    .collect::<Vec<_>>();
                expected_rows.push((
                    names,
                    fragments,
                    x86_64_system_v_register_unit_call_keys()[0],
                ));
            }
        }
    }
    for arity in 1..=4 {
        for float_mask in 1..(1_u16 << arity) {
            let names = ["rcx", "rdx", "r8", "r9"]
                .into_iter()
                .take(arity)
                .enumerate()
                .filter(|(position, _)| float_mask & (1 << position) == 0)
                .map(|(_, name)| name.to_owned())
                .chain(
                    (0..arity)
                        .filter(|position| float_mask & (1 << position) != 0)
                        .map(|position| format!("xmm{position}")),
                )
                .collect::<Vec<_>>();
            expected_rows.push((names, 1, x86_64_microsoft_register_unit_call_keys()[0]));
        }
    }
    let keys = x86_64_system_v_mixed_aggregate_call_keys()
        .into_iter()
        .chain(x86_64_microsoft_mixed_aggregate_call_keys())
        .collect::<Vec<_>>();
    assert_eq!(keys.len(), expected_rows.len());
    for (key, (inputs, fragments, base_key)) in keys.into_iter().zip(expected_rows) {
        let row = catalog
            .constraints
            .iter()
            .find(|row| row.key == key)
            .unwrap();
        let base = catalog
            .constraints
            .iter()
            .find(|row| row.key == base_key)
            .unwrap();
        let names = inputs
            .iter()
            .map(String::as_str)
            .chain(["rax", "rdx"].into_iter().take(fragments))
            .collect::<Vec<_>>();
        assert_eq!(row.operands.len(), names.len());
        for (position, (operand, name)) in row.operands.iter().zip(names).enumerate() {
            let view = model.model().view_named(name).unwrap();
            assert_eq!(operand.operand, position as u16);
            assert_eq!(operand.fixed_view, Some(view.id));
            assert_eq!(operand.class, view.class);
            assert_eq!(
                operand.access,
                if position < inputs.len() {
                    RegisterOperandAccess::Use
                } else {
                    RegisterOperandAccess::Def
                }
            );
            assert_eq!(operand.tied_to, None);
            assert!(!operand.early_clobber);
        }
        let result_units = ["rax", "rdx"]
            .into_iter()
            .take(fragments)
            .flat_map(|name| &model.model().view_named(name).unwrap().write_units)
            .collect::<Vec<_>>();
        assert_eq!(
            row.clobbers,
            base.clobbers
                .iter()
                .copied()
                .filter(|unit| !result_units.contains(&unit))
                .collect::<Vec<_>>()
        );
        assert_eq!(row.implicit_uses, base.implicit_uses);
        assert_eq!(row.implicit_defs, base.implicit_defs);

        for mutation in 0..3 {
            let mut changed = catalog.clone();
            let row = changed
                .constraints
                .iter_mut()
                .find(|row| row.key == key)
                .unwrap();
            match mutation {
                0 => {
                    row.clobbers.push(*result_units[0]);
                    row.clobbers.sort_unstable();
                }
                1 => row.operands.last_mut().unwrap().access = RegisterOperandAccess::Use,
                _ => {
                    row.operands[inputs.len() - 1].fixed_view =
                        Some(model.model().view_named("r11").unwrap().id)
                }
            }
            assert!(
                validate_x86_64_register_constraint_catalog(changed, &model).is_err(),
                "{key:?} mutation {mutation}"
            );
        }
    }
}
