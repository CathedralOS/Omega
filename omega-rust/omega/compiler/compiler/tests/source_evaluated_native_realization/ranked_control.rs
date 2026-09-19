//! A normalized foreign call inside a ranked machine (`terminates by`):
//! physical-evidence derivation itself no longer blocks ranked control — its
//! survivor/physical-child bijection is occurrence-coordinate keyed and
//! cycle-agnostic — but the call's normalized-import transport still stops in
//! legalization's source-custody replay, upstream of native emission. This
//! test pins that exact frontier: once the emit slice replays source custody
//! for boundary calls inside ranked control, this test must flip to the full
//! survivor/child assertions the acyclic cases already carry.

use super::{
    Fixture, admit_import, terminal_authority_permission_policy, terminal_authority_policy,
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
#[test]
fn ranked_machine_foreign_call_stops_at_legalization_source_custody() {
    let fixture = Fixture::with_source(
        "ranked-foreign-caller",
        "linux_x86_64",
        r#"use omega::language::core::external_binding;

boundary trait Trace {
    machine record(value: u64);
}

linux_x86_64 machine record_binding() -> Binding<9, 6, 11> {
    Binding::DllImport {
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
    );
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

    let admission = admit_import(
        &retained,
        SameStackContributionAdmissionReceiptId::from_normalized_identity(0x5241_4e4b_0001)
            .unwrap(),
    );
    let policy = terminal_authority_policy(&retained);
    let permission_policy = terminal_authority_permission_policy(&retained);
    let image_request = native_realization::ExecutableImageEmissionRequest::direct(
        retained
            .native_realization_proposal()
            .expect("native proposal")
            .subsystem(),
    );
    let diagnostics = realize_retained_native_artifact(
        retained,
        RetainedNativeRealizationRequest {
            profile: &proof_admission::AdmissionProfile::default(),
            optimization_selections:
                &optimization_core::PostTerminalOptimizationSelections::default(),
            terminal_authority_policy: policy,
            accepted_package_terminal_authority_permission_policy:
                native_realization::current_terminal_authority_permission_policy(),
            terminal_authority_permission_policy: Some(permission_policy),
            image_request,
            imports: &[SourceEvaluatedImportSettlement::new(
                &admission.execution,
                &admission.same_stack,
            )],
        },
    )
    .map(|_| ())
    .expect_err(
        "a foreign call inside a ranked machine must still stop upstream of physical evidence",
    )
    .1;
    let rendered = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("SourceCustodyMismatch"),
        "the foreign call at machine {} operation {ranked_call} must stop at legalization source custody, not deeper: {rendered}",
        ranked_machine.id
    );
}
