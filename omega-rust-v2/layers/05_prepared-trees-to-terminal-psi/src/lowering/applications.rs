use super::applied_tree::LoweringTree;
use super::callbacks::Callback;
use crate::LoweringError;
use crate::declarations::regions::{self, RegionData};
use omega_resolved::DeclarationId;
use omega_syntax::NodeKind;
use omega_typed::{CallUse, MachineApplication, TypedCall, TypedSignature, TypedTree, ValueType};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
};

pub(crate) struct MachineInstance<'a> {
    pub application: MachineApplication,
    pub callbacks: Vec<Callback<'a>>,
    pub signature: Cow<'a, TypedSignature>,
    pub tree: LoweringTree<'a>,
}

/// Root declarations are already sorted. Follow their checked executable calls
/// in occurrence order so file order cannot change an instance's machine ID.
pub(crate) fn plan<'a>(
    prepared: &omega_prepared::PreparedTrees<'_>,
    declarations: &[&'a TypedSignature],
    trees: &'a [TypedTree<'a>],
    library_trees: &'a [TypedTree<'a>],
    typed: &'a omega_typed::TypedTrees<'a>,
    region_data: &RegionData<'_>,
    only_root: Option<&DeclarationId>,
) -> Result<Vec<MachineInstance<'a>>, LoweringError> {
    // A library body's calls are rows of the library's forest. Its callers
    // are another package's declarations, so one map serves both forests.
    let library_calls = typed
        .library
        .into_iter()
        .flat_map(|library| library.calls.iter());
    let mut by_caller: HashMap<&DeclarationId, Vec<&TypedCall>> = HashMap::new();
    for call in typed
        .calls
        .iter()
        .chain(library_calls)
        .filter(|call| call.use_site == CallUse::Execution)
    {
        if let Some(caller) = &call.caller {
            by_caller.entry(caller).or_default().push(call);
        }
    }
    for calls in by_caller.values_mut() {
        calls.sort_by_key(|call| call.node);
    }
    let templates: HashMap<_, _> = declarations.iter().map(|&sig| (&sig.machine, sig)).collect();
    // A machine's body is in a tree of the package that declares it. For an
    // imported machine that is the library body this typing round selected
    // (`imported_machine_definition`), never a same-spelled declaration of
    // another library. A machine the compiler supplies has no body.
    let definition = |machine: &DeclarationId| -> Option<(&'a TypedSignature, &'a TypedTree<'a>)> {
        if let Some(&sig) = templates.get(machine) {
            return Some((sig, &trees[sig.node.file]));
        }
        let (_, sig) = typed.imported_machine_definition(machine)?;
        Some((sig, library_trees.get(sig.node.file)?))
    };
    let mut queue = Vec::new();
    let mut seen = HashSet::new();
    let mut region_bodies = HashSet::new();
    // Each judged machine with whether it is a root of this package and the
    // callees its body applies its own regions to. A local declaration is a
    // root unless it captures an enclosing application.
    let mut roots = Vec::with_capacity(declarations.len());

    for &sig in declarations {
        let mut region_callees = Vec::new();
        // A helper with no *own* parameters can still require its lexical application.
        let captures = std::iter::successors(sig.machine.enclosing_machine.as_ref(), |owner| owner.enclosing_machine.as_ref())
            .any(|owner| typed.generic_parameters.get(owner).is_some_and(|p| !p.parameters.is_empty()));
        let calls = by_caller.get(&sig.machine).map_or(&[][..], Vec::as_slice);
        let region_only = one_region_body(sig, &trees[sig.node.file], calls, region_data, &mut region_callees);
        if !captures && region_only {
            region_bodies.insert(&sig.machine);
        }
        roots.push((sig, !captures, region_callees));
    }
    // An imported body is judged the same way, from its library's own facts.
    // Only bodies this package can reach are judged: the targets of its own
    // calls, then the targets of those bodies' calls.
    let mut reached: Vec<&DeclarationId> = typed
        .calls
        .iter()
        .filter(|call| call.use_site == CallUse::Execution)
        .map(|call| &call.machine)
        .collect();
    let mut judged = HashSet::new();
    while let Some(machine) = reached.pop() {
        if templates.contains_key(machine) || !judged.insert(machine) {
            continue;
        }
        let Some((sig, tree)) = definition(machine) else { continue };
        let calls = by_caller.get(&sig.machine).map_or(&[][..], Vec::as_slice);
        reached.extend(calls.iter().map(|call| &call.machine));
        let mut region_callees = Vec::new();
        if one_region_body(sig, tree, calls, region_data, &mut region_callees) {
            region_bodies.insert(&sig.machine);
        }
        // It is no root: only a call that reaches it plans it below.
        roots.push((sig, false, region_callees));
    }
    // A body that applies its regions at a call keeps its one body only while
    // each such callee keeps one too: the call names a region of the callee,
    // and only a one-body callee has region binders in Psi. Dropping one
    // machine can drop its callers, so repeat until nothing changes.
    loop {
        let open: Vec<_> = roots
            .iter()
            .filter(|(sig, _, callees)| region_bodies.contains(&sig.machine) && callees.iter().any(|callee| !region_bodies.contains(callee)))
            .map(|(sig, _, _)| &sig.machine)
            .collect();
        if open.is_empty() {
            break;
        }
        for machine in open {
            region_bodies.remove(machine);
        }
    }
    // A whole package roots every local body. A unit rooted at one machine
    // plans the others only when a planned call demands them.
    for (sig, root, _) in roots {
        if !root || only_root.is_some_and(|only| only != &sig.machine) {
            continue;
        }
        let file = trees[sig.node.file].resolved().file;
        let region_only = region_bodies.contains(&sig.machine);
        if region_only || file.generic_parameters(sig.node.node).next().is_none() {
            let app = MachineApplication {
                machine: Box::new(sig.machine.clone()),
                arguments: vec![],
                enclosing_arguments: vec![],
            };
            seen.insert(app.clone());
            queue.push((app, None));
        }
    }
    let mut instances = Vec::new();
    while let Some((application, _)) = queue.get(instances.len()) {
        let application = application.clone();
        let (template, template_tree) = definition(application.machine.as_ref()).expect("a queued application names a planned definition");
        let mut signature = Cow::Borrowed(template);
        if !application.arguments.is_empty() {
            let sig = signature.to_mut();
            for (_, carrier) in &mut sig.parameters {
                *carrier = application.instantiate(carrier);
            }
            sig.result = application.instantiate(&sig.result);
        }
        let mut callbacks = Vec::new();
        for (slot, arg) in application.arguments.iter().enumerate() {
            if let ValueType::MachineSelection(selected) = arg {
                let bound = typed
                    .callable_bounds
                    .iter()
                    .find(|(owner, i, _)| *owner == template.machine && *i == slot)
                    .ok_or_else(|| LoweringError::new(&template.site, "static callback lacks its typed binder contract"))?;
                callbacks.push(Callback::bind(prepared, slot, selected, &bound.2, &templates)?);
            }
        }
        // Every fact about this body comes from the forest that declares it.
        let forest = template_tree.forest();
        let tree = LoweringTree::for_application(forest, template_tree, &signature, &application, &callbacks)?;
        // A call applies regions to a one-body callee in the row beside it
        // (`CallRegions`). An argument is one of two things. It can be the
        // caller's own region binder, which has an ordinal only while the
        // caller keeps its one body. Or typing inferred it inside the caller's
        // body, as in `size(held)` after `let held = Schema { storage: bytes };`.
        // No binder names that region, so the row says the operands carry it
        // and `calls::region_arguments` refuses one that no operand carries.
        // Any other argument has no Psi form.
        let applies_regions = |call: &TypedCall| {
            let own_binders = application.arguments.is_empty() && region_bodies.contains(&template.machine);
            call.type_arguments.iter().all(|argument| match argument {
                ValueType::Parameter { owner, .. } => own_binders && owner.as_ref() == &template.machine,
                ValueType::InferredLifetime { .. } => true,
                _ => false,
            })
        };
        for call in by_caller.get(&template.machine).into_iter().flatten() {
            let next = if let Some(selected) = selected_implementation(forest, &templates, call, &application)? {
                selected
            } else if !templates.contains_key(&call.machine) {
                // An imported machine with a body is planned because this call
                // demands it, from its own library tree and under its own
                // package. Only a body without static arguments is planned
                // here: one with no binders, or the one body of a region-only
                // machine. Any other call keeps its refusal at the call.
                let Some((target, target_tree)) = definition(&call.machine) else { continue };
                let unbound = target_tree
                    .resolved()
                    .file
                    .generic_parameters(target.node.node)
                    .next()
                    .is_none();
                if !(unbound && call.type_arguments.is_empty() && call.enclosing_arguments.is_empty()) {
                    if !region_bodies.contains(&target.machine) {
                        continue;
                    }
                    if !applies_regions(call) {
                        return Err(LoweringError::new(&call.site, "call of a lifetime-generic machine requires its region arguments in Psi"));
                    }
                }
                MachineApplication {
                    machine: Box::new(target.machine.clone()),
                    arguments: vec![],
                    enclosing_arguments: vec![],
                }
            } else {
                let Some(&target) = templates.get(&call.machine) else { continue };
                // In a whole package this body is already a root and `seen`
                // holds it. A unit rooted at one machine plans it here.
                let whole_body = MachineApplication {
                    machine: Box::new(target.machine.clone()),
                    arguments: vec![],
                    enclosing_arguments: vec![],
                };
                let target_file = trees[target.node.file].resolved().file;
                let count = target_file.generic_parameters(target.node.node).count();
                if count == 0 && call.type_arguments.is_empty() && call.enclosing_arguments.is_empty() {
                    if seen.insert(whole_body.clone()) {
                        queue.push((whole_body, Some(instances.len())));
                    }
                    continue;
                }
                // The callee has one body for every region and is already a
                // root. The call selects that body, and its lowering names the
                // regions it applies beside the call, including one that
                // reaches only the result.
                if region_bodies.contains(&target.machine) {
                    if !applies_regions(call) {
                        return Err(LoweringError::new(&call.site, "call of a lifetime-generic machine requires its region arguments in Psi"));
                    }
                    if seen.insert(whole_body.clone()) {
                        queue.push((whole_body, Some(instances.len())));
                    }
                    continue;
                }
                let arguments: Vec<_> = call.type_arguments.iter().map(|v| instantiate(&application, v)).collect();
                let binders = forest.generic_parameters.get(&target.machine);
                if !call.enclosing_arguments.is_empty()
                    || count != arguments.len()
                    || !target_file
                        .generic_parameters(target.node.node)
                        .zip(&arguments)
                        .enumerate()
                        .all(|(slot, ((_, _, kind), arg))| match (kind, arg) {
                            (NodeKind::ConstParam, ValueType::StaticInteger { carrier, value }) => {
                                // Admit closed integer values and direct forwarding, not computed families.
                                matches!(call.type_arguments[slot], ValueType::StaticInteger { .. } | ValueType::Parameter { .. })
                                    && binders.filter(|p| p.equations.is_empty() && p.witness_count == 0)
                                        .and_then(|p| p.parameters.get(slot)).and_then(|p| p.carrier.as_deref()) == Some(carrier.as_ref())
                                    && carrier.contains_integer(value)
                            }
                            (NodeKind::ConstParam, ValueType::MachineSelection(_) | ValueType::Implementation(_)) => true,
                            (NodeKind::TypeParam, _) => !matches!(arg, ValueType::MachineSelection(_) | ValueType::StaticInteger { .. }),
                            _ => false,
                        })
                {
                    return Err(LoweringError::new(
                        &target.site,
                        "open generic machine requires a closed application before lowering",
                    ));
                }
                MachineApplication {
                    machine: Box::new(target.machine.clone()),
                    arguments,
                    enclosing_arguments: vec![],
                }
            };
            if seen.insert(next.clone()) {
                // Ordinary recursion reuses an existing instance. Growing the argument
                // tuple along a recursive chain requires a proven finite family.
                let mut ancestor = Some(instances.len());
                while let Some(index) = ancestor {
                    if queue[index].0.machine == next.machine
                        && call
                            .type_arguments
                            .iter()
                            .flat_map(ValueType::components)
                            .any(|ty| matches!(ty, ValueType::Parameter { .. }))
                    {
                        return Err(LoweringError::new(
                            &call.site,
                            "recursive specialization changes its application without a finite family",
                        ));
                    }
                    ancestor = queue[index].1;
                }
                queue.push((next, Some(instances.len())));
            }
        }
        instances.push(MachineInstance {
            application,
            signature,
            tree,
            callbacks,
        });
    }
    Ok(instances)
}

