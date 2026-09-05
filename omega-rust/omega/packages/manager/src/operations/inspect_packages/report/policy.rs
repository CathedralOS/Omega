//! Summary of typed findings. Evidence alone owns canonical policy syntax.

use super::{CanonicalSourceClosureSubject, Name, Output};
use package_evidence::record::*;
use std::fmt::{self, Write};

mod callables;

pub(super) fn render(
    output: &mut Output,
    label: &str,
    policy: &PackagePolicyBaseline,
    source: &CanonicalSourceClosureSubject,
    verbose: bool,
) -> fmt::Result {
    writeln!(output, "{label}")?;
    let names = Names(source);
    writeln!(
        output,
        "  public declarations: data {} traits {} conformances {} domains {} propositions {} constants {} operators {}; full shapes: --details",
        policy.public_data().len(),
        policy.public_traits().len(),
        policy.public_conformances().len(),
        policy.public_domains().len(),
        policy.public_propositions().len(),
        policy.public_consts().len(),
        policy.public_operators().len()
    )?;
    for constant in policy.public_consts() {
        writeln!(output, "  constant {}", names.name(constant.identity()))?;
    }
    writeln!(
        output,
        "  callables {} (roles and supplies are compiler classifications)",
        policy.callables().callables().len()
    )?;
    for callable in policy.callables().callables() {
        names.callable(output, callable)?;
    }
    for interface in policy.public_traits() {
        writeln!(
            output,
            "  trait {} boundary {}",
            names.name(interface.identity()),
            interface.is_boundary()
        )?;
        for requirement in interface.requirements() {
            writeln!(
                output,
                "    requirement {} checked-reach unknown (requirement); default-realization {}",
                names.name(requirement.identity()),
                requirement.has_default_realization()
            )?;
            names.reach(
                output,
                "declared-reach",
                Some(requirement.service_reach()),
                "not declared",
            )?;
            names.invocations(
                output,
                "declared-invocations",
                Some(requirement.synchronous_invocations()),
                "not declared",
            )?;
            writeln!(
                output,
                "    installation-bound {} suspend {} block {}",
                requirement.service_reach_is_installation_bound(),
                requirement.suspends(),
                requirement.blocks()
            )?;
            contracts(output, requirement.contracts())?;
        }
    }
    for operator in policy.public_operators() {
        writeln!(
            output,
            "  operator {} boundary {}; calling and authority application details: --details",
            names.name(operator.coordinate().identity()),
            operator.is_boundary()
        )?;
        contracts(output, operator.contracts())?;
    }
    writeln!(
        output,
        "  dangerous-authority {} (recorded compiler classifications; not a claim of exercised authority)",
        policy.dangerous_capabilities().len()
    )?;
    for danger in policy.dangerous_capabilities() {
        writeln!(
            output,
            "    {:?} {}",
            danger.class(),
            names.name(danger.service())
        )?;
    }
    writeln!(
        output,
        "  dangerous-authority-slack {}",
        policy.slack_uses().len()
    )?;
    for slack in policy.slack_uses() {
        writeln!(
            output,
            "    {:?} {} declared but absent from checked reach of {}",
            slack.class(),
            names.name(slack.service()),
            names.name(slack.callable())
        )?;
    }
    writeln!(
        output,
        "  external-supplies {} (opaque executable assumptions)",
        policy.external_supplies().len()
    )?;
    for supply in policy.external_supplies() {
        writeln!(output, "    external {}", names.name(supply.callable()))?;
        // The typed binding retains exact destinations; Debug escapes its strings.
        writeln!(output, "    binding {:?}", supply.binding())?;
    }
    names.providers(output, policy.selected_providers())?;
    writeln!(
        output,
        "  terminal-permission-services {}",
        policy.terminal_permissions().services().len()
    )?;
    for service in policy.terminal_permissions().services() {
        writeln!(output, "    service {}", names.name(service.service()))?;
        for permission in service.permissions() {
            writeln!(
                output,
                "    permission {} {:?}",
                names.name(permission.requirement()),
                permission.permitted()
            )?;
        }
        for method in service.methods() {
            names.method(output, method)?;
        }
    }
    names.representation(output, policy.representation())?;
    writeln!(
        output,
        "  semantic-dependencies {}; complete owner/type relationships: --details",
        policy.semantic_dependencies().len()
    )?;
    for demand in policy.boundary_applications().demands() {
        writeln!(
            output,
            "  boundary-demand {} producer {}; arguments: --details",
            names.name(demand.operator_coordinate().identity()),
            names.name(demand.producer_callable())
        )?;
    }
    for realization in policy.boundary_applications().realizations() {
        writeln!(
            output,
            "  boundary-realization {} provider-plan {} {:?}",
            names.name(realization.operator_coordinate().identity()),
            realization.selected_plan_index(),
            realization.realization()
        )?;
    }
    if verbose {
        writeln!(output, "  complete canonical policy:")?;
        let text = match policy.canonical_text() {
            Ok(text) => text,
            Err(_) => return output.fail("package inspection compiler policy rendering failed"),
        };
        for line in text.lines() {
            writeln!(output, "  {line}")?;
        }
    }
    Ok(())
}

