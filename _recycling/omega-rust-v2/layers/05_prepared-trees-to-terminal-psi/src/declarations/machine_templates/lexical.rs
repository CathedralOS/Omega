use super::*;
use crate::{TemplateResult, TemplateSelection};
use omega_resolved::DeclarationId;
use omega_typed::{MachineApplication, TypedStage};

type Captures = Vec<(crate::DeclarationId, Vec<TypeExpression>)>;

pub(super) fn captures(
    typed: &TypedStage,
    machine: &DeclarationId,
    rows: &[(DeclarationId, Vec<ValueType>)],
    site: &Site,
) -> Result<Captures, LoweringError> {
    // The same typed field also carries enclosing trait arguments at member
    // calls. Those are not lexical machine captures and need member lowering.
    let lexical = std::iter::successors(machine.enclosing_machine.as_ref(), |owner| owner.enclosing_machine.as_ref())
        .filter_map(|owner| Some((owner, typed.generic_parameters.get(owner)?.parameters.len())))
        .filter(|(_, count)| *count != 0);
    if !lexical.eq(rows.iter().map(|(owner, args)| (owner, args.len()))) {
        return Err(LoweringError::new(site, "template selection needs its exact lexical machine arguments"));
    }
    rows.iter().map(|(owner, args)| Ok((
        crate::identity::declaration(owner, site)?,
        args.iter().map(|arg| type_expression(arg, site)).collect::<Result<_, _>>()?,
    ))).collect()
}

/// A static integer operand, as in `pick::<3>`, is a value index of the
/// selected machine. It is published as data applications publish theirs.
pub(super) fn selection(typed: &TypedStage, application: &MachineApplication, site: &Site) -> Result<TemplateSelection, LoweringError> {
    Ok(TemplateSelection {
        declaration: crate::identity::declaration(&application.machine, site)?,
        arguments: application.arguments.iter().map(|arg| super::super::type_expression::indexed_expression(arg, site)).collect::<Result<_, _>>()?,
        enclosing_arguments: captures(typed, &application.machine, &application.enclosing_arguments, site)?,
    })
}

/// The typed occurrence must be a static argument read. Its resolved binder
/// node identifies the declaring machine and position, even across scopes with
/// the same carrier. Don't use the flat evaluation slot as a binder ordinal.
pub(super) fn static_result(typed: &TypedStage, tree: &TypedTree<'_>, sig: &TypedSignature, node: usize) -> Option<TemplateResult> {
    let mut rows = tree.semantics_at(node);
    let NodeSemantics::Expression(e) = rows.next()? else { return None };
    if rows.next().is_some() || !static_value_carrier(&e.ty) || e.ty != sig.result || e.static_argument_slot.is_none() {
        return None;
    }
    let resolved = tree.resolved();
    let [Symbol::Binding { binder_node, kind: NodeKind::ConstParam, .. }] = resolved.symbol(node)?.visible_names() else { return None };
    let machine = resolved.file.ancestors(*binder_node).find(|&n| resolved.file.nodes[n].kind == NodeKind::Machine)?;
    let owner = resolved.declaration_at(machine)?;
    let index = resolved.file.generic_parameters(machine).position(|(_, n, _)| n == *binder_node)?;
    let param = typed.generic_parameters.get(owner)?.parameters.get(index)?;
    if param.kind != NodeKind::ConstParam || param.carrier.as_deref() != Some(&e.ty) { return None }
    Some(TemplateResult::StaticParameter { owner: crate::identity::portable(owner)?, index, site: e.site.clone() })
}

/// The closed scalar carriers for which a template can return the selected static value.
pub(super) fn static_value_carrier(carrier: &ValueType) -> bool {
    match carrier {
        ValueType::Bool => true,
        ValueType::Integer { width: Some(width), .. } => *width > 0,
        _ => false,
    }
}
