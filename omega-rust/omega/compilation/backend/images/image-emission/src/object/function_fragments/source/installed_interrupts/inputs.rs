//! Reconstruct source operand addresses from declarations and completed homes.
//! Selected memory labels are not consulted when deciding which backing is read.
use super::{
    Error,
    expressions::{Binary, Expr, Word},
};
use selected_instructions::{FrameStorageSlotId, SelectedFunction};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use target_operations::{TargetElementView, TargetFunction, TargetUnitOperation, TargetViewExtent};
use terminal_psi::{StructuralArgument, StructuralTypeShape};

pub(super) struct Inputs {
    pub places: [PlaceId; 6],
    pub pointers: [Expr; 6],
    pub stack_length: Expr,
    pub arrival_length: Expr,
    pub layout: target_operations_to_selected_instructions::InstalledInterruptOperandLayout,
}
struct Homes<'a> {
    target: &'a TargetFunction,
    selected: &'a SelectedFunction,
}
impl Homes<'_> {
    fn root(&self, place: PlaceId) -> Result<(StructuralTypeId, Expr), Error> {
        use super::super::structural_homes::{SourceHome, resolve};
        let (ty, home) = resolve(self.target, self.selected, place).map_err(|reason| {
            Error::StructuralSourceHome {
                machine: self.target.machine,
                place,
                reason,
            }
        })?;
        let address = match home {
            SourceHome::BorrowedFormal(place) => Expr::new(Word::Parameter(place)),
            SourceHome::Allocation { slot, indirect } => {
                let address = Expr::new(Word::Frame(FrameStorageSlotId::Local(slot)));
                if indirect { address.load(8) } else { address }
            }
        };
        Ok((ty, address))
    }

    fn argument(&self, argument: &StructuralArgument) -> Result<(StructuralTypeId, Expr), Error> {
        let (root, mut address) = self.root(argument.place)?;
        let geometry = target_operations_to_selected_instructions::reference_formation_geometry(
            root,
            &argument.path,
            &self.target.graph.structural_types,
        )
        .ok_or(Error::Mismatch(
            "installation operand lost target reference geometry",
        ))?;
        let mut selected = geometry.selected.as_structural().ok_or(Error::Mismatch(
            "installation operand has no structural referent",
        ))?;
        let mut first = 0;
        for slot in &geometry.pointer_slots {
            for index in geometry
                .elements
                .get(slot.indices.clone())
                .ok_or(Error::Mismatch("installation pointer selectors changed"))?
            {
                address = Expr::binary(
                    Binary::Add,
                    address,
                    Expr::binary(
                        Binary::Multiply,
                        Expr::new(Word::Value(index.index)),
                        Expr::constant(u64::from(index.stride)),
                    ),
                );
            }
            address = address.offset(slot.byte_offset).load(8);
            first = slot.indices.end;
        }
        for index in &geometry.elements[first..] {
            address = Expr::binary(
                Binary::Add,
                address,
                Expr::binary(
                    Binary::Multiply,
                    Expr::new(Word::Value(index.index)),
                    Expr::constant(u64::from(index.stride)),
                ),
            );
        }
        address = address.offset(geometry.byte_offset);
        if let Some(StructuralTypeShape::Reference { referent, .. }) =
            terminal_psi::find_structural_type(&self.target.graph.structural_types, selected)
                .map(|row| &row.shape)
        {
            selected = *referent;
        }
        Ok((selected, address))
    }
    fn view(&self, argument: &StructuralArgument) -> Result<(StructuralTypeId, Expr, Expr), Error> {
        if argument.path.is_empty() {
            let mut established = self.target.graph.operations().filter_map(|row| match row {
                TargetUnitOperation::EstablishElementView { result, view }
                    if result.place == argument.place =>
                {
                    Some((result.structural_type, view))
                }
                _ => None,
            });
            if let Some((ty, view)) = established.next() {
                if established.next().is_some() {
                    return Err(Error::Mismatch(
                        "installation view has duplicate establishments",
                    ));
                }
                if let TargetElementView::Established {
                    place,
                    source_byte_offset,
                    indices,
                    extent: TargetViewExtent::Fixed(count),
                    region: target_operations::TargetElementViewRegion::WholeCollection,
                    ..
                } = view
                {
                    let (_, mut address) = self.root(*place)?;
                    for index in indices {
                        address = Expr::binary(
                            Binary::Add,
                            address,
                            Expr::binary(
                                Binary::Multiply,
                                Expr::new(Word::Value(index.operand.source_value())),
                                Expr::constant(u64::from(index.stride)),
                            ),
                        );
                    }
                    return Ok((
                        ty,
                        address.offset(*source_byte_offset),
                        Expr::constant(*count),
                    ));
                }
                return Err(Error::Mismatch(
                    "installation view derivation requires complete selected replay",
                ));
            }
        }
        let (ty, address) = self.argument(argument)?;
        Ok((ty, address.clone().load(8), address.offset(8).load(8)))
    }
}
impl Inputs {
    pub(super) fn derive(
        target: &TargetFunction,
        selected: &SelectedFunction,
        arguments: &[StructuralArgument; 6],
    ) -> Result<Self, Error> {
        let homes = Homes { target, selected };
        let mut values = Vec::with_capacity(6);
        let mut types = Vec::with_capacity(6);
        let mut lengths = Vec::with_capacity(2);
        for (ordinal, argument) in arguments.iter().enumerate() {
            if matches!(ordinal, 3 | 4) {
                let (ty, pointer, length) = homes.view(argument)?;
                types.push(ty);
                values.push(pointer);
                lengths.push(length);
            } else {
                let (ty, pointer) = homes.argument(argument)?;
                types.push(ty);
                values.push(pointer);
            }
        }
        let pointers = values
            .try_into()
            .map_err(|_| Error::Mismatch("installation operand roster changed"))?;
        let types = types
            .try_into()
            .map_err(|_| Error::Mismatch("installation type roster changed"))?;
        let layout =
            target_operations_to_selected_instructions::InstalledInterruptOperandLayout::derive(
                &target.graph.structural_types,
                types,
            )
            .map_err(Error::Mismatch)?;
        let mut lengths = lengths.into_iter();
        Ok(Self {
            places: std::array::from_fn(|index| arguments[index].place),
            pointers,
            stack_length: lengths
                .next()
                .ok_or(Error::Mismatch("stack length missing"))?,
            arrival_length: lengths
                .next()
                .ok_or(Error::Mismatch("arrival length missing"))?,
            layout,
        })
    }
}
