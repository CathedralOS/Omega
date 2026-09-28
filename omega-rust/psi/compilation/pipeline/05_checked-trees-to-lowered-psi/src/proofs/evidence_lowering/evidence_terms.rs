//! The proposition vocabulary, evidence terms, interfaces and contract
//! lanes in terminal form.

use crate::proofs::evidence_lowering::checked_evidence_requirement_identity;
use crate::proofs::evidence_lowering::producer_provenance::evidence_term_root;
use crate::proofs::evidence_lowering::proof_output_calls::proof_output_forwarded_source;
use crate::proofs::{
    BTreeMap, BTreeSet, CheckedPropositionBinderArgumentKind, CheckedPropositionBinderKind,
    CheckedPropositionEvidence, CheckedTrees, EvidenceContractLane, EvidenceContractLaneKind,
    EvidenceInterfaceIdentity, EvidenceProjectionIdentity, EvidenceRequirementIdentity,
    EvidenceTermDeclaration, EvidenceTermId, LoweringError, MachineId,
    PropositionApplicationIdentity, PropositionBinderArgumentIdentity,
    PropositionBinderArgumentKind, PropositionBinderDeclaration, PropositionBinderKind,
    PropositionDeclaration, PropositionEvidence, PropositionId, proposition_id,
};

pub(crate) fn lower_proposition_vocabulary(
    checked: &CheckedTrees,
    term_ids: &[Option<EvidenceTermId>],
) -> Result<
    (
        Vec<PropositionDeclaration>,
        Vec<PropositionApplicationIdentity>,
        Vec<(symbols::SymbolHandle, PropositionId)>,
    ),
    LoweringError,
