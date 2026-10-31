//! Freeze the final checked nominal-use roster before target policy consumes it.
//!
//! Source correspondence stays under the prepared declaration product's exact
//! checked owner. Consumers borrow canonical uses and requirement coordinates;
//! they never rescan typed declarations or infer identity from an emitted call.

use crate::PreparedTerminalRoots;
use crate::machine_lowering::declarations;
use crate::prepared_terminal_roots::{PreparedRoot, RootDeclaration};
use checked_trees::CheckedTrees;
use std::sync::Arc;
use symbols::SymbolHandle;
use terminal_psi::{
    CallbackContractEnvelope, CallbackContractRefinement, CallbackDeclarationRoster,
    CallbackEntryResources, CallbackRequirementDeclaration, CallbackRequirementId,
    CallbackUseDeclaration, CallbackUseId, TerminalIdentityTable, TerminalIdentityTableBuilder,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct RequirementCorrespondence {
    owner: SymbolHandle,
    member: SymbolHandle,
    declaration: CallbackRequirementId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UseCorrespondence {
    usage: CallbackUseId,
    site: checked_trees::NominalMachineUseSite,
    registration: SymbolHandle,
    name: Option<String>,
    span: Option<source::SourceSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PreparedCallbackDeclarations {
    pub declarations: CallbackDeclarationRoster,
    requirements: Vec<RequirementCorrespondence>,
    occurrences: Vec<UseCorrespondence>,
}

impl PreparedCallbackDeclarations {
    pub(super) fn prepare(
        checked: &CheckedTrees,
        sources: &[(source::SourceId, [u8; 32])],
        identities: &mut TerminalIdentityTableBuilder,
    ) -> Self {
        let mut prepared = Self::default();
        // This is the completed, possibly pruned roster. Preserve all remaining
        // occurrences, including symbolic/dormant uses and duplicate bodies.
        for usage in &checked.facts.nominal_machine_uses.uses {
            let classification = checked.callback_declaration(usage);
            let requirement =
                match prepared.requirements.iter().find(|association| {
                    association.owner == usage.satisfaction_trait
                        && association.member == usage.satisfaction_requirement
                }) {
                    Some(association) => association.declaration,
                    None => {
                        let typed = &checked.typed;
                        let callable = typed
                            .trait_machine_signature(usage.satisfaction_requirement)
                            .map(|(owner, signature)| {
                                typed.trait_requirement_callable_identity_text(owner, signature)
                            })
                            .ok_or_else(|| {
                                "callback requirement has no exact trait callable declaration"
                                    .to_owned()
                            });
                        let declaration = prepared.declarations.requirements.insert(
                            CallbackRequirementDeclaration {
                                owner: declarations::nominal(
                                    identities,
                                    checked,
                                    sources,
                                    usage.satisfaction_trait,
                                )
                                .map_err(str::to_owned),
                                member: declarations::nominal(
                                    identities,
                                    checked,
                                    sources,
                                    usage.satisfaction_requirement,
                                )
                                .map_err(str::to_owned),
                                callable,
                                is_boundary: classification.satisfaction_is_boundary,
                            },
                        );
                        prepared.requirements.push(RequirementCorrespondence {
                            owner: usage.satisfaction_trait,
                            member: usage.satisfaction_requirement,
                            declaration,
                        });
                        declaration
                    }
                };
            let selected_declaration =
                declarations::identity(identities, checked, sources, usage.selected_machine);
            let declaration = prepared.declarations.uses.insert(CallbackUseDeclaration {
                application_index: usage.application_index,
                static_machine_ordinal: usage.static_machine_ordinal,
                registration: classification.registration,
                registration_declaration: declarations::nominal(
                    identities,
                    checked,
                    sources,
                    usage.registration_operation,
                )
                .map_err(str::to_owned),
                requirement,
                requirement_overload: usage.canonical_requirement_overload.clone(),
                published: envelope(usage.published_requirement_envelope),
                actual: envelope(usage.selected_actual_envelope),
                refinement: CallbackContractRefinement {
                    published: CallbackContractEnvelope {
                        report_fingerprint: usage
                            .refinement
                            .published_requirement_report_fingerprint,
                        commitment: usage.refinement.published_requirement_commitment.as_bytes(),
                    },
                    actual: CallbackContractEnvelope {
                        report_fingerprint: usage.refinement.selected_actual_report_fingerprint,
                        commitment: usage.refinement.selected_actual_commitment.as_bytes(),
                    },
                },
                entry: usage.callback_entry.map(|entry| CallbackEntryResources {
                    machine: declarations::nominal(identities, checked, sources, entry.machine())
                        .map_err(str::to_owned),
                    entry: declarations::nominal(identities, checked, sources, entry.entry())
                        .map_err(str::to_owned),
                    contract: CallbackContractEnvelope {
                        report_fingerprint: entry.contract_report_fingerprint(),
                        commitment: entry.contract_commitment().as_bytes(),
                    },
                    stack_report_fingerprint: entry.stack_report_fingerprint(),
                    logical_work_report_fingerprint: entry
                        .logical_structural_work_report_fingerprint(),
                    machine_state_report_fingerprint: entry.machine_state_report_fingerprint(),
                    envelope_report_fingerprint: entry.envelope_report_fingerprint(),
                }),
                selected_declaration,
                selected_entry: declarations::nominal(
                    identities,
                    checked,
                    sources,
                    usage.selected_entry,
                )
                .map_err(str::to_owned),
            });
            prepared.occurrences.push(UseCorrespondence {
                usage: declaration,
                site: usage.site,
                registration: usage.registration_operation,
                name: checked
                    .typed
                    .machine(usage.selected_machine)
                    .map(|machine| machine.name.as_str().to_owned()),
                span: checked.symbols.symbol_source_span(usage.selected_machine),
            });
        }
        prepared
    }

    /// Close exact source owner/member associations in this product's namespace.
    /// Callers supply only unapplied calling declarations, preserving the existing
    /// empty-boundary-argument rule. Unrelated plans cannot introduce new uses.
    pub fn calling_requirements(
        &self,
        declarations: impl IntoIterator<Item = (SymbolHandle, SymbolHandle)>,
    ) -> Vec<CallbackRequirementId> {
        declarations
            .into_iter()
            .filter_map(|(owner, member)| {
                self.requirements
                    .iter()
                    .find(|association| association.owner == owner && association.member == member)
                    .map(|association| association.declaration)
            })
            .collect()
    }

    pub(super) fn selected_roots(
        &self,
        identities: Arc<TerminalIdentityTable>,
        selected: impl IntoIterator<Item = CallbackUseId>,
    ) -> PreparedTerminalRoots {
        let mut roots = arena::Arena::new();
        for selected in selected {
            let usage = self.declarations.uses.get(selected);
            let diagnostic = self
                .occurrences
                .iter()
                .find(|occurrence| occurrence.usage == selected);
            roots.insert(PreparedRoot {
                declaration: match &usage.selected_declaration {
                    Ok(declaration)
                        if !identities.is_invocation_local(declaration.machine.identity())
                            && !identities.is_invocation_local(declaration.state.identity())
                            && !identities
                                .is_invocation_local(declaration.semantic_declaration)
                            && declaration.specialization.as_ref().is_none_or(
                                |specialization| {
                                    !identities
                                        .is_invocation_local(specialization.template.identity())
                                        && !identities.is_invocation_local(
                                            specialization.template_semantic_declaration,
                                        )
                                },
                            ) =>
                    {
                        RootDeclaration::Available(declaration.clone())
                    }
                    Ok(_) => RootDeclaration::Unavailable(
                        "checked declaration has no exact package or toolchain owner".into(),
                    ),
                    Err(reason) => RootDeclaration::Unavailable(reason.clone()),
                },
                name: diagnostic.and_then(|diagnostic| diagnostic.name.clone()),
                span: diagnostic.and_then(|diagnostic| diagnostic.span),
            });
        }
        PreparedTerminalRoots { identities, roots }
    }
}

fn envelope(
    envelope: checked_trees::CheckedMachineContractEnvelopeIdentity,
) -> CallbackContractEnvelope {
    CallbackContractEnvelope {
        report_fingerprint: envelope.contract_report_fingerprint,
        commitment: envelope.contract_commitment.as_bytes(),
    }
}
