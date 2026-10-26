//! The checked proposition vocabulary and lowered proposition applications.

use arena::{Handle, HandleSpan};
use checked_trees::ContractProofFactKind;
use symbols::SymbolHandle;
use typed_trees::proposition::ProofSubstitutions;

pub(crate) fn contract_proposition_labels(
    program: &dyn ::validation::CheckingInput,
    contracts: &[typed_trees::signature::SignatureContract],
    kind: typed_trees::signature::SignatureContractKind,
    substitutions: &[(SymbolHandle, String, String)],
) -> std::collections::BTreeSet<String> {
    use typed_trees::domain::ProofFact;

    contracts
        .iter()
        .filter(|contract| contract.kind == kind)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .filter_map(|fact| match fact {
            ProofFact::Expression(expression) => Some(format!(
                "boolean:{}",
                program.render_proof_expression(
                    *expression,
                    ProofSubstitutions::ByParameter(substitutions)
                )
            )),
            ProofFact::Proposition(application) => {
                proposition_application_label(program, application, substitutions)
            }
            ProofFact::Membership(_) => None,
        })
        .collect()
}

/// Binder and argument labels for one proposition application under the
/// caller's parameter substitutions.
pub(crate) fn proposition_label_parts(
    program: &dyn ::validation::CheckingInput,
    application: &typed_trees::proposition::PropositionApplication,
    substitutions: &[(SymbolHandle, String, String)],
) -> (Vec<String>, Vec<String>) {
    let binder_labels = application
        .binder_arguments
        .iter()
        .map(|argument| {
            substitutions
                .iter()
                .find(|(symbol, _, _)| *symbol == argument.symbol)
                .map(|(_, _, replacement)| replacement.clone())
                .unwrap_or_else(|| argument.display_name())
        })
        .collect::<Vec<_>>();
    let argument_labels = program
        .expression_table
        .expression_handles(application.arguments)
        .iter()
        .map(|argument| {
            program
                .render_proof_expression(*argument, ProofSubstitutions::ByParameter(substitutions))
        })
        .collect::<Vec<_>>();
    (binder_labels, argument_labels)
}

pub(crate) fn proposition_application_label(
    program: &dyn ::validation::CheckingInput,
    application: &typed_trees::proposition::PropositionApplication,
    substitutions: &[(SymbolHandle, String, String)],
) -> Option<String> {
    let (binder_labels, argument_labels) =
        proposition_label_parts(program, application, substitutions);
    program.normalized_identity_label(application, &binder_labels, &argument_labels)
}

pub(crate) fn build_checked_proposition_vocabulary(
    program: &dyn ::validation::CheckingInput,
) -> checked_trees::CheckedPropositionVocabulary {
    let declarations = program
        .propositions()
        .iter()
        .filter_map(|declaration| {
            let evidence = match declaration.body {
                typed_trees::proposition::PropositionBody::Primitive => {
                    checked_trees::CheckedPropositionEvidence::FactOnly
                }
                typed_trees::proposition::PropositionBody::Witness { evidence } => {
                    checked_trees::CheckedPropositionEvidence::Witness {
                        evidence_type: program.display_type_reference(evidence),
                    }
                }
                typed_trees::proposition::PropositionBody::Transparent { .. } => return None,
            };
            let binders = program
                .proposition_binders(declaration)
                .iter()
                .map(|binder| checked_trees::CheckedPropositionBinder {
                    name: binder.name.as_str().to_owned(),
                    kind: match binder.kind {
                        typed_trees::proposition::PropositionBinderKind::Type => {
                            checked_trees::CheckedPropositionBinderKind::Type
                        }
                        typed_trees::proposition::PropositionBinderKind::Const {
                            type_reference,
                        } => checked_trees::CheckedPropositionBinderKind::Const {
                            type_identity: program.normalized_type_identity(type_reference),
                        },
                        typed_trees::proposition::PropositionBinderKind::Machine => {
                            checked_trees::CheckedPropositionBinderKind::Machine
                        }
                    },
                })
                .collect();
            let parameter_types = program
                .proposition_parameters(declaration)
                .iter()
                .map(|parameter| program.display_type_reference(parameter.type_reference))
                .collect();
            Some(checked_trees::CheckedPropositionDeclaration {
                symbol: declaration.symbol,
                name: declaration.name.as_str().to_owned(),
                is_public: declaration.is_public,
                binders,
                parameter_types,
                evidence,
            })
        })
        .collect();
    let applications = typed_trees::references::program_proof_facts(program)
        .filter_map(|(_, fact)| {
            let application = fact.as_proposition()?;
            let normalized =
                program.normalize_nominal_proposition_application(application, None)?;
            Some(lower_checked_proposition_application(normalized))
        })
        .collect();
    checked_trees::CheckedPropositionVocabulary {
        declarations,
        applications,
    }
}

pub(crate) fn lower_checked_proposition_application(
    normalized: typed_trees::proposition::NormalizedPropositionApplicationIdentity,
) -> checked_trees::CheckedPropositionApplication {
    checked_trees::CheckedPropositionApplication::from_normalized(normalized)
}

pub(crate) fn lower_checked_evidence_interface(
    interface: &typed_trees::proposition::NormalizedEvidenceInterfaceIdentity,
) -> checked_trees::CheckedEvidenceInterfaceIdentity {
    checked_trees::CheckedEvidenceInterfaceIdentity::from_normalized(interface)
}

pub(crate) fn fact_handles(
    facts: HandleSpan<typed_trees::domain::ProofFact>,
) -> impl Iterator<Item = Handle<typed_trees::domain::ProofFact>> {
    facts.handles()
}

pub(crate) fn contract_fact_kind(
    kind: &typed_trees::signature::SignatureContractKind,
) -> Option<ContractProofFactKind> {
    match kind {
        typed_trees::signature::SignatureContractKind::Requires => {
            Some(ContractProofFactKind::Requires)
        }
        typed_trees::signature::SignatureContractKind::Ensures => {
            Some(ContractProofFactKind::Ensures)
        }
        typed_trees::signature::SignatureContractKind::EnsuresForResultCase { .. } => None,
        typed_trees::signature::SignatureContractKind::Crashes { .. } => None,
    }
}
