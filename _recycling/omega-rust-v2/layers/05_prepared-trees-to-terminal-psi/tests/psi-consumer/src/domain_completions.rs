//! Read the domains a jump establishes from published Psi alone, applying the
//! completion rules of `source-rust/subsystems/compiler/psi/artifact/wire/encoding.md` (scalar return origins).
//! This reader checks the accounting; it proves no predicate.
use omega_psi::{BlockTerminator, CompletionOrigin, DomainAtom, LoweredPackage, MachineBody, SuccessorEdge, TypeShape, ValueOrigin};

/// The domain atoms the destination at `argument` gains on `edge`, in destination order,
/// each with the origin its completion row names. An atom keeps its closed arguments.
pub fn cast_completions<'a>(
    package: &'a LoweredPackage,
    body: &'a MachineBody,
    edge: &'a SuccessorEdge,
    argument: usize,
) -> Result<Vec<(&'a DomainAtom, CompletionOrigin)>, &'static str> {
    let arrival = body
        .blocks
        .iter()
        .find(|block| block.id == edge.target)
        .ok_or("edge target is not a block")?;
    let (&source, &destination) = edge
        .arguments
        .get(argument)
        .zip(arrival.scalar_parameters.get(argument))
        .ok_or("argument has no destination parameter")?;
    if source == destination || body.values.get(destination.index()).map(|value| &value.origin) != Some(&ValueOrigin::BlockParameter(arrival.id)) {
        return Err("destination is not a fresh parameter of the arrival");
    }
    let shape = |value: omega_psi::ValueId| {
        body.values
            .get(value.index())
            .and_then(|value| package.types.get(value.ty))
    };
    let members = |shape: Option<&TypeShape>| -> Result<_, &'static str> {
        match shape {
            Some(TypeShape::PrimitiveScalar(carrier)) => Ok((*carrier, Vec::new())),
            Some(TypeShape::QualifiedScalar { carrier, domains }) => Ok((
                *carrier,
                package
                    .types
                    .domain_atoms(*domains)
                    .ok_or("unknown domain set")?
                    .collect::<Vec<_>>(),
            )),
            _ => Err("completion argument is not a scalar"),
        }
    };
    let (from, held) = members(shape(source))?;
    let (to, gained) = members(shape(destination))?;
    if from != to {
        return Err("completion changes the payload carrier");
    }
    let rows: Vec<_> = edge
        .domain_completions
        .iter()
        .filter(|row| row.argument_index == argument)
        .collect();
    let mut changes = edge.scalar_qualification_changes.iter().filter(|change| change.argument_index == argument);
    let added: Vec<_> = gained.iter().filter(|atom| !held.contains(atom)).collect();
    let removes = held.iter().any(|atom| !gained.contains(atom));
    match (changes.next().map(|change| change.direction), changes.next()) {
        (None, None) if removes => return Err("a completion cannot remove a domain without a replacement marker"),
        (None, None) => {}
        (Some(omega_psi::QualificationDirection::Replace), None) if removes && !added.is_empty() => {
            for (declaration, _) in held.iter().filter(|atom| !gained.contains(atom)) {
                let domain = package.domains.iter().find(|row| row.declaration == **declaration)
                    .ok_or("a removed domain is absent from the catalog")?;
                if domain.cleanup_target.is_some() {
                    return Err("replacement needs the removed domain's cleanup obligation");
                }
            }
        }
        _ => return Err("qualification marker disagrees with the completion on this argument"),
    }
    if rows.len() != added.len()
        || rows
            .iter()
            .zip(&added)
            .any(|(row, (declaration, arguments))| row.domain.declaration != **declaration || row.domain.arguments != *arguments)
    {
        return Err("completion rows do not name each added domain once in destination order");
    }
    let mut proved = Vec::new();
    let mut pending_parents = Vec::new();
    for row in &rows {
        let domain = package
            .domains
            .iter()
            .find(|domain| domain.declaration == row.domain.declaration)
            .ok_or("completion names an undeclared domain")?;
        if row.domain.arguments.len() != domain.index_binders.len() {
            return Err("a completion atom does not supply one argument per domain index");
        }
        if domain.issuers != omega_psi::DomainIssuers::Unrestricted {
            return Err("a vacuous or proved origin cannot authorize restricted issuance");
        }
        let (payload, parents) = members(package.types.get(domain.carrier))?;
        if payload != from {
            return Err("the domain declaration has a different payload carrier");
        }
        pending_parents.push(((&row.domain.declaration, row.domain.arguments.as_slice()), parents));
        match row.origin {
            CompletionOrigin::Vacuous if domain.requires.is_empty() => {}
            CompletionOrigin::Vacuous => return Err("a vacuous origin names a domain with predicates"),
            CompletionOrigin::Proved if !domain.requires.is_empty() => proved.push(&row.domain),
            CompletionOrigin::Proved => return Err("a proved origin names a predicate-free domain"),
        }
    }
    // Seed with this argument's existing qualifications. A new row contributes its
    // exact atom only after all of its own prerequisites have been accounted for.
    // The rows' canonical order need not be an order of parenthood.
    let mut grounded: Vec<_> = held.into_iter().filter(|atom| gained.contains(atom)).collect();
    while !pending_parents.is_empty() {
        let count = pending_parents.len();
        pending_parents.retain(|(atom, parents)| {
            if parents.iter().all(|parent| grounded.contains(parent)) {
                grounded.push(*atom);
                false
            } else {
                true
            }
        });
        if count == pending_parents.len() {
            return Err("an added domain lacks independently grounded parent membership on this argument");
        }
    }
    // Every arrival owes exactly the proved predicates, bound to this destination.
    let owed: Vec<_> = arrival
        .scalar_domain_invariants
        .iter()
        .filter(|invariant| invariant.subject == destination)
        .map(|invariant| &invariant.domain)
        .collect();
    if owed != proved {
        return Err("the arrival invariants differ from the proved completions");
    }
    if !body
        .blocks
        .iter()
        .any(|block| matches!(&block.terminator, BlockTerminator::Jump(jump) if std::ptr::eq(jump, edge)))
    {
        return Err("edge is not a jump of this body");
    }
    Ok(rows.iter().map(|row| (&row.domain, row.origin)).collect())
}

