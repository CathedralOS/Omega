//! One typed owner joins descriptor producer lineage to stored access ceilings.
//! A carrier's declared access is metadata. Every view/subslice hop still owes
//! its full source projection; reference initializers never choose the pointer
//! that a current carrier contains. Each query owns its cycle roster.
use super::*;

pub(super) fn backing_access<'types>(
    caller: &PsiOptimizationFunction,
    place: PlaceId,
    types: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration> + Clone,
) -> Option<terminal_psi::StructuralAccess> {
    let declared = structural_source_contract(caller, place)?.claimed_backing_access(types.clone());
    let mut place = place;
    let mut visited = Vec::new();
    loop {
        if visited.contains(&place) {
            return None;
        }
        visited.push(place);
        let contract = structural_source_contract(caller, place)?;
        let Some(result) = contract.operation_result else {
            return Some(declared);
        };
        let mut producers = caller.block_nodes().filter(|node| {
            node.operation
                .structural_result()
                .is_some_and(|(_, candidate)| candidate.place == place)
        });
        let producer = producers.next()?;
        if producers.next().is_some() || producer.operation.structural_result()?.1 != result {
            return None;
        }
        let source = match &producer.operation {
            O::EstablishElementView { source, .. }
            | abstract_operations::element_view_or_byte_subslice_pattern!(source, ..) => source,
            // A current returned/stored descriptor retains its own declared
            // ceiling. Following EstablishReference origins here would replace
            // its actual stored pointer with one possible historical origin.
            _ => return Some(declared),
        };
        let source_contract = structural_source_contract(caller, source.place)?;
        if !terminal_semantics::descriptor_source_projection_accessible(
            types.clone(),
            source_contract.structural_type,
            source_contract.multiplicity,
            source_contract.access,
            &source.path,
            source.access,
        ) {
            return None;
        }
        place = source.place;
    }
}
