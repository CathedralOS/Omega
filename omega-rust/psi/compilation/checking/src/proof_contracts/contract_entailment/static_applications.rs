//! Callable `where` facts under an already admitted, complete static tuple.
//!
//! Values bind exact declaration symbols simultaneously. The ordinary scoped
//! arithmetic judgment owns integer entailment; this adapter supplies Boolean
//! composition and canonical Boolean constants, not domain membership evidence.

use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
use numerics::bignum::BigInt;
use symbols::SymbolHandle;
use typed_trees::data::TypeParameterKind;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, StaticMachineArgument, UnaryOperator,
};
use typed_trees::signature::{SignatureContractKind, StateSignature};

use super::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticHypothesis, ScopedArithmeticValue, StrictArithmeticImplicationJudgment,
    scoped_arithmetic_implication_for_declaration,
};

/// Judge every expression fact on an exact applied callable signature.
///
/// The caller first admits the complete tuple's categories, carriers, bounds,
/// lifetimes and nominal authority through ordinary static argument admission.
/// `signature` retains its applied declaration scope; `lexical_owner` and prior
/// hypotheses retain the caller's scope. The lexical machine contributes only
/// declared Requires facts over immutable binders and value parameters. A
/// source call additionally supplies its exact containing state's Requires;
/// declaration-only applications have no state assumptions. Mutable runtime
/// subjects never supply this judgment. Forwarding uses exact symbols.
/// Non-expression facts and unsupported values answer `Unknown`, which is not
/// admission. This operation borrows frozen representations and mints no proof.
pub fn static_application_where_judgment(
    program: &dyn crate::CheckingInput,
    signature: &StateSignature,
    arguments: &[StaticMachineArgument],
    lexical_owner: SymbolHandle,
    caller_state: Option<&typed_trees::state::State>,
    hypotheses: &[ScopedArithmeticHypothesis],
) -> StrictArithmeticImplicationJudgment {
    use StrictArithmeticImplicationJudgment::{Proven, Unknown};

    let parameters = program.state_signature_type_parameters(signature);
    if !signature.symbol.is_valid()
        || !lexical_owner.is_valid()
        || parameters.len() != arguments.len()
    {
        return Unknown;
    }
    if let Some(state) = caller_state
        && !program
            .machine_by_symbol(lexical_owner)
            .is_some_and(|machine| {
                program
                    .machine_states(machine)
                    .iter()
                    .any(|candidate| candidate.symbol == state.symbol)
            })
    {
        return Unknown;
    }
    let (caller_bindings, mut prior_hypotheses) =
        immutable_caller_requires(program, lexical_owner, caller_state);
    prior_hypotheses.extend_from_slice(hypotheses);
    let mut integer_bindings = Vec::new();
    let mut boolean_bindings = Vec::new();
    for (position, (parameter, argument)) in parameters.iter().zip(arguments).enumerate() {
        if !parameter.symbol.is_valid()
            || parameters[..position]
                .iter()
                .any(|earlier| earlier.symbol == parameter.symbol)
        {
            return Unknown;
        }
        if !matches!(
            parameter.kind,
            TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. }
        ) {
            continue;
        }
        match constant_value(program, argument) {
            Some(ConstantValue::Integer(value)) => integer_bindings.push(ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::Symbol(parameter.symbol),
                value: ScopedArithmeticValue::Integer(value),
            }),
            Some(ConstantValue::Boolean(value)) => {
                boolean_bindings.push((parameter.symbol, value));
            }
            None => {
                // Preserve the caller's already established mathematical value.
                // A missing or conflicting roster supplies no symbolic proof.
                let mut supplied = None;
                for binding in caller_bindings.iter().chain(
                    prior_hypotheses
                        .iter()
                        .flat_map(|hypothesis| &hypothesis.proposition.bindings),
                ) {
                    if argument.symbol.is_valid()
                        && binding.binder == ScopedArithmeticBinder::Symbol(argument.symbol)
                    {
                        if supplied.is_some_and(|existing| existing != &binding.value) {
                            return Unknown;
                        }
                        supplied = Some(&binding.value);
                    }
                }
                if let Some(value) = supplied {
                    integer_bindings.push(ScopedArithmeticBinding {
                        binder: ScopedArithmeticBinder::Symbol(parameter.symbol),
                        value: value.clone(),
                    });
                }
            }
        }
    }
    let context = StaticFacts {
        program,
        signature,
        lexical_owner,
        hypotheses: &prior_hypotheses,
        integer_bindings,
        boolean_bindings,
    };
    let mut judgment = Proven;
    for fact in program.proof_facts.span_or_empty(signature.where_facts) {
        let next = match fact {
            ProofFact::Expression(expression) => {
                let mut remaining = 4096;
                context.judge_alternatives(*expression, prior_hypotheses.clone(), &mut remaining, 0)
            }
            _ => Unknown,
        };
        judgment = conjunction(judgment, next);
    }
    judgment
}

