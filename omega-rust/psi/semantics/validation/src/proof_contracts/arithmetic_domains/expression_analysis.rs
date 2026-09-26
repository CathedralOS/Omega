//! Recursive arithmetic expression analysis.
//!
//! This module owns the operand-domain walk and interval result for one
//! expression. Declaration checks, flow-state ownership, and total-proposition
//! formation stay in their dedicated modules.
//!
//! [`analyze`] first asks whether the expression already has a value that does
//! not depend on its structure (`folded_values`), then dispatches on the node:
//! operators raise their domain and overflow obligations in `binary` and
//! `unary`, conversions in `cast`, call results take their declared or
//! inferred bounds in `call`, and places read their flow-tracked interval in
//! `place`.

mod binary;
mod call;
mod cast;
mod folded_values;
mod place;
mod unary;
mod unsigned_constants;

use super::{
    ArithmeticDomain, Diagnostic, ExpressionHandle, ExpressionNode, Interval, Machine,
    PrimitiveType, State, TypedTrees, ValueEnvironment, integer_ranges::literal_interval,
};

/// The result of analysing an expression for the domain + overflow rules.
pub(super) struct Analysis {
    /// The arithmetic domain (`None` = neutral: a literal or `bool` result).
    pub(super) domain: Option<ArithmeticDomain>,
    /// The value range, for the overflow proof obligation.
    pub(super) interval: Interval,
    /// The integer primitive type, for the overflow range bound (`None` when it
    /// cannot be determined, e.g. a bare literal).
    pub(super) primitive: Option<PrimitiveType>,
}

const NEUTRAL: Analysis = Analysis {
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
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    environment: &ValueEnvironment,
    target_primitive: Option<PrimitiveType>,
    target_domain: ArithmeticDomain,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Analysis {
    ExpressionWalk {
        program,
        machine,
        state,
        environment,
        target_primitive,
        target_domain,
        owner,
    }
    .analyze(expression, diagnostics)
}

/// The facts every node of one expression walk shares: where it is, what flow
/// facts hold, and which destination the enclosing statement names.
#[derive(Clone, Copy)]
struct ExpressionWalk<'a> {
    program: &'a TypedTrees,
    machine: &'a Machine,
    state: Option<&'a State>,
    environment: &'a ValueEnvironment,
    target_primitive: Option<PrimitiveType>,
    target_domain: ArithmeticDomain,
    owner: &'a str,
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

    fn analyze(&self, expression: ExpressionHandle, diagnostics: &mut Vec<Diagnostic>) -> Analysis {
        if let Some(folded) = folded_values::analyze(self, expression, diagnostics) {
            return folded;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Binary(binary) => {
                binary::analyze(self, expression, binary, diagnostics)
            }
            ExpressionNode::Unary(unary) => unary::analyze(self, unary, diagnostics),
            ExpressionNode::Cast(cast) => cast::analyze(self, cast, diagnostics),
            ExpressionNode::Call(call) => call::analyze(self, expression, call, diagnostics),
            ExpressionNode::Integer(value) => Analysis {
                domain: None,
                interval: literal_interval(value),
                primitive: unsigned_constants::integer_literal_primitive(self.program, expression),
            },
            ExpressionNode::Float(_) | ExpressionNode::Boolean(_) => NEUTRAL,
            _ => place::analyze(self, expression),
        }
    }
}
