//! Recursive arithmetic expression analysis.
//!
//! This module owns the operand-domain walk and interval result for one
//! expression. Declaration checks, flow-state ownership, and total-proposition
//! formation stay in their dedicated modules.
//!
//! [`analyze`] checks evaluated children even when their parent is an aggregate,
//! projection, or a call with a known result. Operators and dispatch own their
//! contextual operand walks; complete anonymous numeric subtrees keep their
//! exact mathematical folding. Other nodes check their children before
//! `folded_values` supplies a result independent of that structure:
//! operators raise their domain and overflow obligations in `binary` and
//! `unary`, conversions in `cast`, call results take their declared or
//! inferred bounds in `call`, `dispatch` checks branches and joins their result
//! bounds, and places read their flow-tracked interval in `place`.

mod binary;
mod call;
mod cast;
mod dispatch;
mod folded_values;
mod place;
mod trap_coverage;
mod unary;
mod unsigned_constants;

use super::{
    ArithmeticDomain, Diagnostic, ExpressionHandle, ExpressionNode, Interval, Machine,
    PrimitiveType, State, ValueEnvironment, integer_ranges::literal_interval,
};

/// The result of analysing an expression for the domain + overflow rules.
pub(super) struct Analysis {
    pub(super) recipe: Option<checked_trees::IntegerRangeRecipe>,
    /// The arithmetic domain (`None` = neutral: a literal or `bool` result).
    pub(super) domain: Option<ArithmeticDomain>,
    /// The value range, for the overflow proof obligation.
    pub(super) interval: Interval,
    /// The integer primitive type, for the overflow range bound (`None` when it
    /// cannot be determined, e.g. a bare literal).
    pub(super) primitive: Option<PrimitiveType>,
}

impl Analysis {
    /// Preserve a target-dependent snapshot, or materialize the concrete
    /// mathematical bounds already known at this evaluation position.
    pub(super) fn range_recipe(
        &self,
        program: &dyn crate::CheckingInput,
        owner: symbols::SymbolHandle,
        expression: ExpressionHandle,
    ) -> Option<checked_trees::IntegerRangeRecipe> {
        use numerics::bignum::BigInt;
        if let Some(recipe) = &self.recipe {
            return Some(recipe.clone());
        }
        if let Some(value) = program.closed_integer_expression_value_in(expression, owner) {
            return Some(checked_trees::IntegerRangeRecipe::bounds(
                value.clone(),
                value,
            ));
        }
        let low = self.interval.low.map(BigInt::from_i64)?;
        let high = match self.interval.high {
            Some(high) => BigInt::from_i64(high),
            None if self.primitive == Some(PrimitiveType::U64) => BigInt::from_u64(u64::MAX),
            None => return None,
        };
        Some(checked_trees::IntegerRangeRecipe::bounds(low, high))
    }
}

/// The value judgment and the occurrence decisions made while establishing it.
pub(super) struct CheckedAnalysis {
    pub(super) value: Analysis,
    pub(super) decisions: ArithmeticDecisions,
}

/// Occurrence decisions lowering consumes instead of proving again. Each one
/// travels out of the same branch-sensitive walk that justified it;
/// reconstructing it later from syntax loses the environment that justified
/// an arm's cast or a guarded divisor.
#[derive(Debug, Default)]
pub(crate) struct ArithmeticDecisions {
    pub(crate) integer_range_obligations: Vec<checked_trees::IntegerRangeObligation>,
    pub(crate) exact_integer_casts: Vec<crate::ExactIntegerCastFact>,
    /// Immediate receiving ranges proved for exact assignment occurrences.
    /// A deferred invariant window never contributes a successful row.
    pub(crate) integer_assignments: Vec<checked_trees::CheckedIntegerAssignmentDecision>,
    /// Integer operations whose every formation condition under their
    /// selected policy was proven: a nonzero divisor and, for an Exact signed
    /// division, the excluded `MIN / -1` pair; a representable Exact sum,
    /// difference or product; an in-width shift count plus any Exact
    /// left-shift representability condition.
    pub(crate) decided_formations: Vec<ExpressionHandle>,
    /// The interval of each integer expression this walk checked, in the
    /// branch-local environment it was checked in: the left operand of `&&`
    /// or `||` and refuted earlier arms narrow what follows. Checking clients
    /// consult it instead of keeping private interval engines: the range
    /// checker bounds `values[position - 1]` behind `position > 0 &&` with it,
    /// and the call-requires check proves `index + 1 <= 4` after a refuted
    /// `index >= 4 ->` arm.
    pub(crate) expression_values: Vec<CheckedIntegerValue>,
}

