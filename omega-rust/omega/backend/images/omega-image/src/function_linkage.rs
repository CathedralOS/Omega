//! A second, independent derivation of the object-to-image symbol
//! correspondence, so the reused handle coordinates are re-proved rather than
//! trusted.

use crate::{FinalImage, FinalImageSection};
use omega_object_file::{ObjectPlan, SectionKind, SymbolKind, SymbolSection};
use psi_diagnostics::Diagnostic;

use crate::symbols::final_image_symbol_handle;

pub fn validate_final_image_function_linkage(
    image: &FinalImage,
    object: &ObjectPlan,
) -> Result<(), Diagnostic> {
    if image.target != object.target {
        return Err(Diagnostic::error(
            "final image function linkage target does not match the object plan",
        ));
    }
    let expected_entry = final_image_symbol_handle(object.layout.entry_symbol);
    if !expected_entry.is_valid() || image.symbol_table.entry_symbol != expected_entry {
        return Err(Diagnostic::error(
            "final image does not retain the exact object entry-symbol identity",
        ));
    }

    let mut bound_symbols = Vec::with_capacity(object.layout.function_symbols.len());
    for (binding_index, binding) in object.layout.function_symbols.iter() {
        let Some((object_symbol_handle, object_symbol)) =
            omega_object_file::object_function_symbol(object, binding.identity)
        else {
            return Err(Diagnostic::error(format!(
                "final image function binding #{} is not exact and unique",
                binding_index.arena_index()
            )));
        };
        if bound_symbols.contains(&object_symbol_handle) {
            return Err(Diagnostic::error(format!(
                "final image function binding #{} aliases another function symbol",
                binding_index.arena_index()
            )));
        }
        bound_symbols.push(object_symbol_handle);

        let final_handle = final_image_symbol_handle(object_symbol_handle);
        if !image.symbol_table.symbols.is_valid(final_handle) {
            return Err(Diagnostic::error(format!(
                "final image lost function binding #{}",
                binding_index.arena_index()
            )));
        }
        let final_symbol = image.symbol_table.symbols.get(final_handle);
        if final_symbol.name != object_symbol.name
            || final_symbol.section != FinalImageSection::Text
            || final_symbol.offset != object_symbol.offset
            || final_symbol.size != object_symbol.size
            || final_symbol.kind != SymbolKind::Function
        {
            return Err(Diagnostic::error(format!(
                "final image function binding #{} drifted from its exact object text symbol",
                binding_index.arena_index()
            )));
        }
    }

    let object_function_symbols = object
        .layout
        .symbols
        .iter()
        .filter(|(_, symbol)| {
            symbol.kind == SymbolKind::Function
                && symbol.section == SymbolSection::Section(SectionKind::Text)
        })
        .count();
    let final_function_symbols = image
        .symbol_table
        .symbols
        .iter()
        .filter(|(_, symbol)| {
            symbol.kind == SymbolKind::Function && symbol.section == FinalImageSection::Text
        })
        .count();
    if object_function_symbols != bound_symbols.len()
        || final_function_symbols != bound_symbols.len()
        || !bound_symbols.contains(&object.layout.entry_symbol)
    {
        return Err(Diagnostic::error(
            "final image function symbols are not an exact identity-owned copy of the object plan",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FinalImageInput, FinalImageSymbol, build_final_image};
    use omega_function_identity::{MachineFunctionIdentity, StateKey};
    use omega_object_file::{FunctionSymbolPlan, SymbolPlan};
    use omega_target::NativeTarget;

    fn linked_image() -> (FinalImage, ObjectPlan) {
        let target = NativeTarget::linux_x64();
        let identity = MachineFunctionIdentity::source(StateKey {
            machine: psi_arena::Handle::from_parts(1, 2),
            state: psi_arena::Handle::from_parts(3, 4),
            segment_index: 5,
        });
        let mut object = ObjectPlan::with_capacities(target, 0, 1, 1);
        let symbol = object.layout.symbols.insert(SymbolPlan {
            name: omega_object_file::entry_symbol_name(target),
            section: SymbolSection::Section(SectionKind::Text),
            offset: 0,
            size: 8,
            kind: SymbolKind::Function,
            import_library: String::new(),
        });
        object.layout.entry_symbol = symbol;
        object
            .layout
            .function_symbols
            .insert(FunctionSymbolPlan { identity, symbol });
        let image = build_final_image(FinalImageInput {
            target,
            object: &object,
            relocations: &omega_object_file::RelocationPlan::with_target(target),
            text_bytes: &[0; 8],
            data_bytes: &[],
        });
        (image, object)
    }

    #[test]
    fn final_image_retains_exact_identity_owned_function_symbols() {
        let (image, object) = linked_image();
        validate_final_image_function_linkage(&image, &object)
            .expect("exact final-image function carrier");

        let function = image.symbol_table.entry_symbol;
        for mutate in [
            |symbol: &mut FinalImageSymbol| symbol.name.push_str("_drift"),
            |symbol: &mut FinalImageSymbol| symbol.offset += 1,
            |symbol: &mut FinalImageSymbol| symbol.size += 1,
            |symbol: &mut FinalImageSymbol| symbol.section = FinalImageSection::Data,
            |symbol: &mut FinalImageSymbol| symbol.kind = SymbolKind::Object,
        ] {
            let mut drifted = image.clone();
            mutate(drifted.symbol_table.symbols.get_mut(function));
            assert!(validate_final_image_function_linkage(&drifted, &object).is_err());
        }

        let mut wrong_entry = image.clone();
        wrong_entry.symbol_table.entry_symbol = psi_arena::Handle::invalid();
        assert!(validate_final_image_function_linkage(&wrong_entry, &object).is_err());

        let mut extra = image;
        extra.symbol_table.symbols.insert(FinalImageSymbol {
            name: "unowned".into(),
            section: FinalImageSection::Text,
            offset: 8,
            size: 4,
            kind: SymbolKind::Function,
        });
        assert!(validate_final_image_function_linkage(&extra, &object).is_err());
    }
}
