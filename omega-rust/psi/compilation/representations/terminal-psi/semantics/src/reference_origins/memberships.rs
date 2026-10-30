//! Caller facts preserved by completed reference identity, not by a callee's
//! private result contract. One typed leaf relation covers direct references,
//! records and arrays. Capture possible input endpoints before moves, read their
//! facts after normal call effects, and meet every possible origin. This is a
//! read-only evidence query: it creates no address, access, loan or claim.
//!
//! Public result rows remain establishment guarantees. Additional rows describe
//! only caller evidence still true of the same shared referent at completion.
//! Exclusive results retain their declared restoration debts; attaching an
//! incidental current value to them would invent a new mutation invariant.
use super::{ReferenceCallContract, ReferenceCallOrigin, ReferenceCallResult};
use crate::descriptor_lifetimes::DescriptorPosition;
use crate::descriptor_results::{
    self as descriptors, DescriptorAnalysis, DescriptorMachine, PayloadMembershipContract,
};
use semantic_vocabulary::{MachineId, PlaceId, ReferenceOriginExtent, StructuralTypeId};
use terminal_psi::{
    Operation, ReferenceOriginProjection, StructuralAccess, StructuralOperationResult,
    StructuralPathQualification, StructuralPathSegment, StructuralResultDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine, TerminalModule,
};

/// Disposable batch owner. Its graph borrows no evolving result roster; callers
/// drop this context before publishing a retention sweep's new operation rows.
pub struct ReferenceResultMemberships<'module> {
    module: &'module TerminalModule,
    graph: std::cell::OnceCell<Vec<DescriptorMachine>>,
}

impl<'module> ReferenceResultMemberships<'module> {
    pub fn new(module: &'module TerminalModule) -> Self {
        Self {
            module,
            graph: std::cell::OnceCell::new(),
        }
    }

    pub fn for_operation(
        &self,
        caller: &TerminalMachine,
        operation: &Operation,
    ) -> Result<Vec<StructuralPathQualification>, &'static str> {
        let (terminal_psi::call_structural_pair_pattern!(callee, structural_arguments, ..)) =
            &operation.kind
        else {
            // Public opaque routes retain only their declared guarantees. Their
            // WithinRegion relation supplies no exact incidental endpoint fact.
            return Ok(Vec::new());
        };
        let caller = self
            .module
            .machine(caller.id)
            .ok_or("reference call has no caller")?;
        let result = operation
            .result
            .structural()
            .ok_or("reference call has no result")?;
        let callee = self
            .module
            .machine(*callee)
            .ok_or("reference call has no callee")?;
        let signature = callee
            .result
            .structural()
            .ok_or("reference callee has no result")?;
        if signature.reference_sources.is_empty() {
            return Ok(Vec::new());
        }
        if result.structural_type != signature.structural_type
            || result.multiplicity != signature.multiplicity
        {
            return Err("reference call result disagrees with its public carrier");
        }
        let mut positions = caller.blocks.iter().flat_map(|block| {
            block
                .operations
                .iter()
                .enumerate()
                .filter_map(move |(ordinal, candidate)| {
                    (candidate.id == operation.id && candidate == operation)
                        .then_some((block.id, ordinal))
                })
        });
        let (block, ordinal) = positions
            .next()
            .ok_or("reference call has no current-body cut")?;
        if positions.next().is_some() {
            return Err("reference call has ambiguous current-body cuts");
        }
        let position = (
            block,
            u32::try_from(ordinal).map_err(|_| "call ordinal overflows")?,
        );
        let mappings = super::bind_reference_call_results(
            self.module.structural_types.iter(),
            ReferenceCallContract::Machine {
                parameters: &callee.structural_parameters,
                sources: &signature.reference_sources,
            },
            structural_arguments,
        )?;
        preserve_call_memberships(
            &self.module.structural_types,
            &self.module.structural_domains,
            || {
                self.graph
                    .get_or_init(|| descriptors::terminal_borrowed_storage_graph(self.module))
                    .as_slice()
            },
            caller.id,
            position,
            result,
            signature,
            &mappings,
            &|place| descriptors::terminal_payload_contract(caller, place),
        )
    }
}