/// A type of the caller's body at `application`. A machine with one body for
/// every region has no arguments, and its own region binders stay as written:
/// there is nothing to substitute for them.
fn instantiate(
    application: &MachineApplication,
    value: &ValueType,
) -> ValueType {
    if application.arguments.is_empty() && application.enclosing_arguments.is_empty() {
        value.clone()
    } else {
        application.instantiate(value)
    }
}

/// Whether `sig`, declared in `tree`, has one executable body for every
/// region. A lifetime with no Psi slot still needs its open template. A
/// direct reference can instead name the region on one executable body. So
/// can a record with a type row, provided the body keeps the region there:
/// until records had slots such a machine was an open template, and it stays
/// one when its body cannot close. The telescope and the body's rows are the
/// declaring forest's, which for an imported machine is its library's.
fn one_region_body<'a>(
    sig: &TypedSignature,
    tree: &TypedTree<'_>,
    calls: &[&'a TypedCall],
    region_data: &RegionData<'_>,
    region_callees: &mut Vec<&'a DeclarationId>,
) -> bool {
    let file = tree.resolved().file;
    let telescope = tree.forest().generic_parameters.get(&sig.machine);
    telescope.is_some_and(|params| {
        file.generic_parameters(sig.node.node)
            .map(|(_, _, kind)| kind)
            .eq(params.parameters.iter().map(|p| p.kind))
    }) && regions::single_body(sig, telescope, region_data).is_some_and(|slots| {
        slots
            .iter()
            .all(|region| region.aggregate_parameters.is_empty() && region.aggregate_result.is_empty())
            || closes_over_regions(sig, tree, calls, region_data, region_callees)
    })
}

