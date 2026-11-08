//! Freeze the final checked nominal-use roster before target policy consumes it.
//!
//! Source correspondence stays under the prepared declaration product's exact
//! checked owner. Consumers borrow canonical uses and requirement coordinates;
//! they never rescan typed declarations or infer identity from an emitted call.

use crate::PreparedTerminalRoots;
use crate::machine_lowering::declarations;
use checked_trees::CheckedTrees;
use std::sync::Arc;
use symbols::SymbolHandle;
use terminal_psi::{
    CallbackContractEnvelope, CallbackContractRefinement, CallbackDeclarationRoster,
    CallbackEntryResources, CallbackRequirementDeclaration, CallbackRequirementId,
    CallbackUseDeclaration, CallbackUseId, TerminalIdentityTable, TerminalIdentityTableBuilder,
    TerminalRootDeclaration, TerminalRootRequest,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct RequirementCorrespondence {
    receiving_application: Result<String, String>,
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
    /// Checked selections explicitly retained by installation operations.
    /// Retaining these bodies does not admit the installation's backing.
    installed_uses: Vec<CallbackUseId>,
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
        let ordinary = checked
            .facts
            .nominal_machine_uses
            .uses
            .iter()
            .map(|usage| (usage, false));
        let installed = checked
            .facts
            .installed_interrupt_registrations
            .iter()
            .flat_map(|registration| {
                registration
                    .callbacks
                    .uses
                    .iter()
                    .map(|usage| (usage, true))
            });
        for (usage, installed) in ordinary.chain(installed) {
            let classification = checked.callback_declaration(usage);
            let requirement =
                match prepared.requirements.iter().find(|association| {
                    association.owner == usage.satisfaction_trait
                        && association.member == usage.satisfaction_requirement
                        && association.receiving_application == usage.receiving_application
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
                            receiving_application: usage.receiving_application.clone(),
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
            if installed {
                prepared.installed_uses.push(declaration);
            }
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

    /// Keep the selected handler available for installation planning. Calling
    /// selection and checked entry evidence are required here; actual table,
    /// root and arrival admission remain native installation obligations.
    pub fn installed_roots(
        &self,
        calling_requirements: &[CallbackRequirementId],
    ) -> Result<Vec<CallbackUseId>, Vec<diagnostics::Diagnostic>> {
        let mut roots = Vec::new();
        let mut errors = Vec::new();
        for &usage in &self.installed_uses {
            let selected = self.declarations.uses.get(usage);
            let declaration = self.declarations.declaration(selected);
            if !declaration.has_checked_entry || !declaration.satisfaction_is_boundary {
                errors.push(diagnostics::Diagnostic::error(
                    "interrupt installation requires its checked callback entry contract",
                ));
            } else if calling_requirements
                .iter()
                .filter(|requirement| **requirement == selected.requirement)
                .count()
                != 1
            {
                errors.push(diagnostics::Diagnostic::error(
                    "interrupt installation requires one selected callback calling plan",
                ));
            } else {
                roots.push(usage);
            }
        }
        if errors.is_empty() {
            Ok(roots)
        } else {
            Err(errors)
        }
    }

    /// Match the child selected by the binder as well as the inherited member.
    /// Matching only the parent would let a sibling's policy admit this callback.
    pub fn calling_requirements<'a>(
        &self,
        declarations: impl IntoIterator<Item = (&'a str, SymbolHandle)>,
    ) -> Vec<CallbackRequirementId> {
        declarations
            .into_iter()
            .filter_map(|(receiving_application, member)| {
                self.requirements
                    .iter()
                    .find(|association| {
                        association.receiving_application.as_deref() == Ok(receiving_application)
                            && association.member == member
                    })
                    .map(|association| association.declaration)
            })
            .collect()
    }

    pub(super) fn selected_roots(
        &self,
        identities: Arc<TerminalIdentityTable>,
        selected: impl IntoIterator<Item = CallbackUseId>,
    ) -> PreparedTerminalRoots {
        let requests = selected.into_iter().map(|selected| {
            let usage = self.declarations.uses.get(selected);
            let diagnostic = self
                .occurrences
                .iter()
                .find(|occurrence| occurrence.usage == selected);
            TerminalRootRequest {
                declaration: match &usage.selected_declaration {
                    Ok(declaration) => TerminalRootDeclaration::Available(declaration.clone()),
                    Err(reason) => TerminalRootDeclaration::Unavailable(reason.clone()),
                },
                name: diagnostic.and_then(|diagnostic| diagnostic.name.clone()),
                span: diagnostic.and_then(|diagnostic| diagnostic.span),
            }
        });
        PreparedTerminalRoots::new(identities, requests)
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
