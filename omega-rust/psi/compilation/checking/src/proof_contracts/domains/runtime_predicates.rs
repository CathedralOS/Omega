//! Exact predicate projection of a scalar membership requirement. The source
//! domain and its subject remain separate from the projected Boolean clauses:
//! equal predicates do not identify domains, and predicates cannot mint routed
//! membership. Both checked production and receiving replay use this owner.

use checked_trees::{
    CheckedBooleanExpression as Boolean, CheckedIntegerBinaryKind,
    CheckedIntegerComparisonKind as Comparison, CheckedScalarExpression as Scalar,
};
use typed_trees::domain::{DomainDefinition, ProofFact, ProofMembershipFact};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::types::PrimitiveType;

/// The exact declared carrier of a membership whose complete meaning fits this
/// predicate package. Indexed and routed theory needs its separate evidence
/// transport; declining it here must never drop only the unsupported rows.
pub fn scalar_membership_carrier(
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
    membership: &ProofMembershipFact,
    subject: &Scalar,
) -> Option<Vec<Boolean>> {
    let mut active = Vec::new();
    project(program, membership, subject, &mut active)
}

fn project(
    program: &dyn crate::CheckingInput,
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
        domain,
        subject,
        primitive,
    };
    let mut predicates = Vec::new();
    for fact in program.proof_facts.span(domain.facts)? {
        match fact {
            ProofFact::Expression(expression) => predicates.push(spelling.boolean(*expression, 0)?),
            ProofFact::Membership(nested) => {
                if crate::exact_domain_subject_type(program, domain, nested.value).is_none() {
                    return None;
                }
                predicates.extend(project(program, nested, subject, active)?);
            }
            ProofFact::Proposition(_) => return None,
        }
    }
    active.pop();
    Some(predicates)
}

