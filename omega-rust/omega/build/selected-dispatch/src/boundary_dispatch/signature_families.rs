//! Finite signature families, probes and const identities.

use typed_trees::TypedTrees;

/// One canonical value in a finite-family tuple. `identity` is the string a
/// `MachineSpecialization` retains in `const_argument_identities` for the
/// same static argument; `display` is diagnostic-only spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FamilyValue {
    identity: String,
    display: String,
}

/// One complete tuple of a finite family: every const/value binder bound to
/// a closed canonical value, in the signature's binder declaration order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FamilyTuple {
    pub(crate) identities: Box<[String]>,
    pub(crate) display: Box<[String]>,
}

/// The outcome of reading a generic requirement's signature `where` clause.
pub(crate) enum FamilyProbe {
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

/// Extract the explicit finite family declared by a generic requirement's
/// signature `where` clause: one disjunction of complete binder equalities
/// such as `where Width == 16 || Width == 32`. Multi-binder alternatives must
/// write each tuple's full correlation (`W == 16 && L == 4 || W == 32 && L
/// == 8`); the compiler never invents a Cartesian product from independently
/// listed values.
pub(crate) fn finite_signature_family(
    typed: &TypedTrees,
    signature: &typed_trees::signature::StateSignature,
) -> FamilyProbe {
    let type_parameters = typed.state_signature_type_parameters(signature);
    let binders = type_parameters
        .iter()
        .filter(|parameter| {
            matches!(
                parameter.kind,
                typed_trees::data::TypeParameterKind::Const { .. }
                    | typed_trees::data::TypeParameterKind::Value { .. }
            )
        })
        .collect::<Vec<_>>();
    if binders.len() != type_parameters.len() {
        return FamilyProbe::NotFinite(
            "the requirement carries non-value generic binders, which value equality cannot enumerate"
                .to_owned(),
        );
    }
    let facts = typed.proof_facts.span_or_empty(signature.where_facts);
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
    let typed_trees::domain::ProofFact::Expression(root) = fact else {
        return FamilyProbe::NotFinite(
            "the `where` clause is a domain membership, not an explicit finite enumeration"
                .to_owned(),
        );
    };

    let mut tuples = Vec::new();
    for alternative in or_alternatives(typed, *root) {
        let mut assignments: Vec<Option<FamilyValue>> = vec![None; binders.len()];
        for conjunct in and_conjuncts(typed, alternative) {
            let Some((binder_index, value)) = equality_assignment(typed, &binders, conjunct) else {
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
    typed: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> Vec<typed_trees::expression::ExpressionHandle> {
    let mut alternatives = Vec::new();
    let mut stack = vec![expression];
    while let Some(current) = stack.pop() {
        match typed.expression_table.expression(current) {
            typed_trees::expression::ExpressionNode::Binary(binary)
                if binary.operator == typed_trees::expression::BinaryOperator::Or =>
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
    typed: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> Vec<typed_trees::expression::ExpressionHandle> {
    let mut conjuncts = Vec::new();
    let mut stack = vec![expression];
    while let Some(current) = stack.pop() {
        match typed.expression_table.expression(current) {
            typed_trees::expression::ExpressionNode::Binary(binary)
                if binary.operator == typed_trees::expression::BinaryOperator::And =>
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
/// value binders, in either operand order. Returns the binders' declaration
/// index and the canonical literal value.
fn equality_assignment(
    typed: &TypedTrees,
    binders: &[&typed_trees::data::TypeParameter],
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<(usize, FamilyValue)> {
    let typed_trees::expression::ExpressionNode::Binary(binary) =
        typed.expression_table.expression(expression)
    else {
        return None;
    };
    if binary.operator != typed_trees::expression::BinaryOperator::Equal {
        return None;
    }
    let (binder, value) = match (
        binder_operand(typed, binders, binary.left),
        binder_operand(typed, binders, binary.right),
        literal_operand(typed, binary.left),
        literal_operand(typed, binary.right),
    ) {
        (Some(binder), None, None, Some(value)) | (None, Some(binder), Some(value), None) => {
            (binder, value)
        }
        _ => return None,
    };
    let carrier = match &binders[binder].kind {
        typed_trees::data::TypeParameterKind::Const { type_reference }
        | typed_trees::data::TypeParameterKind::Value { type_reference } => *type_reference,
        _ => return None,
    };
    value_fits_carrier(typed, carrier, &value.identity).then_some((binder, value))
}

/// Whether the operand names one of the signature's declared value binders.
/// Signature `where` expressions are not symbol-bound at this stage; the
/// binder's own declaration name is the scoped reference.
fn binder_operand(
    typed: &TypedTrees,
    binders: &[&typed_trees::data::TypeParameter],
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<usize> {
    let typed_trees::expression::ExpressionNode::Name(path) =
        typed.expression_table.expression(expression)
    else {
        return None;
    };
    let members = typed.expression_table.name_path_members(path.members);
    let [member] = members else { return None };
    binders.iter().position(|parameter| {
        parameter.name.as_str() == member.as_str()
            || (path.symbol.is_valid() && parameter.symbol == path.symbol)
    })
}

/// Read a closed literal operand: integer, Boolean, or a named const
/// declaration whose canonical encoding is a scalar.
fn literal_operand(
    typed: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<FamilyValue> {
    match typed.expression_table.expression(expression) {
        typed_trees::expression::ExpressionNode::Integer(literal) => {
            let value = literal
                .value_i64()
                .map(i128::from)
                .or_else(|| literal.value_u64().map(i128::from))?;
            Some(FamilyValue {
                identity: integer_const_identity(value),
                display: value.to_string(),
            })
        }
        typed_trees::expression::ExpressionNode::Boolean(value) => Some(FamilyValue {
            identity: canonical_const_identity(
                "bool",
                &language_semantics::const_value::CanonicalConstValue::boolean(*value).encoding,
            ),
            display: value.to_string(),
        }),
        typed_trees::expression::ExpressionNode::Name(path) => {
            let members = typed.expression_table.name_path_members(path.members);
            let [member] = members else { return None };
            let declaration = typed.const_declarations().iter().find(|declaration| {
                typed.symbols.name(declaration.symbol) == member.as_str()
                    || (path.symbol.is_valid() && declaration.symbol == path.symbol)
            })?;
            let value = language_semantics::const_value::CanonicalConstValue::new(
                typed.display_type_reference(declaration.declared_type),
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
                            &language_semantics::const_value::CanonicalConstValue::boolean(value)
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

/// The canonical const identity a specialization retains for one closed
/// integer argument: `named(integer-const(v))` in normalized-type-identity
/// terms.
fn integer_const_identity(value: i128) -> String {
    format!("named(integer-const({value}))")
}

/// The canonical const identity for one non-integer canonical value:
/// `named(canonical-const(type(T),encoding(E)))`.
fn canonical_const_identity(type_name: &str, encoding: &str) -> String {
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

/// Carrier legality for one roster literal: integer literals need an integer
/// carrier whose range contains them; Boolean literals need `bool`.
fn value_fits_carrier(
    typed: &TypedTrees,
    carrier: typed_trees::types::TypeReferenceHandle,
    identity: &str,
) -> bool {
    let Some(primitive) = typed.primitive_type_reference(carrier) else {
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
    primitive == typed_trees::types::PrimitiveType::Bool
}

fn primitive_integer_range(primitive: typed_trees::types::PrimitiveType) -> Option<(i128, i128)> {
    use typed_trees::types::PrimitiveType::*;
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

/// The canonical const identity one call-site static machine argument
/// contributes to a family tuple, or `None` when the argument is not a
/// closed static value. This is `TypedTrees::static_const_argument_identity`,
/// which mirrors the specialization pipeline's spelling and identity exactly.
pub(crate) fn call_argument_const_identity(
    typed: &TypedTrees,
    argument: &typed_trees::expression::StaticMachineArgument,
) -> Option<String> {
    typed.static_const_argument_identity(argument)
}
