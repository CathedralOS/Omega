use crate::{InvocationClause, LoweringError, SynchronousEntry, site_at};
use omega_syntax::NodeKind;
use omega_typed::{InvocationTarget, MachineApplication, Site, TypedSignature, TypedTree, ValueType};

/// A missing or extra typed occurrence cannot turn a written clause into an empty ceiling.
pub(super) fn authored(
    tree: &TypedTree<'_>,
    signature: &TypedSignature,
    application: Option<&MachineApplication>,
) -> Result<Vec<InvocationClause>, LoweringError> {
    let file = tree.resolved().file;
    let mut clauses = Vec::new();
    let mut records = std::collections::HashMap::new();
    for (owner, site, target) in &tree.forest().invocation_ceilings {
        if owner == &signature.machine && records.insert(site, target).is_some() {
            return Err(LoweringError::new(site, "entry clause has duplicate checked targets"));
        }
    }
    for &child in &file.nodes[signature.node.node].children {
        if file.nodes[child].kind != NodeKind::Invokes { continue; }
        let site = site_at(tree, child);
        let target = records.remove(&site)
            .ok_or_else(|| LoweringError::new(&site, "entry clause lacks its checked occurrence"))?;
        let entry = target_entry(tree, signature, target, application, &site)?;
        clauses.push(InvocationClause { site, entry });
    }
    if let Some(site) = records.keys().min_by_key(|at| at.span.get_start_offset()) {
        return Err(LoweringError::new(site, "checked entry has no authored clause"));
    }
    Ok(clauses)
}

pub(super) fn target_entry(
    tree: &TypedTree<'_>, signature: &TypedSignature, target: &InvocationTarget,
    application: Option<&MachineApplication>, site: &Site,
) -> Result<SynchronousEntry, LoweringError> {
    let instantiate = |ty: &ValueType| match application {
        Some(app) => app.instantiate(ty),
        None => ty.clone(),
    };
    Ok(match target {
        InvocationTarget::Declaration(slot) => {
            let arguments = slot.trait_arguments.iter().map(instantiate).collect::<Vec<_>>();
            SynchronousEntry::Declaration(super::members::slot(&slot.declaration, &arguments, site)?)
        }
        InvocationTarget::RuntimeParameter { owner, index } if owner == &signature.machine
            && matches!(signature.parameters.get(*index), Some((_, ValueType::Callable(_)))) =>
                SynchronousEntry::RuntimeParameter(*index),
        InvocationTarget::StaticParameter { owner, index } if owner == &signature.machine
            && tree.forest().generic_parameters.get(owner).and_then(|params| params.parameters.get(*index))
                .is_some_and(|param| matches!(param.carrier.as_deref(), Some(ValueType::Callable(_)))) =>
        {
            let binder = ValueType::Parameter { owner: Box::new(owner.clone()), index: *index };
            match instantiate(&binder) {
                ValueType::MachineSelection(selected) =>
                    SynchronousEntry::AppliedMachine(super::machine_templates::selection(tree.forest(), &selected, site)?),
                value if value == binder => SynchronousEntry::StaticParameter(*index),
                _ => return Err(LoweringError::new(site, "entry binder needs its selected callable")),
            }
        }
        InvocationTarget::AppliedMachine(target) => SynchronousEntry::AppliedMachine(
            super::machine_templates::selection(tree.forest(), &target.with_substitutions(instantiate), site)?),
        InvocationTarget::StoredCallable(at) => SynchronousEntry::StoredCallable(at.clone()),
        _ => return Err(LoweringError::new(site, "synchronous entry lacks its owning callable parameter")),
    })
}
