//! A scoped judgment over scalar replacement edges. Machine requirements are
//! entry assumptions. A destination invariant is an obligation on every incoming
//! edge; it becomes available only after its argument has discharged it.
use omega_psi::*;
use std::collections::{BTreeMap, VecDeque};

mod guards;
mod predicates;
use predicates::{Facts, fits, instantiate};

#[derive(Debug)]
pub struct ScalarReplacementError {
    pub site: Site,
    pub message: String,
}

type Reason<T> = Result<T, &'static str>;
type Environment = BTreeMap<ValueId, Facts>;

fn edges(block: &BranchBlock) -> impl Iterator<Item = &SuccessorEdge> {
    match &block.terminator {
        BlockTerminator::Jump(edge) => [Some(edge), None],
        BlockTerminator::Conditional { when_true, when_false, .. } => [Some(when_true), Some(when_false)],
        _ => [None, None],
    }
    .into_iter()
    .flatten()
}

fn scalar(
    unit: &LoweredPackage,
    ty: TypeId,
) -> Reason<(ScalarType, Vec<DomainAtom>)> {
    match unit.types.get(ty) {
        Some(TypeShape::PrimitiveScalar(carrier)) => Ok((*carrier, Vec::new())),
        Some(TypeShape::QualifiedScalar { carrier, domains }) => {
            let mut atoms = Vec::new();
            for (declaration, arguments) in unit
                .types
                .domain_atoms(*domains)
                .ok_or("unknown qualification set")?
            {
                let atom = DomainAtom {
                    declaration: declaration.clone(),
                    arguments: arguments.to_vec(),
                };
                if atoms.contains(&atom) {
                    return Err("duplicate domain atom");
                }
                atoms.push(atom);
            }
            Ok((*carrier, atoms))
        }
        _ => Err("expected a primitive scalar carrier"),
    }
}

fn domain<'a>(
    unit: &'a LoweredPackage,
    atom: &DomainAtom,
) -> Reason<&'a LoweredDomain> {
    let mut matches = unit
        .domains
        .iter()
        .filter(|row| row.declaration == atom.declaration);
    let row = matches.next().ok_or("missing domain declaration")?;
    if matches.next().is_some() {
        return Err("ambiguous domain declaration");
    }
    if row.index_binders.len() != atom.arguments.len()
        || row.parameters.len() != atom.arguments.len()
        || row
            .index_binders
            .iter()
            .enumerate()
            .any(|(i, slot)| row.index_binders[..i].contains(slot))
    {
        return Err("domain argument count differs from its declaration");
    }
    for (&index, arg) in row.index_binders.iter().zip(&atom.arguments) {
        let Some(TemplateBinder::Const(ty) | TemplateBinder::Kinded(ty)) = row.parameters.get(index) else {
            return Err("unsupported domain index binder");
        };
        let [TypeExpressionStep::Scalar(carrier)] = ty.0.as_slice() else {
            return Err("unsupported domain index carrier");
        };
        let valid = match arg {
            DeclarationTerm::StaticInteger { carrier: actual, value } => actual == carrier && fits(&ScalarLiteral::Integer(value.clone()), *carrier),
            DeclarationTerm::StaticBoolean(_) => *carrier == ScalarType::Boolean,
            _ => false,
        };
        if !valid {
            return Err("domain argument differs from its index carrier");
        }
    }
    Ok(row)
}

fn value(
    body: &MachineBody,
    id: ValueId,
) -> Reason<&Value> {
    body.values.get(id.index()).ok_or("unknown scalar value")
}

fn target<'a>(
    body: &'a MachineBody,
    edge: &SuccessorEdge,
) -> Reason<&'a BranchBlock> {
    body.blocks
        .get(edge.target.get() - 1)
        .filter(|block| block.id == edge.target)
        .ok_or("unknown successor block")
}

fn has_replacements(
    unit: &LoweredPackage,
    body: &MachineBody,
) -> Reason<bool> {
    for block in &body.blocks {
        for edge in edges(block) {
            if edge
                .scalar_qualification_changes
                .iter()
                .any(|change| change.direction == QualificationDirection::Replace)
            {
                return Ok(true);
            }
            let dest = target(body, edge)?;
            if edge.arguments.len() != dest.scalar_parameters.len() {
                return Err("successor scalar arity differs");
            }
            for (&source, &param) in edge.arguments.iter().zip(&dest.scalar_parameters) {
                let from = value(body, source)?.ty;
                let to = value(body, param)?.ty;
                if from == to {
                    continue;
                }
                let left = unit.types.get(from).ok_or("unknown scalar source type")?;
                let right = unit.types.get(to).ok_or("unknown scalar destination type")?;
                if matches!(left, TypeShape::QualifiedScalar { .. }) || matches!(right, TypeShape::QualifiedScalar { .. }) {
                    let (_, before) = scalar(unit, from)?;
                    let (_, after) = scalar(unit, to)?;
                    if before.iter().any(|a| !after.contains(a)) && after.iter().any(|a| !before.contains(a)) {
                        return Ok(true);
                    }
                }
            }
        }
    }
    Ok(false)
}

