use super::*;
use register_model::RegisterOperandAccess;

#[test]
fn every_register_call_arity_retains_exact_target_abi_and_rejects_clobber_loss() {
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let selected_keys = environment.selected_keys();
        let (keys, floating_keys) = selected_keys.call_scalar.split_at(case.arguments.len() + 1);
        let expected_floating_keys = match case.target.architecture {
            Architecture::X86_64 => isa_x86_64::x86_64_float_scalar_call_keys(
                case.target == NativeTarget::windows_x64(),
            ),
            Architecture::Aarch64 => isa_aarch64::aarch64_float_scalar_call_keys(
                case.target == NativeTarget::macos_arm64(),
            ),
        };
        assert_eq!(floating_keys, expected_floating_keys);
        assert_eq!(keys.len(), case.arguments.len() + 1);
        let generic = environment.constraint(case.call).unwrap();
        let raw = target_physical_register_model(case.target);
        let catalog = target_constraint_catalog(case.target, environment.physical());
        for (arity, key) in keys.iter().copied().enumerate() {
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
        let float_name = match case.target.architecture {
            Architecture::X86_64 => "xmm0",
            Architecture::Aarch64 => "d0",
        };
        let expected_inputs = selected_keys
            .call_unit
            .iter()
            .chain(&selected_keys.call_unit_mixed)
            .copied()
            .map(|key| (key, float_name))
            .chain(
                selected_keys
                    .call_unit_mixed
                    .iter()
                    .copied()
                    .map(|key| (key, case.result)),
            )
            .collect::<Vec<_>>();
        assert_eq!(floating_keys.len(), expected_inputs.len());
        for (key, (input_key, result_name)) in floating_keys.iter().copied().zip(expected_inputs) {
            let row = environment.constraint(key).unwrap();
            let input = environment.constraint(input_key).unwrap();
            let result = environment
                .physical()
                .model()
                .view_named(result_name)
                .unwrap();
            let (output, arguments) = row.operands.split_last().unwrap();
            assert_eq!(arguments, input.operands);
            assert_eq!(output.operand as usize, arguments.len());
            assert_eq!(output.access, RegisterOperandAccess::Def);
            assert_eq!(output.class, result.class);
            assert_eq!(output.fixed_view, Some(result.id));
            assert_eq!(row.implicit_uses, input.implicit_uses);
            assert_eq!(row.implicit_defs, input.implicit_defs);
            let expected_clobbers = input
                .clobbers
                .iter()
                .copied()
                .filter(|unit| !result.write_units.contains(unit))
                .collect::<Vec<_>>();
            assert_eq!(row.clobbers, expected_clobbers);
            let mut corrupted = catalog.clone();
            row_mut(&mut corrupted, key).clobbers.remove(0);
            let error = validate_target_register_environment(case.target, raw.clone(), corrupted)
                .expect_err("each mixed-bank scalar row retains exact remaining ABI clobbers");
            assert_target_semantic_error(case.target, key, error);
        }
    }
}
