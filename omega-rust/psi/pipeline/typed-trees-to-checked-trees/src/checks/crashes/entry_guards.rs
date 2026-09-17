//! Incoming guards describe current values at an edge, whereas published
//! scalar crash routes bind invocation-entry operands.
//!
//! A retained guard fact covers an authored route only while each read in it
//! still evaluates to the entry operand its spelling claims. Admission
//! therefore resolves every binding-rooted place leaf through the shared
//! entry-provenance law in `facts::crash_entry_values` — the same law
//! call-actual substitution already uses — and keeps the leaf's conjunct
//! only when the proven entry operand equals the name-encoded claim. A
//! pristine mutable scalar read satisfies that check exactly while its
//! storage provably still holds the bound snapshot; a mutated storage read,
//! a same-name local shadow, or an unresolved alias does not. Leaves that
//! cannot claim an entry position at all — machine-rooted projections,
//! qualified names, constants — keep their existing opaque atoms.

use symbols::SymbolHandle;
use typed_trees::machine::Machine;
use typed_trees::statement::{StatementNode, TransitionGuardNode};
use typed_trees::{
    TypedTrees,
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator},
};

/// Where each `when` guard in `machine` was evaluated: the guard's own
/// transition statement. A guard edge carries its fact into joined and
/// continued states, so the state a guard applies to is not the state its
/// operand reads ran in — provenance has to answer for the evaluation site.
pub(super) fn guard_eval_sites(
    program: &TypedTrees,
    machine: &Machine,
) -> Vec<(ExpressionHandle, SymbolHandle, usize)> {
    let mut sites = Vec::new();
    for state in program.machine_states(machine) {
        for (ordinal, statement) in program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .enumerate()
        {
            if let StatementNode::Transition(transition) = statement
                && let TransitionGuardNode::When(guard) = transition.guard
            {
                sites.push((guard, state.symbol, ordinal));
            }
        }
    }
    sites
}

/// The guard conjuncts still entitled to speak for invocation-entry
/// operands, each with its negated polarity. A guard whose every sensitive
/// leaf proves its claimed operand keeps its whole normalized identity, so
/// a compound published route can still match it. Otherwise only the
/// individually clean top-level conjuncts survive: a spoiled leaf cannot
/// launder a sound sibling, and a sound sibling does not rescue a spoiled
/// one.
pub(super) fn entry_meaning_conjuncts(
    program: &TypedTrees,
    machine: &Machine,
    eval_sites: &[(ExpressionHandle, SymbolHandle, usize)],
    guard: ExpressionHandle,
    negated: bool,
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
) -> Vec<(ExpressionHandle, bool)> {
    let Some(eval_site) = eval_sites
        .iter()
        .find(|(handle, ..)| *handle == guard)
        .map(|(_, state, ordinal)| (*state, *ordinal))
    else {
        // A guard whose evaluation site cannot be located cannot prove any
        // binding-rooted read; nothing it names is admissible.
        return Vec::new();
    };
    let context = GuardContext {
        program,
        machine,
        eval_site,
        parameter_names,
        content_conservation,
    };
    if context.leaves_hold_entry_meaning(guard) {
        return vec![(guard, negated)];
    }
    let mut surviving = Vec::new();
    context.collect_clean_conjuncts(guard, negated, &mut surviving);
    surviving
}

#[derive(Clone, Copy)]
struct GuardContext<'a> {
    program: &'a TypedTrees,
    machine: &'a Machine,
    /// `(state, statement ordinal)` where the guard expression ran.
    eval_site: (SymbolHandle, usize),
    parameter_names: &'a [String],
    content_conservation: &'a [validation::ContentConservationSourcePlan],
}

