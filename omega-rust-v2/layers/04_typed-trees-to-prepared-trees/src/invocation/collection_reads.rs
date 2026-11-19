use super::*;
use omega_toolchain::{CollectionPrimitive, core_interface};
use omega_typer::{Access, Projection};

impl ConstantEvaluation<'_, '_> {
    pub(super) fn schedule_array_read<'a>(
        &self,
        tree: &TypedTree<'_>,
        node: usize,
        result: &ValueType,
        scope: &Scope<'a>,
        execution: &mut Execution<'a>,
    ) -> Result<bool, EvaluationError> {
        let file = tree.resolved().file;
        let site = file.node_id(node);
        let read = core_interface().collection(CollectionPrimitive::Read);
        let Some(access) = tree.semantics_at(node).find_map(|record| match record {
            NodeSemantics::CollectionAccess(access) => Some(access),
            _ => None,
        }) else {
            return Ok(false);
        };
        let [base, selector] = *file.nodes[node].children.as_slice() else {
            return Err(UNSUPPORTED_BODY.into());
        };
        let base = file.node_id(file.ungroup(base));
        let selector = file.node_id(selector);
        let base_type = checked_carrier_in_scope(tree, base.node, scope.application())?;
        let ValueType::Array(element, _) = &base_type else {
            return Err(UNSUPPORTED_BODY.into());
        };
        if &access.machine != read
            || &scope.instantiate(&access.element) != element.as_ref()
            || &scope.instantiate(&access.result) != result
            || element.as_ref() != result
        {
            return Err(UNSUPPORTED_BODY.into());
        }
        let (_, path) = self.trees.place_path(site);
        if !path
            .last()
            .is_some_and(|step| step.base == base && step.through.is_none() && step.projection == Some(Projection::Index(selector)))
        {
            return Err(UNSUPPORTED_BODY.into());
        }
        let mut bound = false;
        let mut borrowed = false;
        for record in tree.semantics_at(node) {
            match record {
                NodeSemantics::Expression(_) | NodeSemantics::CollectionAccess(_) => {}
                NodeSemantics::Obligation(o) => match &o.kind {
                    ObligationKind::CallPrecondition(machine) if machine == read => bound = true,
                    ObligationKind::Borrow { access: Access::Shared, place }
                        if place.span == file.span(file.nodes[node].children[0]) && place.file == file.path && place.package == file.package =>
                    {
                        borrowed = true
                    }
                    other if self.admits_owned_return(scope, other, result) => {}
                    _ => return Err(UNSUPPORTED_BODY.into()),
                },
                _ => return Err(UNSUPPORTED_BODY.into()),
            }
        }
        let index_type = checked_carrier_in_scope(tree, selector.node, scope.application())?;
        if !bound
            || !borrowed
            || index_type
                != (ValueType::Integer {
                    signed: false,
                    width: Some(64),
                })
        {
            return Err(UNSUPPORTED_BODY.into());
        }
        execution.steps.extend([
            Step::ReadArray,
            Step::Evaluate(selector, index_type, scope.clone()),
            Step::Evaluate(base, base_type, scope.clone()),
        ]);
        Ok(true)
    }

    pub(super) fn schedule_array_length<'a>(
        &self,
        tree: &TypedTree<'_>,
        node: usize,
        result: &ValueType,
        scope: &Scope<'a>,
        execution: &mut Execution<'a>,
    ) -> Result<bool, EvaluationError> {
        let file = tree.resolved().file;
        let Some(operation) = tree.semantics_at(node).find_map(|record| match record {
            NodeSemantics::Operation(op) if op.token == ".len" => Some(op),
            _ => None,
        }) else {
            return Ok(false);
        };
        let [base] = *file.nodes[node].children.as_slice() else {
            return Err(UNSUPPORTED_BODY.into());
        };
        let base = file.node_id(file.ungroup(base));
        let base_type = checked_carrier_in_scope(tree, base.node, scope.application())?;
        if !matches!(base_type, ValueType::Array(..))
            || !matches!(operation.operands.as_slice(), [ty] if scope.instantiate(ty) == base_type)
            || operation.result != *result
            || tree
                .semantics_at(node)
                .any(|record| !matches!(record, NodeSemantics::Expression(_) | NodeSemantics::Operation(_)))
        {
            return Err(UNSUPPORTED_BODY.into());
        }
        execution.steps.extend([
            Step::ArrayLength,
            Step::Evaluate(base, base_type, scope.clone()),
        ]);
        Ok(true)
    }
}
