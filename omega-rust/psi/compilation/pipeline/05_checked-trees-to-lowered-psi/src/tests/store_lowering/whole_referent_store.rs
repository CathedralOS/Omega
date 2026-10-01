//! Whole-value assignment through an exclusive loan lowers to a root window:
//! the case or record value is established, the loan's whole referent moves
//! out with no field and an empty path, the value reseats it, and a displaced
//! affine referent is discarded on the statement's continuation.

use terminal_psi::OperationKind;

/// The decoder's final state: `verdict = Verdict::Sound` through a loan the
/// state received as a block parameter. `Verdict` is affine, `Mode` is
/// `[copy]`, and `Point` is a record.
const WHOLE_REFERENT_STORES: &str = r#"
    data Verdict { case Invalid; case Sound; }
    data Mode [copy] { case Idle; case Busy(step: u64); }
    data Point { x: u32; y: u32; }
    machine settle(verdict: &mut Verdict, bad: u64) {
        transition bad == 0 {
            true -> sound(verdict)
            _ -> invalid(verdict)
        }
        state sound(verdict: &mut Verdict) { verdict = Verdict::Sound; }
        state invalid(verdict: &mut Verdict) { verdict = Verdict::Invalid; }
    }
    machine busy(mode: &mut Mode, step: u64) { mode = Mode::Busy { step: step }; }
    machine adopt(point: &mut Point, other: Point) { point = other; }
"#;

fn root_window_operations(name: &str) -> (usize, usize) {
    let checked = crate::front_end::checked_program(WHOLE_REFERENT_STORES);
    let lowered = crate::lower_machine(&checked, crate::TerminalMachineSelection::Name(name))
        .unwrap_or_else(|error| panic!("`{name}` lowers its whole-referent store: {error:?}"));
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap_or_else(|error| panic!("`{name}` verifies independently: {error:?}"));
    let operations = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    let moves = operations
        .iter()
        .filter(|operation| {
            matches!(&operation.kind,
                OperationKind::MoveStructuralField { path, field: None, .. } if path.is_empty())
        })
        .count();
    let stores = operations
        .iter()
        .filter(|operation| {
            matches!(&operation.kind,
                OperationKind::StoreStructuralField { path, field: None, .. } if path.is_empty())
        })
        .count();
    (moves, stores)
}

#[test]
fn an_affine_case_literal_replaces_the_referent_in_each_state() {
    assert_eq!(root_window_operations("settle"), (2, 2));
}

#[test]
fn a_copy_case_literal_with_payload_replaces_the_referent() {
    assert_eq!(root_window_operations("busy"), (1, 1));
}

#[test]
fn an_owned_record_parameter_replaces_the_referent() {
    assert_eq!(root_window_operations("adopt"), (1, 1));
}
