//! Explicit inspection of the native inputs published for these requirements.
use diagnostics::Diagnostic;
use semantic_vocabulary::BoundaryMachineId;
use source_compiler::SourcePublication;

pub fn prepare_provider_execution_inputs(
    publication: &SourcePublication,
    sources: &package_compilation::CheckedSourceCustody,
    native_supplies: &[provider_choices::NativeBuildSupply],
    profile: target::TargetProfile,
    reached: &[BoundaryMachineId],
    observer: &mut impl source_compiler::PsiCompilationObserver,
) -> Result<native_compiler::TargetSupplyExecutionInputs, Vec<Diagnostic>> {
    let origins = portable_publication::ProductCodeSource {
        publication,
        sources,
    }
    .publish_provider_origins(native_supplies, profile, reached, observer)?;
    native_compiler::plan_provider_execution_inputs(origins, profile, reached)
}
