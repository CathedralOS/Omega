//! Unfinished checks block consumers of their types, bodies, and contracts.
//! A static expression can still be ready inside an unfinished declaration.
use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::{InvocationTarget, ObligationKind, StaticValueDemand, TypeError, TypedStage, ValueType};
use omega_parser::NodeKind;
use omega_resolver::{DeclarationId, Symbol, SymbolResolvedTrees, TreeNodeId};
use std::collections::HashMap;

#[derive(Default)]
pub(crate) struct PreparationDependencies<'a> {
    declarations: HashMap<&'a DeclarationId, TreeNodeId>,
    unavailable: Vec<Vec<bool>>,
}

impl<'a> PreparationDependencies<'a> {
    pub(super) fn collect(
        resolved: &'a SymbolResolvedTrees<'_>,
        catalog: &DeclarationCatalog<'_>,
        output: &TypedStage,
        demands: &[StaticValueDemand],
        blocked: Vec<TreeNodeId>,
    ) -> Self {
        if demands.is_empty() {
            debug_assert!(blocked.is_empty(), "a postponed check must have an outstanding demand");
            return Self::default();
        }
        let files: Vec<_> = resolved.trees().map(|tree| tree.file).collect();
        let declarations: HashMap<_, _> = resolved
            .trees()
            .flat_map(|tree| {
                tree.declaration_nodes()
                    .map(move |(node, id)| (id, tree.file.node_id(node)))
            })
            .collect();

        let mut dependents = Vec::with_capacity(output.calls.len());
        let mut depend_on = |user, declaration: &DeclarationId| {
            if let Some(&node) = declarations.get(declaration) {
                dependents.push((node, user));
            }
        };
        // The whole stored shape must be checked, including fields of other
        // sum cases and inline field declarations with their own bodies.
        for (owner, fields) in &catalog.fields {
            if let Some(&node) = declarations.get(owner) {
                for declaration in fields
                    .ordered
                    .iter()
                    .flat_map(|field| type_dependencies(&field.ty))
                {
                    depend_on(node, declaration);
                }
            }
        }
        for (case, owner) in &catalog.case_owner {
            if let Some(&node) = declarations.get(owner) {
                depend_on(node, case);
            }
        }
        for (machine, signature) in &catalog.signatures {
            if let Some(&node) = declarations.get(machine) {
                for declaration in signature
                    .parameters
                    .iter()
                    .map(|(_, ty)| ty)
                    .chain([&signature.result])
                    .flat_map(type_dependencies)
                {
                    depend_on(node, declaration);
                }
            }
        }
        for expression in &output.expressions {
            for declaration in type_dependencies(&expression.ty) {
                depend_on(expression.node, declaration);
            }
        }
        for demand in demands {
            let request = demand.request();
            for declaration in type_dependencies(&request.carrier) {
                depend_on(request.node, declaration);
            }
        }
        for call in &output.calls {
            depend_on(call.node, &call.machine);
            if call.layout_query.is_some() {
                for declaration in call.type_arguments.iter().flat_map(type_dependencies) {
                    depend_on(call.node, declaration);
                }
            }
            for argument in call
                .type_arguments
                .iter()
                .chain(
                    call.enclosing_arguments
                        .iter()
                        .flat_map(|(_, arguments)| arguments),
                )
                .flat_map(ValueType::components)
            {
                if let ValueType::MachineSelection(machine, _) = argument {
                    depend_on(call.node, machine);
                }
            }
            for operand in &call.operands {
                if let Some(declaration) = operand.callable.as_ref().and_then(invocation_declaration) {
                    depend_on(call.node, declaration);
                }
            }
        }
        for call in &output.indirect_calls {
            if let Some(declaration) = invocation_declaration(&call.entry) {
                depend_on(call.node, declaration);
            }
        }
        for (node, constant) in &output.constant_references {
            depend_on(*node, constant);
        }
        for row in &output.conformance_rows {
            depend_on(row.node, &row.implementation);
            // An inline row can be the implementation declaration itself;
            // the conformance still needs that implementation's checks.
            if let Some(owner) = &row.conformance
                && let Some(&node) = declarations.get(owner)
            {
                depend_on(node, &row.implementation);
            }
        }
        for obligation in &output.obligations {
            match &obligation.kind {
                ObligationKind::RefineCallable { machine, .. } | ObligationKind::RefineMathematicalCitation { machine, .. } => {
                    depend_on(obligation.node, machine)
                }
                ObligationKind::SupplyHypothesis {
                    evidence: ValueType::ProofCitation(machine),
                    ..
                } => depend_on(obligation.node, machine),
                ObligationKind::RefineEvidence { implementation, .. } => depend_on(obligation.node, implementation),
                _ => {}
            }
        }
        for usage in resolved.uses() {
            if let Symbol::Declaration(id) = &usage.target
                && (matches!(id.kind, NodeKind::Data | NodeKind::Case | NodeKind::Domain | NodeKind::Conformance)
                    || files[usage.node.file].nodes[usage.node.node]
                        .parent
                        .is_some_and(|parent| files[usage.node.file].nodes[parent].kind == NodeKind::DroppedBy))
            {
                depend_on(usage.node, id);
            }
        }

        let parent = |node: TreeNodeId| {
            let file = files[node.file];
            let syntax = &file.nodes[node.node];
            // A separately declared item has its own checks. A field, case,
            // contract clause or selection row contributes to its owner's checks.
            if matches!(
                syntax.kind,
                NodeKind::Data
                    | NodeKind::Machine
                    | NodeKind::AsmMachine
                    | NodeKind::Const
                    | NodeKind::Domain
                    | NodeKind::Trait
                    | NodeKind::MathLet
                    | NodeKind::Conformance
            ) {
                return None;
            }
            syntax.parent.map(|parent| file.node_id(parent))
        };
        dependents.sort_unstable();
        let mut unavailable: Vec<_> = files.iter().map(|file| vec![false; file.nodes.len()]).collect();
        let mut work = Vec::new();
        let block = |node: TreeNodeId, unavailable: &mut [Vec<bool>], work: &mut Vec<TreeNodeId>| {
            if !std::mem::replace(&mut unavailable[node.file][node.node], true) {
                work.push(node);
            }
        };
        for node in blocked {
            block(node, &mut unavailable, &mut work);
        }
        for demand in demands {
            // The missing value blocks its use. Its expression can run once
            // that expression and all its dependencies have completed typing.
            if let StaticValueDemand::Closed(request) = demand
                && let Some(user) = parent(request.node)
            {
                block(user, &mut unavailable, &mut work);
            }
        }
        while let Some(node) = work.pop() {
            let start = dependents.partition_point(|&(used, _)| used < node);
            let uses = dependents[start..]
                .iter()
                .take_while(|&&(used, _)| used == node)
                .map(|&(_, user)| user);
            for user in uses.chain(parent(node)) {
                block(user, &mut unavailable, &mut work);
            }
        }
        Self { declarations, unavailable }
    }