/// Adapter-neutral normal-completion operation. Terminal and current optimized
/// bodies supply the same typed flow and origin binding, so neither keeps a
/// frozen source summary after transformations. Unknown candidates prevent a
/// leaf's additional facts; they are never filtered out of its intersection.
#[allow(clippy::too_many_arguments)]
pub fn reference_call_preserved_memberships(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    graph: &[DescriptorMachine],
    identity: MachineId,
    position: DescriptorPosition,
    result: &StructuralOperationResult,
    declaration: &StructuralResultDeclaration,
    mappings: &[ReferenceCallResult],
    source: &impl Fn(PlaceId) -> Option<PayloadMembershipContract>,
) -> Result<Vec<StructuralPathQualification>, &'static str> {
    preserve_call_memberships(
        types,
        domains,
        || graph,
        identity,
        position,
        result,
        declaration,
        mappings,
        source,
    )
}

#[allow(clippy::too_many_arguments)]
fn preserve_call_memberships<'input>(
    types: &'input [StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    graph: impl Fn() -> &'input [DescriptorMachine],
    identity: MachineId,
    position: DescriptorPosition,
    result: &StructuralOperationResult,
    declaration: &StructuralResultDeclaration,
    mappings: &[ReferenceCallResult],
    source: &impl Fn(PlaceId) -> Option<PayloadMembershipContract>,
) -> Result<Vec<StructuralPathQualification>, &'static str> {
    if result.structural_type != declaration.structural_type
        || result.multiplicity != declaration.multiplicity
    {
        return Err("preserved reference result has a different declared carrier");
    }
    // Mask this receiving annotation at every dynamic instance of the static
    // operation, including loop backedges. A pre-call endpoint may still name
    // an older result occurrence; only the public declaration can seed that
    // producer while proving this operation's additional caller rows. This
    // prevents a result annotation from proving itself through payload history.
    let replay_source = |place| {
        if place == result.place {
            Some(PayloadMembershipContract {
                scalar_observations: Vec::new(),
                scalar_requirements: Vec::new(),
                carrier: declaration.structural_type,
                qualifications: declaration.qualifications.clone(),
                projected_qualifications: declaration.projected_qualifications.clone(),
            })
        } else {
            source(place)
        }
    };
    // Unsupported leaves still validate before any graph adaptation is needed.
    let mut replay = None;
    let mut retained = Vec::new();
    for (ordinal, mapping) in mappings.iter().enumerate() {
        if mappings[..ordinal]
            .iter()
            .any(|previous| previous.path == mapping.path)
        {
            return Err("returned reference membership repeats an output leaf");
        }
        let Some(leaf) =
            crate::runtime_structural_path_tip(types.iter(), result.structural_type, &mapping.path)
        else {
            return Err("returned reference membership has no typed leaf");
        };
        let StructuralTypeShape::Reference {
            referent,
            access: StructuralAccess::SharedBorrow,
        } = leaf.shape
        else {
            continue;
        };
        let mut common: Option<PayloadMembershipContract> = None;
        let mut complete = !mapping.sources.is_empty();
        for candidate in &mapping.sources {
            let selected = (|| {
                if candidate.extent != ReferenceOriginExtent::Projected {
                    return None;
                }
                let ReferenceCallOrigin::Parameter { source: actual, .. } = &candidate.origin
                else {
                    return None;
                };
                let root = replay_source(actual.place)?;
                let mut path = actual.path.clone();
                path.extend(candidate.suffix.iter().map(|projection| match projection {
                    ReferenceOriginProjection::Exact(segment) => segment.clone(),
                    ReferenceOriginProjection::Element => StructuralPathSegment::AllElements,
                }));
                // A final Referent at ordinary storage is an origin marker,
                // while one at a reference carrier is a real pointer crossing.
                if let Some((StructuralPathSegment::Referent, prefix)) = path.split_last()
                    && crate::qualification_path_carrier(types.iter(), root.carrier, prefix)
                        .and_then(|carrier| terminal_psi::find_structural_type(types, carrier))
                        .is_some_and(|row| {
                            !matches!(row.shape, StructuralTypeShape::Reference { .. })
                        })
                {
                    path.pop();
                }
                let query = descriptors::descriptor_contents_query(
                    types,
                    actual.place,
                    root.carrier,
                    &path,
                    None,
                )?;
                let (graph, analysis) = replay.get_or_insert_with(|| {
                    let graph = graph();
                    (
                        graph,
                        DescriptorAnalysis::with_borrowed_storage_slots(types, graph),
                    )
                });
                let targets = analysis.payload_targets(identity, position, &query)?;
                descriptors::payload_memberships(types, domains, targets, |root| {
                    descriptors::completed_payload_contract(
                        types,
                        domains,
                        graph,
                        identity,
                        position,
                        root,
                        &replay_source,
                    )
                    .ok()
                })?
                .presented(types, domains, referent)
            })();
            let Some(selected) = selected else {
                complete = false;
                break;
            };
            if let Some(common) = &mut common {
                common
                    .qualifications
                    .retain(|domain| selected.qualifications.contains(domain));
                common
                    .projected_qualifications
                    .retain(|row| selected.projected_qualifications.contains(row));
            } else {
                common = Some(selected);
            }
        }
        if complete && let Some(common) = common {
            let mut destination = mapping.path.clone();
            destination.push(StructuralPathSegment::Referent);
            StructuralPathQualification::append_under(
                &common.qualifications,
                &common.projected_qualifications,
                &destination,
                &mut retained,
            );
        }
    }
    retained.sort();
    retained.dedup();
    Ok(retained)
}

