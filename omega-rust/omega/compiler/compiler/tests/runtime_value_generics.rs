//! Runtime-capable value binders realize as ordinary trailing scalar
//! parameters on one shared dynamic machine body, while statically known
//! arguments keep the closed substitution path. These tests publish Terminal
//! artifacts from checked Omega source, inspect the emitted call operations
//! and machine boundaries, and then replay the artifact in the terminal
//! interpreter at every fuel pause so the exact captured subject, its contract
//! obligations, and generic-to-generic forwarding are witnessed on the
//! published representation rather than only at check time. The rejection
//! half is witnessed against the checked fail-corpus fixtures: invalid bounds,
//! duplicated or lost linear custody, and static-only uses of a runtime-bound
//! binder must keep their named diagnostics.

use compiler::{CompileOptions, CompileRequest, RequestedCompileProduct, compile};
use semantic_vocabulary::{MachineId, ValueId};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralByteArrayValue, TerminalStructuralValue,
};
use terminal_psi::{OperationKind, StructuralPathSegment, TerminalModule};

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

/// Collect every in-module direct call operation across all of a machine's
/// blocks as `(callee, scalar arguments, obligation count)` rows in authored
/// order. `CallUnit`/`CallStructuralScalar` carry the receiver and claims on
/// their own lanes; the scalar tuple stays directly comparable.
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
            }
            | OperationKind::CallUnit {
                callee,
                arguments,
                requirement_obligations,
                ..
            }
            | OperationKind::CallStructuralScalar {
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
        &[],
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
        &[],
    );
}

/// A result qualification names the caller's captured subject, not the
/// template binder: `-> u64[0..=Bound]` specializes to a range on the
/// realized trailing parameter, the caller's inferred result indexes on the
/// argument itself (`u64[0..=n]`), and a declared local bound naming that
/// argument discharges the call. A reassigned source binds its current value
/// and the closed literal keeps its own specialization.
#[test]
fn runtime_bound_result_range_stays_with_the_captured_subject() {
    let published = publish(
        "result-range",
        r#"
use omega::language::core::external_binding;

boundary trait Trace { machine record(value: u64); }
linux_x86_64 machine trace_leaf(value: u64) satisfies Trace::record via Binding::Syscall(1);

data Main {
    values: [u32; 8];
}

machine ranged<Bound: u64>() -> u64[0..=Bound] {
    Bound
}

machine Main::main(&mut self) reaches Trace {
    let n: u64 = 7;
    let captured: u64[0..=n] = ranged<n>();
    Trace::record(captured);
    let mut source: u64 = 3;
    source = 5;
    let current: u64 = ranged<source>();
    Trace::record(current);
    let closed: u64 = ranged<4>();
    Trace::record(closed);
}
"#,
    );
    let module = &published.module;
    let entry_calls = calls(module, module.entry);
    assert_eq!(
        entry_calls.len(),
        3,
        "entry places the two runtime calls and the closed call"
    );
    assert_eq!(
        entry_calls[0].0, entry_calls[1].0,
        "distinct runtime subjects reuse the one dynamic body"
    );
    assert_ne!(
        entry_calls[0].0, entry_calls[2].0,
        "the closed literal keeps its own specialization"
    );
    assert_eq!(
        module.machines.len(),
        3,
        "entry plus one dynamic body and one closed body"
    );

    replay(
        &published,
        &[7, 5, 4],
        "each body returns its own captured subject; the bound rides the index",
        &[],
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
        &[],
    );
}

/// Collect every in-module receiver call across all of a machine's blocks as
/// `(callee, scalar argument count, structural argument count, obligation
/// count)` rows in authored order. `Main::` receiver methods lower to the
/// structural-argument call variants, so this walks the `Call`, `CallUnit`,
/// and `CallStructuralScalar` shapes together.
fn receiver_calls(
    module: &TerminalModule,
    machine: MachineId,
) -> Vec<(MachineId, usize, usize, usize)> {
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
            } => Some((*callee, arguments.len(), 0, requirement_obligations.len())),
            OperationKind::CallUnit {
                callee,
                arguments,
                structural_arguments,
                requirement_obligations,
                ..
            } => Some((
                *callee,
                arguments.len(),
                structural_arguments.len(),
                requirement_obligations.len(),
            )),
            OperationKind::CallStructuralScalar {
                callee,
                arguments,
                structural_arguments,
                requirement_obligations,
                ..
            } => Some((
                *callee,
                arguments.len(),
                structural_arguments.len(),
                requirement_obligations.len(),
            )),
            _ => None,
        })
        .collect()
}

