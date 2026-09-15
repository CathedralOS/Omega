//! Runtime-capable value binders realize as ordinary trailing scalar
//! parameters on one shared dynamic machine body, while statically known
//! arguments keep the closed substitution path. These tests publish Terminal
//! artifacts from checked Omega source, inspect the emitted call operations
//! and machine boundaries, and then replay the artifact in the terminal
//! interpreter at every fuel pause so the exact captured subject, its contract
//! obligations, and generic-to-generic forwarding are witnessed on the
//! published representation rather than only at check time.

use compiler::{CompileOptions, CompileRequest, RequestedCompileProduct, compile};
use semantic_vocabulary::{MachineId, ValueId};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue, TerminalStructuralValue,
};
use terminal_psi::{OperationKind, TerminalModule};

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

struct Published {
    // The fixture directory must outlive every use of the retained bytes.
    _fixture: Fixture,
    semantic: Vec<u8>,
    proof: Vec<u8>,
    module: TerminalModule,
}

fn publish(name: &str, source: &str) -> Published {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "omega-runtime-value-generics-{name}-{}-{stamp}",
        std::process::id(),
    )));
    fs::create_dir(&fixture.0).unwrap();
    let main = fixture.0.join("main.omg");
    fs::write(&main, source).unwrap();
    fs::write(
        fixture.0.join("build.omg"),
        r#"
machine build(builder: &mut Build) {
    builder.application("runtime-value-generics");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
}
"#,
    )
    .unwrap();
    let request = CompileRequest::new(CompileOptions {
        root_path: main,
        build_dir: Some(fixture.0.join("build")),
        target_name: Some("linux_x86_64".to_owned()),
    })
    .with_requested_product(RequestedCompileProduct::TerminalArtifact);
    let report = compile(request)
        .and_then(compiler::CompileOutcomes::into_single_report)
        .unwrap_or_else(|diagnostics| {
            panic!(
                "runtime value generic publication failed; artifacts at {}:\n{}",
                fixture.0.display(),
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
        });
    let retained = report
        .into_retained_terminal_artifact()
        .expect("Terminal product");
    let artifact = retained.artifact();
    Published {
        semantic: artifact.semantic_bytes().to_vec(),
        proof: artifact.proof_bytes().to_vec(),
        module: terminal_codec::decode_module(artifact.semantic_bytes()).unwrap(),
        _fixture: fixture,
    }
}

/// Collect every in-module `Call` operation across all of a machine's blocks
/// as `(callee, arguments, obligation count)` rows in authored order.
fn calls(module: &TerminalModule, machine: MachineId) -> Vec<(MachineId, Vec<ValueId>, usize)> {
    module
        .machines
        .iter()
        .find(|candidate| candidate.id == machine)
        .unwrap()
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            OperationKind::Call {
                callee,
                arguments,
                requirement_obligations,
                ..
            } => Some((*callee, arguments.clone(), requirement_obligations.len())),
            _ => None,
        })
        .collect()
}

#[test]
fn runtime_bound_arguments_share_one_dynamic_body() {
    let published = publish(
        "shared-body",
        r#"
use omega::language::core::external_binding;

boundary trait Trace { machine record(value: u64); }
linux_x86_64 machine trace_leaf(value: u64) satisfies Trace::record via Binding::Syscall(1);

data Main {
    values: [u32; 8];
}

machine prefix_count<Count: u32>(base: u32) -> u32 {
    Count
}

machine Main::main(&mut self) reaches Trace {
    let n: u32 = 3;
    let m: u32 = 4;
    let first: u32 = prefix_count<n>(7);
    Trace::record(first as u64);
    let second: u32 = prefix_count<m>(8);
    Trace::record(second as u64);
    let closed: u32 = prefix_count<4>(9);
    Trace::record(closed as u64);
}
"#,
    );
    let module = &published.module;
    let calls = calls(module, module.entry);
    assert_eq!(
        calls.len(),
        3,
        "entry places exactly the three authored calls"
    );
    let dynamic_callee = calls[0].0;
    assert_eq!(
        calls[1].0, dynamic_callee,
        "distinct runtime subjects reuse the one dynamic body"
    );
    assert_ne!(
        calls[2].0, dynamic_callee,
        "the statically known argument keeps a closed specialization"
    );
    assert_eq!(
        calls[0].1.len(),
        2,
        "runtime calls carry the authored argument and the captured subject"
    );
    assert_eq!(
        calls[1].1.len(),
        2,
        "the second runtime call passes its own subject independently"
    );
    assert_eq!(
        calls[2].1.len(),
        1,
        "the static call keeps only the authored argument"
    );
    let dynamic = module
        .machines
        .iter()
        .find(|machine| machine.id == dynamic_callee)
        .unwrap();
    assert_eq!(
        dynamic.parameters.len(),
        2,
        "the dynamic body takes one authored parameter plus the binder subject"
    );
    assert_eq!(
        module.machines.len(),
        3,
        "no per-value body is emitted: entry, one dynamic body, one static body"
    );

    replay(
        &published,
        &[3, 4, 4],
        "each body returns its exact captured subject",
    );
}

