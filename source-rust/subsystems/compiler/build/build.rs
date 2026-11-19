//! Drive a project build. Each plan describes a whole new compilation.

pub mod output_directory;
pub mod published_terminal;

use compiler::{Artifact, CompileRequest, PublishedPsi, StageSource, compile};
use diagnostics::Diagnostic;

/// The host resolves the inert plan into exact inputs and granted services.
pub enum NextStage<'a, Services, Grants> {
    Run {
        name: String,
        source: StageSource<'a>,
        services: Services,
        grants: Grants,
    },
    Emit(CompileRequest<'a>),
}

/// The interpreter returns plan data. Only the host can issue service handles.
pub trait BuildHost {
    type Plan;
    type Services;
    type Grants;

    /// The service-free core stage selects the first inputs from the invocation.
    fn core_plan(&mut self) -> Result<Self::Plan, Vec<Diagnostic>>;

    /// Bind reflected identities in the next stage's sources, reject prerequisite
    /// cycles, and issue fresh service bindings within the predecessor's grants.
    fn admit(
        &mut self,
        plan: Self::Plan,
        grants: Self::Grants,
    ) -> Result<NextStage<'_, Self::Services, Self::Grants>, Vec<Diagnostic>>;

    fn interpret_plan(
        &mut self,
        program: PublishedPsi,
        services: Self::Services,
    ) -> Result<Self::Plan, Vec<Diagnostic>>;
}

pub fn build<H: BuildHost>(
    host: &mut H,
    grants: H::Grants,
    max_planning_stages: usize,
) -> Result<Artifact, Vec<Diagnostic>> {
    run_stages(host, grants, max_planning_stages, compile)
}

/// Run the requested validators on the final product of this build.
pub fn build_with_validators<H: BuildHost>(
    host: &mut H,
    grants: H::Grants,
    max_planning_stages: usize,
    validators: &compiler::stage_validators::StageValidatorRequest,
) -> Result<
    (
        Artifact,
        Vec<compiler::stage_validators::StageValidationRow>,
    ),
    compiler::stage_validators::NativeValidationError,
> {
    run_stages(host, grants, max_planning_stages, |request| {
        compiler::compile_with_validators(request, validators)
    })
}

fn run_stages<H: BuildHost, T, E: From<Vec<Diagnostic>>>(
    host: &mut H,
    mut grants: H::Grants,
    max_planning_stages: usize,
    emit: impl FnOnce(CompileRequest<'_>) -> Result<T, E>,
) -> Result<T, E> {
    let mut plan = host.core_plan()?;
    let mut stages = Vec::new();

    loop {
        match host.admit(plan, grants)? {
            NextStage::Emit(request) => return emit(request),
            NextStage::Run {
                name,
                source,
                services,
                grants: next_grants,
            } => {
                stages.push(name);
                if stages.len() > max_planning_stages {
                    return Err(vec![Diagnostic::error(format!(
                        "build exceeded its limit of {max_planning_stages} planning stages: {}",
                        stages.join(" -> "),
                    ))]
                    .into());
                }
                let Artifact::Psi(program) = compile(CompileRequest::Psi(source))? else {
                    return Err(vec![Diagnostic::error(
                        "build stage requested Psi and received a native artifact",
                    )]
                    .into());
                };
                plan = host.interpret_plan(program, services)?;
                grants = next_grants;
            }
        }
    }
}
