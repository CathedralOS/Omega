//! Exact foreign locator vocabulary, shared by policy provider bindings.

use super::*;

pub(crate) fn encode_locator(
    encoder: &mut Encoder,
    locator: &PackageReviewForeignLocator,
) -> Result<(), PackageReviewEncodingError> {
    match locator {
        PackageReviewForeignLocator::PeByName { library, export } => {
            encoder.tag("pe_by_name", 0);
            encoder.field("library", |encoder| encoder.bytes(library))?;
            encoder.field("export", |encoder| encoder.bytes(export))?;
        }
        PackageReviewForeignLocator::PeByOrdinal { library, ordinal } => {
            encoder.tag("pe_by_ordinal", 1);
            encoder.field("library", |encoder| encoder.bytes(library))?;
            encoder.field("ordinal", |encoder| {
                encoder.u16(*ordinal);
                Ok(())
            })?;
        }
        PackageReviewForeignLocator::ElfVersioned {
            object,
            symbol,
            version,
        } => {
            encoder.tag("elf_versioned", 2);
            encoder.field("object", |encoder| encoder.bytes(object))?;
            encoder.field("symbol", |encoder| encoder.bytes(symbol))?;
            encoder.field("version", |encoder| encoder.bytes(version))?;
        }
        PackageReviewForeignLocator::MachODylibSymbol {
            install_name,
            symbol,
        } => {
            encoder.tag("macho_dylib_symbol", 3);
            encoder.field("install_name", |encoder| encoder.bytes(install_name))?;
            encoder.field("symbol", |encoder| encoder.bytes(symbol))?;
        }
    }
    Ok(())
}
