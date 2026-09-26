//! Resolving selection requirements and checking refinements.

use crate::machine_execution::reflection::schema_graph::exact_symbol_identity;
use crate::machine_execution::reflection::selection::scoped_members::ScopedMember;
use crate::machine_execution::reflection::selection::{
    DemandedTraitApplication, SelectionChoice, SelectionRequirement, first_occurrence_normalized,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::machine::Machine;
use typed_trees::trait_definition::{Conformance, ConformanceImplementation, TraitDefinition};
use typed_trees::types::TypeReferenceHandle;

/// The requirement claim resolved against the checking program: the exact
/// trait definition and the pinned requirement signature when present.
/// Checks below compare handles directly once resolution has happened.
pub(crate) struct ResolvedRequirement<'program> {
    definition: &'program TraitDefinition,
    requirement_symbol: Option<SymbolHandle>,
    /// The demanded trait application the selection pins — a conformance or
    /// satisfies edge at a different application does not refine it.
    trait_application: Option<DemandedTraitApplication>,
}

pub(crate) fn resolve_requirement<'program>(
    typed: &'program TypedTrees,
    requirement: &SelectionRequirement,
) -> Result<ResolvedRequirement<'program>, String> {
    let mut matches = typed.traits().iter().filter(|definition| {
        exact_symbol_identity(typed, definition.symbol)
            .is_ok_and(|(identity, _)| identity == requirement.trait_identity)
    });
    let Some(definition) = matches.next() else {
        return Err(format!(
            "selection requirement `{}` does not resolve to a trait declaration in this program",
            requirement.trait_identity
        ));
    };
    if matches.next().is_some() {
        return Err(format!(
            "selection requirement `{}` resolves to more than one trait declaration",
            requirement.trait_identity
        ));
    }
    let requirement_symbol = match &requirement.requirement_identity {
        Some(identity) => Some(
            typed
                .trait_machine_signatures(definition)
                .iter()
                .find(|signature| {
                    exact_symbol_identity(typed, signature.symbol)
                        .is_ok_and(|(candidate, _)| candidate == *identity)
                })
                .map(|signature| signature.symbol)
                .ok_or_else(|| {
                    format!(
                        "selection requirement `{identity}` is not a requirement of trait `{}`",
                        requirement.trait_identity
                    )
                })?,
        ),
        None => None,
    };
    Ok(ResolvedRequirement {
        definition,
        requirement_symbol,
        trait_application: requirement.trait_application.clone(),
    })
}

fn resolve_conformance<'program>(
    typed: &'program TypedTrees,
    identity: &str,
) -> Result<&'program Conformance, String> {
    let mut matches = typed.conformances().iter().filter(|conformance| {
        exact_symbol_identity(typed, conformance.symbol)
            .is_ok_and(|(candidate, _)| candidate == identity)
    });
    match (matches.next(), matches.next()) {
        (Some(conformance), None) => Ok(conformance),
        (Some(_), Some(_)) => Err(format!(
            "selection conformance `{identity}` resolves to more than one declaration"
        )),
        _ => Err(format!(
            "selection conformance `{identity}` does not resolve to a conformance in this program"
        )),
    }
}

fn resolve_machine<'program>(
    typed: &'program TypedTrees,
    identity: &str,
) -> Result<&'program Machine, String> {
    let mut matches = typed.machines().iter().filter(|machine| {
        exact_symbol_identity(typed, machine.symbol)
            .is_ok_and(|(candidate, _)| candidate == identity)
    });
    match (matches.next(), matches.next()) {
        (Some(machine), None) => Ok(machine),
        (Some(_), Some(_)) => Err(format!(
            "selection machine `{identity}` resolves to more than one declaration"
        )),
        _ => Err(format!(
            "selection machine `{identity}` does not resolve to a machine in this program"
        )),
    }
}