/// Check each closed body's mixed scalar qualification transfers under its
/// entry contract. The judgment covers the edges and the facts used there.
pub fn check_scalar_replacements(unit: &LoweredPackage) -> Result<(), ScalarReplacementError> {
    for machine in &unit.machines {
        let body = &machine.body;
        let error = |message: &'static str| ScalarReplacementError {
            site: body.diagnostics.body.clone(),
            message: message.into(),
        };
        if has_replacements(unit, body).map_err(error)? {
            check_body(unit, machine)?;
        }
    }
    Ok(())
}

fn entry_facts(
    unit: &LoweredPackage,
    machine: &LoweredMachine,
) -> Reason<Environment> {
    let mut env = Environment::new();
    if machine.parameter_types.len() != machine.body.parameters.len() {
        return Err("formal parameter count differs");
    }
    for (index, (&param, &ty)) in machine
        .body
        .parameters
        .iter()
        .zip(&machine.parameter_types)
        .enumerate()
    {
        let Parameter::Scalar(id) = param else { continue };
        let row = value(&machine.body, id)?;
        if row.ty != ty || row.origin != ValueOrigin::Parameter || env.contains_key(&id) {
            return Err("invalid scalar formal definition");
        }
        let mut facts = Facts::at(id);
        if let Ok((carrier, atoms)) = scalar(unit, ty) {
            for clause in &machine.contract.requires {
                if let Some(bound) = predicates::formal_fact(&clause.predicate, index)
                    && predicates::well_typed(&bound, carrier)
                {
                    facts.assume(bound);
                }
            }
            for atom in &atoms {
                let decl = domain(unit, atom)?;
                let (base, parents) = scalar(unit, decl.carrier)?;
                if base != carrier || !parents.iter().all(|p| atoms.contains(p)) {
                    return Err("formal domain lacks its carrier memberships");
                }
                for clause in &decl.requires {
                    let p = instantiate(&clause.predicate, decl, atom)?;
                    if !predicates::well_typed(&p, carrier) || !facts.proves(&p) {
                        return Err("formal domain predicate is missing from the entry contract");
                    }
                }
            }
        }
        env.insert(id, facts);
    }
    Ok(env)
}