/// Whether the body of the lifetime-only machine `sig` can be the one body
/// for every region. Its signature already has a slot for each region.
///
/// The body must keep each region where Psi can name it. Every value whose
/// type names a binder of `sig` is a direct reference at that binder, a
/// record `region_data` lowers, or a view read out of such a record
/// (`reads_stored_view`). A call may apply a region only as a plain
/// lifetime argument, as in `size::<'u>(schema)`: Psi names such an argument
/// beside the call when the callee has one body too. `region_callees`
/// receives each callee so applied, and the caller decides whether they all
/// keep one body. A region inside any other static argument, or one applied
/// through evidence or an enclosing application, has no Psi form. A body that
/// needs one stays an open template with its binders intact.
fn closes_over_regions<'a>(
    sig: &TypedSignature,
    tree: &TypedTree<'_>,
    calls: &[&'a TypedCall],
    region_data: &RegionData<'_>,
    region_callees: &mut Vec<&'a DeclarationId>,
) -> bool {
    let machine = &sig.machine;
    let is_region =
        |part: &ValueType| matches!(part, ValueType::InferredLifetime { .. }) || matches!(part, ValueType::Parameter { owner, .. } if owner.as_ref() == machine);
    let applies_region = |argument: &ValueType| argument.components().any(is_region);
    for call in calls {
        if call.enclosing_arguments.iter().any(|(_, arguments)| arguments.iter().any(applies_region)) {
            return false;
        }
        if call.type_arguments.iter().any(applies_region) {
            if !call.type_arguments.iter().all(is_region) {
                return false;
            }
            if let Some(key) = crate::declarations::members::Key::from_call(call, tree.forest()) {
                // A requirement has no local body. Its declaration owns the
                // binders and member planning checks the realized interface.
                if key.region_count != call.type_arguments.len() { return false; }
            } else if call.evidence.is_none() {
                region_callees.push(&call.machine);
            } else {
                return false;
            }
        }
    }
    let file = tree.resolved().file;
    let declaration = sig.node.node;
    let mut node = declaration + 1;
    while node < file.nodes[declaration].end {
        // A nested machine is its own declaration with its own signature.
        if file.nodes[node].kind == NodeKind::Machine {
            node = file.nodes[node].end;
            continue;
        }
        let kept = tree.semantics_at(node).all(|record| match record {
            omega_typed::NodeSemantics::Expression(expression) => {
                regions::keeps_binders(&expression.ty, machine, region_data) || reads_stored_view(tree, node, &expression.ty, machine)
            }
            omega_typed::NodeSemantics::Landing(landing) => regions::keeps_binders(&landing.ty, machine, region_data),
            _ => true,
        });
        if !kept {
            return false;
        }
        node += 1;
    }
    true
}

