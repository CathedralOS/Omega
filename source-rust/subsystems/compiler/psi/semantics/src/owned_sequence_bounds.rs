use semantic_vocabulary::StructuralTypeId;

/// Bound owned sequences constructed by the included Terminal machines.
pub fn closed_owned_sequence_bounds(
    module: &terminal_psi::TerminalModule,
    included: impl Fn(semantic_vocabulary::MachineId) -> bool,
) -> Result<Vec<(StructuralTypeId, u64)>, &'static str> {
    let mut bounds = arena::IdTable::from_rows(
        module
            .types.structural_types
            .iter()
            .map(|ty| (ty.id, (ty, None::<u64>))),
    )
    .map_err(|_| "owned sequence bounds require exact structural identities")?;
    for machine in module
        .machines
        .iter()
        .filter(|machine| included(machine.id))
    {
        for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
            let terminal_psi::OperationKind::EstablishStructuralArray { elements } =
                &operation.kind
            else {
                continue;
            };
            let result = operation
                .result
                .structural()
                .ok_or("sequence construction has no structural result")?;
            let (declaration, bound) = bounds
                .get_mut(result.structural_type)
                .ok_or("sequence construction has no structural declaration")?;
            if matches!(
                declaration.shape,
                terminal_psi::StructuralTypeShape::ElementView { access: None, .. }
            ) {
                let count = u64::try_from(elements.len())
                    .map_err(|_| "sequence roster does not fit its extent carrier")?;
                *bound = Some(bound.map_or(count, |earlier| earlier.max(count)));
            }
        }
    }
    let mut rows = module
        .types.structural_types
        .iter()
        .filter_map(|ty| {
            bounds
                .get(ty.id)
                .and_then(|(_, bound)| *bound)
                .map(|bound| (ty.id, bound))
        })
        .collect::<Vec<_>>();
    rows.sort_unstable_by_key(|(identity, _)| *identity);
    Ok(rows)
}