pub(super) enum ConstantValue {
    Integer(BigInt),
    Boolean(bool),
}

/// A Const or Value binder is immutable across every state and effect in an
/// invocation, even when a Value actual was supplied at runtime. Only its exact
/// lexical declaration can create its mathematical atom; a later assignment to
/// the caller's original storage cannot retag this captured argument.
/// Immutable data parameters are equally fixed for the invocation: each one
/// contributes a shared private atom so an ordinary runtime subject bound to
/// it can prove a fact the caller's Requires already establishes about that
/// parameter -- the same membership discharge the finite-family roster needs.
/// Mutable and non-integer parameters mint no atom; their value can change or
/// leave the integer language, and requires facts over them stay inadmissible.
fn immutable_caller_requires(
    program: &dyn crate::CheckingInput,
    owner: SymbolHandle,
    caller_state: Option<&typed_trees::state::State>,
) -> (
    Vec<ScopedArithmeticBinding>,
    Vec<ScopedArithmeticHypothesis>,
) {
    let Some(machine) = program.machine_by_symbol(owner) else {
        return (Vec::new(), Vec::new());
    };
    let mut symbols = Vec::new();
    let mut bindings = Vec::new();
    for parameter in program.machine_type_parameters(machine) {
        let (TypeParameterKind::Const { type_reference }
        | TypeParameterKind::Value { type_reference }) = parameter.kind
        else {
            continue;
        };
        if !parameter.symbol.is_valid() || symbols.contains(&parameter.symbol) {
            continue;
        }
        symbols.push(parameter.symbol);
        let Some(primitive) =
            crate::value_custody::recasts::exact_primitive_type(program, type_reference)
        else {
            continue;
        };
        if primitive.accepts_integer_literal() {
            bindings.push(ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::Symbol(parameter.symbol),
                value: ScopedArithmeticValue::Atom {
                    identity: format!("\0static-const:{:?}", parameter.symbol),
                    unsigned: !primitive.is_signed_integer(),
                },
            });
        }
    }
    for state in program.machine_states(machine) {
        for parameter in program
            .state_parameters
            .span_or_empty(state.parameters)
            .iter()
        {
            if parameter.is_mutable
                || !parameter.symbol.is_valid()
                || symbols.contains(&parameter.symbol)
            {
                continue;
            }
            let Some(primitive) = program
                .primitive_type_reference(parameter.type_reference)
                .filter(|primitive| primitive.is_fixed_integer())
            else {
                continue;
            };
            symbols.push(parameter.symbol);
            bindings.push(ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::Symbol(parameter.symbol),
                value: ScopedArithmeticValue::Atom {
                    identity: format!("\0runtime-subject:{:?}", parameter.symbol),
                    unsigned: !primitive.is_signed_integer(),
                },
            });
        }
    }
    let facts = crate::proof_contracts::proof_facts::contract_expression_facts(
        program,
        program.machine_contracts(machine).iter().chain(
            caller_state
                .into_iter()
                .flat_map(|state| program.state_contracts(state)),
        ),
        SignatureContractKind::Requires,
    );
    let hypotheses = facts
        .filter(|expression| immutable_fact(program, *expression, &symbols))
        .map(|expression| ScopedArithmeticHypothesis {
            proposition: ScopedArithmeticExpression {
                owner,
                expression,
                bindings: bindings.clone(),
            },
            holds: true,
        })
        .collect();
    (bindings, hypotheses)
}

