use checked_trees::{CheckedUnitCallPlan, CheckedUnitCallResult, CheckedUnitEffectOperationPlan};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use checked_trees_to_lowered_psi::lower_machine;
use proof_admission::AdmissionProfile;
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_section};
use terminal_fuel::{FuelChargeSite, TerminalFuelMeter};
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralValue,
};
use terminal_psi::{OperationKind, OperationResult, Terminator};

#[path = "affine_result_calls/chains.rs"]
mod chains;

#[path = "affine_result_calls/nested.rs"]
mod nested;

#[path = "affine_result_calls/argument_order.rs"]
mod argument_order;

const IDENTITY_CALL: &str = "data Value { number: u64; }
    machine forward(value: Value) -> Value { value }
    machine Main::caller(value: Value) { let result: Value = forward(value); }";

/// Every fixture here runs inside the same `Main` harness.
fn harness(source: &str) -> String {
    format!("data Main {{}} machine Main::run() {{}} {source}")
}

fn checked(source: &str) -> checked_trees::CheckedTrees {
    crate::front_end::checked_program(&harness(source))
}

/// Whether `call` is an ordinary (non-boundary) call with a structural result.
fn is_ordinary_structural_call(call: &CheckedUnitCallPlan) -> bool {
    !call.is_boundary() && call.structural_result().is_some()
}

/// Whether `call` is an ordinary (non-boundary) call that binds no result.
fn is_ordinary_unit_call(call: &CheckedUnitCallPlan) -> bool {
    !call.is_boundary() && *call.result() == CheckedUnitCallResult::Unit
}

fn structural_call_mut(checked: &mut checked_trees::CheckedTrees) -> &mut CheckedUnitCallPlan {
    checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.operations)
        .filter_map(CheckedUnitEffectOperationPlan::call_mut)
        .find(|call| is_ordinary_structural_call(call))
        .expect("ordinary checked structural call plan")
}

#[test]
fn ordinary_result_calls_reject_checked_custody_drift() {
    for corruption in 0..4 {
        let mut checked = checked(IDENTITY_CALL);
        let call = structural_call_mut(&mut checked);
        if corruption == 0 {
            call.target.contract_report_fingerprint ^= 1;
        } else {
            let (result, discard_result_on_return) = call
                .structural_result_mut()
                .expect("structural call result");
            match corruption {
                1 => result.type_identity.push_str("-missing"),
                2 => result.binding_ordinal += 1,
                3 => *discard_result_on_return = false,
                _ => unreachable!(),
            }
        }
        assert!(
            lower_machine(&checked, TerminalMachineSelection::Name("Main::caller")).is_err(),
            "corruption {corruption}"
        );
    }
}
