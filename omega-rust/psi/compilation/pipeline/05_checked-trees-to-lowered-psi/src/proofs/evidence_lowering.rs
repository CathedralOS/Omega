//! Proposition and evidence artifact lowering.
//!
//! This file installs the artifacts and names checked evidence identities.
//! `outcome_specific_ensures.rs` lowers outcome-specific ensures,
//! `evidence_terms.rs` lowers the proposition
//! vocabulary, evidence terms, interfaces and contract lanes,
//! `proof_output_calls.rs` lowers proof-output calls and runtime calls and
//! `producer_provenance.rs` lowers evidence producer provenance.

mod evidence_terms;
mod outcome_specific_calls;
mod outcome_specific_ensures;
mod proof_output_calls;

use super::{CheckedTrees, EvidenceTermId, LoweredPsi, LoweringError, unsupported};
use crate::proofs::evidence_lowering::evidence_terms::{
    lower_evidence_contract_lanes, lower_evidence_term_ids, lower_evidence_terms,
    lower_proposition_vocabulary,
};
use crate::proofs::evidence_lowering::outcome_specific_calls::lower_outcome_specific_call_evidence;
use crate::proofs::evidence_lowering::outcome_specific_ensures::outcome_specific_ensure_rows;
use crate::proofs::evidence_lowering::proof_output_calls::lower_proof_output_calls;
use std::collections::BTreeMap;

pub(crate) fn lower_and_install_evidence_artifacts(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    source_machine_ids: &[(symbols::SymbolHandle, semantic_vocabulary::MachineId)],
    lowered: &mut LoweredPsi,
) -> Result<(), LoweringError> {
    for (position, (source, terminal)) in source_machine_ids.iter().enumerate() {
        if source_machine_ids[..position]
            .iter()
            .any(|(previous_source, previous_terminal)| {
                previous_source == source || previous_terminal == terminal
            })
        {
            return unsupported("evidence body mapping repeats a source or terminal machine");
        }
        if lowered.semantic_module.machine(*terminal).is_none() {
            return unsupported("evidence body mapping names an absent terminal machine");
        }
    }
    // Checked handles, including forwarded aliases, share one namespace across
    // the emitted bodies. Per-body allocation would collide or lose vocabulary.
    // Identities are first minted in lane order (machine, contract lane,
    // position), which fixes which checked terms share one identity. The
    // module's canonical order then requires every term identity to be
    // monotone in (proposition application, interface): propositions are
    // numbered by content and a proposition may project another term, so the
    // numbering is settled by renumbering in canonical order and re-lowering
    // the vocabulary until the identities stop moving.
    let mut term_ids = lower_evidence_term_ids(checked, source_machine_ids)?.term_ids;
    let mut rounds = 0_u8;
    let (declarations, applications, evidence_terms) = loop {
        let (declarations, applications, declaration_ids) =
            lower_proposition_vocabulary(identities, checked, &term_ids)?;
        let evidence_terms = lower_evidence_terms(
            identities,
            checked,
            &declaration_ids,
            &applications,
            term_ids.clone(),
        )?;
        let mut canonical = evidence_terms
            .declarations
            .iter()
            .map(|term| (term.proposition, term.interface.clone(), term.id))
            .collect::<Vec<_>>();
        canonical.sort();
        let renumbered = canonical
            .iter()
            .enumerate()
            .map(|(position, (_, _, previous))| {
                let id = EvidenceTermId::new(
                    u64::try_from(position)
                        .expect("evidence term count fits u64")
                        .checked_add(1)
                        .expect("one-based evidence term identity fits u64"),
                )
                .expect("one-based evidence term identity is nonzero");
                (*previous, id)
            })
            .collect::<BTreeMap<_, _>>();
        if renumbered.iter().all(|(previous, id)| previous == id) {
            break (declarations, applications, evidence_terms);
        }
        rounds += 1;
        if rounds > 4 {
            return unsupported("evidence term identities reach no canonical order");
        }
        term_ids = term_ids
            .iter()
            .map(|id| id.map(|previous| renumbered[&previous]))
            .collect();
    };
    let mut evidence_contract_lanes = Vec::new();
    let mut proof_output_calls = Vec::new();
    for &(machine, terminal_machine) in source_machine_ids {
        let outcome_specific_ensures = outcome_specific_ensure_rows(
            identities,
            checked,
            machine,
            terminal_machine,
            &lowered.semantic_module,
            &evidence_terms.term_ids,
            &evidence_terms.declarations,
        )?;
        evidence_contract_lanes.extend(lower_evidence_contract_lanes(
            checked,
            machine,
            terminal_machine,
            &evidence_terms.term_ids,
        )?);
        proof_output_calls.extend(lower_proof_output_calls(
            identities,
            checked,
            machine,
            terminal_machine,
            &lowered.semantic_module,
            &lowered.source_call_occurrences,
            &evidence_terms.term_ids,
            &declarations,
            &applications,
        )?);
        lower_outcome_specific_call_evidence(
            identities,
            checked,
            machine,
            lowered,
            &evidence_terms.term_ids,
            &declarations,
            &applications,
            &evidence_terms.declarations,
        )?;
        let body = lowered
            .semantic_module
            .machines
            .iter_mut()
            .find(|candidate| candidate.id == terminal_machine)
            .ok_or(LoweringError::Unsupported(
                "terminal body is absent while installing guarded guarantees",
            ))?;
        body.contract.outcome_specific_ensures = outcome_specific_ensures;
    }
    evidence_contract_lanes.sort_unstable();
    proof_output_calls.sort_unstable();
    lowered.semantic_module.proposition_declarations = declarations;
    lowered.semantic_module.proposition_applications = applications;
    lowered.semantic_module.evidence_terms = evidence_terms.declarations;
    lowered.semantic_module.evidence_contract_lanes = evidence_contract_lanes;
    lowered.semantic_module.proof_output_calls = proof_output_calls;
    Ok(())
}

