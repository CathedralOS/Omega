use super::{Encoder, PackageReviewEncodingError};
use crate::encoding::encode::values::{
    external_policy::{encode_locator, encode_producer},
    identity::encode_nominal,
};
use crate::record::PackagePolicyProviderBinding;

pub(super) fn binding(
    encoder: &mut Encoder,
    binding: &PackagePolicyProviderBinding,
) -> Result<(), PackageReviewEncodingError> {
    match binding {
        PackagePolicyProviderBinding::StringBackedImportBootstrap { library, symbol } => {
            encoder.byte(0);
            encoder.string(library)?;
            encoder.string(symbol)?;
        }
        PackagePolicyProviderBinding::Syscall { number, evaluated } => {
            encoder.byte(1);
            encoder.i64(*number);
            encoder.option(evaluated.as_ref(), |encoder, evaluated| {
                encoder.string(&evaluated.target)?;
                encode_producer(encoder, &evaluated.producer)
            })?;
        }
        PackagePolicyProviderBinding::CompilerIntrinsic { machine } => {
            encoder.byte(2);
            encoder.string(machine)?;
        }
        PackagePolicyProviderBinding::VtableSlot { index } => {
            encoder.byte(3);
            encoder.i64(*index);
        }
        PackagePolicyProviderBinding::VtableField {
            table,
            field,
            table_declaration,
        } => {
            encoder.byte(4);
            encoder.string(table)?;
            encoder.string(field)?;
            encode_nominal(encoder, table_declaration)?;
        }
        PackagePolicyProviderBinding::TableFunction {
            table,
            field,
            table_declaration,
        } => {
            encoder.byte(5);
            encoder.string(table)?;
            encoder.string(field)?;
            encode_nominal(encoder, table_declaration)?;
        }
        PackagePolicyProviderBinding::CheckedAdapter {
            machine_identity,
            machine_package_identity,
        } => {
            encoder.byte(6);
            encoder.string(machine_identity)?;
            encoder.optional_package_identity(*machine_package_identity);
        }
        PackagePolicyProviderBinding::Import {
            target,
            locator,
            producer,
        } => {
            encoder.byte(7);
            encoder.string(target)?;
            encode_locator(encoder, locator)?;
            encode_producer(encoder, producer)?;
        }
    }
    Ok(())
}
