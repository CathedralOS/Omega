use super::rejected;
use crate::capture::semantics::{
    declarations::{nominal_identity, trait_requirement_identity_from_symbols},
    facts::exactly_one,
};
use crate::record::{
    PackagePolicyProgressPremise, PackagePolicyServiceProgressRoute, PackagePolicyTermination,
    PackageReviewProgressSubject,
};
use checked_trees::RealizedMachineContractEnvelope;
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;
use typed_trees::{machine::Machine, state::State};

pub(crate) fn termination(
    compilation: &CheckedCompilation,
    machine: &Machine,
    entry: &State,
    envelope: &RealizedMachineContractEnvelope,
) -> Result<PackagePolicyTermination, Vec<Diagnostic>> {
    let plan = checked_plan(compilation, machine, entry, envelope)?;
    project_guarantee(compilation, machine, entry, &plan.checked_summary)
}

pub(crate) fn declared_termination(
    compilation: &CheckedCompilation,
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
    compilation: &'a CheckedCompilation,
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
    compilation: &CheckedCompilation,
    machine: &Machine,
    entry: &State,
    guarantee: &language_semantics::TerminationGuarantee,
) -> Result<PackagePolicyTermination, Vec<Diagnostic>> {
    let language_semantics::TerminationGuarantee::Terminates { premises } = guarantee else {
        return Ok(PackagePolicyTermination::NoGuarantee);
    };
    let parameters = compilation.state_parameters(entry);
    let mut projected = Vec::new();
    for premise in premises {
        let profile = exactly_one(
            compilation
                .domain_definitions()
                .iter()
                .filter(|domain| domain.semantic_id == premise.profile),
            machine.name.as_str(),
            "progress profile",
        )?;
        if profile.classification != Some(language_semantics::DomainClassification::ProgressProfile)
        {
            return Err(rejected(
                "termination premise does not name an exact progress profile",
            ));
        }
        let root = premise.subject.root;
        let subject = if let Some(parameter) =
            parameters.iter().find(|parameter| parameter.symbol == root)
        {
            if parameter.is_self {
                PackageReviewProgressSubject::Receiver
            } else {
                let position = parameters
                    .iter()
                    .filter(|parameter| !parameter.is_self)
                    .position(|parameter| parameter.symbol == root)
                    .ok_or_else(|| {
                        rejected("termination parameter has no canonical entry position")
                    })?;
                PackageReviewProgressSubject::Parameter(
                    u32::try_from(position)
                        .map_err(|_| rejected("termination position exceeds portable width"))?,
                )
            }
        } else if compilation
            .data_definitions()
            .iter()
            .any(|data| data.symbol == root && data.is_public)
            || compilation
                .domain_definitions()
                .iter()
                .any(|domain| domain.symbol == root && domain.is_public)
            || compilation
                .machines()
                .iter()
                .any(|machine| machine.symbol == root && machine.is_public)
        {
            PackageReviewProgressSubject::Declaration(nominal_identity(compilation, root)?)
        } else {
            return Err(rejected(
                "termination premise root is neither an entry parameter nor a public declaration",
            ));
        };
        let mut establishment_routes = profile.establishment_routes.iter().map(|route| {
            Ok(PackagePolicyServiceProgressRoute {
                kind: match route {
                    language_semantics::DomainEstablishmentRoute::CheckedRequirement {..} => effects::provider_plan::ServiceProgressEstablishmentRouteKind::CheckedRequirement,
                    language_semantics::DomainEstablishmentRoute::BoundaryRequirement {..} => effects::provider_plan::ServiceProgressEstablishmentRouteKind::BoundaryRequirement,
                },
                requirement_owner: nominal_identity(compilation, route.source_symbol())?,
                requirement: trait_requirement_identity_from_symbols(compilation, route.source_symbol(), route.requirement_symbol(), "callable progress establishment")?,
            })
        }).collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        establishment_routes.sort();
        establishment_routes.dedup();
        projected.push(PackagePolicyProgressPremise {
            profile: nominal_identity(compilation, profile.symbol)?,
            subject,
            projections: premise
                .subject
                .projections
                .iter()
                .map(|symbol| nominal_identity(compilation, *symbol))
                .collect::<Result<Vec<_>, _>>()?,
            establishment_routes,
        });
    }
    projected.sort();
    projected.dedup();
    Ok(PackagePolicyTermination::Terminates {
        premises: projected,
    })
}