    pub(crate) fn node_is_ready(
        &self,
        node: TreeNodeId,
    ) -> bool {
        self.unavailable
            .get(node.file)
            .and_then(|nodes| nodes.get(node.node))
            .is_none_or(|blocked| !blocked)
    }

    pub(crate) fn declaration_is_ready(
        &self,
        declaration: &DeclarationId,
    ) -> bool {
        self.declarations
            .get(declaration)
            .is_none_or(|&node| self.node_is_ready(node))
    }

    pub(super) fn ready_demands(
        &self,
        mut demands: Vec<StaticValueDemand>,
    ) -> Result<Vec<StaticValueDemand>, TypeError> {
        if let Some(first) = demands.first()
            && demands
                .iter()
                .all(|demand| !self.node_is_ready(demand.request().node))
        {
            return Err(TypeError::for_site(
                &first.request().expression,
                "static evaluation is blocked by a typing dependency cycle",
            ));
        }
        demands.retain(|demand| self.node_is_ready(demand.request().node));
        Ok(demands)
    }
}

fn type_dependencies(ty: &ValueType) -> impl Iterator<Item = &DeclarationId> {
    ty.components().flat_map(|part| {
        part.nominal_parts()
            .map(|(id, _)| id)
            .into_iter()
            .chain(part.domains().iter().map(|domain| &domain.declaration))
    })
}

fn invocation_declaration(target: &InvocationTarget) -> Option<&DeclarationId> {
    match target {
        InvocationTarget::Declaration(slot) => Some(&slot.declaration),
        InvocationTarget::AppliedMachine { declaration, .. } => Some(declaration),
        _ => None,
    }
}