fn hex_bytes(bytes: &[u8]) -> String {
    let mut rendered = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut rendered, "{byte:02x}").expect("writing to a String cannot fail");
    }
    rendered
}

/// Lowered kind of one proposition binder argument.
pub(super) fn binder_argument_kind(
    kind: crate::proofs::CheckedPropositionBinderArgumentKind,
) -> terminal_psi::PropositionBinderArgumentKind {
    match kind {
        crate::proofs::CheckedPropositionBinderArgumentKind::Type => {
            terminal_psi::PropositionBinderArgumentKind::Type
        }
        crate::proofs::CheckedPropositionBinderArgumentKind::Const => {
            terminal_psi::PropositionBinderArgumentKind::Const
        }
        crate::proofs::CheckedPropositionBinderArgumentKind::Machine => {
            terminal_psi::PropositionBinderArgumentKind::Machine
        }
    }
}

pub(crate) fn checked_evidence_requirement_identity(
    checked: &CheckedTrees,
    declaring_trait: symbols::SymbolHandle,
    requirement: symbols::SymbolHandle,
) -> Result<String, LoweringError> {
    Ok(checked.typed.callable_identity_text(
        exact_trait_requirement(checked, declaring_trait, requirement)?.overload_identity,
    ))
}

/// Every binder-argument evidence projection lowers the same three checked
/// fields into one identity; only the term-id lookup differs per call site.
pub(super) fn evidence_projection_identity(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    projection: &checked_trees::CheckedEvidenceProjection,
    term: crate::proofs::EvidenceTermId,
) -> Result<crate::proofs::EvidenceProjectionIdentity, LoweringError> {
    Ok(crate::proofs::EvidenceProjectionIdentity {
        term,
        declaring_trait_identity: identities.intern(
            &checked
                .symbols
                .display_path(projection.declaring_trait, "::"),
        ),
        declaring_trait_arguments: projection.declaring_trait_arguments.clone(),
        requirement_identity: identities.intern(&checked_evidence_requirement_identity(
            checked,
            projection.declaring_trait,
            projection.requirement,
        )?),
    })
}

/// One lowered conformance-table row a checked requirement row contributes:
/// the canonical value tuple and the exact realization that tuple selects.
pub(crate) struct CheckedRequirementFamilyRow {
    /// Canonical const identities in the requirement's const/value binder
    /// declaration order; empty for a nongeneric requirement.
    pub family_tuple: Vec<String>,
    /// The machine the row names: the row's own provider for a nongeneric
    /// requirement, or the tuple's bare value-tuple specialization instance.
    pub realization_machine: symbols::SymbolHandle,
    /// The state at the row realization state's declaration position on the
    /// realized machine.
    pub realization_state: symbols::SymbolHandle,
}

