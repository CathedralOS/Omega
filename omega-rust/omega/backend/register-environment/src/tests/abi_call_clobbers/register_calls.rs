use super::*;
use register_model::RegisterOperandAccess;

#[test]
fn every_register_call_arity_retains_exact_target_abi_and_rejects_clobber_loss() {
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let keys = environment.selected_keys().call_i64;
        assert_eq!(keys.len(), case.arguments.len() + 1);
        let generic = environment.constraint(case.call).unwrap();
        let raw = target_physical_register_model(case.target);
        let catalog = target_constraint_catalog(case.target, environment.physical());
        for (arity, key) in keys.into_iter().enumerate() {
            let row = environment.constraint(key).unwrap();
            assert_eq!(row.operands.len(), arity + 1);
            let selected_uses: &[&str] = match case.target.architecture {
                Architecture::X86_64 => &["rsp", "rip"],
                Architecture::Aarch64 => &["sp", "pc"],
            };
            assert_eq!(
                row.implicit_uses,
                units_for_names(environment.physical().model(), selected_uses)
            );
            assert_eq!(row.implicit_defs, generic.implicit_defs);
            assert_eq!(row.clobbers, generic.clobbers);
            for (ordinal, (operand, name)) in row
                .operands
                .iter()
                .zip(case.arguments[..arity].iter().copied().chain([case.result]))
                .enumerate()
            {
                assert_eq!(operand.operand, ordinal as u16);
                assert_eq!(
                    operand.access,
                    if ordinal == arity {
                        RegisterOperandAccess::Def
                    } else {
                        RegisterOperandAccess::Use
                    }
                );
                assert_eq!(
                    operand.fixed_view,
                    Some(environment.physical().model().view_named(name).unwrap().id)
                );
            }
            let mut corrupted = catalog.clone();
            row_mut(&mut corrupted, key).clobbers.remove(0);
            let error = validate_target_register_environment(case.target, raw.clone(), corrupted)
                .expect_err("every arity must retain its exact ABI clobbers");
            assert_target_semantic_error(case.target, key, error);
        }
    }
}
