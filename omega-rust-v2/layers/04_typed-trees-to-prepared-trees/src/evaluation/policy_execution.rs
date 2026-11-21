//! Evaluate the immutable inputs of a policy's demanded construction. Frames
//! hold only frozen input data and call-scoped shared borrows of that data.
//! Stop before publishing a value whose default domain has not been proved.
use omega_prepared::{PolicyConstruction, PolicyFieldInput, PreparedValue};
use omega_resolved::{DeclarationId, Symbol};
use omega_syntax::NodeKind;
use omega_typed::{
    Access, AccessPolicySelection, CallUse, MachineApplication, Multiplicity, NodeSemantics, ObligationKind, TypedSignature, TypedTree, TypedTrees, ValueType,
    site_at,
};
use std::collections::HashSet;

#[derive(Clone)]
enum Operand {
    Value(PreparedValue),
    Shared(PreparedValue),
}

pub(super) fn prepare(
    forest: &TypedTrees<'_>,
    selection: &AccessPolicySelection,
    schema: &PreparedValue,
    boundary_slots: &[omega_typed::BoundarySlot],
) -> Option<PolicyConstruction> {
    let binders = &forest
        .generic_parameters
        .get(&selection.plan_machine)?
        .parameters;
    let [region] = binders.as_slice() else { return None };
    if region.kind != NodeKind::LifetimeParam {
        return None;
    }
    let mut applied = MachineApplication {
        machine: Box::new(selection.plan_machine.clone()),
        arguments: vec![ValueType::InferredLifetime {
            site: Box::new(selection.site.clone()),
            index: 0,
        }],
        enclosing_arguments: vec![(selection.policy.clone(), selection.arguments.clone())],
    };
    let mut arguments = vec![Operand::Value(schema.clone())];
    let mut current = forest;
    let mut seen = HashSet::new();
    loop {
        if !seen.insert(applied.machine.clone())
            || boundary_slots.iter().any(|slot| {
                slot.requirement.declaration == *applied.machine && slot.requirement.trait_arguments.is_empty() && slot.arguments == applied.arguments
            })
        {
            return None;
        }
        let (tree, sig) = definition(current, &applied.machine)?;
        current = tree.forest();
        let effects = current
            .machine_effects
            .iter()
            .find(|row| row.machine == sig.machine)?;
        if !effects.terminates
            || effects.pause.suspend
            || effects.pause.block
            || !effects.reach.is_empty()
            || !effects.invokes.is_empty()
            || effects
                .crash_ceiling
                .as_ref()
                .is_some_and(|causes| !causes.is_empty())
            || sig.requires
            || sig.satisfies.is_some()
            || !sig.multiplicity_bounds.is_empty()
        {
            return None;
        }
        let file = tree.resolved().file;
        if file.child(sig.node.node, NodeKind::Hypothesis).is_some() {
            return None;
        }
        let params = file.child(sig.node.node, NodeKind::Params)?;
        let formals = &file.nodes[params].children;
        if formals.len() != arguments.len() || formals.len() != sig.parameters.len() {
            return None;
        }
        let body = file.child(sig.node.node, NodeKind::Body)?;
        let [tail] = *file.nodes[body].children.as_slice() else { return None };
        if file.nodes[tail].kind != NodeKind::Tail {
            return None;
        }
        let [result] = *file.nodes[tail].children.as_slice() else { return None };
        let frame = Frame {
            tree: &tree,
            applied: &applied,
            sig,
            formals,
            arguments: &arguments,
        };
        let result_type = applied.instantiate(&sig.result);
        if tree.construction(result).is_some() {
            let (fields, constrained) = frame.fields(result, &result_type, true)?;
            return constrained.then(|| PolicyConstruction {
                machine: applied,
                node: file.node_id(result),
                site: site_at(file, result),
                carrier: result_type,
                fields,
            });
        }
        let mut invocation = None;
        for row in tree.semantics_at(result) {
            match row {
                NodeSemantics::Call(call) if invocation.is_none() => invocation = Some(call),
                NodeSemantics::Expression(e) if applied.instantiate(&e.ty) == result_type => {}
                NodeSemantics::Obligation(o) if matches!(&o.kind, ObligationKind::ReturnBorrow { machine } if machine == &sig.machine) => {}
                _ => return None,
            }
        }
        let call = invocation?;
        if call.use_site != CallUse::Execution
            || call.caller.as_ref() != Some(&sig.machine)
            || call.evidence.is_some()
            || call.acknowledgements.suspend
            || call.acknowledgements.block
            || call.layout_query.is_some()
            || current
                .boundary_calls
                .iter()
                .any(|(site, _)| site == &call.site)
        {
            return None;
        }
        let next = MachineApplication {
            machine: Box::new(call.machine.clone()),
            arguments: call.type_arguments.clone(),
            enclosing_arguments: call.enclosing_arguments.clone(),
        }
        .with_substitutions(|ty| applied.instantiate(ty));
        let (_, target) = definition(current, &call.machine)?;
        if next.instantiate(&target.result) != result_type || target.parameters.len() != call.operands.len() {
            return None;
        }
        arguments = call
            .operands
            .iter()
            .zip(&target.parameters)
            .map(|(operand, (_, ty))| {
                if operand.callable.is_some() || operand.node.file != file.ordinal() || operand.site != site_at(file, operand.node.node) {
                    return None;
                }
                frame.operand(operand.node.node, &next.instantiate(ty))
            })
            .collect::<Option<_>>()?;
        applied = next;
    }
}

