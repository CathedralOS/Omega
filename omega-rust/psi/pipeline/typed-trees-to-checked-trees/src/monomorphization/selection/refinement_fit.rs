//! Transparent-refinement carriers on static evidence binders.
//!
//! `wiki/spec/language/conformances.md` ("Transparent refinements"):
//! "A refinement names a structural bound on an existing base conformance,
//! not a new nominal satisfaction target. A machine cannot `satisfies
//! LocalLogger`; a static evidence binder may require it and receive an
//! explicitly selected `Logger` conformance whose complete contract fits."
//!
//! Resolving the carrier to its base is only half of that sentence: the
//! selected conformance must also FIT. Admitting the identity without the
//! fit check would silently accept a base conformance the refinement
//! forbids, so both live here and are consulted together.

use crate::monomorphization::{Diagnostic, SymbolHandle, TypedTrees};
use typed_trees::trait_definition::{
    Conformance, TraitDefinition, TraitRefinementClause, TraitRefinementReach,
};

/// The nominal base a conformance-bound carrier denotes, plus the refinement
/// that named it. An ordinary trait carrier denotes itself and refines
/// nothing.
pub(crate) struct ResolvedBoundCarrier<'program> {
    /// The nominal trait a selected conformance must actually satisfy.
    pub(crate) base: SymbolHandle,
    /// The refinement whose clauses bound that base, when the authored
    /// carrier was a refinement.
    pub(crate) refinement: Option<&'program TraitDefinition>,
}

/// Resolve an authored conformance-bound carrier through `refines`.
///
/// Symbol resolution rejects a refinement of a refinement ("refine the
/// nominal base directly"), so exactly one hop reaches the nominal base.
pub(crate) fn resolve_bound_carrier<'program>(
    program: &'program TypedTrees,
    carrier: SymbolHandle,
) -> ResolvedBoundCarrier<'program> {
    let Some(definition) = program
        .traits()
        .iter()
        .find(|definition| definition.symbol == carrier)
    else {
        return ResolvedBoundCarrier {
            base: carrier,
            refinement: None,
        };
    };
    match &definition.refines {
        Some(base) => ResolvedBoundCarrier {
            base: base.symbol,
            refinement: Some(definition),
        },
        None => ResolvedBoundCarrier {
            base: carrier,
            refinement: None,
        },
    }
}

/// "`machine *` applies to every present and future base requirement. A
/// targeted clause names one exact requirement. Unmentioned requirements and
/// contract axes inherit the base." An exact name therefore wins over the
/// wildcard, and no clause at all is inheritance — no concrete obligation.
fn covering_clause<'program>(
    refinement: &'program TraitDefinition,
    requirement: &str,
) -> Option<&'program TraitRefinementClause> {
    refinement
        .refinement_clauses
        .iter()
        .find(|clause| {
            clause
                .requirement
                .as_ref()
                .is_some_and(|name| name.as_str() == requirement)
        })
        .or_else(|| {
            refinement
                .refinement_clauses
                .iter()
                .find(|clause| clause.requirement.is_none())
        })
}

/// Reject the selected conformance when one of its realization machines
/// violates an axis the covering clause AUTHORED. "Omission here means
/// inheritance, unlike the omission rules of an ordinary machine contract",
/// so only authored axes carry an obligation at the concrete site.
pub(crate) fn refinement_fit_diagnostics(
    program: &TypedTrees,
    refinement: &TraitDefinition,
    conformance: &Conformance,
    template_name: &str,
    binder_name: &str,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    // A bodyless conformance retains no rows here; evidence binding rejects
    // it on its own ground before a structural bound could be read off it.
    let Some(rows) = program.closed_conformance_rows(conformance) else {
        return diagnostics;
    };
    let selected_name = conformance
        .alias
        .as_ref()
        .map_or("<unnamed>", |name| name.as_str());
    for row in rows {
        let Some(clause) = covering_clause(refinement, row.requirement_name.as_str()) else {
            continue;
        };
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == row.realization_machine)
        else {
            // An invalid realization survivor is an incomplete internal row
            // and fails closed in its own check.
            continue;
        };
        let reject = |axis: String| {
            Diagnostic::error(format!(
                "generic machine `{template_name}` cannot bind `{binder_name}` to conformance `{selected_name}`: refinement `{}` {axis}",
                refinement.name.as_str(),
            ))
        };
        let requirement = row.requirement_name.as_str();
        let realization = row.realization_name.as_str();

        // "`suspends false` and `blocks false` explicitly remove those
        // possibilities". An authored `suspends true` re-states what the
        // base already permits and binds nothing extra.
        if !clause.signature.suspends_keyword_source_spans.is_empty()
            && !clause.signature.suspends
            && machine.suspends
        {
            diagnostics.push(reject(format!(
                "removes `suspends` from `{requirement}`, but its selected realization `{realization}` suspends",
            )));
        }
        if !clause.signature.blocks_keyword_source_spans.is_empty()
            && !clause.signature.blocks
            && machine.blocks
        {
            diagnostics.push(reject(format!(
                "removes `blocks` from `{requirement}`, but its selected realization `{realization}` blocks",
            )));
        }

        // "Refinements may narrow obligations or strengthen guarantees":
        // an authored `terminates` demands a PUBLISHED termination promise.
        // A privately derived body publishes none, so it cannot carry the
        // strengthened guarantee. (There is no `terminates false` spelling;
        // omission is inheritance.)
        if matches!(
            clause.signature.termination_guarantee,
            language_semantics::TerminationGuarantee::Terminates { .. }
        ) && !matches!(
            machine.termination_plan.interface.published(),
            Some(language_semantics::TerminationGuarantee::Terminates { .. })
        ) {
            diagnostics.push(reject(format!(
                "requires `terminates` on `{requirement}`, but its selected realization `{realization}` publishes no termination guarantee",
            )));
        }

        // "`reaches;` is empty. `reaches _;` introduces an independent
        // abstract row for that requirement bounded by the inherited row."
        // Only a concrete clause row bounds a concrete realization; the
        // inherited and independent-abstract forms impose nothing here.
        if let TraitRefinementReach::Concrete(clause_row) = clause.service_reach {
            let permitted = program.service_reach_rows.services(clause_row);
            let reached = program
                .service_reach_rows
                .services(machine.service_reach_row);
            if let Some(escaping) = reached
                .iter()
                .find(|service| !permitted.contains(service))
                .copied()
            {
                let escaping = program
                    .service_reaches
                    .definition(escaping)
                    .map_or("<service>", |definition| definition.name.as_str());
                diagnostics.push(reject(format!(
                    "narrows `reaches` on `{requirement}`, but its selected realization `{realization}` reaches `{escaping}`",
                )));
            }
        }
    }
    diagnostics
}
