//! Render the foreign implementation selected by each evaluated native Build binding.

use super::{Names, Output};
use review_evidence::record::{
    PackagePolicyNativeBinding, PackagePolicyProviderBinding, PackageReviewForeignLocator,
};
use std::fmt::{self, Write};

impl Names<'_> {
    pub(super) fn native_binding(
        &self,
        output: &mut Output,
        native: &PackagePolicyNativeBinding,
    ) -> fmt::Result {
        let requirement = self.name(native.method().requirement());
        match &native.application {
            review_evidence::record::PackagePolicyNativeApplication::Concrete(application) => {
                write!(output, "  native binding {requirement}")?;
                if application.is_some() {
                    let [calling] = native.method().calling() else {
                        return output
                            .fail("native application lost its unique checked calling plan");
                    };
                    write!(output, "<")?;
                    for (index, operand) in calling.abi().operands.iter().enumerate() {
                        if index != 0 {
                            write!(output, ", ")?;
                        }
                        write!(output, "{}", operand.identity)?;
                    }
                    write!(output, ">")?;
                }
            }
            review_evidence::record::PackagePolicyNativeApplication::UnusedGeneric {
                unbound_parameters,
                ..
            } => {
                write!(
                    output,
                    "  unused generic native import {requirement}<{}>",
                    unbound_parameters.join(", ")
                )?;
            }
        }
        match &native.binding {
            PackagePolicyProviderBinding::Import { locator, .. } => match locator {
                PackageReviewForeignLocator::PeByName { library, export } => writeln!(
                    output,
                    " -> DLL \"{}\" export \"{}\"",
                    library.escape_ascii(),
                    export.escape_ascii()
                )?,
                PackageReviewForeignLocator::PeByOrdinal { library, ordinal } => writeln!(
                    output,
                    " -> DLL \"{}\" ordinal {ordinal}",
                    library.escape_ascii()
                )?,
                PackageReviewForeignLocator::ElfVersioned {
                    object,
                    symbol,
                    version,
                } => writeln!(
                    output,
                    " -> ELF \"{}\" symbol \"{}\" version \"{}\"",
                    object.escape_ascii(),
                    symbol.escape_ascii(),
                    version.escape_ascii()
                )?,
                PackageReviewForeignLocator::MachODylibSymbol {
                    install_name,
                    symbol,
                } => writeln!(
                    output,
                    " -> dylib \"{}\" symbol \"{}\"",
                    install_name.escape_ascii(),
                    symbol.escape_ascii()
                )?,
            },
            PackagePolicyProviderBinding::Syscall { number, .. } => {
                writeln!(output, " -> syscall {number}")?
            }
            PackagePolicyProviderBinding::FunctionField { selector, .. } => writeln!(
                output,
                " -> parameter {} field \"{}\"",
                selector.table.declaration_position,
                selector.field.name.escape_ascii()
            )?,
            _ => return output.fail("unrecognized native binding in checked package policy"),
        }
        let Some(producer) = native.producer() else {
            return output.fail("native binding is missing its evaluated Build producer");
        };
        writeln!(
            output,
            "    supplied by {}",
            self.name(producer.declaration())
        )?;
        self.method(output, native.method())
    }
}
