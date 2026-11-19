// Canonical encoding for selected provider binding mechanisms and table-field storage correspondence (tag 0 is permanently retired).

use super::{Encoder, PackageReviewEncodingError};
use crate::encoding::encode::values::{
    external_policy::{encode_locator, encode_producer},
    identity::encode_nominal,
};
use crate::record::PackagePolicyProviderBinding;

fn encode_table_field_storage(
    encoder: &mut Encoder,
    storage: &Result<effects::provider_plan::TableFieldStorage, String>,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("storage", |encoder| match storage {
        Ok(storage) => {
            encoder.tag("checked", 0);
            encoder.string_field("table_application", &storage.table_application)?;
            encoder.string_field("field_declaration", &storage.field_declaration)
        }
        Err(reason) => {
            encoder.tag("unavailable", 1);
            encoder.string_field("reason", reason)
        }
    })
}

pub(in crate::encoding) fn binding(
    encoder: &mut Encoder,
    binding: &PackagePolicyProviderBinding,
) -> Result<(), PackageReviewEncodingError> {
    match binding {
        PackagePolicyProviderBinding::Syscall { number, evaluated } => {
            encoder.tag("syscall", 1);
            encoder.i64_field("number", *number)?;
            encoder.field("evaluated", |encoder| {
                encoder.option(evaluated.as_ref(), |encoder, evaluated| {
                    encoder.field("target", |encoder| encoder.string(&evaluated.target))?;
                    encoder.field_by("producer", &evaluated.producer, encode_producer)
                })
            })?;
        }
        PackagePolicyProviderBinding::CompilerIntrinsic { machine } => {
            encoder.tag("compiler_intrinsic", 2);
            encoder.field("machine", |encoder| encoder.string(machine))?;
        }
        PackagePolicyProviderBinding::VtableSlot { index } => {
            encoder.tag("vtable_slot", 3);
            encoder.i64_field("index", *index)?;
        }
        PackagePolicyProviderBinding::VtableField {
            table,
            field,
            table_declaration,
            storage,
        } => {
            encoder.tag("vtable_field", 4);
            encoder.field("table", |encoder| encoder.string(table))?;
            encoder.field("field", |encoder| encoder.string(field))?;
            encoder.field_by("table_declaration", table_declaration, encode_nominal)?;
            encode_table_field_storage(encoder, storage)?;
        }
        PackagePolicyProviderBinding::TableFunction {
            table,
            field,
            table_declaration,
            storage,
        } => {
            encoder.tag("table_function", 5);
            encoder.field("table", |encoder| encoder.string(table))?;
            encoder.field("field", |encoder| encoder.string(field))?;
            encoder.field_by("table_declaration", table_declaration, encode_nominal)?;
            encode_table_field_storage(encoder, storage)?;
        }
        PackagePolicyProviderBinding::CheckedAdapter {
            machine_identity,
            machine_package_identity,
        } => {
            encoder.tag("checked_adapter", 6);
            encoder.string_field("machine_identity", machine_identity)?;
            encoder.optional_package_identity_field(
                "machine_package_identity",
                *machine_package_identity,
            )?;
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
