//! Selected comparisons, reads and scratch loops for one guarded installation.
use super::*;
pub(super) struct Checks<'a> {
    pub(super) row: &'a LegalizedScalarInstruction,
    pub(super) region: &'a RefusalRegion,
    pub(super) anchor: PlaceId,
    pub(super) home: VirtualRegisterId,
}
impl Checks<'_> {
    pub(super) fn constant(
        &self,
        builder: &mut Builder<'_>,
        value: u64,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        scalar::constant(builder, self.row, self.anchor, value)
    }
    pub(super) fn binary(
        &self,
        builder: &mut Builder<'_>,
        kind: SelectedInstructionKind,
        left: VirtualRegisterId,
        right: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        scalar::binary(builder, self.row, self.anchor, kind, left, right)
    }
    pub(super) fn add(
        &self,
        builder: &mut Builder<'_>,
        left: VirtualRegisterId,
        right: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        self.binary(
            builder,
            SelectedInstructionKind::WrappingAddI64,
            left,
            right,
        )
    }
    pub(super) fn compare(
        &self,
        builder: &mut Builder<'_>,
        left: VirtualRegisterId,
        right: VirtualRegisterId,
        kind: SelectedInstructionKind,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let result = transport_register(builder, self.anchor, 0)?;
        builder.emit(
            SelectedInstructionKind::CompareI64,
            builder.constraints.keys.compare_i64,
            &[left, right],
            provenance(self.row),
        )?;
        builder.emit(
            kind,
            builder.constraints.keys.materialize_boolean,
            &[result],
            provenance(self.row),
        )?;
        Ok(result)
    }
    pub(super) fn equal(
        &self,
        builder: &mut Builder<'_>,
        left: VirtualRegisterId,
        right: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        self.compare(
            builder,
            left,
            right,
            SelectedInstructionKind::MaterializeBooleanEqual,
        )
    }
    pub(super) fn require(
        &self,
        builder: &mut Builder<'_>,
        predicate: VirtualRegisterId,
    ) -> Result<(), SelectedInstructionError> {
        control::refuse_unless(builder, self.row, self.region, predicate)
    }
    pub(super) fn equal_constant(
        &self,
        builder: &mut Builder<'_>,
        value: VirtualRegisterId,
        expected: u64,
    ) -> Result<(), SelectedInstructionError> {
        let expected = self.constant(builder, expected)?;
        let predicate = self.equal(builder, value, expected)?;
        self.require(builder, predicate)
    }
    pub(super) fn at_most(
        &self,
        builder: &mut Builder<'_>,
        left: VirtualRegisterId,
        right: VirtualRegisterId,
    ) -> Result<(), SelectedInstructionError> {
        let predicate = self.compare(
            builder,
            left,
            right,
            SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
        )?;
        self.require(builder, predicate)
    }
    pub(super) fn aligned(
        &self,
        builder: &mut Builder<'_>,
        value: VirtualRegisterId,
        alignment: u32,
    ) -> Result<(), SelectedInstructionError> {
        if !alignment.is_power_of_two() {
            return Err(failure("installed stack alignment is not a power of two"));
        }
        let mask = self.constant(builder, u64::from(alignment - 1))?;
        let low = self.binary(builder, SelectedInstructionKind::BitwiseAndI64, value, mask)?;
        self.equal_constant(builder, low, 0)
    }
    pub(super) fn address(
        &self,
        builder: &mut Builder<'_>,
        base: VirtualRegisterId,
        index: VirtualRegisterId,
        stride: u32,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let stride = self.constant(builder, u64::from(stride))?;
        let offset = self.binary(
            builder,
            SelectedInstructionKind::WrappingMultiplyI64,
            index,
            stride,
        )?;
        self.add(builder, base, offset)
    }
    pub(super) fn load(
        &self,
        builder: &mut Builder<'_>,
        place: PlaceId,
        pointer: VirtualRegisterId,
        offset: u32,
        width: u32,
        storage: StorageAccess<'_>,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let result = transport_register(builder, place, offset)?;
        let (kind, key) = crate::selection::scalar_call_abi::load_transfer(
            &builder.constraints.keys,
            width,
            offset,
        )
        .ok_or_else(|| failure("installation read has unsupported width"))?;
        storage.footprint(
            builder,
            self.row,
            place,
            offset,
            width,
            super::super::runtime_address::Written::None,
        )?;
        builder.emit(
            kind,
            key.ok_or_else(|| failure("installation read has no target form"))?,
            &[pointer, result],
            provenance(self.row),
        )?;
        Ok(result)
    }
    pub(super) fn scratch(
        &self,
        builder: &mut Builder<'_>,
        offset: u32,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        self.load(
            builder,
            self.anchor,
            self.home,
            offset,
            8,
            StorageAccess::Local(LocalStorageSlotId::Boundary {
                operation: self.row.operation,
            }),
        )
    }
    pub(super) fn save(
        &self,
        builder: &mut Builder<'_>,
        offset: u32,
        value: VirtualRegisterId,
    ) -> Result<(), SelectedInstructionError> {
        StorageAccess::Local(LocalStorageSlotId::Boundary {
            operation: self.row.operation,
        })
        .footprint(
            builder,
            self.row,
            self.anchor,
            offset,
            8,
            super::super::runtime_address::Written::Structural,
        )?;
        builder.emit_store(offset, 8, self.home, value, provenance(self.row))
    }
    pub(super) fn reset(
        &self,
        builder: &mut Builder<'_>,
        offset: u32,
    ) -> Result<(), SelectedInstructionError> {
        let zero = self.constant(builder, 0)?;
        self.save(builder, offset, zero)
    }
    pub(super) fn increment(
        &self,
        builder: &mut Builder<'_>,
        offset: u32,
    ) -> Result<(), SelectedInstructionError> {
        let old = self.scratch(builder, offset)?;
        let one = self.constant(builder, 1)?;
        let next = self.add(builder, old, one)?;
        self.save(builder, offset, next)
    }
    pub(super) fn iterate(
        &self,
        builder: &mut Builder<'_>,
        counter: u32,
        length: VirtualRegisterId,
        body: impl FnOnce(&mut Builder<'_>, VirtualRegisterId) -> Result<(), SelectedInstructionError>,
    ) -> Result<(), SelectedInstructionError> {
        self.reset(builder, counter)?;
        control::while_nonzero(
            builder,
            self.row,
            |builder| {
                let index = self.scratch(builder, counter)?;
                self.compare(
                    builder,
                    index,
                    length,
                    SelectedInstructionKind::MaterializeBooleanU64LessThan,
                )
            },
            |builder| {
                let index = self.scratch(builder, counter)?;
                body(builder, index)?;
                self.increment(builder, counter)
            },
        )
    }
}