fn immutable_fact(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    const_symbols: &[SymbolHandle],
) -> bool {
    let mut pending = vec![expression];
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if !program.expression_table.expression_is_valid(expression) {
            return false;
        }
        if visited.contains(&expression) {
            continue;
        }
        if visited.len() >= 4096 {
            return false;
        }
        visited.push(expression);
        let node = program.expression_table.expression(expression);
        match node {
            ExpressionNode::Name(path) => {
                let bound_const = path.symbol.is_valid()
                    && path.head_symbol == path.symbol
                    && program
                        .expression_table
                        .name_path_members(path.members)
                        .len()
                        == 1
                    && const_symbols.contains(&path.symbol);
                if !bound_const
                    && constant_value(
                        program,
                        &StaticMachineArgument {
                            symbol: path.symbol,
                            ..StaticMachineArgument::default()
                        },
                    )
                    .is_none()
                {
                    return false;
                }
            }
            // Even repeatable calls owe occurrence admission. A lexical owner
            // alone does not establish their preconditions or a live result.
            ExpressionNode::Call(_) | ExpressionNode::Atomic(_) | ExpressionNode::Borrow(_) => {
                return false;
            }
            _ => {}
        }
        crate::expression_traversal::children(program, node, |child| pending.push(child));
    }
    true
}

pub(super) fn constant_value(
    program: &dyn crate::CheckingInput,
    argument: &StaticMachineArgument,
) -> Option<ConstantValue> {
    // This representation-owned query reads evaluated declaration encodings
    // and canonical static atoms; arbitrary diagnostic paths are not values.
    let spelling = typed_trees::typed_trees::static_const_argument_spelling(
        program.typed_program(),
        argument,
    )?;
    if let Some(value) = CanonicalConstValue::from_atom(&spelling) {
        return match value.decode_encoding()? {
            DecodedCanonicalConstValue::Integer { value, .. } => {
                Some(ConstantValue::Integer(BigInt::from_i128(value)))
            }
            DecodedCanonicalConstValue::Boolean(value) => Some(ConstantValue::Boolean(value)),
            _ => None,
        };
    }
    spelling
        .parse::<i128>()
        .ok()
        .map(|value| ConstantValue::Integer(BigInt::from_i128(value)))
}

struct StaticFacts<'program> {
    program: &'program dyn crate::CheckingInput,
    signature: &'program StateSignature,
    lexical_owner: SymbolHandle,
    hypotheses: &'program [ScopedArithmeticHypothesis],
    integer_bindings: Vec<ScopedArithmeticBinding>,
    boolean_bindings: Vec<(SymbolHandle, bool)>,
}

