//! The concrete Console provider owns its writer and native byte leaves.

use super::*;

#[test]
fn selected_console_adapters_use_only_requirement_arguments() {
    let canary = pass_canary(fixture_roster::RUNTIME_ADAPTER_FORWARDING_EXIT);
    for target in [
        "windows_x86_64",
        "macos_arm64",
        "linux_x86_64",
        "linux_arm64",
    ] {
        let checked = compile_to_checked(&canary.join("main.omg"), Some(target))
            .unwrap_or_else(|diagnostics| panic!("{target}: {diagnostics:#?}"));
        let plan = checked
            .selected_provider_plans()
            .plans()
            .iter()
            .find(|plan| plan.schema.trait_name == "Console")
            .unwrap();
        assert_eq!(plan.provider_type, "ConsoleNativeProvider");
        assert!(plan.covers_schema());
        for method in ["write", "write_line"] {
            let name = format!("ConsoleNativeProvider::{method}");
            let adapter = checked
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == name)
                .unwrap();
            let [entry] = checked.machine_states(adapter) else {
                panic!("one adapter state");
            };
            let parameters = checked.state_parameters(entry);
            assert_eq!(
                parameters
                    .iter()
                    .map(|parameter| parameter.name.as_str())
                    .collect::<Vec<_>>(),
                ["text"],
                "{target}: {name} must not receive an extra service receiver"
            );
            let expected = checked_adapter_identity(&checked, &name);
            assert!(plan.rows.iter().any(|row| row.method == method && matches!(&row.binding,
                effects::provider_plan::ProviderBinding::CheckedAdapter { machine_identity, .. } if machine_identity == &expected)));
        }
        let writer = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "console_write_bytes")
            .unwrap();
        let expected_parameters: &[&[&str]] = &[
            &["bytes", "newline"],
            &["byte", "bytes", "newline"],
            &["newline"],
            &["byte"],
            &[],
        ];
        let states = checked.machine_states(writer);
        assert_eq!(states.len(), expected_parameters.len());
        for (state, expected) in states.iter().zip(expected_parameters) {
            assert_eq!(
                checked
                    .state_parameters(state)
                    .iter()
                    .map(|parameter| parameter.name.as_str())
                    .collect::<Vec<_>>(),
                *expected
            );
        }
        let byte_leaf = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "ConsoleNativeProvider::write_byte")
            .unwrap();
        let byte_entry = checked.machine_states(byte_leaf)[0].symbol;
        let byte_calls = states
            .iter()
            .flat_map(|state| checked.statement_table.statements(state.statement_nodes))
            .filter_map(|statement| {
                let checked_trees::statement::StatementNode::Call(call) = statement else {
                    return None;
                };
                Some(call.target_symbol)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            byte_calls,
            [byte_entry, byte_entry],
            "{target}: both output paths stay inside the selected concrete provider"
        );

        let main = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::main")
            .unwrap();
        let entry = &checked.machine_states(main)[0];
        let mut adapter_calls = 0;
        for statement in checked.statement_table.statements(entry.statement_nodes) {
            let checked_trees::statement::StatementNode::Call(call) = statement else {
                continue;
            };
            if [
                "ConsoleNativeProvider::write",
                "ConsoleNativeProvider::write_line",
            ]
            .contains(&call.target.as_str())
            {
                assert_eq!(
                    checked
                        .statement_table
                        .expression_handles(call.arguments)
                        .len(),
                    1,
                    "{target}: selected dispatch must not inject a phantom receiver"
                );
                adapter_calls += 1;
            }
        }
        assert_eq!(adapter_calls, 5);
    }
}

#[test]
fn selected_console_writer_preserves_checked_output_on_each_target() {
    let canary = pass_canary(fixture_roster::RUNTIME_ADAPTER_FORWARDING_EXIT);
    for target in [
        "windows_x86_64",
        "macos_arm64",
        "linux_x86_64",
        "linux_arm64",
    ] {
        let checked = compile_to_checked(&canary.join("main.omg"), Some(target))
            .unwrap_or_else(|diagnostics| panic!("{target}: {diagnostics:#?}"));
        let outcome = interpret(&checked, &[]);
        assert_eq!(
            outcome.error, None,
            "{target}: checked-source writer execution"
        );
        assert_eq!(outcome.exit_code, 70);
        assert_eq!(
            outcome.stdout, b"Field\nLiteral\n\x80\xff",
            "{target}: selected writer output"
        );
    }
}
