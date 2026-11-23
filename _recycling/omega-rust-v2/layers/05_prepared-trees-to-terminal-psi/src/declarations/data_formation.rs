//! Formation questions belong to the checker. The field type uses normalized
//! indices; its authored arithmetic can still owe separate range proofs.
use super::type_expression::indexed_expression;
use crate::{LoweringError, TypeExpression, TypeExpressionStep as Step};
use omega_psi::{DataFormation, ScalarType};
use omega_resolved::DeclarationId;
use omega_syntax::NodeKind;
use omega_typed::{NodeSemantics, ObligationKind, Site, TypedTree, TypingObligation, ValueType};

/// One binder of the owner's telescope at its argument position: its
/// declaration, its index there and its carrier. A lifetime binder keeps its
/// position, since a static operand names its binder by argument slot, and
/// has no carrier, so no index operand is ever one.
type Binder<'a> = (&'a DeclarationId, usize, Option<&'a ValueType>);

pub(super) fn read(tree: &TypedTree<'_>, scope: usize, owner: &DeclarationId) -> Result<Vec<(Site, DataFormation)>, LoweringError> {
    let file = tree.resolved().file;
    let forest = tree.forest();
    let owner_site = crate::site_at(tree, scope);
    let mut binders = Vec::new();
    for id in forest.enclosing_data_parameters.get(owner).into_iter().flatten().chain(std::iter::once(owner)) {
        if let Some(scope) = forest.generic_parameters.get(id) {
            for (index, p) in scope.parameters.iter().enumerate() {
                let ty = if p.kind == NodeKind::LifetimeParam {
                    None
                } else {
                    Some(p.carrier.as_deref().filter(|_| p.kind == NodeKind::ConstParam)
                        .ok_or_else(|| LoweringError::new(&owner_site, "static formation requires its declared binder carrier"))?)
                };
                binders.push((id, index, ty));
            }
        }
    }
    let mut rows = Vec::new();
    for o in &forest.obligations {
        let at = o.node.node;
        let in_owner = o.site.package == owner_site.package && o.site.file == owner_site.file
            && o.node.file == file.node_id(scope).file && (scope..file.nodes[scope].end).contains(&at)
            && std::iter::successors(Some(at), |&n| file.nodes[n].parent)
                .find(|&n| matches!(file.nodes[n].kind, NodeKind::Data | NodeKind::Machine)) == Some(scope);
        let terms = |args: &[ValueType]| args.iter().map(|arg| indexed_expression(arg, &o.site)).collect::<Result<_, _>>();
        let question = match &o.kind {
            ObligationKind::FormRecursiveApplication { declaration, active_arguments, arguments } if declaration == owner => {
                DataFormation::RecursiveApplication { active_arguments: terms(active_arguments)?, arguments: terms(arguments)? }
            }
            ObligationKind::PositiveWidth(width) if in_owner => DataFormation::PositiveWidth(indexed_expression(width, &o.site)?),
            ObligationKind::OperationContract { operands, .. } if in_owner => {
                // Predicates are separately lowered into the data's contract.
                if file.ancestors(at).take_while(|&n| n != scope).any(|n| file.nodes[n].kind == NodeKind::Requires) {
                    continue;
                }
                let refuse = || LoweringError::new(&o.site, "static index operation needs its exact operands and carrier");
                let [left, right] = file.nodes[at].children.as_slice() else { return Err(refuse()) };
                let [ty, other] = operands.as_slice() else { return Err(refuse()) };
                let op = tree.semantics_at(at).find_map(|s| match s {
                    NodeSemantics::Operation(op) if op.result == *ty && ty == other => Some(op),
                    _ => None,
                }).ok_or_else(refuse)?;
                let operation = crate::machines::arithmetic::index_operation(op, o, owner)?;
                let carrier = crate::lowering::types::scalar_carrier(ty).map_err(|_| refuse())?;
                let read = |n| operand(tree, n, ty, carrier, &binders, o);
                DataFormation::IntegerOperation { operation, carrier, operands: [read(*left)?, read(*right)?] }
            }
            _ => continue,
        };
        rows.push((o.site.clone(), question));
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(rows)
}

fn operand(
    tree: &TypedTree<'_>, node: usize, ty: &ValueType, carrier: ScalarType, binders: &[Binder<'_>], question: &TypingObligation,
) -> Result<TypeExpression, LoweringError> {
    let file = tree.resolved().file;
    let n = file.ungroup(node);
    for row in tree.semantics_at(n) {
        match row {
            NodeSemantics::Expression(e) if &e.ty == ty => {
                if let Some(&(id, index, Some(bound))) = e.static_argument_slot.and_then(|slot| binders.get(slot)) && bound == ty {
                    let owner = crate::identity::declaration(id, &question.site)?;
                    return Ok(TypeExpression(vec![Step::GenericParameter { owner, index }]));
                }
            }
            NodeSemantics::Landing(l) if file.nodes[n].kind == NodeKind::Literal && &l.ty == ty && l.value.is_integer() => {
                return Ok(TypeExpression(vec![Step::StaticInteger { carrier, value: l.value.to_integer() }]));
            }
            _ => {}
        }
    }
    Err(LoweringError::new(&crate::site_at(tree, n), "static index operation needs a checked binder or integer literal"))
}