struct Predicate<'program> {
    program: &'program dyn crate::CheckingInput,
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
        if matches!(self.primitive, PrimitiveType::F32 | PrimitiveType::F64) {
            return self.float_boolean(expression, depth);
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Boolean(value) => Some(Boolean::Constant(*value)),
            ExpressionNode::Name(_)
                if self.primitive == PrimitiveType::Bool
                    && crate::exact_domain_subject_type(self.program, self.domain, expression)
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
                    left: Box::new(self.scalar(left, depth + 1)?),
                    right: Box::new(self.scalar(right, depth + 1)?),
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

    /// A predicate on `f32` or `f64` is an IEEE comparison, not an integer
    /// relation. The subject is the parameter; the other operand is a landed
    /// literal in that format. Token-lowered comparisons are calls whose
    /// machine binding is the sealed float operator.
    fn float_boolean(&self, expression: ExpressionHandle, depth: usize) -> Option<Boolean> {
        if depth >= 128 {
            return None;
        }
        if let Some(binary) = self.program.expression_table.binary_node(expression)
            && matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
        {
            let left = Box::new(self.float_boolean(binary.left, depth + 1)?);
            let right = Box::new(self.float_boolean(binary.right, depth + 1)?);
            return Some(if binary.operator == BinaryOperator::And {
                Boolean::And { left, right }
            } else {
                Boolean::Or { left, right }
            });
        }
        let (operator, left, right) = self.float_comparison(expression)?;
        let (kind, left, right) = match operator {
            BinaryOperator::Equal => (
                checked_trees::CheckedIeeeFloatComparisonKind::Equal,
                left,
                right,
            ),
            BinaryOperator::NotEqual => (
                checked_trees::CheckedIeeeFloatComparisonKind::NotEqual,
                left,
                right,
            ),
            BinaryOperator::Less => (
                checked_trees::CheckedIeeeFloatComparisonKind::LessThan,
                left,
                right,
            ),
            BinaryOperator::LessOrEqual => (
                checked_trees::CheckedIeeeFloatComparisonKind::LessOrEqual,
                left,
                right,
            ),
            BinaryOperator::Greater => (
                checked_trees::CheckedIeeeFloatComparisonKind::LessThan,
                right,
                left,
            ),
            BinaryOperator::GreaterOrEqual => (
                checked_trees::CheckedIeeeFloatComparisonKind::LessOrEqual,
                right,
                left,
            ),
            _ => return None,
        };
        Some(Boolean::ScalarIeeeFloatComparison {
            kind,
            left: Box::new(self.float_scalar(left)?),
            right: Box::new(self.float_scalar(right)?),
        })
    }

    fn float_comparison(
        &self,
        expression: ExpressionHandle,
    ) -> Option<(BinaryOperator, ExpressionHandle, ExpressionHandle)> {
        if let Some(binary) = self.program.expression_table.binary_node(expression)
            && matches!(
                binary.operator,
                BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
            )
        {
            return Some((binary.operator, binary.left, binary.right));
        }
        let call = self.program.expression_table.call_node(expression)?;
        let (machine, _) = crate::find_machine_by_entry_state(self.program, call.target_symbol)?;
        let binding = self
            .program
            .machine_token_binding_by_symbol(machine.symbol)?;
        let (operator, format) =
            typed_trees::operator::primitive_float_binary_semantics(self.program, binding)?;
        let expected = match self.primitive {
            PrimitiveType::F32 => numerics::literals::FloatFormat::F32,
            PrimitiveType::F64 => numerics::literals::FloatFormat::F64,
            _ => return None,
        };
        if format != expected {
            return None;
        }
        let [left, right] = self
            .program
            .expression_table
            .expression_handles(call.arguments)
        else {
            return None;
        };
        Some((operator, *left, *right))
    }

    fn float_scalar(&self, expression: ExpressionHandle) -> Option<Scalar> {
        if crate::exact_domain_subject_type(self.program, self.domain, expression).is_some() {
            return Some(self.subject.clone());
        }
        let literal = self.program.expression_table.expression(expression);
        let bits = match literal {
            ExpressionNode::Float(literal) => match self.primitive {
                PrimitiveType::F32 => {
                    semantic_vocabulary::IeeeFloatValue::Binary32(literal.value_f32().to_bits())
                }
                PrimitiveType::F64 => {
                    semantic_vocabulary::IeeeFloatValue::Binary64(literal.value_f64().to_bits())
                }
                _ => return None,
            },
            _ => return None,
        };
        Some(Scalar::IeeeFloatLiteral { value: bits })
    }

    fn scalar(&self, expression: ExpressionHandle, depth: usize) -> Option<Scalar> {
        if depth >= 128
            || !self.primitive.accepts_integer_literal()
            || self.primitive == PrimitiveType::Addr
        {
            return None;
        }
        if crate::exact_domain_subject_type(self.program, self.domain, expression).is_some() {
            return Some(self.subject.clone());
        }
        if let ExpressionNode::Binary(binary) = self.program.expression_table.expression(expression)
        {
            // These operators have no authored overload and are total at the
            // common fixed carrier. Preserve their children as expressions;
            // a domain is not restricted to comparing its bare subject.
            let kind = match binary.operator {
                BinaryOperator::BitwiseAnd => Some(CheckedIntegerBinaryKind::BitwiseAnd),
                BinaryOperator::BitwiseOr => Some(CheckedIntegerBinaryKind::BitwiseOr),
                BinaryOperator::BitwiseXor => Some(CheckedIntegerBinaryKind::BitwiseXor),
                _ => None,
            };
            if let Some(kind) = kind {
                let reference = crate::value_custody::expression_types::domain_expression_result_type_reference(
                    self.program, self.domain, expression,
                )?;
                if self.program.primitive_type_reference(reference) != Some(self.primitive) {
                    return None;
                }
                return Some(Scalar::IntegerBinary {
                    kind,
                    primitive_type: self.primitive,
                    left: Box::new(self.scalar(binary.left, depth + 1)?),
                    right: Box::new(self.scalar(binary.right, depth + 1)?),
                    formation_decided: false,
                });
            }
        }
        // Literals, typed constants and exact closed casts share the existing
        // declaration-owned numeric query. Never relabel an already-landed
        // incompatible carrier or evaluate a runtime name as a constant.
        let value = self
            .program
            .closed_integer_value_in(expression, self.domain.symbol)?;
        if value
            .primitive
            .is_some_and(|primitive| primitive != self.primitive)
        {
            return None;
        }
        Some(Scalar::IntegerLiteral {
            literal: crate::land_integer_value(&value.value, self.primitive)?,
        })
    }
}

/// Reconstruct a field restriction from its exact declaration. Closed numeric
/// operands retain their own carriers and selected operator meanings.
/// This is source authority, independent of a producer's retained field plan.
/// Compiler-known value refinements use the same subject-bound meaning as
/// callable contracts. Projecting that meaning does not prove membership or
/// change an invariant-window fact into a per-write storage restriction.
pub fn scalar_field_domain_predicate(
    program: &dyn crate::CheckingInput,
    constraint: &typed_trees::types::DomainConstraint,
    primitive: PrimitiveType,
) -> Option<Boolean> {
    scalar_domain_predicate(
        program,
        constraint,
        Scalar::Parameter {
            position: 0,
            primitive_type: primitive,
        },
    )
}

