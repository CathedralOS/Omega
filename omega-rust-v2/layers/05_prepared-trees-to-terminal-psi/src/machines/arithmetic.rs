use crate::lowering::applied_tree::LoweringTree;
use num_bigint::BigInt;
use omega_syntax::NodeKind;
use omega_typed::{NodeSemantics, ObligationKind, TypedOperation, TypingObligation, ValueType};

use super::body::{site_at, unsupported_node};
use crate::{ArithmeticPolicy, IntegerOperation, IntegerPrimitive, LoweringError};

/// Typing supplies the folded number. A concrete integer uses the landing on
/// the operator or its enclosing parentheses. Only the result must fit, so
/// `-128` can land on `i8`. Preserve any operand that still needs execution or proof.
pub(crate) fn folded_exact_operator<'a>(
    tree: &'a LoweringTree<'_>,
    node: usize,
    carrier: &ValueType,
    inherited: Option<&'a BigInt>,
) -> Result<Option<&'a BigInt>, LoweringError> {
    let file = tree.resolved().file;
    let Some(folded) = tree.semantics_at(node).find_map(|record| match record {
        NodeSemantics::Operation(op) if matches!(op.result, ValueType::Exact { .. }) => Some(&op.result),
        _ => None,
    }) else {
        return Ok(None);
    };
    let ValueType::Exact { value, fractional_origin } = folded else {
        return Err(unsupported_node(file, node, "exact operator has no integer value"));
    };
    if !value.is_integer() {
        return Err(unsupported_node(file, node, "exact operator has no integer value"));
    }
    let mut landed = inherited.is_some();
    for child in node..file.nodes[node].end {
        if !matches!(file.nodes[child].kind, NodeKind::Literal | NodeKind::Paren | NodeKind::Binary | NodeKind::Unary) {
            return Err(unsupported_node(file, child, "exact operator contains a non-constant operand"));
        }
        for record in tree.semantics_at(child) {
            match record {
                NodeSemantics::Operation(_) => {}
                NodeSemantics::Expression(e) if e.static_argument_slot.is_none() => {}
                NodeSemantics::Landing(landing) if child == node && &landing.ty == carrier => landed = true,
                _ => return Err(unsupported_node(file, child, "exact operator carries an unsupported typed record")),
            }
        }
    }
    // Typing records a landing only after checking fractional history: an
    // implicit integer landing refuses it and an explicit `as` acknowledges it.
    // Without a landing, nothing has admitted that history.
    if fractional_origin.is_some() && !landed {
        return Err(unsupported_node(file, node, "exact operator has no integer value"));
    }
    let value = value.numer();
    if inherited.is_some_and(|outer| outer != value) {
        return Err(LoweringError::new(&site_at(file, node), "conflicting exact numeric landings"));
    }
    if folded != carrier && !(landed && carrier.contains_integer(value)) {
        return Err(unsupported_node(file, node, "exact operator has no integer landing"));
    }
    Ok(Some(value))
}

pub(crate) enum LoweredInteger {
    Ordinary(IntegerOperation),
    Trapping(crate::TrappingIntegerPrimitive),
}

impl LoweredInteger {
    pub(super) fn emit(
        self,
        operands: Vec<crate::ValueId>,
    ) -> crate::OperationKind {
        match self {
            Self::Ordinary(operation) => crate::OperationKind::Integer { operation, operands },
            Self::Trapping(primitive) => crate::OperationKind::TrappingInteger {
                primitive,
                left: operands[0],
                right: operands[1],
            },
        }
    }
}

/// Index formation uses the same primitive contract, but has no runtime trap edge.
pub(crate) fn index_operation(
    operation: &TypedOperation,
    obligation: &TypingObligation,
    owner: &omega_resolved::DeclarationId,
) -> Result<IntegerOperation, LoweringError> {
    match lower_integer(operation, &[obligation], owner)? {
        LoweredInteger::Ordinary(op) => Ok(op),
        LoweredInteger::Trapping(_) => Err(LoweringError::new(&operation.site, "index operation needs a static formation contract")),
    }
}

