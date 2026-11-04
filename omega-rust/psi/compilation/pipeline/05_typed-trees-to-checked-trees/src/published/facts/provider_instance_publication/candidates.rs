//! Record exact provider edges before executable bodies are projected away.
//!
//! A member's independent satisfaction belongs to that machine, while Build
//! selects its recorded named implementation. Keep both identities and the
//! exact receiving application here. Lowering consumes this association; it
//! cannot reconstruct ownership from an attached data name or a body signature.

use checked_trees::{
    CheckedProviderCandidate, CheckedProviderCandidateDeclaration,
    CheckedProviderInstancePublication, CheckedProviderReceivingApplication, CheckedTrees,
};
use symbols::SymbolHandle;
use typed_trees::{
    TypedTrees,
    machine::{Machine, SatisfiedDeclaration, resolve_satisfied_declaration},
    trait_definition::{Conformance, TraitDefinition},
};

#[derive(Clone, Copy)]
enum Requirement<'a> {
    Trait {
        definition: &'a TraitDefinition,
        signature: &'a typed_trees::signature::StateSignature,
    },
    TopLevel(&'a Machine),
    Operator(&'a typed_trees::operator::OperatorDefinition),
}
impl Requirement<'_> {
    fn symbol(self) -> SymbolHandle {
        match self {
            Self::Trait { signature, .. } => signature.symbol,
            Self::TopLevel(machine) => machine.symbol,
            Self::Operator(operator) => operator.symbol,
        }
    }
}

pub(super) fn retain(checked: &CheckedTrees, publication: &mut CheckedProviderInstancePublication) {
    let typed = &checked.typed;
    let mut requirements = Vec::new();
    for definition in typed
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
    {
        requirements.extend(typed.executable_trait_machine_instances(definition).map(
            |signature| Requirement::Trait {
                definition,
                signature,
            },
        ));
    }
    requirements.extend(
        typed
            .machines()
            .iter()
            .filter(|machine| {
                machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
            })
            .map(Requirement::TopLevel),
    );
    requirements.extend(
        typed
            .all_operators()
            .filter(|operator| operator.is_boundary)
            .map(Requirement::Operator),
    );
    for machine in typed
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody)
    {
        for requirement in &requirements {
            match candidates(checked, publication, machine, *requirement) {
                Ok(declarations) => {
                    for declaration in declarations {
                        publication.candidates.insert(CheckedProviderCandidate {
                            boundary: requirement.symbol(),
                            candidate: machine.symbol,
                            declaration: Ok(declaration),
                        });
                    }
                }
                Err(reason) => {
                    publication.candidates.insert(CheckedProviderCandidate {
                        boundary: requirement.symbol(),
                        candidate: machine.symbol,
                        declaration: Err(reason),
                    });
                }
            }
        }
    }
}

fn candidates(
    checked: &CheckedTrees,
    publication: &CheckedProviderInstancePublication,
    machine: &Machine,
    requirement: Requirement<'_>,
) -> Result<Vec<CheckedProviderCandidateDeclaration>, String> {
    let typed = &checked.typed;
    if let Requirement::Trait {
        definition,
        signature,
    } = requirement
    {
        let mut declarations = Vec::new();
        if let Some(application) = typed.closed_requirement_application(signature.symbol) {
            // These complete associations were admitted before publication.
            // Their clause owner need not be the public selecting map, and a
            // closed specialization need not repeat its authored edge.
            for association in checked
                .facts
                .provider_closed_requirements
                .iter()
                .filter(|row| {
                    row.provider == machine.symbol
                        && row.template == application.template
                        && row.closed_requirement == signature.symbol
                        && row.receiving_application == application.trait_application_identity
                })
            {
                declarations.push(CheckedProviderCandidateDeclaration {
                    provider: provider_declaration(typed, association)?,
                    receiving: receiving_handle(publication, association)?,
                });
            }
        } else {
            for edge in exact_requirement_edges(typed, machine, definition, signature.symbol)? {
                if edge.lifetime_parameters > 0 || !edge.reach.reaches(typed, machine)? {
                    continue;
                }
                declarations.push(CheckedProviderCandidateDeclaration {
                    provider: edge.provider,
                    receiving: arena::Handle::default(),
                });
            }
        }
        return Ok(declarations);
    }
    let supplies = typed_trees::provider_declarations::independent_satisfactions(typed, machine)
        .any(|conformance| {
            if conformance.external_binding.is_some() {
                return false;
            }
            match requirement {
                Requirement::TopLevel(declaration) => {
                    (conformance.symbol == declaration.symbol
                        && conformance.requirement_symbol == declaration.symbol)
                        || checked
                            .facts
                            .provider_closed_requirements
                            .iter()
                            .any(|row| {
                                row.provider == machine.symbol
                                    && row.selecting_declaration == machine.symbol
                                    && row.template == conformance.symbol
                                    && row.closed_requirement == declaration.symbol
                            })
                }
                Requirement::Operator(operator) => conformance.requirement_symbol
                    == operator.symbol
                    && typed_trees::operator::resolve_satisfied_boundary_operator_for_conformance(
                        typed,
                        machine,
                        conformance,
                    )
                    .is_some_and(|resolved| resolved.symbol == operator.symbol),
                Requirement::Trait { .. } => unreachable!(),
            }
        });
    if !supplies {
        return Ok(Vec::new());
    }
    let provider =
        typed_trees::provider_declarations::satisfaction_provider_declaration(typed, machine)
            .ok_or("exact satisfaction has no recorded provider declaration")?;
    Ok(vec![CheckedProviderCandidateDeclaration {
        provider,
        receiving: arena::Handle::default(),
    }])
}