fn definition<'a>(
    forest: &'a TypedTrees<'_>,
    machine: &DeclarationId,
) -> Option<(TypedTree<'a>, &'a TypedSignature)> {
    if let Some(definition) = forest.imported_machine_definition(machine) {
        return Some(definition);
    }
    let sig = forest.signatures.iter().find(|sig| &sig.machine == machine)?;
    let tree = forest.trees().nth(sig.node.file)?;
    (tree.resolved().declaration_at(sig.node.node) == Some(machine) && site_at(tree.resolved().file, sig.node.node) == sig.site).then_some((tree, sig))
}

struct Frame<'a, 'tree> {
    tree: &'a TypedTree<'tree>,
    applied: &'a MachineApplication,
    sig: &'a TypedSignature,
    formals: &'a [usize],
    arguments: &'a [Operand],
}

impl Frame<'_, '_> {
    fn formal(
        &self,
        node: usize,
        ty: &ValueType,
    ) -> Option<Operand> {
        let mut rows = self.tree.semantics_at(node);
        let Some(NodeSemantics::Expression(e)) = rows.next() else { return None };
        if self.applied.instantiate(&e.ty) != *ty || e.static_argument_slot.is_some() || rows.next().is_some() {
            return None;
        }
        let Symbol::Binding {
            binder_node,
            kind: NodeKind::Param,
            ..
        } = self.tree.resolved().symbol(node)?
        else {
            return None;
        };
        let position = self.formals.iter().position(|formal| formal == binder_node)?;
        (self.applied.instantiate(&self.sig.parameters[position].1) == *ty).then(|| self.arguments[position].clone())
    }

    fn leaf(
        &self,
        node: usize,
        ty: &ValueType,
    ) -> Option<Operand> {
        let file = self.tree.resolved().file;
        let mut rows = self.tree.semantics_at(node);
        let Some(NodeSemantics::Expression(e)) = rows.next() else { return None };
        if self.applied.instantiate(&e.ty) != *ty || e.static_argument_slot.is_some() {
            return None;
        }
        match file.nodes[node].kind {
            NodeKind::PathExpression => self.formal(node, ty),
            NodeKind::ArrayLiteral if matches!(ty, ValueType::Slice(_)) && file.nodes[node].children.is_empty() && rows.next().is_none() => {
                Some(Operand::Value(PreparedValue::Array([].into())))
            }
            NodeKind::Borrow => {
                let [place] = *file.nodes[node].children.as_slice() else { return None };
                let ValueType::Reference(Access::Shared, referent) = ty else { return None };
                let Some(NodeSemantics::Obligation(o)) = rows.next() else { return None };
                if rows.next().is_some() || !matches!(&o.kind, ObligationKind::Borrow { access: Access::Shared, place: at } if *at == site_at(file, place)) {
                    return None;
                }
                let Operand::Value(value) = self.formal(place, referent)? else { return None };
                Some(Operand::Shared(value))
            }
            NodeKind::Dereference => {
                let [place] = *file.nodes[node].children.as_slice() else { return None };
                let Some(NodeSemantics::Obligation(o)) = rows.next() else { return None };
                if rows.next().is_some() || !matches!(&o.kind, ObligationKind::CopyRead(read) if self.applied.instantiate(read) == *ty) {
                    return None;
                }
                let forest = self.tree.forest();
                let declared = |id: &DeclarationId| {
                    forest
                        .data_properties
                        .iter()
                        .chain(forest.library.into_iter().flat_map(|lib| &lib.data_properties))
                        .find(|(owner, _)| owner == id)
                        .map(|(_, p)| p.multiplicity)
                };
                if ty.multiplicity_with(&declared, &|_, _| None) != Some(Multiplicity::Copy) {
                    return None;
                }
                let Operand::Shared(value) = self.formal(place, &ValueType::Reference(Access::Shared, Box::new(ty.clone())))? else {
                    return None;
                };
                Some(Operand::Value(value))
            }
            _ => None,
        }
    }

