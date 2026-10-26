//! Closed named implementations retain provider identity independently of bodies.
//!
//! One exact requirement edge has two carriers in the checked program: a whole
//! conformance map owns a row per requirement it realizes, and a machine owns
//! its own `satisfies Trait<::requirement` clause, which realizes the
//! requirement with that machine's ordinary body. Both carriers hold the same
//! selection shape -- an exact requirement joined to a realizing body -- so the
//! catalog walks the edge set once and answers a candidate by its edge, never
//! by the spelling that authored it. The declaration owning the edge supplies
//! provider identity: the map, or the data declaration an inline member is
//! attached to.
//!
//! A carrier with its own regional or static telescope owns no closed plan in
//! this roster: its exact correspondence lives in a retained receiving
//! application instead, and inventing one here would confuse declaration
//! parameters with their actuals.

use super::{CatalogRequirement, CheckedTrees, LoweringError, Machine, unsupported};
use checked_trees::typed_trees::TypedTrees;
use checked_trees::typed_trees::machine::{SatisfiedDeclaration, resolve_satisfied_declaration};
use checked_trees::typed_trees::trait_definition::{Conformance, TraitDefinition};

pub(super) fn provider_identities(
    checked: &CheckedTrees,
    machine: &Machine,
    requirement: CatalogRequirement<'_>,
) -> Result<Vec<(symbols::SymbolHandle, String)>, LoweringError> {
    let CatalogRequirement::Trait {
        definition,
        signature,
    } = requirement
    else {
        return Ok(Vec::new());
    };
    let typed = &checked.typed;
    let application = typed.closed_requirement_application(signature.symbol);
    let template = application.map_or(signature.symbol, |application| application.template);
    let mut identities = Vec::new();
    for edge in exact_requirement_edges(typed, machine, definition, template)? {
        if let Some(application) = application {
            // Checking joined the complete receiving application to this
            // carrier's retained row. Comparing the unapplied declaration with
            // the receiver here would confuse declaration parameters with
            // their actuals.
            let retained = checked
                .facts
                .provider_closed_requirements
                .iter()
                .any(|row| {
                    row.provider == machine.symbol
                        && row.selecting_declaration == edge.selecting_declaration
                        && row.template == template
                        && row.closed_requirement == signature.symbol
                        && row.receiving_application == application.trait_application_identity
                });
            if !retained
                || !typed.requirement_realization_matches(
                    signature.symbol,
                    edge.realization,
                    machine.symbol,
                )
            {
                continue;
            }
        } else if edge.lifetime_parameters > 0 {
            // A regional candidate needs a retained closed application and its
            // checked slot mapping; the nominal template supplies neither.
            continue;
        } else if !edge.reach.reaches(typed, machine)? {
            continue;
        }
        identities.push((edge.provider, edge.provider_identity));
    }
    Ok(identities)
}

/// One exact requirement edge a candidate body may realize.
struct RequirementEdge {
    /// The declaration `Build::select_provider` names.
    provider: symbols::SymbolHandle,
    /// The authored identity of that declaration.
    provider_identity: String,
    /// The declaration checking keyed the retained receiving application by.
    selecting_declaration: symbols::SymbolHandle,
    /// The body the edge names.
    realization: symbols::SymbolHandle,
    /// How the edge reaches a body when the candidate is not `realization`.
    reach: EdgeReach,
    /// The carrier declaration's own lifetime telescope.
    lifetime_parameters: usize,
}

/// How an exact requirement edge reaches the body that realizes it.
enum EdgeReach {
    /// The edge names this body: a machine's own clause, or a conformance row
    /// whose realization is the candidate.
    Named,
    /// A conformance row names a portable family head; one of its members
    /// realizes the row on the selected target.
    FamilyHead(symbols::SymbolHandle),
    /// A conformance row names a template a checked specialization
    /// instantiates.
    Specialization(symbols::SymbolHandle),
}

impl EdgeReach {
    /// Whether `machine` is the body this edge reaches.
    fn reaches(&self, typed: &TypedTrees, machine: &Machine) -> Result<bool, LoweringError> {
        match self {
            Self::Named => Ok(true),
            Self::FamilyHead(head) => Ok(typed.target_machine_families.iter().any(|family| {
                family.canonical == *head
                    && family
                        .members
                        .iter()
                        .any(|member| member.machine == machine.symbol)
            })),
            Self::Specialization(template) => {
                // Specialization owns the ordered template-to-instance state
                // correspondence; the map's entry above must remain its origin.
                let receipts = typed
                    .machine_specializations
                    .iter()
                    .filter(|receipt| {
                        receipt.template == *template && receipt.instance == machine.symbol
                    })
                    .count();
                if receipts == 0 {
                    return Ok(false);
                }
                let states = typed
                    .machine(*template)
                    .map_or(0, |template| typed.machine_states(template).len());
                if receipts != 1 || typed.machine_states(machine).len() != states {
                    return unsupported("named provider lost its exact specialization entry");
                }
                Ok(true)
            }
        }
    }
}