pub(crate) fn lower_integer(
    operation: &TypedOperation,
    obligations: &[&TypingObligation],
    machine: &omega_resolved::DeclarationId,
) -> Result<LoweredInteger, LoweringError> {
    let error = |message| LoweringError::new(&operation.site, message);
    let token = operation.token.as_str();
    let (policy, token) = if token.len() > 1 {
        match token.as_bytes()[token.len() - 1] {
            b'%' => (Some(ArithmeticPolicy::Wrapping), &token[..token.len() - 1]),
            b'|' => (Some(ArithmeticPolicy::Saturating), &token[..token.len() - 1]),
            b'!' => (None, &token[..token.len() - 1]),
            _ => (Some(ArithmeticPolicy::Exact), token),
        }
    } else {
        (Some(ArithmeticPolicy::Exact), token)
    };
    use ArithmeticPolicy::Exact;
    use IntegerPrimitive::*;
    let primitive = match (token, operation.operands.len()) {
        ("+", 2) => Add,
        ("-", 2) => Subtract,
        ("*", 2) => Multiply,
        ("/", 2) => Divide,
        ("%", 2) => Remainder,
        ("-", 1) if policy == Some(Exact) => Negate,
        ("<<", 2) => ShiftLeft,
        (">>", 2) => ShiftRight,
        ("&", 2) if policy == Some(Exact) => BitwiseAnd,
        ("|", 2) if policy == Some(Exact) => BitwiseOr,
        ("^", 2) if policy == Some(Exact) => BitwiseXor,
        ("~", 1) if policy == Some(Exact) => BitwiseNot,
        ("==", 2) if policy == Some(Exact) => Equal,
        ("!=", 2) if policy == Some(Exact) => NotEqual,
        ("<", 2) if policy == Some(Exact) => LessThan,
        ("<=", 2) if policy == Some(Exact) => LessOrEqual,
        (">", 2) if policy == Some(Exact) => GreaterThan,
        (">=", 2) if policy == Some(Exact) => GreaterOrEqual,
        _ => return Err(error("unsupported typed integer operator")),
    };
    let comparison = matches!(primitive, Equal | NotEqual | LessThan | LessOrEqual | GreaterThan | GreaterOrEqual);
    let shift = matches!(primitive, ShiftLeft | ShiftRight);
    let [first, rest @ ..] = operation.operands.as_slice() else {
        return Err(error("integer operator has no operands"));
    };
    let is_fixed_integer = |carrier: &ValueType| matches!(carrier, ValueType::Integer { width: Some(width), .. } if *width > 0);
    if !operation.operands.iter().all(is_fixed_integer)
        || if comparison {
            operation.result != ValueType::Bool
        } else {
            &operation.result != first
                || (!shift && rest.iter().any(|operand| operand != first))
                || (primitive == Negate && !matches!(first, ValueType::Integer { signed: true, .. }))
        }
    {
        return Err(error("typed integer operator has incompatible carriers"));
    }

    // Typing emits a generic formation question for numeric binary results,
    // including total bitwise operators.
    let mut has_contract = false;
    let mut covers_trap = false;
    for obligation in obligations {
        if obligation.node != operation.node || obligation.site != operation.site {
            return Err(error("integer operation obligation has a different source occurrence"));
        }
        match &obligation.kind {
            ObligationKind::OperationContract { token, operands } if !has_contract && token == &operation.token && operands == &operation.operands => {
                has_contract = true;
            }
            ObligationKind::CrashCoverage {
                machine: owner,
                cause: omega_typed::CrashCause::Trap,
            } if !covers_trap && policy.is_none() && owner == machine => {
                covers_trap = true;
            }
            _ => return Err(LoweringError::new(&obligation.site, "unexpected integer operation obligation")),
        }
    }
    if has_contract != (!comparison && primitive != BitwiseNot) {
        return Err(error("typed integer operation has incomplete formation records"));
    }

    let Some(policy) = policy else {
        if !covers_trap {
            return Err(error("trapping integer operation lacks its occurrence's Trap coverage"));
        }
        use crate::TrappingIntegerPrimitive as Trap;
        let primitive = match primitive {
            Add => Trap::Add,
            Subtract => Trap::Subtract,
            Multiply => Trap::Multiply,
            Divide => Trap::Divide,
            Remainder => Trap::Remainder,
            ShiftLeft => Trap::ShiftLeft,
            ShiftRight => Trap::ShiftRight,
            _ => return Err(error("integer primitive has no trapping contract")),
        };
        return Ok(LoweredInteger::Trapping(primitive));
    };
    let signed = matches!(first, ValueType::Integer { signed: true, .. });
    let questions = IntegerOperation::required_obligations(primitive, policy, signed)
        .ok_or_else(|| error("integer primitive has no formation contract under the selected policy"))?;
    Ok(LoweredInteger::Ordinary(IntegerOperation {
        primitive,
        policy,
        obligations: questions.to_vec(),
    }))
}

