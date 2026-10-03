//! Scalar contract predicates share logical structure and exact entry/result
//! identities. They are not body expressions: mutable entry snapshots are
//! available to requires, while normal guarantees cannot reread them as old
//! values. Integer leaves keep their contextual landing and operator owner.
use crate::values::operator_is_builtin;
use crate::values::scalar::boolean_lowering::construct_integer_comparison;
use crate::values::scalar::contract_entry;
use crate::values::scalar::expression_facts::is_integer;
use crate::values::scalar::scalar_lowering::{construct_integer_binary, lower_return_expression};
use crate::values::scalar_expression_type;
use checked_trees::CheckedOperatorFacts;
use checked_trees::CheckedOperatorResolutionStatus;
use checked_trees::CheckedScalarExpression;
use checked_trees::{
    CheckedBooleanExpression, CheckedIntegerComparisonKind, CheckedIntegerMathExpression,
    CheckedIntegerMathTerm,
};
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::TypedTrees;
use typed_trees::expression::BinaryOperator;
use typed_trees::expression::ExpressionHandle;
use typed_trees::expression::ExpressionNode;
use typed_trees::expression::UnaryOperator;
use typed_trees::signature::StateParameter;
use typed_trees::types::PrimitiveType;
use typed_trees::types::TypeReferenceHandle;
use typed_trees::types::TypeReferenceNode;

/// The caller supplies a predicate from this machine's exact contract clause.
/// Entry scalar parameters precede the reserved ensures-only result position.
/// The caller owns the shared work budget, including unsuccessful reads.
pub(crate) fn lower_scalar_contract_predicate(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    machine: &typed_trees::machine::Machine,
    expression: ExpressionHandle,
    allow_result: bool,
    remaining: &mut usize,
) -> Option<CheckedBooleanExpression> {
    ContractPredicates {
        program,
        operators,
        machine,
        parameters: contract_entry::authored_entry_parameters(program, machine)?,
        allow_result,
        remaining,
    }
    .boolean(expression, 0)
}

/// One authored `requires` predicate on a non-entry state, lowered into the
/// closed scalar contract namespace. The state's authored parameter roster
/// indexes the subjects; no result position exists on an arrival contract.
/// The machine still owns the builtin-meaning checks inside the predicate.
pub(crate) fn lower_state_scalar_contract_predicate(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
    remaining: &mut usize,
) -> Option<CheckedBooleanExpression> {
    ContractPredicates {
        program,
        operators,
        machine,
        parameters: program.state_parameters(state),
        allow_result: false,
        remaining,
    }
    .boolean(expression, 0)
}

struct ContractPredicates<'program, 'budget> {
    program: &'program TypedTrees,
    operators: &'program CheckedOperatorFacts,
    machine: &'program typed_trees::machine::Machine,
    parameters: &'program [StateParameter],
    allow_result: bool,
    remaining: &'budget mut usize,
}

