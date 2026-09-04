//! Checked proof-SCC custody at the source-to-Terminal boundary.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use psi_checked_trees::{
    CheckedProofRankingRelation, CheckedProofRecursiveCallSite, CheckedProofRecursiveComponent,
    CheckedProofRecursiveTransitionLane, CheckedTrees,
    data::{DataDefinition, DataField, DataMember},
    types::TypeReferenceHandle,
};
use psi_core::{ContractId, EvidenceIdentity, ObligationId, RecursiveComponentId};
use psi_proof_admission::{
    CertificateEnvelope, CertificateObligation, EvidenceRoute, ProofNode, ProofRule,
    ProofSystemMarker, RecursiveComponentCertificate, RecursiveEdgeCertificate,
};
use psi_symbols::SymbolHandle;
use psi_terminal::{
    TerminalModule, TerminalProofRankingRelation, TerminalProofRecursiveCallSite,
    TerminalProofRecursiveComponent, TerminalProofRecursiveEdge, TerminalProofRecursiveField,
    TerminalProofRecursiveMember, TerminalProofRecursiveTransitionLane, TerminalProofRecursiveType,
};
use psi_terminal_verifier::{
    ProofBundle, RecursiveComponentEvidence, proof_recursive_component_identity,
    reconstruct_proof_recursive_component_obligations,
};
use sha2::{Digest, Sha256};

use crate::LoweringError;

const CERTIFICATE_IDENTITY_DOMAIN: &[u8] = b"psi.source-proof-recursion.certificate.v1\0";
const ROUTE_IDENTITY_DOMAIN: &[u8] = b"psi.source-proof-recursion.route.v1\0";

const fn symbol_key(symbol: SymbolHandle) -> (u32, u32) {
    (symbol.arena_index(), symbol.generation())
}

