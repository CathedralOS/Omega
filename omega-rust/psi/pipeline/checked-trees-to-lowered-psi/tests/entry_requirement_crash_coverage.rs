//! Entry requirements justify crash routes without changing their entry namespace.
//!
//! Fixtures shared by the entry requirement crash coverage tests: typed
//! programs, callers and the trap assertions.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_interpreter::TerminalStructuralInputs;
#[path = "entry_requirement_crash_coverage/boolean_and_disjunctive_requirements.rs"]
mod boolean_and_disjunctive_requirements;
#[path = "entry_requirement_crash_coverage/numeric_and_field_entry_requirements.rs"]
mod numeric_and_field_entry_requirements;
#[path = "entry_requirement_crash_coverage/structural_and_attached_requirements.rs"]
mod structural_and_attached_requirements;

use proof_admission::AdmissionProfile;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_section};
use terminal_interpreter::{
    TerminalArtifactInterpretError, TerminalInterpretError, interpret_terminal_artifact,
};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::CheckingRequest;
use typed_trees_to_checked_trees::lower_typed_trees;

fn typed(source: &str) -> typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    lower_symbol_resolved_trees(&resolved)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

fn with_caller(declarations: &str, call: &str) -> String {
    format!(
        r#"
        {declarations}
        machine value() -> bool
        requires true == true
        ensures true == true
        crashes Trap
        {{ {call} }}
        "#,
    )
}

fn assert_trap(source: &str) {
    assert_trap_with_module_check(source, |_| {});
}

fn assert_trap_with_module_check(
    source: &str,
    check_module: impl Fn(&terminal_psi::TerminalModule),
) {
    assert_trap_at_entry_with_module_check(source, "value", check_module);
}

fn assert_trap_at_entry_with_module_check(
    source: &str,
    entry: &str,
    check_module: impl Fn(&terminal_psi::TerminalModule),
) {
    assert_trap_with_entry_arguments(source, entry, false, check_module);
}

fn assert_trap_with_entry_arguments(
    source: &str,
    entry: &str,
    has_opaque_record_argument: bool,
    check_module: impl Fn(&terminal_psi::TerminalModule),
) {
    let artifact = {
        let checked = lower_typed_trees(typed(source), &CheckingRequest::settled())
            .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
        let lowered = checked_trees_to_lowered_psi::lower_machine(
            &checked,
            TerminalMachineSelection::Name(entry),
        )
        .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
        check_module(&lowered.semantic_module);
        (
            encode_module(&lowered.semantic_module).unwrap(),
            encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap(),
        )
    };
    let decoded_module = decode_module(&artifact.0).expect("decode terminal module");
    let decoded_proof = decode_proof_bundle(&artifact.1).expect("decode proof bundle");
    check_module(&decoded_module);
    terminal_verifier::verify_module(
        &decoded_module,
        &decoded_proof,
        &AdmissionProfile::default(),
    )
    .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
    // Boolean actuals remain source literals. The mixed fixture supplies only
    // an opaque record; its entry has no Requires and no field is observed.
    let result = if has_opaque_record_argument {
        let entry = decoded_module
            .machines
            .iter()
            .find(|machine| machine.id == decoded_module.entry)
            .expect("exact decoded entry");
        assert!(entry.parameters.is_empty(), "no host Boolean assumptions");
        assert!(entry.contract.requires.is_empty(), "no host entry proof");
        let [record] = entry.structural_parameters.as_slice() else {
            panic!("one exact opaque record argument");
        };
        struct NoEffects;
        impl terminal_interpreter::TerminalEffectHandler for NoEffects {
            fn handle_effect(
                &mut self,
                _effect: &terminal_interpreter::TerminalEffect,
            ) -> Result<(), terminal_interpreter::TerminalEffectRejection> {
                panic!("the trigger must trap before the Sink effect");
            }
        }
        terminal_interpreter::interpret_terminal_artifact_measured(
            &artifact.0,
            &artifact.1,
            &AdmissionProfile::default(),
            &[],
            TerminalStructuralInputs {
                arguments: &[terminal_interpreter::TerminalStructuralValue {
                    opaque_identity: 71,
                    structural_type: record.structural_type,
                    qualifications: Vec::new(),
                    path: Vec::new(),
                }],
                ..Default::default()
            },
            &mut NoEffects,
        )
        .map(|_| ())
    } else {
        interpret_terminal_artifact(&artifact.0, &artifact.1, &AdmissionProfile::default(), &[])
            .map(|_| ())
    };
    assert!(
        matches!(result,
        Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(crash)))
            if crash.cause == terminal_psi::CrashCause::Trap),
        "{source}"
    );
}