impl ContractPredicates<'_, '_> {
    fn subject(&self, expression: ExpressionHandle) -> Option<(usize, TypeReferenceHandle, bool)> {
        let program = self.program;
        let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
            return None;
        };
        if let Some((position, parameter)) =
            self.parameters.iter().enumerate().find(|(_, parameter)| {
                parameter.symbol == path.symbol && path.head_symbol == parameter.symbol
            })
        {
            if parameter.is_self
                || parameter.is_const
                || !matches!(program.expression_table.name_path_members(path.members), [name] if name == &parameter.name)
            {
                return None;
            }
            // An ensures name denotes the post-state, not an implicit old(...).
            if self.allow_result && parameter.is_mutable {
                return None;
            }
            program.primitive_type_reference(parameter.type_reference)?;
            // Erased formals index the proof-only roster in authored order.
            if parameter.relevance.is_erased() {
                let erased_position = self.parameters[..position]
                    .iter()
                    .filter(|parameter| {
                        parameter.relevance.is_erased()
                            && program
                                .primitive_type_reference(parameter.type_reference)
                                .is_some()
                    })
                    .count();
                return Some((erased_position, parameter.type_reference, true));
            }
            let scalar_position = self.parameters[..position]
                .iter()
                .filter(|parameter| {
                    crate::values::scalar::occupies_scalar_position(program, parameter)
                })
                .count();
            return Some((scalar_position, parameter.type_reference, false));
        }
        // Equal spelling or carrier does not establish result ownership. This
        // occurrence must belong to this machine's exact authored ensures.
        let entry = program.machine_states(self.machine).first()?;
        (self.allow_result
            && validation::reserved_result_owner(program, expression)
                == Some((self.machine.symbol, entry.return_type)))
        .then(|| {
            (
                self.parameters
                    .iter()
                    .filter(|parameter| {
                        crate::values::scalar::occupies_scalar_position(program, parameter)
                    })
                    .count(),
                entry.return_type,
                false,
            )
        })
    }

    /// One comparison operand as a typed scalar term: an exact entry/result
    /// subject, or integer arithmetic (`+`, `-`, `*` with builtin meaning)
    /// over such subjects and contextual integer literals. The operands'
    /// arithmetic policies select the operation as they do in a body, so an
    /// Exact, Wrapping or Saturating carrier keeps its own meaning; Trapping
    /// arithmetic, which can crash, or any other spelling stays outside the
    /// closed language and lowers to `None`.
    fn integer_term(
        &self,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Option<(CheckedScalarExpression, TypeReferenceHandle)> {
        let program = self.program;
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(_) => {
                let (position, reference, erased) = self.subject(expression)?;
                let primitive_type = program.primitive_type_reference(reference)?;
                Some((
                    if erased {
                        CheckedScalarExpression::ErasedParameter {
                            position,
                            primitive_type,
                        }
                    } else {
                        CheckedScalarExpression::Parameter {
                            position,
                            primitive_type,
                        }
                    },
                    reference,
                ))
            }
            ExpressionNode::Binary(binary)
                if depth < 64 && operator_is_builtin(self.operators, expression) =>
            {
                let spelling = match binary.operator {
                    BinaryOperator::Add => language_core::OperatorSpelling::Add,
                    BinaryOperator::Subtract => language_core::OperatorSpelling::Subtract,
                    BinaryOperator::Multiply => language_core::OperatorSpelling::Multiply,
                    _ => return None,
                };
                let terms = [binary.left, binary.right]
                    .map(|operand| self.integer_term(operand, depth + 1));
                // The typed peer supplies the carrier; a literal peer lands on
                // it. Two literals have no carrier and stay unsupported here.
                let (carrier_type, carrier_reference) =
                    terms.iter().flatten().find_map(|(term, reference)| {
                        Some((scalar_expression_type(term)?, *reference))
                    })?;
                if !is_integer(carrier_type) {
                    return None;
                }
                let operand_types = [Some(carrier_reference), Some(carrier_reference)];
                if !typed_trees::operator::has_builtin_spelled_expression_meaning(
                    program,
                    self.machine.symbol,
                    expression,
                    spelling,
                    &operand_types,
                ) {
                    return None;
                }
                let [left, right] = [
                    (binary.left, terms[0].clone()),
                    (binary.right, terms[1].clone()),
                ]
                .map(|(operand, term)| match term {
                    Some((term, reference)) => {
                        (scalar_expression_type(&term) == Some(carrier_type)).then(|| {
                            (
                                term,
                                program.arithmetic_domain_for_type_reference(reference),
                                reference,
                            )
                        })
                    }
                    None => lower_contract_integer_literal(
                        program,
                        self.operators,
                        operand,
                        carrier_type,
                    )
                    .map(|literal| (literal, ArithmeticDomain::Exact, carrier_reference)),
                });
                let (left, left_domain, left_reference) = left?;
                let (right, right_domain, right_reference) = right?;
                // A contract term selects the same builtin operation a body
                // expression over these carriers would: the operands' policies
                // combine through the shared scalar semantics, so a Wrapping
                // field subtracts modulo its width here exactly as it does
                // when the body stores it.
                let (term, domain) = construct_integer_binary(
                    binary.operator,
                    left,
                    left_domain,
                    right,
                    right_domain,
                )?;
                // A contract term is a total observation. Trapping arithmetic
                // owns a crash site at its operation and has no proposition.
                if matches!(
                    &term,
                    CheckedScalarExpression::IntegerBinary { kind, .. } if kind.is_trapping()
                ) {
                    return None;
                }
                // Keep the operand reference that carries the selected policy
                // so enclosing arithmetic composes with the same meaning.
                let reference =
                    if program.arithmetic_domain_for_type_reference(left_reference) == domain {
                        left_reference
                    } else {
                        right_reference
                    };
                Some((term, reference))
            }
            ExpressionNode::Member(_) => {
                // `subject.len` reads the live extent of a borrowed view or
                // bounded byte field parameter in the same closed namespace
                // the body's scalar lowering publishes. Its carrier is the
                // builtin u64 exact type.
                if let Some(term) = super::structural_fields::structural_sequence_length(
                    program,
                    self.parameters,
                    expression,
                ) {
                    // Only a whole-parameter extent is admissible in the closed
                    // contract namespace: a nested path needs canonical field ids
                    // the namespace does not mint, and a view-local root resolves
                    // no retained place here. Both stay erased as before.
                    if let CheckedScalarExpression::StructuralParameterByteLength { root, path } =
                        &term
                    {
                        let admissible =
                            matches!(root, checked_trees::CheckedStorageRoot::Parameter { .. })
                                && path.is_empty();
                        if !admissible {
                            return None;
                        }
                    }
                    let reference = program.type_reference_table.named_references().find_map(
                        |(reference, symbol, _)| {
                            (program.symbols.builtin_type_atom(symbol)
                                == Some(symbols::BuiltinTypeAtom::U64))
                            .then_some(reference)
                        },
                    )?;
                    return Some((term, reference));
                }
                // A scalar member read (`self.rows`, `picker.picks`) names
                // statically selected field storage below one structural
                // parameter, so the contract operand resolves against its
                // declaration at that parameter's authored position.
                let (position, _, reference) =
                    super::structural_fields::structural_parameter_place(
                        program,
                        self.parameters,
                        expression,
                    )?;
                if !self
                    .parameters
                    .get(position as usize)
                    .is_some_and(|parameter| caller_storage_root(program, parameter))
                {
                    return None;
                }
                let (term, _) = super::structural_fields::lower_structural_parameter_field(
                    program,
                    self.parameters,
                    expression,
                )?;
                let CheckedScalarExpression::StructuralParameterField { .. } = term else {
                    return None;
                };
                // Reading a field for comparison preserves its carrier value;
                // its arithmetic policy constrains operations, not observation.
                Some((term, reference))
            }
            _ => None,
        }
    }

    fn integer_math_expression(
        &mut self,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Option<CheckedIntegerMathExpression> {
        let mut terms = arena::Arena::new();
        let root = self.integer_math_term(expression, depth, &mut terms)?;
        Some(CheckedIntegerMathExpression { terms, root })
    }

    fn integer_math_term(
        &mut self,
        expression: ExpressionHandle,
        depth: usize,
        terms: &mut arena::Arena<CheckedIntegerMathTerm>,
    ) -> Option<arena::Handle<CheckedIntegerMathTerm>> {
        if depth >= 64
            || *self.remaining == 0
            || !self
                .program
                .expression_table
                .expression_is_valid(expression)
        {
            return None;
        }
        *self.remaining -= 1;
        let program = self.program;
        let term = if let Some((primitive_type, argument)) =
            validation::integer_embedding_argument(program, expression)
        {
            let operand = self
                .integer_term(argument, depth + 1)
                .map(|(term, _)| term)
                .or_else(|| {
                    lower_contract_integer_literal(
                        program,
                        self.operators,
                        argument,
                        primitive_type,
                    )
                })?;
            if scalar_expression_type(&operand) != Some(primitive_type)
                || !is_integer(primitive_type)
            {
                return None;
            }
            CheckedIntegerMathTerm::Embed {
                primitive_type,
                operand,
            }
        } else if let ExpressionNode::Binary(binary) =
            program.expression_table.expression(expression)
            && validation::proof_integer_expression(program, expression)
            && operator_is_builtin(self.operators, expression)
        {
            let operator = binary.operator;
            if !matches!(
                operator,
                BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply
            ) {
                return None;
            }
            let left = self.integer_math_term(binary.left, depth + 1, terms)?;
            let right = self.integer_math_term(binary.right, depth + 1, terms)?;
            match operator {
                BinaryOperator::Add => CheckedIntegerMathTerm::Add { left, right },
                BinaryOperator::Subtract => CheckedIntegerMathTerm::Subtract { left, right },
                BinaryOperator::Multiply => CheckedIntegerMathTerm::Multiply { left, right },
                _ => return None,
            }
        } else {
            CheckedIntegerMathTerm::Literal(validation::anonymous_integer_math_literal(
                program, expression,
            )?)
        };
        Some(terms.insert(term))
    }

    fn boolean_type(&self, expression: ExpressionHandle) -> Option<TypeReferenceHandle> {
        if matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Name(_)
        ) {
            return self.subject(expression).map(|(_, reference, _)| reference);
        }
        self.program
            .type_reference_table
            .named_references()
            .find_map(|(reference, symbol, _)| {
                (self.program.symbols.builtin_type_atom(symbol)
                    == Some(symbols::BuiltinTypeAtom::Bool))
                .then_some(reference)
            })
    }

    fn boolean(
        &mut self,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Option<CheckedBooleanExpression> {
        if depth >= 64
            || *self.remaining == 0
            || !self
                .program
                .expression_table
                .expression_is_valid(expression)
        {
            return None;
        }
        *self.remaining -= 1;
        let program = self.program;
        match program.expression_table.expression(expression) {
            ExpressionNode::Boolean(value) => Some(CheckedBooleanExpression::Constant(*value)),
            ExpressionNode::Call(_) => {
                let (predicate, operand) =
                    typed_trees::byte_predicates::byte_predicate_call(program, expression)?;
                let (root, path, _) = super::boolean_lowering::comparable_byte_carrier(
                    program,
                    self.parameters,
                    operand,
                )?;
                // Contracts observe the exact formal's bytes, never an equally
                // numbered body-local descriptor. The common carrier query
                // retains projections and authenticates the readable byte type;
                // Terminal lowering owns the independent content proposition.
                root.parameter()?;
                Some(CheckedBooleanExpression::ByteSequencePredicate {
                    predicate,
                    root,
                    path,
                })
            }
            ExpressionNode::Name(_) => {
                let (position, reference, erased) = self.subject(expression)?;
                (program.primitive_type_reference(reference) == Some(PrimitiveType::Bool))
                    .then_some(if erased {
                        CheckedBooleanExpression::ErasedParameter { position }
                    } else {
                        CheckedBooleanExpression::Parameter { position }
                    })
            }
            ExpressionNode::Unary(unary)
                if unary.operator == UnaryOperator::LogicalNot
                    && operator_is_builtin(self.operators, expression) =>
            {
                Some(CheckedBooleanExpression::Not(Box::new(
                    self.boolean(unary.operand, depth + 1)?,
                )))
            }
            ExpressionNode::Binary(binary) => {
                if !operator_is_builtin(self.operators, expression) {
                    return None;
                }
                if validation::proof_integer_expression(program, binary.left)
                    || validation::proof_integer_expression(program, binary.right)
                {
                    let (kind, swap, negate) = match binary.operator {
                        BinaryOperator::Equal => {
                            (CheckedIntegerComparisonKind::Equal, false, false)
                        }
                        BinaryOperator::NotEqual => {
                            (CheckedIntegerComparisonKind::Equal, false, true)
                        }
                        BinaryOperator::Less => {
                            (CheckedIntegerComparisonKind::LessThan, false, false)
                        }
                        BinaryOperator::LessOrEqual => {
                            (CheckedIntegerComparisonKind::LessOrEqual, false, false)
                        }
                        BinaryOperator::Greater => {
                            (CheckedIntegerComparisonKind::LessThan, true, false)
                        }
                        BinaryOperator::GreaterOrEqual => {
                            (CheckedIntegerComparisonKind::LessOrEqual, true, false)
                        }
                        _ => return None,
                    };
                    let left = self.integer_math_expression(
                        if swap { binary.right } else { binary.left },
                        depth + 1,
                    )?;
                    let right = self.integer_math_expression(
                        if swap { binary.left } else { binary.right },
                        depth + 1,
                    )?;
                    let comparison =
                        CheckedBooleanExpression::IntegerMathComparison { kind, left, right };
                    return Some(if negate {
                        CheckedBooleanExpression::Not(Box::new(comparison))
                    } else {
                        comparison
                    });
                }
                let subjects =
                    [binary.left, binary.right].map(|expression| self.integer_term(expression, 0));
                if let Some(comparison) = lower_integer_contract_comparison(
                    program,
                    self.operators,
                    self.machine.symbol,
                    expression,
                    subjects,
                ) {
                    return Some(comparison);
                }
                if !matches!(
                    binary.operator,
                    BinaryOperator::And
                        | BinaryOperator::Or
                        | BinaryOperator::Equal
                        | BinaryOperator::NotEqual
                ) {
                    return None;
                }
                let left = Box::new(self.boolean(binary.left, depth + 1)?);
                let right = Box::new(self.boolean(binary.right, depth + 1)?);
                Some(match binary.operator {
                    BinaryOperator::And => CheckedBooleanExpression::And { left, right },
                    BinaryOperator::Or => CheckedBooleanExpression::Or { left, right },
                    BinaryOperator::Equal | BinaryOperator::NotEqual => {
                        let spelling = if binary.operator == BinaryOperator::Equal {
                            language_core::OperatorSpelling::Equal
                        } else {
                            language_core::OperatorSpelling::NotEqual
                        };
                        // Children have been checked as Boolean predicates. Keep
                        // exact formal types for overload selection; the reserved
                        // result is not an unknown wildcard operand.
                        let types = [
                            Some(self.boolean_type(binary.left)?),
                            Some(self.boolean_type(binary.right)?),
                        ];
                        if !typed_trees::operator::has_builtin_spelled_expression_meaning(
                            program,
                            self.machine.symbol,
                            expression,
                            spelling,
                            &types,
                        ) {
                            return None;
                        }
                        let equality = CheckedBooleanExpression::Equal { left, right };
                        if binary.operator == BinaryOperator::Equal {
                            equality
                        } else {
                            CheckedBooleanExpression::Not(Box::new(equality))
                        }
                    }
                    _ => return None,
                })
            }
            _ => None,
        }
    }
}

