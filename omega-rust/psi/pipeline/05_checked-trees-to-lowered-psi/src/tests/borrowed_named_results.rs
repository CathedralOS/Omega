//! `&'a V` borrowed named results spell their reference shell in the plan and
//! loan the referent's storage affinely — the named-carrier analog of a `&mut`
//! primitive reference result. Whole shared-borrowed parameters forward as the
//! result's ingress loan; `&self` receivers and member-leaf projections still
//! mint no structural custody and keep their declines.
use crate::{TerminalMachineSelection, lower_machine};

#[test]
fn detached_shared_borrowed_named_result_lowers_and_verifies() {
    let checked = crate::front_end::checked_program(
        r#"
            data Box { value: u64; }

            machine Box::forward<'r>(v: &'r Box) -> &'r Box {
                v
            }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Box::forward"))
        .expect("borrowed named result composes and lowers");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("borrowed named result verifies");
}

#[test]
fn shared_self_receiver_results_keep_their_decline() {
    // `&'a self` receivers mint no structural parameter custody, so a whole
    // `self` reborrow has no ingress loan to name.
    let checked = crate::front_end::checked_program(
        r#"
            data Box { value: u64; }

            machine Box::borrow<'r>(&self) -> &'r Box {
                self
            }
        "#,
    );
    let error = lower_machine(&checked, TerminalMachineSelection::Name("Box::borrow"))
        .expect_err("shared self receivers have no structural parameter custody yet");
    assert!(
        matches!(
            error,
            crate::lowering_error::LoweringError::InvalidUnitMachinePlan { .. }
                | crate::lowering_error::LoweringError::Unsupported(_)
        ),
        "unexpected outcome: {error:?}"
    );
}

#[test]
fn member_leaf_shared_results_keep_their_decline() {
    let checked = crate::front_end::checked_program(
        r#"
            data Box { value: u64; }
            data Wrap<'r> { inner: &'r Box; tag: u64; }

            machine Wrap::get_inner<'r>(&self) -> &'r Box {
                self.inner
            }
        "#,
    );
    let error = lower_machine(&checked, TerminalMachineSelection::Name("Wrap::get_inner"))
        .expect_err("member-leaf shared results have no receiver custody yet");
    assert!(
        matches!(
            error,
            crate::lowering_error::LoweringError::InvalidUnitMachinePlan { .. }
                | crate::lowering_error::LoweringError::Unsupported(_)
        ),
        "unexpected outcome: {error:?}"
    );
}

#[test]
fn mutable_named_results_keep_their_owned_decline() {
    // `&mut V` referees are named records, not primitives: `parts` still owns
    // that family and the result-type gate keeps declining it.
    let checked = crate::front_end::checked_program(
        r#"
            data Box { value: u64; }

            machine Box::forward_mut<'r>(v: &'r mut Box) -> &'r mut Box {
                v
            }
        "#,
    );
    let error = lower_machine(&checked, TerminalMachineSelection::Name("Box::forward_mut"))
        .expect_err("mutable named results stay outside the shared view family");
    assert!(
        matches!(
            error,
            crate::lowering_error::LoweringError::InvalidUnitMachinePlan { .. }
                | crate::lowering_error::LoweringError::Unsupported(_)
        ),
        "unexpected outcome: {error:?}"
    );
}
