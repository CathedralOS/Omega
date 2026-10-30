//! Seal captured checked-Build source under its completed package output.
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    CapturedNativeDeclaration, CapturedNativeIntent, ExecutedNativeBinding, NativeBindingFormation,
};
use semantic_vocabulary::PackageKeyIdentity;
pub fn capture_package_intents(
    package: PackageKeyIdentity,
    formations: &[NativeBindingFormation],
    executed: &[ExecutedNativeBinding],
) -> Result<(Vec<CapturedNativeDeclaration>, Vec<CapturedNativeIntent>), Vec<Diagnostic>> {
    let declarations = formations
        .iter()
        .map(|formation| {
            formation
                .seal(package)
                .map_err(|reason| vec![Diagnostic::error(reason)])
        })
        .collect::<Result<Vec<_>, _>>()?;
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
                intent: event.intent.clone(),
                static_environment: event.static_environment.clone(),
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    Ok((declarations, intents))
}