#[derive(Debug)]
pub(crate) struct CheckedIntegerValue {
    pub(crate) expression: ExpressionHandle,
    pub(crate) interval: Interval,
    pub(crate) recipe: Option<checked_trees::IntegerRangeRecipe>,
}

impl ArithmeticDecisions {
    pub(super) fn record_interval(&mut self, expression: ExpressionHandle, interval: Interval) {
        // An unconstrained occurrence participates in the join too. Omitting
        // it would let a bounded visit stand for every visit of the expression.
        self.expression_values.push(CheckedIntegerValue {
            expression,
            interval,
            recipe: None,
        });
    }

    pub(crate) fn extend(&mut self, other: Self) {
        self.integer_range_obligations
            .extend(other.integer_range_obligations);
        self.exact_integer_casts.extend(other.exact_integer_casts);
        self.integer_assignments.extend(other.integer_assignments);
        self.decided_formations.extend(other.decided_formations);
        self.expression_values.extend(other.expression_values);
    }
}

const NEUTRAL: Analysis = Analysis {
    recipe: None,
    domain: None,
    interval: Interval::UNBOUNDED,
    primitive: None,
};

/// A comparison, logical operator, or `!` yields a `bool` whose integer value
/// is 0 or 1, never an unbounded operand for enclosing arithmetic.
const BOOLEAN_INTERVAL: Interval = Interval {
    low: Some(0),
    high: Some(1),
};

#[allow(clippy::too_many_arguments)]
pub(super) fn analyze(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    environment: &ValueEnvironment,
    target_primitive: Option<PrimitiveType>,
    target_domain: ArithmeticDomain,
    owner: &str,
    infer_call_bodies: bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> CheckedAnalysis {
    let mut decisions = ArithmeticDecisions::default();
    let value = ExpressionWalk {
        program,
        machine,
        state,
        environment,
        target_primitive,
        target_domain,
        owner,
        infer_call_bodies,
        checks_trap_coverage: true,
        checks_occurrence_fit: true,
    }
    .analyze(expression, diagnostics, &mut decisions);
    CheckedAnalysis { value, decisions }
}

/// A carrier/policy meaning query over `expression` with no flow facts: the
/// walk an ordered-value operand or a relational range proof asks for the
/// primitive and arithmetic domain of a composite. It declines body inference
/// and trap coverage alike. Coverage would re-derive the machine's route
/// complement and re-decide formation under it, and that formation judgment
/// is what asked for this meaning in the first place (`proves_range` →
/// `ordered_values::operand` → here), so a coverage-checking query recursed
/// without bound on every guarded Trapping site. Ordinary validation still
/// owes every formation diagnostic, including coverage, on its own walk.
pub(super) fn analyze_meaning(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    owner: &str,
) -> CheckedAnalysis {
    let mut decisions = ArithmeticDecisions::default();
    let value = ExpressionWalk {
        program,
        machine,
        state,
        environment: &ValueEnvironment::new(),
        target_primitive: None,
        target_domain: ArithmeticDomain::Exact,
        owner,
        infer_call_bodies: false,
        checks_trap_coverage: false,
        checks_occurrence_fit: false,
    }
    .analyze(expression, &mut Vec::new(), &mut decisions);
    CheckedAnalysis { value, decisions }
}

/// The facts every node of one expression walk shares: where it is, what flow
/// facts hold, and which destination the enclosing statement names.
#[derive(Clone, Copy)]
struct ExpressionWalk<'a> {
    /// Meaning-only queries must not recursively execute the runtime-prefix
    /// proof owner. Only the validating walk publishes formation decisions.
    checks_occurrence_fit: bool,
    program: &'a dyn crate::CheckingInput,
    machine: &'a Machine,
    state: Option<&'a State>,
    environment: &'a ValueEnvironment,
    target_primitive: Option<PrimitiveType>,
    target_domain: ArithmeticDomain,
    owner: &'a str,
    /// Body inference captures one callee only. Descendant calls retain their
    /// declared guarantees without recursively deriving another body bound.
    /// Carrier-only queries also decline body inference: a range is not type
    /// selection authority, and their re-entry must not restart that capture.
    infer_call_bodies: bool,
    /// Whether a Trapping operation must prove its trap fits the machine's
    /// published Trap ceiling. A coverage re-analysis under the route
    /// complement only asks whether one node forms; its nested operations
    /// were already judged by the walk that reached them. A meaning query
    /// (`analyze_meaning`) declines it too, because coverage re-enters the
    /// range proof that issued the query.
    checks_trap_coverage: bool,
}

