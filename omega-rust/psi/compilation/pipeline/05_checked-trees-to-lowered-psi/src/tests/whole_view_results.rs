//! A `collection[0..collection.len]` completion resolves to the carrier
//! parameter itself — or the stored `&` field's referent leaf for member
//! projections — so lowering emits the `EstablishReference` straight through to
//! module validation. The structural-type roster admits a shared `&[T]`
//! result reference, so a parameter's whole view validates and verifies with
//! its result naming the parameter's own storage. A member's whole view stops
//! at reference custody, which does not yet count a stored `&` field's
//! referent as an exact formal referent; partial subslices instead keep their
//! derived-place source and its own provenance gate.
use crate::{TerminalMachineSelection, lower_machine, lowering_error::LoweringError};

#[test]
fn parameter_whole_view_result_reaches_module_validation() {
    let checked = crate::front_end::checked_program(
        r#"
            data Nv { tag: u64; }

            machine Nv::head_all(&self, x: &[u8]) -> &[u8] {
                x[0..x.len]
            }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Nv::head_all"))
        .expect("whole parameter views pass module validation");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("whole parameter views verify");
    let module = &lowered.semantic_module;
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("entry machine");
    let [parameter] = machine.structural_parameters.as_slice() else {
        panic!("the view parameter is the only structural parameter");
    };
    let terminal_psi::TerminalMachineResult::Structural(result) = &machine.result else {
        panic!("structural result");
    };
    let [source] = result.reference_sources.as_slice() else {
        panic!("one reference source: {:?}", result.reference_sources);
    };
    let source = source.sole_source().expect("one possible reference origin");
    assert_eq!(source.place, parameter.place);
    assert!(
        source.path.is_empty(),
        "the whole view names the carrier parameter itself"
    );
    // The roster is custody, not decoration: a result that names no source
    // for its reference is rejected.
    let mut unmapped = module.clone();
    let entry = unmapped.entry;
    let unmapped_machine = unmapped
        .machines
        .iter_mut()
        .find(|machine| machine.id == entry)
        .expect("entry machine");
    let terminal_psi::TerminalMachineResult::Structural(result) = &mut unmapped_machine.result
    else {
        panic!("structural result");
    };
    result.reference_sources.clear();
    assert!(terminal_verifier::validate_module(&unmapped).is_err());
}

#[test]
fn member_whole_view_result_reaches_module_validation() {
    let checked = crate::front_end::checked_program(
        r#"
            data Wv<'r> { view: &'r [u8]; tag: u64; }

            machine Wv::head_all(&self) -> &'r [u8] {
                self.view[0..self.view.len]
            }
        "#,
    );
    let error = lower_machine(&checked, TerminalMachineSelection::Name("Wv::head_all"))
        .expect_err("member whole views stop at reference custody");
    assert!(
        matches!(
            error,
            LoweringError::InvalidTerminalModule(
                terminal_verifier::ModuleError::InvalidReferenceCustody { .. }
            )
        ),
        "unexpected outcome: {error:?}"
    );
}

#[test]
fn derived_subslice_results_keep_their_provenance_decline() {
    // An owned carrier's `[0..len]` — and every `[start..end]` that is not the
    // whole carrier — mints a derived subslice place, which the reference
    // roster does not count as an exact formal referent.
    let checked = crate::front_end::checked_program(
        r#"
            data Nv { tag: u64; }

            machine Nv::head_owned(&self, x: [u8; 64]) -> &[u8] {
                x[0..x.len]
            }
        "#,
    );
    let error = lower_machine(&checked, TerminalMachineSelection::Name("Nv::head_owned"))
        .expect_err("derived subslice places keep their custody decline");
    assert!(
        matches!(
            error,
            LoweringError::InvalidTerminalModule(
                terminal_verifier::ModuleError::InvalidReferenceCustody { .. }
            ) | LoweringError::InvalidUnitMachinePlan { .. }
                | LoweringError::Unsupported(_)
        ),
        "unexpected outcome: {error:?}"
    );
}
