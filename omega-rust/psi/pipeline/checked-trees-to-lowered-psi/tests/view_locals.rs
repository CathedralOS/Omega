//! A view local is an established view place exactly as a whole view
//! parameter is: it can be narrowed by a `let`, measured and indexed, and a
//! narrowed local can be narrowed again. Each program lowers to Terminal Psi,
//! round-trips through the codec, and is independently verified -- which
//! demands that every subslice's source is an established view dominating it
//! and that its length observes that exact source -- and executed where the
//! interpreter supports the view's establishment.
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::IntegerValue;
use terminal_codec::{encode_module, encode_proof_section};
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecutionResult,
    TerminalScalarValue, interpret_terminal_artifact_measured,
};

#[derive(Default)]
struct Observed(Vec<i128>);

impl TerminalEffectHandler for Observed {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
            panic!("expected an observation boundary");
        };
        let [TerminalScalarValue::Integer { value, .. }] = arguments.as_slice() else {
            panic!("expected one integer observation");
        };
        self.0.push(match value {
            IntegerValue::Unsigned(value) => i128::try_from(*value).unwrap(),
            IntegerValue::Signed(value) => *value,
        });
        Ok(())
    }
}

fn lower(source: &str) -> Result<lowered_psi::LoweredPsi, String> {
    let checked = crate::front_end::checked_program(source);
    checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .map_err(|error| format!("{error:?}"))
}

fn execute(source: &str) -> Vec<i128> {
    let lowered = lower(source).expect("the view-local program lowers");
    let mut observed = Observed::default();
    let result = interpret_terminal_artifact_measured(
        &encode_module(&lowered.semantic_module).unwrap(),
        &encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
        &mut observed,
    )
    .unwrap();
    assert_eq!(result.value(), TerminalExecutionResult::Unit);
    observed.0
}

/// Count every view subslice in the lowered module whose source is itself an
/// operation result rather than a machine parameter: a narrowed view local,
/// not a narrowed formal.
fn subslices_of_established_views(lowered: &lowered_psi::LoweredPsi) -> usize {
    lowered
        .semantic_module
        .machines
        .iter()
        .map(|machine| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter(|operation| {
                    let (terminal_psi::OperationKind::ElementViewSubslice { source, .. }
                    | terminal_psi::OperationKind::ByteSequenceSubslice { source, .. }) =
                        operation.kind
                    else {
                        return false;
                    };
                    machine.structural_places.iter().any(|place| {
                        place.id == source
                            && matches!(
                                place.kind,
                                semantic_vocabulary::StructuralPlaceKind::OperationResult { .. }
                            )
                    })
                })
                .count()
        })
        .sum()
}

const BYTES: &str = r#"
    boundary trait Output {
        machine observe(value: u64) reaches Output;
    }
    data Helper {}
    machine Helper::measure(bytes: &[u8]) reaches Output {
        let tail: &[u8] = bytes[..];
        let window: &[u8] = tail[..];
        Output::observe(tail.len);
        Output::observe(window.len);
    }
    data Root {}
    machine Root::enter() reaches Output {
        Helper::measure("\x0A\x14\x1E");
        Helper::measure("");
    }
"#;

/// A view parameter narrowed by a `let` is a view local; narrowing that
/// local again reads an operation result, and each local measures its own
/// extent.
#[test]
fn a_byte_view_local_is_narrowed_measured_and_narrowed_again() {
    let lowered = lower(BYTES).expect("byte view-local subslices lower");
    // `window` narrows the local `tail`, never the `bytes` formal.
    assert_eq!(subslices_of_established_views(&lowered), 1);
    assert_eq!(execute(BYTES), vec![3, 3, 0, 0]);
}

/// An element view borrowed from a local array narrows, measures and reads
/// the same way: `tail` narrows the `as_slice` local and `window` narrows
/// `tail`, so both ranges read an established view.
#[test]
fn an_element_view_local_is_narrowed_read_and_verified() {
    let lowered = lower(ELEMENTS).expect("element view-local subslices lower");
    assert_eq!(subslices_of_established_views(&lowered), 2);
    // Reload the artifact and verify it independently: each subslice's
    // source must be an established view that dominates it, and each read
    // and length must observe the exact narrowed view. (The interpreter does
    // not yet execute `as_slice` over a local array, with or without a
    // subslice, so this case stops at verification.)
    let module = terminal_codec::decode_module(&encode_module(&lowered.semantic_module).unwrap())
        .expect("reload module");
    let proof = terminal_codec::decode_proof_bundle(
        &encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap(),
    )
    .expect("reload proof");
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("a subslice of a view local verifies independently");
    let reads = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::ElementViewRead { .. }
                    | terminal_psi::OperationKind::ElementViewLength { .. }
            )
        })
        .count();
    assert!(reads >= 3, "two reads and a length observe the view locals");
}

const ELEMENTS: &str = r#"
    boundary trait Output {
        machine observe(value: i32) reaches Output;
        machine count(value: u64) reaches Output;
    }
    data Root {}
    machine Root::enter() reaches Output {
        let values: [i32; 4] = [11, 29, 37, 43];
        let view: &[i32] = values.as_slice();
        let tail: &[i32] = view[1..];
        let window: &[i32] = tail[1..3];
        Output::observe(tail[0]);
        Output::observe(window[1]);
        Output::count(window.len);
    }
"#;

#[test]
fn a_view_local_range_that_overruns_its_source_is_refused() {
    // The narrowed view holds three elements; `tail[1..5]` cannot be proved
    // within it, so checking rejects the program before any lowering.
    let source = ELEMENTS.replace("tail[1..3]", "tail[1..5]");
    let checked = crate::front_end::checked_program_result(&source);
    assert!(
        checked.is_err(),
        "an overrunning view-local range must not check: {checked:?}"
    );
}