impl GuardContext<'_> {
    /// Split the top-level conjunctive structure the same way the
    /// consequence collectors do — positive `&&`, negated `||`, and `!`
    /// polarity — and keep each conjunct only while every sensitive leaf in
    /// it still holds its claimed entry operand.
    fn collect_clean_conjuncts(
        &self,
        expression: ExpressionHandle,
        negated: bool,
        output: &mut Vec<(ExpressionHandle, bool)>,
    ) {
        if !self
            .program
            .expression_table
            .expression_is_valid(expression)
        {
            return;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                self.collect_clean_conjuncts(unary.operand, !negated, output)
            }
            ExpressionNode::Binary(binary)
                if (!negated && binary.operator == BinaryOperator::And)
                    || (negated && binary.operator == BinaryOperator::Or) =>
            {
                self.collect_clean_conjuncts(binary.left, negated, output);
                self.collect_clean_conjuncts(binary.right, negated, output);
            }
            ExpressionNode::Binary(binary)
                if matches!(
                    binary.operator,
                    BinaryOperator::Equal | BinaryOperator::NotEqual
                ) =>
            {
                // `x == true` and `x != false` establish `x`; `x == false`
                // and `x != true` establish `!x`. Peel the normalization so
                // one spoiled operand cannot discard a sound sibling — the
                // surviving conjunct is checked again on its own leaves.
                let operand_and_literal = match (
                    self.program.expression_table.expression(binary.left),
                    self.program.expression_table.expression(binary.right),
                ) {
                    (ExpressionNode::Boolean(literal), _) => Some((binary.right, *literal)),
                    (_, ExpressionNode::Boolean(literal)) => Some((binary.left, *literal)),
                    _ => None,
                };
                match operand_and_literal {
                    Some((operand, literal)) => {
                        let equality_is_negated = if binary.operator == BinaryOperator::Equal {
                            negated
                        } else {
                            !negated
                        };
                        self.collect_clean_conjuncts(
                            operand,
                            equality_is_negated == literal,
                            output,
                        );
                    }
                    None if self.leaves_hold_entry_meaning(expression) => {
                        output.push((expression, negated));
                    }
                    None => {}
                }
            }
            _ if self.leaves_hold_entry_meaning(expression) => {
                output.push((expression, negated));
            }
            _ => {}
        }
    }

    /// Every place expression in this subtree either cannot claim an entry
    /// operand or proves the exact operand its spelling encodes.
    fn leaves_hold_entry_meaning(&self, root: ExpressionHandle) -> bool {
        let mut pending = vec![root];
        let mut seen = Vec::new();
        while let Some(expression) = pending.pop() {
            if !self
                .program
                .expression_table
                .expression_is_valid(expression)
            {
                return false;
            }
            if seen.contains(&expression) {
                continue;
            }
            seen.push(expression);
            match self.program.expression_table.expression(expression) {
                ExpressionNode::Match(dispatch) => {
                    pending.push(dispatch.subject);
                    for arm in self.program.expression_table.match_arms(dispatch.arms) {
                        if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                            pending.push(pattern);
                        }
                        pending.push(arm.value);
                    }
                }
                ExpressionNode::Name(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Indexed(_) => {
                    if !self.place_leaf_holds_entry_meaning(expression, &mut pending) {
                        return false;
                    }
                }
                ExpressionNode::Binary(binary) => pending.extend([binary.left, binary.right]),
                ExpressionNode::Unary(unary) => pending.push(unary.operand),
                ExpressionNode::Borrow(borrow) => pending.push(borrow.target),
                ExpressionNode::Cast(cast) => pending.push(cast.value),
                ExpressionNode::Range(range) => pending.extend(
                    [range.start, range.end]
                        .into_iter()
                        .filter(|value| value.is_valid()),
                ),
                ExpressionNode::Atomic(atomic) => pending.extend(
                    [atomic.value, atomic.result]
                        .into_iter()
                        .filter(|value| value.is_valid()),
                ),
                ExpressionNode::Call(call) => {
                    if call.receiver.is_valid() {
                        pending.push(call.receiver);
                    }
                    pending.extend(
                        self.program
                            .expression_table
                            .expression_handles(call.arguments),
                    );
                }
                ExpressionNode::ArrayLiteral(elements) => {
                    pending.extend(self.program.expression_table.expression_handles(*elements))
                }
                ExpressionNode::StructLiteral(literal) => pending.extend(
                    self.program
                        .expression_table
                        .struct_fields(literal.fields)
                        .iter()
                        .map(|field| field.value),
                ),
                ExpressionNode::Boolean(_)
                | ExpressionNode::Float(_)
                | ExpressionNode::Integer(_)
                | ExpressionNode::String(_)
                | ExpressionNode::ZeroValue(_) => {}
            }
        }
        true
    }

    /// One maximal place expression. Index operands and a non-place base
    /// (a call whose result is projected, for example) are their own reads
    /// and are pushed back onto the walk; the leaf itself is checked once,
    /// against the binding its chain resolves to.
    fn place_leaf_holds_entry_meaning(
        &self,
        leaf: ExpressionHandle,
        pending: &mut Vec<ExpressionHandle>,
    ) -> bool {
        let mut base = leaf;
        loop {
            if !self.program.expression_table.expression_is_valid(base) {
                return false;
            }
            match self.program.expression_table.expression(base) {
                ExpressionNode::Member(member) => base = member.receiver,
                ExpressionNode::Indexed(indexed) => {
                    pending.push(indexed.index);
                    base = indexed.collection;
                }
                ExpressionNode::Borrow(borrow) => base = borrow.target,
                _ => break,
            }
        }
        let ExpressionNode::Name(path) = self.program.expression_table.expression(base) else {
            // The place is rooted at a computed base such as a call result;
            // that base is a read of its own and is visited separately.
            pending.push(base);
            return true;
        };
        let members = self
            .program
            .expression_table
            .name_path_members(path.members);
        let root_symbol = if members.len() == 1 {
            path.symbol
        } else {
            path.head_symbol
        };
        let Some(binding) = self.root_binding(root_symbol) else {
            return true;
        };
        if !binding.sensitive {
            return true;
        }
        let proven = crate::facts::crash_entry_operand(
            self.program,
            self.machine.symbol,
            self.eval_site.0,
            self.eval_site.1,
            leaf,
        )
        .map(checked_trees::CrashPredicateIdentity::from_expression);
        proven
            == Some(crate::facts::canonical_crash_path_predicate(
                self.program,
                leaf,
                false,
                self.parameter_names,
                self.content_conservation,
            ))
    }

    /// The state parameter or already-declared local `symbol` names at the
    /// evaluation site. A leaf claims an entry operand whenever the binding
    /// can change underfoot (mutable storage) or its spelling collides with
    /// an entry parameter name, since the name encoding would claim that
    /// parameter position. An immutable non-colliding binding encodes an
    /// inert name atom and needs no provenance.
    fn root_binding(&self, symbol: SymbolHandle) -> Option<RootBinding> {
        if !symbol.is_valid() {
            return None;
        }
        let (state_symbol, before_statement) = self.eval_site;
        let state = self
            .program
            .machine_states(self.machine)
            .iter()
            .find(|state| state.symbol == state_symbol)?;
        for statement in self
            .program
            .statement_table
            .statements(state.statement_nodes)
            .get(..before_statement)
            .unwrap_or(&[])
        {
            if let StatementNode::LocalData(local) = statement
                && local.symbol == symbol
            {
                return Some(RootBinding {
                    sensitive: local.is_mutable
                        || self
                            .parameter_names
                            .iter()
                            .any(|name| name == local.name.as_str()),
                });
            }
        }
        self.program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.symbol == symbol && !parameter.is_self)
            .map(|parameter| RootBinding {
                sensitive: parameter.is_mutable
                    || self
                        .parameter_names
                        .iter()
                        .any(|name| name == parameter.name.as_str()),
            })
    }
}

struct RootBinding {
    sensitive: bool,
}