fn assert_unconditional_call_trap(source: &str) {
    assert_unconditional_call_trap_at_entry(source, "value");
}

fn assert_unconditional_call_trap_at_entry(source: &str, entry: &str) {
    assert_unconditional_call_trap_with_structural_arguments(source, entry, false);
}

fn assert_unconditional_call_trap_with_structural_arguments(
    source: &str,
    entry: &str,
    permits_structural_arguments: bool,
) {
    assert_trap_with_entry_arguments(source, entry, permits_structural_arguments, |module| {
        let mut checked_calls = 0;
        for operation in module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
        {
            let (callee, crash_continuations) = match &operation.kind {
                terminal_psi::OperationKind::Call {
                    callee,
                    crash_continuations,
                    ..
                } => (callee, crash_continuations),
                terminal_psi::OperationKind::CallUnit {
                    callee,
                    structural_arguments,
                    claim_transfers,
                    crash_continuations,
                    ..
                } => {
                    if !permits_structural_arguments {
                        assert!(structural_arguments.is_empty(), "scalar-only Unit fixture");
                        assert!(claim_transfers.is_empty(), "no structural claim transport");
                    }
                    (callee, crash_continuations)
                }
                _ => continue,
            };
            let callee = module
                .machines
                .iter()
                .find(|machine| machine.id == *callee)
                .expect("exact retained call target");
            if !callee.parameters.is_empty() {
                continue;
            }
            assert_eq!(crash_continuations, &callee.contract.crash_routes);
            assert_eq!(
                crash_continuations,
                &[terminal_psi::CrashRouteBucket {
                    cause: terminal_psi::CrashCause::Trap,
                    alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
                }]
            );
            checked_calls += 1;
        }
        assert_eq!(
            checked_calls, 1,
            "one unchanged unconditional trigger continuation"
        );
    });
}

fn integer_field_entry_source(
    declarations: &str,
    parameter_type: &str,
    requirement: &str,
    route: &str,
    body_prefix: &str,
) -> String {
    format!(
        "{declarations}\ndata Helper {{}}\ndata Main {{}}\n\
         boundary trait Sink {{ machine record(value: bool); }}\n\
         machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
         machine Helper::forward(record: {parameter_type})\n\
         reaches Sink requires {requirement}\ncrashes Trap {route}\n\
         {{ {body_prefix} Sink::record(trigger()); }}\n\
         machine Main::value(record: {parameter_type})\n\
         requires {requirement}\ncrashes Trap {route}\n\
         {{ Helper::forward(record); }}"
    )
}

fn assert_structural_entry_requirement_artifact(source: &str) {
    let checked = lower_typed_trees(typed(source), &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    for contract in &checked.facts.contract_plans.machines {
        for bucket in contract.crash.published() {
            for guard in bucket.alternative_guards() {
                if let checked_trees::CrashRouteGuard::Predicate(predicate) = guard {
                    assert!(
                        predicate.scalar_expression().is_some(),
                        "{source}: missing retained runtime predicate for {}",
                        checked.typed.symbols.name(contract.machine)
                    );
                }
            }
        }
    }
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Main::value"),
    )
    .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
    let semantics = encode_module(&lowered.semantic_module).expect("encode shared entry module");
    let evidence = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("encode shared entry proof");
    let module = decode_module(&semantics).expect("decode shared entry module");
    let proof = decode_proof_bundle(&evidence).expect("decode shared entry proof");
    assert_eq!(module, lowered.semantic_module);
    assert_eq!(proof, lowered.proof_bundle);
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("shared entry hypotheses independently cover the unchanged call routes");
    let mut unconditional_calls = 0;
    for operation in module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
    {
        let terminal_psi::OperationKind::Call {
            callee,
            crash_continuations,
            ..
        } = &operation.kind
        else {
            continue;
        };
        let target = module
            .machines
            .iter()
            .find(|machine| machine.id == *callee)
            .expect("retained call target");
        if target.parameters.is_empty() && target.structural_parameters.is_empty() {
            assert_eq!(crash_continuations, &target.contract.crash_routes);
            assert_eq!(
                crash_continuations,
                &[terminal_psi::CrashRouteBucket {
                    cause: terminal_psi::CrashCause::Trap,
                    alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
                }]
            );
            unconditional_calls += 1;
        }
    }
    assert!(
        unconditional_calls > 0,
        "the unconditional callee is retained"
    );
    // This artifact still declares an entry requirement. Verification checks
    // call proof transport; no arbitrary host record is executed as its witness.
}
