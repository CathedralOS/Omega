//! Complete selected temporary loans before the next borrowed arrival uses
//! its captured address. This is restoration of existing entry authority, not
//! an origin inference or permission inferred from equal bytes.

use super::*;

/// Selected cleanup can finish a temporary loan used to capture an arrival.
/// Its storage/address was captured before cleanup; after complete restoration
/// a direct borrowed entry still supplies the original authority. Do not keep
/// that disposed temporary as the next iteration's live granting permission.
/// Other origins and independent validity/presentation debts retain their exact
/// history. In particular, neither equal backing nor a backedge closes a loan.
pub(in crate::legalization::scalar_graph_input::reference_custody) fn complete_cleanup(
    function: &PsiOptimizationFunction,
    custody: &mut Custody,
    types: &[StructuralTypeDeclaration],
    event: retirements::DisposalEvent,
) -> Result<(), LegalizationError> {
    if !custody
        .retired
        .values()
        .any(|retired| retired.disposals.contains(&event))
    {
        return Ok(());
    }
    let mut sources = custody.borrowed_sources.as_ref().clone();
    for alternatives in sources.values_mut().flatten() {
        for source in alternatives.iter_mut() {
            if source.parents.is_empty()
                || source.placed.is_some()
                || !entry_authorizes(function, custody, types, source)
            {
                continue;
            }
            source.parents = restored_parents(custody, &source.parents, event)?;
        }
        canonicalize(alternatives);
    }
    custody.borrowed_sources = std::sync::Arc::new(sources);
    Ok(())
}

fn restored_parents(
    custody: &Custody,
    parents: &[OccurrenceIdentity],
    event: retirements::DisposalEvent,
) -> Result<Vec<OccurrenceIdentity>, LegalizationError> {
    let mut restored = Vec::new();
    for grant in parents {
        let mut pending = vec![(grant, BTreeSet::new())];
        while let Some((grant, mut ancestry)) = pending.pop() {
            if !ancestry.insert(grant) {
                return Err(LegalizationError::ReferenceCompletion {
                    reason: terminal_semantics::ReferenceCompletionError::CyclicAncestry,
                });
            }
            let closed = custody.retired.get(grant).filter(|retired| {
                retired.disposals.contains(&event)
                    && retired.leaf.lifetime_debts.is_empty()
                    && retired.leaf.placed.is_none()
                    && !retired.leaf.backing.is_empty()
                    && retired.leaf.backing.iter().all(|backing| {
                        matches!(backing, descriptor_backing::Backing::Source(root)
                        if *root == retired.leaf.root)
                    })
                    && !custody.leaves.values().any(|child| {
                        child.identity == *grant
                            || custody.descends_from(child, grant)
                            || custody.lifetime_depends_on(child, grant)
                    })
            });
            if let Some(closed) = closed {
                pending.extend(
                    closed
                        .leaf
                        .parents
                        .iter()
                        .map(|parent| (parent, ancestry.clone())),
                );
            } else {
                restored.push(grant.clone());
            }
        }
    }
    restored.sort();
    restored.dedup();
    Ok(restored)
}

fn entry_authorizes(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    source: &Source,
) -> bool {
    let origins = source.root.origins();
    !origins.is_empty()
        && origins.iter().all(|origin| {
            let target_operations::TargetReferenceOriginBase::Storage { root } = origin.base else {
                return false;
            };
            let Some(parameter) = function.structural_parameter(root) else {
                return false;
            };
            if parameter.access == StructuralAccess::Owned
                || !access_covers(parameter.access, source.access)
                || origin.extent != semantic_vocabulary::ReferenceOriginExtent::Projected
            {
                return false;
            }
            let Some(path) = origin
                .path
                .iter()
                .map(|part| match part {
                    terminal_psi::ReferenceOriginProjection::Exact(segment)
                        if *segment != StructuralPathSegment::Referent =>
                    {
                        Some(segment.clone())
                    }
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()
            else {
                return false;
            };
            storage_projection(
                function,
                custody,
                types,
                &StructuralArgument {
                    place: root,
                    path,
                    access: source.access,
                },
            )
            .is_some_and(|projection| projection.matches(types.iter(), source.projection))
        })
}

#[cfg(test)]
mod tests;