/// Non-generic control for the indexed-field carriers below: an attached
/// fixed-array element already reads and writes at a literal index through the
/// ordinary primitive-store/read operations, so a later generic body's use of
/// that path is attributable to the value binder rather than to the field
/// store itself. The element type is `u8` because the interpreter only admits
/// host-initialized fixed-array backing for true byte arrays; wider element
/// arrays still have no entry-storage input. Runtime-indexed element access
/// (`self.values[n]`) is the STATE-LOCAL-VALUE-FRONTIER slice: Terminal keeps
/// only exact static path segments, so a runtime index has no primitive
/// element operation yet.
#[test]
fn indexed_scalar_field_carries_the_current_runtime_value() {
    let published = publish(
        "indexed-field",
        r#"
use omega::language::core::external_binding;

boundary trait Trace { machine record(value: u64); }
linux_x86_64 machine trace_leaf(value: u64) satisfies Trace::record via Binding::Syscall(1);

data Main {
    values: [u8; 8];
}

machine Main::main(&mut self) reaches Trace {
    let mut source: u8 = 4;
    source = 5;
    self.values[3] = source;
    let first: u8 = self.values[3];
    Trace::record(first as u64);
}
"#,
    );
    replay(
        &published,
        &[5],
        "the indexed field element stores and returns the current value",
        &array_backing(&published, 8),
    );
}

/// The captured subject flows through an indexed scalar field: `put` writes
/// its realized `Count` argument into the literal-indexed element and `at`
/// reads that element back, so each call's own subject is what the field
/// preserves. `put` also stores its authored `v` at a second index, keeping
/// the appended subject distinct from ordinary arguments in the emitted call.
/// The element and binder types are `u8` because host-initialized
/// fixed-array backing is only admitted for byte arrays. Indexing the element
/// by `Count` itself remains the STATE-LOCAL-VALUE-FRONTIER slice (no
/// dynamic-index primitive operation exists in Terminal); this covers the
/// subject-through-indexed-field half on the supported surface.
#[test]
fn runtime_bound_subject_flows_through_indexed_field_writes() {
    let published = publish(
        "indexed-field-generic",
        r#"
use omega::language::core::external_binding;

boundary trait Trace { machine record(value: u64); }
linux_x86_64 machine trace_leaf(value: u64) satisfies Trace::record via Binding::Syscall(1);

data Main {
    values: [u8; 8];
}

machine Main::at<Count: u8>(&self) -> u8
requires
    Count <= 7;
{
    self.values[3]
}

machine Main::put<Count: u8>(&mut self, v: u8)
requires
    Count <= 7;
{
    self.values[3] = Count;
    self.values[4] = v;
}

machine Main::main(&mut self) reaches Trace {
    let n: u8 = 3;
    let mut source: u8 = 4;
    source = 5;
    self.put<n>(20);
    let first: u8 = self.at<n>();
    Trace::record(first as u64);
    self.put<source>(30);
    let second: u8 = self.at<source>();
    Trace::record(second as u64);
    let stored_arg: u8 = self.values[4];
    Trace::record(stored_arg as u64);
}
"#,
    );
    let module = &published.module;
    let entry_calls = receiver_calls(module, module.entry);
    assert_eq!(
        entry_calls.len(),
        4,
        "entry interleaves the two writes and two reads; Trace::record stays a boundary call"
    );
    let put_callee = entry_calls[0].0;
    let at_callee = entry_calls[1].0;
    assert_eq!(
        entry_calls[2].0, put_callee,
        "the second write's distinct runtime subject reuses the same dynamic body"
    );
    assert_eq!(
        entry_calls[3].0, at_callee,
        "the second read's distinct runtime subject reuses the same dynamic body"
    );
    assert_ne!(
        at_callee, put_callee,
        "the read body is its own specialization, not the write body"
    );
    for (index, (_, scalars, structurals, obligations)) in entry_calls.iter().enumerate() {
        assert_eq!(
            (*structurals, *obligations),
            (1, 1),
            "call {index} carries the receiver and owes the bound on its own subject"
        );
        assert_eq!(
            *scalars,
            if index % 2 == 0 { 2 } else { 1 },
            "call {index} appends its exact captured subject after any authored scalar"
        );
    }
    assert_eq!(
        module.machines.len(),
        3,
        "entry plus one dynamic body per receiver method, no per-value bodies"
    );

    replay(
        &published,
        &[3, 5, 30],
        "each read returns the captured subject its preceding write stored",
        &array_backing(&published, 8),
    );
}

