//! Read the domains a jump establishes from published Psi alone, applying the
//! completion rules of `wiki/spec/terminal-psi/encoding.md` (scalar return origins).
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
    let members = |value| -> Result<_, &'static str> {
        match shape(value) {
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
    let (from, held) = members(source)?;
    let (to, gained) = members(destination)?;
    if from != to {
        return Err("completion changes the payload carrier");
    }
    let rows: Vec<_> = edge
        .domain_completions
        .iter()
        .filter(|row| row.argument_index == argument)
        .collect();
    if edge
        .scalar_qualification_changes
        .iter()
        .any(|change| change.argument_index == argument)
    {
        return Err("an argument with completions also carries a qualification change");
    }
    let added: Vec<_> = gained.iter().filter(|atom| !held.contains(atom)).collect();
    if held.iter().any(|atom| !gained.contains(atom)) {
        return Err("a completion cannot remove a domain");
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
    for row in &rows {
        let domain = package
            .domains
            .iter()
            .find(|domain| domain.declaration == row.domain.declaration)
            .ok_or("completion names an undeclared domain")?;
        if row.domain.arguments.len() != domain.index_binders.len() {
            return Err("a completion atom does not supply one argument per domain index");
        }
        if !matches!(package.types.get(domain.carrier), Some(TypeShape::PrimitiveScalar(_))) {
            return Err("a vacuous or proved origin needs empty required memberships");
        }
        match row.origin {
            CompletionOrigin::Vacuous if domain.requires.is_empty() => {}
            CompletionOrigin::Vacuous => return Err("a vacuous origin names a domain with predicates"),
            CompletionOrigin::Proved if !domain.requires.is_empty() => proved.push(&row.domain),
            CompletionOrigin::Proved => return Err("a proved origin names a predicate-free domain"),
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
