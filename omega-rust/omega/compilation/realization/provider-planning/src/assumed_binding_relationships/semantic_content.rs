//! Canonical content of the already produced directional semantic obligations.
//!
//! Public contract commitments own proposition content. Coordinates below refer
//! to authored clauses/facts or subject qualification layers in those contracts,
//! never arena indices or diagnostic names. Each direction retains its complete
//! premise roster and each goal's produced judgment, including refutation.
//! This component does not certify the supplier dependency closure, establish a
//! domain claim, accept the assumption, or install a runtime relationship.

use super::declarations::{DeclaredRequirement, PublicRequirement};
use super::{
    AssumedBindingContractSide, AssumedBindingPublicClaim, AssumedBindingSemanticConnection,
    AssumedBindingSemanticJudgment, AssumedBindingSemanticSource, AssumedBindingSemanticSubject,
};
use crate::digest_encoder::DomainDigestEncoder;
use arena::{Handle, HandleSpan};
use checked_trees::typed_trees::domain::ProofFact;
use checked_trees::typed_trees::signature::{SignatureContract, SignatureContractKind};
use checked_trees::typed_trees::types::TypeReferenceNode;
use checked_trees::{CheckedTrees, ContractProofFactKind, ContractProofFactOwner};

/// Exact public difference content, separate from the full relationship digest.
/// Construction is private to the original semantic-connection producer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssumedBindingSemanticContent([u8; 32]);

impl AssumedBindingSemanticContent {
    pub const fn digest(self) -> [u8; 32] {
        self.0
    }
}

pub(super) fn capture(
    program: &CheckedTrees,
    required: &DeclaredRequirement<'_>,
    supplied: &DeclaredRequirement<'_>,
    required_contract: [u8; 32],
    supplied_contract: [u8; 32],
    connection: &AssumedBindingSemanticConnection,
) -> Option<AssumedBindingSemanticContent> {
    let mut encoder = DomainDigestEncoder::new(b"omega.assumed-binding-semantic-obligations.v1");
    encoder.raw(&required_contract);
    encoder.raw(&supplied_contract);
    for (direction, obligations) in [(1, connection.entry()), (2, connection.exit())] {
        encoder.byte(direction);
        encoder.u64(obligations.premises().len() as u64);
        for claim in obligations.premises() {
            encode_claim(&mut encoder, program, required, supplied, claim)?;
        }
        encoder.u64(obligations.obligations().len() as u64);
        for obligation in obligations.obligations() {
            encode_claim(&mut encoder, program, required, supplied, obligation.goal())?;
            encoder.byte(match obligation.judgment() {
                AssumedBindingSemanticJudgment::Proven => 1,
                AssumedBindingSemanticJudgment::Assumed => 2,
                AssumedBindingSemanticJudgment::Refuted => 3,
            });
        }
    }
    Some(AssumedBindingSemanticContent(encoder.finish()))
}