/// A multi-state template cloned for a runtime `Value` subject carries the
/// realized subject as a trailing parameter on every cloned state, so a `->`
/// transition between them owes the target that appended subject exactly as a
/// rewritten call site does. `allowed` stores its forwarded subject into one
/// literal-indexed field slot; `denied` receives its own forwarded subject and
/// stores it into another. The second call's distinct runtime subject reuses
/// the same dynamic body, and the whole chain replays through Terminal.
#[test]
fn runtime_bound_subject_survives_state_transitions() {
    let published = publish(
        "transitioned-subject",
        r#"
use omega::language::core::external_binding;

boundary trait Trace { machine record(value: u64); }
linux_x86_64 machine trace_leaf(value: u64) satisfies Trace::record via Binding::Syscall(1);

data Main {
    values: [u8; 8];
}

machine Main::walk<Count: u8>(&mut self, n: u8) {
    transition n == 3 {
        true -> allowed(n)
        false -> denied()
    }
    state allowed(&mut self, n: u8) {
        self.values[3] = Count;
    }
    state denied(&mut self) {
        self.values[4] = Count;
    }
}

machine Main::main(&mut self) reaches Trace {
    let n: u8 = 3;
    self.walk<n>(n);
    let stored: u8 = self.values[3];
    Trace::record(stored as u64);
    let m: u8 = 4;
    self.walk<m>(m);
    let forwarded: u8 = self.values[4];
    Trace::record(forwarded as u64);
}
"#,
    );
    let module = &published.module;
    let entry_calls = calls(module, module.entry);
    assert_eq!(
        entry_calls.len(),
        2,
        "entry places the two `walk` calls; Trace::record stays a boundary call"
    );
    assert_eq!(
        entry_calls[0].0, entry_calls[1].0,
        "distinct runtime subjects share the one transitioned dynamic body"
    );
    assert_eq!(
        entry_calls[0].1.len(),
        2,
        "the transitioned call carries the authored argument and the captured subject"
    );
    assert_eq!(
        module.machines.len(),
        2,
        "entry plus one dynamic multi-state body, no per-value bodies"
    );

    replay(
        &published,
        &[3, 4],
        "each transition arm stores its own forwarded subject where main observes it",
        &array_backing(&published, 8),
    );
}

