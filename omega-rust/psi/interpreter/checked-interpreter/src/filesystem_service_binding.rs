use checked_trees::CheckedTrees;
use symbols::SymbolHandle;

/// Same-program routing custody for one compiler-resolved filesystem boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FilesystemServiceBinding {
    checked_program_address: usize,
    declaration_symbol: SymbolHandle,
}

impl FilesystemServiceBinding {
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
