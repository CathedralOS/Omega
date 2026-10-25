//! Published Trap ceilings over executable Trapping operations.
//!
//! Guarded crashes (wiki/spec/language/effects.md#guarded-crashes) require,
//! for each derived site guard `D`, that the same-cause published guards
//! `C_i` satisfy `D implies OR_i(C_i)`. A Trapping primitive traps exactly
//! when its Exact counterpart's formation conditions fail: a conversion's
//! operand outside the target carrier (or a NaN float source), a sum,
//! difference or product outside its carrier, a zero divisor or the signed
//! `MIN / -1` pair, a shift count outside the shifted width. A site's `D` is
//! that failure over its own operands, carrier and signedness, conjoined with
//! the path facts that reach it.
//!
//! The implication is decided contrapositively by the judgment that already
//! decides Exact formation: narrow the site's flow environment by the
//! negation of every published Trap route, then require the operation to form
//! exactly. No route is matched by predicate shape, so a route over another
//! operand, one that names a wider carrier, or one that misses a side of a
//! signed range leaves the operation able to trap, and the site is uncovered.
//!
//! Routes describe invocation-entry values. A route joins the complement only
//! at an entry-state site and only when every place it reads is an immutable
//! by-value scalar entry parameter, whose current value is its entry value.
//! Any other route contributes no premise: dropping a conjunct of the
//! complement weakens the assumption, which keeps the judgment sound and only
//! costs completeness.
//!
//! An inferred interface (a private checked body without a `crashes` clause)
//! widens its summary from these sites instead, and an unconditional
//! `crashes Trap` permits every trap. Explicit `crash` transitions, calls and
//! selected operators keep their coverage checks in checked-tree production.

use super::ExpressionWalk;
use crate::proof_contracts::arithmetic_domains::guard_narrowing::narrow_environment_by_condition;
use crate::proof_contracts::arithmetic_domains::{
    Diagnostic, ExpressionHandle, ExpressionNode, Machine, ProofFact, SignatureContractKind, State,
    ValueEnvironment,
};
use typed_trees::signature::CrashCause;

/// What a machine's crash interface permits for a Trap.
enum TrapCeiling {
    /// An inferred body summary, or an unconditional `crashes Trap` route.
    Permitted,
    /// The guarded `crashes Trap` alternatives; empty when the published
    /// ceiling names no Trap route at all.
    Guarded(Vec<ExpressionHandle>),
}

fn trap_ceiling(program: &dyn crate::CheckingInput, machine: &Machine) -> TrapCeiling {
    if !program.machine_publishes_crash_ceiling(machine) {
        return TrapCeiling::Permitted;
    }
    let mut routes = Vec::new();
    for contract in program.machine_contracts(machine) {
        if !matches!(
            contract.kind,
            SignatureContractKind::Crashes {
                cause: CrashCause::Trap
            }
        ) {
            continue;
        }
        let facts = program.proof_facts.span_or_empty(contract.facts);
        // An empty route list means `true`, as does a literal `true` route.
        if facts.is_empty()
            || facts.iter().any(|fact| {
                matches!(fact, ProofFact::Expression(route)
                    if matches!(program.expression_table.expression(*route), ExpressionNode::Boolean(true)))
            })
        {
            return TrapCeiling::Permitted;
        }
        // A membership or proposition route has no interval reading; leaving
        // it out of the complement only weakens the assumption.
        routes.extend(facts.iter().filter_map(|fact| match fact {
            ProofFact::Expression(route) => Some(*route),
            ProofFact::Membership(_) | ProofFact::Proposition(_) => None,
        }));
    }
    TrapCeiling::Guarded(routes)
}

/// Whether every place `route` reads is an immutable by-value scalar
/// parameter of the entry state, so its current value is the invocation-entry
/// value the published route describes.
fn reads_only_stable_entry_parameters(
    program: &dyn crate::CheckingInput,
    entry: &State,
    route: ExpressionHandle,
) -> bool {
    let mut pending = vec![route];
    while let Some(expression) = pending.pop() {
        match program.expression_table.expression(expression) {
            ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => {}
            ExpressionNode::Unary(unary) => pending.push(unary.operand),
            ExpressionNode::Binary(binary) => pending.extend([binary.left, binary.right]),
            ExpressionNode::Name(path)
                if program
                    .expression_table
                    .name_path_members(path.members)
                    .len()
                    == 1 =>
            {
                let stable = program.state_parameters(entry).iter().any(|parameter| {
                    parameter.symbol == path.symbol
                        && !parameter.is_mutable
                        && program
                            .primitive_type_reference(parameter.type_reference)
                            .is_some()
                });
                if !stable {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}

impl ExpressionWalk<'_> {
    /// Require a Trapping operation's possible trap to fit the machine's
    /// published Trap ceiling. `forms_on_this_path` decides the operation's
    /// Exact formation under this walk's own facts; `forms_under` re-decides
    /// it under another walk's environment. `site` names the operation, its
    /// operands and what makes it trap, for the diagnostic.
    pub(super) fn require_trap_coverage(
        &self,
        expression: ExpressionHandle,
        forms_on_this_path: impl FnOnce() -> bool,
        forms_under: impl FnOnce(&ExpressionWalk) -> bool,
        site: impl FnOnce() -> String,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if !self.checks_trap_coverage {
            return;
        }
        let Some(machine) = self.machine else {
            if !forms_on_this_path() {
                diagnostics.push(Diagnostic::error(format!("uncovered Trap crash in {}: {} has no executable crash route in this declaration", self.owner, site())));
            }
            return;
        };
        let TrapCeiling::Guarded(routes) = trap_ceiling(self.program, machine) else {
            return;
        };
        if forms_on_this_path() {
            return;
        }
        if let Some(complement) = self.route_complement_environment(&routes) {
            let covering = ExpressionWalk {
                environment: &complement,
                checks_trap_coverage: false,
                ..*self
            };
            if forms_under(&covering) {
                return;
            }
        }
        let coverage = if routes.is_empty() {
            format!(
                "the published crash ceiling of machine `{}` names no `crashes Trap` route",
                machine.name,
            )
        } else {
            "no published `crashes Trap` route covers that condition over these operands".to_owned()
        };
        diagnostics.push(
            Diagnostic::error(format!(
                "uncovered Trap crash in {}: {}, and {coverage}; publish a same-cause \
                 `crashes Trap` route whose guard covers this site, or prove the operation \
                 cannot trap here",
                self.owner,
                site(),
            ))
            .with_source_span(self.program.expression_table.source_span(expression)),
        );
    }

    /// An operand's source spelling for a site diagnostic.
    pub(super) fn operand_label(&self, operand: ExpressionHandle) -> String {
        self.program
            .expression_table
            .display_name(operand)
            .trim_end()
            .to_owned()
    }

    /// The site environment narrowed by the negation of every published Trap
    /// route that reads only stable entry parameters. `None` when no route can
    /// join: the site is outside the entry state or every route reads other
    /// storage.
    fn route_complement_environment(
        &self,
        routes: &[ExpressionHandle],
    ) -> Option<ValueEnvironment> {
        let state = self.state?;
        let machine = self.machine?;
        let entry = self.program.machine_states(machine).first()?;
        if entry.symbol != state.symbol {
            return None;
        }
        let mut complement = self.environment.clone();
        let mut narrowed = false;
        for &route in routes {
            if !reads_only_stable_entry_parameters(self.program, entry, route) {
                continue;
            }
            narrow_environment_by_condition(
                self.program,
                machine,
                Some(entry),
                &mut complement,
                route,
                false,
            );
            narrowed = true;
        }
        narrowed.then_some(complement)
    }
}