/// The runtime-bound subject need not be scalar: a `Value` binder over a
/// `data` carrier realizes its trailing parameter as a structural argument, so
/// `take` binds the caller's `Token` by value and reads its `id`. Two distinct
/// subjects ride the one shared dynamic body, and each replays its own
/// captured record through the terminal interpreter. Linear-carrier custody
/// obligations — duplicated or dropped `V` subjects — are checked rejections
/// pinned by the fail-corpus fixtures below; checked-unit planning does not
/// yet admit `[linear]` data, so the positive direction is witnessed here on
/// the ordinary structural lane.
#[test]
fn runtime_bound_structural_subject_shares_one_dynamic_body() {
    let published = publish(
        "structural-subject",
        r#"
use omega::language::core::external_binding;

boundary trait Trace { machine record(value: u64); }
linux_x86_64 machine trace_leaf(value: u64) satisfies Trace::record via Binding::Syscall(1);

data Main {
    values: [u8; 8];
}

data Token {
    id: u32;
}

machine take<V: Token>() -> u32 {
    V.id
}

machine Main::main(&mut self) reaches Trace {
    let t: Token = Token { id: 7 };
    let first: u32 = take<t>();
    Trace::record(first as u64);
    let u: Token = Token { id: 8 };
    let second: u32 = take<u>();
    Trace::record(second as u64);
}
"#,
    );
    let module = &published.module;
    let entry_calls = receiver_calls(module, module.entry);
    assert_eq!(
        entry_calls.len(),
        2,
        "entry places exactly the two `take` calls; Trace::record stays a boundary call"
    );
    for (index, (_, scalars, structurals, _)) in entry_calls.iter().enumerate() {
        assert_eq!(
            (*scalars, *structurals),
            (0, 1),
            "call {index} carries only its captured subject, on the structural lane"
        );
    }
    assert_eq!(
        entry_calls[0].0, entry_calls[1].0,
        "distinct structural subjects share the one dynamic body"
    );
    let take = module
        .machines
        .iter()
        .find(|machine| machine.id == entry_calls[0].0)
        .unwrap();
    assert_eq!(
        take.structural_parameters.len(),
        1,
        "the dynamic body realizes `V` as its single structural parameter"
    );
    assert_eq!(
        module.machines.len(),
        2,
        "entry plus the dynamic `take` body, no per-value bodies"
    );

    // `main` never indexes `self.values`, so the receiver is dropped from the
    // entry's structural parameters and there is no array field to back.
    replay(
        &published,
        &[7, 8],
        "each call returns its own captured subject's id",
        &[],
    );
}

/// Compile a fail-corpus fixture through the ordinary check product and return
/// its rendered diagnostics. The corpus pair (`main.omg`, `expected.txt`)
/// pins the rejection surface each scenario must keep producing; the
/// canary-suite fail driver claims its own list, so this target drives the
/// value-binder rejections directly.
fn reject(path: &str) {
    let canary = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate should live under omega-rust/omega/compiler/compiler")
        .join("tests/omega/fail")
        .join(path);
    let expected = fs::read_to_string(canary.join("expected.txt"))
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", canary.display()))
        .trim()
        .to_owned();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "omega-runtime-value-generics-reject-{}-{stamp}",
        std::process::id(),
    )));
    fs::create_dir(&fixture.0).unwrap();
    let diagnostics = compile(CompileRequest::new(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(fixture.0.join("build")),
        target_name: Some("linux_x86_64".to_owned()),
    }))
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect_err("runtime value binder misuse should be rejected");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains(&expected),
        "{} missing expected fragment {expected:?}:\n{combined}",
        canary.display(),
    );
}

/// The remaining rejection surface of the runtime/value-binder contract:
/// invalid bounds on the declared carrier or `requires` bound, duplicated or
/// dropped linear custody of the bound subject, and static-only uses of a
/// runtime-bound binder in array extents and nested static arguments.
#[test]
fn runtime_value_generic_rejections_keep_their_diagnostics() {
    for path in [
        "generics/value_generic_static_argument_out_of_range",
        "generics/value_generic_runtime_subject_out_of_range",
        "generics/value_generic_static_requires_violation",
        "generics/value_generic_linear_subject_duplicated",
        "generics/value_generic_linear_subject_dropped",
        "generics/value_generic_runtime_static_length",
        "generics/value_generic_static_nested_argument",
    ] {
        reject(path);
    }
}

