//! Reconstruct the selected word expressions used by one installation.
//! Register provenance supplies no computed value: every definition is decoded.
use super::Error;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    FrameStorageSlotId, SelectedFunction, SelectedImageAddressReference, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind as Kind, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use semantic_vocabulary::{IntegerValue, PlaceId, ValueId};
use std::sync::Arc;
mod copies;

#[derive(Clone, Debug)]
pub(super) struct Expr(Arc<Word>);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RootBoundary {
    RetainedStart,
    ActivationBase,
}
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Word {
    Constant(u64),
    Parameter(PlaceId),
    Value(ValueId),
    Frame(FrameStorageSlotId),
    Image(SelectedImageAddressReference),
    RootBoundary {
        kind: RootBoundary,
        instruction: Option<SelectedInstructionId>,
    },
    Load {
        address: Expr,
        bytes: u8,
        instruction: Option<SelectedInstructionId>,
    },
    Binary(Binary, Expr, Expr),
    Cpu {
        gdt: Expr,
        task: Expr,
        scratch: Expr,
        bytes: u32,
    },
    Call(SelectedInstructionId),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Binary {
    Add,
    Subtract,
    Multiply,
    And,
    Or,
    Xor,
    Left,
    Right,
    Equal,
    Less,
    AtMost,
}
impl PartialEq for Expr {
    fn eq(&self, other: &Self) -> bool {
        match (self.word(), other.word()) {
            (Word::RootBoundary { kind: a, .. }, Word::RootBoundary { kind: b, .. }) => a == b,
            (
                Word::Load {
                    address: a,
                    bytes: aw,
                    ..
                },
                Word::Load {
                    address: b,
                    bytes: bw,
                    ..
                },
            ) => a == b && aw == bw,
            (a, b) => a == b,
        }
    }
}
impl Eq for Expr {}
impl Expr {
    pub(super) fn new(word: Word) -> Self {
        Self(Arc::new(word))
    }
    pub(super) fn word(&self) -> &Word {
        &self.0
    }
    pub(super) fn visit_loads(
        &self,
        visit: &mut impl FnMut(&Expr, u8, Option<SelectedInstructionId>) -> Result<(), Error>,
    ) -> Result<(), Error> {
        match self.word() {
            Word::Load {
                address,
                bytes,
                instruction,
            } => {
                visit(address, *bytes, *instruction)?;
                address.visit_loads(visit)?;
            }
            Word::Binary(_, left, right) => {
                left.visit_loads(visit)?;
                right.visit_loads(visit)?;
            }
            Word::Cpu {
                gdt, task, scratch, ..
            } => {
                gdt.visit_loads(visit)?;
                task.visit_loads(visit)?;
                scratch.visit_loads(visit)?;
            }
            _ => {}
        }
        Ok(())
    }
    pub(super) fn constant(value: u64) -> Self {
        Self::new(Word::Constant(value))
    }
    pub(super) fn binary(kind: Binary, left: Self, right: Self) -> Self {
        if let (Word::Constant(a), Word::Constant(b)) = (left.word(), right.word()) {
            let value = match kind {
                Binary::Add => a.wrapping_add(*b),
                Binary::Subtract => a.wrapping_sub(*b),
                Binary::Multiply => a.wrapping_mul(*b),
                Binary::And => a & b,
                Binary::Or => a | b,
                Binary::Xor => a ^ b,
                Binary::Left => a.wrapping_shl(*b as u32),
                Binary::Right => a.wrapping_shr(*b as u32),
                Binary::Equal => u64::from(a == b),
                Binary::Less => u64::from(a < b),
                Binary::AtMost => u64::from(a <= b),
            };
            return Self::constant(value);
        }
        if kind == Binary::Add {
            if let (Word::Binary(Binary::Add, base, previous), Word::Constant(extra)) =
                (left.word(), right.word())
            {
                if let Word::Constant(offset) = previous.word() {
                    return Self::binary(
                        Binary::Add,
                        base.clone(),
                        Self::constant(offset.wrapping_add(*extra)),
                    );
                }
            }
            if matches!(left.word(), Word::Constant(_))
                && !matches!(right.word(), Word::Constant(_))
            {
                return Self::binary(kind, right, left);
            }
        }
        if matches!(kind, Binary::Add | Binary::Or | Binary::Xor) && right == Self::constant(0) {
            return left;
        }
        if matches!(kind, Binary::Add | Binary::Or | Binary::Xor) && left == Self::constant(0) {
            return right;
        }
        if kind == Binary::Multiply && right == Self::constant(1) {
            return left;
        }
        if kind == Binary::And && right == Self::constant(u64::MAX) {
            return left;
        }
        Self::new(Word::Binary(kind, left, right))
    }
    pub(super) fn offset(self, offset: u32) -> Self {
        Self::binary(Binary::Add, self, Self::constant(u64::from(offset)))
    }
    pub(super) fn load(self, bytes: u8) -> Self {
        Self::new(Word::Load {
            address: self,
            bytes,
            instruction: None,
        })
    }
}

pub(super) fn operand(row: &SelectedInstruction, index: u16) -> Result<VirtualRegisterId, Error> {
    let mut found = row.operands.iter().filter(|value| value.operand == index);
    match (found.next(), found.next()) {
        (Some(value), None) => Ok(value.virtual_register),
        _ => Err(Error::Mismatch(
            "installation instruction has missing or duplicate operand",
        )),
    }
}

pub(super) struct Expressions<'a> {
    function: &'a SelectedFunction,
    graph: &'a super::Graph<'a>,
    definitions: Vec<Option<&'a SelectedInstruction>>,
    preceding: Vec<(SelectedInstructionId, Option<&'a SelectedInstruction>)>,
    memo: Vec<Option<Expr>>,
    active: Vec<bool>,
    spills: super::super::spill_values::SpillValues<'a>,
}
impl<'a> Expressions<'a> {
    pub(super) fn new(
        function: &'a SelectedFunction,
        graph: &'a super::Graph<'a>,
    ) -> Result<Self, Error> {
        let mut definitions = vec![None; function.virtual_registers.len()];
        let mut preceding = Vec::new();
        for block in &function.blocks {
            let mut previous = None;
            for row in block.instructions.iter().chain(std::iter::once(
                selected_instructions::terminator_instruction(&block.terminator),
            )) {
                preceding.push((row.id, previous));
                previous = Some(row);
                for value in &row.operands {
                    if matches!(
                        value.access,
                        RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
                    ) {
                        let slot = definitions
                            .get_mut(value.virtual_register.0 as usize)
                            .ok_or(Error::Mismatch("installation definition has no register"))?;
                        if slot.replace(row).is_some() {
                            return Err(Error::Mismatch(
                                "installation replay requires a unique register definition",
                            ));
                        }
                    }
                }
            }
        }
        preceding.sort_unstable_by_key(|row| row.0);
        let count = definitions.len();
        Ok(Self {
            function,
            graph,
            definitions,
            preceding,
            memo: vec![None; count],
            active: vec![false; count],
            spills: super::super::spill_values::SpillValues::new(function)
                .map_err(Error::Mismatch)?,
        })
    }
    pub(super) fn input(&mut self, row: &SelectedInstruction, index: u16) -> Result<Expr, Error> {
        let register = operand(row, index)?;
        self.require_available(register, row.id)?;
        self.value(register)
    }
    pub(super) fn private_store(&self, row: &SelectedInstruction) -> bool {
        self.spills.private_store(row)
    }
    pub(super) fn allocation_transport(
        &mut self,
        row: &SelectedInstruction,
    ) -> Result<bool, Error> {
        if self.spills.private_store(row) || self.spills.private_address(row) {
            return Ok(true);
        }
        if row.kind == Kind::CopyI64 {
            return self.register_copy(row);
        }
        self.private_reload(row)
    }
    pub(super) fn private_reload(&mut self, row: &SelectedInstruction) -> Result<bool, Error> {
        if row.kind != (Kind::Load64 { byte_offset: 0 }) {
            return Ok(false);
        }
        let address = self.input(row, 0)?;
        if let Word::Frame(FrameStorageSlotId::Local(
            home @ selected_instructions::LocalStorageSlotId::Spill { .. },
        )) = address.word()
        {
            let writer = self.spills.writer(row.id, *home).map_err(Error::Mismatch)?;
            self.input(writer, 0)?;
            return Ok(true);
        }
        Ok(false)
    }
    fn loaded(&mut self, row: &SelectedInstruction, offset: u32, bytes: u8) -> Result<Expr, Error> {
        let address = self.input(row, 0)?;
        if let Word::Frame(FrameStorageSlotId::Local(
            home @ selected_instructions::LocalStorageSlotId::Spill { .. },
        )) = address.word()
        {
            if bytes != 8 || offset != 0 {
                return Err(Error::Mismatch(
                    "spill reload changes the stored word width or offset",
                ));
            }
            let writer = self.spills.writer(row.id, *home).map_err(Error::Mismatch)?;
            // Recursive availability proves each value birth dominates this
            // store. The must-writer proof makes the store dominate this reload.
            // Re-entering a birth while bypassing the store would therefore
            // create an uninitialized path, which the writer query rejects.
            return self.input(writer, 0);
        }
        Ok(Expr::new(Word::Load {
            address: address.offset(offset),
            bytes,
            instruction: Some(row.id),
        }))
    }
    fn require_available(
        &self,
        register: VirtualRegisterId,
        use_at: SelectedInstructionId,
    ) -> Result<(), Error> {
        let definition = self
            .definitions
            .get(register.0 as usize)
            .ok_or(Error::Mismatch("installation use names a missing register"))?;
        if let Some(definition) = definition {
            if !self.graph.precedes(definition.id, use_at)? {
                return Err(Error::Mismatch(
                    "installation uses a value before its definition on this path",
                ));
            }
        } else {
            let register = self
                .function
                .virtual_register(register)
                .ok_or(Error::Mismatch("installation input register is missing"))?;
            if let VirtualRegisterOrigin::BlockParameter { block, .. } = register.origin {
                if !self.graph.block_dominates(block, use_at)? {
                    return Err(Error::Mismatch(
                        "installation uses a block parameter outside its incoming scope",
                    ));
                }
            } else if !self.function.is_entry_register(register) {
                return Err(Error::Mismatch(
                    "installation input has no definition or entry supply",
                ));
            }
        }
        Ok(())
    }
    pub(super) fn defining_instruction(
        &self,
        register: VirtualRegisterId,
    ) -> Result<SelectedInstructionId, Error> {
        self.definitions
            .get(register.0 as usize)
            .and_then(|row| *row)
            .map(|row| row.id)
            .ok_or(Error::Mismatch(
                "installation result register has no actual defining instruction",
            ))
    }
    pub(super) fn value(&mut self, register: VirtualRegisterId) -> Result<Expr, Error> {
        let index = register.0 as usize;
        if let Some(value) = self.memo.get(index).and_then(|value| value.clone()) {
            return Ok(value);
        }
        let active = self.active.get_mut(index).ok_or(Error::Mismatch(
            "installation expression references missing register",
        ))?;
        if *active {
            return Err(Error::Mismatch(
                "installation expression has cyclic definitions",
            ));
        }
        *active = true;
        let row = self.definitions[index];
        let value = if let Some(row) = row {
            self.definition(row, register)?
        } else {
            match self
                .function
                .virtual_register(register)
                .ok_or(Error::Mismatch("installation input register is missing"))?
                .origin
            {
                VirtualRegisterOrigin::StructuralParameter {
                    place,
                    parameter_index,
                } => {
                    let parameter = self
                        .function
                        .structural
                        .as_ref()
                        .and_then(|row| row.parameters.get(parameter_index))
                        .ok_or(Error::Mismatch(
                            "installation parameter lost its structural declaration",
                        ))?;
                    if parameter.semantic.place != place {
                        return Err(Error::Mismatch(
                            "installation parameter names different source storage",
                        ));
                    }
                    Expr::new(Word::Parameter(place))
                }
                VirtualRegisterOrigin::EntryParameter { source_value, .. }
                | VirtualRegisterOrigin::BlockParameter { source_value, .. } => {
                    Expr::new(Word::Value(source_value))
                }
                _ => {
                    return Err(Error::Mismatch(
                        "installation expression has no definition or admitted input",
                    ));
                }
            }
        };
        self.active[index] = false;
        self.memo[index] = Some(value.clone());
        Ok(value)
    }
    fn definition(
        &mut self,
        row: &SelectedInstruction,
        register: VirtualRegisterId,
    ) -> Result<Expr, Error> {
        Ok(match &row.kind {
            Kind::MaterializeI64 { value } => Expr::constant(match value {
                IntegerValue::Unsigned(value) => u64::try_from(*value)
                    .map_err(|_| Error::Mismatch("installation constant exceeds machine word"))?,
                IntegerValue::Signed(value) => i64::try_from(*value).map_err(|_| {
                    Error::Mismatch("installation signed constant exceeds machine word")
                })? as u64,
            }),
            Kind::CopyI64 => self.input(row, 0)?,
            Kind::FrameAddress {
                slot: FrameStorageSlotId::ActivationBase,
                byte_offset,
            } => {
                if *byte_offset != 0 {
                    return Err(Error::Mismatch(
                        "activation base observation has a displacement",
                    ));
                }
                Expr::new(Word::RootBoundary {
                    kind: RootBoundary::ActivationBase,
                    instruction: Some(row.id),
                })
            }
            Kind::ImageAddress {
                reference: SelectedImageAddressReference::RetainedEntryStorageStart,
            } => Expr::new(Word::RootBoundary {
                kind: RootBoundary::RetainedStart,
                instruction: Some(row.id),
            }),
            Kind::FrameAddress { slot, byte_offset } => {
                Expr::new(Word::Frame(*slot)).offset(*byte_offset)
            }
            Kind::ImageAddress { reference } => Expr::new(Word::Image(reference.clone())),
            Kind::AddressOffset { byte_offset } => self.input(row, 0)?.offset(*byte_offset),
            Kind::Load8 { byte_offset } => self.loaded(row, *byte_offset, 1)?,
            Kind::Load16 { byte_offset } => self.loaded(row, *byte_offset, 2)?,
            Kind::Load32 { byte_offset } => self.loaded(row, *byte_offset, 4)?,
            Kind::Load64 { byte_offset } => self.loaded(row, *byte_offset, 8)?,
            Kind::WrappingAddI64 | Kind::ByteViewAddress => self.binary(row, Binary::Add)?,
            Kind::WrappingSubtractI64 => self.binary(row, Binary::Subtract)?,
            Kind::WrappingMultiplyI64 => self.binary(row, Binary::Multiply)?,
            Kind::BitwiseAndI64 => self.binary(row, Binary::And)?,
            Kind::BitwiseOrI64 => self.binary(row, Binary::Or)?,
            Kind::BitwiseXorI64 => self.binary(row, Binary::Xor)?,
            Kind::WrappingShiftLeftI64 => self.binary(row, Binary::Left)?,
            Kind::WrappingShiftRightU64 => self.binary(row, Binary::Right)?,
            Kind::MaterializeBooleanEqual
            | Kind::MaterializeBooleanU64LessThan
            | Kind::MaterializeBooleanU64LessOrEqual => {
                let compare = self
                    .preceding
                    .binary_search_by_key(&row.id, |entry| entry.0)
                    .ok()
                    .and_then(|index| self.preceding[index].1)
                    .ok_or(Error::Mismatch("installation Boolean has no comparison"))?;
                if compare.kind != Kind::CompareI64 {
                    return Err(Error::Mismatch(
                        "installation Boolean does not read the expected comparison flags",
                    ));
                }
                let kind = match row.kind {
                    Kind::MaterializeBooleanEqual => Binary::Equal,
                    Kind::MaterializeBooleanU64LessThan => Binary::Less,
                    _ => Binary::AtMost,
                };
                self.binary(compare, kind)?
            }
            Kind::InstalledInterruptCpuCheck { gdt_bytes } => {
                if operand(row, 3)? != register {
                    return Err(Error::Mismatch(
                        "CPU check scratch is not its Boolean result",
                    ));
                }
                Expr::new(Word::Cpu {
                    gdt: self.input(row, 0)?,
                    task: self.input(row, 1)?,
                    scratch: self.input(row, 2)?,
                    bytes: *gdt_bytes,
                })
            }
            Kind::CallScalar { .. } => {
                if row.operands.last().map(|value| value.virtual_register) != Some(register) {
                    return Err(Error::Mismatch(
                        "installation call scratch is not its result",
                    ));
                }
                Expr::new(Word::Call(row.id))
            }
            _ => {
                return Err(Error::Mismatch(
                    "installation expression uses an unsupported selected definition",
                ));
            }
        })
    }
    fn binary(&mut self, row: &SelectedInstruction, kind: Binary) -> Result<Expr, Error> {
        Ok(Expr::binary(kind, self.input(row, 0)?, self.input(row, 1)?))
    }
}