/// Complete original declaration inventory; open methods remain schemas even
/// when no closed call demands a body. This is not finite execution coverage.
pub(crate) fn checked_requirement_schema(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    declaring_trait: symbols::SymbolHandle,
    requirement: symbols::SymbolHandle,
    realization_state: symbols::SymbolHandle,
) -> Result<terminal_psi::ClosedConformanceSchema, LoweringError> {
    let exact = exact_trait_requirement(checked, declaring_trait, requirement)?;
    let kind = if !exact.declares_local_generic_binders {
        terminal_psi::ClosedConformanceSchemaKind::Nongeneric
    } else {
        match checked.typed.finite_signature_family(exact.signature) {
            checked_trees::finite_family::FamilyProbe::Finite { .. } => {
                terminal_psi::ClosedConformanceSchemaKind::Finite
            }
            checked_trees::finite_family::FamilyProbe::NotFinite(_) => {
                terminal_psi::ClosedConformanceSchemaKind::Open
            }
        }
    };
    let definition = validation::contract_identities::canonical_requirement_schema_bytes(
        checked,
        exact.signature,
    )
    .map_err(|_| {
        LoweringError::Unsupported("original conformance schema contract cannot be encoded")
    })?;
    let parameter_kinds = checked
        .typed
        .state_signature_type_parameters(exact.signature)
        .iter()
        .map(|parameter| match parameter.kind {
            checked_trees::data::TypeParameterKind::Type => 1,
            checked_trees::data::TypeParameterKind::Const { .. } => 2,
            checked_trees::data::TypeParameterKind::Value { .. } => 3,
            checked_trees::data::TypeParameterKind::Machine { .. } => 4,
            checked_trees::data::TypeParameterKind::Implementation => 5,
            checked_trees::data::TypeParameterKind::Proposition { .. } => 6,
        })
        .collect();
    let finite_tuples = if kind == terminal_psi::ClosedConformanceSchemaKind::Finite {
        match checked.typed.finite_signature_family(exact.signature) {
            checked_trees::finite_family::FamilyProbe::Finite { mut tuples, .. } => {
                tuples.sort_by(|left, right| left.identities.cmp(&right.identities));
                Some(
                    tuples
                        .iter()
                        .map(|tuple| {
                            tuple
                                .identities
                                .iter()
                                .map(|identity| identities.intern(identity))
                                .collect()
                        })
                        .collect(),
                )
            }
            _ => return unsupported("finite conformance schema lost its original declared roster"),
        }
    } else {
        None
    };
    Ok(terminal_psi::ClosedConformanceSchema {
        declaring_trait_identity: identities
            .intern(&checked.symbols.display_path(declaring_trait, "::")),
        public_requirement_identity: identities.intern(&checked_evidence_requirement_identity(
            checked,
            declaring_trait,
            requirement,
        )?),
        requirement_identity: identities.intern(&checked.symbols.display_path(requirement, "::")),
        realization_identity: identities
            .intern(&checked.symbols.display_path(realization_state, "::")),
        definition,
        kind,
        lifetime_parameter_count: exact
            .signature
            .lifetime_parameters
            .len()
            .try_into()
            .map_err(|_| {
                LoweringError::Unsupported(
                    "conformance method lifetime telescope exceeds portable positions",
                )
            })?,
        parameter_kinds,
        finite_tuples,
    })
}

/// Static selection retains open schemas without inventing a finite tuple.
/// Dynamic consumers continue to use the strict finite-family operation below.
pub(crate) fn checked_static_conformance_rows(
    checked: &CheckedTrees,
    declaring_trait: symbols::SymbolHandle,
    requirement: symbols::SymbolHandle,
    realization_machine: symbols::SymbolHandle,
    realization_state: symbols::SymbolHandle,
) -> Result<Vec<CheckedRequirementFamilyRow>, LoweringError> {
    let exact = exact_trait_requirement(checked, declaring_trait, requirement)?;
    if exact.declares_local_generic_binders
        && matches!(
            checked.typed.finite_signature_family(exact.signature),
            checked_trees::finite_family::FamilyProbe::NotFinite(_)
        )
    {
        return Ok(Vec::new());
    }
    checked_requirement_family_rows(
        checked,
        declaring_trait,
        requirement,
        realization_machine,
        realization_state,
    )
}

