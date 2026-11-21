use crate::{LoweringError, LoweringTypes, QualificationDirection, ScalarType};
use omega_resolved::DeclarationId;
use omega_typed::{Access, ObligationKind, TypedOperation, TypingObligation, ValueType};

/// Inputs to the scalar view compatibility judgment, not a judgment result.
/// Semantic integer widths and Boolean validity do not specify storage layout.
pub(super) struct ScalarRecast<'a> {
    pub access: crate::ReferenceAccess,
    pub source: &'a ValueType,
    pub source_referent: ScalarType,
    pub destination_referent: ScalarType,
}

pub(super) fn scalar_recast<'a>(
    operation: &'a TypedOperation,
    obligations: &[&TypingObligation],
    machine: &DeclarationId,
) -> Result<ScalarRecast<'a>, LoweringError> {
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
    let access = match from_access {
        Access::Shared => crate::ReferenceAccess::Shared,
        Access::Mut => crate::ReferenceAccess::Mutable,
        Access::Write => return Err(error("write-only views admit no recast")),
    };
    let scalar = |referent: &ValueType| {
        if matches!(referent, ValueType::Integer { width: Some(0), .. }) {
            return Err(error("recast requires positive referent widths"));
        }
        // A recast only views stored bits, so an IEEE float referent is a
        // stored scalar like any integer; nothing here computes on it.
        crate::lowering::types::stored_scalar(referent).map_err(|_| error("recast requires unqualified scalar referents"))
    };
    let source_referent = scalar(from)?;
    let destination_referent = scalar(to)?;
    if source_referent == destination_referent {
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
    Ok(ScalarRecast {
        access,
        source,
        source_referent,
        destination_referent,
    })
}

pub(super) struct ScalarCast<'a> {
    pub operand_type: &'a ValueType,
    pub convert_integer: bool,
    pub change: Option<ScalarChange>,
}

pub(super) enum ScalarChange {
    Ordinary(QualificationDirection),
    /// Every added domain once, in destination order, with its own origin.
    Completion(Vec<(omega_psi::DeclarationId, omega_psi::CompletionOrigin)>),
}

/// L3 has resolved each named domain and accounted for the written entries.
/// Psi can encode a strict addition or erasure on one same-carrier edge.
/// An ordinary addition introduces only predicate-free tags, so an addition
/// that includes a predicate names every added domain with its origin instead.
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
    let removes = before.iter().any(|atom| !after.contains(atom));
    let change = match (added.is_empty(), removes) {
        (false, false) => {
            let establishment = |atom: &omega_typed::DomainApplication| types.domain_establishment(&atom.declaration);
            for atom in &added {
                if establishment(atom).is_some_and(|d| d.predicate && d.requires_parent) {
                    return Err(error("predicate completion does not support a domain requiring parent membership"));
                }
            }
            if !added
                .iter()
                .any(|atom| establishment(atom).is_some_and(|d| d.predicate))
            {
                Some(ScalarChange::Ordinary(QualificationDirection::Add))
            } else {
                // encoding.md: a vacuous origin introduces only a predicate-free, route-free tag
                // whose required memberships are empty.
                let mut completions = Vec::with_capacity(added.len());
                for atom in &added {
                    let origin = match establishment(atom) {
                        Some(d) if d.predicate => omega_psi::CompletionOrigin::Proved,
                        Some(d) if d.requires_parent => {
                            return Err(error("vacuous completion does not support a domain requiring parent membership"));
                        }
                        Some(_) => omega_psi::CompletionOrigin::Vacuous,
                        None => return Err(error("cast completion requires a local domain declaration")),
                    };
                    completions.push((crate::identity::declaration(&atom.declaration, &operation.site)?, origin));
                }
                // A completion names a declaration without its application arguments. Until it
                // carries them, a cast that mixes origins refuses every applied domain.
                if completions
                    .iter()
                    .any(|(_, origin)| *origin == omega_psi::CompletionOrigin::Vacuous)
                    && added.iter().any(|atom| !atom.arguments.is_empty())
                {
                    return Err(error("a completion with mixed origins does not support an applied domain"));
                }
                Some(ScalarChange::Completion(completions))
            }
        }
        (true, true) => Some(ScalarChange::Ordinary(QualificationDirection::Erase)),
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
