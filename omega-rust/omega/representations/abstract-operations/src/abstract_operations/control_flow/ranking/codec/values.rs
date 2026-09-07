//! Semantic leaf encoding; full propositions and modules use their canonical owner.
use super::RankedCustodyCodecError as Error;
use super::wire::{Reader, Wire};
use semantic_vocabulary::*;
use terminal_psi::{
    SemanticFingerprint, StructuralMultiplicity, StructuralPathSegment, TerminalModule,
    TerminalPsiIdentity, VocabularyMarker,
};
macro_rules! semantic_id {
    ($($name:ty),*) => { $(impl Wire for $name {
        fn write(&self, bytes:&mut Vec<u8>)->Result<(),Error>{self.get().write(bytes)}
        fn read(reader:&mut Reader<'_>)->Result<Self,Error>{Self::new(u64::read(reader)?).ok_or(Error::InvalidEncoding)}
    })* };
}
semantic_id!(
    MachineId,
    BlockId,
    EdgeId,
    OperationId,
    ValueId,
    ObligationId,
    PlaceId,
    ClaimId
);
impl Wire for TerminalModule {
    fn write(&self, bytes: &mut Vec<u8>) -> Result<(), Error> {
        terminal_codec::encode_module(self)?.write(bytes)
    }
    fn read(reader: &mut Reader<'_>) -> Result<Self, Error> {
        Ok(terminal_codec::decode_module(&Vec::<u8>::read(reader)?)?)
    }
}
impl Wire for Proposition {
    fn write(&self, bytes: &mut Vec<u8>) -> Result<(), Error> {
        terminal_codec::canonical_proposition_order_key(self)?.write(bytes)
    }
    fn read(reader: &mut Reader<'_>) -> Result<Self, Error> {
        Ok(terminal_codec::decode_canonical_proposition(
            &Vec::<u8>::read(reader)?,
        )?)
    }
}
impl Wire for TerminalPsiIdentity {
    fn write(&self, bytes: &mut Vec<u8>) -> Result<(), Error> {
        self.vocabulary_marker.get().write(bytes)?;
        bytes.extend_from_slice(self.program_fingerprint.as_bytes());
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<Self, Error> {
        Ok(Self {
            vocabulary_marker: VocabularyMarker::new(u16::read(reader)?)
                .ok_or(Error::InvalidEncoding)?,
            program_fingerprint: SemanticFingerprint::from_bytes(
                reader
                    .take(32)?
                    .try_into()
                    .map_err(|_| Error::InvalidEncoding)?,
            ),
        })
    }
}
impl Wire for FuelScheduleIdentity {
    fn write(&self, bytes: &mut Vec<u8>) -> Result<(), Error> {
        self.marker().write(bytes)
    }
    fn read(reader: &mut Reader<'_>) -> Result<Self, Error> {
        Self::new(u32::read(reader)?).ok_or(Error::InvalidEncoding)
    }
}
impl Wire for IntegerType {
    fn write(&self, bytes: &mut Vec<u8>) -> Result<(), Error> {
        (match self.carrier() {
            IntegerCarrier::Fixed => 0u8,
            IntegerCarrier::Address => 1,
        })
        .write(bytes)?;
        (match self.sign() {
            IntegerSign::Unsigned => 0u8,
            IntegerSign::Signed => 1,
        })
        .write(bytes)?;
        self.bits().write(bytes)
    }
    fn read(reader: &mut Reader<'_>) -> Result<Self, Error> {
        let carrier = u8::read(reader)?;
        let sign = match u8::read(reader)? {
            0 => IntegerSign::Unsigned,
            1 => IntegerSign::Signed,
            _ => return Err(Error::InvalidEncoding),
        };
        let bits = u16::read(reader)?;
        match carrier {
            0 => Self::new(sign, bits).map_err(|_| Error::InvalidEncoding),
            1 if sign == IntegerSign::Unsigned => {
                Self::address(bits).map_err(|_| Error::InvalidEncoding)
            }
            _ => Err(Error::InvalidEncoding),
        }
    }
}
impl Wire for IntegerValue {
    fn write(&self, bytes: &mut Vec<u8>) -> Result<(), Error> {
        match self {
            Self::Unsigned(value) => {
                0u8.write(bytes)?;
                value.write(bytes)
            }
            Self::Signed(value) => {
                1u8.write(bytes)?;
                value.write(bytes)
            }
        }
    }
    fn read(reader: &mut Reader<'_>) -> Result<Self, Error> {
        match u8::read(reader)? {
            0 => Ok(Self::Unsigned(u128::read(reader)?)),
            1 => Ok(Self::Signed(i128::read(reader)?)),
            _ => Err(Error::InvalidEncoding),
        }
    }
}
impl Wire for StructuralMultiplicity {
    fn write(&self, bytes: &mut Vec<u8>) -> Result<(), Error> {
        (match self {
            Self::Unrestricted => 0u8,
            Self::Affine => 1,
            Self::Linear => 2,
        })
        .write(bytes)
    }
    fn read(reader: &mut Reader<'_>) -> Result<Self, Error> {
        match u8::read(reader)? {
            0 => Ok(Self::Unrestricted),
            1 => Ok(Self::Affine),
            2 => Ok(Self::Linear),
            _ => Err(Error::InvalidEncoding),
        }
    }
}
impl Wire for StructuralPathSegment {
    fn write(&self, bytes: &mut Vec<u8>) -> Result<(), Error> {
        match self {
            Self::Field(value) => {
                0u8.write(bytes)?;
                value.write(bytes)
            }
            Self::FixedIndex(value) => {
                1u8.write(bytes)?;
                value.write(bytes)
            }
        }
    }
    fn read(reader: &mut Reader<'_>) -> Result<Self, Error> {
        match u8::read(reader)? {
            0 => Ok(Self::Field(String::read(reader)?)),
            1 => Ok(Self::FixedIndex(u64::read(reader)?)),
            _ => Err(Error::InvalidEncoding),
        }
    }
}