/// Exact initialized backing for the entry receiver's sole fixed byte-array
/// field, supplied as opaque host contents so the interpreter's primitive
/// store/read operations have established storage to replace and observe.
fn array_backing(published: &Published, length: usize) -> Vec<TerminalStructuralByteArrayValue> {
    let entry = published
        .module
        .machines
        .iter()
        .find(|machine| machine.id == published.module.entry)
        .unwrap();
    let [parameter] = entry.structural_parameters.as_slice() else {
        panic!("entry retains exactly the receiver structural parameter");
    };
    let record = published
        .module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == parameter.structural_type)
        .unwrap();
    let terminal_psi::StructuralTypeShape::Record { fields } = &record.shape else {
        panic!("receiver retains its authored record shape");
    };
    let field = fields
        .iter()
        .find(|field| {
            let terminal_psi::StructuralFieldType::Structural(child) = field.field_type else {
                return false;
            };
            published.module.structural_types.iter().any(|declaration| {
                declaration.id == child
                    && matches!(
                        declaration.shape,
                        terminal_psi::StructuralTypeShape::FixedArray { .. }
                    )
            })
        })
        .unwrap_or_else(|| panic!("receiver retains its authored array field"));
    vec![TerminalStructuralByteArrayValue {
        argument_index: 0,
        path: vec![StructuralPathSegment::Field(field.identity.clone())],
        bytes: vec![0; length],
    }]
}

