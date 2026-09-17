//! The requirement-side intrinsic bridge: a core float intrinsic spelled as a
//! top-level `boundary requirement` settles its direct call to the same
//! compiler-intrinsic execution the operator spelling settles to, keyed on the
//! requirement symbol.

use super::fixture_roster;
use crate::{
    Command, CompileRequest, CompilerOptions, RequestedCompileProduct,
    compile_reviewed_repository_fixture, compile_rooted_canary_for_native_host, executable_name,
    fs, interpret, pass_canary, reviewed_repository_fixture_package_inputs,
};
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

    // The retained Terminal product (macOS arm64 needs no x86 FMA deployment
    // admission) rejoins the Terminal FMA occurrence to the requirement-keyed
    // demand: one D29 demand, one exact compiler-intrinsic realization row,
    // one nearest-FMA occurrence proposal naming the same plan.
    let root_path = canary.join("main.omg");
    let package_inputs =
        reviewed_repository_fixture_package_inputs(&root_path, Some("macos_arm64"))
            .expect("the requirement-spelled FMA fixture derives reviewed package inputs");
    let mut request = CompileRequest::new(CompilerOptions {
        root_path,
        build_dir: None,
        target_name: Some("macos_arm64".into()),
    })
    .with_requested_product(RequestedCompileProduct::TerminalArtifact);
    if let Some(package_inputs) = package_inputs {
        request = request.with_package_inputs(package_inputs);
    }
    let report = compiler::compile(request)
        .and_then(compiler::CompileOutcomes::into_single_report)
        .expect("the requirement-spelled FMA fixture produces a retained Terminal product");
    let retained = report
        .into_retained_terminal_artifact()
        .expect("the product retains its Terminal artifact");
    let proposal = retained
        .native_realization_proposal()
        .expect("the product retains its native proposal");
    // The verdict machine's float compare is its own (operator-keyed) D29 row;
    // the FMA row is the one whose demand names the requirement declaration
    // (the retained product is its own compilation, so rows are matched by
    // identity text, not by this compilation's symbol handles).
    let scope = proposal.checked_boundary_operator_scope();
    let fma_demands = proposal
        .boundary_application_demands()
        .rows()
        .iter()
        .filter(|demand| {
            demand
                .requirement()
                .declaration()
                .canonical()
                .contains("F32::fused_multiply_add")
        })
        .collect::<Vec<_>>();
    let [demand] = fma_demands.as_slice() else {
        panic!("one D29 demand keyed on the requirement declaration: {scope:?}");
    };
    let fma_occurrences = scope
        .occurrences()
        .iter()
        .filter(|occurrence| occurrence.terminal_operation() == demand.terminal_operation())
        .collect::<Vec<_>>();
    let [occurrence] = fma_occurrences.as_slice() else {
        panic!("one checked-to-Terminal FMA occurrence for the demand");
    };
    assert_eq!(
        demand.application(),
        &boundary_applications::BoundaryApplication::Empty
    );
    let fma_realizations = proposal
        .boundary_application_realizations()
        .rows()
        .iter()
        .filter(|realization| realization.terminal_operation() == occurrence.terminal_operation())
        .collect::<Vec<_>>();
    let [realization] = fma_realizations.as_slice() else {
        panic!("one D29 realization companion for the FMA occurrence");
    };
    assert_eq!(
        realization.role(),
        boundary_applications::BoundaryApplicationRealizationRole::ExactCompilerIntrinsic,
    );
    let [fma_proposal] = proposal.ieee_float_fma_occurrences() else {
        panic!("one nearest-FMA occurrence proposal");
    };
    assert_eq!(
        fma_proposal.terminal_operation(),
        occurrence.terminal_operation()
    );
    let retained_plan =
        &proposal.selected_provider_plans().plans()[fma_proposal.provider_plan_index()];
    assert_eq!(
        retained_plan.name.as_str(),
        "macos_arm64::FloatNativeProvider::satisfies::F32::fused_multiply_add",
        "the target-qualified plan name keys on the requirement path"
    );
    assert_eq!(
        realization.selected_plan_digest(),
        retained_plan.identity_digest().as_bytes(),
        "the D29 realization and the FMA proposal name one selected plan"
    );

    // The direct native route now carries the FMA occurrence's plan custody
    // (Terminal production, D29 custody and coverage all rejoin the
    // requirement-keyed plan), and stops where every host still stops: the
    // Abstract-to-Target stage demands an admitted FMA settlement that native
    // realization does not yet derive from the direct route's proposal rows
    // (and only admits x86 deployment custody). The stop is the explicit
    // settlement demand, not a custody or planning decline.
    let build_dir = std::env::temp_dir().join(format!(
        "omega-named-requirement-fma-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let native = compile_rooted_canary_for_native_host(&canary, build_dir.clone());
    let _ = fs::remove_dir_all(&build_dir);
    match native {
        Ok(_) => {
            let output = Command::new(build_dir.join(executable_name()))
                .output()
                .expect("the requirement-spelled FMA canary should run");
            assert_eq!(
                output.status.code(),
                Some(70),
                "the selected FMA must execute natively with one rounding; stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Err(diagnostics) => {
            let [diagnostic] = diagnostics.as_slice() else {
                panic!("one exact native stop is expected: {diagnostics:#?}");
            };
            assert!(
                diagnostic.message.contains("MissingIeeeFloatFmaSettlement"),
                "the native leg stops only at the admitted-FMA settlement demand: {}",
                diagnostic.message
            );
        }
    }
}