/// A primitive IEEE operator on two unqualified operands of one format, or
/// negation of one. Typing selects it by the complete operand tuple and records
/// the formation question only for the four arithmetic operations; the
/// operation itself is total, so nothing else may accompany it. Policy
/// spellings and mixed formats have no float candidate and stop in typing.
pub(crate) fn lower_float(
    operation: &TypedOperation,
    obligations: &[&TypingObligation],
) -> Result<crate::FloatOperation, LoweringError> {
    use crate::FloatOperation::*;
    let error = |message| LoweringError::new(&operation.site, message);
    let [first, rest @ ..] = operation.operands.as_slice() else {
        return Err(error("float operator has no operands"));
    };
    if rest.iter().any(|operand| operand != first) {
        return Err(error("float operator mixes formats"));
    }
    let primitive = match (operation.token.as_str(), operation.operands.len()) {
        ("-", 1) => Negate,
        ("+", 2) => Add,
        ("-", 2) => Subtract,
        ("*", 2) => Multiply,
        ("/", 2) => Divide,
        ("==", 2) => Equal,
        ("!=", 2) => NotEqual,
        ("<", 2) => Less,
        ("<=", 2) => LessEqual,
        (">", 2) => Greater,
        (">=", 2) => GreaterEqual,
        _ => return Err(error("unsupported typed float operator")),
    };
    let arithmetic = matches!(primitive, Add | Subtract | Multiply | Divide);
    let comparison = !arithmetic && primitive != Negate;
    if operation.result != if comparison { ValueType::Bool } else { first.clone() } {
        return Err(error("typed float operator has an incompatible result"));
    }
    let mut has_contract = false;
    for obligation in obligations {
        match &obligation.kind {
            ObligationKind::OperationContract { token, operands }
                if arithmetic && !has_contract && obligation.node == operation.node && token == &operation.token && operands == &operation.operands =>
            {
                has_contract = true
            }
            _ => return Err(LoweringError::new(&obligation.site, "unexpected float operation obligation")),
        }
    }
    if has_contract != arithmetic {
        return Err(error("typed float operation has incomplete formation records"));
    }
    Ok(primitive)
}

/// An anonymous numeric expression that typing landed in `f32` or `f64`, such
/// as `1.5` in `a + 1.5` or `-(1.0 / 3.0)`. Its exact rational value rounds once
/// to the format; preparation publishes those bits, so lowering never rounds.
pub(crate) fn folded_float(
    tree: &LoweringTree<'_>,
    prepared: &omega_prepared::PreparedTrees<'_>,
    node: usize,
    carrier: &ValueType,
) -> Result<Option<crate::IeeeFloatBits>, LoweringError> {
    let format = match carrier {
        ValueType::Float32 => omega_target_contract::FloatFormat::Binary32,
        ValueType::Float64 => omega_target_contract::FloatFormat::Binary64,
        _ => return Ok(None),
    };
    let file = tree.resolved().file;
    if !matches!(file.nodes[node].kind, NodeKind::Literal | NodeKind::Paren | NodeKind::Binary | NodeKind::Unary)
        || !tree
            .semantics_at(node)
            .any(|record| matches!(record, NodeSemantics::Landing(landing) if &landing.ty == carrier))
    {
        return Ok(None);
    }
    for child in node..file.nodes[node].end {
        if !matches!(file.nodes[child].kind, NodeKind::Literal | NodeKind::Paren | NodeKind::Binary | NodeKind::Unary) {
            return Err(unsupported_node(file, child, "float landing contains a non-constant operand"));
        }
        for record in tree.semantics_at(child) {
            match record {
                NodeSemantics::Operation(op) if matches!(op.result, ValueType::Exact { .. }) => {}
                NodeSemantics::Expression(e) if e.static_argument_slot.is_none() => {}
                NodeSemantics::Landing(landing) if child == node && &landing.ty == carrier => {}
                _ => return Err(unsupported_node(file, child, "float landing carries an unsupported typed record")),
            }
        }
    }
    // Preparation lands floats for the package it prepared, keyed by that
    // package's nodes. A library body's node would select another literal.
    if !std::ptr::eq(tree.forest(), prepared.typed()) {
        return Err(unsupported_node(file, node, "float landing has no single prepared value"));
    }
    match prepared.landed_float(file.node_id(node), format) {
        Some(omega_prepared::PreparedFloat::Binary32(bits)) => Ok(Some(crate::IeeeFloatBits::Binary32(bits))),
        Some(omega_prepared::PreparedFloat::Binary64(bits)) => Ok(Some(crate::IeeeFloatBits::Binary64(bits))),
        _ => Err(unsupported_node(file, node, "float landing has no single prepared value")),
    }
}
