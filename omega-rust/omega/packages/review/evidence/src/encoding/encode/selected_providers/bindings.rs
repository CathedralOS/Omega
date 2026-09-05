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
            encoder.tag("string_backed_import_bootstrap", 0);
            encoder.field("library", |encoder| encoder.string(library))?;
            encoder.field("symbol", |encoder| encoder.string(symbol))?;
        }
        PackagePolicyProviderBinding::Syscall { number, evaluated } => {
            encoder.tag("syscall", 1);
            encoder.field("number", |encoder| {
                encoder.i64(*number);
                Ok(())
            })?;
            encoder.field("evaluated", |encoder| {
                encoder.option(evaluated.as_ref(), |encoder, evaluated| {
                    encoder.field("target", |encoder| encoder.string(&evaluated.target))?;
                    encoder.field("producer", |encoder| {
                        encode_producer(encoder, &evaluated.producer)
                    })
                })
            })?;
        }
        PackagePolicyProviderBinding::CompilerIntrinsic { machine } => {
            encoder.tag("compiler_intrinsic", 2);
            encoder.field("machine", |encoder| encoder.string(machine))?;
        }
        PackagePolicyProviderBinding::VtableSlot { index } => {
            encoder.tag("vtable_slot", 3);
            encoder.field("index", |encoder| {
                encoder.i64(*index);
                Ok(())
            })?;
        }
        PackagePolicyProviderBinding::VtableField {
            table,
            field,
            table_declaration,
        } => {
            encoder.tag("vtable_field", 4);
            encoder.field("table", |encoder| encoder.string(table))?;
            encoder.field("field", |encoder| encoder.string(field))?;
            encoder.field("table_declaration", |encoder| {
                encode_nominal(encoder, table_declaration)
            })?;
        }
        PackagePolicyProviderBinding::TableFunction {
            table,
            field,
            table_declaration,
        } => {
            encoder.tag("table_function", 5);
            encoder.field("table", |encoder| encoder.string(table))?;
            encoder.field("field", |encoder| encoder.string(field))?;
            encoder.field("table_declaration", |encoder| {
                encode_nominal(encoder, table_declaration)
            })?;
        }
        PackagePolicyProviderBinding::CheckedAdapter {
            machine_identity,
            machine_package_identity,
        } => {
            encoder.tag("checked_adapter", 6);
            encoder.field("machine_identity", |encoder| {
                encoder.string(machine_identity)
            })?;
            encoder.field("machine_package_identity", |encoder| {
                encoder.optional_package_identity(*machine_package_identity);
                Ok(())
            })?;
        }
        PackagePolicyProviderBinding::Import {
            target,
            locator,
            producer,
        } => {
            encoder.tag("import", 7);
            encoder.field("target", |encoder| encoder.string(target))?;
            encoder.field("locator", |encoder| encode_locator(encoder, locator))?;
            encoder.field("producer", |encoder| encode_producer(encoder, producer))?;
        }
    }
    Ok(())
}