> {
    let placeholder = proposition_id(1);
    let mut declarations = checked
        .facts
        .proof
        .proposition_vocabulary
        .declarations
        .iter()
        .map(|declaration| {
            let evidence = match &declaration.evidence {
                CheckedPropositionEvidence::FactOnly => PropositionEvidence::FactOnly,
                CheckedPropositionEvidence::Witness { evidence_type } => {
                    PropositionEvidence::Witness {
                        evidence_type: evidence_type.clone(),
                    }
                }
            };
            let binders = declaration
                .binders
                .iter()
                .map(|binder| PropositionBinderDeclaration {
                    name: binder.name.clone(),
                    kind: match &binder.kind {
                        CheckedPropositionBinderKind::Type => PropositionBinderKind::Type,
                        CheckedPropositionBinderKind::Const { type_identity } => {
                            PropositionBinderKind::Const {
                                type_identity: type_identity.clone(),
                            }
                        }
                        CheckedPropositionBinderKind::Machine => PropositionBinderKind::Machine,
                    },
                })
                .collect();
            (
                declaration.symbol,
                PropositionDeclaration {
                    id: placeholder,
                    name: declaration.name.clone(),
                    binders,
                    parameter_types: declaration.parameter_types.clone(),
                    evidence,
                },
            )
        })
        .collect::<Vec<_>>();
    declarations.sort_by(|left, right| left.1.cmp(&right.1));
    for (index, (_, declaration)) in declarations.iter_mut().enumerate() {
        declaration.id = proposition_id(
            u64::try_from(index)
                .expect("proposition declaration count fits u64")
                .checked_add(1)
                .expect("one-based proposition identity fits u64"),
        );
    }
    let declaration_ids = declarations
        .iter()
        .map(|(symbol, declaration)| (*symbol, declaration.id))
        .collect::<Vec<_>>();

    let retained_term_applications =
        checked
            .facts
            .proof
            .evidence_terms
            .iter()
            .filter_map(|(handle, term)| {
                let index = usize::try_from(handle.arena_index() - 1)
                    .expect("arena indices fit the host address space");
                term_ids
                    .get(index)
                    .copied()
                    .flatten()
                    .map(|_| &term.proposition)
            });
    let mut applications = Vec::new();
    for application in checked
        .facts
        .proof
        .proposition_vocabulary
        .applications
        .iter()
        .chain(retained_term_applications)
    {
        let Some(declaration) = declaration_ids
            .iter()
            .find_map(|(symbol, id)| (*symbol == application.declaration).then_some(*id))
        else {
            continue;
        };
        let mut binder_arguments = Vec::new();
        let mut belongs_to_selected_machine = true;
        for argument in &application.binder_arguments {
            let evidence_projection = if let Some(projection) = &argument.evidence_projection {
                let index = usize::try_from(projection.term.arena_index() - 1)
                    .expect("arena indices fit the host address space");
                let Some(term) = term_ids.get(index).copied().flatten() else {
                    belongs_to_selected_machine = false;
                    break;
                };
                Some(EvidenceProjectionIdentity {
                    term,
                    declaring_trait_identity: checked
                        .symbols
                        .display_path(projection.declaring_trait, "::"),
                    declaring_trait_arguments: projection.declaring_trait_arguments.clone(),
                    requirement_identity: checked_evidence_requirement_identity(
                        checked,
                        projection.declaring_trait,
                        projection.requirement,
                    )?,
                })
            } else {
                None
            };
            binder_arguments.push(PropositionBinderArgumentIdentity {
                kind: match argument.kind {
                    CheckedPropositionBinderArgumentKind::Type => {
                        PropositionBinderArgumentKind::Type
                    }
                    CheckedPropositionBinderArgumentKind::Const => {
                        PropositionBinderArgumentKind::Const
                    }
                    CheckedPropositionBinderArgumentKind::Machine => {
                        PropositionBinderArgumentKind::Machine
                    }
                },
                identity: argument.identity.clone(),
                evidence_projection,
            });
        }
        if !belongs_to_selected_machine {
            continue;
        }
        applications.push(PropositionApplicationIdentity {
            id: placeholder,
            declaration,
            binder_arguments,
            arguments: application.arguments.clone(),
            evidence_interface: application
                .evidence_interface
                .as_ref()
                .map(|interface| lower_evidence_interface(checked, interface))
                .transpose()?,
        });
    }
    applications.sort();
    applications.dedup();
    for (index, application) in applications.iter_mut().enumerate() {
        application.id = proposition_id(
            u64::try_from(index)
                .expect("proposition application count fits u64")
                .checked_add(1)
                .expect("one-based proposition identity fits u64"),
        );
    }
    Ok((
        declarations
            .into_iter()
            .map(|(_, declaration)| declaration)
            .collect(),
        applications,
        declaration_ids,
    ))
}

/// Retain one terminal identity per distinct checked evidence term. Direct
/// forwarding aliases its output to the exact source term and therefore does
/// not mint a second identity. A selected producer keeps its output identity
/// distinct; its conformance provenance is lowered into the proof bundle.
pub(crate) struct LoweredEvidenceTerms {
    pub(crate) declarations: Vec<EvidenceTermDeclaration>,
    pub(crate) term_ids: Vec<Option<EvidenceTermId>>,
}

pub(crate) struct LoweredEvidenceTermIds {
    pub(crate) term_ids: Vec<Option<EvidenceTermId>>,
}

