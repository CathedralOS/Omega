//! Seal captured checked-Build source under its completed package output.
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    CapturedNativeDeclaration, CapturedNativeIntent, ExecutedNativeBinding, NativeBindingFormation,
};
use semantic_vocabulary::PackageKeyIdentity;
pub fn capture_package_intents(
    typed: &typed_trees::TypedTrees,
    package: PackageKeyIdentity,
    formations: &[NativeBindingFormation],
    executed: &[ExecutedNativeBinding],
) -> Result<(Vec<CapturedNativeDeclaration>, Vec<CapturedNativeIntent>), Vec<Diagnostic>> {
    let declarations = formations
        .iter()
        .map(|formation| {
            let source = formation
                .seal(package)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            let (_, selected) =
                super::requirement(typed, &formation.request).map_err(|error| vec![error])?;
            let program = typed.product_declarations();
            let (owner, signature) =
                program.trait_machine_signature(selected).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "captured native requirement lost its Product signature",
                    )]
                })?;
            let anchor = symbols::RetainedDeclaration::capture(&program.symbols, selected)
                .and_then(|anchor| {
                    symbols::RetainedDeclaration::from_retained_parts(
                        anchor.source().clone(),
                        anchor.span(),
                        anchor.kind(),
                    )
                })
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            let dependency_alias =
                source
                    .requirement_path
                    .split_once("::")
                    .and_then(|(alias, _)| {
                        program
                            .symbols
                            .source_map()
                            .and_then(|sources| sources.product_dependency_target(package, alias))
                            .map(|_| alias.to_owned())
                    });
            Ok(CapturedNativeDeclaration {
                source,
                requirement: anchor,
                requirement_identity: program
                    .trait_requirement_callable_identity_text(owner, signature),
                dependency_alias,
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    let intents = executed
        .iter()
        .map(|event| {
            let index = formations
                .iter()
                .position(|formation| {
                    formation.request == event.request
                        && formation.author == event.selecting_machine
                })
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "native effect lost its exact captured source formation and author",
                    )]
                })?;
            Ok(CapturedNativeIntent {
                declaration: u32::try_from(index).map_err(|_| {
                    vec![Diagnostic::error(
                        "native formation roster exceeds its coordinate",
                    )]
                })?,
                target: event.target,
                locator: event.locator.clone(),
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    Ok((declarations, intents))
}
