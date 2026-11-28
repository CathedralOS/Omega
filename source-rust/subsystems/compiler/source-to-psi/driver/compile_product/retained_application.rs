//! Carry a closed Product operand across package captures. Reopening checks the
//! declaration spans against the receiving sources and requires the same identity.

use crate::declaration_selection::{ProductDeclarationOperand, ProductOperandRole};
use crate::source_state::{TypedTrees, product_operand::ProductOperandApplication};
use arena::Handle;
use diagnostics::Diagnostic;
use retained_product_selections::RetainedProductApplication;
use std::sync::Arc;
use symbols::SymbolHandle;

/// Save the prepared arguments and the identity the compiler gave this application.
pub(super) fn capture(
    typed: &TypedTrees,
    application_handle: Handle<ProductOperandApplication>,
    identity: String,
) -> Result<RetainedProductApplication, String> {
    if !typed
        .product_operand_applications
        .is_valid(application_handle)
    {
        return Err("an applied provider choice lost its prepared application".to_owned());
    }
    let application = typed.product_operand_applications.get(application_handle);
    let owner = typed
        .expression_table
        .static_machine_application(application.supplying_application)
        .lexical_owner;
    Ok(RetainedProductApplication {
        graph: crate::source_state::product_operand::capture_closed_operand(
            typed,
            &application.argument,
            owner,
        )?,
        sources: typed
            .symbols
            .source_map()
            .ok_or("an applied provider choice has no source map for its anchors")?,
        identity,
    })
}

/// Find the declared file among the selected sources of the receiving program.
/// A named package can supply its Product to an interpreted Build; the receiver
/// checks that file again in its Build scope. A package's Build chooses among
/// its own files, which it reads in its Build scope, and its consumer's Product
/// receives that choice over the same files in Product scope. Keep its owner,
/// bytes, origin and location fixed across either transition.
pub(super) fn receiving_file<'a>(
    files: &'a crate::source_inputs::SourceMap,
    original: &source_admission::SourceFile,
    scope: source_provenance::DependencyScope,
) -> Option<&'a source_admission::SourceFile> {
    let mut expected = original.clone();
    if original.dependency_scope != scope
        && (original.package_identity.is_some() || original.origin.is_toolchain())
    {
        expected.dependency_scope = scope;
    }
    let mut matching = files
        .selected_files()
        .filter(|file| file.continues_capture_of(&expected));
    match (matching.next(), matching.next()) {
        (Some(file), None) => Some(file),
        _ => None,
    }
}

/// Use the receiving program's declaration locations to rebuild the application
/// and check its identity.
pub(super) fn prepare(
    retained: &RetainedProductApplication,
    program: &mut TypedTrees,
    role: ProductOperandRole,
    declaration: SymbolHandle,
    authored_path: &str,
    scope: source_provenance::DependencyScope,
) -> Result<Handle<ProductOperandApplication>, Diagnostic> {
    let files = program.symbols.source_map().ok_or_else(|| {
        Diagnostic::error("retained application lost its receiving source inventory")
    })?;
    let own = |span: source_text::SourceSpan| -> Option<source_text::SourceSpan> {
        let file = retained.sources.get(span.source_id)?;
        let here = receiving_file(&files, file, scope)?;
        Some(source_text::SourceSpan::new(here.source_id, span.span))
    };
    let mut graph = retained.graph.clone();
    let mut foreign = false;
    let mut anchor = |declaration: &mut Option<source_text::SourceSpan>| {
        if let Some(span) = *declaration {
            match own(span) {
                Some(here) => *declaration = Some(here),
                None => foreign = true,
            }
        }
    };
    graph
        .names
        .for_each_mut(|_, name| anchor(&mut name.declaration));
    graph
        .operands
        .for_each_mut(|_, operand| anchor(&mut operand.declaration));
    if foreign {
        return Err(Diagnostic::error(format!(
            "retained provider choice `{}` is applied to a declaration in a source this program did not load",
            authored_path
        )));
    }
    let operand = ProductDeclarationOperand {
        path: authored_path.to_owned(),
        source_span: program
            .symbols
            .symbol_provenance_source_span(declaration)
            .unwrap_or_default(),
        application: Some(Arc::new(graph)),
    };
    let application =
        typed_trees::product_operand::formation::prepare_retained_product_application(
            program,
            &operand,
            role,
            declaration,
        )
        .map_err(|mut diagnostics| diagnostics.remove(0))?;
    let here = program
        .product_operand_application_identity(application)
        .map_err(Diagnostic::error)?;
    if here.as_deref() != Some(retained.identity.as_str()) {
        return Err(Diagnostic::error(format!(
            "retained provider choice `{}` forms `{}` here and was chosen as `{}`",
            authored_path,
            here.as_deref().unwrap_or("no application"),
            retained.identity,
        )));
    }
    program.retain_dependency_product_application(application);
    Ok(application)
}
