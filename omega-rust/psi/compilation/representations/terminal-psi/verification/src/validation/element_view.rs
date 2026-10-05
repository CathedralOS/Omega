//! Element views: establishing one, measuring it, reading through it,
//! storing through it, and narrowing it to a subslice.

pub(crate) mod establishment;
pub(crate) mod length;
pub(crate) mod mutable_establishment;
pub(crate) mod read;
pub(crate) mod subslice;
pub(crate) mod write;

use crate::validation::{StructuralAccess, StructuralPlaceKind, TerminalMachine, TerminalModule};

/// The operation result a live view binding occupies, paired with the
/// access its establishment minted: a shared view lends only shared; a
/// mutable view supplies every borrow kind. Neither kind lets the place
/// masquerade as an owned local.
pub(crate) fn view_loan_result_in_module<'a>(
    module: &TerminalModule,
    caller: &'a TerminalMachine,
    place: crate::validation::PlaceId,
) -> Option<(
    &'a terminal_psi::StructuralOperationResult,
    StructuralAccess,
)> {
    let access = view_access(module, caller, place)?;
    let StructuralPlaceKind::OperationResult {
        producer,
        structural_type,
    } = caller
        .structural_places
        .iter()
        .find(|row| row.id == place)?
        .kind
    else {
        return None;
    };
    let result = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == producer)?
        .result
        .structural()?;
    (result.place == place && result.structural_type == structural_type).then_some((result, access))
}

/// Reconstruct authority from the exact input or producer chain. Multiplicity
/// chooses how a range retains its loan, but cannot promote a shared source.
/// Every step consumes a distinct declaration in a valid chain, so the place
/// count bounds malformed cycles without using the host stack.
fn view_access(
    module: &TerminalModule,
    machine: &TerminalMachine,
    mut place: crate::validation::PlaceId,
) -> Option<StructuralAccess> {
    use crate::validation::{OperationKind, StructuralMultiplicity};
    let mut requested = None;
    let mut mutable_required = false;
    for _ in 0..machine.structural_places.len() {
        let declaration = machine
            .structural_places
            .iter()
            .find(|row| row.id == place)?;
        let established = match declaration.kind {
            StructuralPlaceKind::Parameter { position } => {
                machine
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == place && parameter.position == position)?
                    .access
            }
            StructuralPlaceKind::BlockParameter { .. } => {
                crate::validation::block_views::parameter(machine, place)?.access
            }
            StructuralPlaceKind::ByteSequenceLiteral { .. } => StructuralAccess::SharedBorrow,
            StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            } => {
                let operation = machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .find(|operation| operation.id == producer)?;
                let result = operation.result.structural()?;
                if result.place != place
                    || result.structural_type != structural_type
                    || !result.qualifications.is_empty()
                    || !result.projected_qualifications.is_empty()
                    || !result.claims.is_empty()
                {
                    return None;
                }
                match &operation.kind {
                    OperationKind::EstablishElementView { source, .. }
                        if source.access == StructuralAccess::SharedBorrow
                            && result.multiplicity == StructuralMultiplicity::Unrestricted =>
                    {
                        StructuralAccess::SharedBorrow
                    }
                    OperationKind::EstablishMutableElementView { source, .. }
                        if source.access == StructuralAccess::MutableBorrow
                            && result.multiplicity == StructuralMultiplicity::Affine =>
                    {
                        StructuralAccess::MutableBorrow
                    }
                    OperationKind::ElementViewSubslice { source, .. }
                    | OperationKind::ByteSequenceSubslice { source, .. } => {
                        let access = match result.multiplicity {
                            StructuralMultiplicity::Unrestricted => StructuralAccess::SharedBorrow,
                            StructuralMultiplicity::Affine => StructuralAccess::MutableBorrow,
                            _ => return None,
                        };
                        if mutable_required && access != StructuralAccess::MutableBorrow {
                            return None;
                        }
                        requested.get_or_insert(access);
                        mutable_required = access == StructuralAccess::MutableBorrow;
                        place = *source;
                        continue;
                    }
                    OperationKind::StructuralLeafCopy { .. } => {
                        if result.multiplicity != StructuralMultiplicity::Unrestricted
                            || !terminal_semantics::descriptor_results::is_shared_descriptor(
                                module.structural_types.iter(),
                                structural_type,
                            )
                        {
                            return None;
                        }
                        // Leaf-copy verification checks the exact readable
                        // source and copyable selected type independently.
                        StructuralAccess::SharedBorrow
                    }
                    OperationKind::CallStructural { .. }
                    | OperationKind::CallStructuralWithScalarArguments { .. } => {
                        // A signature alone cannot lend backing. Reconstruct
                        // every normal return and substitute this exact call.
                        terminal_semantics::descriptor_results::call_origins(
                            module, machine, operation,
                        )?;
                        StructuralAccess::SharedBorrow
                    }
                    _ => return None,
                }
            }
            _ => return None,
        };
        if !matches!(
            established,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        ) || mutable_required && established != StructuralAccess::MutableBorrow
        {
            return None;
        }
        return Some(requested.unwrap_or(established));
    }
    None
}
