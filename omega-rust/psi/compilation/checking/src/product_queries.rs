//! Demand-time Product declaration queries, independent of an execution engine.
//!
//! Locators select within the query's lexical Product scope. Compatibility is
//! supplied by the slot owner and cannot execute Product bodies. The result
//! retains source identity; its runtime carrier and activation custody belong
//! to the caller, not to this declaration lookup.

use language_semantics::declaration_selection::{
    DescribedProductEntry, DescribedProductProvider, DescribedProductSchema,
};
use source::SourceSpan;
use typed_trees::TypedTrees;
use typed_trees::product_operand::{ProductEntryCompatibility, ProductEntryDeclaration};

/// The lexical Product namespace is common to every description role. A
/// dependency alias selects that exact package's public declarations; otherwise
/// only the author's Product instance is visible, including its private names.
struct ProductScope<'path> {
    occurrence: SourceSpan,
    path: &'path str,
    qualified: bool,
    dependency: Option<semantic_vocabulary::PackageKeyIdentity>,
}

impl<'path> ProductScope<'path> {
    fn new(program: &TypedTrees, occurrence: SourceSpan, name: &'path str) -> Self {
        let qualified = name.split_once("::");
        let dependency = qualified
            .and_then(|(alias, _)| program.symbols.product_dependency_target(occurrence, alias));
        Self {
            occurrence,
            path: match (qualified, dependency) {
                (Some((_, path)), Some(_)) => path,
                _ => name,
            },
            qualified: qualified.is_some(),
            dependency,
        }
    }

    fn contains(
        &self,
        program: &TypedTrees,
        symbol: symbols::SymbolHandle,
        is_public: bool,
    ) -> bool {
        let matches_path = if self.qualified {
            program.symbols.display_path(symbol, "::") == self.path
        } else {
            program.symbols.name(symbol) == self.path
        };
        let authorized = match self.dependency {
            Some(package) => {
                is_public
                    && program.symbols.symbol_product_package_identity(symbol) == Some(package)
            }
            None => program
                .symbols
                .symbol_source_span(symbol)
                .is_some_and(|span| {
                    program
                        .symbols
                        .same_product_package_instance(self.occurrence, span)
                }),
        };
        matches_path
            && authorized
            && program
                .symbols
                .source_reference_can_describe_product_symbol(self.occurrence, symbol)
    }
}

pub fn describe_schema(
    program: &TypedTrees,
    occurrence: SourceSpan,
    path: &[u8],
) -> Result<DescribedProductSchema, String> {
    let selected = select_declaration(
        program,
        occurrence,
        path,
        "schema",
        "data",
        program
            .data_definitions()
            .iter()
            .map(|definition| (definition.symbol, definition.is_public)),
    )?;
    Ok(DescribedProductSchema {
        declaration: symbols::RetainedDeclaration::capture(&program.symbols, selected)
            .map_err(str::to_owned)?,
        canonical_path: program.symbols.display_path(selected, "::"),
    })
}

pub fn describe_provider(
    program: &TypedTrees,
    occurrence: SourceSpan,
    path: &[u8],
) -> Result<DescribedProductProvider, String> {
    let selected = select_declaration(
        program,
        occurrence,
        path,
        "provider",
        "provider",
        program
            .conformances()
            .iter()
            .map(|implementation| (implementation.symbol, implementation.is_public)),
    )?;
    Ok(DescribedProductProvider {
        declaration: symbols::RetainedDeclaration::capture(&program.symbols, selected)
            .map_err(str::to_owned)?,
        canonical_path: program.symbols.display_path(selected, "::"),
    })
}

/// Resolve a declaration roster under the author's Product scope. The roster
/// supplies the semantic role; a same-spelled declaration of another kind is
/// not a candidate. No selected body executes or grants provider admission.
fn select_declaration(
    program: &TypedTrees,
    occurrence: SourceSpan,
    path: &[u8],
    role: &str,
    declaration_kind: &str,
    declarations: impl Iterator<Item = (symbols::SymbolHandle, bool)>,
) -> Result<symbols::SymbolHandle, String> {
    let name = std::str::from_utf8(path)
        .map_err(|_| format!("product {role} path must be canonical UTF-8"))?;
    if name.is_empty() {
        return Err(format!(
            "product {role} selection requires a non-empty path"
        ));
    }
    let scope = ProductScope::new(program, occurrence, name);
    let mut selected = None;
    let mut inaccessible = false;
    for (symbol, is_public) in declarations {
        let canonical = program.symbols.display_path(symbol, "::");
        let leaf = program.symbols.name(symbol);
        let probe = name.split_once("::").map_or(name, |(_, path)| path);
        inaccessible |= leaf == probe || canonical == probe;
        if !scope.contains(program, symbol, is_public) {
            continue;
        }
        if selected.replace(symbol).is_some() {
            return Err(format!(
                "product {role} `{name}` is ambiguous within its package"
            ));
        }
    }
    selected.ok_or_else(|| if inaccessible {
        format!("product {role} `{name}` is not a {declaration_kind} declaration visible from this build occurrence's package")
    } else {
        format!("product {role} `{name}` names no {declaration_kind} declaration in this build occurrence's package")
    })
}

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
    let scope = ProductScope::new(program, occurrence, name);
    let mut visible = false;
    let mut selected = None;
    let mut rejection = None;
    for machine in program.machines() {
        if !scope.contains(program, machine.symbol, machine.is_public) {
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
        let probe = name.split_once("::").map_or(name, |(_, path)| path);
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