pub(crate) fn lower_evidence_term_ids(
    checked: &CheckedTrees,
    selected_machine: symbols::SymbolHandle,
) -> Result<LoweredEvidenceTermIds, LoweringError> {
    let mut parents = (0..checked.facts.proof.evidence_terms.len()).collect::<Vec<_>>();
    let guarded_terms = selected_guarded_evidence_terms(checked, selected_machine);
    let invocations = checked
        .facts
        .proof
        .proof_output_calls
        .iter()
        .filter_map(|(_, invocation)| {
            (invocation.caller_machine_symbol == selected_machine).then_some(invocation)
        })
        .collect::<Vec<_>>();
    for (_, forwarding) in checked.facts.proof.evidence_forwardings.iter() {
        if forwarding.machine_symbol != selected_machine {
            continue;
        }
        if let checked_trees::EvidenceAssignmentSource::Forwarded { term: source } =
            &forwarding.source
        {
            let output = usize::try_from(forwarding.output.arena_index() - 1)
                .expect("arena indices fit the host address space");
            let source = usize::try_from(source.arena_index() - 1)
                .expect("arena indices fit the host address space");
            let output_root = evidence_term_root(&mut parents, output);
            let source_root = evidence_term_root(&mut parents, source);
            parents[output_root] = source_root;
        }
    }
    for invocation in &invocations {
        for output in &invocation.outputs {
            let Some(source) = proof_output_forwarded_source(checked, invocation, output) else {
                continue;
            };
            if let Some(output) = output.output {
                let output = usize::try_from(output.arena_index() - 1)
                    .expect("arena indices fit the host address space");
                let source = usize::try_from(source.arena_index() - 1)
                    .expect("arena indices fit the host address space");
                let output_root = evidence_term_root(&mut parents, output);
                let source_root = evidence_term_root(&mut parents, source);
                parents[output_root] = source_root;
            }
        }
    }

    let mut roots = BTreeMap::<usize, (u8, usize)>::new();
    for (handle, term) in checked.facts.proof.evidence_terms.iter() {
        if term.owner
            != (checked_trees::ContractProofFactOwner::Machine {
                machine_symbol: selected_machine,
            })
        {
            continue;
        }
        let index = usize::try_from(handle.arena_index() - 1)
            .expect("arena indices fit the host address space");
        let root = evidence_term_root(&mut parents, index);
        let lane_key = match term.kind {
            checked_trees::ContractProofFactKind::Requires => (0_u8, term.lane_position),
            checked_trees::ContractProofFactKind::Ensures
                if guarded_terms.contains(
                    &usize::try_from(handle.arena_index() - 1)
                        .expect("arena indices fit the host address space"),
                ) =>
            {
                (2_u8, term.lane_position)
            }
            checked_trees::ContractProofFactKind::Ensures => (1_u8, term.lane_position),
        };
        roots
            .entry(root)
            .and_modify(|previous| *previous = (*previous).min(lane_key))
            .or_insert(lane_key);
    }
    let mut package_identity_position = 0_usize;
    for invocation in invocations {
        for argument in &invocation.evidence_arguments {
            let index = usize::try_from(argument.source.arena_index() - 1)
                .expect("arena indices fit the host address space");
            let root = evidence_term_root(&mut parents, index);
            roots
                .entry(root)
                .or_insert((3_u8, package_identity_position));
            package_identity_position = package_identity_position
                .checked_add(1)
                .expect("proof-output identity order fits usize");
        }
        for output in &invocation.outputs {
            let forwarded = proof_output_forwarded_source(checked, invocation, output).is_some();
            let retains_callee_term =
                !forwarded && invocation.static_requirement_dispatch.is_none();
            for handle in output
                .output
                .into_iter()
                .chain(retains_callee_term.then_some(output.callee_output))
            {
                let index = usize::try_from(handle.arena_index() - 1)
                    .expect("arena indices fit the host address space");
                let root = evidence_term_root(&mut parents, index);
                roots
                    .entry(root)
                    .or_insert((3_u8, package_identity_position));
                package_identity_position = package_identity_position
                    .checked_add(1)
                    .expect("proof-output identity order fits usize");
            }
        }
    }
    let mut roots = roots
        .into_iter()
        .map(|(root, lane_key)| (lane_key, root))
        .collect::<Vec<_>>();
    roots.sort_unstable();
    let root_ids = roots
        .into_iter()
        .enumerate()
        .map(|(index, (_, root))| {
            let id = EvidenceTermId::new(
                u64::try_from(index)
                    .expect("evidence term count fits u64")
                    .checked_add(1)
                    .expect("one-based evidence term identity fits u64"),
            )
            .expect("one-based evidence term identity is nonzero");
            (root, id)
        })
        .collect::<BTreeMap<_, _>>();
    let mut term_ids = vec![None; parents.len()];
    for (handle, _) in checked.facts.proof.evidence_terms.iter() {
        let index = usize::try_from(handle.arena_index() - 1)
            .expect("arena indices fit the host address space");
        let root = evidence_term_root(&mut parents, index);
        term_ids[index] = root_ids.get(&root).copied();
    }
    Ok(LoweredEvidenceTermIds { term_ids })
}

