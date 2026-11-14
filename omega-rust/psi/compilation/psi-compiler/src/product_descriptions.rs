/*
We answer a Build's questions about Product declarations
(`describe_schema`, `describe_provider`, `describe_entry`).
*/

/*
A Build may ask for a Product declaration by path and receive a description
it can hand back to a later request: `BuildProduct::entry(&builder.product,
"support::setup::launch", slot)` yields the entry a root binding then names
(`wiki/spec/build/scoped_execution.md`, "Selecting product declarations
without executing them"). Nothing of the Product runs. The answer is the one
declaration the path selects from where the Build asked.

A path is read in the package of the Build source that wrote it. A bare path
names a declaration of that package's Product. A path through a Product
dependency alias names a public declaration of that dependency. Borrowing the
Build grants no other package's names, and the ordinary temporal rule still
decides whether the declaration exists yet where the question was asked.

A description says which declaration, and no more. Whether a described
machine is a valid entry for its slot is judged where the entry is checked,
as it is for an entry bound by a written path (`root_bindings`). So two
visible machines of one name are refused as ambiguous here; neither is
preferred for fitting the slot.
*/

use language_semantics::declaration_selection::{
    DescribedProductEntry, DescribedProductProvider, DescribedProductSchema,
};
use semantic_vocabulary::ProgramEntrySourceCatalog;
use source::SourceSpan;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

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

pub(crate) fn describe_schema(
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
        declaration: symbols::RetainedDeclaration::capture(&program.symbols, selected)
            .map_err(str::to_owned)?,
        canonical_path: program.symbols.display_path(selected, "::"),
    })
}

pub(crate) fn describe_provider(
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
        declaration: symbols::RetainedDeclaration::capture(&program.symbols, selected)
            .map_err(str::to_owned)?,
        canonical_path: program.symbols.display_path(selected, "::"),
    })
}

pub(crate) fn describe_entry(
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
        declaration: symbols::RetainedDeclaration::capture(&program.symbols, selected)
            .map_err(str::to_owned)?,
        machine_name: machine.name.as_str().to_owned(),
        slot: slot.to_owned(),
    })
}
