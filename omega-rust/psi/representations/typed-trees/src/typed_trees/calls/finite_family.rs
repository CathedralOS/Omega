//! Finite signature families, probes and const identities.
//!
//! A trait requirement that declares local const/value binders is a finite
//! generic method family only when its signature `where` clause is one
//! explicit disjunction of complete binder equalities, for example
//! `where Width == 16 || Width == 32`. The roster extracted here is the
//! single authority both the local `dyn` surface and the selected-dispatch
//! boundary consume: duplicates and source order normalize deterministically,
//! correlated `&&` alternatives keep their authored groupings, and partial
//! tuples, opaque predicates, ranges, or mere finite carrier ranges never
//! enumerate.

use crate::TypedTrees;

/// One canonical value in a finite-family tuple. `identity` is the string a
/// `MachineSpecialization` retains in `const_argument_identities` for the
/// same static argument; `display` is diagnostic-only spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyValue {
    pub identity: String,
    pub display: String,
}

/// One complete tuple of a finite family: every const/value binder bound to
/// a closed canonical value, in the signature's binder declaration order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyTuple {
    pub identities: Box<[String]>,
    pub display: Box<[String]>,
}

/// The outcome of reading a generic requirement's signature `where` clause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FamilyProbe {
    /// Not an explicit finite enumeration: no `where` clause, or facts that
    /// are not one disjunction of complete binder equalities. Opaque
    /// predicates, inequality ranges, and independently listed values never
    /// enumerate; the requirement stays dynamically ineligible for the
    /// recorded reason.
    NotFinite(String),
    /// A normalized duplicate-free roster of complete tuples.
    Finite {
        arity: usize,
        tuples: Vec<FamilyTuple>,
    },
}

impl TypedTrees {
    /// Extract the explicit finite family declared by a generic requirement's
    /// signature `where` clause: one disjunction of complete binder equalities
    /// such as `where Width == 16 || Width == 32`. Multi-binder alternatives
    /// must write each tuple's full correlation (`W == 16 && L == 4 || W ==
    /// 32 && L == 8`); the compiler never invents a Cartesian product from
    /// independently listed values.
    pub fn finite_signature_family(
        &self,
        signature: &crate::signature::StateSignature,
    ) -> FamilyProbe {
        let type_parameters = self.state_signature_type_parameters(signature);
        let binders = type_parameters
            .iter()
            .filter(|parameter| {
                matches!(
                    parameter.kind,
                    crate::data::TypeParameterKind::Const { .. }
                        | crate::data::TypeParameterKind::Value { .. }
                )
            })
            .collect::<Vec<_>>();
        if binders.len() != type_parameters.len() {
            return FamilyProbe::NotFinite(
                "the requirement carries non-value generic binders, which value equality cannot enumerate"
                    .to_owned(),
            );
        }
        let facts = self.proof_facts.span_or_empty(signature.where_facts);
        let [fact] = facts else {
            return if facts.is_empty() {
                FamilyProbe::NotFinite("declares no finite-family `where` clause".to_owned())
            } else {
                FamilyProbe::NotFinite(
                    "the finite family must be one explicit disjunction of complete binder equalities"
                        .to_owned(),
                )
            };
        };
        let crate::domain::ProofFact::Expression(root) = fact else {
            return FamilyProbe::NotFinite(
                "the `where` clause is a domain membership, not an explicit finite enumeration"
                    .to_owned(),
            );
        };

        let mut tuples = Vec::new();
        for alternative in self.or_alternatives(*root) {
            let mut assignments: Vec<Option<FamilyValue>> = vec![None; binders.len()];
            for conjunct in self.and_conjuncts(alternative) {
                let Some((binder_index, value)) = self.equality_assignment(&binders, conjunct)
                else {
                    return FamilyProbe::NotFinite(
                        "the `where` clause is not explicit `Binder == literal` equalities; opaque \
                         predicates and inequalities do not enumerate a finite family"
                            .to_owned(),
                    );
                };
                if assignments[binder_index].is_some() {
                    return FamilyProbe::NotFinite(
                        "an alternative binds the same value binder more than once".to_owned(),
                    );
                }
                assignments[binder_index] = Some(value);
            }
            if assignments.iter().any(Option::is_none) {
                return FamilyProbe::NotFinite(
                    "an alternative does not bind every value binder; a finite family requires \
                     complete tuples so authored correlations are preserved"
                        .to_owned(),
                );
            }
            let tuple = assignments
                .into_iter()
                .map(|value| value.expect("complete alternative tuple"))
                .collect::<Vec<_>>();
            tuples.push(FamilyTuple {
                identities: tuple.iter().map(|value| value.identity.clone()).collect(),
                display: tuple.iter().map(|value| value.display.clone()).collect(),
            });
        }
        tuples.sort_by(|left, right| left.identities.cmp(&right.identities));
        tuples.dedup_by(|left, right| left.identities == right.identities);
        FamilyProbe::Finite {
            arity: binders.len(),
            tuples,
        }
    }

