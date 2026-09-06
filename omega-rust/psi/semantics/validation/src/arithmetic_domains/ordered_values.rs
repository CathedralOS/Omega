//! Exact temporary operand identities for live ordered arithmetic facts.
//!
//! Paths are write-frame metadata only. Equality uses resolved declarations and
//! the complete argument tree of an eligible normal-return call.

use super::*;
use symbols::SymbolHandle;
use typed_trees::statement::StatementNode;

mod requirements;
pub use requirements::validate_ordered_requirement_call_totality;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Operand {
    Place {
        root: SymbolHandle,
        fields: Vec<SymbolHandle>,
        path: String,
    },
    Integer(
        numerics::literals::IntegerLiteral,
        Option<numerics::literals::IntegerLanding>,
    ),
    Call {
        target: SymbolHandle,
        arguments: Vec<Operand>,
    },
    Binary {
        operator: BinaryOperator,
        primitive: PrimitiveType,
        domain: ArithmeticDomain,
        operands: Vec<Operand>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Relation {
    pub(super) left: Operand,
    pub(super) right: Operand,
    pub(super) floor: i64,
}

impl Operand {
    pub(super) fn parameter(parameter: &typed_trees::signature::StateParameter) -> Self {
        Self::Place {
            root: parameter.symbol,
            fields: Vec::new(),
            path: parameter.name.as_str().to_owned(),
        }
    }

    pub(super) fn survives(&self, written: &[String]) -> bool {
        match self {
            Self::Place { path, .. } => {
                !written.iter().any(|write| place_paths_overlap(path, write))
            }
            Self::Integer(..) => true,
            Self::Call { arguments, .. } => arguments.iter().all(|value| value.survives(written)),
            Self::Binary { operands, .. } => operands.iter().all(|value| value.survives(written)),
        }
    }

    fn rebound(&self, bindings: &[(Operand, Operand)]) -> Vec<Self> {
        let direct = bindings
            .iter()
            .filter(|(source, _)| source == self)
            .map(|(_, target)| target.clone())
            .collect::<Vec<_>>();
        if !direct.is_empty() {
            return direct;
        }
        match self {
            Self::Integer(..) => vec![self.clone()],
            Self::Place { root, fields, path } => bindings
                .iter()
                .filter_map(|(source, target)| {
                    let Self::Place {
                        root: source_root,
                        fields: source_fields,
                        path: source_path,
                    } = source
                    else {
                        return None;
                    };
                    let Self::Place {
                        root: target_root,
                        fields: target_fields,
                        path: target_path,
                    } = target
                    else {
                        return None;
                    };
                    if root != source_root || !fields.starts_with(source_fields) {
                        return None;
                    }
                    let suffix = path.strip_prefix(source_path)?.strip_prefix('.')?;
                    let mut projected = target_fields.clone();
                    projected.extend_from_slice(&fields[source_fields.len()..]);
                    Some(Self::Place {
                        root: *target_root,
                        fields: projected,
                        path: format!("{target_path}.{suffix}"),
                    })
                })
                .collect(),
            Self::Call { target, arguments } => {
                let mut combinations = vec![Vec::new()];
                for argument in arguments {
                    let alternatives = argument.rebound(bindings);
                    combinations = combinations
                        .into_iter()
                        .flat_map(|prefix| {
                            alternatives.iter().map(move |argument| {
                                let mut values = prefix.clone();
                                values.push(argument.clone());
                                values
                            })
                        })
                        .collect();
                }
                combinations
                    .into_iter()
                    .map(|arguments| Self::Call {
                        target: *target,
                        arguments,
                    })
                    .collect()
            }
            Self::Binary {
                operator,
                primitive,
                domain,
                operands,
            } => {
                let [left, right] = operands.as_slice() else {
                    return Vec::new();
                };
                left.rebound(bindings)
                    .into_iter()
                    .flat_map(|left| {
                        right
                            .rebound(bindings)
                            .into_iter()
                            .map(move |right| Self::Binary {
                                operator: *operator,
                                primitive: *primitive,
                                domain: *domain,
                                operands: vec![left.clone(), right],
                            })
                    })
                    .collect()
            }
        }
    }
}

impl Relation {
    pub(super) fn survives(&self, written: &[String]) -> bool {
        self.left.survives(written) && self.right.survives(written)
    }

    pub(super) fn rebound(&self, bindings: &[(Operand, Operand)]) -> Vec<Self> {
        self.left
            .rebound(bindings)
            .into_iter()
            .flat_map(|left| {
                self.right
                    .rebound(bindings)
                    .into_iter()
                    .map(move |right| Self {
                        left: left.clone(),
                        right,
                        floor: self.floor,
                    })
            })
            .collect()
    }
}

pub(super) fn operand(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<Operand> {
    build_operand(program, machine, state, expression, 0)
}

fn build_operand(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<Operand> {
    if !expression.is_valid() || depth >= 128 {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(literal) => {
            Some(Operand::Integer(literal.clone(), literal.landing()))
        }
        ExpressionNode::Name(path) => {
            if !path.symbol.is_valid() || path.symbol != path.head_symbol {
                return None;
            }
            let parameters = program.state_parameters(state);
            let local = program.statement_table.statements(state.statement_nodes).iter().any(|statement| {
                matches!(statement, StatementNode::LocalData(local) if local.symbol == path.symbol)
            });
            let parameter = parameters
                .iter()
                .any(|parameter| parameter.symbol == path.symbol);
            let attached = path.symbol == machine.symbol
                && parameters.iter().any(|parameter| parameter.is_self);
            if !local && !parameter && !attached {
                return None;
            }
            Some(Operand::Place {
                root: path.symbol,
                fields: Vec::new(),
                path: place_path(program, expression)?,
            })
        }
        ExpressionNode::Member(member) if member.case_variant.is_none() => {
            let Operand::Place {
                root, mut fields, ..
            } = build_operand(program, machine, state, member.receiver, depth + 1)?
            else {
                return None;
            };
            let field = if root == machine.symbol {
                crate::exact_self_field(program, machine, expression)?.symbol
            } else {
                member.member_symbol
            };
            if !field.is_valid()
                || declared_place_type_raw(program, machine, Some(state), expression).is_none()
            {
                return None;
            }
            fields.push(field);
            Some(Operand::Place {
                root,
                fields,
                path: place_path(program, expression)?,
            })
        }
        ExpressionNode::Call(call) => {
            let operational = crate::infer_operational_may(program);
            let reaches = crate::infer_service_reaches(program, &operational);
            let (_, entry) = crate::denotational_calls::normal_return_call_candidate(
                program,
                call,
                &operational,
                &reaches,
            )
            .ok()?;
            let arguments = program.expression_table.expression_handles(call.arguments);
            if arguments.len() != program.state_parameters(entry).len() {
                return None;
            }
            Some(Operand::Call {
                target: call.target_symbol,
                arguments: arguments
                    .iter()
                    .map(|argument| build_operand(program, machine, state, *argument, depth + 1))
                    .collect::<Option<Vec<_>>>()?,
            })
        }
        ExpressionNode::Binary(binary)
            if crate::has_builtin_bound_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            ) =>
        {
            let (primitive, domain) = integer_meaning(program, machine, state, expression)?;
            Some(Operand::Binary {
                operator: binary.operator,
                primitive,
                domain,
                operands: vec![
                    build_operand(program, machine, state, binary.left, depth + 1)?,
                    build_operand(program, machine, state, binary.right, depth + 1)?,
                ],
            })
        }
        _ => None,
    }
}

fn integer_meaning(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<(PrimitiveType, ArithmeticDomain)> {
    if let ExpressionNode::Binary(binary) = program.expression_table.expression(expression)
        && matches!(
            binary.operator,
            BinaryOperator::BitwiseAnd | BinaryOperator::BitwiseOr | BinaryOperator::BitwiseXor
        )
    {
        // Fixed-width bitwise operations are policy-independent representation
        // operations (chapter 5). Recover the width from their typed operands;
        // this is not an interval evaluation or an overflow-policy bridge.
        let left = integer_meaning(program, machine, state, binary.left);
        let right = integer_meaning(program, machine, state, binary.right);
        if let (Some((left, _)), Some((right, _))) = (left, right)
            && left != right
        {
            return None;
        }
        return left
            .or(right)
            .map(|(primitive, _)| (primitive, ArithmeticDomain::Exact));
    }
    // Reuse the arithmetic owner's operand-driven carrier/policy selection.
    // This query supplies no range proof; ordinary validation still owes all
    // formation diagnostics, and the empty environment contains no relations.
    let analysis = analyze(
        program,
        machine,
        Some(state),
        expression,
        &ValueEnv::new(),
        None,
        ArithmeticDomain::Exact,
        "ordered operand type",
        &mut Vec::new(),
    );
    let primitive = analysis.primitive?;
    (primitive != PrimitiveType::Addr && primitive_range(primitive).is_some()).then_some((
        primitive,
        analysis.domain.unwrap_or(ArithmeticDomain::Exact),
    ))
}

pub(super) fn record(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    environment: &mut ValueEnv,
    comparison: &typed_trees::expression::TableBinaryExpression,
    positive: bool,
) {
    let (left, right, strict) = match (comparison.operator, positive) {
        (BinaryOperator::GreaterOrEqual, true) | (BinaryOperator::Less, false) => {
            (comparison.left, comparison.right, false)
        }
        (BinaryOperator::Greater, true) | (BinaryOperator::LessOrEqual, false) => {
            (comparison.left, comparison.right, true)
        }
        (BinaryOperator::LessOrEqual, true) | (BinaryOperator::Greater, false) => {
            (comparison.right, comparison.left, false)
        }
        (BinaryOperator::Less, true) | (BinaryOperator::GreaterOrEqual, false) => {
            (comparison.right, comparison.left, true)
        }
        _ => return,
    };
    if integer_meaning(program, machine, state, left).is_none()
        || integer_meaning(program, machine, state, right).is_none()
    {
        return;
    }
    let (Some(left), Some(right)) = (
        operand(program, machine, state, left),
        operand(program, machine, state, right),
    ) else {
        return;
    };
    let relation = Relation {
        left,
        right,
        floor: i64::from(strict),
    };
    if !environment.ordered_values.contains(&relation) {
        environment.ordered_values.push(relation);
    }
}

pub(super) fn subtract_floor(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    environment: &ValueEnv,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> Option<i64> {
    if environment.ordered_values.is_empty() {
        return None;
    }
    let state = state?;
    let left = operand(program, machine, state, left)?;
    let right = operand(program, machine, state, right)?;
    environment
        .ordered_values
        .iter()
        .filter(|relation| relation.left == left && relation.right == right)
        .map(|relation| relation.floor)
        .max()
}
