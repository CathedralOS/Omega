//! Close executed opaque representation requests and replay the same ledger
//! against the final typed graph before constructing property receipts.

use crate::representation_trait::is_compiler_owned_opaque_representation_trait;
use diagnostics::Diagnostic;
use representation_selections::{
    OpaqueRepresentationApplicationOrigin, OpaqueRepresentationLifecycleDisposition,
    OpaqueRepresentationSelection,
};
use source::SourceSpan;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::product_operand::ProductOperandKind;

mod carrier_closure;

/// Reconstruct selections from the complete committed ledger in the final
/// graph, retaining the authored frontier before this Build's generated units.
/// Helper execution contributes its own occurrence; unexecuted syntax does not.
pub fn rederive_opaque_representation_selections(
    typed: &TypedTrees,
    selected_build_machine: Option<SymbolHandle>,
    executed: &[language_semantics::declaration_selection::ExecutedBuildProductRequest],
    source_count: usize,
    retained: &[OpaqueRepresentationSelection],
) -> Result<Vec<OpaqueRepresentationSelection>, Vec<Diagnostic>> {
    use language_semantics::declaration_selection::{
        BuildOperation, BuildProductOperand, ExecutedBuildProductRequest,
    };
    use typed_trees::product_operand::{
        resolve_build_request_author, resolve_product_locator_in_frontier,
    };
    let mut derived: Vec<OpaqueRepresentationSelection> = Vec::new();
    for executed in executed {
        let ExecutedBuildProductRequest::DeclarationSelection(selection) = executed else {
            continue;
        };
        let request = &selection.request;
        if request.operation != BuildOperation::RepresentationSelection {
            continue;
        }
        let refuse =
            |reason: String| vec![Diagnostic::error(reason).with_source_span(request.source_span)];
        if selected_build_machine.is_none() {
            return Err(refuse(
                "opaque representation request has no authoritative Build activation".into(),
            ));
        }
        request
            .validate_shape()
            .map_err(|reason| refuse(reason.into()))?;
        let author = resolve_build_request_author(typed, selection.author).map_err(refuse)?;
        let operand = |position, role| {
            let Some(BuildProductOperand::Declaration(locator)) = request.operands.get(position)
            else {
                return Err(refuse(
                    "representation request lost its declaration locator".into(),
                ));
            };
            resolve_product_locator_in_frontier(typed, &locator.path, locator.source_span, role, source_count)
                .ok_or_else(|| refuse(format!("representation operand `{}` no longer resolves in its admitted Product frontier", locator.path)))
        };
        let opaque = operand(0, ProductOperandKind::Data)?;
        let conformance = operand(1, ProductOperandKind::Representation)?;
        let closed = close_resolved_representation_selection(
            typed,
            author,
            opaque,
            conformance,
            request.source_span,
        )
        .map_err(|diagnostic| vec![diagnostic.with_source_span(request.source_span)])?;
        if derived
            .iter()
            .any(|existing| existing.opaque() == closed.opaque())
        {
            return Err(refuse(
                "build selects an opaque representation more than once".into(),
            ));
        }
        derived.push(closed);
    }
    if derived != retained {
        return Err(vec![Diagnostic::error(format!(
            "final opaque-representation closure disagrees with the executed request ledger (retained {} rows, derived {} rows)",
            retained.len(),
            derived.len()
        ))]);
    }
    Ok(derived)
}

/// Close exact symbols selected by the executed Product request ledger.
pub fn close_resolved_representation_selection(
    typed: &TypedTrees,
    selecting_machine: SymbolHandle,
    opaque: SymbolHandle,
    conformance: SymbolHandle,
    source_span: SourceSpan,
) -> Result<OpaqueRepresentationSelection, Diagnostic> {
    let typed = typed.product_declarations();
    let application =
        validation::conformance_applications::close_conformance_declaration(typed, conformance)?;
    close_resolved_application(typed, selecting_machine, opaque, application, source_span)
}

fn close_resolved_application(
    typed: &TypedTrees,
    selecting_machine: SymbolHandle,
    opaque: SymbolHandle,
    application: typed_trees::typed_trees::ClosedConformanceApplication,
    source_span: SourceSpan,
) -> Result<OpaqueRepresentationSelection, Diagnostic> {
    let opaque_name = typed.symbols.display_path(opaque, "::");
    let conformance_name = typed.symbols.display_path(application.declaration, "::");
    let opaque_definition = typed.data_definition_by_symbol(opaque).ok_or_else(|| Diagnostic::error(format!("opaque representation subject `{opaque_name}` does not resolve to an exact data declaration")))?;
    if opaque_definition.supply_mode != language_semantics::DataSupplyMode::BoundaryOpaque {
        return Err(Diagnostic::error(format!(
            "representation subject `{opaque_name}` is not boundary-opaque data"
        )));
    }
    let conformance = typed
        .conformance_by_symbol(application.declaration)
        .ok_or_else(|| Diagnostic::error("selected opaque conformance disappeared"))?;
    // The conformance and its application may carry distinct symbol instances
    // of the same toolchain trait declaration: a source imported on both the
    // product and build scopes is lowered once per scope, so symbol equality
    // would reject a genuine compiler-owned relationship. Identity is the
    // toolchain declaration itself (origin, path, and closed shape), which the
    // helper already checks on both sides.
    if !is_compiler_owned_opaque_representation_trait(typed, conformance.trait_symbol)
        || !is_compiler_owned_opaque_representation_trait(typed, application.trait_definition)
    {
        return Err(Diagnostic::error(format!(
            "conformance `{}` does not satisfy the exact compiler-owned `OpaqueRepresentation` trait",
            conformance_name,
        )));
    }
    // The implementation names both data types as ordinary trait arguments:
    // `OpaqueRepresentation<Opaque, Representation>`. It has no data subject.
    let trait_arguments = typed
        .type_reference_table
        .type_reference_handles(conformance.arguments);
    let [opaque_trait_argument, representation_trait_argument] = trait_arguments else {
        return Err(Diagnostic::error(format!(
            "conformance `{}` does not retain exactly one opaque argument and one representation argument",
            conformance_name,
        )));
    };
    if typed
        .type_reference_table
        .type_symbol(*opaque_trait_argument)
        != opaque
    {
        return Err(Diagnostic::error(format!(
            "conformance `{}` represents `{}`, not selected opaque declaration `{opaque_name}`",
            conformance_name,
            typed.display_type_reference(*opaque_trait_argument),
        )));
    }
    let carrier = typed
        .type_reference_table
        .type_symbol(*representation_trait_argument);
    let Some(carrier_definition) = typed.data_definition_by_symbol(carrier) else {
        return Err(Diagnostic::error(format!(
            "conformance `{}` represents `{opaque_name}` as `{}`, which is not one exact concrete data declaration",
            conformance_name,
            typed.display_type_reference(*representation_trait_argument),
        )));
    };
    if carrier_definition.supply_mode != language_semantics::DataSupplyMode::CheckedShape {
        return Err(Diagnostic::error(format!(
            "opaque representation carrier `{}` must be ordinary checked-shape data",
            carrier_definition.name,
        )));
    }
    let copy_disposition = carrier_closure::validate_inert_carrier(
        typed,
        opaque_definition,
        *representation_trait_argument,
    )?;

    Ok(OpaqueRepresentationSelection::from_validated_application(
        opaque,
        carrier,
        *representation_trait_argument,
        application,
        OpaqueRepresentationLifecycleDisposition::Inert,
        copy_disposition,
        OpaqueRepresentationApplicationOrigin::NamedConformance,
        selecting_machine,
        source_span,
    ))
}