/// Start the published artifact with the entry's declared structural
/// arguments and any host-initialized byte-array backing, run it to
/// completion, and then replay it at every fuel split, asserting the same
/// observed trace every time.
fn replay(
    published: &Published,
    expected: &[u64],
    context: &str,
    byte_arrays: &[TerminalStructuralByteArrayValue],
) {
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
        TerminalExecution::start_artifact(
            &published.semantic,
            &published.proof,
            &proof_admission::AdmissionProfile::default(),
            &[],
            TerminalStructuralInputs {
                arguments: &structural_arguments,
                byte_arrays,
                ..Default::default()
            },
        )
        .expect("published artifact independently verifies and reloads")
    };
    let complete = TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit);
    let mut execution = execute();
    let mut meter = TerminalFuelMeter::with_allowance(1000);
    let mut handler = Trace::default();
    assert_eq!(
        execution.resume(&mut meter, &mut handler).unwrap(),
        complete
    );
    assert_eq!(handler.0, expected, "{context}");
    let total = meter.usage().total_units();
    for allowance in 0..total {
        let mut execution = execute();
        let mut meter = TerminalFuelMeter::with_allowance(allowance);
        let mut handler = Trace::default();
        assert!(matches!(
            execution.resume(&mut meter, &mut handler).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        meter.replenish(total - allowance).unwrap();
        assert_eq!(
            execution.resume(&mut meter, &mut handler).unwrap(),
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

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[path = "support/console_acceptance.rs"]
mod console_acceptance;
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[path = "support/macos_entry_acceptance.rs"]
mod macos_entry_acceptance;

/// The native leg: the same runtime-bound subjects execute on the macOS ARM64
/// host and surface through the process exit code, so the captured subject,
/// its forwarding, its guard-established requirement, its flow through an
/// indexed scalar field, and its transport across cloned state transitions
/// are witnessed on the realized machine code rather than only on the
/// interpreter. Fixtures spell the receiver `console: Service<Console> in
/// Bound` with an explicit provider selection: a bare `console: Console`
/// instance field has no Fused establishment row, so the hosted receiver
/// bridge rejects any entry that retains its receiver (states or attached
/// fields): a bare boundary-trait field rejects as a non-carrier under
/// `wiki/spec/build/entry_roots.md` "Entry shape and arrival bridge". Binder
/// carriers
/// are `i32`/`u8` because the exit code is an `i32` and native realization
/// admits their widening; `in Wrapping` retags keep the sums realizable.
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod native {
    use super::Fixture;
    use build_declarations::{BuildDeclaration, extract_build_declaration};
    use compiler::{
        CheckedCompileRequest, CompileOptions, CompileRequest, RequestedCompileProduct, compile,
        compile_to_checked,
    };
    use diagnostics::Diagnostic;
    use package_compilation::{
        PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
    };
    use semantic_vocabulary::PackageKeyIdentity;
    use std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(4)
            .expect("compiler crate should live under omega-rust/omega/compiler/compiler")
            .to_path_buf()
    }

    fn identity(marker: u8) -> PackageKeyIdentity {
        PackageKeyIdentity::from_digest([marker; 32]).expect("nonzero fixture identity")
    }

    fn render(diagnostics: &[Diagnostic]) -> String {
        diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The reviewed-fixture package route the canary suite uses for std
    /// fixtures: the standard library bound by repository path, the macOS
    /// program entry accepted from the checked target contract, and the exact
    /// Console `exit_process` provider plan accepted from a preliminary
    /// checked compile of the fixture itself.
    fn package_inputs(root: &Path) -> Result<PackageCompilationInputs, Vec<Diagnostic>> {
        let main = root.join("main.omg");
        let standard_library = repo_root().join("source/library/std");
        let declaration = extract_build_declaration(root)
            .unwrap_or_else(|error| panic!("fixture {}: {error}", root.display()));
        let root_role = declaration.kind();
        let root_name = match declaration {
            BuildDeclaration::Application(application) => application.name,
            BuildDeclaration::Package(package) => package.name,
            BuildDeclaration::Workspace(_) => panic!("native fixture cannot be a workspace"),
        };
        let root_identity = identity(1);
        let std_identity = identity(2);
        let inputs = PackageCompilationInputs::new(
            root_identity,
            root_role,
            vec![
                PackageSourceBinding::new(
                    root_identity,
                    root_name.into_string(),
                    root.to_path_buf(),
                ),
                PackageSourceBinding::new(
                    std_identity,
                    "omega-language-std",
                    standard_library.clone(),
                ),
            ],
            vec![PackageDependencyBinding::new(
                root_identity,
                "omega_language_std",
                std_identity,
            )],
        )
        .unwrap_or_else(|errors| panic!("native fixture inputs: {errors:#?}"));
        let mut bindings = vec![
            super::macos_entry_acceptance::candidate_macos_entry_binding(
                &standard_library,
                std_identity,
            )?,
        ];
        let inputs = inputs
            .with_accepted_semantic_bindings(bindings.clone())
            .map_err(|errors| vec![Diagnostic::error(format!("entry acceptance: {errors:?}"))])?;
        let preliminary = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(inputs.clone()),
            ..CheckedCompileRequest::new(&main, Some("macos_arm64"))
        })?;
        bindings.push(super::console_acceptance::candidate_console_exit_binding(
            &preliminary,
            std_identity,
            false,
            false,
        )?);
        inputs
            .with_accepted_semantic_bindings(bindings)
            .map_err(|errors| vec![Diagnostic::error(format!("console acceptance: {errors:?}"))])
    }

    /// Compile `source` as a macOS ARM64 application, publish the native
    /// artifact, run it, and compare the process exit code.
    fn run_native(name: &str, source: &str, expected_exit: i32) {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let fixture = Fixture(std::env::temp_dir().join(format!(
            "omega-runtime-value-generics-native-{name}-{}-{stamp}",
            std::process::id(),
        )));
        fs::create_dir(&fixture.0).unwrap();
        fs::write(fixture.0.join("main.omg"), source).unwrap();
        let standard_library = repo_root()
            .join("source/library/std")
            .to_string_lossy()
            .replace('\\', "/");
        fs::write(
            fixture.0.join("build.omg"),
            format!(
                "machine build(builder: &mut Build) {{\n    builder.application(\"runtime-value-generics-{name}\");\n    builder.depend(Source::Path {{ location: \"{standard_library}\" }});\n    builder.select_provider<Console, ConsoleNativeProvider>();\n    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);\n}}\n"
            ),
        )
        .unwrap();
        let inputs = package_inputs(&fixture.0).unwrap_or_else(|diagnostics| {
            panic!(
                "{name}: native fixture acceptance failed; artifacts at {}:\n{}",
                fixture.0.display(),
                render(&diagnostics)
            )
        });
        let permission_policy = native_realization::terminal_authority_permission_policy_with_rows(
            inputs
                .accepted_semantic_bindings()
                .flat_map(|binding| binding.terminal_authority_permissions())
                .cloned()
                .collect(),
        )
        .expect("accepted Console permissions form a valid policy");
        let build_dir = fixture.0.join("build");
        let request = CompileRequest::new(CompileOptions {
            root_path: fixture.0.join("main.omg"),
            build_dir: Some(build_dir.clone()),
            target_name: Some("macos_arm64".to_owned()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
        .with_terminal_authority_permission_policy(permission_policy)
        .with_package_inputs(inputs);
        let report = compile(request)
            .and_then(compiler::CompileOutcomes::into_single_report)
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "{name}: native compilation failed; artifacts at {}:\n{}",
                    fixture.0.display(),
                    render(&diagnostics)
                )
            })
            .publish_retained_native_artifact(&build_dir)
            .unwrap_or_else(|error| panic!("{name}: native publication failed: {error}"));
        let executable = report
            .checked_native_executable_path()
            .unwrap_or_else(|| panic!("{name}: no checked native executable receipt"))
            .to_path_buf();
        let output = Command::new(&executable)
            .output()
            .unwrap_or_else(|error| panic!("{name}: cannot run {}: {error}", executable.display()));
        assert_eq!(
            output.status.code(),
            Some(expected_exit),
            "{name}: native exit code; stderr:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// Two runtime subjects reach one dynamic body and the literal argument
    /// keeps its closed specialization: 3 + 4 + 4.
    #[test]
    fn runtime_bound_arguments_share_one_dynamic_body_natively() {
        run_native(
            "shared-body",
            r#"
use omega_language_std::console;
use omega::language::core::service;

data Main {
    console: Service<Console> in Bound;
}

machine prefix_count<Count: i32>(base: i32) -> i32 in Wrapping {
    Count as i32 in Wrapping
}

machine Main::main(&mut self) reaches Console {
    let n: i32 = 3;
    let m: i32 = 4;
    let first: i32 in Wrapping = prefix_count<n>(7);
    let second: i32 in Wrapping = prefix_count<m>(8);
    let closed: i32 in Wrapping = prefix_count<4>(9);
    let total: i32 in Wrapping = first + second + closed;
    self.console.exit_process(total as i32);
}
"#,
            11,
        );
    }

    const FORWARDING_SOURCE: &str = r#"
use omega_language_std::console;
use omega::language::core::service;

data Main {
    console: Service<Console> in Bound;
}

machine bounded<Count: i32>(base: i32) -> i32 in Wrapping
requires
    Count <= 10;
{
    Count as i32 in Wrapping
}

machine forward_bounded<K: i32>(base: i32) -> i32 in Wrapping
requires
    K <= 10;
{
    let inner: i32 in Wrapping = bounded<K>(base);
    inner
}

machine reassigned_source() -> i32 in Wrapping {
    let mut source: i32 = 4;
    source = 6;
    let current: i32 in Wrapping = bounded<source>(0);
    current
}

machine Main::main(&mut self) reaches Console {
    let n: i32 = 3;
    let obliged: i32 in Wrapping = bounded<n>(2);
    let chained: i32 in Wrapping = forward_bounded<n>(3);
    let current: i32 in Wrapping = reassigned_source();
    let total: i32 in Wrapping = obliged + chained + current;
    self.console.exit_process(total as i32);
}
"#;

    /// Each call owes its own requirement; forwarding and reassignment retain
    /// the captured runtime subject: 3 + 3 + 6.
    #[test]
    fn runtime_bound_requirements_stay_explicit_and_forward_natively() {
        run_native("forwarding", FORWARDING_SOURCE, 12);
    }

    #[test]
    fn runtime_bound_inline_mutable_subject_composes_with_provider_natively() {
        let source = FORWARDING_SOURCE.replace(
            "let current: i32 in Wrapping = reassigned_source();",
            "let mut source: i32 = 4; source = 6; let current: i32 in Wrapping = bounded<source>(0);",
        );
        run_native("inline-mutable-subject", &source, 12);
    }

    #[test]
    fn runtime_bound_distinct_mutable_locals_keep_captured_subjects_natively() {
        let source = FORWARDING_SOURCE.replace(
            "let current: i32 in Wrapping = reassigned_source();",
            "let mut source: i32 = 4;
             let saved: i32 in Wrapping = bounded<source>(0);
             source = 6;
             let mut other: i32 = 7; other = 8;
             let current: i32 in Wrapping = bounded<source>(0) + saved + bounded<other>(0);",
        );
        run_native("distinct-mutable-subjects", &source, 24);
    }

    /// A dominating transition guard establishes the requirement for the
    /// state's forwarded subject: the allowed state exits with it.
    #[test]
    fn runtime_bound_subject_keeps_its_transition_guard_proof_natively() {
        run_native(
            "guarded",
            r#"
use omega_language_std::console;
use omega::language::core::service;

data Main {
    console: Service<Console> in Bound;
}

machine bounded<Count: i32>(base: i32) -> i32 in Wrapping
requires
    Count <= 10;
{
    Count as i32 in Wrapping
}

machine Main::main(&mut self) reaches Console {
    let n: i32 = 3;
    transition n <= 10 {
        true -> allowed(n)
        false -> denied()
    }
    state allowed(&mut self, k: i32) {
        let guarded: i32 in Wrapping = bounded<k>(5);
        self.console.exit_process(guarded as i32);
    }
    state denied(&mut self) {
        self.console.exit_process(99);
    }
}
"#,
            3,
        );
    }

    /// The captured subject flows through a literal-indexed scalar field on
    /// the receiver: each `put` stores its own realized `Count`, each `at`
    /// reads it back, and the authored argument stays distinct: 3 + 5 + 30.
    /// Both the Unit setter and scalar getter require a bound on their own
    /// captured subject; native selection retains each ordered obligation.
    #[test]
    fn runtime_bound_subject_flows_through_indexed_field_writes_natively() {
        run_native(
            "indexed-field",
            r#"
use omega_language_std::console;
use omega::language::core::service;

data Main {
    console: Service<Console> in Bound;
    values: [u8; 8];
}

machine Main::at<Count: u8>(&self) -> u8
requires Count <= 7;
{
    self.values[3]
}

machine Main::put<Count: u8>(&mut self, v: u8)
requires
    Count <= 7;
{
    self.values[3] = Count;
    self.values[4] = v;
}

machine Main::main(&mut self) reaches Console {
    let n: u8 = 3;
    let source: u8 = 5;
    self.put<n>(20);
    let first: u8 = self.at<n>();
    self.put<source>(30);
    let second: u8 = self.at<source>();
    let stored_arg: u8 = self.values[4];
    let total: i32 in Wrapping = (first as i32 in Wrapping) + (second as i32 in Wrapping) + (stored_arg as i32 in Wrapping);
    self.console.exit_process(total as i32);
}
"#,
            38,
        );
    }

    /// A multi-state template cloned for a runtime subject forwards that
    /// subject across its `->` transitions: each arm stores the subject it
    /// received, and distinct subjects reuse the one body: 3 + 4.
    #[test]
    fn runtime_bound_subject_survives_state_transitions_natively() {
        run_native(
            "transitioned-subject",
            r#"
use omega_language_std::console;
use omega::language::core::service;

data Main {
    console: Service<Console> in Bound;
    values: [u8; 8];
}

machine Main::walk<Count: u8>(&mut self, n: u8) {
    transition n == 3 {
        true -> allowed(n)
        false -> denied()
    }
    state allowed(&mut self, n: u8) {
        self.values[3] = Count;
    }
    state denied(&mut self) {
        self.values[4] = Count;
    }
}

machine Main::main(&mut self) reaches Console {
    let n: u8 = 3;
    self.walk<n>(n);
    let m: u8 = 4;
    self.walk<m>(m);
    let stored: u8 = self.values[3];
    let forwarded: u8 = self.values[4];
    let total: i32 in Wrapping = (stored as i32 in Wrapping) + (forwarded as i32 in Wrapping);
    self.console.exit_process(total as i32);
}
"#,
            7,
        );
    }
}

/// The native leg runs only where the macOS ARM64 application can execute;
/// every other host reports the skip instead of silently passing.
#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
#[test]
fn native_leg_requires_a_macos_arm64_host() {
    eprintln!(
        "skipped: runtime value generic native execution needs a macOS ARM64 host (this host is {}-{})",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
}
