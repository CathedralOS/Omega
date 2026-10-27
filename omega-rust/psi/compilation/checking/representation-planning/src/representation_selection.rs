//! Form opaque carrier declarations and activate only authenticated Build events.
//! Static source presence is a formation obligation, never proof of execution.

use crate::representation_trait::is_compiler_owned_opaque_representation_trait;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    BuildOperation, ExecutedProductSelection, ProductDeclarationRequest,
};
use representation_selections::{
    OpaqueRepresentationApplicationOrigin, OpaqueRepresentationCopyDisposition,
    OpaqueRepresentationLifecycleDisposition, OpaqueRepresentationSelection,
};
use source::SourceSpan;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::product_operand::{ProductOperandKind, resolve_product_path};
use typed_trees::{TypedTrees, typed_trees::ClosedConformanceApplication};

mod carrier_closure;

/// Closed declaration facts tied to their producing program. Formation grants
/// no active representation choice; only an authenticated execution selects it.
#[derive(Clone)]
pub struct FormedOpaqueRepresentation<'program> {
    _program: &'program TypedTrees,
    opaque: SymbolHandle,
    carrier: SymbolHandle,
    carrier_type: typed_trees::types::TypeReferenceHandle,
    application: ClosedConformanceApplication,
    copy_disposition: OpaqueRepresentationCopyDisposition,
}

impl FormedOpaqueRepresentation<'_> {
    fn activate(self, activation: SymbolHandle, span: SourceSpan) -> OpaqueRepresentationSelection {
        OpaqueRepresentationSelection::from_validated_application(
            self.opaque,
            self.carrier,
            self.carrier_type,
            self.application,
            OpaqueRepresentationLifecycleDisposition::Inert,
            self.copy_disposition,
            OpaqueRepresentationApplicationOrigin::NamedConformance,
            activation,
            span,
        )
    }
}

