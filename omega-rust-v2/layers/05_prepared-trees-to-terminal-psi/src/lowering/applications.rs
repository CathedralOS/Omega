use super::applied_tree::LoweringTree;
use super::callbacks::Callback;
use crate::LoweringError;
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
    declarations: &[&'a TypedSignature],
    trees: &'a [TypedTree<'a>],
    typed: &'a omega_typed::TypedTrees<'a>,
) -> Result<Vec<MachineInstance<'a>>, LoweringError> {
    let mut by_caller: HashMap<&DeclarationId, Vec<&TypedCall>> = HashMap::new();
    for call in typed
        .calls
        .iter()
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
    let mut queue = Vec::new();
    let mut seen = HashSet::new();

    for &sig in declarations {
        let file = trees[sig.node.file].resolved().file;
        if file.generic_parameters(sig.node.node).next().is_none() {
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
        let template = templates[application.machine.as_ref()];
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
                callbacks.push(Callback::bind(slot, selected, &bound.2, &templates)?);
            }
        }
        let tree = LoweringTree::for_application(&trees[template.node.file], &signature, &application, &callbacks)?;
        for call in by_caller.get(&template.machine).into_iter().flatten() {
            let next = if let Some(selected) = selected_implementation(typed, &templates, trees, call, &application)? {
                selected
            } else {
                let Some(&target) = templates.get(&call.machine) else { continue };
                let target_file = trees[target.node.file].resolved().file;
                let count = target_file.generic_parameters(target.node.node).count();
                if count == 0 && call.type_arguments.is_empty() && call.enclosing_arguments.is_empty() {
                    continue;
                }
                if !call.enclosing_arguments.is_empty()
                    || count != call.type_arguments.len()
                    || !target_file
                        .generic_parameters(target.node.node)
                        .zip(&call.type_arguments)
                        .all(|((_, _, kind), arg)| {
                            kind == NodeKind::TypeParam && !matches!(arg, ValueType::MachineSelection(_))
                                || kind == NodeKind::ConstParam
                                    && matches!(application.instantiate(arg), ValueType::MachineSelection(_) | ValueType::Implementation(_))
                        })
                {
                    return Err(LoweringError::new(
                        &target.site,
                        "open generic machine requires a closed application before lowering",
                    ));
                }
                MachineApplication {
                    machine: Box::new(target.machine.clone()),
                    arguments: call
                        .type_arguments
                        .iter()
                        .map(|v| application.instantiate(v))
                        .collect(),
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

/// A member call whose closed evidence selects a conformance runs that row's
/// implementation. The requirement's own static arguments specialize the
/// implementation by binder position. Without them the implementation is a
/// rooted declaration and a boundary call has no body to schedule.
fn selected_implementation(
    typed: &omega_typed::TypedTrees<'_>,
    templates: &HashMap<&DeclarationId, &TypedSignature>,
    trees: &[TypedTree<'_>],
    call: &TypedCall,
    application: &MachineApplication,
) -> Result<Option<MachineApplication>, LoweringError> {
    if call.type_arguments.is_empty() || call.evidence.is_none() {
        return Ok(None);
    }
    let closed = call.with_substitutions(|v| application.instantiate(v));
    let Some(key) = crate::declarations::members::Key::from_call(&closed) else {
        return Ok(None);
    };
    let Some(implementation) = key.implementation(typed, &call.site)? else {
        return Ok(None);
    };
    let target = templates
        .get(implementation)
        .ok_or_else(|| LoweringError::new(&call.site, "selected conformance implementation needs its packaged generic body"))?;
    let file = trees[target.node.file].resolved().file;
    let arguments = key.static_arguments();
    let selection = |arg: &ValueType| matches!(arg, ValueType::MachineSelection(_) | ValueType::Implementation(_));
    if file.generic_parameters(target.node.node).count() != arguments.len()
        || !file
            .generic_parameters(target.node.node)
            .zip(arguments)
            .all(|((_, _, kind), arg)| kind == NodeKind::TypeParam && !selection(arg) || kind == NodeKind::ConstParam && selection(arg))
    {
        return Err(LoweringError::new(
            &target.site,
            "selected implementation binders require a supported closed application",
        ));
    }
    Ok(Some(MachineApplication {
        machine: Box::new(target.machine.clone()),
        arguments: arguments.to_vec(),
        enclosing_arguments: vec![],
    }))
}