/// Project the complete declaration onto the caller's exact scalar subject.
/// Fields and value contracts share this decoder; projection grants no membership.
pub fn scalar_domain_predicate(
    program: &dyn crate::CheckingInput,
    constraint: &typed_trees::types::DomainConstraint,
    subject: Scalar,
) -> Option<Boolean> {
    let primitive = subject.primitive_type()?;
    if let typed_trees::types::DomainConstraintSubject::Value(domain) = constraint.subject {
        return crate::scalar_value_domain_predicate(domain, subject);
    }
    // An exact interval summary states the entire domain. Project it through
    // the same predicate route as other domains, retaining its closed bounds
    // rather than reconstructing selectable source tokens.
    if let Some((minimum, maximum)) =
        crate::exact_declared_domain_carrier_interval(program, primitive, constraint)
    {
        if minimum > maximum {
            return None;
        }
        let endpoint = |value: &numerics::bignum::BigInt| {
            Some(Scalar::IntegerLiteral {
                literal: crate::land_integer_value(value, primitive)?,
            })
        };
        return Some(conjunction(
            Boolean::IntegerComparison {
                kind: Comparison::LessOrEqual,
                left: Box::new(endpoint(&minimum)?),
                right: Box::new(subject.clone()),
            },
            Boolean::IntegerComparison {
                kind: Comparison::LessOrEqual,
                left: Box::new(subject),
                right: Box::new(endpoint(&maximum)?),
            },
        ));
    }
    // Static tag identity and indexed arguments remain on the checked type.
    // With no predicates or routes, its storage restriction is exactly true;
    // use the same exact-instance classification as scalar signatures rather
    // than treating every indexed family as an unsupported predicate theory.
    if super::scalar_tags::scalar_tag_constraint(program, constraint, primitive) {
        return Some(Boolean::Constant(true));
    }
    if constraint.subject != typed_trees::types::DomainConstraintSubject::Declared
        || !constraint.arguments.is_empty()
        || !constraint.establishment_routes.is_empty()
    {
        return None;
    }
    let membership = ProofMembershipFact {
        domain_symbol: constraint.symbol,
        semantic_domain: constraint.semantic_id,
        ..ProofMembershipFact::default()
    };
    if scalar_membership_carrier(program, &membership)? != primitive {
        return None;
    }
    let predicates = project(program, &membership, &subject, &mut Vec::new())?;
    Some(
        predicates
            .into_iter()
            .reduce(conjunction)
            .unwrap_or(Boolean::Constant(true)),
    )
}

fn conjunction(left: Boolean, right: Boolean) -> Boolean {
    Boolean::And {
        left: Box::new(left),
        right: Box::new(right),
    }
}

/// Assemble the complete restriction after source reconstruction has intersected
/// interval shells. A general predicate keeps both endpoints; interval-only
/// fields retain their compact representation.
pub fn scalar_field_restriction(
    primitive: PrimitiveType,
    bounds: Option<semantic_vocabulary::BoundedIntegerType>,
    predicate: Option<Boolean>,
) -> Option<checked_trees::CheckedUnitStructuralFieldType> {
    use checked_trees::CheckedUnitStructuralFieldType as Field;
    let Some(mut predicate) = predicate else {
        return Some(bounds.map_or(Field::Scalar(primitive), Field::BoundedInteger));
    };
    if let Some(bounds) = bounds {
        let subject = Scalar::Parameter {
            position: 0,
            primitive_type: primitive,
        };
        let endpoint = |value| {
            use numerics::bignum::BigInt;
            let value = match value {
                semantic_vocabulary::IntegerValue::Signed(value) => BigInt::from_i128(value),
                semantic_vocabulary::IntegerValue::Unsigned(value) => BigInt::from_u128(value),
            };
            Some(Scalar::IntegerLiteral {
                literal: crate::land_integer_value(&value, primitive)?,
            })
        };
        predicate = conjunction(
            predicate,
            Boolean::And {
                left: Box::new(Boolean::IntegerComparison {
                    kind: Comparison::LessOrEqual,
                    left: Box::new(endpoint(bounds.minimum())?),
                    right: Box::new(subject.clone()),
                }),
                right: Box::new(Boolean::IntegerComparison {
                    kind: Comparison::LessOrEqual,
                    left: Box::new(subject),
                    right: Box::new(endpoint(bounds.maximum())?),
                }),
            },
        );
    }
    Some(Field::RefinedScalar {
        primitive,
        predicate,
    })
}