/// Retain exactly the checked recursive proof components reachable through
/// the selected source proof closure, then derive evidence for the verifier-
/// reconstructed questions. Frontend arena handles stop at this function.
pub(super) fn lower_and_install_proof_recursion(
    checked: &CheckedTrees,
    source_machines: &[SymbolHandle],
    module: &mut TerminalModule,
    proof_bundle: &mut ProofBundle,
) -> Result<(), LoweringError> {
    let reachable = proof_machine_dependency_closure(checked, source_machines);
    let mut selected = checked
        .facts
        .termination
        .proof_recursive_components
        .iter()
        .filter(|component| {
            component
                .members
                .iter()
                .any(|member| reachable.contains(&symbol_key(member.machine)))
        })
        .map(|component| {
            if !component
                .members
                .iter()
                .all(|member| reachable.contains(&symbol_key(member.machine)))
            {
                return Err(LoweringError::Unsupported(
                    "selected proof closure contains only part of a recursive component",
                ));
            }
            let identities = component
                .members
                .iter()
                .map(|member| hermetic_identity(checked, member.machine, "proof machine"))
                .collect::<Result<Vec<_>, _>>()?;
            Ok((identities, component))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    selected.sort_by(|left, right| left.0.cmp(&right.0));

    let mut next_contract = module
        .machines
        .iter()
        .map(|machine| machine.contract.id.get())
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "proof-recursion contract identity space is exhausted",
        ))?;
    let mut components = Vec::with_capacity(selected.len());
    for (_, component) in selected {
        components.push(lower_component(checked, component, &mut next_contract)?);
    }
    components.sort();
    module.proof_recursive_components = components;

    let reconstructed = reconstruct_proof_recursive_component_obligations(module)
        .map_err(LoweringError::InvalidTerminalModule)?;
    proof_bundle.recursive_components = module
        .proof_recursive_components
        .iter()
        .zip(reconstructed)
        .map(|(component, obligation)| {
            let component_id = proof_recursive_component_identity(component);
            let ranking_relation =
                obligation
                    .ranking_relation
                    .ok_or(LoweringError::Unsupported(
                        "checked proof recursion has no ranking relation",
                    ))?;
            let certificate = RecursiveComponentCertificate {
                identity: certificate_identity(component_id),
                ranking_relation,
                well_foundedness: semantic_axiom_route(
                    component_id,
                    0,
                    &obligation.well_foundedness,
                ),
                edges: obligation
                    .edges
                    .iter()
                    .enumerate()
                    .map(|(index, edge)| RecursiveEdgeCertificate {
                        obligation: edge.decrease.obligation.id,
                        evidence: semantic_axiom_route(
                            component_id,
                            u64::try_from(index)
                                .expect("recursive edge count fits u64")
                                .checked_add(1)
                                .expect("recursive edge route position fits u64"),
                            &edge.decrease,
                        ),
                    })
                    .collect(),
            };
            Ok(RecursiveComponentEvidence {
                component: component_id,
                certificate,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    proof_bundle
        .recursive_components
        .sort_by_key(|component| component.component);
    Ok(())
}

fn proof_machine_dependency_closure(
    checked: &CheckedTrees,
    roots: &[SymbolHandle],
) -> BTreeSet<(u32, u32)> {
    let machines = checked.typed.machines();
    let mut dependency_owners = BTreeMap::new();
    for machine in machines {
        dependency_owners.insert(symbol_key(machine.symbol), machine.symbol);
        for state in checked.typed.machine_states(machine) {
            dependency_owners.insert(symbol_key(state.symbol), machine.symbol);
        }
    }
    let mut reached = BTreeSet::new();
    let mut pending = roots.iter().copied().collect::<VecDeque<_>>();
    while let Some(symbol) = pending.pop_front() {
        let Some(owner) = dependency_owners.get(&symbol_key(symbol)).copied() else {
            continue;
        };
        let owner_key = symbol_key(owner);
        if !reached.insert(owner_key) {
            continue;
        }
        let machine = machines
            .iter()
            .find(|machine| machine.symbol == owner)
            .expect("known machine symbol has a declaration");
        for dependency in psi_validation::machine_call_dependency_symbols(&checked.typed, machine) {
            if let Some(dependency_owner) = dependency_owners.get(&symbol_key(dependency))
                && !reached.contains(&symbol_key(*dependency_owner))
            {
                pending.push_back(*dependency_owner);
            }
        }
    }
    reached
}

fn lower_component(
    checked: &CheckedTrees,
    component: &CheckedProofRecursiveComponent,
    next_contract: &mut u64,
) -> Result<TerminalProofRecursiveComponent, LoweringError> {
    let mut source_members = component
        .members
        .iter()
        .map(|member| {
            Ok((
                hermetic_identity(checked, member.machine, "proof machine")?,
                hermetic_identity(checked, member.rank_parameter, "proof rank parameter")?,
                member,
            ))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    source_members.sort_by(|left, right| (&left.0, &left.1).cmp(&(&right.0, &right.1)));

    let mut member_bindings = BTreeMap::new();
    let mut members = Vec::with_capacity(source_members.len());
    for (machine_identity, rank_parameter_identity, source) in source_members {
        let contract = ContractId::new(*next_contract).ok_or(LoweringError::Unsupported(
            "proof-recursion contract identity is reserved",
        ))?;
        *next_contract = next_contract
            .checked_add(1)
            .ok_or(LoweringError::Unsupported(
                "proof-recursion contract identity space is exhausted",
            ))?;
        let rank_type = rank_parameter_type(checked, source.machine, source.rank_parameter).ok_or(
            LoweringError::Unsupported("checked proof rank parameter type is absent"),
        )?;
        if checked
            .typed
            .package_qualified_type_identity(rank_type)
            .as_str()
            != component.rank_type_identity
        {
            return Err(LoweringError::Unsupported(
                "checked proof component member rank type is stale",
            ));
        }
        member_bindings.insert(
            symbol_key(source.machine),
            (contract, source.rank_parameter, rank_type),
        );
        members.push(TerminalProofRecursiveMember {
            contract,
            machine_identity,
            rank_parameter_identity,
        });
    }

    let root_type = component
        .members
        .first()
        .and_then(|member| rank_parameter_type(checked, member.machine, member.rank_parameter))
        .ok_or(LoweringError::Unsupported(
            "checked proof rank parameter type is absent",
        ))?;
    if checked
        .typed
        .package_qualified_type_identity(root_type)
        .as_str()
        != component.rank_type_identity
    {
        return Err(LoweringError::Unsupported(
            "checked proof rank type identity is stale",
        ));
    }

    let mut type_fields = BTreeMap::<String, BTreeMap<String, String>>::new();
    type_fields
        .entry(component.rank_type_identity.clone())
        .or_default();
    let mut edges = Vec::with_capacity(component.edges.len());
    for edge in &component.edges {
        let (caller, caller_rank_parameter, source_rank_type) = member_bindings
            .get(&symbol_key(edge.caller))
            .copied()
            .ok_or(LoweringError::Unsupported(
                "checked recursive caller is outside its component",
            ))?;
        let (callee, callee_rank_parameter, _) = member_bindings
            .get(&symbol_key(edge.callee))
            .copied()
            .ok_or(LoweringError::Unsupported(
                "checked recursive callee is outside its component",
            ))?;
        if edge.caller_rank_parameter != caller_rank_parameter
            || edge.callee_rank_parameter != callee_rank_parameter
        {
            return Err(LoweringError::Unsupported(
                "checked recursive edge rank parameter is stale",
            ));
        }
        let (strict_member_path, final_type) = lower_member_path(
            checked,
            source_rank_type,
            &edge.strict_member_path,
            &mut type_fields,
        )?;
        if checked
            .typed
            .package_qualified_type_identity(final_type)
            .as_str()
            != component.rank_type_identity
        {
            return Err(LoweringError::Unsupported(
                "checked recursive member path does not return to the rank type",
            ));
        }
        edges.push(TerminalProofRecursiveEdge {
            caller,
            callee,
            site: lower_call_site(checked, edge.site)?,
            strict_member_path,
        });
    }
    edges.sort();

    let types = type_fields
        .into_iter()
        .map(|(identity, fields)| TerminalProofRecursiveType {
            identity,
            fields: fields
                .into_iter()
                .map(|(identity, type_identity)| TerminalProofRecursiveField {
                    identity,
                    type_identity,
                })
                .collect(),
        })
        .collect();
    Ok(TerminalProofRecursiveComponent {
        ranking_relation: match component.ranking_relation {
            CheckedProofRankingRelation::StructuralSubterm => {
                TerminalProofRankingRelation::StructuralSubterm
            }
        },
        rank_type_identity: component.rank_type_identity.clone(),
        types,
        members,
        edges,
    })
}

fn rank_parameter_type(
    checked: &CheckedTrees,
    machine_symbol: SymbolHandle,
    parameter_symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)?;
    checked
        .typed
        .machine_states(machine)
        .iter()
        .flat_map(|state| checked.typed.state_parameters(state))
        .find(|parameter| parameter.symbol == parameter_symbol)
        .map(|parameter| parameter.type_reference)
}

fn lower_member_path(
    checked: &CheckedTrees,
    mut current_type: TypeReferenceHandle,
    path: &[SymbolHandle],
    types: &mut BTreeMap<String, BTreeMap<String, String>>,
) -> Result<(Vec<String>, TypeReferenceHandle), LoweringError> {
    let mut identities = Vec::with_capacity(path.len());
    for field_symbol in path {
        let owner_symbol = checked
            .typed
            .type_reference_table
            .type_reference(current_type)
            .type_symbol(&checked.typed.type_reference_table);
        let (_, field) = exact_field(checked, owner_symbol, *field_symbol).ok_or(
            LoweringError::Unsupported("checked recursive field path is stale"),
        )?;
        let owner_identity = checked
            .typed
            .package_qualified_type_identity(current_type)
            .into_string();
        let field_identity = hermetic_identity(checked, *field_symbol, "proof recursive field")?;
        let target_identity = checked
            .typed
            .package_qualified_type_identity(field.type_reference)
            .into_string();
        if let Some(previous) = types
            .entry(owner_identity)
            .or_default()
            .insert(field_identity.clone(), target_identity)
            && previous
                != checked
                    .typed
                    .package_qualified_type_identity(field.type_reference)
                    .as_str()
        {
            return Err(LoweringError::Unsupported(
                "proof recursive field identity has conflicting target types",
            ));
        }
        identities.push(field_identity);
        current_type = field.type_reference;
    }
    Ok((identities, current_type))
}

fn exact_field(
    checked: &CheckedTrees,
    owner_symbol: SymbolHandle,
    field_symbol: SymbolHandle,
) -> Option<(&DataDefinition, &DataField)> {
    let data = checked
        .typed
        .data_definitions()
        .iter()
        .find(|data| data.symbol == owner_symbol)?;
    let mut found = None;
    for member in checked.typed.data_members(data) {
        match member {
            DataMember::Field(field) if field.symbol == field_symbol => {
                if found.replace(field).is_some() {
                    return None;
                }
            }
            DataMember::Variant(variant) => {
                for field in checked.typed.data_payload_fields(variant) {
                    if field.symbol == field_symbol && found.replace(field).is_some() {
                        return None;
                    }
                }
            }
            DataMember::Field(_) => {}
        }
    }
    found.map(|field| (data, field))
}

fn lower_call_site(
    checked: &CheckedTrees,
    site: CheckedProofRecursiveCallSite,
) -> Result<TerminalProofRecursiveCallSite, LoweringError> {
    let index = |value: usize| {
        u64::try_from(value).map_err(|_| {
            LoweringError::Unsupported("proof recursive call-site coordinate exceeds u64")
        })
    };
    Ok(match site {
        CheckedProofRecursiveCallSite::Statement {
            state,
            statement_index,
        } => TerminalProofRecursiveCallSite::Statement {
            state_identity: hermetic_identity(checked, state, "proof recursive state")?,
            statement_index: index(statement_index)?,
        },
        CheckedProofRecursiveCallSite::Expression {
            state,
            statement_index,
            expression_ordinal,
        } => TerminalProofRecursiveCallSite::Expression {
            state_identity: hermetic_identity(checked, state, "proof recursive state")?,
            statement_index: index(statement_index)?,
            expression_ordinal: index(expression_ordinal)?,
        },
        CheckedProofRecursiveCallSite::Transition {
            state,
            statement_index,
            lane,
        } => TerminalProofRecursiveCallSite::Transition {
            state_identity: hermetic_identity(checked, state, "proof recursive state")?,
            statement_index: index(statement_index)?,
            lane: match lane {
                CheckedProofRecursiveTransitionLane::Target => {
                    TerminalProofRecursiveTransitionLane::Target
                }
                CheckedProofRecursiveTransitionLane::Continuation => {
                    TerminalProofRecursiveTransitionLane::Continuation
                }
            },
        },
    })
}

fn hermetic_identity(
    checked: &CheckedTrees,
    symbol: SymbolHandle,
    subject: &'static str,
) -> Result<String, LoweringError> {
    checked
        .typed
        .normalized_hermetic_symbol_identity(symbol)
        .map_err(|_| match subject {
            "proof machine" => {
                LoweringError::Unsupported("proof machine has no hermetic declaration identity")
            }
            "proof rank parameter" => LoweringError::Unsupported(
                "proof rank parameter has no hermetic declaration identity",
            ),
            "proof recursive field" => LoweringError::Unsupported(
                "proof recursive field has no hermetic declaration identity",
            ),
            _ => LoweringError::Unsupported(
                "proof recursive state has no hermetic declaration identity",
            ),
        })
}

fn certificate_identity(component: RecursiveComponentId) -> EvidenceIdentity {
    derived_evidence_identity(CERTIFICATE_IDENTITY_DOMAIN, component, 0, None)
}

fn semantic_axiom_route(
    component: RecursiveComponentId,
    position: u64,
    obligation: &CertificateObligation,
) -> EvidenceRoute {
    EvidenceRoute::CertificateDerived(CertificateEnvelope {
        identity: derived_evidence_identity(
            ROUTE_IDENTITY_DOMAIN,
            component,
            position,
            Some(obligation.obligation.id),
        ),
        proof_system_marker: ProofSystemMarker::CURRENT,
        proof: ProofNode {
            conclusion: obligation.obligation.proposition.clone(),
            rule: ProofRule::SemanticAxiom { index: 0 },
        },
    })
}

fn derived_evidence_identity(
    domain: &[u8],
    component: RecursiveComponentId,
    position: u64,
    obligation: Option<ObligationId>,
) -> EvidenceIdentity {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(component.get().to_le_bytes());
    digest.update(position.to_le_bytes());
    if let Some(obligation) = obligation {
        digest.update(obligation.get().to_le_bytes());
    }
    let digest: [u8; 32] = digest.finalize().into();
    EvidenceIdentity::new(u64::from_le_bytes(digest[..8].try_into().unwrap()) | 1)
        .expect("forcing the low bit makes the evidence identity nonzero")
}
