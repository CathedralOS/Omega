//! Join an image's exact import rows to the bodies already carried by encoding.

use crate::{FinalImage, FinalImageImportPlan, FinalImageSymbolHandle};
use diagnostics::Diagnostic;
use machine_code::import_code::{EncodedImportBody, ImportCodeIdentity};

impl FinalImage {
    pub fn import_code_identity(
        &self,
        symbol: FinalImageSymbolHandle,
    ) -> Result<ImportCodeIdentity, Diagnostic> {
        if !self.symbol_table.symbols.is_valid(symbol) {
            return Err(Diagnostic::error(
                "encoded import references an invalid image symbol",
            ));
        }
        let mut rows = self
            .symbol_table
            .imports
            .iter()
            .filter(|(_, row)| row.symbol_handle == symbol);
        let (_, row) = rows
            .next()
            .ok_or_else(|| Diagnostic::error("encoded import has no exact image import row"))?;
        if rows.next().is_some() {
            return Err(Diagnostic::error(
                "encoded import has duplicate image import rows",
            ));
        }
        match &row.import {
            FinalImageImportPlan::None => Err(Diagnostic::error(
                "image import has no retained physical import plan",
            )),
            FinalImageImportPlan::Normalized(locator) => {
                Ok(ImportCodeIdentity::Normalized(locator.clone()))
            }
            FinalImageImportPlan::StringBackedBootstrap { library } => {
                Ok(ImportCodeIdentity::StringBackedBootstrap {
                    library: library.clone(),
                    symbol: self.symbol_table.symbols.get(symbol).name.clone(),
                })
            }
        }
    }

    pub fn encoded_import_body(
        &self,
        symbol: FinalImageSymbolHandle,
    ) -> Result<&EncodedImportBody, Diagnostic> {
        if self.import_program.target != self.target {
            return Err(Diagnostic::error(
                "encoded import program belongs to another image target",
            ));
        }
        self.import_program
            .body(&self.import_code_identity(symbol)?)
            .ok_or_else(|| {
                Diagnostic::error("image import lacks one exact previously encoded body")
            })
    }

    pub fn validate_import_program(&self) -> Result<(), Diagnostic> {
        if self.import_program.target != self.target
            || self.import_program.imports.len() != self.symbol_table.imports.len()
        {
            return Err(Diagnostic::error(
                "image import rows differ from the closed encoded program",
            ));
        }
        for (_, row) in self.symbol_table.imports.iter() {
            self.encoded_import_body(row.symbol_handle)?;
        }
        let resolver_required = self.target.object_format == target::ObjectFormat::Elf
            && !self.import_program.imports.is_empty();
        if self.import_program.resolver.is_some() != resolver_required {
            return Err(Diagnostic::error(
                "encoded import resolver does not match the image import roster",
            ));
        }
        Ok(())
    }
}