    /// Flatten an authored `||` chain into its alternatives.
    fn or_alternatives(
        &self,
        expression: crate::expression::ExpressionHandle,
    ) -> Vec<crate::expression::ExpressionHandle> {
        let mut alternatives = Vec::new();
        let mut stack = vec![expression];
        while let Some(current) = stack.pop() {
            match self.expression_table.expression(current) {
                crate::expression::ExpressionNode::Binary(binary)
                    if binary.operator == crate::expression::BinaryOperator::Or =>
                {
                    stack.push(binary.right);
                    stack.push(binary.left);
                }
                _ => alternatives.push(current),
            }
        }
        alternatives
    }

    /// Flatten an authored `&&` chain into its conjuncts.
    fn and_conjuncts(
        &self,
        expression: crate::expression::ExpressionHandle,
    ) -> Vec<crate::expression::ExpressionHandle> {
        let mut conjuncts = Vec::new();
        let mut stack = vec![expression];
        while let Some(current) = stack.pop() {
            match self.expression_table.expression(current) {
                crate::expression::ExpressionNode::Binary(binary)
                    if binary.operator == crate::expression::BinaryOperator::And =>
                {
                    stack.push(binary.right);
                    stack.push(binary.left);
                }
                _ => conjuncts.push(current),
            }
        }
        conjuncts
    }

    /// Read one `Binder == literal` conjunct against the signature's declared
    /// value binders, in either operand order. Returns the binders'
    /// declaration index and the canonical literal value.
    fn equality_assignment(
        &self,
        binders: &[&crate::data::TypeParameter],
        expression: crate::expression::ExpressionHandle,
    ) -> Option<(usize, FamilyValue)> {
        let crate::expression::ExpressionNode::Binary(binary) =
            self.expression_table.expression(expression)
        else {
            return None;
        };
        if binary.operator != crate::expression::BinaryOperator::Equal {
            return None;
        }
        let (binder, value) = match (
            self.binder_operand(binders, binary.left),
            self.binder_operand(binders, binary.right),
            self.literal_operand(binary.left),
            self.literal_operand(binary.right),
        ) {
            (Some(binder), None, None, Some(value)) | (None, Some(binder), Some(value), None) => {
                (binder, value)
            }
            _ => return None,
        };
        let carrier = match &binders[binder].kind {
            crate::data::TypeParameterKind::Const { type_reference }
            | crate::data::TypeParameterKind::Value { type_reference } => *type_reference,
            _ => return None,
        };
        self.value_fits_carrier(carrier, &value.identity)
            .then_some((binder, value))
    }

    /// Whether the operand names one of the signature's declared value
    /// binders. Signature `where` expressions are not symbol-bound at this
    /// stage; the binder's own declaration name is the scoped reference.
    fn binder_operand(
        &self,
        binders: &[&crate::data::TypeParameter],
        expression: crate::expression::ExpressionHandle,
    ) -> Option<usize> {
        let crate::expression::ExpressionNode::Name(path) =
            self.expression_table.expression(expression)
        else {
            return None;
        };
        let members = self.expression_table.name_path_members(path.members);
        let [member] = members else { return None };
        binders.iter().position(|parameter| {
            parameter.name.as_str() == member.as_str()
                || (path.symbol.is_valid() && parameter.symbol == path.symbol)
        })
    }

