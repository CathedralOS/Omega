use crate::{LoweringError, LoweringTypes, QualificationDirection};
use omega_resolved::DeclarationId;
use omega_typed::{Access, ObligationKind, TypedOperation, TypingObligation, ValueType};

/// An admitted same-storage view change between unqualified primitive
/// integral referents of one exact semantic width.
pub(super) struct ScalarBitRecast<'a> {
    /// The checked source reference carrier; the operand lowers at this type.
    pub source: &'a ValueType,
    pub width: u64,
    pub source_signed: bool,
    pub destination_signed: bool,
}

/// L3 recorded a typed `Recast` between two references of one access. Psi can
/// carry exactly the case where both referents are unqualified fixed-width
/// integers of equal width: every bit pattern of either carrier is a valid
/// pattern of the other, so the view reinterprets the top bit (sign versus
/// magnitude) without converting, extending or truncating any value.
/// Floats, Booleans, nominal or qualified referents, unequal widths and
/// mutable views stay refused: they need layout, validity, domain or source
/// handback judgments that this handoff cannot carry. The emitted operation
/// is the checker's recast subject; L5 establishes no compatibility fact.
pub(super) fn scalar_bit_recast<'a>(
    operation: &'a TypedOperation,
    obligations: &[&TypingObligation],
    machine: &DeclarationId,
) -> Result<ScalarBitRecast<'a>, LoweringError> {
    let error = |message| LoweringError::new(&operation.site, message);
    let [source] = operation.operands.as_slice() else {
        return Err(error("recast requires one source view"));
    };
    let target = &operation.result;
    let (ValueType::Reference(from_access, from), ValueType::Reference(to_access, to)) = (source, target) else {
        return Err(error("recast requires unqualified source and destination references"));
    };
    if operation.token != "as" || from_access != to_access {
        return Err(error("recast cannot change access authority"));
    }
    match from_access {
        Access::Shared => {}
        Access::Mut => return Err(error("mutable recast needs source handback evidence that Psi cannot carry")),
        Access::Write => return Err(error("write-only views admit no recast")),
    }
    let (
        ValueType::Integer {
            signed: source_signed,
            width: Some(source_width),
        },
        ValueType::Integer {
            signed: destination_signed,
            width: Some(destination_width),
        },
    ) = (from.as_ref(), to.as_ref())
    else {
        return Err(error("recast requires unqualified fixed-width integral referents"));
    };
    if *source_width == 0 || source_width != destination_width {
        return Err(error("recast requires referents of exactly equal semantic width"));
    }
    if source_signed == destination_signed {
        return Err(error("recast between identical carriers has no Psi view change"));
    }
    let mut recast = false;
    for obligation in obligations {
        match &obligation.kind {
            ObligationKind::Recast { source: s, destination: d } if !recast && s == source && d == target => recast = true,
            ObligationKind::ReturnBorrow { machine: owner } if owner == machine => {}
            _ => return Err(error("recast has an unsupported obligation")),
        }
    }
    if !recast {
        return Err(error("recast lacks its typed representation obligation"));
    }
    Ok(ScalarBitRecast {
        source,
        width: *source_width,
        source_signed: *source_signed,
        destination_signed: *destination_signed,
    })
}

pub(super) struct ScalarCast<'a> {
    pub operand_type: &'a ValueType,
    pub convert_integer: bool,
    pub change: Option<QualificationDirection>,
}

/// L3 has resolved each named domain and accounted for the written entries.
/// Psi can encode a strict addition or erasure on one same-carrier edge.
pub(super) fn scalar_cast<'a>(
    operation: &'a TypedOperation,
    obligations: &[&TypingObligation],
    types: &LoweringTypes,
) -> Result<ScalarCast<'a>, LoweringError> {
    let error = |message| LoweringError::new(&operation.site, message);
    let [source] = operation.operands.as_slice() else {
        return Err(error("cast requires one source value"));
    };
    let target = &operation.result;
    let literal = matches!(source, ValueType::Exact { .. });
    if operation.token != "as"
        || crate::lowering::types::scalar_carrier(target.carrier()).is_err()
        || types.id_for(target).is_err()
        || !literal && types.id_for(source).is_err()
    {
        return Err(error("cast requires supported scalar types and domain declarations"));
    }
    let before = source.domains();
    let after = target.domains();
    let added = after
        .iter()
        .filter(|atom| !before.contains(atom))
        .collect::<Vec<_>>();
    if added
        .iter()
        .any(|member| types.has_domain_predicate(&member.declaration))
    {
        return Err(error("adding a predicate-bearing domain requires a retained establishment proof"));
    }
    let removes = before.iter().any(|atom| !after.contains(atom));
    let change = match (added.is_empty(), removes) {
        (false, false) => Some(QualificationDirection::Add),
        (true, true) => Some(QualificationDirection::Erase),
        (true, false) => None,
        (false, true) => return Err(error("Psi has no specified edge for simultaneous domain removal and addition")),
    };
    let convert_integer = !literal && source.carrier() != target.carrier();
    if convert_integer
        && !(before.is_empty()
            && matches!(source, ValueType::Integer { width: Some(_), .. })
            && matches!(target.carrier(), ValueType::Integer { width: Some(_), .. }))
    {
        return Err(error("unsupported scalar carrier conversion"));
    }
    let mut pending = added;
    let mut needs_conversion = convert_integer;
    for obligation in obligations {
        match &obligation.kind {
            ObligationKind::EstablishDomain(domain) => {
                let Some(position) = pending.iter().position(|&atom| atom == domain) else {
                    return Err(error("cast establishment disagrees with its destination domains"));
                };
                pending.swap_remove(position);
            }
            ObligationKind::ExactCast { source: from, destination: to } if needs_conversion && from == source && to == target.carrier() => {
                needs_conversion = false
            }
            _ => return Err(error("cast has an unsupported obligation")),
        }
    }
    if needs_conversion || !pending.is_empty() {
        return Err(error("cast lacks its typed conversion or establishment obligation"));
    }
    Ok(ScalarCast {
        operand_type: if literal { target.carrier() } else { source },
        convert_integer,
        change,
    })
}
