//! Exact predicate projection of a scalar membership requirement. The source
//! domain and its subject remain separate from the projected Boolean clauses:
//! equal predicates do not identify domains, and predicates cannot mint routed
//! membership. Both checked production and receiving replay use this owner.

use checked_trees::{
    CheckedBooleanExpression as Boolean, CheckedIntegerComparisonKind as Comparison,
    CheckedOperatorFacts, CheckedScalarExpression as Scalar,
};
use typed_trees::TypedTrees;
use typed_trees::domain::{DomainDefinition, ProofFact, ProofMembershipFact};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::types::PrimitiveType;

/// The exact declared carrier of a membership whose complete meaning fits this
/// predicate package. Indexed and routed theory needs its separate evidence
/// transport; declining it here must never drop only the unsupported rows.
pub fn scalar_membership_carrier(
    program: &TypedTrees,
    membership: &ProofMembershipFact,
) -> Option<PrimitiveType> {
    let domain = typed_trees::domain::domain_by_symbol(program, membership.domain_symbol)?;
    if program
        .domain_definitions()
        .iter()
        .filter(|row| row.symbol == domain.symbol)
        .count()
        != 1
        || !domain.symbol.is_valid()
        || program.symbols.get(domain.symbol).kind != symbols::SymbolKind::Domain
        || !membership.domain_arguments.is_empty()
        || !typed_trees::domain::index_parameters(program, domain).is_empty()
        || !domain.index_arguments.is_empty()
        || domain.alias.is_some()
        || !domain.establishment_routes.is_empty()
        || !super::scalar_tags::exact_instance(program, domain, &[], membership.semantic_domain)
    {
        return None;
    }
    crate::value_custody::recasts::exact_primitive_type(program, domain.target_type)
}

/// Project every authored predicate, in declaration order, under one already
/// checked scalar subject. The caller separately rejoins that subject to the
/// membership's source expression; this query grants no value or provenance.
pub fn scalar_membership_predicates(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    membership: &ProofMembershipFact,
    subject: &Scalar,
) -> Option<Vec<Boolean>> {
    let mut active = Vec::new();
    project(program, operators, membership, subject, &mut active)
}

fn project(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    membership: &ProofMembershipFact,
    subject: &Scalar,
    active: &mut Vec<symbols::SymbolHandle>,
) -> Option<Vec<Boolean>> {
    let primitive = scalar_membership_carrier(program, membership)?;
    if subject.primitive_type() != Some(primitive) || active.contains(&membership.domain_symbol) {
        return None;
    }
    active.push(membership.domain_symbol);
    let domain = typed_trees::domain::domain_by_symbol(program, membership.domain_symbol)?;
    let spelling = Predicate {
        program,
        operators,
        domain,
        subject,
        primitive,
    };
    let mut predicates = Vec::new();
    for fact in program.proof_facts.span(domain.facts)? {
        match fact {
            ProofFact::Expression(expression) => predicates.push(spelling.boolean(*expression, 0)?),
            ProofFact::Membership(nested) => {
                if crate::exact_domain_self_type(program, domain, nested.value).is_none() {
                    return None;
                }
                predicates.extend(project(program, operators, nested, subject, active)?);
            }
            ProofFact::Proposition(_) => return None,
        }
    }
    active.pop();
    Some(predicates)
}

struct Predicate<'program> {
    program: &'program TypedTrees,
    operators: &'program CheckedOperatorFacts,
    domain: &'program DomainDefinition,
    subject: &'program Scalar,
    primitive: PrimitiveType,
}

impl Predicate<'_> {
    fn boolean(&self, expression: ExpressionHandle, depth: usize) -> Option<Boolean> {
        if depth >= 128
            || !self
                .program
                .expression_table
                .expression_is_valid(expression)
        {
            return None;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Boolean(value) => Some(Boolean::Constant(*value)),
            ExpressionNode::Name(_)
                if self.primitive == PrimitiveType::Bool
                    && crate::exact_domain_self_type(self.program, self.domain, expression)
                        .is_some() =>
            {
                match self.subject {
                    Scalar::Boolean(value) => Some((**value).clone()),
                    Scalar::Parameter { position, .. } => Some(Boolean::Parameter {
                        position: *position,
                    }),
                    Scalar::ErasedParameter { position, .. } => Some(Boolean::ErasedParameter {
                        position: *position,
                    }),
                    _ => None,
                }
            }
            ExpressionNode::Unary(unary)
                if unary.operator == UnaryOperator::LogicalNot
                    && crate::has_builtin_domain_decomposed_guard_meaning(
                        self.program,
                        self.domain,
                        expression,
                    ) =>
            {
                Some(Boolean::Not(Box::new(
                    self.boolean(unary.operand, depth + 1)?,
                )))
            }
            ExpressionNode::Binary(binary)
                if crate::has_builtin_domain_decomposed_guard_meaning(
                    self.program,
                    self.domain,
                    expression,
                ) =>
            {
                if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
                    let left = Box::new(self.boolean(binary.left, depth + 1)?);
                    let right = Box::new(self.boolean(binary.right, depth + 1)?);
                    return Some(if binary.operator == BinaryOperator::And {
                        Boolean::And { left, right }
                    } else {
                        Boolean::Or { left, right }
                    });
                }
                if self.primitive == PrimitiveType::Bool
                    && matches!(
                        binary.operator,
                        BinaryOperator::Equal | BinaryOperator::NotEqual
                    )
                {
                    let value = Boolean::Equal {
                        left: Box::new(self.boolean(binary.left, depth + 1)?),
                        right: Box::new(self.boolean(binary.right, depth + 1)?),
                    };
                    return Some(if binary.operator == BinaryOperator::NotEqual {
                        Boolean::Not(Box::new(value))
                    } else {
                        value
                    });
                }
                let (kind, left, right, negated) = match binary.operator {
                    BinaryOperator::Equal => (Comparison::Equal, binary.left, binary.right, false),
                    BinaryOperator::NotEqual => {
                        (Comparison::Equal, binary.left, binary.right, true)
                    }
                    BinaryOperator::Less => {
                        (Comparison::LessThan, binary.left, binary.right, false)
                    }
                    BinaryOperator::LessOrEqual => {
                        (Comparison::LessOrEqual, binary.left, binary.right, false)
                    }
                    BinaryOperator::Greater => {
                        (Comparison::LessThan, binary.right, binary.left, false)
                    }
                    BinaryOperator::GreaterOrEqual => {
                        (Comparison::LessOrEqual, binary.right, binary.left, false)
                    }
                    _ => return None,
                };
                let comparison = Boolean::IntegerComparison {
                    kind,
                    left: Box::new(self.scalar(left)?),
                    right: Box::new(self.scalar(right)?),
                };
                Some(if negated {
                    Boolean::Not(Box::new(comparison))
                } else {
                    comparison
                })
            }
            _ => None,
        }
    }

    fn scalar(&self, expression: ExpressionHandle) -> Option<Scalar> {
        if !self.primitive.accepts_integer_literal() || self.primitive == PrimitiveType::Addr {
            return None;
        }
        if crate::exact_domain_self_type(self.program, self.domain, expression).is_some() {
            return Some(self.subject.clone());
        }
        crate::land_anonymous_integer_expression(self.program, expression, self.primitive, |node| {
            !self.operators.uses.iter().any(|(_, operator)| {
                operator.expression == node
                    && operator.status
                        != checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
            })
        })
        .map(|literal| Scalar::IntegerLiteral { literal })
    }
}