/// Whether `node` reads a view that a record holds by value at a region of
/// `machine`, as `schema.storage` does in
/// `forward<'t>(schema: Schema<'t>) -> Schema<'t> { Schema::<'t> { storage: schema.storage } }`.
///
/// The value is at `'t` and has no region slot. It needs none: the body
/// copies the stored descriptor out of the field, and that copy names the
/// field of `schema`, whose record slot says the field is at `'t`. Only this
/// form qualifies, a chain of fields from one binding with no reference on
/// the way. A read through a reference, a view some other expression
/// computes, and a view rebound to a local keep the machine an open template.
fn reads_stored_view(
    tree: &TypedTree<'_>,
    node: usize,
    ty: &ValueType,
    machine: &DeclarationId,
) -> bool {
    let file = tree.resolved().file;
    if file.nodes[node].kind != NodeKind::FieldAccess || regions::stored_view(ty, machine).is_none() {
        return false;
    }
    let (root, steps) = tree.forest().place_path(file.node_id(node));
    root.file == file.node_id(node).file
        && file
            .nodes
            .get(root.node)
            .is_some_and(|root| root.kind == NodeKind::PathExpression)
        && !steps.is_empty()
        && steps
            .iter()
            .all(|step| step.through.is_none() && matches!(step.projection, Some(omega_typed::Projection::Field(_))))
}