/// The carrier a selected operation covers must be the member type's head
/// nominal — the declaration for a named member, the element type for an
/// array member, the referee for a reference member, the enclosing sum for a
/// case member. A member type with no nominal subject admits no operation.
fn check_carrier_covers_member(
    typed: &TypedTrees,
    carrier: SymbolHandle,
    member: &ScopedMember,
) -> Result<(), String> {
    let Some(head) = &member.head_nominal_identity else {
        return Err(format!(
            "selection for `{}` requires an operation on the member's declared type, which has no nominal subject",
            member.member_identity
        ));
    };
    if !carrier.is_valid() {
        return Err(format!(
            "selection for `{}` names an operation with no carrier",
            member.member_identity
        ));
    }
    let carrier_identity = exact_symbol_identity(typed, carrier)?.0;
    if carrier_identity != *head {
        return Err(format!(
            "selection for `{}` names an operation on `{carrier_identity}` which does not cover the member's declared type",
            member.member_identity
        ));
    }
    Ok(())
}

/// Whether the application a conformance or satisfies edge actually carries
/// meets a demanded trait application: every type argument compares by
/// package-qualified identity and lifetime arguments compare under
/// first-occurrence normalization, so binder renaming on either side cannot
/// drift the demand.
fn application_satisfied(
    typed: &TypedTrees,
    demanded: &DemandedTraitApplication,
    type_arguments: &[TypeReferenceHandle],
    lifetime_arguments: &[u32],
) -> bool {
    let carried: Vec<String> = type_arguments
        .iter()
        .map(|argument| {
            typed
                .package_qualified_type_identity(*argument)
                .into_string()
        })
        .collect();
    carried == demanded.type_argument_identities
        && first_occurrence_normalized(lifetime_arguments) == demanded.lifetime_arguments
}

/// The application one conformance or satisfies edge carries on its trait.
fn carried_application_matches(
    typed: &TypedTrees,
    demanded: &Option<DemandedTraitApplication>,
    arguments: arena::HandleSpan<TypeReferenceHandle>,
    lifetime_arguments: &[u32],
) -> bool {
    match demanded {
        None => true,
        Some(demanded) => application_satisfied(
            typed,
            demanded,
            typed.type_reference_table.type_reference_handles(arguments),
            lifetime_arguments,
        ),
    }
}

/// Whether `machine` binds a requirement of the resolved trait through the
/// attached-form name rule: `Carrier::requirement_name` attached to the
/// conformance's carrier. This is the language's own binding for
/// `AttachedRequirementMachines` conformances; an authored `satisfies` edge
/// on the machine is checked separately by the machine path. When a trait
/// application is demanded the conformance in scope carries it — the
/// machine binds under that conformance's application.
fn machine_binds_attached_requirement(
    typed: &TypedTrees,
    machine: &Machine,
    conformance: &Conformance,
    requirement: &ResolvedRequirement,
) -> bool {
    if machine.attached_data_symbol != conformance.carrier_symbol {
        return false;
    }
    let carrier_name = typed.symbols.name(conformance.carrier_symbol);
    typed
        .trait_machine_signatures(requirement.definition)
        .iter()
        .any(|signature| {
            requirement
                .requirement_symbol
                .is_none_or(|required| signature.symbol == required)
                && machine.name.as_str() == format!("{carrier_name}::{}", signature.name.as_str())
        })
}