/// The declarations that can own an exact requirement edge for this candidate,
/// in one walk: every whole conformance map, then the candidate's own
/// declaration.
enum EdgeCarrier<'a> {
    /// A conformance map owns one row per exact requirement edge.
    ConformanceMap(&'a Conformance),
    /// A machine owns its own exact clause, realized by its own body.
    Machine {
        declaration: &'a Machine,
        lifetime_parameters: usize,
    },
}

fn edge_carriers<'a>(typed: &'a TypedTrees, machine: &'a Machine) -> Vec<EdgeCarrier<'a>> {
    let mut carriers = typed
        .conformances()
        .iter()
        .filter(|implementation| implementation.type_parameters.is_empty())
        .map(EdgeCarrier::ConformanceMap)
        .collect::<Vec<_>>();
    // A specialization instance is realized through the authored template that
    // carries its edge, and a statically telescoped declaration owns no closed
    // provider plan; neither declaration holds a selection of its own. The
    // telescope includes nominal and enclosing implementation binders, even
    // when the method's value signature does not mention them. Captured
    // specialization operands are not additional declaration binders.
    let (type_parameters, lifetime_parameters) =
        typed.machine_declaration_scope(machine, |left, right| left.symbol == right.symbol);
    if type_parameters.is_empty()
        && !typed
            .machine_specializations
            .iter()
            .any(|receipt| receipt.instance == machine.symbol)
    {
        carriers.push(EdgeCarrier::Machine {
            declaration: machine,
            lifetime_parameters: lifetime_parameters.len(),
        });
    }
    carriers
}

/// Every exact requirement edge for `template` that `machine` realizes.
fn exact_requirement_edges(
    typed: &TypedTrees,
    machine: &Machine,
    definition: &TraitDefinition,
    template: symbols::SymbolHandle,
) -> Result<Vec<RequirementEdge>, LoweringError> {
    let mut edges = Vec::new();
    for carrier in edge_carriers(typed, machine) {
        match carrier {
            EdgeCarrier::ConformanceMap(implementation) => {
                let mut matching = implementation.rows.iter().filter(|row| {
                    row.declaring_trait == definition.symbol && row.requirement == template
                });
                let Some(row) = matching.next() else {
                    continue;
                };
                if matching.next().is_some() {
                    return unsupported("named provider repeats an exact boundary requirement row");
                }
                let Some(realization) = typed.machine(row.realization_machine) else {
                    return unsupported("named provider row has no exact realization machine");
                };
                if !typed
                    .machine_states(realization)
                    .first()
                    .is_some_and(|entry| entry.symbol == row.realization_state)
                {
                    return unsupported("named provider row has no exact realization entry");
                }
                edges.push(RequirementEdge {
                    provider: implementation.symbol,
                    provider_identity: typed.symbols.display_path(implementation.symbol, "::"),
                    selecting_declaration: implementation.symbol,
                    realization: realization.symbol,
                    reach: if realization.target_family_interface {
                        // The row names a target family's head, the declaration
                        // every member body implements; each member is a
                        // candidate, and Omega installs the selected target's
                        // member. The head itself has no body and never enters
                        // the catalog.
                        EdgeReach::FamilyHead(realization.symbol)
                    } else if realization.symbol == machine.symbol {
                        EdgeReach::Named
                    } else {
                        EdgeReach::Specialization(realization.symbol)
                    },
                    lifetime_parameters: implementation.lifetime_parameters.len(),
                });
            }
            EdgeCarrier::Machine {
                declaration: owner,
                lifetime_parameters,
            } => {
                if !declares_exact_requirement(typed, owner, definition, template) {
                    continue;
                }
                let Some(provider_identity) = typed.attached_data_path(owner) else {
                    continue;
                };
                if !owner.attached_data_symbol.is_valid() {
                    continue;
                }
                edges.push(RequirementEdge {
                    provider: owner.attached_data_symbol,
                    provider_identity,
                    selecting_declaration: owner.symbol,
                    realization: owner.symbol,
                    reach: EdgeReach::Named,
                    lifetime_parameters,
                });
            }
        }
    }
    Ok(edges)
}

/// Whether this machine itself declares `satisfies Trait<::requirement` for the
/// requirement template. A `via` binding is an external leaf rather than a
/// checked adapter body, and resolves through its own supply path.
fn declares_exact_requirement(
    typed: &TypedTrees,
    machine: &Machine,
    definition: &TraitDefinition,
    template: symbols::SymbolHandle,
) -> bool {
    typed
        .machine_trait_conformances(machine)
        .iter()
        .any(|conformance| {
            conformance.external_binding.is_none()
                && matches!(
                    resolve_satisfied_declaration(typed, machine, conformance),
                    Some(SatisfiedDeclaration::Trait {
                        definition: satisfied,
                        requirement,
                    }) if satisfied.symbol == definition.symbol && requirement.symbol == template
                )
        })
}