/// Whether a structural parameter's member storage is a place the caller
/// names before and after the call: the receiver, or a parameter borrowed
/// through a shared or exclusive reference. A contract fact about that
/// storage substitutes to the caller's actual place, entry and post-state
/// alike. An owned by-value parameter is the callee's own copy, so its
/// post-state is no caller fact and its members stay outside the namespace.
fn caller_storage_root(program: &TypedTrees, parameter: &StateParameter) -> bool {
    if parameter.is_self {
        return true;
    }
    let mut type_reference = parameter.type_reference;
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Reference { access, .. } => {
                return matches!(
                    access,
                    language_core::ReferenceAccess::Shared
                        | language_core::ReferenceAccess::Mutable
                );
            }
            _ => return false,
        }
    }
}

/// Subject readers retain their own namespace custody. Comparison selection,
/// contextual literal landing, and same-carrier construction have one owner.
pub(super) fn lower_integer_contract_comparison(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    owner: symbols::SymbolHandle,
    expression: ExpressionHandle,
    subjects: [Option<(CheckedScalarExpression, TypeReferenceHandle)>; 2],
) -> Option<CheckedBooleanExpression> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    if operators.uses.iter().any(|(_, operator)| {
        operator.expression == expression
            && operator.status != CheckedOperatorResolutionStatus::BuiltinFallback
    }) {
        return None;
    }
    use language_core::OperatorSpelling;
    let spelling = match binary.operator {
        BinaryOperator::Equal => OperatorSpelling::Equal,
        BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
        BinaryOperator::Less => OperatorSpelling::Less,
        BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
        BinaryOperator::Greater => OperatorSpelling::Greater,
        BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
        _ => return None,
    };
    let operand_types =
        [(binary.left, &subjects[0]), (binary.right, &subjects[1])].map(|(expression, subject)| {
            subject
                .as_ref()
                .map(|(_, type_reference)| *type_reference)
                .or_else(|| validation::landed_integer_literal_type_reference(program, expression))
        });
    // A typed subject or already-landed literal supplies contextual landing.
    // Keep literal comparisons as predicates too: folding their value here
    // would lose the selected operator and carrier needed by source replay.
    let contextual_type = operand_types.into_iter().flatten().next()?;
    let contextual_primitive = program.primitive_type_reference(contextual_type)?;
    if !is_integer(contextual_primitive) {
        return None;
    }
    if !typed_trees::operator::has_builtin_spelled_expression_meaning(
        program,
        owner,
        expression,
        spelling,
        &operand_types,
    ) {
        return None;
    }
    let operand = |expression, subject: Option<(CheckedScalarExpression, TypeReferenceHandle)>| {
        if let Some((value, type_reference)) = subject {
            let primitive_type = program.primitive_type_reference(type_reference)?;
            if !is_integer(primitive_type) || scalar_expression_type(&value) != Some(primitive_type)
            {
                return None;
            }
            return Some(value);
        }
        lower_contract_integer_literal(program, operators, expression, contextual_primitive)
    };
    let [left_subject, right_subject] = subjects;
    let left = operand(binary.left, left_subject)?;
    let right = operand(binary.right, right_subject)?;
    construct_integer_comparison(binary.operator, left, right)
}