#[test]
fn runtime_bound_requirements_stay_explicit_and_forward() {
    let published = publish(
        "forwarding",
        r#"
use omega::language::core::external_binding;

boundary trait Trace { machine record(value: u64); }
linux_x86_64 machine trace_leaf(value: u64) satisfies Trace::record via Binding::Syscall(1);

data Main {
    values: [u32; 8];
}

machine bounded<Count: u32>(base: u32) -> u32
requires
    Count <= 10;
{
    Count
}

machine forward_bounded<K: u32>(base: u32) -> u32
requires
    K <= 10;
{
    let inner: u32 = bounded<K>(base);
    inner
}

machine Main::main(&mut self) reaches Trace {
    let n: u32 = 3;
    let obliged: u32 = bounded<n>(2);
    Trace::record(obliged as u64);
    let chained: u32 = forward_bounded<n>(3);
    Trace::record(chained as u64);
    let mut source: u32 = 4;
    source = 6;
    let current: u32 = bounded<source>(0);
    Trace::record(current as u64);
}
"#,
    );
    let module = &published.module;
    let entry_calls = calls(module, module.entry);
    assert_eq!(entry_calls.len(), 3);
    let bounded_callee = entry_calls[0].0;
    let forward_callee = entry_calls[1].0;
    assert_eq!(
        entry_calls[2].0, bounded_callee,
        "the reassigned source's current subject reaches the same dynamic body"
    );
    assert_ne!(
        forward_callee, bounded_callee,
        "the generic caller is its own dynamic body"
    );
    for (index, (_, _, obligations)) in entry_calls.iter().enumerate() {
        assert_eq!(
            *obligations, 1,
            "call {index} owes the bounded requirement on its own subject"
        );
    }
    let forward = module
        .machines
        .iter()
        .find(|machine| machine.id == forward_callee)
        .unwrap();
    let inner_calls = calls(module, forward_callee);
    assert_eq!(inner_calls.len(), 1, "forward_bounded calls bounded once");
    let (inner_callee, inner_arguments, inner_obligations) = &inner_calls[0];
    assert_eq!(*inner_callee, bounded_callee);
    assert_eq!(
        *inner_obligations, 1,
        "the forwarded call re-owes the requirement on the forwarded subject"
    );
    let parameter_ids: Vec<ValueId> = forward
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect();
    for argument in inner_arguments {
        assert!(
            parameter_ids.contains(argument),
            "the forwarded call passes the generic caller's own parameter values"
        );
    }
    assert_eq!(
        module.machines.len(),
        3,
        "entry plus one dynamic body per template, no per-value bodies"
    );

    replay(
        &published,
        &[3, 3, 6],
        "bounded returns the captured subject; reassignment captures the current value",
    );
}

