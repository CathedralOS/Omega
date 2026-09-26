//! The checked proposition vocabulary and lowered proposition applications.

use crate::checked_trees::ContractProofFactKind;
use arena::{Handle, HandleSpan};
use symbol_resolved_trees_to_typed_trees::typed_trees::proposition::{
    ProofSubstitutions, PropositionLabels,
};
use symbols::SymbolHandle;

pub(crate) fn contract_proposition_labels(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    contracts: &[symbol_resolved_trees_to_typed_trees::typed_trees::signature::SignatureContract],
    kind: symbol_resolved_trees_to_typed_trees::typed_trees::signature::SignatureContractKind,
    substitutions: &[(SymbolHandle, String, String)],
) -> std::collections::BTreeSet<String> {
    use symbol_resolved_trees_to_typed_trees::typed_trees::domain::ProofFact;

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
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    application: &symbol_resolved_trees_to_typed_trees::typed_trees::proposition::PropositionApplication,
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
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
) -> crate::checked_trees::CheckedPropositionVocabulary {
    let declarations = program
        .propositions()
        .iter()
        .filter_map(|declaration| {
            let evidence = match declaration.body {
                symbol_resolved_trees_to_typed_trees::typed_trees::proposition::PropositionBody::Primitive => {
                    crate::checked_trees::CheckedPropositionEvidence::FactOnly
                }
                symbol_resolved_trees_to_typed_trees::typed_trees::proposition::PropositionBody::Witness { evidence } => {
                    crate::checked_trees::CheckedPropositionEvidence::Witness {
                        evidence_type: program.display_type_reference(evidence),
                    }
                }
                symbol_resolved_trees_to_typed_trees::typed_trees::proposition::PropositionBody::Transparent { .. } => return None,
            };
            let binders = program
                .proposition_binders(declaration)
                .iter()
                .map(|binder| crate::checked_trees::CheckedPropositionBinder {
                    name: binder.name.as_str().to_owned(),
                    kind: match binder.kind {
                        symbol_resolved_trees_to_typed_trees::typed_trees::proposition::PropositionBinderKind::Type => {
                            crate::checked_trees::CheckedPropositionBinderKind::Type
                        }
                        symbol_resolved_trees_to_typed_trees::typed_trees::proposition::PropositionBinderKind::Const {
                            type_reference,
                        } => crate::checked_trees::CheckedPropositionBinderKind::Const {
                            type_identity: program.display_type_reference(type_reference),
                        },
                        symbol_resolved_trees_to_typed_trees::typed_trees::proposition::PropositionBinderKind::Machine => {
                            crate::checked_trees::CheckedPropositionBinderKind::Machine
                        }
                    },
                })
                .collect();
            let parameter_types = program
                .proposition_parameters(declaration)
                .iter()
                .map(|parameter| program.display_type_reference(parameter.type_reference))
                .collect();
            Some(crate::checked_trees::CheckedPropositionDeclaration {
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
            let symbol_resolved_trees_to_typed_trees::typed_trees::domain::ProofFact::Proposition(
                application,
            ) = fact
            else {
                return None;
            };
            let normalized =
                program.normalize_nominal_proposition_application(application, None)?;
            Some(lower_checked_proposition_application(normalized))
        })
        .collect();
    crate::checked_trees::CheckedPropositionVocabulary {
        declarations,
        applications,
    }
}

pub(crate) fn lower_checked_proposition_application(
    normalized: symbol_resolved_trees_to_typed_trees::typed_trees::proposition::NormalizedPropositionApplicationIdentity,
) -> crate::checked_trees::CheckedPropositionApplication {
    let evidence_interface = match &normalized.classification {
        symbol_resolved_trees_to_typed_trees::typed_trees::proposition::PropositionEvidenceClassification::FactOnly => None,
        symbol_resolved_trees_to_typed_trees::typed_trees::proposition::PropositionEvidenceClassification::Witness {
            interface, ..
        } => interface.as_ref().map(lower_checked_evidence_interface),
    };
    crate::checked_trees::CheckedPropositionApplication {
        declaration: normalized.declaration,
        binder_arguments: normalized
            .binder_arguments
            .into_iter()
            .map(|argument| crate::checked_trees::CheckedPropositionBinderArgument {
                kind: match argument.kind {
                    symbol_resolved_trees_to_typed_trees::typed_trees::proposition::PropositionBinderArgumentKind::Type => {
                        crate::checked_trees::CheckedPropositionBinderArgumentKind::Type
                    }
                    symbol_resolved_trees_to_typed_trees::typed_trees::proposition::PropositionBinderArgumentKind::Const => {
                        crate::checked_trees::CheckedPropositionBinderArgumentKind::Const
                    }
                    symbol_resolved_trees_to_typed_trees::typed_trees::proposition::PropositionBinderArgumentKind::Machine => {
                        crate::checked_trees::CheckedPropositionBinderArgumentKind::Machine
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
    interface: &symbol_resolved_trees_to_typed_trees::typed_trees::proposition::NormalizedEvidenceInterfaceIdentity,
) -> crate::checked_trees::CheckedEvidenceInterfaceIdentity {
    crate::checked_trees::CheckedEvidenceInterfaceIdentity {
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
                |requirement| crate::checked_trees::CheckedEvidenceRequirementIdentity {
                    declaring_trait: requirement.declaring_trait,
                    declaring_trait_arguments: requirement.declaring_trait_arguments.clone(),
                    requirement: requirement.requirement,
                },
            )
            .collect(),
    }
}

pub(crate) fn fact_handles(
    facts: HandleSpan<symbol_resolved_trees_to_typed_trees::typed_trees::domain::ProofFact>,
) -> impl Iterator<Item = Handle<symbol_resolved_trees_to_typed_trees::typed_trees::domain::ProofFact>>
{
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
    kind: &symbol_resolved_trees_to_typed_trees::typed_trees::signature::SignatureContractKind,
) -> Option<ContractProofFactKind> {
    match kind {
        symbol_resolved_trees_to_typed_trees::typed_trees::signature::SignatureContractKind::Requires => {
            Some(ContractProofFactKind::Requires)
        }
        symbol_resolved_trees_to_typed_trees::typed_trees::signature::SignatureContractKind::Ensures => {
            Some(ContractProofFactKind::Ensures)
        }
        symbol_resolved_trees_to_typed_trees::typed_trees::signature::SignatureContractKind::EnsuresForResultCase { .. } => None,
        symbol_resolved_trees_to_typed_trees::typed_trees::signature::SignatureContractKind::Crashes { .. } => None,
    }
}
