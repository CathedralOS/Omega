//! Case payloads read through a sum-typed record field.
//!
//! `transition self.mode { Mode::Run { speed } -> seen(speed) }` tests the
//! field's case and reads the selected payload in place. Only a whole root has
//! a dispatch terminator, so the field's guard stays a membership test and the
//! payload read is an ordinary scalar observation whose carrier ends in the
//! `Case` step. Verification admits that read only below the test's true edge;
//! the interpreter reads the overlay the stored discriminator selects.
use crate::{TerminalMachineSelection, lower_machine};
use semantic_vocabulary::{CanonicalStructuralPathSegment, IntegerSign, IntegerType, IntegerValue};
use terminal_interpreter::{
    AcceptTerminalEffects, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralCaseValue, TerminalStructuralInputs, TerminalStructuralValue,
};
use terminal_psi::{OperationKind, StructuralPathSegment, StructuralTypeShape, TerminalModule};

/// Walk sits first so a stale tag or an unguarded overlay read would take it.
const REASSIGNED: &str = r#"
    data Mode { case Stand; case Walk(pace: i32); case Run(speed: i32); }
    data Root { mode: Mode; }
    machine Root::pick(&mut self) -> i32 {
        self.mode = Mode::Walk { pace: 9 };
        self.mode = Mode::Run { speed: 70 };
        transition self.mode {
            Mode::Walk { pace } -> seen(pace)
            Mode::Run { speed } -> seen(speed)
            _ -> seen(72)
        }
        state seen(&mut self, value: i32) -> i32 { value }
    }
"#;

/// A bounded payload keeps its declared range at the read: `apply`'s bounded
/// formal accepts `level` only through the payload field's own invariant.
const BOUNDED: &str = r#"
    data Command { case SetLevel(level: i32 [0..=100]); case Dim(amount: i32 [0..=10]); case Off; }
    data Root { command: Command; }
    machine Root::pick(&mut self) -> i32 {
        self.command = Command::SetLevel { level: 65 };
        transition self.command {
            Command::SetLevel { level } -> apply(level)
            Command::Dim { amount } -> apply(amount * 10)
            Command::Off -> apply(0)
        }
        state apply(&mut self, target: i32 [0..=100]) -> i32 { target }
    }
"#;

fn lowered(source: &str) -> lowered_psi::LoweredPsi {
    let checked = crate::front_end::checked_program(source);
    lower_machine(&checked, TerminalMachineSelection::Name("Root::pick"))
        .unwrap_or_else(|error| panic!("the field case transition lowers: {error:?}"))
}

/// Run `Root::pick` with its sum field starting in the payloadless case named
/// `initial`, after independently checking the serialized artifact.
fn picked(lowered: &lowered_psi::LoweredPsi, field: &str, initial: &str) -> i128 {
    let module = &lowered.semantic_module;
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("entry machine");
    let initial = module
        .structural_types
        .iter()
        .find_map(|declaration| match &declaration.shape {
            StructuralTypeShape::Sum { cases } => cases
                .iter()
                .find(|case| case.identity == initial)
                .map(|case| case.id),
            _ => None,
        })
        .expect("initial payloadless case");
    let mut execution = terminal_interpreter::TerminalExecution::start_artifact(
        &terminal_codec::encode_module(module).expect("encode module"),
        &terminal_codec::encode_proof_section(module, &lowered.proof_bundle).expect("encode proof"),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[TerminalStructuralValue {
                opaque_identity: 700,
                structural_type: entry.structural_parameters[0].structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            cases: &[TerminalStructuralCaseValue {
                argument_index: 0,
                path: vec![StructuralPathSegment::Field(field.into())],
                case: initial,
            }],
            ..Default::default()
        },
    )
    .expect("the published artifact independently checks");
    let status = execution
        .resume(
            &mut terminal_fuel::TerminalFuelMeter::unbounded(),
            &mut AcceptTerminalEffects,
        )
        .expect("the artifact runs");
    let TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
        TerminalScalarValue::Integer { scalar_type, value },
    )) = status
    else {
        panic!("expected a completed integer result: {status:?}");
    };
    assert_eq!(
        scalar_type,
        IntegerType::new(IntegerSign::Signed, 32).unwrap()
    );
    let IntegerValue::Signed(value) = value else {
        panic!("an i32 result is signed");
    };
    value
}

fn verify(
    module: &TerminalModule,
    proof: &terminal_psi::ProofBundle,
) -> Result<(), terminal_verifier::VerificationError> {
    terminal_verifier::verify_module(module, proof, &proof_admission::AdmissionProfile::default())
        .map(|_| ())
}

/// The case-qualified reads of the lowered module, in block order.
fn case_reads(module: &mut TerminalModule) -> Vec<&mut terminal_psi::Operation> {
    module
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .filter(|operation| {
            matches!(&operation.kind, OperationKind::IntegerStructuralField { path, .. }
                if matches!(path.last(), Some(CanonicalStructuralPathSegment::Case(_))))
        })
        .collect()
}

