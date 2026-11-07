// Bounded recovery for selected-provider bindings and table-field storage.
// Provider-binding tag `0` carried the retired string-backed import bootstrap; recovery rejects it as `RetiredVocabulary` rather than reinterpreting two authored strings as a physical locator.

use super::{Error, Reader, nominal, package};
use crate::encoding::recovery::policy::external::{locator, producer};
use crate::record::{PackagePolicyProviderBinding, PackagePolicyProviderEvaluatedSyscall};

fn table_field_storage(
    reader: &mut Reader<'_>,
) -> Result<Result<effects::provider_plan::TableFieldStorage, String>, Error> {
    Ok(super::super::tag!(reader => {
        0 => Ok(effects::provider_plan::TableFieldStorage {
            table_application: reader.string()?,
            field_declaration: reader.string()?,
        }),
        1 => Err(reader.string()?),
    }))
}

pub(in crate::encoding::recovery) fn binding(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyProviderBinding, Error> {
    use PackagePolicyProviderBinding as Binding;
    Ok(super::super::tag!(reader => {
        0 => return Err(Error::RetiredVocabulary),
        1 => Binding::Syscall {
            number: reader.i64()?,
            evaluated: reader.option(|reader| {
                Ok(PackagePolicyProviderEvaluatedSyscall {
                    target: reader.string()?,
                    producer: producer(reader)?,
                })
            })?,
        },
        2 => Binding::CompilerIntrinsic {
            machine: reader.string()?,
        },
        3 => Binding::VtableSlot {
            index: reader.i64()?,
        },
        4 => Binding::VtableField {
            table: reader.string()?,
            field: reader.string()?,
            table_declaration: nominal(reader)?,
            storage: table_field_storage(reader)?,
        },
        5 => Binding::TableFunction {
            table: reader.string()?,
            field: reader.string()?,
            table_declaration: nominal(reader)?,
            storage: table_field_storage(reader)?,
        },
        6 => Binding::CheckedAdapter {
            machine_identity: reader.string()?,
            machine_package_identity: reader.option(package)?,
        },
        7 => Binding::Import {
            target: reader.string()?,
            locator: locator(reader)?,
            producer: producer(reader)?,
        },
    }))
}
