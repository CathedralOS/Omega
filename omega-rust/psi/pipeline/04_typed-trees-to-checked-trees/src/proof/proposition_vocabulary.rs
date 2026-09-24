//! The checked proposition vocabulary and lowered proposition applications.

use arena::{Handle, HandleSpan};
use checked_trees::ContractProofFactKind;
use symbols::SymbolHandle;
use typed_trees::proposition::{ProofSubstitutions, PropositionLabels};

pub(crate) fn contract_proposition_labels(
    program: &typed_trees::TypedTrees,
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

pub(crate) fn proposition_application_label(
    program: &typed_trees::TypedTrees,
    application: &typed_trees::proposition::PropositionApplication,
    substitutions: &[(SymbolHandle, String, String)],
) -> Option<String> {
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
    program
        .normalize_proposition_application(
            application,
            Some(PropositionLabels {
                binder_labels: &binder_labels,
                argument_labels: &argument_labels,
            }),
        )
        .map(|formula| formula.identity_label())
}

pub(crate) fn build_checked_proposition_vocabulary(
    program: &typed_trees::TypedTrees,
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
                            type_identity: program.display_type_reference(type_reference),
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
    let applications = program
        .proof_facts
        .iter()
        .filter_map(|(_, fact)| {
            let typed_trees::domain::ProofFact::Proposition(application) = fact else {
                return None;
            };
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
    let evidence_interface = match &normalized.classification {
        typed_trees::proposition::PropositionEvidenceClassification::FactOnly => None,
        typed_trees::proposition::PropositionEvidenceClassification::Witness {
            interface, ..
        } => interface.as_ref().map(lower_checked_evidence_interface),
    };
    checked_trees::CheckedPropositionApplication {
        declaration: normalized.declaration,
        binder_arguments: normalized
            .binder_arguments
            .into_iter()
            .map(|argument| checked_trees::CheckedPropositionBinderArgument {
                kind: match argument.kind {
                    typed_trees::proposition::PropositionBinderArgumentKind::Type => {
                        checked_trees::CheckedPropositionBinderArgumentKind::Type
                    }
                    typed_trees::proposition::PropositionBinderArgumentKind::Const => {
                        checked_trees::CheckedPropositionBinderArgumentKind::Const
                    }
                    typed_trees::proposition::PropositionBinderArgumentKind::Machine => {
                        checked_trees::CheckedPropositionBinderArgumentKind::Machine
                    }
                },
                identity: argument.identity,
                evidence_projection: None,
            })
            .collect(),
        arguments: normalized.arguments,
        evidence_interface,
    }
}

pub(crate) fn lower_checked_evidence_interface(
    interface: &typed_trees::proposition::NormalizedEvidenceInterfaceIdentity,
) -> checked_trees::CheckedEvidenceInterfaceIdentity {
    checked_trees::CheckedEvidenceInterfaceIdentity {
        trait_symbol: interface.trait_symbol,
        arguments: interface
            .arguments
            .iter()
            .map(|argument| argument.as_str().to_owned())
            .collect(),
        requirements: interface
            .requirements
            .iter()
            .map(
                |requirement| checked_trees::CheckedEvidenceRequirementIdentity {
                    declaring_trait: requirement.declaring_trait,
                    declaring_trait_arguments: requirement.declaring_trait_arguments.clone(),
                    requirement: requirement.requirement,
                },
            )
            .collect(),
    }
}

pub(crate) fn fact_handles(
    facts: HandleSpan<typed_trees::domain::ProofFact>,
) -> impl Iterator<Item = Handle<typed_trees::domain::ProofFact>> {
    (0..facts.count()).map(move |offset| {
        Handle::from_parts(
            facts
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("proof fact handle index overflow"),
            facts.start().generation(),
        )
    })
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
