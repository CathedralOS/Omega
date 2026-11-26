//! A preparing Build has its own checked program, source address and host observations.
use super::declarations::unavailable;
use build_declarations::BuildEntryPath;
use build_evaluation_result::ComputedBuildConfig;
use diagnostics::Diagnostic;
use package_compilation::BuildObservationSummary;
use semantic_vocabulary::{ServiceId, TargetMachineCallable};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewedPreparation {
    pub file: String,
    pub callable: String,
    pub successor: BuildEntryPath,
    pub observation: BuildObservationSummary,
    pub concrete_reach: Vec<String>,
    pub installation_reach: Vec<(String, Vec<String>)>,
}

impl ReviewedPreparation {
    pub(super) fn from_stage(
        successor: &BuildEntryPath,
        stage: &ComputedBuildConfig,
    ) -> Result<Self, Vec<Diagnostic>> {
        let selected = stage
            .executed_entry
            .as_ref()
            .ok_or_else(|| unavailable("the executed preparation declaration"))?;
        let code = stage
            .accepted_builds
            .last()
            .ok_or_else(|| unavailable("the preparation's checked program"))?;
        let module = code.view().module();
        let Some(TargetMachineCallable::Machine(entry)) = module.entry else {
            return Err(unavailable("the preparation's checked entry"));
        };
        let callable = module
            .machine(entry)
            .and_then(|machine| machine.contract.declaration.as_ref())
            .and_then(|declaration| module.identities.text(declaration.callable))
            .filter(|&name| name == selected.callable_identity)
            .ok_or_else(|| unavailable("the executed preparation's exact callable"))?;
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
            callable: callable.to_owned(),
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
