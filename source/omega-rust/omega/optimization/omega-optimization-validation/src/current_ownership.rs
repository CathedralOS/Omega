use std::collections::{BTreeMap, BTreeSet};

use omega_optimization_unit::{OptimizationBlock, PsiOptimizationFunction};
use omega_terminal_abstract_operations::TerminalAbstractOperation as O;
use psi_core::{BlockId, ClaimId, PlaceId};
use psi_terminal::{StructuralMultiplicity, StructuralPathSegment};

use crate::OptimizationUnitValidationError;

/// Claim state reconstructed only from the current executable CFG. This is
/// deliberately separate from immutable verifier-frontier custody and from
/// cached `OwnershipEvent` rows.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LiveClaim {
    input: Option<PlaceId>,
    path: Vec<StructuralPathSegment>,
    multiplicity: Option<StructuralMultiplicity>,
}

pub(super) fn validate_current_claim_frontier(
    function: &PsiOptimizationFunction,
    blocks: &BTreeMap<BlockId, &OptimizationBlock>,
    successors: &BTreeMap<BlockId, Vec<BlockId>>,
) -> Result<(), OptimizationUnitValidationError> {
    let mut claims = BTreeMap::<ClaimId, LiveClaim>::new();
    for claim in &function.entry_claim_declarations {
        let parameter = function
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == claim.input)
            .expect("structural signature validation precedes current claim replay");
        claims.insert(
            claim.claim,
            LiveClaim {
                input: Some(claim.input),
                path: claim.path.clone(),
                multiplicity: Some(if claim.path.is_empty() {
                    parameter.multiplicity
                } else {
                    StructuralMultiplicity::Linear
                }),
            },
        );
    }
    for claim in &function.content_entry_claims {
        let parameter = function
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == claim.input.root);
        claims.entry(claim.claim).or_insert(LiveClaim {
            input: parameter.map(|_| claim.input.root),
            path: Vec::new(),
            multiplicity: parameter.map(|parameter| parameter.multiplicity),
        });
    }

    let mut predecessor_edges = blocks
        .keys()
        .map(|block| (*block, 0usize))
        .collect::<BTreeMap<_, _>>();
    for targets in successors.values() {
        for target in targets {
            *predecessor_edges
                .get_mut(target)
                .expect("CFG validation precedes current claim replay") += 1;
        }
    }
    let mut ready = predecessor_edges
        .iter()
        .filter_map(|(block, count)| (*count == 0).then_some(*block))
        .collect::<BTreeSet<_>>();
    let mut incoming = BTreeMap::<BlockId, Vec<BTreeMap<ClaimId, LiveClaim>>>::new();
    incoming.insert(function.entry, vec![claims]);

    while let Some(block_id) = ready.pop_first() {
        let frontiers = incoming
            .remove(&block_id)
            .expect("total reachable acyclic CFG has a current claim frontier");
        let mut claims = frontiers
            .first()
            .expect("reachable block has an incoming claim frontier")
            .clone();
        if frontiers.iter().any(|frontier| frontier != &claims) {
            return Err(OptimizationUnitValidationError::CurrentClaimJoinMismatch {
                machine: function.machine,
                block: block_id,
            });
        }

        let block = blocks[&block_id];
        for (node_index, node) in block.nodes.iter().enumerate() {
            let node_index =
                u32::try_from(node_index).expect("optimization-unit node position fits u32");
            let transferred = match &node.operation {
                O::CallUnit {
                    claim_transfers, ..
                }
                | O::CallStructuralScalar {
                    claim_transfers, ..
                }
                | O::CallStructural {
                    claim_transfers, ..
                } => claim_transfers
                    .iter()
                    .map(|transfer| transfer.claim)
                    .collect::<Vec<_>>(),
                O::BoundaryCall {
                    completion_receipts,
                    ..
                } => completion_receipts
                    .iter()
                    .map(|receipt| receipt.claim)
                    .collect(),
                _ => Vec::new(),
            };
            for claim in transferred {
                if claims.remove(&claim).is_none() {
                    return Err(OptimizationUnitValidationError::CurrentClaimNotLive {
                        machine: function.machine,
                        block: block_id,
                        node: node_index,
                        claim,
                    });
                }
            }

            let structural_result = match &node.operation {
                O::EstablishPayloadlessCase { result, .. } | O::CallStructural { result, .. } => {
                    Some(result)
                }
                _ => None,
            };
            if let Some(result) = structural_result {
                for binding in &result.claims {
                    let claim = LiveClaim {
                        input: Some(result.place),
                        path: binding.path.clone(),
                        multiplicity: Some(if binding.path.is_empty() {
                            result.multiplicity
                        } else {
                            StructuralMultiplicity::Linear
                        }),
                    };
                    if claims.insert(binding.claim, claim).is_some() {
                        return Err(OptimizationUnitValidationError::CurrentClaimAlreadyLive {
                            machine: function.machine,
                            block: block_id,
                            node: node_index,
                            claim: binding.claim,
                        });
                    }
                }
            }

            match &node.operation {
                O::Return { .. } | O::ReturnUnit { .. } => {
                    if let Some(claim) = claims.iter().find_map(|(claim, live)| {
                        (live.multiplicity == Some(StructuralMultiplicity::Linear))
                            .then_some(*claim)
                    }) {
                        return Err(
                            OptimizationUnitValidationError::CurrentLinearClaimAtReturn {
                                machine: function.machine,
                                block: block_id,
                                claim,
                            },
                        );
                    }
                }
                O::ReturnStructural {
                    source,
                    returned_claims,
                    ..
                } => {
                    let expected = claims
                        .iter()
                        .filter_map(|(claim, live)| (live.input == Some(*source)).then_some(*claim))
                        .collect::<Vec<_>>();
                    if returned_claims != &expected {
                        return Err(
                            OptimizationUnitValidationError::CurrentStructuralReturnClaimSetMismatch {
                                machine: function.machine,
                                block: block_id,
                            },
                        );
                    }
                    for claim in returned_claims {
                        claims.remove(claim);
                    }
                    if let Some(claim) = claims.keys().next().copied() {
                        return Err(
                            OptimizationUnitValidationError::CurrentClaimLiveAfterStructuralReturn {
                                machine: function.machine,
                                block: block_id,
                                claim,
                            },
                        );
                    }
                }
                O::Crash {
                    frontier_lower_bound,
                    ..
                } => {
                    if frontier_lower_bound != &claims.keys().copied().collect::<Vec<_>>() {
                        return Err(
                            OptimizationUnitValidationError::CurrentCrashClaimFrontierMismatch {
                                machine: function.machine,
                                block: block_id,
                            },
                        );
                    }
                }
                _ => {}
            }
        }

        for edge in &block
            .nodes
            .last()
            .expect("validated block is nonempty")
            .successors
        {
            incoming
                .entry(edge.target)
                .or_default()
                .push(claims.clone());
            let count = predecessor_edges
                .get_mut(&edge.target)
                .expect("validated edge target is indexed");
            *count -= 1;
            if *count == 0 {
                ready.insert(edge.target);
            }
        }
    }

    Ok(())
}
