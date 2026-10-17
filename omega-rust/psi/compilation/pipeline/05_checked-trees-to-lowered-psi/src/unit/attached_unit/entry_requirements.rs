//! Publish the same checked invocation contract for every callable body.
//! Structural entry predicates and closed scalar clauses share one namespace;
//! bound generic cleanup prerequisites retain their distinct checked origin.
use crate::lowering_error::LoweringError;
use crate::unit::runtime_requirements::lower_structural_runtime_requirement;
use checked_trees::CheckedTrees;
use semantic_vocabulary::Proposition;
use symbols::SymbolHandle;
use terminal_psi::{StructuralParameterDeclaration, StructuralTypeDeclaration, ValueDeclaration};

pub(crate) struct EntryRequirements {
    pub(crate) requires: Vec<Proposition>,
    pub(crate) runtime_requirements: Vec<Proposition>,
    pub(crate) source_checked_requires: Vec<Proposition>,
}

pub(crate) fn lower(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
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
        runtime_requirements = checked_requirements
            .iter()
            .map(|requirement| {
                lower_structural_runtime_requirement(
                    identities,
                    requirement,
                    scalar_parameters,
                    erased_scalar_parameters,
                    predicate_parameters,
                    structural_types,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
    }
    let contextual = &checked.facts.contextual_cleanup.entry_requirements;
    for (_, requirement) in contextual.rows.iter() {
        if requirement.machine != source {
            continue;
        }
        let expression = requirement.predicate();
        runtime_requirements.push(lower_structural_runtime_requirement(
            identities,
            &expression,
            scalar_parameters,
            erased_scalar_parameters,
            predicate_parameters,
            structural_types,
        )?);
    }
    crate::scalar_lowering::contracts::canonicalize_requires(&mut runtime_requirements)?;
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
        let views = crate::scalar_lowering::contracts::ContractViewNamespace {
            result: crate::scalar_lowering::contracts::ContractStructuralResult::Absent,
            parameters: &view_roster,
            structural_types,
            element_views: &element_views,
        };
        requires = authored_requires
            .iter()
            .map(|clause| {
                crate::scalar_lowering::contracts::clauses(
                    identities,
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
    // Source checking already decides each authored requirement at every
    // ordinary call occurrence. Keep that origin through canonical merging;
    // a generated runtime row must not acquire it merely by roster position.
    let mut source_checked_requires = requires.clone();
    normalize_requires(&mut source_checked_requires)?;
    requires.extend(runtime_requirements.iter().cloned());
    normalize_requires(&mut requires)?;
    Ok(EntryRequirements {
        requires,
        runtime_requirements,
        source_checked_requires,
    })
}

fn normalize_requires(requires: &mut Vec<Proposition>) -> Result<(), LoweringError> {
    // A requires roster is itself a conjunction, so a row that is a
    // top-level conjunction (crash arithmetic's runtime package restates
    // the authored clauses that way) is split into its conjuncts. Each
    // fact is then one row, and the dedup below cannot keep both a
    // clause and a conjunction containing it.
    let mut pending = std::mem::take(requires);
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
    crate::scalar_lowering::contracts::canonicalize_requires(requires)
}