struct Names<'a>(&'a CanonicalSourceClosureSubject);

impl Names<'_> {
    fn name<'a>(&'a self, identity: &'a PackageReviewNominalIdentity) -> Name<'a> {
        Name {
            source: self.0,
            identity,
        }
    }

    fn providers(
        &self,
        output: &mut Output,
        providers: &PackagePolicySelectedProviders,
    ) -> fmt::Result {
        writeln!(
            output,
            "  providers {} families {}",
            providers.plans().len(),
            providers.families().len()
        )?;
        for (index, plan) in providers.plans().iter().enumerate() {
            writeln!(
                output,
                "    provider-plan {index} {:?} schema {} grants {:?}",
                plan.plan_name(),
                self.name(plan.schema_declaration()),
                plan.grants()
            )?;
            if let Some(provider) = plan.provider_type_declaration() {
                writeln!(output, "    provider-type {}", self.name(provider))?;
            } else {
                writeln!(
                    output,
                    "    provider-type {:?} (no declaration)",
                    plan.provider_type()
                )?;
            }
            for row in plan.rows() {
                writeln!(
                    output,
                    "    binding {} -> {} {:?}",
                    self.name(row.requirement()),
                    self.name(row.realization()),
                    row.binding()
                )?;
                if let Some(reach) = row.installation_reach() {
                    self.reach(
                        output,
                        "installation-upper-bound",
                        Some(reach.upper_bound()),
                        "unknown",
                    )?;
                    self.reach(
                        output,
                        "installation-resolved",
                        Some(reach.resolved()),
                        "unknown",
                    )?;
                }
            }
            for method in plan.methods() {
                self.method(output, method)?;
            }
        }
        for family in providers.families() {
            writeln!(
                output,
                "    family {} provider {} authority {:?} coverage {:?}",
                self.name(family.family_identity()),
                self.name(family.provider_type_declaration()),
                family.authority(),
                family.coverage()
            )?;
            for coordinate in family.coordinates() {
                writeln!(
                    output,
                    "    coordinate {} provider-plan {}",
                    self.name(coordinate.operator_declaration()),
                    coordinate.plan_index()
                )?;
            }
        }
        Ok(())
    }

    fn representation(
        &self,
        output: &mut Output,
        representation: &PackagePolicyRepresentation,
    ) -> fmt::Result {
        writeln!(
            output,
            "  opaque-representation declarations {} candidates {} selections {} demands {}",
            representation.declarations().len(),
            representation.producer_availability().len(),
            representation.selected_availability().len(),
            representation.demands().len()
        )?;
        for declaration in representation.declarations() {
            writeln!(output, "    opaque {}", self.name(declaration))?;
        }
        for candidate in representation.producer_availability() {
            writeln!(
                output,
                "    candidate {} carrier {}",
                self.name(candidate.opaque()),
                self.name(candidate.carrier())
            )?;
        }
        for selection in representation.selected_availability() {
            writeln!(
                output,
                "    selected {} carrier {} origin {:?} lifecycle {:?} copy {:?}",
                self.name(selection.opaque()),
                self.name(selection.carrier()),
                selection.origin(),
                selection.lifecycle(),
                selection.copy_disposition()
            )?;
        }
        for demand in representation.demands() {
            writeln!(
                output,
                "    demand {} requirement {}; physical calling details: --details",
                self.name(demand.opaque()),
                self.name(demand.calling().requirement())
            )?;
        }
        Ok(())
    }
}

fn contracts(output: &mut Output, contracts: &[PackageReviewCallableContract]) -> fmt::Result {
    for contract in contracts {
        writeln!(output, "    contract {contract:?}")?;
    }
    Ok(())
}

pub(super) fn observations(
    output: &mut Output,
    projection: &CheckedPackageReviewProjection,
    source: &CanonicalSourceClosureSubject,
) -> fmt::Result {
    let names = Names(source);
    for obligation in projection.contract_entailment_open_obligations() {
        writeln!(
            output,
            "  open-obligation {} reason {:?} goal {:?}",
            names.name(obligation.callable()),
            obligation.reason(),
            obligation.goal()
        )?;
    }
    for discharge in projection.contract_entailment_assumption_discharges() {
        writeln!(output, "  assumption-discharge {discharge:?}")?;
    }
    Ok(())
}
