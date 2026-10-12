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
    scoped_arithmetic_implication,
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
    let parameters = program
        .state_signature_type_parameters(signature)
        .iter()
        .collect::<Vec<_>>();
    callable_where_judgment(
        program,
        signature.symbol,
        &parameters,
        program.state_signature_parameters(signature),
        signature.where_facts,
        arguments,
        lexical_owner,
        caller_state,
        hypotheses,
    )
}

/// One judgment for requirement and ordinary machine predicate storage. The
/// caller supplies the authentic complete declaration-order telescope; no
/// copied signature, name lookup or partial invocation tuple establishes it.
#[allow(clippy::too_many_arguments)]
pub fn callable_where_judgment(
    program: &dyn crate::CheckingInput,
    declaration: SymbolHandle,
    parameters: &[&typed_trees::data::TypeParameter],
    runtime_parameters: &[typed_trees::signature::StateParameter],
    where_facts: arena::HandleSpan<ProofFact>,
    arguments: &[StaticMachineArgument],
    lexical_owner: SymbolHandle,
    caller_state: Option<&typed_trees::state::State>,
    hypotheses: &[ScopedArithmeticHypothesis],
) -> StrictArithmeticImplicationJudgment {
    use StrictArithmeticImplicationJudgment::{Proven, Unknown};
    if !declaration.is_valid() || !lexical_owner.is_valid() || parameters.len() != arguments.len() {
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
    let (caller_bindings, caller_booleans, mut prior_hypotheses) =
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
                boolean_bindings.push((parameter.symbol, BooleanValue::Known(value)));
            }
            None => {
                if let Some((_, value)) = caller_booleans
                    .iter()
                    .find(|(symbol, _)| *symbol == argument.symbol)
                {
                    boolean_bindings.push((parameter.symbol, *value));
                }
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
        declaration,
        runtime_parameters,
        lexical_owner,
        hypotheses: &prior_hypotheses,
        integer_bindings,
        boolean_bindings,
        hypothesis_booleans: caller_booleans,
    };
    let mut judgment = Proven;
    for fact in program.proof_facts.span_or_empty(where_facts) {
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

/// Public predicates must entail a provider's declaration predicates under
/// exact source binder correspondence. The provider's own predicates are
/// goals, never assumed caller facts. Unsupported proof vocabulary is Unknown.
#[allow(clippy::too_many_arguments)]
pub fn callable_where_refinement_judgment(
    program: &dyn crate::CheckingInput,
    declaration: SymbolHandle,
    parameters: &[&typed_trees::data::TypeParameter],
    runtime_parameters: &[typed_trees::signature::StateParameter],
    where_facts: arena::HandleSpan<ProofFact>,
    required: &StateSignature,
    correspondence: &[(SymbolHandle, SymbolHandle)],
) -> StrictArithmeticImplicationJudgment {
    use StrictArithmeticImplicationJudgment::{Proven, Unknown};
    let mut bindings = Vec::new();
    let mut boolean_bindings = Vec::new();
    for parameter in parameters {
        let TypeParameterKind::Const { type_reference } = parameter.kind else {
            continue;
        };
        let Some(primitive) =
            crate::value_custody::recasts::exact_primitive_type(program, type_reference)
        else {
            continue;
        };
        if primitive == typed_trees::types::PrimitiveType::Bool {
            boolean_bindings.push((parameter.symbol, BooleanValue::Symbol(parameter.symbol)));
        }
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
    let mut required_bindings = bindings.clone();
    let mut required_booleans = boolean_bindings.clone();
    for (required, actual) in correspondence {
        if let Some((_, value)) = boolean_bindings
            .iter()
            .find(|(symbol, _)| symbol == actual)
            .copied()
        {
            required_booleans.push((*required, value));
        }
        if let Some(binding) = bindings
            .iter()
            .find(|binding| binding.binder == ScopedArithmeticBinder::Symbol(*actual))
        {
            required_bindings.push(ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::Symbol(*required),
                value: binding.value.clone(),
            });
        }
    }
    let mut hypotheses = Vec::new();
    for fact in program.proof_facts.span_or_empty(required.where_facts) {
        let Some(expression) = fact.as_expression() else {
            return Unknown;
        };
        hypotheses.push(ScopedArithmeticHypothesis {
            proposition: ScopedArithmeticExpression {
                owner: required.symbol,
                expression: *expression,
                bindings: required_bindings.clone(),
            },
            holds: true,
        });
    }
    let immutable_symbols = parameters
        .iter()
        .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Const { .. }))
        .map(|parameter| parameter.symbol)
        .chain(
            program
                .state_signature_type_parameters(required)
                .iter()
                .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Const { .. }))
                .map(|parameter| parameter.symbol),
        )
        .collect::<Vec<_>>();
    for expression in crate::proof_contracts::proof_facts::contract_expression_facts(
        program,
        program.state_signature_contracts(required).iter(),
        SignatureContractKind::Requires,
    )
    .filter(|expression| immutable_fact(program, *expression, &immutable_symbols))
    {
        hypotheses.push(ScopedArithmeticHypothesis {
            proposition: ScopedArithmeticExpression {
                owner: required.symbol,
                expression,
                bindings: required_bindings.clone(),
            },
            holds: true,
        });
    }
    let context = StaticFacts {
        program,
        declaration,
        runtime_parameters,
        lexical_owner: declaration,
        hypotheses: &hypotheses,
        integer_bindings: bindings,
        boolean_bindings,
        hypothesis_booleans: required_booleans,
    };
    let mut judgment = Proven;
    for fact in program.proof_facts.span_or_empty(where_facts) {
        let next = match fact {
            ProofFact::Expression(expression) => {
                let mut remaining = 4096;
                context.judge_alternatives(*expression, hypotheses.clone(), &mut remaining, 0)
            }
            _ => Unknown,
        };
        judgment = conjunction(judgment, next);
    }
    judgment
}

