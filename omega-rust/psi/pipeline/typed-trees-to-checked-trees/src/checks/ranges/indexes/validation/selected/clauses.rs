use language_core::operator_spelling::OperatorSpelling;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::operator::OperatorDefinition;
use typed_trees::signature::SignatureContractKind;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Operand {
    Zero,
    CollectionLength,
    Position(usize),
}

pub(super) struct SelectedClauses {
    pub(super) labels: Vec<String>,
    /// Bit 2 names the scalar index/range start; bit 3 names the range end.
    /// These obligations must be discharged separately from upper bounds.
    pub(super) lower_bound_positions: u8,
}

pub(super) fn validate(
    program: &TypedTrees,
    operator: &OperatorDefinition,
    spelling: OperatorSpelling,
) -> Result<SelectedClauses, &'static str> {
    let parameters = program.operator_parameters(operator);
    let expected_count = if spelling == OperatorSpelling::Index {
        2
    } else {
        3
    };
    if parameters.len() != expected_count {
        return Err("has an unsupported selected operand telescope");
    }
    let mut symbols = [SymbolHandle::invalid(); 3];
    for (ordinal, parameter) in parameters
        .iter()
        .filter(|parameter| parameter.is_self)
        .chain(parameters.iter().filter(|parameter| !parameter.is_self))
        .enumerate()
    {
        if !parameter.symbol.is_valid() || symbols[..ordinal].contains(&parameter.symbol) {
            return Err("has an unbound or duplicate selected formal parameter");
        }
        symbols[ordinal] = parameter.symbol;
    }
    let mut coverage = 0u8;
    let mut labels = Vec::new();
    for contract in program
        .operator_contracts(operator)
        .iter()
        .filter(|contract| contract.kind == SignatureContractKind::Requires)
    {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let ProofFact::Expression(expression) = fact else {
                return Err(
                    "has an unsupported selected `requires` fact; bounds alone cannot discharge it",
                );
            };
            coverage |= clause(program, *expression, &symbols, spelling, 0).ok_or(
                "has an unsupported selected `requires` clause; bounds alone cannot discharge it",
            )?;
            labels.push(program.expression_table.display_name(*expression));
        }
    }
    let required = if spelling == OperatorSpelling::Index {
        1
    } else {
        3
    };
    if coverage & required != required {
        return Err(
            "selected `requires` does not state the complete collection-relative bounds obligation",
        );
    }
    Ok(SelectedClauses {
        labels,
        lower_bound_positions: coverage & 12,
    })
}

fn clause(
    program: &TypedTrees,
    expression: ExpressionHandle,
    parameters: &[SymbolHandle; 3],
    spelling: OperatorSpelling,
    depth: usize,
) -> Option<u8> {
    if depth >= 128 || !expression.is_valid() {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            clause(program, atomic.value, parameters, spelling, depth + 1)
        }
        ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And => Some(
            clause(program, binary.left, parameters, spelling, depth + 1)?
                | clause(program, binary.right, parameters, spelling, depth + 1)?,
        ),
        ExpressionNode::Binary(binary) => {
            let mut left = operand(program, binary.left, parameters, 0)?;
            let mut right = operand(program, binary.right, parameters, 0)?;
            let strict = match binary.operator {
                BinaryOperator::Less => true,
                BinaryOperator::LessOrEqual => false,
                BinaryOperator::Greater => {
                    std::mem::swap(&mut left, &mut right);
                    true
                }
                BinaryOperator::GreaterOrEqual => {
                    std::mem::swap(&mut left, &mut right);
                    false
                }
                _ => return None,
            };
            match (spelling, left, right, strict) {
                (
                    OperatorSpelling::Index,
                    Operand::Position(1),
                    Operand::CollectionLength,
                    true,
                ) => Some(1),
                (OperatorSpelling::Range, Operand::Position(1), Operand::Position(2), false) => {
                    Some(1)
                }
                (
                    OperatorSpelling::Range,
                    Operand::Position(2),
                    Operand::CollectionLength,
                    false,
                ) => Some(2),
                (_, Operand::Zero, Operand::Position(1), false) => Some(4),
                (OperatorSpelling::Range, Operand::Zero, Operand::Position(2), false) => Some(8),
                _ => None,
            }
        }
        _ => None,
    }
}

fn operand(
    program: &TypedTrees,
    expression: ExpressionHandle,
    parameters: &[SymbolHandle; 3],
    depth: usize,
) -> Option<Operand> {
    if depth >= 128 || !expression.is_valid() {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => operand(program, atomic.value, parameters, depth + 1),
        ExpressionNode::Integer(value) if value.value_i64() == Some(0) => Some(Operand::Zero),
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            if members.len() == 1 {
                parameters
                    .iter()
                    .position(|symbol| symbol.is_valid() && *symbol == path.symbol)
                    .map(Operand::Position)
            } else if members.len() == 2
                && members[1].as_str() == "len"
                && path.head_symbol == parameters[0]
            {
                Some(Operand::CollectionLength)
            } else {
                None
            }
        }
        ExpressionNode::Member(member)
            if member.member.as_str() == "len"
                && operand(program, member.receiver, parameters, depth + 1)
                    == Some(Operand::Position(0)) =>
        {
            Some(Operand::CollectionLength)
        }
        _ => None,
    }
}
