//! Only the portable publication issuer can authorize provider selection for a checked Product.
pub type SelectedExecution = native_compiler::PublishedSelection<
    portable_publication::PortableDemandIssuer,
    native_compiler::SelectedExecution,
>;

/// A raw native calculation cannot substitute a different publisher's facts.
///
/// ```compile_fail
/// fn install_other_issuers_roots(facts: backend_plan::ProviderDemandInputs) -> products::SelectedProviderResult {
///     let alternate = backend_plan::IssuedProviderDemand::issue((), facts);
///     native_compiler::select_published_providers(alternate).unwrap().into_result()
/// }
/// ```
pub type SelectedProviderResult = native_compiler::PublishedSelection<
    portable_publication::PortableDemandIssuer,
    native_compiler::SelectedProviderResult,
>;
