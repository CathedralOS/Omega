use super::{Error, Reader, nominal, package};
use crate::encoding::recovery::policy::external::{locator, producer};
use crate::record::{PackagePolicyProviderBinding, PackagePolicyProviderEvaluatedSyscall};

pub(super) fn binding(reader: &mut Reader<'_>) -> Result<PackagePolicyProviderBinding, Error> {
    use PackagePolicyProviderBinding as Binding;
    Ok(match reader.byte()? {
        0 => Binding::StringBackedImportBootstrap {
            library: reader.string()?,
            symbol: reader.string()?,
        },
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
        },
        5 => Binding::TableFunction {
            table: reader.string()?,
            field: reader.string()?,
            table_declaration: nominal(reader)?,
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
        _ => return Err(Error::InvalidTag),
    })
}