fn encode_claim(
    encoder: &mut DomainDigestEncoder,
    program: &CheckedTrees,
    required: &DeclaredRequirement<'_>,
    supplied: &DeclaredRequirement<'_>,
    claim: &AssumedBindingPublicClaim,
) -> Option<()> {
    let declaration = match claim.side() {
        AssumedBindingContractSide::Required => {
            encoder.byte(1);
            required
        }
        AssumedBindingContractSide::Supplied => {
            encoder.byte(2);
            supplied
        }
    };
    match claim.subject() {
        AssumedBindingSemanticSubject::Parameter { ordinal, symbol } => {
            if declaration.parameters.get(ordinal)?.symbol != symbol {
                return None;
            }
            encoder.byte(1);
            encoder.u64(ordinal as u64);
        }
        AssumedBindingSemanticSubject::Result => encoder.byte(2),
        AssumedBindingSemanticSubject::Predicate => encoder.byte(3),
    }
    let (first, second) = public_contracts(declaration);
    match claim.source() {
        AssumedBindingSemanticSource::Absent => return None,
        AssumedBindingSemanticSource::Contract(handle) => {
            let checked = program.facts.proof.contract_facts.get(handle);
            if !super::semantic_obligations::owns_contract(declaration, checked.owner) {
                return None;
            }
            let (scope, contracts) = match checked.owner {
                ContractProofFactOwner::MachineState { .. } => (1, second),
                ContractProofFactOwner::Machine { .. }
                | ContractProofFactOwner::StateSignature { .. } => (0, first),
                _ => return None,
            };
            let (clause, fact) = fact_coordinate(program, contracts, checked.kind, checked.fact)?;
            encoder.byte(1);
            encoder.byte(scope);
            encoder.u64(clause as u64);
            encoder.u64(fact as u64);
        }
        AssumedBindingSemanticSource::ResultCase { contract, fact } => {
            let mut coordinates = first
                .handles()
                .enumerate()
                .map(|(ordinal, candidate)| (0, ordinal, candidate))
                .chain(
                    second
                        .handles()
                        .enumerate()
                        .map(|(ordinal, candidate)| (1, ordinal, candidate)),
                )
                .filter(|(_, _, candidate)| *candidate == contract);
            let (scope, clause, _) = coordinates.next()?;
            if coordinates.next().is_some() {
                return None;
            }
            let authored = program.signature_contracts.get(contract);
            let SignatureContractKind::EnsuresForResultCase {
                result_data,
                result_case,
            } = authored.kind
            else {
                return None;
            };
            let guard = claim.result_case()?;
            if guard.data != result_data || guard.case != result_case {
                return None;
            }
            let fact = authored
                .facts
                .handles()
                .position(|candidate| candidate == fact)?;
            encoder.byte(2);
            encoder.byte(scope);
            encoder.u64(clause as u64);
            encoder.u64(fact as u64);
        }
        AssumedBindingSemanticSource::DeclaredType {
            type_reference,
            constraint_ordinal,
        } => {
            let mut current = match claim.subject() {
                AssumedBindingSemanticSubject::Parameter { ordinal, .. } => {
                    declaration.parameters.get(ordinal)?.type_reference
                }
                AssumedBindingSemanticSubject::Result => declaration.result,
                AssumedBindingSemanticSubject::Predicate => return None,
            };
            let mut depth = 0u64;
            let mut visited = Vec::new();
            loop {
                if !current.is_valid() || visited.contains(&current) {
                    return None;
                }
                visited.push(current);
                let node = program.type_reference_table.type_reference(current);
                if current == type_reference {
                    let TypeReferenceNode::Constrained { constraints, .. } = node else {
                        return None;
                    };
                    program
                        .type_reference_table
                        .constraints(*constraints)
                        .get(constraint_ordinal)?;
                    break;
                }
                current = match node {
                    TypeReferenceNode::Constrained { base_type, .. } => *base_type,
                    TypeReferenceNode::Reference { referee, .. } => *referee,
                    _ => return None,
                };
                depth += 1;
            }
            encoder.byte(3);
            encoder.u64(depth);
            encoder.u64(constraint_ordinal as u64);
        }
    }
    Some(())
}

fn public_contracts(
    declaration: &DeclaredRequirement<'_>,
) -> (HandleSpan<SignatureContract>, HandleSpan<SignatureContract>) {
    match declaration.declaration {
        PublicRequirement::Trait { signature, .. } => (signature.contracts, HandleSpan::empty()),
        PublicRequirement::Machine { declaration, entry } => {
            (declaration.contracts, entry.contracts)
        }
    }
}

fn fact_coordinate(
    program: &CheckedTrees,
    contracts: HandleSpan<SignatureContract>,
    kind: ContractProofFactKind,
    fact: Handle<ProofFact>,
) -> Option<(usize, usize)> {
    let mut coordinates = contracts
        .handles()
        .enumerate()
        .flat_map(|(clause, handle)| {
            let contract = program.signature_contracts.get(handle);
            let same_kind = matches!(
                (kind, &contract.kind),
                (
                    ContractProofFactKind::Requires,
                    SignatureContractKind::Requires
                ) | (
                    ContractProofFactKind::Ensures,
                    SignatureContractKind::Ensures
                )
            );
            contract
                .facts
                .handles()
                .enumerate()
                .filter(move |(_, candidate)| same_kind && *candidate == fact)
                .map(move |(ordinal, _)| (clause, ordinal))
        });
    let coordinate = coordinates.next()?;
    // A reused fact without its authored clause association is unavailable;
    // never pick the first same-handle row and call it an exact coordinate.
    coordinates.next().is_none().then_some(coordinate)
}