fn receiving_handle(
    publication: &CheckedProviderInstancePublication,
    association: &checked_trees::CheckedProviderClosedRequirement,
) -> Result<arena::Handle<CheckedProviderReceivingApplication>, String> {
    let mut rows = publication
        .receivers
        .iter()
        .filter(|(_, row)| row.association == *association);
    let (handle, _) = rows
        .next()
        .ok_or("provider lost its checked receiving publication")?;
    if rows.next().is_some() {
        return Err("provider has multiple checked receiving publications".into());
    }
    Ok(handle)
}

pub(super) fn provider_declaration(
    typed: &TypedTrees,
    association: &checked_trees::CheckedProviderClosedRequirement,
) -> Result<SymbolHandle, String> {
    if typed
        .conformance_by_symbol(association.selecting_declaration)
        .is_some()
    {
        return Ok(association.selecting_declaration);
    }
    let machine = typed
        .machine(association.selecting_declaration)
        .ok_or("provider association lost its selecting declaration")?;
    typed_trees::provider_declarations::satisfaction_provider_declaration(typed, machine)
        .ok_or_else(|| "provider association has no recorded provider declaration".into())
}

/// One exact requirement edge a candidate body may realize.
struct RequirementEdge {
    /// The declaration `Build::select_provider` names.
    provider: symbols::SymbolHandle,
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
    fn reaches(&self, typed: &TypedTrees, machine: &Machine) -> Result<bool, String> {
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
                let source = typed
                    .machine(*template)
                    .and_then(|machine| typed.machine_states(machine).first());
                let target = typed.machine_states(machine).first();
                let (Some(source), Some(target)) = (source, target) else {
                    return Err("named provider lost its instantiated entry".into());
                };
                Ok(typed.instantiated_machine_state_matches(
                    machine.symbol,
                    *template,
                    source.symbol,
                    target.symbol,
                ))
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
) -> Result<Vec<RequirementEdge>, String> {
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
                    return Err("named provider repeats an exact boundary requirement row".into());
                }
                let Some(realization) = typed.machine(row.realization_machine) else {
                    return Err("named provider row has no exact realization machine".into());
                };
                if !typed
                    .machine_states(realization)
                    .first()
                    .is_some_and(|entry| entry.symbol == row.realization_state)
                {
                    return Err("named provider row has no exact realization entry".into());
                }
                edges.push(RequirementEdge {
                    provider: implementation.symbol,
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
                let Some(provider) =
                    typed_trees::provider_declarations::satisfaction_provider_declaration(
                        typed, owner,
                    )
                else {
                    return Err("exact satisfaction has no recorded provider declaration".into());
                };
                edges.push(RequirementEdge {
                    provider,
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
    typed_trees::provider_declarations::independent_satisfactions(typed, machine).any(
        |conformance| {
            conformance.external_binding.is_none()
                && matches!(
                    resolve_satisfied_declaration(typed, machine, conformance),
                    Some(SatisfiedDeclaration::Trait {
                        definition: satisfied,
                        requirement,
                    }) if satisfied.symbol == definition.symbol && requirement.symbol == template
                )
        },
    )
}