/// Shared referent observations may be forgotten at a public result edge.
/// Other projected obligations, including exclusive restoration debts, remain
/// exact. This attenuates evidence only; it never adds a public result fact or
/// changes a reference's backing, access or custody.
pub fn reference_result_memberships_weaken<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    carrier: StructuralTypeId,
    source: &[StructuralPathQualification],
    declared: &[StructuralPathQualification],
) -> bool {
    declared.iter().all(|row| source.contains(row))
        && source.iter().all(|row| {
            if declared.contains(row) {
                return true;
            }
            let mut crosses_shared = false;
            for (index, segment) in row.path.iter().enumerate() {
                if *segment != StructuralPathSegment::Referent {
                    continue;
                }
                let Some(declaration) =
                    crate::qualification_path_carrier(types.clone(), carrier, &row.path[..index])
                        .and_then(|carrier| crate::structural_declaration(types.clone(), carrier))
                else {
                    return false;
                };
                match declaration.shape {
                    StructuralTypeShape::Reference {
                        access: StructuralAccess::SharedBorrow,
                        ..
                    } => {
                        crosses_shared = true;
                    }
                    // A shared ancestor does not erase an exclusive
                    // descendant's restoration obligation. Every crossed
                    // reference edge must be observational before this row
                    // can be forgotten at the public result boundary.
                    _ => return false,
                }
            }
            crosses_shared
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{BlockId, ScalarType};
    use std::cell::Cell;
    use terminal_psi::{StructuralArgument, StructuralMultiplicity, TerminalIdentityIndex};

    fn declarations(
        access: StructuralAccess,
    ) -> (
        Vec<StructuralTypeDeclaration>,
        StructuralOperationResult,
        StructuralResultDeclaration,
    ) {
        let scalar = StructuralTypeId::new(1).unwrap();
        let reference = StructuralTypeId::new(2).unwrap();
        let types = [
            (
                scalar,
                StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            ),
            (
                reference,
                StructuralTypeShape::Reference {
                    referent: scalar,
                    access,
                },
            ),
        ]
        .into_iter()
        .map(|(id, shape)| StructuralTypeDeclaration {
            quotient_carrier: None,
            id,
            identity: TerminalIdentityIndex(1),
            owns_linear_claim: false,
            copyable: false,
            no_code_affine_cleanup: true,
            descriptor_cleanup: true,
            shape,
            scalar_restriction: None,
        })
        .collect();
        let result = StructuralOperationResult {
            place: PlaceId::new(99).unwrap(),
            structural_type: reference,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        };
        let declaration = StructuralResultDeclaration {
            place: result.place,
            structural_type: reference,
            multiplicity: result.multiplicity,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            reference_sources: Vec::new(),
        };
        (types, result, declaration)
    }

    fn mapping(extent: ReferenceOriginExtent) -> ReferenceCallResult {
        ReferenceCallResult {
            path: Vec::new(),
            sources: vec![super::super::ReferenceCallSource {
                origin: ReferenceCallOrigin::Parameter {
                    source: StructuralArgument {
                        place: PlaceId::new(3).unwrap(),
                        path: Vec::new(),
                        access: StructuralAccess::SharedBorrow,
                    },
                    transfers_existing: false,
                },
                suffix: Vec::new(),
                endpoint: StructuralTypeId::new(1).unwrap(),
                extent,
            }],
        }
    }

    fn observe(
        types: &[StructuralTypeDeclaration],
        result: &StructuralOperationResult,
        declaration: &StructuralResultDeclaration,
        mappings: &[ReferenceCallResult],
    ) -> (
        Result<Vec<StructuralPathQualification>, &'static str>,
        usize,
    ) {
        let builds = Cell::new(0);
        let outcome = preserve_call_memberships(
            types,
            &[],
            || {
                builds.set(builds.get() + 1);
                &[]
            },
            MachineId::new(1).unwrap(),
            (BlockId::new(1).unwrap(), 0),
            result,
            declaration,
            mappings,
            &|_| {
                Some(PayloadMembershipContract {
                    scalar_observations: Vec::new(),
                    scalar_requirements: Vec::new(),
                    carrier: StructuralTypeId::new(1).unwrap(),
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                })
            },
        );
        (outcome, builds.get())
    }

    #[test]
    fn undemanded_rows_still_reject_malformed_mappings_without_a_graph() {
        let (types, mut result, declaration) = declarations(StructuralAccess::MutableBorrow);
        let repeated = [
            mapping(ReferenceOriginExtent::Projected),
            mapping(ReferenceOriginExtent::Projected),
        ];
        assert_eq!(
            observe(&types, &result, &declaration, &repeated),
            (
                Err("returned reference membership repeats an output leaf"),
                0
            )
        );
        let mut invalid = mapping(ReferenceOriginExtent::Projected);
        invalid.path.push(StructuralPathSegment::FixedIndex(0));
        assert_eq!(
            observe(&types, &result, &declaration, &[invalid]),
            (Err("returned reference membership has no typed leaf"), 0)
        );
        result.structural_type = StructuralTypeId::new(1).unwrap();
        assert_eq!(
            observe(&types, &result, &declaration, &[]),
            (
                Err("preserved reference result has a different declared carrier"),
                0
            )
        );
    }

    #[test]
    fn only_a_projected_shared_referent_demands_the_graph() {
        for access in [
            StructuralAccess::MutableBorrow,
            StructuralAccess::SharedBorrow,
        ] {
            let (types, result, declaration) = declarations(access);
            assert_eq!(
                observe(&types, &result, &declaration, &[]),
                (Ok(Vec::new()), 0)
            );
            assert_eq!(
                observe(
                    &types,
                    &result,
                    &declaration,
                    &[mapping(ReferenceOriginExtent::WithinRegion)]
                ),
                (Ok(Vec::new()), 0)
            );
            assert_eq!(
                observe(
                    &types,
                    &result,
                    &declaration,
                    &[mapping(ReferenceOriginExtent::Projected)]
                ),
                (
                    Ok(Vec::new()),
                    usize::from(access == StructuralAccess::SharedBorrow)
                )
            );
        }
    }
}