/// Anonymous arithmetic lands at the comparison or typed peer's carrier through
/// the ordinary scalar lowerer. With no runtime namespace, only a completed
/// literal can supply this operand: typed operations retain their own policy
/// and remain with `integer_term`, rather than being reinterpreted as constants.
fn lower_contract_integer_literal(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    expression: ExpressionHandle,
    primitive: PrimitiveType,
) -> Option<CheckedScalarExpression> {
    let value = lower_return_expression(
        program,
        operators,
        expression,
        &[],
        &[],
        &[],
        &[],
        primitive,
        &[],
    )?;
    matches!(value, CheckedScalarExpression::IntegerLiteral { .. }).then_some(value)
}

/// One authored parameter range constraint observed by the scalar contract
/// family. The three projections share a single constraint walk so a
/// requires-tail clause row and its retained roster evidence can never
/// disagree about which source range they describe.
pub(crate) struct ParameterRangeRequirements {
    /// Scalar refinements as closed predicates, one slot per authored
    /// constraint. The crash requirement capsule and boundary replay read
    /// this projection. Legacy floating ranges retain `None` because their
    /// separate endpoint roster has not joined this predicate owner.
    pub(crate) scalar_predicates: Vec<Option<CheckedBooleanExpression>>,
    /// Requires-tail rows in authored constraint order: supported integer
    /// ranges as `Predicate` clauses and retained floating ranges as
    /// `FloatRange` clauses carrying their IEEE endpoints verbatim. `None`
    /// marks a present range that could not be retained at all.
    pub(crate) scalar_clauses: Vec<Option<checked_trees::ClosedScalarContractValue>>,
    /// The retained floating roster in dense scalar-parameter order. `None`
    /// records an incomplete roster — an authored floating range whose
    /// endpoints could not be retained exactly — so consumers fail closed
    /// rather than read a partial roster as complete.
    pub(crate) float_entry_ranges: Option<Vec<checked_trees::ClosedFloatRangeRequirement>>,
    /// The retained integer roster in dense scalar-parameter order. `None`
    /// records an incomplete roster — a present integer range whose
    /// normalized endpoints could not be retained exactly — so consumers
    /// fail closed rather than read a partial roster as complete. Each row
    /// shares the exact landed literals its `Predicate` clause carries.
    pub(crate) integer_entry_ranges: Option<Vec<checked_trees::ClosedIntegerRangeRequirement>>,
}