/// The terminal table rows one checked conformance row lowers to: one row for
/// a nongeneric requirement, or one row per declared roster tuple for a finite
/// generic family. Each family row names the unique bare value-tuple
/// specialization of the row's provider template that the tuple's canonical
/// const identities select. A tuple with no
/// retained specialization, or with several, rejects the row rather than
/// lowering a partial or ambiguous family.
pub(crate) fn checked_requirement_family_rows(
    checked: &CheckedTrees,
    declaring_trait: symbols::SymbolHandle,
    requirement: symbols::SymbolHandle,
    realization_machine: symbols::SymbolHandle,
    realization_state: symbols::SymbolHandle,
) -> Result<Vec<CheckedRequirementFamilyRow>, LoweringError> {
    let exact = exact_trait_requirement(checked, declaring_trait, requirement)?;
    // A requirement with no local binders keeps exactly one empty-tuple row
    // even if it carries a trivially satisfiable `where` clause.
    if !exact.declares_local_generic_binders {
        return Ok(vec![CheckedRequirementFamilyRow {
            family_tuple: Vec::new(),
            realization_machine,
            realization_state,
        }]);
    }
    let tuples = match checked.typed.finite_signature_family(exact.signature) {
        checked_trees::finite_family::FamilyProbe::Finite { tuples, .. } => tuples,
        checked_trees::finite_family::FamilyProbe::NotFinite(_) => {
            return unsupported(
                "conformance requirement declares requirement-local generic binders without a \
                 finite family roster",
            );
        }
    };
    let template = checked
        .typed
        .machine(realization_machine)
        .ok_or(LoweringError::Unsupported(
            "conformance family row has no exact realization template machine",
        ))?;
    let state_position = typed_trees::state::state_position(
        checked.typed.machine_states(template),
        realization_state,
    )
    .ok_or(LoweringError::Unsupported(
        "conformance family row has no exact realization template state",
    ))?;
    tuples
        .iter()
        .map(|tuple| {
            let mut specializations =
                checked
                    .typed
                    .machine_specializations
                    .iter()
                    .filter(|specialization| {
                        specialization.template == realization_machine
                            && specialization.const_argument_identities.as_slice()
                                == tuple.identities.as_ref()
                            && specialization.is_const_only()
                    });
            let Some(specialization) = specializations.next() else {
                return unsupported(
                    "conformance family tuple has no bare value-tuple specialization of the \
                     row's provider",
                );
            };
            if specializations.next().is_some() {
                return unsupported(
                    "conformance family tuple resolves to ambiguous provider specializations",
                );
            }
            let instance = checked.typed.machine(specialization.instance).ok_or(
                LoweringError::Unsupported(
                    "conformance family specialization has no retained instance machine",
                ),
            )?;
            let instance_state = checked
                .typed
                .machine_states(instance)
                .get(state_position)
                .ok_or(LoweringError::Unsupported(
                    "conformance family specialization lost its tuple state",
                ))?;
            Ok(CheckedRequirementFamilyRow {
                family_tuple: tuple.identities.to_vec(),
                realization_machine: instance.symbol,
                realization_state: instance_state.symbol,
            })
        })
        .collect()
}

/// The exact typed requirement one `(declaring trait, requirement)` symbol
/// pair names, projected to the facts evidence and dynamic rows consume.
struct ExactTraitRequirement<'a> {
    /// The resolved signature; `finite_signature_family` reads its `where`
    /// clause for the family roster.
    signature: &'a checked_trees::signature::StateSignature,
    /// Canonical normalized overload identity; never empty.
    overload_identity: typed_trees::type_identity::CallableIdentityId,
    /// Whether the signature declares requirement-local generic binders.
    declares_local_generic_binders: bool,
}

fn exact_trait_requirement<'a>(
    checked: &'a CheckedTrees,
    declaring_trait: symbols::SymbolHandle,
    requirement: symbols::SymbolHandle,
) -> Result<ExactTraitRequirement<'a>, LoweringError> {
    let mut matches = checked
        .typed
        .traits()
        .iter()
        .filter(|definition| definition.symbol == declaring_trait)
        .flat_map(|definition| {
            checked
                .typed
                .trait_machine_signatures(definition)
                .iter()
                .filter(move |signature| signature.symbol == requirement)
                .map(move |signature| (definition, signature))
        });
    let (definition, signature) = matches.next().ok_or(LoweringError::Unsupported(
        "evidence producer row has no exact trait requirement",
    ))?;
    if matches.next().is_some() {
        return unsupported("evidence producer row has an ambiguous trait requirement");
    }
    let overload_identity = checked
        .typed
        .trait_requirement_callable_identity(definition, signature);
    if !overload_identity.is_valid() {
        return unsupported("evidence producer row has an empty requirement identity");
    }
    Ok(ExactTraitRequirement {
        signature,
        overload_identity,
        declares_local_generic_binders: !signature.lifetime_parameters.is_empty()
            || !checked
                .typed
                .state_signature_type_parameters(signature)
                .is_empty(),
    })
}

pub(crate) fn checked_evidence_machine_identity(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<String, LoweringError> {
    let mut matches = checked
        .typed
        .machines()
        .iter()
        .filter(|candidate| candidate.symbol == machine);
    let machine = matches.next().ok_or(LoweringError::Unsupported(
        "evidence producer row has no exact realization machine",
    ))?;
    if matches.next().is_some() {
        return unsupported("evidence producer row has an ambiguous realization machine");
    }
    let mut identity = checked
        .typed
        .callable_identity_text(checked.typed.machine_callable_identity(machine));
    if checked
        .typed
        .executable_application_template(machine.symbol)
        .map_err(|message| LoweringError::Unsupported(message))?
        .is_some()
    {
        let checked_identity = checked
            .facts
            .specialization_identities
            .for_instance(machine.symbol)
            .ok_or(LoweringError::Unsupported(
                "evidence machine lost its checked executable application identity",
            ))?;
        identity = format!(
            "specialized-machine|callable={}:{}|application={}",
            identity.len(),
            identity,
            hex_bytes(&checked_identity.commitment.as_bytes())
        );
    }
    Ok(identity)
}
