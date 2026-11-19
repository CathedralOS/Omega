//! Resolve Build requests for declarations from their authored source scope.

pub mod authored_requests;
mod opaque_representations;
mod root_bindings;
mod root_grants;
pub use opaque_representations::collect_opaque_representations;
pub use root_bindings::{RootBinding, collect_root_bindings};
pub use root_grants::collect_product_root_grants;

use crate::declaration_selection::DescribedProductEntry;
use crate::declaration_selection::DescribedProductProvider;
use crate::declaration_selection::DescribedProductSchema;
use crate::source_state::TypedTrees;
use semantic_vocabulary::ProgramEntrySourceCatalog;
use source_text::SourceSpan;
use symbols::SymbolHandle;

/// Retain the exact lexical machine occurrence that issued a Build selection.
pub fn capture_build_selection_origin(
    symbols: &crate::source_symbols::SymbolTable,
    activation: SymbolHandle,
    author: SymbolHandle,
    occurrence: SourceSpan,
    event_ordinal: Option<u64>,
) -> Result<crate::declaration_selection::BuildSelectionOrigin, &'static str> {
    let activation = crate::source_symbols::retain_declaration(symbols, activation)?;
    let author = crate::source_symbols::retain_declaration(symbols, author)?;
    if author.source().source_id != occurrence.source_id {
        return Err("Build selection occurrence does not belong to its lexical author");
    }
    crate::declaration_selection::BuildSelectionOrigin::from_retained_parts(
        activation,
        author,
        occurrence.span,
        event_ordinal,
    )
}

/// Where a path is read from, and what it may name there.
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

    fn contains(&self, program: &TypedTrees, symbol: SymbolHandle, is_public: bool) -> bool {
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

    /// The path with its dependency alias removed, as a declaration of any
    /// package would be spelled: used only to say why nothing was selected.
    fn probe(name: &str) -> &str {
        name.split_once("::").map_or(name, |(_, path)| path)
    }
}

fn path_text<'path>(path: &'path [u8], role: &str) -> Result<&'path str, String> {
    let name = std::str::from_utf8(path)
        .map_err(|_| format!("product {role} path must be canonical UTF-8"))?;
    if name.is_empty() {
        return Err(format!(
            "product {role} selection requires a non-empty path"
        ));
    }
    Ok(name)
}

/// The one declaration among `declarations` that `path` selects from
/// `occurrence`. `missing` and `hidden` finish the two refusals: nothing of
/// that path exists, or something does and cannot be named from here.
fn select_declaration(
    program: &TypedTrees,
    occurrence: SourceSpan,
    path: &[u8],
    role: &str,
    missing: &str,
    hidden: &str,
    declarations: impl Iterator<Item = (SymbolHandle, bool)>,
) -> Result<SymbolHandle, String> {
    let name = path_text(path, role)?;
    let scope = ProductScope::new(program, occurrence, name);
    let probe = ProductScope::probe(name);
    let mut selected = None;
    let mut inaccessible = false;
    for (symbol, is_public) in declarations {
        inaccessible |= program.symbols.name(symbol) == probe
            || program.symbols.display_path(symbol, "::") == probe;
        if !scope.contains(program, symbol, is_public) {
            continue;
        }
        if selected.replace(symbol).is_some() {
            return Err(format!(
                "product {role} `{name}` is ambiguous within its package"
            ));
        }
    }
    selected.ok_or_else(|| {
        let reason = if inaccessible { hidden } else { missing };
        format!("product {role} `{name}` {reason} this build occurrence's package")
    })
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
        "names no data declaration in",
        "is not a data declaration visible from",
        program
            .data_definitions()
            .iter()
            .map(|definition| (definition.symbol, definition.is_public)),
    )?;
    Ok(DescribedProductSchema {
        declaration: crate::source_symbols::retain_declaration(&program.symbols, selected)
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
        "names no provider declaration in",
        "is not a provider declaration visible from",
        program
            .conformances()
            .iter()
            .map(|implementation| (implementation.symbol, implementation.is_public)),
    )?;
    Ok(DescribedProductProvider {
        declaration: crate::source_symbols::retain_declaration(&program.symbols, selected)
            .map_err(str::to_owned)?,
        canonical_path: program.symbols.display_path(selected, "::"),
    })
}

pub fn describe_entry(
    program: &TypedTrees,
    occurrence: SourceSpan,
    path: &[u8],
    slot: &[u8],
    slots: &dyn ProgramEntrySourceCatalog,
) -> Result<DescribedProductEntry, String> {
    let slot = std::str::from_utf8(slot)
        .map_err(|_| "product entry slot must be canonical UTF-8".to_owned())?;
    if slot.is_empty() || path.is_empty() {
        return Err("product entry selection requires a non-empty path and slot".to_owned());
    }
    let selected = select_declaration(
        program,
        occurrence,
        path,
        "entry",
        "names no machine in",
        "is not a product declaration visible from",
        program
            .machines()
            .iter()
            .map(|machine| (machine.symbol, machine.is_public)),
    )
    .map_err(|reason| {
        // Two machines of one name: the description does not choose between
        // them by what the slot would accept.
        if reason.ends_with("is ambiguous within its package") {
            format!("{reason}: more than one visible machine has that name")
        } else {
            reason
        }
    })?;
    // The slot must be one the target catalog declares; what it requires of
    // its entry is judged with the entry.
    slots.source_contract(slot)?;
    let machine = program
        .machine(selected)
        .ok_or("product entry lost its machine declaration")?;
    Ok(DescribedProductEntry {
        declaration: crate::source_symbols::retain_declaration(&program.symbols, selected)
            .map_err(str::to_owned)?,
        machine_name: machine.name.as_str().to_owned(),
        slot: slot.to_owned(),
    })
}

pub fn describe_parameter(
    program: &TypedTrees,
    occurrence: SourceSpan,
    selecting: SourceSpan,
    path: &[u8],
) -> Result<semantic_vocabulary::ParameterDescription, String> {
    let declarations = crate::source_state::product_operand::product_candidates(
        program,
        crate::declaration_selection::ProductOperandRole::Formal,
    );
    let selected = select_declaration(
        program,
        occurrence,
        path,
        "parameter",
        "names no formal in",
        "is not a formal visible from",
        declarations.into_iter(),
    )?;
    let author = crate::source_state::product_operand::ProductRequestAuthors::new(program)
        .resolve(selecting)?;
    crate::source_state::reflection::construction::product_parameter_description_identity(
        program,
        selected,
        author,
        occurrence,
        path_text(path, "parameter")?,
    )
}

pub fn describe_field(
    program: &TypedTrees,
    occurrence: SourceSpan,
    selecting: SourceSpan,
    path: &[u8],
) -> Result<semantic_vocabulary::FieldInfoDescription, String> {
    let declarations = crate::source_state::product_operand::product_candidates(
        program,
        crate::declaration_selection::ProductOperandRole::Field,
    );
    let selected = select_declaration(
        program,
        occurrence,
        path,
        "field",
        "names no field in",
        "is not a field visible from",
        declarations.into_iter(),
    )?;
    let author = crate::source_state::product_operand::ProductRequestAuthors::new(program)
        .resolve(selecting)?;
    crate::source_state::reflection::construction::product_field_description_identity(
        program,
        selected,
        author,
        occurrence,
        path_text(path, "field")?,
    )
}