/// Bracket constraints are native numeric requires sugar. Keep every present
/// range: integer intervals land as closed `<=` conjunctions while floating
/// windows retain their authored IEEE endpoints and boundary kind verbatim —
/// the exclusive end stays authored, never an integer predecessor.
pub(crate) fn lower_scalar_parameter_range_requirements(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> ParameterRangeRequirements {
    let Some(entry) = program.machine_states(machine).first() else {
        // An absent entry cannot publish an empty-but-complete range roster.
        return ParameterRangeRequirements {
            scalar_predicates: Vec::new(),
            scalar_clauses: Vec::new(),
            float_entry_ranges: None,
            integer_entry_ranges: None,
        };
    };
    lower_callable_parameter_range_requirements(program, program.state_parameters(entry))
}

/// Parameter ranges belong to the declared callable signature, whether its
/// owner has a machine body or is a bodyless operator requirement.
pub(crate) fn lower_callable_parameter_range_requirements(
    program: &TypedTrees,
    parameters: &[StateParameter],
) -> ParameterRangeRequirements {
    let mut ranges = ParameterRangeRequirements {
        scalar_predicates: Vec::new(),
        scalar_clauses: Vec::new(),
        float_entry_ranges: Some(Vec::new()),
        integer_entry_ranges: Some(Vec::new()),
    };
    let mut scalar_position = 0;
    for parameter in parameters {
        let primitive_type = program.primitive_type_reference(parameter.type_reference);
        let position = scalar_position;
        if primitive_type.is_some() {
            scalar_position += 1;
        }
        let mut type_reference = parameter.type_reference;
        loop {
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    for constraint in program.type_reference_table.constraints(*constraints) {
                        if let typed_trees::types::TypeConstraintNode::Domain(domain) = constraint
                            && let typed_trees::types::DomainConstraintSubject::Value(domain) =
                                domain.subject
                        {
                            let predicate = primitive_type.and_then(|primitive_type| {
                                validation::scalar_value_domain_predicate(
                                    domain,
                                    CheckedScalarExpression::Parameter {
                                        position,
                                        primitive_type,
                                    },
                                )
                            });
                            ranges.scalar_clauses.push(
                                predicate
                                    .clone()
                                    .map(checked_trees::ClosedScalarContractValue::Predicate),
                            );
                            ranges.scalar_predicates.push(predicate);
                            continue;
                        }
                        // A domain whose membership is exactly an interval
                        // means what a bracketed range meant, so it lands the
                        // same closed entry range; any other domain is a
                        // qualification this roster does not carry.
                        if let typed_trees::types::TypeConstraintNode::Domain(domain) = constraint {
                            let Some((low, high)) = primitive_type
                                .filter(|primitive_type| is_integer(*primitive_type))
                                .and_then(|primitive_type| {
                                    validation::exact_declared_domain_carrier_interval(
                                        program,
                                        primitive_type,
                                        domain,
                                    )
                                    .filter(|(minimum, maximum)| minimum <= maximum)
                                })
                            else {
                                continue;
                            };
                            ranges.push_integer(closed_integer_entry_range(
                                program, parameter, position, low, high,
                            ));
                            continue;
                        }
                        let typed_trees::types::TypeConstraintNode::Range {
                            minimum,
                            maximum,
                            end_inclusive,
                        } = constraint
                        else {
                            continue;
                        };
                        let predicate = || {
                            let low = validation::closed_integer_range_bound(program, *minimum)?;
                            let high = validation::closed_integer_range_maximum(
                                program,
                                *maximum,
                                *end_inclusive,
                            )?;
                            closed_integer_entry_range(program, parameter, position, low, high)
                        };
                        let requirement = || {
                            // Existing source validation rejects range constraints
                            // outside Exact: those domains do not enforce stores.
                            if parameter.is_self
                                || parameter.is_const
                                || program
                                    .arithmetic_domain_for_type_reference(parameter.type_reference)
                                    != ArithmeticDomain::Exact
                            {
                                return None;
                            }
                            let primitive_type = primitive_type?;
                            let minimum = validation::closed_float_range_endpoint(
                                program,
                                *minimum,
                                primitive_type,
                            )?;
                            let maximum = validation::closed_float_range_endpoint(
                                program,
                                *maximum,
                                primitive_type,
                            )?;
                            // IEEE order: a NaN endpoint or a reversed window
                            // cannot be retained as a nonempty requirement.
                            if !validation::ieee_float_range_ordered(minimum, maximum) {
                                return None;
                            }
                            Some(checked_trees::ClosedFloatRangeRequirement {
                                position,
                                primitive_type,
                                minimum,
                                maximum,
                                maximum_inclusive: *end_inclusive,
                            })
                        };
                        let predicate = predicate();
                        if matches!(
                            primitive_type,
                            Some(PrimitiveType::F32 | PrimitiveType::F64)
                        ) {
                            // A floating range is a `FloatRange` clause in the
                            // requires tail; its retained evidence rides the
                            // roster. One failed endpoint voids the whole
                            // roster so no consumer reads a partial one.
                            match requirement() {
                                Some(requirement) => {
                                    if let Some(roster) = &mut ranges.float_entry_ranges {
                                        roster.push(requirement);
                                    }
                                    ranges.scalar_clauses.push(Some(
                                        checked_trees::ClosedScalarContractValue::FloatRange(
                                            requirement,
                                        ),
                                    ));
                                }
                                None => {
                                    ranges.float_entry_ranges = None;
                                    ranges.scalar_clauses.push(None);
                                }
                            }
                        } else {
                            ranges.push_integer(predicate);
                            continue;
                        }
                        ranges
                            .scalar_predicates
                            .push(predicate.map(|(predicate, _)| predicate));
                    }
                    type_reference = *base_type;
                }
                _ => break,
            }
        }
    }
    ranges
}

