//! The checker counts the certificates it accepted although the mathematical
//! kernel did not judge them, and reports the count as a note.

use super::*;

const SUCCESSOR: &str = "machine entry<const N: u64>(value: u<N>, limit: u<N>) -> u<N> \
                         requires N > 0u64 && value < limit crashes none { value + 1 }";

fn accepted() -> (crate::lowering::Elaboration, crate::Discharge) {
    let typed = super::width_parameters::typed(SUCCESSOR).expect("the source is typed");
    let entry = super::boundary_calls::entry(&typed);
    let mut form = crate::lowering::elaborate_generic(&typed, entry).expect("source elaborates");
    let discharged = discharge(&mut form, &mut crate::ProofSearchActivity::default())
        .expect("the program is accepted");
    (form, discharged)
}

/// The kernel decision of every accepted certificate, mutable.
fn decisions(
    discharged: &mut crate::Discharge,
) -> Vec<&mut proof_admission::MathematicalCoreDecision> {
    let mut decisions = Vec::new();
    for (_, machine) in discharged.machines.iter_mut() {
        for (_, verdict) in machine.verdicts.iter_mut() {
            if let Ok(crate::Discharged::Certificate(evidence)) = verdict
                && let proof_admission::AcceptedFactRoute::CertificateDerived { acceptance, .. } =
                    &mut evidence.fact.route
            {
                decisions.push(&mut acceptance.mathematical_core);
            }
        }
    }
    decisions
}

#[test]
fn a_program_the_kernel_judges_throughout_has_no_note() {
    let (form, discharged) = accepted();
    let mut groups = Vec::new();
    crate::unjudged::count(&form, &discharged, &mut groups);
    assert!(groups.is_empty(), "{groups:?}");
    assert!(crate::unjudged::note(groups).is_none());
}

#[test]
fn a_certificate_the_kernel_refused_is_counted_by_kind_and_shape() {
    // The same accepted program, with one certificate's kernel decision
    // turned to the refusal the bounded checker records for a shape it
    // cannot translate.
    let (form, mut discharged) = accepted();
    let mut refused = decisions(&mut discharged);
    assert!(!refused.is_empty(), "the program has certificates");
    *refused[0] = proof_admission::MathematicalCoreDecision::Refused(
        "a carrier bound of an integer whose width is a parameter",
    );
    let mut groups = Vec::new();
    crate::unjudged::count(&form, &discharged, &mut groups);
    assert_eq!(groups.len(), 1, "{groups:?}");
    assert_eq!(groups[0].count, 1);
    assert_eq!(
        groups[0].shape,
        "a carrier bound of an integer whose width is a parameter"
    );
    let note = crate::unjudged::note(groups).expect("a note");
    assert_eq!(note.severity, diagnostics::DiagnosticSeverity::Note);
    assert!(
        note.message.starts_with(
            "1 accepted certificate(s) rest on the bounded checker alone, because the \
             mathematical kernel cannot judge their shape yet: 1 for "
        ),
        "{}",
        note.message
    );
    assert!(
        note.message
            .ends_with("(a carrier bound of an integer whose width is a parameter)"),
        "{}",
        note.message
    );
}