fn check_body(
    unit: &LoweredPackage,
    machine: &LoweredMachine,
) -> Result<(), ScalarReplacementError> {
    let body = &machine.body;
    let error = |site: &Site, message: &str| ScalarReplacementError {
        site: site.clone(),
        message: message.into(),
    };
    let fail = |s: &str| error(&body.diagnostics.body, s);
    let mut arrivals = vec![None::<Environment>; body.blocks.len()];
    let mut remaining = vec![0usize; body.blocks.len()];
    let mut assigned = vec![false; body.operations.len()];
    let mut edge_sites = vec![false; body.diagnostics.edges.len()];
    for (index, block) in body.blocks.iter().enumerate() {
        if block.id.get() != index + 1 {
            return Err(fail("block ID differs from its position"));
        }
        let (first, second) = match &block.terminator {
            BlockTerminator::Jump(edge) => (edge.id, None),
            BlockTerminator::Conditional { when_true, when_false, .. } => (when_true.id, Some(when_false.id)),
            BlockTerminator::Return { id, .. } | BlockTerminator::TerminalCall { id, .. } => (*id, None),
            BlockTerminator::ExternalExit(exit) => (exit.id, None),
        };
        for edge in [Some(first), second].into_iter().flatten() {
            let site = edge_sites
                .get_mut(edge.get() - 1)
                .ok_or_else(|| fail("missing edge diagnostic site"))?;
            if *site {
                return Err(fail("two control edges have the same identity"));
            }
            *site = true;
        }
        let range = assigned
            .get_mut(block.operations.clone())
            .ok_or_else(|| fail("invalid block operation range"))?;
        for present in range {
            if *present {
                return Err(fail("overlapping block operations"));
            }
            *present = true;
        }
        for edge in edges(block) {
            let dest = target(body, edge).map_err(fail)?;
            remaining[dest.id.get() - 1] += 1;
        }
    }
    if remaining.first() != Some(&0) || !body.blocks[0].scalar_parameters.is_empty() {
        return Err(fail("replacement proof requires a unique entry with no incoming block parameters"));
    }
    arrivals[0] = Some(entry_facts(unit, machine).map_err(fail)?);
    let mut ready = VecDeque::from([0]);
    let mut visited = 0;
    while let Some(index) = ready.pop_front() {
        visited += 1;
        let block = &body.blocks[index];
        let mut env = arrivals[index]
            .take()
            .ok_or_else(|| fail("missing incoming facts"))?;
        for pos in block.operations.clone() {
            let operation = &body.operations[pos];
            if operation.id.index() != pos {
                return Err(fail("operation ID differs from its position"));
            }
            if let OperationResult::Scalar(id) = operation.result {
                let row = value(body, id).map_err(fail)?;
                if row.origin != ValueOrigin::Operation(operation.id) || env.contains_key(&id) {
                    return Err(fail("invalid scalar operation definition"));
                }
                let mut facts = Facts::at(id);
                if let Ok((carrier, domains)) = scalar(unit, row.ty) {
                    // An arbitrary operation producing a qualified result needs its own proof.
                    if !domains.is_empty() {
                        continue;
                    }
                    match &operation.kind {
                        OperationKind::Constant(literal) if fits(literal, carrier) => facts.literal = Some(literal.clone()),
                        OperationKind::Constant(_) => return Err(fail("literal does not fit its carrier")),
                        OperationKind::Integer { operation, operands } if operation.primitive == IntegerPrimitive::Negate => {
                            let [operand] = operands.as_slice() else { continue };
                            let expected: &[IntegerObligation] = if operation.policy == ArithmeticPolicy::Exact {
                                &[IntegerObligation::ResultFitsCarrier]
                            } else {
                                &[]
                            };
                            if operation.obligations != expected || value(body, *operand).map_err(fail)?.ty != row.ty {
                                continue;
                            }
                            let Some(ScalarLiteral::Integer(n)) = env.get(operand).and_then(|f| f.literal.as_ref()) else {
                                continue;
                            };
                            let result = ScalarLiteral::Integer(-n);
                            if !fits(&result, carrier) {
                                continue;
                            }
                            facts.literal = Some(result);
                        }
                        OperationKind::Integer { operation, operands } if carrier == ScalarType::Boolean => {
                            facts.guard = guards::Guard::read(unit, body, operation, operands, &env).map_err(fail)?;
                        }
                        _ => continue,
                    }
                } else {
                    continue;
                }
                env.insert(id, facts);
            }
        }
        for edge in edges(block) {
            let dest = target(body, edge).map_err(fail)?;
            let site = body
                .diagnostics
                .edges
                .get(edge.id.get() - 1)
                .unwrap_or(&body.diagnostics.body);
            let next = transfer(unit, body, block, edge, dest, &env).map_err(|reason| error(site, &reason))?;
            let i = dest.id.get() - 1;
            if let Some(prior) = &mut arrivals[i] {
                prior.retain(|id, facts| {
                    next.get(id).is_some_and(|other| {
                        facts.intersect(other);
                        facts.origin.get_or_insert(*id);
                        true
                    })
                });
            } else {
                arrivals[i] = Some(next);
            }
            remaining[i] -= 1;
            if remaining[i] == 0 {
                ready.push_back(i);
            }
        }
    }
    if visited != body.blocks.len() {
        return Err(fail("cyclic or unreachable replacement facts require a control-flow proof"));
    }
    Ok(())
}

