//! Demand-time Product declaration queries, independent of an execution engine.
//!
//! Locators select within the query's lexical Product scope. Compatibility is
//! supplied by the slot owner and cannot execute Product bodies. The result
//! retains source identity; its runtime carrier and activation custody belong
//! to the caller, not to this declaration lookup.

use language_semantics::declaration_selection::DescribedProductEntry;
use source::SourceSpan;
use typed_trees::TypedTrees;
use typed_trees::product_operand::{ProductEntryCompatibility, ProductEntryDeclaration};

pub fn describe_entry(
    program: &TypedTrees,
    occurrence: SourceSpan,
    path: &[u8],
    slot: &[u8],
    compatibility: &dyn ProductEntryCompatibility,
) -> Result<DescribedProductEntry, String> {
    let name = std::str::from_utf8(path)
        .map_err(|_| "product entry path must be canonical UTF-8".to_owned())?;
    let slot = std::str::from_utf8(slot)
        .map_err(|_| "product entry slot must be canonical UTF-8".to_owned())?;
    if name.is_empty() || slot.is_empty() {
        return Err("product entry selection requires a non-empty path and slot".to_owned());
    }
    let qualified = name.split_once("::");
    let dependency = qualified
        .and_then(|(alias, _)| program.symbols.product_dependency_target(occurrence, alias));
    let path = match (qualified, dependency) {
        (Some((_, path)), Some(_)) => path,
        _ => name,
    };
    let mut visible = false;
    let mut selected = None;
    let mut rejection = None;
    for machine in program.machines() {
        let matches_path = if dependency.is_some() || qualified.is_some() {
            program.symbols.display_path(machine.symbol, "::") == path
        } else {
            machine.name.as_str() == path
        };
        let authorized = match dependency {
            Some(package) => {
                machine.is_public
                    && program
                        .symbols
                        .symbol_product_package_identity(machine.symbol)
                        == Some(package)
            }
            None => program
                .symbols
                .symbol_source_span(machine.symbol)
                .is_some_and(|span| {
                    program
                        .symbols
                        .same_product_package_instance(occurrence, span)
                }),
        };
        if !matches_path
            || !authorized
            || !program
                .symbols
                .source_reference_can_describe_product_symbol(occurrence, machine.symbol)
        {
            continue;
        }
        visible = true;
        let declaration = ProductEntryDeclaration::from_machine(program, machine);
        match compatibility.validate_entry(program, declaration, slot) {
            Ok(()) => {
                if selected.is_some() {
                    return Err(format!(
                        "product entry `{name}` is ambiguous: more than one visible declaration is compatible with slot `{slot}`"
                    ));
                }
                selected = Some(machine);
            }
            Err(reason) => {
                rejection.get_or_insert(reason);
            }
        }
    }
    if !visible {
        let probe = qualified.map_or(name, |(_, path)| path);
        let inaccessible = program.machines().iter().any(|machine| {
            machine.name.as_str() == probe
                || program.symbols.display_path(machine.symbol, "::") == probe
        });
        return Err(if inaccessible {
            format!(
                "product entry `{name}` is not a product declaration visible from this build occurrence's package"
            )
        } else {
            format!("product entry `{name}` names no machine in this build occurrence's package")
        });
    }
    let machine = selected.ok_or_else(|| {
        format!(
            "product entry `{name}` is incompatible with slot `{slot}`: {}",
            rejection.as_deref().unwrap_or("no compatible declaration"),
        )
    })?;
    Ok(DescribedProductEntry {
        declaration: symbols::RetainedDeclaration::capture(&program.symbols, machine.symbol)
            .map_err(str::to_owned)?,
        machine_name: machine.name.as_str().to_owned(),
        slot: slot.to_owned(),
    })
}
