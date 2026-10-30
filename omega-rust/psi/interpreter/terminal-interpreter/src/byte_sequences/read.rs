//! Borrowed observation of byte backing. Scalar-array storage stays in its
//! original representation; reading one byte does not materialize the array.

use crate::errors::{TerminalInterpretError, malformed};
use crate::scalar_storage::ScalarCell;

#[derive(Debug, Clone, Copy)]
enum ByteStorage<'storage> {
    Contiguous(&'storage [u8]),
    Cells(&'storage [ScalarCell]),
}

/// Read-only bytes borrowed from an invocation. No backing, qualification or
/// guest storage authority transfers to the observer. The borrow prevents
/// mutation or closing the instance while the observation is in use.
#[derive(Debug, Clone, Copy)]
pub struct TerminalByteRead<'storage> {
    storage: ByteStorage<'storage>,
}

impl PartialEq for TerminalByteRead<'_> {
    fn eq(&self, other: &Self) -> bool {
        if let (ByteStorage::Cells(left), ByteStorage::Cells(right)) = (self.storage, other.storage)
            && left == right
        {
            return true;
        }
        self.len() == other.len()
            && (0..self.len()).all(|byte_index| {
                match (self.read(byte_index), other.read(byte_index)) {
                    (Ok(left), Ok(right)) => left == right,
                    _ => false,
                }
            })
    }
}

impl Eq for TerminalByteRead<'_> {}

impl<'storage> TerminalByteRead<'storage> {
    pub(crate) fn contiguous(bytes: &'storage [u8]) -> Self {
        Self {
            storage: ByteStorage::Contiguous(bytes),
        }
    }

    /// Borrow the actual packed cells, including live original native homes.
    /// Formation observes every cell but never materializes an array copy.
    pub(crate) fn cells(values: &'storage [ScalarCell]) -> Result<Self, TerminalInterpretError> {
        for cell in values {
            crate::structural_inputs::byte_arrays::storage::scalar_byte(&cell.read()?)?;
        }
        Ok(Self {
            storage: ByteStorage::Cells(values),
        })
    }

    pub fn len(&self) -> usize {
        match self.storage {
            ByteStorage::Contiguous(bytes) => bytes.len(),
            ByteStorage::Cells(values) => values.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Read one byte, refusing an out-of-range index or malformed scalar
    /// backing. No allocation or complete-array conversion is needed.
    pub fn read(&self, byte_index: usize) -> Result<u8, TerminalInterpretError> {
        match self.storage {
            ByteStorage::Contiguous(bytes) => bytes.get(byte_index).copied().ok_or_else(malformed),
            ByteStorage::Cells(values) => {
                let value = values.get(byte_index).ok_or_else(malformed)?.read()?;
                crate::structural_inputs::byte_arrays::storage::scalar_byte(&value)
            }
        }
    }

    /// Copy into caller-owned, exactly sized storage. A sponsor can inspect
    /// `len`, reserve live storage and result custody, then allocate this
    /// destination. An error never publishes a result; malformed scalar
    /// backing may leave an already copied prefix in the caller's buffer.
    pub fn copy_to(&self, destination: &mut [u8]) -> Result<(), TerminalInterpretError> {
        if destination.len() != self.len() {
            return Err(malformed());
        }
        match self.storage {
            ByteStorage::Contiguous(bytes) => destination.copy_from_slice(bytes),
            ByteStorage::Cells(_) => {
                for (byte_index, byte) in destination.iter_mut().enumerate() {
                    *byte = self.read(byte_index)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ByteStorage, TerminalByteRead};
    use crate::scalar_storage::ScalarCell;
    use crate::values::TerminalScalarValue;
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};

    fn scalar(byte: u128) -> TerminalScalarValue {
        TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            value: IntegerValue::Unsigned(byte),
        }
    }

    #[test]
    fn byte_read_borrows_both_storage_representations_and_copies_only_on_request() {
        let backing = [4, 8, 15, 16];
        let scalar_backing = backing.map(|byte| ScalarCell::Local(scalar(u128::from(byte))));
        let contiguous = TerminalByteRead::contiguous(&backing[1..3]);
        let scalars = TerminalByteRead::cells(&scalar_backing[1..3]).unwrap();
        assert_eq!(contiguous, scalars);
        let ByteStorage::Contiguous(bytes) = contiguous.storage else {
            panic!("contiguous")
        };
        let ByteStorage::Cells(values) = scalars.storage else {
            panic!("scalars")
        };
        assert_eq!(bytes.as_ptr(), backing[1..].as_ptr());
        assert_eq!(values.as_ptr(), scalar_backing[1..].as_ptr());
        for view in [contiguous, scalars] {
            assert_eq!(view.len(), 2);
            assert_eq!(view.read(0).unwrap(), 8);
            assert_eq!(view.read(1).unwrap(), 15);
            assert!(view.read(2).is_err());
            let mut wrong_size = [99; 3];
            assert!(view.copy_to(&mut wrong_size).is_err());
            assert_eq!(wrong_size, [99; 3]);
            let mut output = [0; 2];
            view.copy_to(&mut output).unwrap();
            assert_eq!(output, [8, 15]);
        }
    }

    #[test]
    fn byte_read_checks_the_whole_window_before_exposing_malformed_scalars() {
        assert!(
            TerminalByteRead::cells(&[
                ScalarCell::Local(scalar(8)),
                ScalarCell::Local(scalar(256))
            ])
            .is_err()
        );
        assert!(
            TerminalByteRead::cells(&[ScalarCell::Local(TerminalScalarValue::Boolean(true))])
                .is_err()
        );
        for empty in [
            TerminalByteRead::contiguous(&[]),
            TerminalByteRead::cells(&[]).unwrap(),
        ] {
            assert!(empty.is_empty());
            assert!(empty.read(0).is_err());
            empty.copy_to(&mut []).unwrap();
        }
    }
}
