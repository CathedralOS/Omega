//! Only the portable publication issuer can authorize provider selection for a checked Product.
pub type SelectedExecution = native_compiler::PublishedSelection<
    portable_publication::PortableDemandIssuer,
    native_compiler::SelectedExecution,
>;

/// A raw native calculation cannot substitute a different publisher's facts.
///
/// ```compile_fail
/// fn install_other_issuers_roots(facts: backend_plan::ProviderDemandInputs, authorization: terminal_installation::AuthorizedTerminalArtifact) -> products::SelectedProviderResult {
///     let alternate = backend_plan::IssuedProviderDemand::issue((), facts);
///     let admitted = backend_plan::AdmittedProviderDemand::new(alternate, authorization).unwrap();
///     native_compiler::select_published_providers(admitted).unwrap().into_result()
/// }
/// ```
pub type SelectedProviderResult = native_compiler::PublishedSelection<
    portable_publication::PortableDemandIssuer,
    native_compiler::SelectedProviderResult,
>;
