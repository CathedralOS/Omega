//! Target-origin admission for the payload-free hosted byte-output candidate.
use super::*;

#[test]
fn macos_origin_infers_output_and_exit_and_replays_exact_custody() {
    let source = r#"
        boundary trait Console {
            machine write_byte(byte: i32);
            machine exit_process(code: i32);
            machine read_byte(value: i32);
        }
        data ConsoleNativeProvider {}
        boundary machine ConsoleNativeProvider::write_byte(byte: i32)
            satisfies Console::write_byte;
        boundary machine ConsoleNativeProvider::exit_process(code: i32)
            satisfies Console::exit_process;
        boundary machine ConsoleNativeProvider::read_byte(value: i32)
            satisfies Console::read_byte;
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let evaluated = crate::evaluated_via_bindings::evaluate_via_bindings(
        &typed,
        Some(target::TargetProfile::MacosArm64),
        None,
    )
    .unwrap();
    let origins = typed
        .machines()
        .iter()
        .map(|machine| SelectedTargetMachineOrigin {
            machine: machine.symbol,
            target: "macos_arm64".to_owned(),
        })
        .collect::<Vec<_>>();
    let candidates = derive_satisfies_plans_with_evaluated_bindings_and_target_machine_origins(
        &typed,
        Some("macos_arm64"),
        &evaluated,
        &origins,
    )
    .unwrap();
    let [candidate] = candidates.as_slice() else {
        panic!("one Console candidate");
    };
    let mut methods = candidate
        .plan
        .rows
        .iter()
        .map(|row| row.method.as_str())
        .collect::<Vec<_>>();
    methods.sort_unstable();
    assert_eq!(methods, ["exit_process", "write_byte"]);
    assert_eq!(candidate.plan.target, "macos_arm64");
    assert!(
        candidate
            .plan
            .rows
            .iter()
            .all(|row| matches!(row.binding, ProviderBinding::CompilerIntrinsic { .. }))
    );
    assert!(validate_derived_provider_plan_candidates(&typed, &evaluated, &candidates).is_empty());
    for wrong_target in ["linux_arm64", "windows_x86_64"] {
        let mut wrong_origins = origins.clone();
        for origin in &mut wrong_origins {
            origin.target = wrong_target.to_owned();
        }
        assert!(
            derive_satisfies_plans_with_evaluated_bindings_and_target_machine_origins(
                &typed,
                Some("macos_arm64"),
                &evaluated,
                &wrong_origins,
            )
            .unwrap()
            .is_empty()
        );
    }
    assert!(derive_satisfies_plans_with_provenance(&typed, Some("macos_arm64")).is_empty());
    let mut changed = candidate.clone();
    changed.provenance.row_target_machine_origins[0]
        .as_mut()
        .unwrap()
        .target = "linux_arm64".into();
    assert!(!validate_derived_provider_plan_candidates(&typed, &evaluated, &[changed]).is_empty());
}
