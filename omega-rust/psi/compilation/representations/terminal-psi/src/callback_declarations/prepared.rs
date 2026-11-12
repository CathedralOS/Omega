//! Retain checked callback choices without retaining a source compiler.
//!
//! Source checking projects this owner's records once. Native selection can
//! compare emitted registrations and retain roots using those same records.

use super::*;
use crate as terminal_psi;
use crate::{
    PreparedTerminalRoots, TerminalIdentityTable, TerminalRootDeclaration, TerminalRootRequest,
};
use std::sync::Arc;
use symbols::SymbolHandle;

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
    name: Option<String>,
    span: Option<source::SourceSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PreparedCallbackDeclarations {
    pub declarations: CallbackDeclarationRoster,
    requirements: Vec<RequirementCorrespondence>,
    occurrences: Vec<UseCorrespondence>,
    installed_uses: Vec<CallbackUseId>,
}

impl PreparedCallbackDeclarations {
    /// Keep the projected callback records together after checking source.
    /// The records carry their original IDs and diagnostic locations;
    /// matching an emitted registration still checks every semantic field.
    pub fn from_projection(
        declarations: CallbackDeclarationRoster,
        requirements: Vec<(
            Result<String, String>,
            SymbolHandle,
            SymbolHandle,
            CallbackRequirementId,
        )>,
        occurrences: Vec<(CallbackUseId, Option<String>, Option<source::SourceSpan>)>,
        installed_uses: Vec<CallbackUseId>,
    ) -> Self {
        Self {
            declarations,
            requirements: requirements
                .into_iter()
                .map(|(receiving_application, owner, member, declaration)| {
                    RequirementCorrespondence {
                        receiving_application,
                        owner,
                        member,
                        declaration,
                    }
                })
                .collect(),
            occurrences: occurrences
                .into_iter()
                .map(|(usage, name, span)| UseCorrespondence { usage, name, span })
                .collect(),
            installed_uses,
        }
    }

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

    // One checked registration can be emitted at several operations.
    // Its physical application number distinguishes those operations;
    // it does not identify the checked policy they must all retain.
    // Prepare a borrowed semantic index once for this whole batch. The
    // supplied registrar pair is the emitted boundary's machine/state
    // identity (or the installed registrar repeated), not a name lookup.
    pub fn corresponding_uses<'a>(
        &'a self,
        identities: &'a TerminalIdentityTable,
        emitted_identities: &'a TerminalIdentityTable,
        applications: impl Iterator<
            Item = (
                &'a terminal_psi::TerminalCallbackApplication,
                [&'a terminal_psi::NominalDeclarationIdentity; 2],
                bool,
            ),
        > + 'a,
    ) -> impl Iterator<Item = Result<Vec<CallbackUseId>, String>> + 'a {
        use std::fmt::Write;