impl ExpressionWalk<'_> {
    /// The same walk under a different destination: an operand whose value is
    /// explicitly converted never sees the enclosing statement's target.
    fn with_destination(
        &self,
        target_primitive: Option<PrimitiveType>,
        target_domain: ArithmeticDomain,
    ) -> Self {
        Self {
            target_primitive,
            target_domain,
            ..*self
        }
    }

    fn analyze(
        &self,
        expression: ExpressionHandle,
        diagnostics: &mut Vec<Diagnostic>,
        decisions: &mut ArithmeticDecisions,
    ) -> Analysis {
        let node = self.program.expression_table.expression(expression);
        if !matches!(
            node,
            ExpressionNode::Binary(_)
                | ExpressionNode::Unary(_)
                | ExpressionNode::Cast(_)
                | ExpressionNode::Match(_)
        ) {
            // The parent's scalar bound says nothing about the formation of
            // its evaluated children: a known array length still evaluates its
            // receiver, and a field projection still constructs every sibling.
            // These edges do not inherit the enclosing scalar destination.
            // Field/element/formal landing checks own their respective types;
            // typed arithmetic retains its operand-selected carrier and policy.
            let child_walk = self.with_destination(None, ArithmeticDomain::Exact);
            let mut analyze_child = |child: ExpressionHandle| {
                if child.is_valid() {
                    child_walk.analyze(child, diagnostics, decisions);
                }
            };
            if matches!(node, ExpressionNode::Atomic(_)) {
                if let Some(operands) =
                    crate::value_custody::atomic_operations::atomic_value_operands(
                        self.program,
                        expression,
                    )
                {
                    for operand in operands {
                        analyze_child(operand);
                    }
                } else {
                    diagnostics.push(Diagnostic::error(format!(
                        "invalid atomic operation carrier in {}: cannot identify its evaluated operands",
                        self.owner,
                    )));
                }
            } else {
                crate::expression_traversal::children(self.program, node, analyze_child);
            }
        }
        let analysis = self.analyze_value(expression, node, diagnostics, decisions);
        decisions.expression_values.push(CheckedIntegerValue {
            expression,
            interval: analysis.interval,
            recipe: analysis.recipe.clone(),
        });
        analysis
    }

    /// The value judgment of one node whose children were already checked.
    fn analyze_value(
        &self,
        expression: ExpressionHandle,
        node: &ExpressionNode,
        diagnostics: &mut Vec<Diagnostic>,
        decisions: &mut ArithmeticDecisions,
    ) -> Analysis {
        if let Some(folded) = folded_values::analyze(self, expression, diagnostics) {
            return folded;
        }
        match node {
            ExpressionNode::Binary(binary) => {
                binary::analyze(self, expression, binary, diagnostics, decisions)
            }
            ExpressionNode::Unary(unary) => unary::analyze(self, unary, diagnostics, decisions),
            ExpressionNode::Cast(cast) => {
                cast::analyze(self, expression, cast, diagnostics, decisions)
            }
            ExpressionNode::Call(call) => call::analyze(self, expression, call),
            ExpressionNode::Match(dispatch) => {
                dispatch::analyze(self, expression, dispatch, diagnostics, decisions)
            }
            ExpressionNode::Integer(value) => Analysis {
                recipe: None,
                domain: None,
                interval: literal_interval(value),
                primitive: unsigned_constants::integer_literal_primitive(self.program, expression),
            },
            ExpressionNode::Float(_) | ExpressionNode::Boolean(_) => NEUTRAL,
            _ => place::analyze(self, expression),
        }
    }
}