pub(crate) fn check_conformance_refines(
    typed: &TypedTrees,
    conformance_identity: &str,
    requirement: &ResolvedRequirement,
    member: &ScopedMember,
) -> Result<(), String> {
    let conformance = resolve_conformance(typed, conformance_identity)?;
    if conformance.trait_symbol != requirement.definition.symbol {
        return Err(format!(
            "selection for `{}` names conformance `{conformance_identity}` of a different trait; insufficient contract",
            member.member_identity
        ));
    }
    if !carried_application_matches(
        typed,
        &requirement.trait_application,
        conformance.arguments,
        &conformance.trait_lifetime_arguments,
    ) {
        return Err(format!(
            "selection for `{}` names conformance `{conformance_identity}` at a different trait application than demanded",
            member.member_identity
        ));
    }
    check_carrier_covers_member(typed, conformance.carrier_symbol, member)?;
    if let Some(requirement_symbol) = requirement.requirement_symbol {
        let realized = match &conformance.implementation {
            ConformanceImplementation::Closed { rows } => rows.iter().any(|row| {
                row.requirement == requirement_symbol && row.realization_machine.is_valid()
            }),
            ConformanceImplementation::AttachedRequirementMachines => {
                let requirement_name = typed
                    .trait_machine_signatures(requirement.definition)
                    .iter()
                    .find(|signature| signature.symbol == requirement_symbol)
                    .map(|signature| signature.name.as_str().to_owned());
                match requirement_name {
                    Some(requirement_name) => {
                        let carrier_name = typed.symbols.name(conformance.carrier_symbol);
                        let attached = format!("{carrier_name}::{requirement_name}");
                        typed.machines().iter().any(|machine| {
                            machine.attached_data_symbol == conformance.carrier_symbol
                                && (machine.name.as_str() == attached
                                    || typed.machine_trait_conformances(machine).iter().any(
                                        |edge| {
                                            edge.symbol == conformance.trait_symbol
                                                && edge.requirement_symbol == requirement_symbol
                                        },
                                    ))
                        })
                    }
                    None => false,
                }
            }
        };
        if !realized {
            return Err(format!(
                "selection for `{}` names conformance `{conformance_identity}` which does not realize the pinned requirement",
                member.member_identity
            ));
        }
    }
    Ok(())
}

pub(crate) fn check_machine_refines(
    typed: &TypedTrees,
    machine_identity: &str,
    requirement: &ResolvedRequirement,
    member: &ScopedMember,
) -> Result<(), String> {
    let machine = resolve_machine(typed, machine_identity)?;
    // Path A: an authored `satisfies` edge on the machine itself names the
    // trait and optionally the exact requirement row; a demanded application
    // must be the one the edge carries.
    if typed
        .machine_trait_conformances(machine)
        .iter()
        .any(|edge| {
            edge.symbol == requirement.definition.symbol
                && requirement
                    .requirement_symbol
                    .is_none_or(|required| edge.requirement_symbol == required)
                && carried_application_matches(
                    typed,
                    &requirement.trait_application,
                    edge.arguments,
                    &edge.trait_lifetime_arguments,
                )
        })
    {
        return check_carrier_covers_member(typed, machine.attached_data_symbol, member);
    }
    // Path B: the machine is a realization of the required trait on the
    // member's carrier — a closed conformance row, or the attached-form
    // `Carrier::requirement` name binding — at the demanded application when
    // one is pinned.
    for conformance in typed
        .conformances()
        .iter()
        .filter(|conformance| conformance.trait_symbol == requirement.definition.symbol)
        .filter(|conformance| {
            carried_application_matches(
                typed,
                &requirement.trait_application,
                conformance.arguments,
                &conformance.trait_lifetime_arguments,
            )
        })
    {
        let realizes = match &conformance.implementation {
            ConformanceImplementation::Closed { rows } => rows.iter().any(|row| {
                row.realization_machine == machine.symbol
                    && requirement
                        .requirement_symbol
                        .is_none_or(|required| row.requirement == required)
            }),
            ConformanceImplementation::AttachedRequirementMachines => {
                machine_binds_attached_requirement(typed, machine, conformance, requirement)
            }
        };
        if realizes {
            return check_carrier_covers_member(typed, conformance.carrier_symbol, member);
        }
    }
    Err(format!(
        "selection for `{}` names machine `{machine_identity}` which does not realize the required contract",
        member.member_identity
    ))
}

/// Eager producer-side check that a choice refines the required contract for
/// `member`. Replay runs the same checks independently; a rejection here is
/// the policy's error at the point of selection.
pub(crate) fn check_choice_refines(
    typed: &TypedTrees,
    requirement: &SelectionRequirement,
    member: &ScopedMember,
    choice: &SelectionChoice,
) -> Result<(), String> {
    let requirement = resolve_requirement(typed, requirement)?;
    match choice {
        SelectionChoice::Conformance {
            conformance_identity,
        } => check_conformance_refines(typed, conformance_identity, &requirement, member),
        SelectionChoice::Machine { machine_identity } => {
            check_machine_refines(typed, machine_identity, &requirement, member)
        }
        SelectionChoice::Excluded => Ok(()),
    }
}
