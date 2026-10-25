//! Proposition construction: comparisons, connectives, and case sums
//! become `Type 0` propositions in the bounded vocabulary — `Id` over the
//! operand carrier, right-nested `Σ`, tagged sums, and boolean literals.

use proof_admission::{Term, TermHandle, shift};
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, UnaryOperator,
};

use super::{Elaborator, ScalarCarrier};

impl<'a> Elaborator<'a> {
    /// is a proposition form at all — `None` names an ordinary term and
    /// falls back to `elaborate_expression`, while `Err` is a refused
    /// proposition. Comparisons need a shared scalar carrier: integer
    /// orders (`<`, `<=`, `>`, `>=`) denote `IntLt`/`IntLe` over the
    /// shared `Int`, `==` denotes `Id` over the operands' carrier, `!=`
    /// denotes `Not` applied to the same `Id`, `&&`
    /// denotes a right-nested `Σ`, `||` the tagged `Two` sum, `true` and
    /// `false` the `Two` identities, and any `bool`-carried subject — a
    /// name, a call result, a field or a cast — means
    /// `subject = true`, the same proposition `lower_proposition` forms.
    pub(super) fn elaborate_proposition(
        &mut self,
        handle: ExpressionHandle,
    ) -> Result<Option<TermHandle>, Vec<diagnostics::Diagnostic>> {
        match self.program.expression_table.expression(handle) {
            ExpressionNode::Boolean(value) => Ok(Some(self.boolean_proposition(*value))),
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                // Preserve the operand's selected proposition meaning before
                // negating it. An opaque Boolean for the whole expression
                // loses integer bounds when a named predicate is applied.
                // IEEE/custom comparisons still denote opaque operations in
                // their own branches; negation gives them no integer law.
                let operand = unary.operand;
                self.elaborate_proposition(operand)
                    .map(|proposition| proposition.map(|term| self.negated_proposition(term)))
            }
            ExpressionNode::Name(..)
            | ExpressionNode::Call(..)
            | ExpressionNode::Unary(..)
            | ExpressionNode::Member(..)
            | ExpressionNode::Cast(..)
            | ExpressionNode::Indexed(..) => self.subject_proposition(handle),
            ExpressionNode::Binary(binary) => {
                let binary = *binary;
                // A resolved ordinary call can introduce a data carrier only
                // through its declared codomain. Prepare operand domains
                // before classifying the relation, just as parameter domains
                // are prepared before elaborating a definition's body. This
                // retains source type obligations without giving calls a law.
                self.source_expression_domain(binary.left)?;
                self.source_expression_domain(binary.right)?;
                match binary.operator {
                    BinaryOperator::Equal => self.equality_proposition(handle, binary).map(Some),
                    BinaryOperator::NotEqual => {
                        self.inequality_proposition(handle, binary).map(Some)
                    }
                    BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual => {
                        self.order_proposition(handle, binary).map(Some)
                    }
                    BinaryOperator::And | BinaryOperator::Or => self
                        .connective_proposition(handle, binary.operator)
                        .map(Some),
                    _ => Ok(None),
                }
            }
            _ => self.subject_proposition(handle),
        }
    }

    /// A bare `bool`-carried subject — a name, call result, `!b`, field
    /// projection or cast — denotes `Id bool subject true`; carriers
    /// that are not `bool` are ordinary terms, not propositions.
    pub(super) fn subject_proposition(
        &mut self,
        handle: ExpressionHandle,
    ) -> Result<Option<TermHandle>, Vec<diagnostics::Diagnostic>> {
        match self.operand_carrier(handle) {
            Some(ScalarCarrier::Boolean) => {
                let subject = self.scalar_operand_term(handle, ScalarCarrier::Boolean)?;
                let ty = self.carrier_term(ScalarCarrier::Boolean);
                let truth = self.boolean_literal(true);
                Ok(Some(self.arena.insert(Term::Id {
                    ty,
                    left: subject,
                    right: truth,
                })))
            }
            _ => Ok(None),
        }
    }

    /// `l != r : Type 0` — `Not (Id S l r)`, reusing the shared-carrier
    /// equality endpoints so `x != y` and `y != x` order canonically the
    /// way `x == y` and `y == x` do. `Not` is an opaque assumption at
    /// `Type 0`, not the kernel's `sEmpty` elimination, so the negated
    /// proposition composes inside `&&`/`||` without leaving the `Type 0`
    /// vocabulary.
    pub(super) fn inequality_proposition(
        &mut self,
        handle: ExpressionHandle,
        binary: TableBinaryExpression,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        let carrier = self
            .operand_carrier(binary.left)
            .or_else(|| self.operand_carrier(binary.right));
        if let Some(term) = self.float_comparison(handle, binary, carrier)? {
            return Ok(term);
        }
        let equality = self.equality_proposition(handle, binary)?;
        Ok(self.negated_proposition(equality))
    }

    /// Logical `!` and `!=` share the existing proposition-negation
    /// vocabulary. Consumers retain its polarity and exact operand instead
    /// of reconstructing an authored definition or assuming a new fact.
    fn negated_proposition(&mut self, proposition: TermHandle) -> TermHandle {
        let position = self.proposition_negation();
        let function = self.constant(position);
        self.arena.insert(Term::Apply {
            function,
            argument: proposition,
        })
    }

    /// `l = r : Type 0` — `Id` over the operands' shared scalar carrier.
    /// `Int` serves every fixed-width integer comparison, `bool` and
    /// `addr` their own carriers, and an interned `data` carrier serves
    /// structural equality — the generic `Id S` route the bounded
    /// denotation keeps for non-integer scalars. Anonymous literals adopt
    /// the other operand's carrier; distinct determined carriers refuse.
    pub(super) fn equality_proposition(
        &mut self,
        handle: ExpressionHandle,
        binary: TableBinaryExpression,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        let carrier = self
            .operand_carrier(binary.left)
            .or_else(|| self.operand_carrier(binary.right))
            .ok_or_else(|| {
                self.refuse(format!(
                    "cannot determine a shared scalar carrier for `{}`",
                    self.program.render_proof_expression(
                        handle,
                        typed_trees::proposition::ProofSubstitutions::None
                    )
                ))
            })?;
        let references = [binary.left, binary.right].map(|operand| {
            validation::mathematical_applications::operand_type_reference(self.program, operand)
        });
        if let Some(term) = self.float_comparison(handle, binary, Some(carrier))? {
            return Ok(term);
        }
        let identity = !matches!(carrier, ScalarCarrier::Float32 | ScalarCarrier::Float64)
            && typed_trees::operator::has_builtin_spelled_expression_meaning(
                self.program,
                self.source_owner,
                handle,
                language_core::OperatorSpelling::Equal,
                &references,
            );
        if !identity {
            // IEEE comparison and custom operators produce a Boolean. Typing
            // their proposition does not authorize reflexive resident-value
            // identity, including the NaN case of x == x.
            let left = self.open_term(handle)?;
            let right = self.boolean_literal(true);
            let ty = self.carrier_term(ScalarCarrier::Boolean);
            return Ok(self.arena.insert(Term::Id { ty, left, right }));
        }
        let mut left = self.scalar_operand_term(binary.left, carrier)?;
        let mut right = self.scalar_operand_term(binary.right, carrier)?;
        // `Id` endpoints order canonically — `x == y` and `y == x`
        // denote one type. The bounded denotation canonicalizes at the
        // proposition level; signatures carry no proposition stage, so
        // the kernel terms themselves order here.
        if self.term_order_key(left) > self.term_order_key(right) {
            std::mem::swap(&mut left, &mut right);
        }
        let ty = self.carrier_term(carrier);
        Ok(self.arena.insert(Term::Id { ty, left, right }))
    }

    /// `IntLt l r` or `IntLe l r : Type 0` — the shared order relations,
    /// reserved for fixed non-address integer operands exactly as the
    /// bounded denotation's `fixed_scalar_relation` gates them. `>` and
    /// `>=` denote the flipped form.
    pub(super) fn order_proposition(
        &mut self,
        handle: ExpressionHandle,
        binary: TableBinaryExpression,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        let carrier = self
            .operand_carrier(binary.left)
            .or_else(|| self.operand_carrier(binary.right));
        let references = [binary.left, binary.right].map(|operand| {
            validation::mathematical_applications::operand_type_reference(self.program, operand)
        });
        if let Some(term) = self.float_comparison(handle, binary, carrier)? {
            return Ok(term);
        }
        let spelling = match binary.operator {
            BinaryOperator::Less => language_core::OperatorSpelling::Less,
            BinaryOperator::LessOrEqual => language_core::OperatorSpelling::LessEqual,
            BinaryOperator::Greater => language_core::OperatorSpelling::Greater,
            BinaryOperator::GreaterOrEqual => language_core::OperatorSpelling::GreaterEqual,
            _ => unreachable!("order relation"),
        };
        if carrier != Some(ScalarCarrier::Integer)
            || !typed_trees::operator::has_builtin_spelled_expression_meaning(
                self.program,
                self.source_owner,
                handle,
                spelling,
                &references,
            )
        {
            let left = self.open_term(handle)?;
            let right = self.boolean_literal(true);
            let ty = self.carrier_term(ScalarCarrier::Boolean);
            return Ok(self.arena.insert(Term::Id { ty, left, right }));
        }
        let left = self.integer_operand(binary.left)?;
        let right = self.integer_operand(binary.right)?;
        let (strict, left, right) = match binary.operator {
            BinaryOperator::Less => (true, left, right),
            BinaryOperator::LessOrEqual => (false, left, right),
            BinaryOperator::Greater => (true, right, left),
            _ => (false, right, left),
        };
        let position = if strict {
            self.integer_less_than()
        } else {
            self.integer_less_or_equal()
        };
        let relation = self.constant(position);
        Ok(self.arena.apply_binary(relation, left, right))
    }

    /// Retain a selected IEEE law beside its typed application. This is an
    /// opaque relation to the kernel: NaN never acquires reflexive Id. Source
    /// predicate consumers may use its positive ordered bounds independently.
    fn float_comparison(
        &mut self,
        handle: ExpressionHandle,
        binary: TableBinaryExpression,
        carrier: Option<ScalarCarrier>,
    ) -> Result<Option<TermHandle>, Vec<diagnostics::Diagnostic>> {
        let (carrier, format) = match carrier {
            Some(ScalarCarrier::Float32) => {
                (ScalarCarrier::Float32, numerics::literals::FloatFormat::F32)
            }
            Some(ScalarCarrier::Float64) => {
                (ScalarCarrier::Float64, numerics::literals::FloatFormat::F64)
            }
            _ => return Ok(None),
        };
        let references = [binary.left, binary.right].map(|operand| {
            validation::mathematical_applications::operand_type_reference(self.program, operand)
        });
        if !validation::has_builtin_or_sealed_float_comparison_meaning(
            self.program,
            self.source_owner,
            handle,
            binary.operator,
            &references,
        ) {
            return Ok(None);
        }
        let Some(reference) = references.into_iter().flatten().find(|reference| {
            matches!(
                (self.program.primitive_type_reference(*reference), format),
                (
                    Some(typed_trees::types::PrimitiveType::F32),
                    numerics::literals::FloatFormat::F32
                ) | (
                    Some(typed_trees::types::PrimitiveType::F64),
                    numerics::literals::FloatFormat::F64
                )
            )
        }) else {
            return Ok(None);
        };
        let existing = self
            .prepared
            .constants
            .iter()
            .find_map(|(_, row)| match row.meaning {
                super::PreparedConstantMeaning::FloatRelation {
                    operator,
                    format: retained,
                    reference: retained_reference,
                } if operator == binary.operator
                    && retained == format
                    && retained_reference == reference =>
                {
                    Some(row.declaration)
                }
                _ => None,
            });
        let position = if let Some(position) = existing {
            position
        } else {
            let domain = self.carrier_term(carrier);
            let codomain = self.type_zero();
            let inner = self.arena.insert(Term::Pi { domain, codomain });
            let ty = self.arena.insert(Term::Pi {
                domain,
                codomain: inner,
            });
            let position = self.push_assumption(ty);
            self.prepared.constants.append(super::PreparedConstant {
                declaration: position,
                meaning: super::PreparedConstantMeaning::FloatRelation {
                    operator: binary.operator,
                    format,
                    reference,
                },
            });
            position
        };
        let left = self.scalar_operand_term(binary.left, carrier)?;
        let right = self.scalar_operand_term(binary.right, carrier)?;
        let relation = self.constant(position);
        Ok(Some(self.arena.apply_binary(relation, left, right)))
    }

    /// `a && b` denotes a right-nested `Σ` and `a || b` the tagged
    /// `Σ(t : Two). caseTwo` sum — the connective denotations of the
    /// bounded vocabulary. Operand spines flatten and order canonically,
    /// so `a && b` and `b && a` denote one type.
    pub(super) fn connective_proposition(
        &mut self,
        handle: ExpressionHandle,
        operator: BinaryOperator,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        let mut operands = Vec::new();
        self.proposition_spine(handle, operator, &mut operands);
        let mut terms = Vec::with_capacity(operands.len());
        for operand in operands {
            match self.elaborate_proposition(operand)? {
                Some(term) => terms.push(term),
                None => {
                    return Err(self.refuse(format!(
                        "operand `{}` of a proposition connective is not a proposition",
                        self.program.render_proof_expression(
                            operand,
                            typed_trees::proposition::ProofSubstitutions::None
                        )
                    )));
                }
            }
        }
        terms.sort_by_key(|term| self.term_order_key(*term));
        Ok(match operator {
            BinaryOperator::And => self.right_nested_sigma(&terms, 0),
            _ => self.tagged_sum(&terms, 0),
        })
    }

    /// Flatten a left-associative `&&`/`||` spine into authored order.
    pub(super) fn proposition_spine(
        &self,
        handle: ExpressionHandle,
        operator: BinaryOperator,
        operands: &mut Vec<ExpressionHandle>,
    ) {
        match self.program.expression_table.expression(handle) {
            ExpressionNode::Binary(binary) if binary.operator == operator => {
                self.proposition_spine(binary.left, operator, operands);
                self.proposition_spine(binary.right, operator, operands);
            }
            _ => operands.push(handle),
        }
    }

    /// `Σ(P, λ_. Q)` right-nested over `terms`, each denoted in the
    /// ambient scope: `Σ` binds `Variable 0`, so every later term shifts
    /// under the binders before it.
    pub(super) fn right_nested_sigma(&mut self, terms: &[TermHandle], under: u32) -> TermHandle {
        let domain = shift(&mut self.arena, terms[0], 0, under);
        let codomain = if terms.len() == 2 {
            shift(&mut self.arena, terms[1], 0, under + 1)
        } else {
            self.right_nested_sigma(&terms[1..], under + 1)
        };
        self.arena.insert(Term::Sigma { domain, codomain })
    }

    /// `Σ(t : Two). caseTwo(λ(_ : Two). Type 0, zero, rest, t)` — the
    /// binary coproduct the disjunction denotation nests, matching the
    /// bounded vocabulary's own `tagged_sum` shape.
    pub(super) fn tagged_sum(&mut self, terms: &[TermHandle], under: u32) -> TermHandle {
        let two = self.arena.insert(Term::Two);
        let type_zero = self.type_zero();
        let motive = self.arena.insert(Term::Lambda {
            domain: two,
            body: type_zero,
        });
        let scrutinee = self.arena.insert(Term::Variable(0));
        let zero_branch = shift(&mut self.arena, terms[0], 0, under + 1);
        let one_branch = if terms.len() == 2 {
            shift(&mut self.arena, terms[1], 0, under + 1)
        } else {
            self.tagged_sum(&terms[1..], under + 1)
        };
        let family = self.arena.insert(Term::CaseTwo {
            motive,
            zero_branch,
            one_branch,
            scrutinee,
        });
        self.arena.insert(Term::Sigma {
            domain: two,
            codomain: family,
        })
    }

    /// `true` denotes `Id Two zero zero` and `false` `Id Two zero one` —
    /// the bounded `Truth`/`Falsehood` denotations.
    pub(super) fn boolean_proposition(&mut self, value: bool) -> TermHandle {
        let ty = self.arena.insert(Term::Two);
        let zero = self.arena.insert(Term::TwoZero);
        let right = if value {
            zero
        } else {
            self.arena.insert(Term::TwoOne)
        };
        self.arena.insert(Term::Id {
            ty,
            left: zero,
            right,
        })
    }
}
