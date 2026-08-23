//! Deterministic review projection of accepted proof and trust evidence.

use super::{ProofCodecError, proof_bundle_fingerprint};
use psi_proof_kernel::AcceptedFactRoute;
use psi_terminal_verifier::VerifiedTerminalModule;

/// Render the review view from the exact bundle and trust closures retained by
/// a successful terminal-Psi verification. This deliberately accepts a
/// `VerifiedTerminalModule`: callers cannot render a certificate as accepted
/// without first running the kernel-backed verifier.
pub fn render_verified_proof_synopsis(
    verified: &VerifiedTerminalModule<'_>,
) -> Result<String, ProofCodecError> {
    use std::fmt::Write;

    let fingerprint = proof_bundle_fingerprint(verified.proof_bundle())?;
    let mut output = String::new();
    writeln!(&mut output, "proof-bundle {fingerprint}")
        .expect("writing a synopsis to a String cannot fail");
    let mut facts = verified.accepted_facts().iter().collect::<Vec<_>>();
    facts.sort_by_key(|fact| fact.obligation);
    for fact in facts {
        writeln!(
            &mut output,
            "obligation {} goal {:?}",
            fact.obligation, fact.proposition
        )
        .expect("writing a synopsis to a String cannot fail");
        match &fact.route {
            AcceptedFactRoute::KernelDerived(judgment) => {
                writeln!(&mut output, "  kernel {judgment:?}")
                    .expect("writing a synopsis to a String cannot fail");
            }
            AcceptedFactRoute::CertificateDerived {
                identity,
                proof_system_marker,
                acceptance,
            } => {
                writeln!(
                    &mut output,
                    "  certificate {identity} proof-system {}",
                    proof_system_marker.get()
                )
                .expect("writing a synopsis to a String cannot fail");
                for rule in &acceptance.rules {
                    writeln!(&mut output, "    rule {rule:?}")
                        .expect("writing a synopsis to a String cannot fail");
                }
                for premise in &acceptance.assumptions {
                    writeln!(
                        &mut output,
                        "    assumption[{}] {:?}",
                        premise.index, premise.proposition
                    )
                    .expect("writing a synopsis to a String cannot fail");
                }
                for premise in &acceptance.semantic_axioms {
                    writeln!(
                        &mut output,
                        "    semantic-axiom[{}] {:?}",
                        premise.index, premise.proposition
                    )
                    .expect("writing a synopsis to a String cannot fail");
                }
            }
            AcceptedFactRoute::Admitted(evidence) => {
                writeln!(
                    &mut output,
                    "  admission {:?} site {} authority {} evidence {} profile-decision {}",
                    evidence.kind,
                    evidence.site,
                    evidence.authority_identity,
                    evidence.evidence_identity,
                    evidence.profile_decision
                )
                .expect("writing a synopsis to a String cannot fail");
            }
        }
    }
    for producer in &verified.proof_bundle().evidence_producers {
        writeln!(
            &mut output,
            "evidence-producer {} term {} conformance {} trait {}",
            producer.id,
            producer.term,
            producer.conformance_identity,
            producer.evidence_trait_identity,
        )
        .expect("writing a synopsis to a String cannot fail");
        for row in &producer.rows {
            writeln!(
                &mut output,
                "  row {} {} -> {} {} {:?}",
                row.declaring_trait_identity,
                row.requirement_identity,
                row.realization_machine_identity,
                row.realization_state_identity,
                row.source,
            )
            .expect("writing a synopsis to a String cannot fail");
        }
    }
    let trust_graph = crate::current_terminal_trust_graph().map_err(ProofCodecError::TrustGraph)?;
    output.push_str(
        &crate::render_terminal_trust_graph(&trust_graph)
            .expect("writing a trust graph to a String cannot fail"),
    );
    Ok(output)
}