#[test]
fn runtime_bound_subject_keeps_its_transition_guard_proof() {
    let published = publish(
        "guarded",
        r#"
use omega::language::core::external_binding;

boundary trait Trace { machine record(value: u64); }
linux_x86_64 machine trace_leaf(value: u64) satisfies Trace::record via Binding::Syscall(1);

data Main {
    values: [u32; 8];
}

machine bounded<Count: u32>(base: u32) -> u32
requires
    Count <= 10;
{
    Count
}

machine Main::main(&mut self) reaches Trace {
    let n: u32 = 3;
    transition n <= 10 {
        true -> allowed(n)
        false -> denied()
    }
    state allowed(&mut self, k: u32) {
        let guarded: u32 = bounded<k>(5);
        Trace::record(guarded as u64);
    }
    state denied(&mut self) {}
}
"#,
    );
    let module = &published.module;
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert_eq!(module.machines.len(), 2, "composed entry plus bounded body");
    let mut guarded_calls = 0usize;
    let mut guard_comparisons = 0usize;
    for operation in entry.blocks.iter().flat_map(|block| &block.operations) {
        match &operation.kind {
            OperationKind::Call {
                callee,
                requirement_obligations,
                ..
            } => {
                let callee_machine = module
                    .machines
                    .iter()
                    .find(|machine| machine.id == *callee)
                    .unwrap();
                assert_eq!(
                    callee_machine.parameters.len(),
                    2,
                    "the guarded call still targets the two-parameter dynamic body"
                );
                assert_eq!(
                    requirement_obligations.len(),
                    1,
                    "the dominating guard discharges but does not erase the obligation"
                );
                guarded_calls += 1;
            }
            OperationKind::IntegerLessOrEqual { .. } => guard_comparisons += 1,
            _ => {}
        }
    }
    assert_eq!(
        guard_comparisons, 1,
        "the transition guard survives as a comparison"
    );
    assert_eq!(guarded_calls, 1, "allowed places exactly one bounded call");

    replay(
        &published,
        &[3],
        "the guard-satisfied state observes the captured subject",
    );
}

/// Start the published artifact with the entry's declared structural
/// arguments, run it to completion, and then replay it at every fuel split,
/// asserting the same observed trace every time.
fn replay(published: &Published, expected: &[u64], context: &str) {
    let entry = published
        .module
        .machines
        .iter()
        .find(|machine| machine.id == published.module.entry)
        .unwrap();
    let structural_arguments: Vec<TerminalStructuralValue> = entry
        .structural_parameters
        .iter()
        .map(|parameter| TerminalStructuralValue {
            opaque_identity: 1,
            structural_type: parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        })
        .collect();
    let execute = || {
        TerminalExecution::start_artifact_with_structural_arguments(
            &published.semantic,
            &published.proof,
            &proof_admission::AdmissionProfile::default(),
            &[],
            &structural_arguments,
        )
        .expect("published artifact independently verifies and reloads")
    };
    let complete = TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit);
    let mut execution = execute();
    let mut meter = TerminalFuelMeter::with_allowance(1000);
    let mut handler = Trace::default();
    assert_eq!(
        execution
            .resume_with_effect_handler(&mut meter, &mut handler)
            .unwrap(),
        complete
    );
    assert_eq!(handler.0, expected, "{context}");
    let total = meter.usage().total_units();
    for allowance in 0..total {
        let mut execution = execute();
        let mut meter = TerminalFuelMeter::with_allowance(allowance);
        let mut handler = Trace::default();
        assert!(matches!(
            execution
                .resume_with_effect_handler(&mut meter, &mut handler)
                .unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        meter.replenish(total - allowance).unwrap();
        assert_eq!(
            execution
                .resume_with_effect_handler(&mut meter, &mut handler)
                .unwrap(),
            complete
        );
        assert_eq!(handler.0, expected, "{context} at fuel split {allowance}");
        assert_eq!(meter.usage().total_units(), total);
    }
}

#[derive(Default)]
struct Trace(Vec<u64>);

impl TerminalEffectHandler for Trace {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
            panic!("only the authored trace boundary is observable")
        };
        let [
            TerminalScalarValue::Integer {
                value: semantic_vocabulary::IntegerValue::Unsigned(value),
                ..
            },
        ] = arguments.as_slice()
        else {
            panic!("trace retains its exact unsigned argument")
        };
        self.0.push(u64::try_from(*value).unwrap());
        Ok(())
    }
}
