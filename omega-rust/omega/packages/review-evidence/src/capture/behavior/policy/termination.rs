use super::rejected;
use crate::capture::PackageReviewInput;
use crate::capture::semantics::{
    declarations::{
        nominal_identity, premise_projections, premise_subject, progress_profile,
        project_service_progress_routes,
    },
    facts::exactly_one,
};
use crate::record::{PackagePolicyProgressPremise, PackagePolicyTermination};
use checked_trees::RealizedMachineContractEnvelope;
use diagnostics::Diagnostic;
use typed_trees::{machine::Machine, state::State};

pub(crate) fn termination(
    compilation: &PackageReviewInput<'_>,
    machine: &Machine,
    entry: &State,
    envelope: &RealizedMachineContractEnvelope,
) -> Result<PackagePolicyTermination, Vec<Diagnostic>> {
    let plan = checked_plan(compilation, machine, entry, envelope)?;
    project_guarantee(compilation, machine, entry, &plan.checked_summary)
}

pub(crate) fn declared_termination(
    compilation: &PackageReviewInput<'_>,
    machine: &Machine,
    entry: &State,
    envelope: &RealizedMachineContractEnvelope,
) -> Result<Option<PackagePolicyTermination>, Vec<Diagnostic>> {
    let plan = checked_plan(compilation, machine, entry, envelope)?;
    plan.interface
        .published()
        .map(|guarantee| project_guarantee(compilation, machine, entry, guarantee))
        .transpose()
}

fn checked_plan<'a>(
    compilation: &'a PackageReviewInput<'a>,
    machine: &Machine,
    entry: &State,
    envelope: &RealizedMachineContractEnvelope,
) -> Result<&'a language_semantics::MachineTerminationPlan, Vec<Diagnostic>> {
    let fact = exactly_one(
        compilation
            .facts
            .termination
            .machines
            .iter()
            .filter(|fact| fact.machine == machine.symbol),
        machine.name.as_str(),
        "termination",
    )?;
    if envelope.machine != machine.symbol
        || fact.plan.checked_summary != envelope.checked_termination
        // Checked lowering preserves the authored interface, but replaces the
        // typed plan's summary with whole-graph progress analysis and resolves
        // the private witness view. Those implementation fields cannot be
        // compared with their pre-check values. The exact checked envelope
        // above is the owner of the retained implementation summary.
        || machine.termination_plan.interface != fact.plan.interface
        || compilation
            .machine_states(machine)
            .first()
            .map(|state| state.symbol)
            != Some(entry.symbol)
    {
        return Err(rejected(
            "termination plan differs from its exact typed declaration, entry, or checked envelope",
        ));
    }
    Ok(&fact.plan)
}

fn project_guarantee(
    compilation: &PackageReviewInput<'_>,
    machine: &Machine,
    entry: &State,
    guarantee: &language_semantics::TerminationGuarantee,
) -> Result<PackagePolicyTermination, Vec<Diagnostic>> {
    let language_semantics::TerminationGuarantee::Terminates { premises } = guarantee else {
        return Ok(PackagePolicyTermination::NoGuarantee);
    };
    let parameters = compilation.state_parameters(entry);
    let subject = format!("reviewed callable `{}`", machine.name.as_str());
    let mut projected = Vec::new();
    for premise in premises {
        let profile = progress_profile(compilation, premise.profile, &subject)?;
        let establishment_routes = project_service_progress_routes(
            compilation,
            profile,
            "callable progress establishment",
        )?;
        projected.push(PackagePolicyProgressPremise {
            profile: nominal_identity(compilation, profile.symbol)?,
            subject: premise_subject(compilation, parameters, premise.subject.root, &subject)?,
            projections: premise_projections(compilation, premise)?,
            establishment_routes,
        });
    }
    projected.sort();
    projected.dedup();
    Ok(PackagePolicyTermination::Terminates {
        premises: projected,
    })
}
