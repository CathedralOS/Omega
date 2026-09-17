//! The requirement-side intrinsic bridge: a core float intrinsic spelled as a
//! top-level `boundary requirement` settles its direct call to the same
//! compiler-intrinsic execution the operator spelling settles to, keyed on the
//! requirement symbol.

use super::fixture_roster;
use crate::{compile_reviewed_repository_fixture, interpret, pass_canary};
use compiler::CheckedCompileRequest;

#[test]
fn requirement_spelled_fused_multiply_add_settles_to_the_selected_unit_intrinsic() {
    let canary = pass_canary(fixture_roster::FLOAT_NAMED_REQUIREMENT_FUSED_MULTIPLY_ADD_EXIT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("the requirement-spelled FMA fixture should compile to checked trees");

    let requirement = checked
        .typed
        .machines()
        .iter()
        .find(|machine| {
            machine.name.as_str() == "F32::fused_multiply_add"
                && machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
        })
        .expect("core spells `F32::fused_multiply_add` as a top-level boundary requirement");
    let uses = checked
        .facts
        .operators
        .named_requirement_uses()
        .filter(|selected_use| selected_use.provider_plan_report_fingerprint != 0)
        .collect::<Vec<_>>();
    let [selected_use] = uses.as_slice() else {
        panic!("exactly one direct requirement call is stamped with its plan: {uses:?}");
    };
    assert_eq!(selected_use.requirement_symbol, requirement.symbol);
    let plan = checked
        .selected_provider_plans()
        .plan_by_report_fingerprint(selected_use.provider_plan_report_fingerprint)
        .expect("the stamped fingerprint names one retained plan");
    assert_eq!(
        plan.name.as_str(),
        "FloatNativeProvider::satisfies::F32::fused_multiply_add"
    );
    assert!(
        checked
            .facts
            .operators
            .named_uses()
            .all(|operator_use| { operator_use.selected_operator_symbol != requirement.symbol }),
        "the requirement spelling retains no operator use"
    );
    assert!(
        checked
            .facts
            .operators
            .uses
            .iter()
            .all(|(_, operator_use)| {
                operator_use.selected_operator_symbol != requirement.symbol
            }),
        "the requirement spelling retains no operator use"
    );

    let entry = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("Main::main");
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .omissions
            .iter()
            .all(|omission| omission.machine != entry.symbol),
        "the settled entry body plans as a Unit machine: {:?}",
        checked.facts.flow.terminal_unit_effects.omissions
    );
    let fma_operations = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .filter(|machine| machine.machine == entry.symbol)
        .flat_map(|machine| machine.operations.iter())
        .filter_map(|operation| match operation {
            checked_trees::CheckedUnitEffectOperationPlan::SelectedIeeeFloatFusedMultiplyAdd {
                requirement_operator,
                provider_plan_report_fingerprint,
                format,
                operands,
                ..
            } => Some((
                *requirement_operator,
                *provider_plan_report_fingerprint,
                *format,
                operands.len(),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        fma_operations,
        vec![(
            requirement.symbol,
            plan.report_fingerprint(),
            semantic_vocabulary::IeeeFloatFormat::Binary32,
            3,
        )],
        "the attached Unit local initializer is the selected nearest FMA keyed on the requirement symbol"
    );

    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "one rounding of (1 + 2^-23)^2 + 2^-24 exceeds 1 + 2^-22"
    );
}
