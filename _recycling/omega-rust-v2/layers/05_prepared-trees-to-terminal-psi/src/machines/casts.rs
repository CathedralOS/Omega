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
    /// An exact carrier change between two numeric carriers.
    pub convert: bool,
    pub change: Option<ScalarChange>,
}

pub(super) enum ScalarChange {
    Ordinary(QualificationDirection),
    /// Every added domain atom once, in destination order, with its own origin.
    Completion { replaces: bool, domains: Vec<(omega_psi::DomainAtom, omega_psi::CompletionOrigin)> },
}

/// L3 has resolved each named domain and accounted for the written entries.
/// Psi keeps every written change on one same-carrier edge.
/// An ordinary addition introduces only predicate-free tags, so an addition
/// that includes a predicate, and every replacement, names each added domain and its origin.
/// Each row keeps the application's closed arguments, so `Above<5>` owes `self > 5`.
///
/// `machine` owns the cast, so it is the issuer of every domain the cast adds.
/// Whether it may issue a domain that selects an access policy is decided by
/// that policy's executed plan, which lowering does not have: a plan can admit
/// no issuer, and then the domain has no establishment route for any machine
/// (`source-rust/subsystems/compiler/psi/artifact/wire/encoding.md`, domain rows). The package therefore
/// reports each such cast to its driver, and this refuses one it does not report.
pub(super) fn scalar_cast<'a>(
    operation: &'a TypedOperation,
    obligations: &[&TypingObligation],
    types: &LoweringTypes,
    machine: &DeclarationId,
) -> Result<ScalarCast<'a>, LoweringError> {
    let error = |message| LoweringError::new(&operation.site, message);
    let [source] = operation.operands.as_slice() else {
        return Err(error("cast requires one source value"));
    };
    let target = &operation.result;
    let literal = matches!(source, ValueType::Exact { .. });
    if operation.token != "as"
        || crate::lowering::types::stored_scalar(target.carrier()).is_err()
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
    for atom in &added {
        if types
            .domain_establishment(&atom.declaration)
            .is_some_and(|domain| domain.selects_policy)
            && !types.reports_policy_introduction(&atom.declaration, machine, &operation.site)
        {
            return Err(LoweringError::new(
                &operation.site,
                format!(
                    "machine `{}` introduces domain `{}` in a cast its package does not report for the pending access plan",
                    machine.path, atom.declaration.path
                ),
            ));
        }
    }
    let change = match (added.is_empty(), removes) {
        (false, _) => {
            let establishment = |atom: &omega_typed::DomainApplication| types.domain_establishment(&atom.declaration);
            if !removes && !added
                .iter()
                .any(|atom| establishment(atom).is_some_and(|d| d.predicate))
            {
                Some(ScalarChange::Ordinary(QualificationDirection::Add))
            } else {
                // Domain declarations already retain their required atoms in the carrier.
                // Every added atom gets its own row on this argument; retained parents
                // arrive on its input. Nobody acquires a parent from its child’s row.
                let mut completions = Vec::with_capacity(added.len());
                for atom in &added {
                    let origin = match establishment(atom) {
                        Some(d) if d.predicate => omega_psi::CompletionOrigin::Proved,
                        Some(_) => omega_psi::CompletionOrigin::Vacuous,
                        None => return Err(error("cast completion requires a local domain declaration")),
                    };
                    completions.push((LoweringTypes::domain_atom(atom, &operation.site)?, origin));
                }
                Some(ScalarChange::Completion { replaces: removes, domains: completions })
            }
        }
        (true, true) => Some(ScalarChange::Ordinary(QualificationDirection::Erase)),
        (true, false) => None,
    };
    // domains.md#carrier-casts: `value as T` keeps the denoted value, so a
    // change between fixed-width integer and IEEE carriers is one exact
    // conversion whose representability the checker proves. It is neither a
    // bit reinterpretation (that is a reference recast) nor a rounding
    // conversion (those are named destination requirements).
    let convert = !literal && source.carrier() != target.carrier();
    let numeric = |ty: &ValueType| matches!(ty, ValueType::Integer { width: Some(_), .. } | ValueType::Float32 | ValueType::Float64);
    if convert && !(before.is_empty() && numeric(source) && numeric(target.carrier())) {
        return Err(error("unsupported scalar carrier conversion"));
    }
    let mut pending = added;
    let mut needs_conversion = convert;
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
        convert,
        change,
    })
}