#[test]
fn a_field_case_payload_reads_in_place_below_its_membership_test() {
    assert_eq!(picked(&lowered(REASSIGNED), "mode", "Stand"), 70);
    let single = REASSIGNED.replace("        self.mode = Mode::Run { speed: 70 };\n", "");
    assert_eq!(picked(&lowered(&single), "mode", "Stand"), 9);
}

/// The payload read carries its field's declared range, which alone proves
/// `apply`'s bounded formal; a formal narrower than the payload's range must
/// not lower.
#[test]
fn a_bounded_field_case_payload_keeps_its_declared_range() {
    let lowered = lowered(BOUNDED);
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the payload's declared range proves the bounded formal");
    let narrowed = BOUNDED.replace("target: i32 [0..=100]", "target: i32 [0..=50]");
    let Ok(checked) = crate::front_end::checked_program_result(&narrowed) else {
        return;
    };
    assert!(lower_machine(&checked, TerminalMachineSelection::Name("Root::pick")).is_err());
}

/// A case-qualified read is authorized only by its own case's membership fact
/// at the read: reading the other arm's overlay, or reading above the test,
/// must reject in independent verification.
#[test]
fn a_field_case_payload_read_without_its_proved_case_rejects() {
    let original = lowered(REASSIGNED);
    verify(&original.semantic_module, &original.proof_bundle).expect("the authored reads verify");

    // Swap which case each read names: each read then sits below the other
    // case's membership test.
    let mut swapped = original.semantic_module.clone();
    let reads = case_reads(&mut swapped);
    let [first, second] = <[_; 2]>::try_from(reads).expect("two payload reads");
    std::mem::swap(&mut first.kind, &mut second.kind);
    assert!(verify(&swapped, &original.proof_bundle).is_err());

    // Hoist the first read into the entry block, above every membership test.
    let mut hoisted = original.semantic_module.clone();
    let machine = &mut hoisted.machines[0];
    let (block, index) = machine
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block, candidate)| {
            candidate
                .operations
                .iter()
                .position(|operation| {
                    matches!(&operation.kind, OperationKind::IntegerStructuralField { path, .. }
                        if matches!(path.last(), Some(CanonicalStructuralPathSegment::Case(_))))
                })
                .map(|index| (block, index))
        })
        .expect("a payload read");
    let read = machine.blocks[block].operations.remove(index);
    let entry = machine
        .blocks
        .iter()
        .position(|candidate| candidate.id == machine.entry)
        .expect("entry block");
    machine.blocks[entry].operations.insert(0, read);
    assert!(verify(&hoisted, &original.proof_bundle).is_err());
}

/// An exhaustive transition's final arm has no membership test of its own.
/// Its payload read is authorized because every other declared case of the
/// field's sum was refuted on the way there.
const EXHAUSTIVE: &str = r#"
    data Shape { case Empty; case Circle(radius: i32); case Rectangle(width: i32, height: i32); }
    data Root { shape: Shape; }
    machine Root::pick(&mut self) -> i32 {
        self.shape = Shape::Rectangle { width: 6, height: 7 };
        transition self.shape {
            Shape::Empty -> seen(0)
            Shape::Circle { radius } -> seen(radius)
            Shape::Rectangle { width, height } -> sides(width, height)
        }
        state seen(&mut self, value: i32) -> i32 { value }
        state sides(&mut self, width: i32, height: i32) -> i32 { height }
    }
"#;

#[test]
fn an_exhaustive_final_arm_reads_its_payload_once_every_other_case_is_refuted() {
    assert_eq!(picked(&lowered(EXHAUSTIVE), "shape", "Empty"), 7);
    let width = EXHAUSTIVE.replace("-> i32 { height }", "-> i32 { width }");
    assert_eq!(picked(&lowered(&width), "shape", "Empty"), 6);

    // The final arm reading the Circle overlay its arrival refuted must reject.
    let original = lowered(EXHAUSTIVE);
    let (circle, radius) = original
        .semantic_module
        .structural_types
        .iter()
        .find_map(|declaration| match &declaration.shape {
            StructuralTypeShape::Sum { cases } => cases
                .iter()
                .find(|case| case.identity == "Circle")
                .map(|case| (case.id, case.fields[0].id)),
            _ => None,
        })
        .expect("the Circle case");
    let mut forged = original.semantic_module.clone();
    let final_arm = case_reads(&mut forged)
        .into_iter()
        .find(|operation| {
            !matches!(&operation.kind, OperationKind::IntegerStructuralField { path, .. }
                if path.last() == Some(&CanonicalStructuralPathSegment::Case(circle)))
        })
        .expect("a final-arm payload read");
    let OperationKind::IntegerStructuralField { path, field, .. } = &mut final_arm.kind else {
        unreachable!("selected a field read");
    };
    *path.last_mut().expect("case step") = CanonicalStructuralPathSegment::Case(circle);
    *field = radius;
    assert!(verify(&forged, &original.proof_bundle).is_err());
}