impl ParameterRangeRequirements {
    /// One integer entry range retains its normalized bounds on the roster
    /// while the requires tail keeps the predicate clause. One failed range
    /// voids the whole roster so no consumer reads a partial one.
    fn push_integer(
        &mut self,
        range: Option<(
            CheckedBooleanExpression,
            checked_trees::ClosedIntegerRangeRequirement,
        )>,
    ) {
        match range {
            Some((predicate, requirement)) => {
                if let Some(roster) = &mut self.integer_entry_ranges {
                    roster.push(requirement);
                }
                self.scalar_clauses.push(Some(
                    checked_trees::ClosedScalarContractValue::Predicate(predicate.clone()),
                ));
                self.scalar_predicates.push(Some(predicate));
            }
            None => {
                self.integer_entry_ranges = None;
                self.scalar_clauses.push(None);
                self.scalar_predicates.push(None);
            }
        }
    }
}

/// A non-entry state's scalar parameter membership in an exactly-interval
/// domain, as the closed predicate its authored `requires` row would lower
/// to: the same interval clause a bracketed range produces, indexed in the
/// state's scalar parameter roster.
pub(crate) fn lower_state_interval_predicate(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    parameter: symbols::SymbolHandle,
    minimum: numerics::bignum::BigInt,
    maximum: numerics::bignum::BigInt,
) -> Option<CheckedBooleanExpression> {
    let parameters = program.state_parameters(state);
    let index = parameters
        .iter()
        .position(|candidate| candidate.symbol == parameter)?;
    let authored = &parameters[index];
    if authored.relevance.is_erased() {
        return None;
    }
    let position = parameters[..index]
        .iter()
        .filter(|parameter| crate::values::scalar::occupies_scalar_position(program, parameter))
        .count();
    closed_integer_entry_range(program, authored, position, minimum, maximum)
        .map(|(predicate, _)| predicate)
}

