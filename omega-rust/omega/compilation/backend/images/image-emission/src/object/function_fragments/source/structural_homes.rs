//! Resolve one declared structural place to its exact retained physical home.
//! Function inputs, operation results and block arrivals use the same join;
//! selected labels alone cannot establish a source allocation or borrowed root.
use selected_instructions::{LocalStorageSlotId, SelectedFunction};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use target_operations::TargetFunction;

pub(super) enum SourceHome {
    Allocation {
        slot: LocalStorageSlotId,
        indirect: bool,
    },
    BorrowedFormal(PlaceId),
}

pub(super) fn resolve(
    target: &TargetFunction,
    selected: &SelectedFunction,
    place: PlaceId,
) -> Result<(StructuralTypeId, SourceHome), &'static str> {
    let mut roots = target
        .graph
        .operations()
        .filter_map(|row| {
            row.structural_result_home()
                .filter(|(_, home)| home.place() == place)
                .map(|(producer, home)| {
                    (
                        Some(LocalStorageSlotId::Structural {
                            operation: producer,
                            place,
                        }),
                        home.structural_type(),
                    )
                })
        })
        .chain(
            target
                .graph
                .parameters
                .iter()
                .filter(|row| row.place == place)
                .map(|row| {
                    (
                        (row.access == terminal_psi::StructuralAccess::Owned)
                            .then_some(LocalStorageSlotId::StructuralParameter { place }),
                        row.structural_type,
                    )
                }),
        )
        .chain(target.graph.blocks.iter().flat_map(|block| {
            block
                .structural_parameters
                .iter()
                .filter(|row| row.place == place)
                .map(|row| {
                    (
                        Some(LocalStorageSlotId::StructuralBlockParameter {
                            block: block.block,
                            place: row.place,
                        }),
                        row.structural_type,
                    )
                })
        }));
    let (producer, root) = roots
        .next()
        .ok_or("source place has no declaration or result home")?;
    if roots.next().is_some() {
        return Err("source place has multiple declarations or result homes");
    }
    let mut allocations = selected
        .local_storage_slots
        .iter()
        .filter(|slot| slot.id.structural_place() == Some(place));
    let allocation = allocations.next();
    if allocations.next().is_some() {
        return Err("source place has multiple allocated homes");
    }
    let base = if let Some(allocation) = allocation {
        if Some(allocation.id) != producer {
            return Err("allocated source home differs from its declared owner");
        }
        let indirect = thin_address_join(target, allocation.id)
            .ok_or("block source home has no unique typed parameter")?;
        if indirect && (allocation.byte_size != 8 || allocation.alignment != 8) {
            return Err("borrowed block source home is not an eight-byte pointer slot");
        }
        SourceHome::Allocation {
            slot: allocation.id,
            indirect,
        }
    } else {
        let parameter = target
            .graph
            .parameters
            .iter()
            .find(|row| row.place == place)
            .ok_or("declared local or block source has no allocated home")?;
        if producer.is_some() || parameter.access == terminal_psi::StructuralAccess::Owned {
            return Err("owned source parameter has no allocated home");
        }
        SourceHome::BorrowedFormal(place)
    };
    Ok((root, base))
}

/// An aggregate borrow arriving at a block stores its pointer in an eight-byte
/// carrier. Owned values and scalar address/codec descriptors use other homes.
pub(super) fn thin_address_join(target: &TargetFunction, slot: LocalStorageSlotId) -> Option<bool> {
    let LocalStorageSlotId::StructuralBlockParameter { block, place } = slot else {
        return Some(false);
    };
    let mut parameters = target
        .graph
        .blocks
        .iter()
        .filter(|row| row.block == block)
        .flat_map(|row| &row.structural_parameters)
        .filter(|row| row.place == place);
    let (Some(parameter), None) = (parameters.next(), parameters.next()) else {
        return None;
    };
    if block == target.graph.entry
        || !abstract_operations::control_flow::address_joins::is_address_join_in(
            parameter,
            &target.graph.structural_types,
        )
    {
        return Some(false);
    }
    let declaration = terminal_psi::find_structural_type(
        &target.graph.structural_types,
        parameter.structural_type,
    )?;
    let scalar_descriptor = match declaration.shape {
        terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar) => {
            runtime_abi::ScalarStorageCodec::from_scalar(scalar).is_some()
        }
        _ => false,
    };
    Some(!scalar_descriptor)
}
