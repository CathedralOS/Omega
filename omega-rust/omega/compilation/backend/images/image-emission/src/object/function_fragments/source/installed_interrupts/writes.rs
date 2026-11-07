//! Account for every store, including preserved bits in private-slot containers.
use super::{
    Error,
    expressions::{Binary, Expr, Expressions, Word},
};
use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind as Kind,
};
use semantic_vocabulary::OperationId;

pub(super) struct Store<'a> {
    pub instruction: &'a SelectedInstruction,
    pub address: Expr,
    pub value: Expr,
    pub bytes: u8,
    consumed: bool,
}
pub(super) struct Writes<'a> {
    pub rows: Vec<Store<'a>>,
}
impl<'a> Writes<'a> {
    pub(super) fn read(
        function: &'a SelectedFunction,
        operation: OperationId,
        expressions: &mut Expressions<'_>,
    ) -> Result<Self, Error> {
        let mut rows = Vec::new();
        for instruction in function
            .instructions()
            .filter(|row| row.provenance.operations.contains(&operation))
        {
            if expressions.private_store(instruction) {
                // A closed private spill has no source-visible writes. Its exact
                // writer is replayed whenever an installation operand reloads it.
                continue;
            }
            if let Kind::Store {
                byte_offset,
                byte_size,
            } = instruction.kind
            {
                rows.push(Store {
                    instruction,
                    address: expressions.input(instruction, 0)?.offset(byte_offset),
                    value: expressions.input(instruction, 1)?,
                    bytes: byte_size,
                    consumed: false,
                });
            } else if matches!(
                instruction.kind,
                Kind::Store64 { .. }
                    | Kind::StorePacked { .. }
                    | Kind::StoreEncodedScalar
                    | Kind::CopyBytes
            ) {
                return Err(Error::Mismatch(
                    "installation contains an unaccounted storage operation",
                ));
            }
        }
        rows.sort_unstable_by_key(|row| row.instruction.id);
        Ok(Self { rows })
    }
    pub(super) fn take(
        &mut self,
        address: Expr,
        value: Expr,
        bytes: u8,
    ) -> Result<SelectedInstructionId, Error> {
        let mut matches = self.rows.iter_mut().filter(|row| {
            !row.consumed
                && row.address == address
                && row.bytes == bytes
                && bits_equal(&row.value, &value, bytes)
        });
        let row = matches.next().ok_or(Error::Mismatch(
            "installation changed a required store address or value",
        ))?;
        // Repeated zero bytes can belong to distinct ordered root updates.
        // The caller proves their phase order; the closed roster below rejects
        // any additional store left after both updates have been consumed.
        row.consumed = true;
        Ok(row.instruction.id)
    }
    pub(super) fn take_bytes(
        &mut self,
        address: Expr,
        value: Expr,
        bytes: u8,
    ) -> Result<Vec<SelectedInstructionId>, Error> {
        (0..bytes)
            .map(|offset| {
                self.take(
                    address.clone().offset(u32::from(offset)),
                    Expr::binary(
                        Binary::Right,
                        value.clone(),
                        Expr::constant(u64::from(offset) * 8),
                    ),
                    1,
                )
            })
            .collect()
    }
    pub(super) fn value(&self, instruction: SelectedInstructionId) -> Result<&Expr, Error> {
        self.rows
            .binary_search_by_key(&instruction, |row| row.instruction.id)
            .ok()
            .map(|index| &self.rows[index].value)
            .ok_or(Error::Mismatch(
                "installation write is missing from its closed roster",
            ))
    }
    pub(super) fn complete(&self) -> Result<(), Error> {
        if self.rows.iter().any(|row| !row.consumed) {
            Err(Error::Mismatch(
                "installation contains an additional storage write",
            ))
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug)]
enum Bit {
    Constant(bool),
    Atom(Expr, u8),
    And(Box<Bit>, Box<Bit>),
    Or(Box<Bit>, Box<Bit>),
    Xor(Box<Bit>, Box<Bit>),
}
impl PartialEq for Bit {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Constant(a), Self::Constant(b)) => a == b,
            (Self::Atom(a, ab), Self::Atom(b, bb)) => a == b && ab == bb,
            (Self::And(a, b), Self::And(c, d))
            | (Self::Or(a, b), Self::Or(c, d))
            | (Self::Xor(a, b), Self::Xor(c, d)) => (a == c && b == d) || (a == d && b == c),
            _ => false,
        }
    }
}
impl Bit {
    fn combine(kind: Binary, left: Self, right: Self) -> Self {
        match (kind, &left, &right) {
            (Binary::And, Self::Constant(false), _) | (Binary::And, _, Self::Constant(false)) => {
                Self::Constant(false)
            }
            (Binary::And, Self::Constant(true), _)
            | (Binary::Or, Self::Constant(false), _)
            | (Binary::Xor, Self::Constant(false), _) => right,
            (Binary::And, _, Self::Constant(true))
            | (Binary::Or, _, Self::Constant(false))
            | (Binary::Xor, _, Self::Constant(false)) => left,
            (Binary::Or, Self::Constant(true), _) | (Binary::Or, _, Self::Constant(true)) => {
                Self::Constant(true)
            }
            (Binary::And | Binary::Or, _, _) if left == right => left,
            (Binary::Xor, _, _) if left == right => Self::Constant(false),
            (Binary::And, _, _) => Self::And(Box::new(left), Box::new(right)),
            (Binary::Or, _, _) => Self::Or(Box::new(left), Box::new(right)),
            _ => Self::Xor(Box::new(left), Box::new(right)),
        }
    }
}
fn bit(value: &Expr, index: u8) -> Bit {
    match value.word() {
        Word::Constant(value) => Bit::Constant((value >> index) & 1 != 0),
        Word::Load { address, bytes, .. } => {
            if u32::from(index) >= u32::from(*bytes) * 8 {
                Bit::Constant(false)
            } else {
                Bit::Atom(
                    address.clone().offset(u32::from(index / 8)).load(1),
                    index % 8,
                )
            }
        }
        Word::Binary(kind @ (Binary::And | Binary::Or | Binary::Xor), left, right) => {
            Bit::combine(*kind, bit(left, index), bit(right, index))
        }
        Word::Binary(Binary::Left, left, right) => {
            if let Word::Constant(shift) = right.word() {
                index
                    .checked_sub((shift % 64) as u8)
                    .map_or(Bit::Constant(false), |index| bit(left, index))
            } else {
                Bit::Atom(value.clone(), index)
            }
        }
        Word::Binary(Binary::Right, left, right) => {
            if let Word::Constant(shift) = right.word() {
                let shifted = u64::from(index) + shift % 64;
                if shifted < 64 {
                    bit(left, shifted as u8)
                } else {
                    Bit::Constant(false)
                }
            } else {
                Bit::Atom(value.clone(), index)
            }
        }
        _ => Bit::Atom(value.clone(), index),
    }
}
pub(super) fn bits_equal(actual: &Expr, expected: &Expr, bytes: u8) -> bool {
    bytes <= 8 && (0..bytes * 8).all(|index| bit(actual, index) == bit(expected, index))
}