        let mut requirements = self.requirements.iter().collect::<Vec<_>>();
        requirements.sort_unstable_by_key(|row| {
            (row.declaration.arena_index(), row.declaration.generation())
        });
        let mut installed = self
            .installed_uses
            .iter()
            .map(|usage| (usage.arena_index(), usage.generation()))
            .collect::<Vec<_>>();
        installed.sort_unstable();
        let mut unavailable_receiving = 0;
        let mut uses = self
            .declarations
            .uses
            .iter()
            .map(|(id, usage)| {
                let requirement = (
                    usage.requirement.arena_index(),
                    usage.requirement.generation(),
                );
                let first = requirements.partition_point(|row| {
                    (row.declaration.arena_index(), row.declaration.generation()) < requirement
                });
                let mut rows = requirements[first..]
                    .iter()
                    .copied()
                    .take_while(|row| row.declaration == usage.requirement);
                let receiving = match (rows.next(), rows.next()) {
                    (Some(row), None) => row.receiving_application.as_deref().ok(),
                    _ => None,
                };
                unavailable_receiving += usize::from(receiving.is_none());
                let is_installed = installed
                    .binary_search(&(id.arena_index(), id.generation()))
                    .is_ok();
                (
                    (receiving, usage.static_machine_ordinal),
                    id,
                    usage,
                    is_installed,
                )
            })
            .collect::<Vec<_>>();
        uses.sort_unstable_by_key(|(key, id, _, _)| (*key, id.arena_index(), id.generation()));
        applications.map(move |(application, registrars, installed)| {
            let receiving = Some(application.calling_owner_application.as_str());
            let key = (receiving, application.static_machine_ordinal);
            let first_receiving = uses.partition_point(|(candidate, _, _, _)| {
                candidate.0 < receiving
            });
            let last_receiving = uses.partition_point(|(candidate, _, _, _)| {
                candidate.0 <= receiving
            });
            let first = uses.partition_point(|(candidate, _, _, _)| *candidate < key);
            let mut candidate_count = 0_usize;
            let mut passed = [0_usize; 6];
            let mut detail_rows = [None; 4];
            let mut representative: Option<(CallbackUseId, &CallbackUseDeclaration)> = None;
            for (_, id, usage, is_installed) in uses[first..]
                .iter()
                .take_while(|(candidate, _, _, _)| *candidate == key)
            {
                let checks = [
                    usage.requirement_overload == application.requirement,
                    usage.published.commitment == application.published_contract,
                    usage.entry.is_some() == application.entry_admitted,
                    *is_installed == installed,
                    usage.selected_declaration.as_ref().is_ok_and(|selected| {
                        selected.same_identity_as(identities, &application.body, emitted_identities)
                    }),
                    usage.registration_declaration.as_ref().is_ok_and(|registration| {
                        registrars.iter().any(|registrar| {
                            registration.same_identity_as(identities, registrar, emitted_identities)
                        })
                    }),
                ];
                candidate_count += 1;
                for (count, passed) in passed.iter_mut().zip(checks) {
                    *count += usize::from(passed);
                }
                // These are the comparisons already used for admission.
                // Retain at most four fixed-width rows so a refusal says
                // which association failed without printing source text.
                if candidate_count <= detail_rows.len() {
                    detail_rows[candidate_count - 1] = Some((
                        *id,
                        checks,
                        [usage.selected_declaration.is_ok(), usage.registration_declaration.is_ok()],
                    ));
                }
                if !checks.into_iter().all(|passed| passed) {
                    continue;
                }
                let Some((old_id, policy)) = representative else {
                    representative = Some((*id, usage));
                    continue;
                };
                // Equivalent occurrences can share a representative only
                // when every policy field agrees; physical numbering is
                // the one field deliberately excluded from that decision.
                let mut expected = policy.clone();
                let mut actual: CallbackUseDeclaration = (*usage).clone();
                expected.application_index = 0;
                actual.application_index = 0;
                if actual != expected {
                    return Err(format!(
                        "checked callback uses disagree on policy: application={} uses={}:{} and {}:{} registration={} refinement={} entry_resources={} selected_entry={}",
                        application.application_index, old_id.arena_index(), old_id.generation(),
                        id.arena_index(), id.generation(),
                        actual.registration == expected.registration && actual.registration_declaration == expected.registration_declaration,
                        actual.refinement == expected.refinement && actual.actual == expected.actual && actual.published == expected.published,
                        actual.entry == expected.entry, actual.selected_entry == expected.selected_entry,
                    ));
                }
                if (id.arena_index(), id.generation()) < (old_id.arena_index(), old_id.generation()) {
                    representative = Some((*id, usage));
                }
            }
            if let Some((id, _)) = representative {
                return Ok(vec![id]);
            }
            let mut details = String::new();
            for (id, checks, available) in detail_rows.into_iter().flatten() {
                write!(&mut details, " use={}:{} checks={checks:?} available[body,registrar]={available:?};", id.arena_index(), id.generation())
                    .expect("writing a diagnostic to String cannot fail");
            }
            let mut reason = format!(
                "callback checked-use association failed: application={} registrar={} static_ordinal={} prepared_uses={} unavailable_receiving={} matching_receiving={} matching_receiving_and_ordinal={} passed[overload,published,entry,membership,body,registrar]={passed:?}; omitted_candidate_rows={};{details}",
                application.application_index, application.registrar.get(), application.static_machine_ordinal,
                uses.len(), unavailable_receiving, last_receiving - first_receiving, candidate_count,
                candidate_count.saturating_sub(4),
            );
            // No arbitrary identity strings enter this message. Keep an
            // explicit final byte bound as its format evolves.
            if reason.len() > 1024 {
                let mut end = 1024;
                while !reason.is_char_boundary(end) { end -= 1; }
                reason.truncate(end);
            }
            Err(reason)
        })
    }

    pub fn selected_roots(
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

#[cfg(test)]
mod correspondence_tests {
    use super::*;
    use crate::{
        CheckedMachineDeclaration, NominalDeclarationIdentity, TerminalCallbackApplication,
        TerminalIdentityTableBuilder,
    };

    fn prepared() -> (
        PreparedCallbackDeclarations,
        TerminalIdentityTable,
        CallbackUseId,
        TerminalCallbackApplication,
        NominalDeclarationIdentity,
    ) {
        let mut identities = TerminalIdentityTableBuilder::default();
        let machine = NominalDeclarationIdentity::new(identities.intern("unit::callback"));
        let entry = NominalDeclarationIdentity::new(identities.intern("unit::callback::entry"));
        let registrar =
            NominalDeclarationIdentity::new(identities.intern("unit::Registrar::register"));
        let owner = NominalDeclarationIdentity::new(identities.intern("unit::Handler"));
        let member = NominalDeclarationIdentity::new(identities.intern("unit::Handler::call"));
        let body = CheckedMachineDeclaration {
            machine: machine.clone(),
            state: entry.clone(),
            semantic_declaration: identities.intern("unit callback semantic declaration"),
            parameter_types: Vec::new(),
            callable: identities.intern("unit::callback()"),
            result_type: identities.intern("u64"),
            contract: [9; 32],
            specialization: None,
        };
        let mut prepared = PreparedCallbackDeclarations::default();
        let requirement =
            prepared
                .declarations
                .requirements
                .insert(CallbackRequirementDeclaration {
                    owner: Ok(owner),
                    member: Ok(member),
                    callable: Ok("unit::Handler::call".into()),
                    is_boundary: true,
                });
        prepared.requirements.push(RequirementCorrespondence {
            receiving_application: Ok("unit::Handler".into()),
            owner: SymbolHandle::invalid(),
            member: SymbolHandle::invalid(),
            declaration: requirement,
        });
        let published = CallbackContractEnvelope {
            report_fingerprint: 1,
            commitment: [7; 32],
        };
        let actual = CallbackContractEnvelope {
            report_fingerprint: 2,
            commitment: [9; 32],
        };
        let usage = prepared.declarations.uses.insert(CallbackUseDeclaration {
            application_index: 0, static_machine_ordinal: 1,
            registration: Ok(language_semantics::callback_declarations::CallbackRegistrationDeclaration::External("unit::Registrar::register".into())),
            registration_declaration: Ok(registrar.clone()), requirement,
            requirement_overload: "unit::Handler::call".into(),
            published: published.clone(), actual: actual.clone(),
            refinement: CallbackContractRefinement { published: published.clone(), actual },
            entry: Some(CallbackEntryResources {
                machine: Ok(machine), entry: Ok(entry.clone()), contract: published,
                stack_report_fingerprint: 3, logical_work_report_fingerprint: 4,
                machine_state_report_fingerprint: 5, envelope_report_fingerprint: 6,
            }),
            selected_declaration: Ok(body.clone()), selected_entry: Ok(entry),
        });
        let application = TerminalCallbackApplication {
            application_index: 0,
            registrar: semantic_vocabulary::OperationId::new(10).unwrap(),
            static_machine_ordinal: 1,
            requirement: "unit::Handler::call".into(),
            calling_owner_application: "unit::Handler".into(),
            boundary: None,
            published_contract: [7; 32],
            entry_admitted: true,
            body,
        };
        let prepared = PreparedCallbackDeclarations::from_projection(
            prepared.declarations,
            prepared
                .requirements
                .into_iter()
                .map(|row| {
                    (
                        row.receiving_application,
                        row.owner,
                        row.member,
                        row.declaration,
                    )
                })
                .collect(),
            Vec::new(),
            prepared.installed_uses,
        );
        (prepared, identities.build(), usage, application, registrar)
    }

    fn match_one(
        prepared: &PreparedCallbackDeclarations,
        identities: &TerminalIdentityTable,
        application: &TerminalCallbackApplication,
        registrar: &NominalDeclarationIdentity,
        installed: bool,
    ) -> Result<Vec<CallbackUseId>, String> {
        prepared
            .corresponding_uses(
                identities,
                identities,
                std::iter::once((application, [registrar, registrar], installed)),
            )
            .next()
            .unwrap()
    }

    #[test]
    fn emitted_copies_keep_one_checked_use_without_reusing_its_physical_number() {
        let (mut prepared, identities, usage, original, registrar) = prepared();
        let mut copy = original.clone();
        // Psi06 assigns this after all original checked uses. It cannot
        // accidentally name a prepared use, but remains a unique emitted occurrence.
        copy.application_index = 1;
        copy.registrar = semantic_vocabulary::OperationId::new(20).unwrap();
        let matches = prepared
            .corresponding_uses(
                &identities,
                &identities,
                [&original, &copy]
                    .into_iter()
                    .map(|application| (application, [&registrar, &registrar], false)),
            )
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(matches, vec![vec![usage], vec![usage]]);
        assert_ne!(original.application_index, copy.application_index);
        assert_ne!(original.registrar, copy.registrar);
        // Source-free table transport can change every numeric identity
        // coordinate while retaining the same complete declarations.
        let mut target = TerminalIdentityTableBuilder::default();
        let result = target.intern("u64");
        let callable = target.intern("unit::callback()");
        let semantic = target.intern("unit callback semantic declaration");
        let entry = NominalDeclarationIdentity::new(target.intern("unit::callback::entry"));
        let machine = NominalDeclarationIdentity::new(target.intern("unit::callback"));
        let target_registrar =
            NominalDeclarationIdentity::new(target.intern("unit::Registrar::register"));
        let mut transported = copy.clone();
        transported.body.machine = machine;
        transported.body.state = entry;
        transported.body.semantic_declaration = semantic;
        transported.body.callable = callable;
        transported.body.result_type = result;
        let target = target.build();
        let transported_ids = prepared
            .corresponding_uses(
                &identities,
                &target,
                std::iter::once((&transported, [&target_registrar, &target_registrar], false)),
            )
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(transported_ids, vec![usage]);
        // An equivalent checked occurrence may share a representative;
        // the complete source roster itself is never removed or rewritten.
        let mut duplicate = prepared.declarations.uses.get(usage).clone();
        duplicate.application_index = 40;
        prepared.declarations.uses.insert(duplicate);
        assert_eq!(
            match_one(&prepared, &identities, &copy, &registrar, false).unwrap(),
            vec![usage]
        );
        assert_eq!(prepared.declarations.uses.len(), 2);
    }

    #[test]
    fn callback_correspondence_refusals_bound_rows_and_distinguish_missing_receiving_metadata() {
        let (mut prepared, identities, usage, original, registrar) = prepared();
        prepared
            .declarations
            .uses
            .get_mut(usage)
            .published
            .commitment = [8; 32];
        for index in 1..8 {
            let mut row = prepared.declarations.uses.get(usage).clone();
            row.application_index = index;
            prepared.declarations.uses.insert(row);
        }
        let reason = match_one(&prepared, &identities, &original, &registrar, false).unwrap_err();
        assert!(reason.len() <= 1024);
        assert!(reason.contains("matching_receiving_and_ordinal=8"));
        assert!(reason.contains("omitted_candidate_rows=4"));
        assert_eq!(reason.matches(" use=").count(), 4);
        prepared.requirements.clear();
        let reason = match_one(&prepared, &identities, &original, &registrar, false).unwrap_err();
        assert!(reason.contains("unavailable_receiving=8"));
        assert!(reason.contains("matching_receiving=0"));
    }

    #[test]
    fn callback_correspondence_preserves_receiving_identity_policy_and_membership() {
        let (prepared, identities, usage, original, registrar) = prepared();
        for changed in 0..6 {
            let mut application = original.clone();
            match changed {
                0 => application.calling_owner_application = "unit::OtherHandler".into(),
                1 => application.requirement = "unit::Handler::other".into(),
                2 => application.static_machine_ordinal += 1,
                3 => application.published_contract = [8; 32],
                4 => application.body.contract = [8; 32],
                5 => application.entry_admitted = false,
                _ => unreachable!(),
            }
            let reason =
                match_one(&prepared, &identities, &application, &registrar, false).unwrap_err();
            assert!(reason.len() <= 1024, "mutation {changed}");
            if changed == 0 {
                assert!(reason.contains("matching_receiving=0"));
            } else if changed == 2 {
                assert!(reason.contains("matching_receiving_and_ordinal=0"));
            } else {
                assert!(
                    reason.contains("passed[overload,published,entry,membership,body,registrar]")
                );
            }
        }
        assert!(
            match_one(
                &prepared,
                &identities,
                &original,
                &original.body.machine,
                false
            )
            .is_err()
        );
        assert!(match_one(&prepared, &identities, &original, &registrar, true).is_err());
        let mut installed = prepared.clone();
        installed.installed_uses.push(usage);
        assert_eq!(
            match_one(&installed, &identities, &original, &registrar, true).unwrap(),
            vec![usage]
        );
        assert!(match_one(&installed, &identities, &original, &registrar, false).is_err());
        let mut specialized = original.clone();
        specialized.body.specialization = Some(terminal_psi::CheckedMachineSpecialization {
            capture: None,
            template: original.body.machine.clone(),
            template_callable: original.body.callable,
            template_semantic_declaration: original.body.semantic_declaration,
            commitment: [1; 32],
        });
        assert!(match_one(&prepared, &identities, &specialized, &registrar, false).is_err());
        for changed in 0..2 {
            let mut ambiguous = prepared.clone();
            let mut conflict = ambiguous.declarations.uses.get(usage).clone();
            conflict.application_index = 1;
            if changed == 0 {
                conflict.entry.as_mut().unwrap().stack_report_fingerprint += 1;
            } else {
                conflict.refinement.actual.commitment = [1; 32];
            }
            ambiguous.declarations.uses.insert(conflict);
            let error =
                match_one(&ambiguous, &identities, &original, &registrar, false).unwrap_err();
            assert!(error.contains("disagree"));
        }
    }
}