pub(crate) fn lower_evidence_terms(
    checked: &CheckedTrees,
    _selected_machine: symbols::SymbolHandle,
    declaration_ids: &[(symbols::SymbolHandle, PropositionId)],
    applications: &[PropositionApplicationIdentity],
    term_ids: Vec<Option<EvidenceTermId>>,
) -> Result<LoweredEvidenceTerms, LoweringError> {
    let mut identities_by_id =
        BTreeMap::<EvidenceTermId, (PropositionId, EvidenceInterfaceIdentity)>::new();
    for (handle, term) in checked.facts.proof.evidence_terms.iter() {
        let index = usize::try_from(handle.arena_index() - 1)
            .expect("arena indices fit the host address space");
        if term_ids.get(index).copied().flatten().is_none() {
            continue;
        }
        let id = term_ids[index].ok_or(LoweringError::Unsupported(
            "selected terminal evidence term has no canonical identity",
        ))?;
        let declaration = declaration_ids
            .iter()
            .find_map(|(symbol, id)| (*symbol == term.proposition.declaration).then_some(*id))
            .ok_or(LoweringError::Unsupported(
                "checked evidence term has no terminal proposition declaration",
            ))?;
        let binder_arguments = term
            .proposition
            .binder_arguments
            .iter()
            .map(|argument| {
                let evidence_projection = argument
                    .evidence_projection
                    .as_ref()
                    .map(|projection| {
                        let projection_index = usize::try_from(projection.term.arena_index() - 1)
                            .expect("arena indices fit the host address space");
                        Ok(EvidenceProjectionIdentity {
                            term: term_ids.get(projection_index).copied().flatten().ok_or(
                                LoweringError::Unsupported(
                                    "evidence-term proposition projects an unrelated term",
                                ),
                            )?,
                            declaring_trait_identity: checked
                                .symbols
                                .display_path(projection.declaring_trait, "::"),
                            declaring_trait_arguments: projection.declaring_trait_arguments.clone(),
                            requirement_identity: checked_evidence_requirement_identity(
                                checked,
                                projection.declaring_trait,
                                projection.requirement,
                            )?,
                        })
                    })
                    .transpose()?;
                Ok(PropositionBinderArgumentIdentity {
                    kind: match argument.kind {
                        CheckedPropositionBinderArgumentKind::Type => {
                            PropositionBinderArgumentKind::Type
                        }
                        CheckedPropositionBinderArgumentKind::Const => {
                            PropositionBinderArgumentKind::Const
                        }
                        CheckedPropositionBinderArgumentKind::Machine => {
                            PropositionBinderArgumentKind::Machine
                        }
                    },
                    identity: argument.identity.clone(),
                    evidence_projection,
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let proposition = applications
            .iter()
            .find(|application| {
                application.declaration == declaration
                    && application.binder_arguments == binder_arguments
                    && application.arguments == term.proposition.arguments
            })
            .map(|application| application.id)
            .ok_or(LoweringError::Unsupported(
                "checked evidence term has no terminal proposition application",
            ))?;
        let checked_interface =
            term.evidence_interface
                .as_ref()
                .ok_or(LoweringError::Unsupported(
                    "terminal evidence term has an unresolved carrierless interface",
                ))?;
        let interface = lower_evidence_interface(checked, checked_interface)?;
        if let Some((previous_proposition, previous_interface)) = identities_by_id.get(&id) {
            if *previous_proposition != proposition || *previous_interface != interface {
                return Err(LoweringError::Unsupported(
                    "forwarded evidence terms disagree on exact terminal identity",
                ));
            }
        } else {
            identities_by_id.insert(id, (proposition, interface));
        }
    }
    let declarations = identities_by_id
        .into_iter()
        .map(|(id, (proposition, interface))| EvidenceTermDeclaration {
            id,
            proposition,
            interface,
        })
        .collect();
    Ok(LoweredEvidenceTerms {
        declarations,
        term_ids,
    })
}

pub(crate) fn lower_evidence_interface(
    checked: &CheckedTrees,
    interface: &checked_trees::CheckedEvidenceInterfaceIdentity,
) -> Result<EvidenceInterfaceIdentity, LoweringError> {
    let mut requirements = interface
        .requirements
        .iter()
        .map(|requirement| {
            Ok(EvidenceRequirementIdentity {
                declaring_trait_identity: checked
                    .symbols
                    .display_path(requirement.declaring_trait, "::"),
                declaring_trait_arguments: requirement.declaring_trait_arguments.clone(),
                requirement_identity: checked_evidence_requirement_identity(
                    checked,
                    requirement.declaring_trait,
                    requirement.requirement,
                )?,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    requirements.sort();
    requirements.dedup();
    Ok(EvidenceInterfaceIdentity {
        trait_identity: checked.symbols.display_path(interface.trait_symbol, "::"),
        arguments: interface.arguments.to_vec(),
        requirements,
    })
}

pub(crate) fn lower_evidence_contract_lanes(
    checked: &CheckedTrees,
    selected_machine: symbols::SymbolHandle,
    terminal_machine: MachineId,
    term_ids: &[Option<EvidenceTermId>],
) -> Result<Vec<EvidenceContractLane>, LoweringError> {
    let guarded_terms = selected_guarded_evidence_terms(checked, selected_machine);
    let mut lanes = checked
        .facts
        .proof
        .evidence_terms
        .iter()
        .filter_map(|(handle, term)| {
            (term.owner
                == checked_trees::ContractProofFactOwner::Machine {
                    machine_symbol: selected_machine,
                }
                && !guarded_terms.contains(
                    &usize::try_from(handle.arena_index() - 1)
                        .expect("arena indices fit the host address space"),
                ))
            .then_some((handle, term))
        })
        .map(|(handle, term)| {
            let index = usize::try_from(handle.arena_index() - 1)
                .expect("arena indices fit the host address space");
            let term_id =
                term_ids
                    .get(index)
                    .copied()
                    .flatten()
                    .ok_or(LoweringError::Unsupported(
                        "selected terminal contract lane has no evidence-term identity",
                    ))?;
            let kind = match term.kind {
                checked_trees::ContractProofFactKind::Requires => {
                    EvidenceContractLaneKind::Requires
                }
                checked_trees::ContractProofFactKind::Ensures => EvidenceContractLaneKind::Ensures,
            };
            Ok(EvidenceContractLane {
                machine: terminal_machine,
                kind,
                position: u32::try_from(term.lane_position).map_err(|_| {
                    LoweringError::Unsupported(
                        "terminal evidence contract lane position exceeds u32",
                    )
                })?,
                term: term_id,
                output_field: (kind == EvidenceContractLaneKind::Ensures)
                    .then(|| term.name.clone()),
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    lanes.sort_unstable();
    Ok(lanes)
}

fn selected_guarded_evidence_terms(
    checked: &CheckedTrees,
    selected_machine: symbols::SymbolHandle,
) -> BTreeSet<usize> {
    checked
        .facts
        .proof
        .outcome_specific_guarantees
        .iter()
        .filter_map(|(_, guarantee)| {
            (guarantee.machine_symbol == selected_machine)
                .then_some(guarantee.evidence_term)
                .flatten()
                .map(|handle| {
                    usize::try_from(handle.arena_index() - 1)
                        .expect("arena indices fit the host address space")
                })
        })
        .collect()
}
