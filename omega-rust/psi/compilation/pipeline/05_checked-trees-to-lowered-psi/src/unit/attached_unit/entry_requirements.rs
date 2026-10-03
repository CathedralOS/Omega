//! Publish the same authored invocation contract for every callable body.
//! Structural entry predicates and closed scalar clauses share one namespace;
//! cleanup selection cannot add or remove a caller obligation.
use crate::lowering_error::LoweringError;
use crate::unit::runtime_requirements::lower_structural_runtime_requirement;
use checked_trees::CheckedTrees;
use semantic_vocabulary::Proposition;
use symbols::SymbolHandle;
use terminal_psi::{StructuralParameterDeclaration, StructuralTypeDeclaration, ValueDeclaration};

pub(crate) struct EntryRequirements {
    pub(crate) requires: Vec<Proposition>,
    pub(crate) runtime_requirements: Vec<Proposition>,
}

pub(crate) fn lower(
    checked: &CheckedTrees,
    source: SymbolHandle,
    scalar_parameters: &[ValueDeclaration],
    erased_scalar_parameters: &[ValueDeclaration],
    predicate_parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<EntryRequirements, LoweringError> {
    let contract =
        checked
            .facts
            .contract_plans
            .for_machine(source)
            .ok_or(LoweringError::Unsupported(
                "callable entry has no canonical checked contract",
            ))?;
    let mut requires = Vec::new();
    let mut runtime_requirements = Vec::new();
    // Requirements justify body operations and calls independently of
    // whether the published crash ceiling mentions gated arithmetic.
    if contract.crash.structural_runtime_requirements().is_some()
        || contract.crash.uses_structural_proof_gated_arithmetic()
    {
        let checked_requirements = contract.crash.structural_runtime_requirements().ok_or(
            LoweringError::Unsupported(
                "proof-gated structural arithmetic lacks a complete checked requirement package",
            ),
        )?;
        crate::unit::runtime_requirements::validate_runtime_memberships(checked, source)?;
        let requirements = checked_requirements
            .iter()
            .map(|requirement| {
                lower_structural_runtime_requirement(
                    requirement,
                    scalar_parameters,
                    erased_scalar_parameters,
                    predicate_parameters,
                    structural_types,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut keyed = requirements
            .into_iter()
            .map(|requirement| {
                terminal_codec::canonical_proposition_order_key(&requirement)
                    .map(|key| (key, requirement))
                    .map_err(|_| {
                        LoweringError::Unsupported(
                            "structural runtime requirement is not canonically encodable",
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        keyed.sort_by(|left, right| left.0.cmp(&right.0));
        keyed.dedup_by(|left, right| left.0 == right.0);
        runtime_requirements = keyed
            .into_iter()
            .map(|(_, requirement)| requirement)
            .collect();
    }
    // Publish each closed authored `requires` clause as its own row
    // beside the runtime requirements: a Unit callee's roster has one row
    // per authored clause, so a caller owes one obligation per clause. A
    // clause the closed lane never retained keeps the historical
    // runtime-only contract rather than a partial roster drifting from
    // the checked plan. Crash-gated arithmetic's runtime requirements can
    // restate an authored clause; the canonical dedup below keeps one
    // row per fact, where one merged conjunction beside them would have
    // published every clause twice.
    let authored_requires = contract.closed_scalar_values.authored_requires();
    if authored_requires.iter().all(Option::is_some) {
        let view_roster: Vec<(u32, StructuralParameterDeclaration)> = predicate_parameters
            .iter()
            .map(|parameter| (parameter.position, parameter.clone()))
            .collect();
        let element_views =
            crate::expression_preparation::bindings::element_views(&view_roster, structural_types);
        let views = crate::scalar_graph::scalar_contracts::ContractViewNamespace {
            parameters: &view_roster,
            structural_types,
            element_views: &element_views,
        };
        requires = authored_requires
            .iter()
            .map(|clause| {
                crate::scalar_graph::scalar_contracts::clauses(
                    std::slice::from_ref(clause),
                    scalar_parameters,
                    erased_scalar_parameters,
                    &views,
                )
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .filter(|clause| *clause != Proposition::Truth)
            .collect();
    }
    requires.extend(runtime_requirements.iter().cloned());
    // A requires roster is itself a conjunction, so a row that is a
    // top-level conjunction (crash arithmetic's runtime package restates
    // the authored clauses that way) is split into its conjuncts. Each
    // fact is then one row, and the dedup below cannot keep both a
    // clause and a conjunction containing it.
    let mut pending = std::mem::take(&mut requires);
    pending.reverse();
    while let Some(requirement) = pending.pop() {
        match requirement {
            Proposition::Conjunction(conjuncts) => {
                pending.extend(conjuncts.into_iter().rev());
            }
            other => requires.push(other),
        }
    }
    // The codec publishes requires rows in canonical byte order, so merge
    // then normalize rather than exposing clause/runtime ordering
    // accidents. `Proposition`'s derived `Ord` is Rust declaration order,
    // not the canonical key; the shared helper orders by encoded bytes.
    crate::scalar_graph::scalar_contracts::canonicalize_requires(&mut requires)?;
    Ok(EntryRequirements {
        requires,
        runtime_requirements,
    })
}
