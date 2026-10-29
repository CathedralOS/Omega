//! Admit reached native Build intent against its exact Product requirements.
//! These rows describe source choices; native ABI, provider evidence and foreign
//! behavior admission remain Omega judgments.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    BuildOperation, ExecutedNativeBinding, ProductDeclarationRequest, ProductOperandRole,
    ProductTarget,
};
use semantic_vocabulary::{NativeBindingLocator, PackageKeyIdentity};
use symbols::SymbolHandle;
use typed_trees::{
    TypedTrees,
    product_operand::{resolve_build_request_author, resolve_product_path},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBindingSelection {
    pub subject: SymbolHandle,
    pub requirement: SymbolHandle,
    pub target: ProductTarget,
    pub locator: NativeBindingLocator,
    pub producer: language_semantics::declaration_selection::NativeBindingProducer,
    pub source_occurrence_digest: [u8; 32],
    pub supplier_package: PackageKeyIdentity,
    pub source_span: source::SourceSpan,
}

fn requirement(
    typed: &TypedTrees,
    request: &ProductDeclarationRequest,
) -> Result<(SymbolHandle, SymbolHandle), Diagnostic> {
    request.validate_shape().map_err(Diagnostic::error)?;
    if request.operation != BuildOperation::NativeBinding {
        return Err(Diagnostic::error(
            "native binding has the wrong designated operation",
        ));
    }
    let [operand] = request.operands.as_ref() else {
        return Err(Diagnostic::error(
            "native binding lost its exact requirement operand",
        ));
    };
    let symbol = resolve_product_path(typed, &operand.path, operand.source_span, ProductOperandRole::Requirement)
        .ok_or_else(|| Diagnostic::error(format!("native binding operand `{}` does not resolve to one visible Product boundary requirement", operand.path)).with_source_span(operand.source_span))?;
    requirement_owner(typed, symbol).map_err(Diagnostic::error)
}

pub fn collect(
    typed: &TypedTrees,
    activation: Option<SymbolHandle>,
    declared: &[std::sync::Arc<ProductDeclarationRequest>],
    executed: &[ExecutedNativeBinding],
) -> Result<Vec<NativeBindingSelection>, Vec<Diagnostic>> {
    for request in declared
        .iter()
        .filter(|request| request.operation == BuildOperation::NativeBinding)
    {
        requirement(typed, request)
            .map_err(|error| vec![error.with_source_span(request.source_span)])?;
    }
    if executed.is_empty() {
        return Ok(Vec::new());
    }
    let root = activation
        .filter(|symbol| typed.machine(*symbol).is_some())
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "native binding has no authoritative completed Build activation",
            )]
        })?;
    let _ = root; // The runtime issued each event against this activation's original Build.
    let mut selections: Vec<NativeBindingSelection> = Vec::new();
    for event in executed {
        let span = event.request.source_span;
        let refuse = |reason| vec![Diagnostic::error(reason).with_source_span(span)];
        if !declared.contains(&event.request) {
            return Err(refuse(
                "executed native binding has no retained source formation request",
            ));
        }
        let (subject, selected) = requirement(typed, &event.request)
            .map_err(|error| vec![error.with_source_span(span)])?;
        let author = resolve_build_request_author(typed, event.selecting_machine)
            .map_err(|reason| vec![Diagnostic::error(reason).with_source_span(span)])?;
        let supplier_package = typed
            .symbols
            .symbol_package_identity(author)
            .ok_or_else(|| refuse("native binding supplier has no immutable package identity"))?;
        if selections.iter().any(|prior| {
            prior.requirement == selected
                && prior.target == event.target
                && prior.supplier_package == supplier_package
                && prior.locator != event.locator
        }) {
            return Err(refuse(
                "conflicting native bindings for the same requirement, target and supplier precedence",
            ));
        }
        // Identical reached effects retain all actual source origins. Native
        // derivation may share their one row without erasing execution provenance.
        let scope = typed_trees::product_operand::native_binding_formation(
            typed,
            author,
            Some(root),
            event.request.clone(),
        )
        .and_then(|formation| formation.seal(supplier_package))
        .map_err(|reason| vec![Diagnostic::error(reason).with_source_span(span)])?;
        selections.push(NativeBindingSelection {
            subject,
            requirement: selected,
            target: event.target,
            locator: event.locator.clone(),
            producer: scope.producer,
            source_occurrence_digest: scope.source_occurrence_digest,
            supplier_package,
            source_span: typed.symbols.rejoin_product_occurrence(span),
        });
    }
    Ok(selections)
}

mod capture;
pub use capture::capture_package_intents;

/// Admit every captured formation before using reached effects. The package
/// output owner has already authenticated source/profile/closure custody; this
/// query supplies only exact Product declaration and visibility judgments.
pub fn collect_captured(
    typed: &TypedTrees,
    declarations: &[language_semantics::declaration_selection::CapturedNativeDeclaration],
    intents: &[language_semantics::declaration_selection::CapturedNativeIntent],
) -> Result<Vec<NativeBindingSelection>, Vec<Diagnostic>> {
    let mut resolved = Vec::with_capacity(declarations.len());
    for declaration in declarations {
        let selected = typed_trees::product_operand::resolve_captured_product_path(typed, declaration, ProductOperandRole::Requirement)
            .ok_or_else(|| vec![Diagnostic::error(format!("captured native requirement `{}` has no unique visible Product declaration in its supplying package", declaration.requirement_path))])?;
        resolved.push(
            requirement_owner(typed, selected).map_err(|reason| vec![Diagnostic::error(reason)])?,
        );
    }
    intents
        .iter()
        .map(|intent| {
            let declaration = declarations
                .get(intent.declaration as usize)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "captured native effect lost its declared source occurrence",
                    )]
                })?;
            let (subject, requirement) = resolved[intent.declaration as usize];
            let span = typed
                .symbols
                .symbol_provenance_source_span(requirement)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "captured native requirement lost its checked Product source",
                    )]
                })?;
            Ok(NativeBindingSelection {
                subject,
                requirement,
                target: intent.target,
                locator: intent.locator.clone(),
                producer: declaration.producer.clone(),
                supplier_package: declaration.producer.package,
                source_occurrence_digest: declaration.source_occurrence_digest,
                source_span: span,
            })
        })
        .collect()
}

fn requirement_owner(
    typed: &TypedTrees,
    symbol: SymbolHandle,
) -> Result<(SymbolHandle, SymbolHandle), String> {
    let owner = typed
        .traits()
        .iter()
        .find(|owner| {
            owner.is_boundary
                && typed
                    .trait_machine_signatures(owner)
                    .iter()
                    .any(|signature| signature.symbol == symbol)
        })
        .ok_or("native binding requirement lost its exact boundary declaration")?;
    if !typed.trait_type_parameters(owner).is_empty() || !owner.lifetime_parameters.is_empty() {
        return Err("native binding requires a closed static boundary application".into());
    }
    Ok((owner.symbol, symbol))
}
