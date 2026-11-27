//! A preparing Build has its own checked program, source address and host observations.
use super::declarations::unavailable;
use build_evaluation_result::{ComputedBuildConfig, ExecutedBuildEntry};
use diagnostics::Diagnostic;
use package_compilation::BuildObservationSummary;
use package_compilation::declarations::BuildEntryPath;
use semantic_vocabulary::{MachineId, ServiceId, TargetMachineCallable};
use terminal_psi::TerminalModule;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewedPreparation {
    pub file: String,
    pub callable: String,
    pub successor: BuildEntryPath,
    pub observation: BuildObservationSummary,
    pub concrete_reach: Vec<String>,
    pub installation_reach: Vec<(String, Vec<String>)>,
}

impl crate::PlannedProduct {
    /// A fresh audit and reuse of a prior audit compare the same executed stages.
    pub fn reviewed_preparations(&self) -> Result<Vec<ReviewedPreparation>, Vec<Diagnostic>> {
        self.evaluated()
            .preparations
            .iter()
            .map(|(next, stage)| ReviewedPreparation::from_stage(next, stage))
            .collect()
    }
}

impl ReviewedPreparation {
    fn from_stage(
        successor: &BuildEntryPath,
        stage: &ComputedBuildConfig,
    ) -> Result<Self, Vec<Diagnostic>> {
        let selected = stage
            .executed_entry
            .as_ref()
            .ok_or_else(|| unavailable("the executed preparation declaration"))?;
        let (module, _) = checked_program(stage, selected)?;
        let services = |ids: &[ServiceId]| {
            ids.iter()
                .map(|&id| {
                    module
                        .service(id)
                        .and_then(|service| module.identities.text(service.identity))
                        .map(str::to_owned)
                        .ok_or_else(|| {
                            unavailable("the preparing Build's checked service identity")
                        })
                })
                .collect::<Result<Vec<_>, _>>()
        };
        let reach = &module.interfaces.root_service_reach;
        let mut installation_reach = Vec::with_capacity(reach.installation_dependencies.len());
        for dependency in &reach.installation_dependencies {
            let requirement = module
                .identities
                .text(dependency.requirement_identity)
                .ok_or_else(|| unavailable("the preparing Build's installation requirement"))?;
            installation_reach.push((requirement.to_owned(), services(&dependency.upper_bound)?));
        }
        Ok(Self {
            file: selected
                .declaration
                .source()
                .logical_filename()
                .map_err(|message| vec![Diagnostic::error(message)])?,
            callable: selected.callable_identity.clone(),
            successor: successor.clone(),
            observation: stage
                .observation_summary
                .clone()
                .ok_or_else(|| unavailable("the preparing Build's observations"))?,
            concrete_reach: services(&reach.concrete)?,
            installation_reach,
        })
    }
}

/// Review the machine the interpreter entered in this stage's published program.
pub(super) fn checked_program<'a>(
    stage: &'a ComputedBuildConfig,
    selected: &ExecutedBuildEntry,
) -> Result<(&'a TerminalModule, MachineId), Vec<Diagnostic>> {
    let code = stage
        .accepted_builds
        .last()
        .ok_or_else(|| unavailable("the selected Build's accepted code"))?;
    let module = code.view().module();
    let Some(TargetMachineCallable::Machine(entry)) = module.entry else {
        return Err(unavailable("the selected Build's machine entry"));
    };
    let declaration = module
        .machine(entry)
        .and_then(|machine| machine.contract.declaration.as_ref())
        .ok_or_else(|| unavailable("the selected Build's public callable contract"))?;
    if module.identities.text(declaration.callable) != Some(selected.callable_identity.as_str()) {
        return Err(unavailable("the exact selected Build/source association"));
    }
    Ok((module, entry))
}