impl StaticFacts<'_> {
    /// An asserted disjunction is two independent implication obligations, not
    /// a selected value or a union of facts. Every feasible alternative must
    /// prove the same goal. The fixed work budget fails closed on large trees.
    fn judge_alternatives(
        &self,
        expression: ExpressionHandle,
        mut hypotheses: Vec<ScopedArithmeticHypothesis>,
        remaining: &mut usize,
        depth: usize,
    ) -> StrictArithmeticImplicationJudgment {
        use StrictArithmeticImplicationJudgment::{Proven, Refuted, Unknown};
        if depth >= 128 {
            return Unknown;
        }
        let Some(next) = remaining.checked_sub(1) else {
            return Unknown;
        };
        *remaining = next;
        for position in 0..hypotheses.len() {
            if !hypotheses[position].holds {
                continue;
            }
            let Some(binary) = self
                .program
                .expression_table
                .binary_node(hypotheses[position].proposition.expression)
            else {
                continue;
            };
            match binary.operator {
                BinaryOperator::And => {
                    let mut right = hypotheses[position].clone();
                    hypotheses[position].proposition.expression = binary.left;
                    right.proposition.expression = binary.right;
                    hypotheses.push(right);
                    return self.judge_alternatives(expression, hypotheses, remaining, depth + 1);
                }
                BinaryOperator::Or => {
                    let mut alternative = hypotheses.clone();
                    hypotheses[position].proposition.expression = binary.left;
                    alternative[position].proposition.expression = binary.right;
                    return match (
                        self.judge_alternatives(expression, hypotheses, remaining, depth + 1),
                        self.judge_alternatives(expression, alternative, remaining, depth + 1),
                    ) {
                        (Proven, Proven) => Proven,
                        (Refuted, Refuted) => Refuted,
                        _ => Unknown,
                    };
                }
                _ => {}
            }
        }
        let scoped = StaticFacts {
            hypotheses: &hypotheses,
            integer_bindings: self.integer_bindings.clone(),
            boolean_bindings: self.boolean_bindings.clone(),
            ..*self
        };
        scoped.judge(expression, 0)
    }

    fn judge(
        &self,
        expression: ExpressionHandle,
        depth: usize,
    ) -> StrictArithmeticImplicationJudgment {
        use StrictArithmeticImplicationJudgment::{Proven, Refuted, Unknown};
        if depth >= 128 {
            return Unknown;
        }
        if let Some(value) = self.boolean_value(expression) {
            return if value { Proven } else { Refuted };
        }
        if let Some(unary) = self.program.expression_table.unary_node(expression)
            && unary.operator == UnaryOperator::LogicalNot
        {
            return match self.judge(unary.operand, depth + 1) {
                Proven => Refuted,
                Refuted => Proven,
                Unknown => Unknown,
            };
        }
        if let Some(binary) = self.program.expression_table.binary_node(expression) {
            match binary.operator {
                BinaryOperator::And => {
                    return conjunction(
                        self.judge(binary.left, depth + 1),
                        self.judge(binary.right, depth + 1),
                    );
                }
                BinaryOperator::Or => {
                    return match (
                        self.judge(binary.left, depth + 1),
                        self.judge(binary.right, depth + 1),
                    ) {
                        (Proven, _) | (_, Proven) => Proven,
                        (Refuted, Refuted) => Refuted,
                        _ => Unknown,
                    };
                }
                BinaryOperator::Equal | BinaryOperator::NotEqual => {
                    if let (Some(left), Some(right)) = (
                        self.boolean_value(binary.left),
                        self.boolean_value(binary.right),
                    ) {
                        if !super::structural_terms::has_identity_equality_meaning(
                            self.program,
                            self.signature.symbol,
                            self.program.state_signature_parameters(self.signature),
                            expression,
                        ) {
                            return Unknown;
                        }
                        let holds = (left == right) == (binary.operator == BinaryOperator::Equal);
                        return if holds { Proven } else { Refuted };
                    }
                }
                _ => {}
            }
        }
        scoped_arithmetic_implication_for_declaration(
            self.program,
            self.lexical_owner,
            self.hypotheses,
            &ScopedArithmeticExpression {
                owner: self.signature.symbol,
                expression,
                bindings: self.integer_bindings.clone(),
            },
        )
    }

    fn boolean_value(&self, expression: ExpressionHandle) -> Option<bool> {
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Boolean(value) => Some(*value),
            ExpressionNode::Name(path) => {
                if self
                    .program
                    .expression_table
                    .name_path_members(path.members)
                    .len()
                    != 1
                {
                    return None;
                }
                if let Some((_, value)) = self
                    .boolean_bindings
                    .iter()
                    .find(|(symbol, _)| *symbol == path.symbol)
                {
                    return Some(*value);
                }
                match constant_value(
                    self.program,
                    &StaticMachineArgument {
                        symbol: path.symbol,
                        ..StaticMachineArgument::default()
                    },
                )? {
                    ConstantValue::Boolean(value) => Some(value),
                    ConstantValue::Integer(_) => None,
                }
            }
            _ => None,
        }
    }
}

fn conjunction(
    left: StrictArithmeticImplicationJudgment,
    right: StrictArithmeticImplicationJudgment,
) -> StrictArithmeticImplicationJudgment {
    use StrictArithmeticImplicationJudgment::{Proven, Refuted, Unknown};
    match (left, right) {
        (Refuted, _) | (_, Refuted) => Refuted,
        (Proven, Proven) => Proven,
        _ => Unknown,
    }
}