fn transfer(
    unit: &LoweredPackage,
    body: &MachineBody,
    block: &BranchBlock,
    edge: &SuccessorEdge,
    dest: &BranchBlock,
    env: &Environment,
) -> Result<Environment, String> {
    let arity = dest.scalar_parameters.len();
    if edge.arguments.len() != arity
        || edge
            .scalar_qualification_changes
            .iter()
            .any(|r| r.argument_index >= arity)
        || edge
            .domain_completions
            .iter()
            .any(|r| r.argument_index >= arity)
    {
        return Err("qualification transfer has an invalid argument position".into());
    }
    if dest
        .scalar_domain_invariants
        .iter()
        .any(|r| !dest.scalar_parameters.contains(&r.subject))
    {
        return Err("arrival invariant names a foreign subject".into());
    }
    let mut next = env.clone();
    if let BlockTerminator::Conditional { condition, when_true, .. } = &block.terminator
        && let Some(guard) = env.get(condition).and_then(|f| f.guard.as_ref())
    {
        guard.apply(when_true.id == edge.id, &mut next);
    }
    for (position, (&source, &destination)) in edge.arguments.iter().zip(&dest.scalar_parameters).enumerate() {
        let to = value(body, destination)?;
        if to.origin != ValueOrigin::BlockParameter(dest.id) || next.contains_key(&destination) {
            return Err("destination is not a fresh block parameter".into());
        }
        let from = value(body, source)?;
        if !env.contains_key(&source) {
            return Err("scalar argument lacks a dominating established definition".into());
        }
        let mut facts = next
            .get(&source)
            .ok_or("scalar argument lacks a dominating established definition")?
            .clone();
        let mut markers = edge
            .scalar_qualification_changes
            .iter()
            .filter(|r| r.argument_index == position);
        let marker = markers.next().map(|r| r.direction);
        if markers.next().is_some() {
            return Err("more than one qualification marker on the argument".into());
        }
        let rows: Vec<_> = edge
            .domain_completions
            .iter()
            .filter(|r| r.argument_index == position)
            .collect();
        let invariants: Vec<_> = dest
            .scalar_domain_invariants
            .iter()
            .filter(|r| r.subject == destination)
            .map(|r| &r.domain)
            .collect();
        if from.ty == to.ty && marker.is_none() && rows.is_empty() && invariants.is_empty() {
            next.insert(destination, facts);
            continue;
        }
        let (carrier, before) = scalar(unit, from.ty)?;
        let (after_carrier, after) = scalar(unit, to.ty)?;
        if carrier != after_carrier {
            return Err("qualification transfer changes the payload carrier".into());
        }
        let added: Vec<_> = after.iter().filter(|atom| !before.contains(atom)).collect();
        let removed: Vec<_> = before.iter().filter(|atom| !after.contains(atom)).collect();
        if !matches!(block.terminator, BlockTerminator::Jump(_)) && (!added.is_empty() || !removed.is_empty() || marker.is_some() || !rows.is_empty()) {
            return Err("qualification transfer must be an unconditional jump".into());
        }
        let expected_marker = match (removed.is_empty(), added.is_empty(), rows.is_empty()) {
            (false, false, _) => Some(QualificationDirection::Replace),
            (false, true, _) => Some(QualificationDirection::Erase),
            (true, false, true) => Some(QualificationDirection::Add),
            _ => None,
        };
        if marker != expected_marker {
            return Err("qualification marker disagrees with the added and removed memberships".into());
        }
        for atom in &removed {
            if domain(unit, atom)?.cleanup_target.is_some() {
                return Err("removed membership has a cleanup obligation".into());
            }
        }
        let needs_rows = marker == Some(QualificationDirection::Replace) || !rows.is_empty();
        if needs_rows && (rows.len() != added.len() || rows.iter().zip(&added).any(|(r, a)| &r.domain != *a)) {
            return Err("completion rows must name exactly the added memberships in destination order".into());
        }
        let mut earned = Vec::new();
        let mut proved = Vec::new();
        let mut grounded: Vec<_> = before.iter().filter(|a| after.contains(a)).cloned().collect();
        let mut pending = Vec::new();
        for (index, atom) in added.iter().enumerate() {
            let decl = domain(unit, atom)?;
            if decl.issuers != DomainIssuers::Unrestricted {
                return Err(format!("added domain {} lacks an authorized issuer", atom.declaration.path));
            }
            let (base, parents) = scalar(unit, decl.carrier)?;
            if base != carrier {
                return Err("domain is declared for another carrier".into());
            }
            pending.push((*atom, parents));
            let origin = if decl.requires.is_empty() {
                CompletionOrigin::Vacuous
            } else {
                CompletionOrigin::Proved
            };
            if (needs_rows && rows[index].origin != origin) || (!needs_rows && origin != CompletionOrigin::Vacuous) {
                return Err("completion origin disagrees with the declaration's obligations".into());
            }
            if origin == CompletionOrigin::Proved {
                proved.push(*atom);
            }
            for clause in &decl.requires {
                let predicate = instantiate(&clause.predicate, decl, atom)?;
                if !predicates::well_typed(&predicate, carrier) || !facts.proves(&predicate) {
                    return Err(format!(
                        "domain {} predicate is not established for this argument at the cast",
                        atom.declaration.path
                    ));
                }
                earned.push(predicate);
            }
        }
        while !pending.is_empty() {
            let prior = pending.len();
            pending.retain(|(atom, parents)| {
                if parents.iter().all(|p| grounded.contains(p)) {
                    grounded.push((*atom).clone());
                    false
                } else {
                    true
                }
            });
            if prior == pending.len() {
                return Err("added domain lacks established parent memberships".into());
            }
        }
        for atom in &after {
            let (base, parents) = scalar(unit, domain(unit, atom)?.carrier)?;
            if base != carrier || !parents.iter().all(|p| after.contains(p)) {
                return Err("destination lacks a retained domain's carrier memberships".into());
            }
        }
        if proved != invariants {
            return Err("destination invariants differ from the added domains' proof obligations".into());
        }
        for p in earned {
            facts.assume(p);
        }
        next.insert(destination, facts);
    }
    Ok(next)
}