pub fn form_opaque_representation_request<'p>(
    typed: &'p TypedTrees,
    request: &ProductDeclarationRequest,
) -> Result<FormedOpaqueRepresentation<'p>, Diagnostic> {
    if request.operation != BuildOperation::RepresentationSelection {
        return Err(Diagnostic::error(
            "opaque formation requires its exact representation request",
        ));
    }
    let [opaque_operand, conformance_operand] = request.operands.as_ref() else {
        return Err(Diagnostic::error(
            "opaque representation request lost its two declaration operands",
        ));
    };
    let opaque = resolve_product_path(
        typed,
        &opaque_operand.path,
        opaque_operand.source_span,
        ProductOperandKind::Data,
    )
    .ok_or_else(|| {
        Diagnostic::error(format!(
            "opaque representation subject `{}` does not resolve to one visible data declaration",
            opaque_operand.path
        ))
        .with_source_span(opaque_operand.source_span)
    })?;
    let conformance = resolve_product_path(
        typed,
        &conformance_operand.path,
        conformance_operand.source_span,
        ProductOperandKind::Representation,
    )
    .ok_or_else(|| {
        Diagnostic::error(format!(
            "opaque representation `{}` does not resolve to one visible named conformance",
            conformance_operand.path
        ))
        .with_source_span(conformance_operand.source_span)
    })?;
    let conformance_argument = typed_trees::expression::StaticMachineArgument {
        symbol: conformance,
        path: conformance_operand
            .path
            .split("::")
            .map(typed_trees::name::Identifier::from)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        ..Default::default()
    };
    let opaque_name = &opaque_operand.path;
    let Some(opaque_definition) = typed.data_definition_by_symbol(opaque) else {
        return Err(Diagnostic::error(format!(
            "opaque representation subject `{opaque_name}` does not resolve to an exact data declaration"
        )));
    };
    if opaque_definition.supply_mode != language_semantics::DataSupplyMode::BoundaryOpaque {
        return Err(Diagnostic::error(format!(
            "representation subject `{opaque_name}` is not boundary-opaque data"
        )));
    }

    if typed.symbols.get(conformance_argument.symbol).kind != SymbolKind::Conformance {
        return Err(Diagnostic::error(format!(
            "opaque representation `{}` does not resolve to an exact named conformance",
            conformance_argument.display_name(),
        )));
    }
    let application = validation::conformance_applications::close_conformance_application(
        typed,
        &conformance_argument,
    )?;
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
            conformance_argument.display_name(),
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
            conformance_argument.display_name(),
        )));
    };
    if typed
        .type_reference_table
        .type_symbol(*opaque_trait_argument)
        != opaque
    {
        return Err(Diagnostic::error(format!(
            "conformance `{}` represents `{}`, not selected opaque declaration `{opaque_name}`",
            conformance_argument.display_name(),
            typed.display_type_reference(*opaque_trait_argument),
        )));
    }
    let carrier = typed
        .type_reference_table
        .type_symbol(*representation_trait_argument);
    let Some(carrier_definition) = typed.data_definition_by_symbol(carrier) else {
        return Err(Diagnostic::error(format!(
            "conformance `{}` represents `{opaque_name}` as `{}`, which is not one exact concrete data declaration",
            conformance_argument.display_name(),
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

    Ok(FormedOpaqueRepresentation {
        _program: typed,
        opaque,
        carrier,
        carrier_type: *representation_trait_argument,
        application,
        copy_disposition,
    })
}

pub fn collect_opaque_representation_selections(
    typed: &TypedTrees,
    activation: Option<SymbolHandle>,
    declared: &[std::sync::Arc<ProductDeclarationRequest>],
    executed: &[ExecutedProductSelection],
) -> Result<Vec<OpaqueRepresentationSelection>, Vec<Diagnostic>> {
    let has_requests = declared
        .iter()
        .any(|request| request.operation == BuildOperation::RepresentationSelection)
        || executed
            .iter()
            .any(|event| event.request.operation == BuildOperation::RepresentationSelection);
    let Some(activation) = activation else {
        return if has_requests {
            Err(vec![Diagnostic::error(
                "representation requests have no authoritative Build activation",
            )])
        } else {
            Ok(Vec::new())
        };
    };
    if typed.machine(activation).is_none() {
        return Err(vec![Diagnostic::error(
            "representation requests lost their exact authoritative Build machine",
        )]);
    }
    let mut formed = Vec::new();
    for request in declared
        .iter()
        .filter(|request| request.operation == BuildOperation::RepresentationSelection)
    {
        let declaration = form_opaque_representation_request(typed, request)
            .map_err(|error| vec![error.with_source_span(request.source_span)])?;
        formed.push((request, declaration));
    }
    let mut selections: Vec<OpaqueRepresentationSelection> = Vec::new();
    for event in executed
        .iter()
        .filter(|event| event.request.operation == BuildOperation::RepresentationSelection)
    {
        if event.composition.is_some() {
            return Err(vec![
                Diagnostic::error("representation choice has an unauthorized composition argument")
                    .with_source_span(event.request.source_span),
            ]);
        }
        let Some((_, declaration)) = formed
            .iter()
            .find(|(request, _)| request.as_ref() == event.request.as_ref())
        else {
            return Err(vec![
                Diagnostic::error(
                    "executed representation choice has no retained authored formation request",
                )
                .with_source_span(event.request.source_span),
            ]);
        };
        if selections
            .iter()
            .any(|selection| selection.opaque() == declaration.opaque)
        {
            return Err(vec![
                Diagnostic::error(format!(
                    "build selects opaque representation `{}` more than once",
                    typed.symbols.display_path(declaration.opaque, "::")
                ))
                .with_source_span(event.request.source_span),
            ]);
        }
        selections.push(
            declaration.clone().activate(
                activation,
                typed
                    .symbols
                    .rejoin_product_occurrence(event.request.source_span),
            ),
        );
    }
    Ok(selections)
}

/// Reclose the retained executed roster against the final graph. Neither a
/// root-only AST scan nor an emitted-call roster proves which choices ran.
pub fn rederive_opaque_representation_selections(
    typed: &TypedTrees,
    activation: Option<SymbolHandle>,
    declared: &[std::sync::Arc<ProductDeclarationRequest>],
    executed: &[ExecutedProductSelection],
    retained: &[OpaqueRepresentationSelection],
) -> Result<Vec<OpaqueRepresentationSelection>, Vec<Diagnostic>> {
    if retained.iter().any(|selection| {
        Some(selection.selecting_machine()) != activation
            || selection.selected_application_commitment()
                != selection.rederived_selected_application_commitment()
    }) {
        return Err(vec![Diagnostic::error(
            "opaque representation selection has stale activation or application custody",
        )]);
    }
    let derived = collect_opaque_representation_selections(typed, activation, declared, executed)?;
    if derived != retained {
        return Err(vec![Diagnostic::error(format!(
            "final opaque-representation rederivation disagrees with executed Build custody (retained {} rows, derived {} rows)",
            retained.len(),
            derived.len()
        ))]);
    }
    Ok(derived)
}