/// The closed `low <= parameter && parameter <= high` clause and its roster
/// row for one integer parameter's interval, whether a bracketed range or an
/// exactly-interval domain stated it.
fn closed_integer_entry_range(
    program: &TypedTrees,
    parameter: &StateParameter,
    position: usize,
    low: numerics::bignum::BigInt,
    high: numerics::bignum::BigInt,
) -> Option<(
    CheckedBooleanExpression,
    checked_trees::ClosedIntegerRangeRequirement,
)> {
    // Existing source validation rejects range constraints outside Exact:
    // those domains do not enforce stores.
    if parameter.is_self
        || parameter.is_const
        || program.arithmetic_domain_for_type_reference(parameter.type_reference)
            != ArithmeticDomain::Exact
    {
        return None;
    }
    let primitive_type = program.primitive_type_reference(parameter.type_reference)?;
    if !is_integer(primitive_type) || low > high {
        return None;
    }
    // Endpoints have already been evaluated under their own selected meaning.
    // Land the normalized interval, not an exclusive end outside the carrier.
    let minimum_literal = validation::land_integer_value(&low, primitive_type)?;
    let maximum_literal = validation::land_integer_value(&high, primitive_type)?;
    let minimum = CheckedScalarExpression::IntegerLiteral {
        literal: minimum_literal.clone(),
    };
    let maximum = CheckedScalarExpression::IntegerLiteral {
        literal: maximum_literal.clone(),
    };
    let subject = CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    };
    // This is the meaning of the interval, not an authored selectable `<=`
    // operator occurrence.
    let predicate = CheckedBooleanExpression::And {
        left: Box::new(construct_integer_comparison(
            BinaryOperator::LessOrEqual,
            minimum,
            subject.clone(),
        )?),
        right: Box::new(construct_integer_comparison(
            BinaryOperator::LessOrEqual,
            subject,
            maximum,
        )?),
    };
    // The retained roster row shares the exact landed endpoints the predicate
    // carries so clause and evidence can never disagree.
    let requirement = checked_trees::ClosedIntegerRangeRequirement {
        position,
        primitive_type,
        minimum: minimum_literal,
        maximum: maximum_literal,
    };
    Some((predicate, requirement))
}

/// Predicate obligations from authored parameter refinements. Legacy floating
/// interval rows keep an unsupported slot until their endpoint owner joins
/// the predicate representation; core value domains already use predicates.
pub(crate) fn lower_scalar_parameter_refinement_predicates(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Vec<Option<CheckedBooleanExpression>> {
    lower_scalar_parameter_range_requirements(program, machine).scalar_predicates
}