    /// Read a closed literal operand: integer, Boolean, or a named const
    /// declaration whose canonical encoding is a scalar.
    fn literal_operand(
        &self,
        expression: crate::expression::ExpressionHandle,
    ) -> Option<FamilyValue> {
        match self.expression_table.expression(expression) {
            crate::expression::ExpressionNode::Integer(literal) => {
                let value = literal
                    .value_i64()
                    .map(i128::from)
                    .or_else(|| literal.value_u64().map(i128::from))?;
                Some(FamilyValue {
                    identity: integer_const_identity(value),
                    display: value.to_string(),
                })
            }
            crate::expression::ExpressionNode::Boolean(value) => Some(FamilyValue {
                identity: canonical_const_identity(
                    "bool",
                    &language_semantics::const_value::CanonicalConstValue::boolean(*value).encoding,
                ),
                display: value.to_string(),
            }),
            crate::expression::ExpressionNode::Name(path) => {
                let members = self.expression_table.name_path_members(path.members);
                let [member] = members else { return None };
                let declaration = self.const_declarations().iter().find(|declaration| {
                    self.symbols.name(declaration.symbol) == member.as_str()
                        || (path.symbol.is_valid() && declaration.symbol == path.symbol)
                })?;
                let value = language_semantics::const_value::CanonicalConstValue::new(
                    self.display_type_reference(declaration.declared_type),
                    declaration.canonical_value_encoding.as_ref()?.clone(),
                    member.as_str(),
                );
                match value.decode_encoding()? {
                    language_semantics::const_value::DecodedCanonicalConstValue::Integer {
                        value,
                        ..
                    } => Some(FamilyValue {
                        identity: integer_const_identity(value),
                        display: value.to_string(),
                    }),
                    language_semantics::const_value::DecodedCanonicalConstValue::Boolean(value) => {
                        Some(FamilyValue {
                            identity: canonical_const_identity(
                                "bool",
                                &language_semantics::const_value::CanonicalConstValue::boolean(
                                    value,
                                )
                                .encoding,
                            ),
                            display: value.to_string(),
                        })
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Carrier legality for one roster literal: integer literals need an
    /// integer carrier whose range contains them; Boolean literals need
    /// `bool`.
    fn value_fits_carrier(
        &self,
        carrier: crate::types::TypeReferenceHandle,
        identity: &str,
    ) -> bool {
        let Some(primitive) = self.primitive_type_reference(carrier) else {
            return false;
        };
        if let Some(value) = identity
            .strip_prefix("named(integer-const(")
            .and_then(|inner| inner.strip_suffix("))"))
            .and_then(|inner| inner.parse::<i128>().ok())
        {
            return primitive_integer_range(primitive)
                .is_some_and(|(low, high)| low <= value && value <= high);
        }
        primitive == crate::types::PrimitiveType::Bool
    }
}

/// The canonical const identity a specialization retains for one closed
/// integer argument: `named(integer-const(v))` in normalized-type-identity
/// terms.
pub fn integer_const_identity(value: i128) -> String {
    format!("named(integer-const({value}))")
}

/// The canonical const identity for one non-integer canonical value:
/// `named(canonical-const(type(T),encoding(E)))`.
pub fn canonical_const_identity(type_name: &str, encoding: &str) -> String {
    fn escape(value: &str) -> String {
        let mut out = String::with_capacity(value.len());
        for character in value.chars() {
            if matches!(character, '\\' | '(' | ')' | ',') {
                out.push('\\');
            }
            out.push(character);
        }
        out
    }
    format!(
        "named(canonical-const(type({}),encoding({})))",
        escape(type_name),
        escape(encoding)
    )
}

fn primitive_integer_range(primitive: crate::types::PrimitiveType) -> Option<(i128, i128)> {
    use crate::types::PrimitiveType::*;
    match primitive {
        Bool | F32 | F64 => None,
        I8 => Some((i128::from(i8::MIN), i128::from(i8::MAX))),
        I16 => Some((i128::from(i16::MIN), i128::from(i16::MAX))),
        I32 => Some((i128::from(i32::MIN), i128::from(i32::MAX))),
        I64 => Some((i128::from(i64::MIN), i128::from(i64::MAX))),
        U8 => Some((0, i128::from(u8::MAX))),
        U16 => Some((0, i128::from(u16::MAX))),
        U32 => Some((0, i128::from(u32::MAX))),
        U64 | Addr => Some((0, i128::from(u64::MAX))),
    }
}
