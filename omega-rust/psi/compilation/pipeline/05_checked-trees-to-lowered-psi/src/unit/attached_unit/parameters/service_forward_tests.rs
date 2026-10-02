//! Forwarding is tied to each authored actual, including same-typed fields.

use super::validate;
use checked_trees::{CheckedStructuralAccess, CheckedTrees, CheckedUnitCallPlan};
use source::{SourceMap, SourceOrigin};
use std::path::PathBuf;

fn checked_source(body: &str) -> CheckedTrees {
    let binding = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../source/library/core/binding.omg"
    ));
    let program = format!(
        "pub boundary trait Host {{ machine mark(value: u64); }}
         pub data Context {{ left: Binding<Host>; right: Binding<Host>; }}
         {body}"
    );
    let mut sources = SourceMap::default();
    let binding_source = sources
        .add_with_metadata(
            PathBuf::from("source/library/core/binding.omg"),
            binding.into(),
            PathBuf::from("source/library/core"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let program_source = sources
        .add(PathBuf::from("tests/main.omg"), program.clone())
        .source_id;
    let mut typed = crate::front_end::typed_program_from_source_map(
        sources,
        &[(binding_source, binding), (program_source, &program)],
    );
    let authorizations = typed
        .traits()
        .iter()
        .filter(|row| row.is_boundary)
        .map(
            |row| typed_trees::typed_trees::FusedServiceErasureAuthorization {
                requirement: row.symbol,
            },
        )
        .collect();
    typed
        .bind_fused_service_erasures(authorizations)
        .expect("selected Fused fixture");
    typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("forwarding source checks")
}

fn field_fixture(composed_caller: bool, composed_target: bool) -> CheckedTrees {
    let target = if composed_target {
        "transition enabled { true -> output(value, host) false -> done(host) }
         state output(value: u64, host: &mut Binding<Host>) { host.mark(value); }
         state done(host: &mut Binding<Host>) {}"
    } else {
        "host.mark(value);"
    };
    let calls = "let value: u64 = 7; emit(value, &mut context.left, true);
                 emit(8, &mut context.right, true);";
    let caller = if composed_caller {
        format!(
            "transition enabled {{ true -> output(context) false -> done(context) }}
                 state output(context: &mut Context) {{ {calls} }}
                 state done(context: &mut Context) {{}}"
        )
    } else {
        calls.into()
    };
    checked_source(&format!(
        "machine emit(value: u64, host: &mut Binding<Host>, enabled: bool) reaches Host {{ {target} }}
         machine forward(context: &mut Context, enabled: bool) reaches Host {{ {caller} }}"
    ))
}

fn ordinary_calls(checked: &mut CheckedTrees) -> Vec<&mut CheckedUnitCallPlan> {
    let plans = &mut checked.facts.flow.terminal_unit_effects;
    plans
        .machines
        .iter_mut()
        .flat_map(|plan| &mut plan.operations)
        .chain(
            plans
                .composed_machines
                .iter_mut()
                .flat_map(|plan| &mut plan.states)
                .flat_map(|state| &mut state.operations),
        )
        .filter_map(|operation| operation.call_mut())
        .filter(|call| !call.is_boundary())
        .collect()
}

#[test]
fn service_field_forwarding_rejoins_direct_and_composed_actuals() {
    for composed_caller in [false, true] {
        for composed_target in [false, true] {
            let mut checked = field_fixture(composed_caller, composed_target);
            assert_eq!(
                ordinary_calls(&mut checked).len(),
                2,
                "producer omitted forwarding calls: caller={composed_caller}, target={composed_target}"
            );
            validate(&checked).expect("exact field loans in an ordinary multi-operation body");
            crate::lower_machine(&checked, crate::TerminalMachineSelection::Name("forward"))
                .expect("authenticated service loans also reach Terminal production");
        }
    }
}

#[test]
fn service_field_forwarding_rejects_same_type_actual_substitution() {
    for composed_caller in [false, true] {
        for composed_target in [false, true] {
            for mutation in 0..3 {
                let mut checked = field_fixture(composed_caller, composed_target);
                let mut calls = ordinary_calls(&mut checked);
                assert_eq!(calls.len(), 2, "producer omitted field forwarding plans");
                let other_path = calls[1].structural_arguments[0].path.clone();
                let argument = &mut calls[0].structural_arguments[0];
                assert_ne!(argument.path, other_path);
                match mutation {
                    0 => argument.path = other_path,
                    1 => argument.access = CheckedStructuralAccess::Owned,
                    2 => argument.path.clear(),
                    _ => unreachable!(),
                }
                assert!(
                    validate(&checked).is_err(),
                    "caller={composed_caller}, target={composed_target}, mutation={mutation}"
                );
                assert!(
                    crate::lower_machine(
                        &checked,
                        crate::TerminalMachineSelection::Name("forward"),
                    )
                    .is_err(),
                    "lowering must retain exact authored-actual custody"
                );
            }
        }
    }
}

#[test]
fn service_whole_parameter_forwarding_preserves_access_and_receipt() {
    for access in ["", "&", "&mut "] {
        let mut checked = checked_source(&format!(
            "machine emit(host: {access}Binding<Host>, value: u64) reaches Host {{ host.mark(value); }}
             machine forward(host: {access}Binding<Host>, value: u64) reaches Host {{
                 host.mark(value); emit(host, value);
             }}"
        ));
        let omissions = checked.facts.flow.terminal_unit_effects.omissions.clone();
        assert_eq!(
            ordinary_calls(&mut checked).len(),
            1,
            "producer omitted whole forwarding for access={access:?}: {omissions:?}"
        );
        validate(&checked).expect("whole service argument with other body operations");
        let emit = checked
            .machines()
            .iter()
            .find(|row| row.name.as_str() == "emit")
            .unwrap()
            .symbol;
        checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|row| row.machine == emit)
            .expect("direct target")
            .structural_parameters[0]
            .fused_service_erasure = None;
        assert!(
            validate(&checked).is_err(),
            "missing target receipt for access={access}"
        );
    }
}