/// A member call whose closed evidence selects a conformance runs that row's
/// implementation. The requirement's own static arguments specialize the
/// implementation by binder position. Without them the implementation is a
/// rooted declaration and a boundary call has no body to schedule.
fn selected_implementation(
    typed: &omega_typed::TypedTrees<'_>,
    templates: &HashMap<&DeclarationId, &TypedSignature>,
    call: &TypedCall,
    application: &MachineApplication,
) -> Result<Option<MachineApplication>, LoweringError> {
    if call.type_arguments.is_empty() || call.evidence.is_none() {
        return Ok(None);
    }
    let closed = call.with_substitutions(|v| instantiate(application, v));
    let Some(key) = crate::declarations::members::Key::from_call(&closed, typed) else {
        return Ok(None);
    };
    if key.region_count != 0 { return Ok(None); }
    let Some(implementation) = key.implementation(typed, &call.site)? else {
        return Ok(None);
    };
    let target = templates
        .get(implementation)
        .ok_or_else(|| LoweringError::new(&call.site, "selected conformance implementation needs its packaged generic body"))?;
    let instance = MachineApplication {
        machine: Box::new(target.machine.clone()),
        arguments: key.static_arguments().to_vec(),
        enclosing_arguments: vec![],
    };
    let supported = typed.generic_parameters.get(implementation).is_some_and(|params| {
        params.parameters.len() == instance.arguments.len()
            && params.parameters.iter().zip(&instance.arguments).all(|(param, arg)| match arg {
                ValueType::StaticInteger { carrier, .. } => {
                    param.kind == NodeKind::ConstParam
                        && matches!(carrier.as_ref(), ValueType::Integer { width: Some(1..), .. })
                        && param.carrier.as_deref().is_some_and(|ty| instance.instantiate(ty) == **carrier)
                }
                ValueType::MachineSelection(_) | ValueType::Implementation(_) => param.kind == NodeKind::ConstParam,
                _ => param.kind == NodeKind::TypeParam,
            })
    });
    if !supported {
        return Err(LoweringError::new(
            &target.site,
            "selected implementation binders require a supported closed application",
        ));
    }
    Ok(Some(instance))
}