enum ConstantValue {
    Integer(BigInt),
    Boolean(bool),
}

/// A static binder is immutable across every state and effect in an invocation.
/// Only its exact lexical declaration can create its mathematical atom.
/// Immutable data parameters are equally fixed for the invocation: each one
/// contributes a shared private atom so an ordinary runtime subject bound to
/// it can prove a fact the caller's Requires already establishes about that
/// parameter -- the same membership discharge the finite-family roster needs.
/// Immutable integer and Boolean subjects retain distinct symbolic atoms.
/// Mutable parameters and other carriers supply no predicate authority.
fn immutable_caller_requires(
    program: &dyn crate::CheckingInput,
    owner: SymbolHandle,
    caller_state: Option<&typed_trees::state::State>,
) -> (
    Vec<ScopedArithmeticBinding>,
    Vec<(SymbolHandle, BooleanValue)>,
    Vec<ScopedArithmeticHypothesis>,
) {
    let Some(machine) = program.machine_by_symbol(owner) else {
        return (Vec::new(), Vec::new(), Vec::new());
    };
    let mut symbols = Vec::new();
    let mut bindings = Vec::new();
    let mut booleans = Vec::new();
    let Ok(declaration) =
        typed_trees::static_applications::StaticApplicationDeclaration::for_symbol(
            program.typed_program(),
            machine.symbol,
        )
    else {
        return (Vec::new(), Vec::new(), Vec::new());
    };
    for parameter in declaration.parameters() {
        let TypeParameterKind::Const { type_reference } = parameter.kind else {
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
        if primitive == typed_trees::types::PrimitiveType::Bool {
            booleans.push((parameter.symbol, BooleanValue::Symbol(parameter.symbol)));
        }
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
                .filter(|primitive| {
                    primitive.is_fixed_integer()
                        || *primitive == typed_trees::types::PrimitiveType::Bool
                })
            else {
                continue;
            };
            symbols.push(parameter.symbol);
            if primitive == typed_trees::types::PrimitiveType::Bool {
                booleans.push((parameter.symbol, BooleanValue::Symbol(parameter.symbol)));
                continue;
            }
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
    let where_facts = program
        .proof_facts
        .span_or_empty(machine.where_facts)
        .iter()
        .filter_map(|fact| fact.as_expression().copied());
    let hypotheses = facts
        .chain(where_facts)
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
    (bindings, booleans, hypotheses)
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

fn constant_value(
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

/// Boolean atoms retain authentic binder correspondence, not a finite-carrier
/// enumeration. Hypotheses can establish one atom without normalizing another
/// declaration's name or borrowing a representative concrete application.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BooleanValue {
    Known(bool),
    Symbol(SymbolHandle),
}

/// An equality proposition retains both authenticated Boolean atoms. Its
/// symmetry is logical equality, never equality of unrelated source handles.
#[derive(Clone, Copy)]
enum BooleanProposition {
    Atom(SymbolHandle),
    Equal(SymbolHandle, SymbolHandle),
}

impl BooleanProposition {
    fn same_proposition(self, other: Self) -> bool {
        match (self, other) {
            (Self::Atom(left), Self::Atom(right)) => left == right,
            (Self::Equal(left, right), Self::Equal(other_left, other_right)) => {
                (left == other_left && right == other_right)
                    || (left == other_right && right == other_left)
            }
            _ => false,
        }
    }
}

struct StaticFacts<'program> {
    program: &'program dyn crate::CheckingInput,
    declaration: SymbolHandle,
    runtime_parameters: &'program [typed_trees::signature::StateParameter],
    lexical_owner: SymbolHandle,
    hypotheses: &'program [ScopedArithmeticHypothesis],
    integer_bindings: Vec<ScopedArithmeticBinding>,
    boolean_bindings: Vec<(SymbolHandle, BooleanValue)>,
    // Hypotheses retain the caller/public scope independently from this
    // invocation goal, including recursive calls to the same declaration.
    hypothesis_booleans: Vec<(SymbolHandle, BooleanValue)>,
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
            hypothesis_booleans: self.hypothesis_booleans.clone(),
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
        if let Some(BooleanValue::Known(value)) = self.boolean_value(expression) {
            return if value { Proven } else { Refuted };
        }
        if let Some((symbol, holds)) =
            self.boolean_literal(expression, self.declaration, &self.boolean_bindings)
        {
            for hypothesis in self.hypotheses {
                if let Some((prior, mut prior_holds)) = self.boolean_literal(
                    hypothesis.proposition.expression,
                    hypothesis.proposition.owner,
                    &self.hypothesis_booleans,
                ) && prior.same_proposition(symbol)
                {
                    if !hypothesis.holds {
                        prior_holds = !prior_holds;
                    }
                    return if prior_holds == holds {
                        Proven
                    } else {
                        Refuted
                    };
                }
            }
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
                    if let (Some(left), Some(right)) = (
                        self.boolean_literal(binary.left, self.declaration, &self.boolean_bindings),
                        self.boolean_literal(
                            binary.right,
                            self.declaration,
                            &self.boolean_bindings,
                        ),
                    ) && left.0.same_proposition(right.0)
                        && left.1 != right.1
                    {
                        return Refuted;
                    }
                    return conjunction(
                        self.judge(binary.left, depth + 1),
                        self.judge(binary.right, depth + 1),
                    );
                }
                BinaryOperator::Or => {
                    if let (Some(left), Some(right)) = (
                        self.boolean_literal(binary.left, self.declaration, &self.boolean_bindings),
                        self.boolean_literal(
                            binary.right,
                            self.declaration,
                            &self.boolean_bindings,
                        ),
                    ) && left.0.same_proposition(right.0)
                        && left.1 != right.1
                    {
                        return Proven;
                    }
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
                            self.declaration,
                            self.runtime_parameters,
                            expression,
                        ) {
                            return Unknown;
                        }
                        if matches!(
                            (left, right),
                            (BooleanValue::Known(_), BooleanValue::Known(_))
                        ) || left == right
                        {
                            let holds =
                                (left == right) == (binary.operator == BinaryOperator::Equal);
                            return if holds { Proven } else { Refuted };
                        }
                    }
                }
                _ => {}
            }
        }
        scoped_arithmetic_implication(
            self.program,
            self.lexical_owner,
            self.hypotheses,
            &ScopedArithmeticExpression {
                owner: self.declaration,
                expression,
                bindings: self.integer_bindings.clone(),
            },
        )
    }

    fn boolean_literal(
        &self,
        expression: ExpressionHandle,
        owner: SymbolHandle,
        bindings: &[(SymbolHandle, BooleanValue)],
    ) -> Option<(BooleanProposition, bool)> {
        if let Some(BooleanValue::Symbol(symbol)) = self.boolean_value_in(expression, bindings) {
            return Some((BooleanProposition::Atom(symbol), true));
        }
        if let Some(unary) = self.program.expression_table.unary_node(expression)
            && unary.operator == UnaryOperator::LogicalNot
        {
            let (symbol, holds) = self.boolean_literal(unary.operand, owner, bindings)?;
            return Some((symbol, !holds));
        }
        let binary = self.program.expression_table.binary_node(expression)?;
        if !matches!(
            binary.operator,
            BinaryOperator::Equal | BinaryOperator::NotEqual
        ) || !super::structural_terms::has_identity_equality_meaning(
            self.program,
            owner,
            &[],
            expression,
        ) {
            return None;
        }
        let pair = (
            self.boolean_value_in(binary.left, bindings)?,
            self.boolean_value_in(binary.right, bindings)?,
        );
        let (proposition, value) = match pair {
            (BooleanValue::Symbol(symbol), BooleanValue::Known(value))
            | (BooleanValue::Known(value), BooleanValue::Symbol(symbol)) => {
                (BooleanProposition::Atom(symbol), value)
            }
            (BooleanValue::Symbol(left), BooleanValue::Symbol(right)) if left != right => {
                (BooleanProposition::Equal(left, right), true)
            }
            _ => return None,
        };
        Some((
            proposition,
            if binary.operator == BinaryOperator::Equal {
                value
            } else {
                !value
            },
        ))
    }

    fn boolean_value(&self, expression: ExpressionHandle) -> Option<BooleanValue> {
        self.boolean_value_in(expression, &self.boolean_bindings)
    }

    fn boolean_value_in(
        &self,
        expression: ExpressionHandle,
        bindings: &[(SymbolHandle, BooleanValue)],
    ) -> Option<BooleanValue> {
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Boolean(value) => Some(BooleanValue::Known(*value)),
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
                if let Some((_, value)) = bindings.iter().find(|(symbol, _)| *symbol == path.symbol)
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
                    ConstantValue::Boolean(value) => Some(BooleanValue::Known(value)),
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