/// Observe the domain declaration's selected cleanup and the cleanup that a
/// straight-line unit machine actually invokes for its owned formal at return.
pub fn returned_domain_cleanup<'a>(
    package: &'a LoweredPackage,
    owner: &str,
    domain_name: &str,
) -> Option<(Option<&'a str>, Vec<String>, Vec<&'a str>)> {
    use omega_psi::{CleanupAction, Parameter, ReturnEdge};
    let machine = super::machine(package, owner)?;
    let [Parameter::Structural(formal)] = machine.body.parameters[..] else { return None };
    let (record, atoms) = super::record_formal(package, owner, 0)?;
    let domain = package.domains.iter().find(|d| d.declaration.path == domain_name)?;
    if domain.carrier != record.carrier { return None; }
    let selected = |id| package.machines.iter().find(|m| m.id == id && m.parameter_types == [record.carrier] && m.result_type.is_none())
        .map(|m| m.declaration.path.as_str());
    let target = match domain.cleanup_target { Some(id) => Some(selected(id)?), None => None };
    let [block] = &machine.body.blocks[..] else { return None };
    let BlockTerminator::Return { result: ReturnEdge::Unit, cleanup, .. } = &block.terminator else { return None };
    let mut invoked = Vec::new();
    for action in cleanup {
        match action {
            CleanupAction::Invoke { place, ty, target } if place.root == formal && place.path.is_empty() && machine.parameter_types == [*ty] => {
                invoked.push(selected(*target)?);
            }
            CleanupAction::DiscardRoot(p) if *p == formal => {}
            _ => return None,
        }
    }
    Some((target, atoms.into_iter().map(|atom| atom.declaration.path.clone()).collect(), invoked))
}
