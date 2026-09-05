//! Deterministic review projection of accepted proof and trust evidence.

use super::{ProofCodecError, proof_bundle_fingerprint};
use proof_admission::AcceptedFactRoute;
use terminal_psi::{TerminalModule, TerminalRankedGuard, TerminalRankedSuccessorArgument};
use terminal_verifier::{VerifiedNativeRankedTerminalModule, VerifiedTerminalModule};

/// Render the review view from the exact bundle and trust closures retained by
/// a successful terminal-Psi verification. This deliberately accepts a
/// `VerifiedTerminalModule`: callers cannot render a certificate as accepted
/// without first running the kernel-backed verifier.
pub fn render_verified_proof_synopsis(
    verified: &VerifiedTerminalModule<'_>,
) -> Result<String, ProofCodecError> {
    render_verified_proof_synopsis_body(
        verified.proof_bundle(),
        verified.accepted_facts(),
        verified.accepted_recursive_components(),
        verified.module(),
        false,
    )
}

/// Render the proof review view for the exact native-ranked countdown slice.
///
/// The extra component rows are available only after the specialized verifier
/// has accepted the retained ranked graph. They describe that closed Terminal
/// representation and its verifier-owned unsigned-countdown rule; they are not
/// a general recursive-component certificate.
pub fn render_verified_native_ranked_countdown_synopsis(
    verified: &VerifiedNativeRankedTerminalModule<'_>,
) -> Result<String, ProofCodecError> {
    render_verified_proof_synopsis_body(
        verified.proof_bundle(),
        verified.accepted_facts(),
        verified.accepted_recursive_components(),
        verified.module(),
        true,
    )
}

fn render_verified_proof_synopsis_body(
    proof_bundle: &terminal_verifier::ProofBundle,
    accepted_facts: &[proof_admission::AcceptedFact],
    recursive_components: &[proof_admission::RecursiveComponentAcceptance],
    module: &TerminalModule,
    render_ranked_countdowns: bool,
) -> Result<String, ProofCodecError> {
    use std::fmt::Write;

    let fingerprint = proof_bundle_fingerprint(proof_bundle)?;
    let mut output = String::new();
    writeln!(&mut output, "proof-bundle {fingerprint}")
        .expect("writing a synopsis to a String cannot fail");
    let mut facts = accepted_facts.iter().collect::<Vec<_>>();
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
    for producer in &proof_bundle.evidence_producers {
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
    render_verified_recursive_components(&mut output, module, recursive_components);
    if render_ranked_countdowns {
        render_verified_ranked_countdowns(&mut output, module);
    }
    let trust_graph = crate::current_terminal_trust_graph().map_err(ProofCodecError::TrustGraph)?;
    output.push_str(
        &crate::render_terminal_trust_graph(&trust_graph)
            .expect("writing a trust graph to a String cannot fail"),
    );
    Ok(output)
}

fn render_verified_recursive_components(
    output: &mut String,
    module: &TerminalModule,
    acceptances: &[proof_admission::RecursiveComponentAcceptance],
) {
    use std::fmt::Write;

    for (component, acceptance) in module
        .proof_recursive_components
        .iter()
        .zip(acceptances.iter())
    {
        let identity = terminal_verifier::proof_recursive_component_identity(component);
        writeln!(
            output,
            "recursive-component {identity} certificate {} relation {} rank-type {}",
            acceptance.certificate, acceptance.ranking_relation, component.rank_type_identity,
        )
        .expect("writing a synopsis to a String cannot fail");
        for member in &component.members {
            writeln!(
                output,
                "  member {} machine {} rank-parameter {}",
                member.contract, member.machine_identity, member.rank_parameter_identity,
            )
            .expect("writing a synopsis to a String cannot fail");
        }
        writeln!(
            output,
            "  well-founded obligation {} route {:?}",
            acceptance.well_foundedness.obligation, acceptance.well_foundedness.route,
        )
        .expect("writing a synopsis to a String cannot fail");
        for edge in &component.edges {
            let obligation = terminal_verifier::proof_recursive_edge_obligation_id(component, edge);
            let accepted = acceptance
                .decreases
                .iter()
                .find(|fact| fact.obligation == obligation)
                .expect("verified recursive component retains one acceptance per exact edge");
            writeln!(
                output,
                "  decrease obligation {} caller {} callee {} site {:?} path {:?} route {:?}",
                obligation,
                edge.caller,
                edge.callee,
                edge.site,
                edge.strict_member_path,
                accepted.route,
            )
            .expect("writing a synopsis to a String cannot fail");
        }
    }
}

fn render_verified_ranked_countdowns(output: &mut String, module: &TerminalModule) {
    use std::fmt::Write;

    for machine in &module.machines {
        let Some(component) = &machine.ranked_scc else {
            continue;
        };
        writeln!(
            output,
            "ranked-countdown machine {} header {} rank {} type {:?}-{:?}-{} lower {:?} upper {:?}",
            machine.id,
            component.header,
            component.rank_parameter,
            component.rank_type.carrier(),
            component.rank_type.sign(),
            component.rank_type.bits(),
            component.lower_bound,
            component.upper_bound,
        )
        .expect("writing a synopsis to a String cannot fail");
        writeln!(
            output,
            "  ranking-rule closed-unsigned-countdown verifier-reconstructed"
        )
        .expect("writing a synopsis to a String cannot fail");
        for edge in &component.covered_cyclic_edges {
            writeln!(
                output,
                "  covered-edge {} source {} target {}",
                edge.edge, edge.source, edge.target,
            )
            .expect("writing a synopsis to a String cannot fail");
            match edge.guard {
                TerminalRankedGuard::UnsignedParameterPositive {
                    block,
                    edge,
                    condition,
                    parameter,
                } => writeln!(
                    output,
                    "    guard unsigned-positive block {block} edge {edge} condition {condition} parameter {parameter}",
                )
                .expect("writing a synopsis to a String cannot fail"),
            }
            match edge.successor_argument {
                TerminalRankedSuccessorArgument::UnsignedParameterMinusOne {
                    argument_index,
                    argument,
                    source_parameter,
                    target_parameter,
                } => writeln!(
                    output,
                    "    successor unsigned-minus-one argument-index {argument_index} argument {argument} source-parameter {source_parameter} target-parameter {target_parameter}",
                )
                .expect("writing a synopsis to a String cannot fail"),
            }
        }
    }
}
