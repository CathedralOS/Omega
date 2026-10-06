use checked_trees::CheckedTrees;
use symbols::SymbolHandle;

/// Same-program routing custody for one compiler-resolved filesystem boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FilesystemServiceBinding {
    checked_program_address: usize,
    declaration_symbol: SymbolHandle,
}

impl FilesystemServiceBinding {
    pub fn from_compiler_resolved_declaration(
        checked: &CheckedTrees,
        declaration_symbol: SymbolHandle,
    ) -> Result<Self, &'static str> {
        if !declaration_symbol.is_valid()
            || checked
                .typed
                .traits()
                .iter()
                .filter(|definition| {
                    definition.is_boundary && definition.symbol == declaration_symbol
                })
                .count()
                != 1
        {
            return Err("filesystem service binding is not one exact checked boundary declaration");
        }
        Ok(Self {
            checked_program_address: std::ptr::from_ref(checked).addr(),
            declaration_symbol,
        })
    }

    pub(crate) fn declaration_symbol_for(
        self,
        checked: &CheckedTrees,
    ) -> Result<SymbolHandle, &'static str> {
        if self.checked_program_address != std::ptr::from_ref(checked).addr() {
            return Err("filesystem service binding belongs to a different checked program");
        }
        Ok(self.declaration_symbol)
    }
}
