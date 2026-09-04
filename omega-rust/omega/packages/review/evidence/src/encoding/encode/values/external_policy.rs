//! Canonical receipt-free external-supply policy, independent of review rows.

use super::callables::encode_external_executable_supply_coordinate;
use super::identity::encode_nominal;
use crate::encoding::PackageReviewEncodingError;
use crate::encoding::encode::encoder::Encoder;
use crate::record::{
    PackagePolicyEvaluatedBindingProducer, PackagePolicyExternalBinding,
    PackagePolicyExternalExecutableSupply, PackageReviewForeignLocator,
};

const MAGIC: &[u8] = b"OMEGA-EXTERNAL-SUPPLY-POLICY\0";
const VERSION: u16 = 1;
const MAXIMUM_BYTES: usize = 4 * 1024 * 1024;

impl PackagePolicyExternalExecutableSupply {
    /// Bounded component-schema bytes for policy comparison. They contain no
    /// acceptance decision, evaluation receipt or executable authority.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, PackageReviewEncodingError> {
        encode(self, MAXIMUM_BYTES)
    }
}

fn encode(
    supply: &PackagePolicyExternalExecutableSupply,
    maximum_bytes: usize,
) -> Result<Vec<u8>, PackageReviewEncodingError> {
    let mut encoder = Encoder::bounded(maximum_bytes);
    encoder.fixed_bytes(MAGIC);
    encoder.u16(VERSION);
    encode_external_executable_supply_coordinate(
        &mut encoder,
        &supply.callable,
        &supply.signature,
        &supply.requirement,
    )?;
    match &supply.binding {
        PackagePolicyExternalBinding::Import { library, symbol } => {
            encoder.byte(0);
            encoder.string(library)?;
            encoder.string(symbol)?;
        }
        PackagePolicyExternalBinding::Syscall { number } => {
            encoder.byte(1);
            encoder.i64(*number);
        }
        PackagePolicyExternalBinding::CompilerIntrinsic => encoder.byte(2),
        PackagePolicyExternalBinding::VtableSlot { index } => {
            encoder.byte(3);
            encoder.i64(*index);
        }
        PackagePolicyExternalBinding::VtableField { field } => {
            encoder.byte(4);
            encoder.string(field)?;
        }
        PackagePolicyExternalBinding::TableFunction { field } => {
            encoder.byte(5);
            encoder.string(field)?;
        }
        PackagePolicyExternalBinding::NormalizedImport {
            target,
            locator,
            producer,
        } => {
            encoder.byte(6);
            encoder.string(target)?;
            encode_locator(&mut encoder, locator)?;
            encode_producer(&mut encoder, producer)?;
        }
        PackagePolicyExternalBinding::NormalizedSyscall {
            target,
            number,
            producer,
        } => {
            encoder.byte(7);
            encoder.string(target)?;
            encoder.i64(*number);
            encode_producer(&mut encoder, producer)?;
        }
    }
    encoder.finish()
}

fn encode_producer(
    encoder: &mut Encoder,
    producer: &PackagePolicyEvaluatedBindingProducer,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &producer.declaration)?;
    encoder.optional_package_identity(producer.package);
    encoder.string(&producer.callable_identity)
}

fn encode_locator(
    encoder: &mut Encoder,
    locator: &PackageReviewForeignLocator,
) -> Result<(), PackageReviewEncodingError> {
    match locator {
        PackageReviewForeignLocator::PeByName { library, export } => {
            encoder.byte(0);
            encoder.bytes(library)?;
            encoder.bytes(export)?;
        }
        PackageReviewForeignLocator::PeByOrdinal { library, ordinal } => {
            encoder.byte(1);
            encoder.bytes(library)?;
            encoder.u16(*ordinal);
        }
        PackageReviewForeignLocator::ElfVersioned {
            object,
            symbol,
            version,
        } => {
            encoder.byte(2);
            encoder.bytes(object)?;
            encoder.bytes(symbol)?;
            encoder.bytes(version)?;
        }
        PackageReviewForeignLocator::MachODylibSymbol {
            install_name,
            symbol,
        } => {
            encoder.byte(3);
            encoder.bytes(install_name)?;
            encoder.bytes(symbol)?;
        }
    }
    Ok(())
}