    fn operand(
        &self,
        node: usize,
        ty: &ValueType,
    ) -> Option<Operand> {
        let Some(construction) = self.tree.construction(node) else {
            return self.leaf(node, ty);
        };
        let (fields, constrained) = self.fields(node, ty, false)?;
        if constrained {
            return None;
        }
        let fields = fields
            .into_iter()
            .map(|(_, input)| match input {
                PolicyFieldInput::Written(value) => Some(value),
                PolicyFieldInput::Zero(ValueType::Slice(_)) => Some(PreparedValue::Array([].into())),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?
            .into();
        let owner = construction.owner.clone();
        Some(Operand::Value(match &construction.case {
            Some(case) => PreparedValue::Sum {
                owner,
                case: case.clone(),
                fields,
            },
            None => PreparedValue::Record { owner, fields },
        }))
    }

    fn fields(
        &self,
        node: usize,
        ty: &ValueType,
        returned: bool,
    ) -> Option<(Vec<(DeclarationId, PolicyFieldInput)>, bool)> {
        let construction = self.tree.construction(node)?;
        let (owner, arguments) = ty.nominal_parts()?;
        if owner != &construction.owner || self.applied.instantiate(&construction.carrier) != *ty {
            return None;
        }
        let forest = self.tree.forest();
        let shape = construction.case.as_ref().unwrap_or(owner);
        let declarations = forest
            .record_fields
            .get(shape)
            .or_else(|| forest.library?.record_fields.get(shape))
            .map_or(&[][..], |rows| rows.as_slice());
        let mut inputs = vec![None; declarations.len()];
        for init in &construction.initializers {
            let field = declarations.get(init.position)?;
            let Operand::Value(value) = self.leaf(init.expression, &field.ty.substitute(owner, arguments))? else {
                return None;
            };
            if inputs[init.position]
                .replace(PolicyFieldInput::Written(value))
                .is_some()
            {
                return None;
            }
        }
        let mut constrained = false;
        for row in self.tree.semantics_at(node) {
            match row {
                NodeSemantics::Expression(e) if self.applied.instantiate(&e.ty) == *ty => {}
                NodeSemantics::Obligation(o) => match &o.kind {
                    ObligationKind::EstablishData(id) if id == owner && !constrained => constrained = true,
                    ObligationKind::ReturnBorrow { machine } if returned && machine == &self.sig.machine => {}
                    ObligationKind::ZeroField { field, ty } => {
                        let slot = declarations.iter().position(|d| &d.declaration == field)?;
                        let field_type = self.applied.instantiate(ty);
                        if declarations[slot].ty.substitute(owner, arguments) != field_type
                            || inputs[slot]
                                .replace(PolicyFieldInput::Zero(field_type))
                                .is_some()
                        {
                            return None;
                        }
                    }
                    _ => return None,
                },
                _ => return None,
            }
        }
        let fields = declarations
            .iter()
            .zip(inputs)
            .map(|(field, input)| Some((field.declaration.clone(), input?)))
            .collect::<Option<_>>()?;
        Some((fields, constrained))
    }
}
