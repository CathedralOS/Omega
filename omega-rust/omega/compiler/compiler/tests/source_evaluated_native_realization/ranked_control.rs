//! A normalized foreign call inside a ranked machine (`terminates by`):
//! physical-evidence derivation itself is cycle-agnostic — its
//! survivor/physical-child bijection is occurrence-coordinate keyed — and
//! legalization now replays the call's source custody, so the surviving
//! boundary occurrence reaches native image emission. There the hosted
//! receiver bridge still refuses the ranked boundary call's contract, storage,
//! or entry custody, upstream of physical evidence. This test pins that exact
//! frontier: once the hosted entry preparation replays boundary calls inside
//! ranked control, this test must flip to the full survivor/child assertions
//! the acyclic cases already carry.

use super::{
    Fixture, TestProviderExecution, admit_import, realize_linux_dynamic_outcome,
    terminal_authority_permission_policy, terminal_authority_policy,
};
use compiler::{
    RetainedNativeRealizationRequest, SourceEvaluatedImportSettlement,
    realize_retained_native_artifact,
};
use task_plans::SameStackContributionAdmissionReceiptId;

/// `Main::spin` is a measured self cycle: `terminates by` admits it and the
/// Terminal machine retains its ranked SCC decomposition. The foreign call in
/// its leading block is a surviving boundary occurrence inside ranked
/// control.
fn ranked_foreign_caller(name: &str) -> Fixture {
    Fixture::with_source(
        name,
        "linux_x86_64",
        r#"use omega::language::core::external_binding;

boundary trait Trace {
    machine record(value: u64);
}

linux_x86_64 machine record_binding() -> ForeignBinding<9, 6, 11> {
    ForeignBinding::DllImport {
        import: DllImport::ElfVersioned {
            object: "libc.so.6",
            symbol: "getpid",
            version: "GLIBC_2.2.5",
        },
    }
}

machine record_leaf(value: u64) satisfies Trace::record via record_binding();

data Main { }
machine Main::spin(&mut self, n: u64) terminates by n; reaches Trace {
    Trace::record(7);
    transition n == 0 {
        true -> done()
        _ -> self.spin(n - 1)
    }
    state done(&mut self) { }
}
machine Main::main(&mut self) {
    self.spin(3);
}
"#,
        r#"machine build(builder: &mut Build) {
    builder.application("ranked-foreign-caller");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
}
"#,
    )
}

#[test]
fn ranked_machine_foreign_call_stops_at_hosted_entry_preparation() {
    let fixture = ranked_foreign_caller("ranked-foreign-caller");
    let retained = fixture.compile_terminal();
    let module = terminal_codec::decode_module(retained.artifact().semantic_bytes())
        .expect("decode terminal module");
    let ranked_machines = module
        .machines
        .iter()
        .filter(|machine| machine.ranked_scc.is_some())
        .collect::<Vec<_>>();
    let [ranked_machine] = ranked_machines.as_slice() else {
        panic!("the fixture retains exactly one ranked machine")
    };
    let ranked_call = ranked_machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::BoundaryCall { .. } => Some(operation.id),
            _ => None,
        })
        .expect("the ranked machine carries the foreign boundary call");

    let (_, diagnostics) = realize_linux_dynamic_outcome(retained, 0x5241_4e4b_0001).expect_err(
        "a foreign call inside a ranked machine must still stop upstream of physical evidence",
    );
    let rendered = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("hosted receiver bridge lost exact contract, storage, or entry custody"),
        "the foreign call at machine {} operation {ranked_call} must stop at hosted entry preparation, not deeper: {rendered}",
        ranked_machine.id
    );
}

/// The surviving boundary occurrence inside ranked control still passes the
/// retained-custody rejoin before it reaches the hosted-entry stop: a
/// substituted provider binding must reject there rather than ride the
/// occurrence to entry preparation. A forged requirement identity is not a
/// demanded selected import, and a forged plan-report coordinate fails the
/// exact-plan join; every rejection returns the consumed dynamic interpreter
/// custody intact.
#[test]
fn ranked_machine_foreign_call_rejects_substituted_provider_bindings() {
    #[derive(Clone, Copy)]
    enum ForgedBinding {
        RequirementIdentity,
        PlanReportCoordinate,
    }

    let fixture = ranked_foreign_caller("ranked-foreign-caller-substituted");
    let expected_interpreter = target::normalize_elf_interpreter_plan(
        b"/lib64/ld-linux-x86-64.so.2".to_vec(),
        target::TargetProfile::LinuxX64,
    )
    .expect("canonical Linux x86-64 interpreter");

    for (receipt_identity, forged) in [
        (0x5241_4e4b_0010, ForgedBinding::RequirementIdentity),
        (0x5241_4e4b_0011, ForgedBinding::PlanReportCoordinate),
    ] {
        let retained = fixture.compile_terminal();
        let admission = admit_import(
            &retained,
            SameStackContributionAdmissionReceiptId::from_normalized_identity(receipt_identity)
                .unwrap(),
        );
        let forged_execution = TestProviderExecution {
            requirement: match forged {
                ForgedBinding::RequirementIdentity => {
                    format!("{}::substituted", admission.execution.requirement)
                }
                ForgedBinding::PlanReportCoordinate => admission.execution.requirement.clone(),
            },
            plan_report_identity: admission.plan_report_identity
                + u64::from(matches!(forged, ForgedBinding::PlanReportCoordinate)),
        };
        let policy = terminal_authority_policy(&retained);
        let permission_policy = terminal_authority_permission_policy(&retained);
        let (image_request, diagnostics) = realize_retained_native_artifact(
            retained,
            RetainedNativeRealizationRequest {
                profile: &proof_admission::AdmissionProfile::default(),
                optimization_selections:
                    &optimization_core::PostTerminalOptimizationSelections::default(),
                terminal_authority_policy: policy,
                accepted_package_terminal_authority_permission_policy:
                    native_realization::current_terminal_authority_permission_policy(),
                terminal_authority_permission_policy: Some(permission_policy),
                image_request: native_realization::ExecutableImageEmissionRequest::dynamic_elf(
                    expected_interpreter.clone(),
                ),
                imports: &[SourceEvaluatedImportSettlement::new(
                    &forged_execution,
                    &admission.same_stack,
                )],
            },
        )
        .expect_err("a substituted provider binding must not reach native custody");
        let native_realization::ExecutableImageEmissionRequest::DynamicElf { interpreter } =
            image_request
        else {
            panic!("the rejection must return the dynamic interpreter custody")
        };
        assert_eq!(interpreter, expected_interpreter);
        let rendered = diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        let expected = match forged {
            ForgedBinding::RequirementIdentity => "is not a demanded selected import",
            ForgedBinding::PlanReportCoordinate => {
                "names a different provider-plan report coordinate"
            }
        };
        assert!(
            rendered.contains(expected),
            "the substituted binding must name its exact rejection: {rendered}"
        );
        assert!(
            !rendered.contains("hosted receiver bridge"),
            "the substituted binding must stop at the custody rejoin, upstream of hosted entry preparation: {rendered}"
        );
    }
}
