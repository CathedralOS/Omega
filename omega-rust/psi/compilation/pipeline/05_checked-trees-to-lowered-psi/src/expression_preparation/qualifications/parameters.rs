//! Effective scalar formals include exact authored entry memberships.
//! A bare type annotation does not erase a requires membership. Freeze that
//! requirement on the formal's SSA declaration, so forwarding uses the same
//! qualified operand path as a constrained-type spelling. Numeric predicates
//! and invocation admission retain their independent owners.

use super::{
    PreparedScalarQualifications, carries_scalar_qualifications, retain_domain,
    retained_domain_predicate, terminal_scalar_type, type_atoms,
};
use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::CheckedTrees;
use language_semantics::SemanticDomainId;
use semantic_vocabulary::{QualifiedScalarType, ScalarQualificationSetId, ScalarType};
use symbols::SymbolHandle;
use terminal_psi::ScalarQualificationCatalog;

pub(super) struct ParameterAtoms {
    state: SymbolHandle,
    position: u32,
    carrier: ScalarType,
    domains: Vec<SemanticDomainId>,
}

#[derive(Clone)]
pub(super) struct ParameterQualification {
    pub state: SymbolHandle,
    pub position: u32,
    pub value_type: QualifiedScalarType,
}

pub(super) fn prepare(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    bodies: &[SymbolHandle],
    catalog: &mut ScalarQualificationCatalog,
    atom_sets: &mut Vec<Vec<String>>,
) -> Result<Vec<ParameterAtoms>, LoweringError> {
    let mut parameters = Vec::new();
    for symbol in bodies {
        let machine = checked.machine(*symbol).ok_or(LoweringError::Unsupported(
            "scalar parameter namespace lost its machine",
        ))?;
        for (state_ordinal, state) in checked.machine_states(machine).iter().enumerate() {
            for (position, parameter) in checked.state_parameters(state).iter().enumerate() {
                let Some(primitive) = checked.primitive_type_reference(parameter.type_reference)
                else {
                    continue;
                };
                if parameter.relevance.is_erased() {
                    continue;
                }
                let carrier = terminal_scalar_type(primitive)?;
                let mut atoms = if carries_scalar_qualifications(checked, parameter.type_reference)
                {
                    type_atoms(checked, parameter.type_reference)?.1
                } else {
                    Vec::new()
                };
                let contracts = checked.state_contracts(state).iter().chain(
                    checked
                        .machine_contracts(machine)
                        .iter()
                        .filter(|_| state_ordinal == 0),
                );
                for contract in contracts.filter(|contract| {
                    contract.kind == typed_trees::signature::SignatureContractKind::Requires
                        && contract.binding.is_none()
                }) {
                    for fact in checked.proof_facts.span_or_empty(contract.facts) {
                        let Some(membership) = fact.as_membership() else {
                            continue;
                        };
                        if validation::single_segment_name_symbol(&checked.typed, membership.value)
                            != Some(parameter.symbol)
                        {
                            continue;
                        }
                        let Some(definition) = checked.domain_definition(membership.domain_symbol)
                        else {
                            return unsupported("scalar entry membership lost its domain");
                        };
                        if definition.classification
                            == Some(language_semantics::DomainClassification::ProgressProfile)
                        {
                            continue;
                        }
                        let routed = validation::field_domains::domain_requires_provenance(
                            &checked.typed,
                            membership.domain_symbol,
                        );
                        let vacuous = validation::field_domains::domain_is_vacuous(
                            &checked.typed,
                            membership.domain_symbol,
                            &mut Vec::new(),
                        );
                        if !routed && !vacuous {
                            continue;
                        }
                        if validation::scalar_membership_value_carrier(&checked.typed, membership)
                            != Some(primitive)
                        {
                            return unsupported(
                                "scalar entry membership has no exact primitive application",
                            );
                        }
                        let predicate = if routed {
                            let subject = checked_trees::CheckedScalarExpression::Parameter {
                                position: 0,
                                primitive_type: primitive,
                            };
                            let predicate = validation::scalar_membership_value_predicates(
                                &checked.typed,
                                membership,
                                &subject,
                            )
                            .ok_or(LoweringError::Unsupported(
                                "scalar entry membership has no complete predicate",
                            ))?
                            .into_iter()
                            .reduce(|left, right| checked_trees::CheckedBooleanExpression::And {
                                left: Box::new(left),
                                right: Box::new(right),
                            })
                            .unwrap_or(checked_trees::CheckedBooleanExpression::Constant(true));
                            Some(retained_domain_predicate(identities, &predicate, carrier)?)
                        } else {
                            None
                        };
                        retain_domain(
                            catalog,
                            identities,
                            checked,
                            membership.domain_symbol,
                            membership.semantic_domain,
                            carrier,
                            predicate,
                        )?;
                        if !atoms.contains(&(membership.domain_symbol, membership.semantic_domain))
                        {
                            atoms.push((membership.domain_symbol, membership.semantic_domain));
                        }
                    }
                }
                let domains = atoms.iter().map(|(_, domain)| *domain).collect::<Vec<_>>();
                let mut names = domains
                    .iter()
                    .map(|domain| {
                        checked
                            .semantic_domains
                            .name(*domain)
                            .map(str::to_owned)
                            .ok_or(LoweringError::Unsupported(
                                "scalar parameter lost a canonical domain",
                            ))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                names.sort();
                names.dedup();
                if !names.is_empty() {
                    atom_sets.push(names);
                }
                parameters.push(ParameterAtoms {
                    state: state.symbol,
                    position: u32::try_from(position).map_err(|_| {
                        LoweringError::Unsupported("scalar parameter position exceeds u32")
                    })?,
                    carrier,
                    domains,
                });
            }
        }
    }
    Ok(parameters)
}

pub(super) fn resolve(
    catalog: &ScalarQualificationCatalog,
    parameters: Vec<ParameterAtoms>,
) -> Result<Vec<ParameterQualification>, LoweringError> {
    parameters
        .into_iter()
        .map(|parameter| {
            let mut domains = parameter
                .domains
                .iter()
                .map(|semantic| {
                    catalog
                        .domains
                        .iter()
                        .find(|domain| {
                            domain.semantic_domain.get() == u64::from(semantic.0)
                                && domain.carrier == parameter.carrier
                        })
                        .map(|domain| domain.id)
                        .ok_or(LoweringError::Unsupported(
                            "scalar formal lost its prepared domain",
                        ))
                })
                .collect::<Result<Vec<_>, _>>()?;
            domains.sort();
            domains.dedup();
            let qualifications = if domains.is_empty() {
                ScalarQualificationSetId::ZERO
            } else {
                catalog
                    .sets
                    .iter()
                    .find(|set| set.domains == domains)
                    .map(|set| set.id)
                    .ok_or(LoweringError::Unsupported(
                        "scalar formal lost its complete qualification set",
                    ))?
            };
            Ok(ParameterQualification {
                state: parameter.state,
                position: parameter.position,
                value_type: QualifiedScalarType {
                    scalar_type: parameter.carrier,
                    qualifications,
                },
            })
        })
        .collect()
}

impl PreparedScalarQualifications {
    pub(crate) fn parameter_value_type(
        &self,
        checked: &CheckedTrees,
        state: SymbolHandle,
        position: u32,
    ) -> Result<QualifiedScalarType, LoweringError> {
        self.parameters
            .iter()
            .find(|parameter| parameter.state == state && parameter.position == position)
            .map(|parameter| parameter.value_type)
            .ok_or_else(|| LoweringError::MissingScalarParameterQualification {
                state: checked.symbols.display_path(state, "::"),
                source_position: position,
                prepared_positions: self.parameters.iter().filter(|parameter| parameter.state == state)
                    .map(|parameter| parameter.position).collect(),
            })
    }
}
