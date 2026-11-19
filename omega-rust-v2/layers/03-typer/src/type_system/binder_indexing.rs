use crate::declarations::data_properties;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::type_system::type_catalog::TypeCatalog;
use crate::{TypeError, ValueType};
use omega_lexer::SourceSpan;
use omega_parser::NodeKind;
use omega_resolver::{DeclarationId, Symbol};
use std::collections::HashMap;

impl<'a> ResolvedSourceFile<'a> {
    pub fn index_generic_parameters(
        &mut self,
        types: &mut TypeCatalog<'a>,
    ) -> Result<(), TypeError> {
        let source = &self.file;
        for &id in self.definitions.values() {
            if matches!(
                id.kind,
                NodeKind::Machine | NodeKind::AsmMachine | NodeKind::MathLet | NodeKind::Data | NodeKind::Trait | NodeKind::Conformance | NodeKind::Domain
            ) {
                types.arities.insert(id, 0);
            }
        }
        for (_, node) in source
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.kind == NodeKind::GenericParams)
        {
            let owner = self.declaration(node.parent.unwrap())?;
            let mut index = 0;
            for &p in &node.children {
                if source.nodes[p].kind == NodeKind::SomeParam {
                    if owner.kind != NodeKind::Data {
                        return Err(TypeError::unsupported(source, p));
                    }
                    let name = source.nodes[p]
                        .words()
                        .nth(1)
                        .ok_or_else(|| TypeError::unsupported(source, p))?;
                    let witness = types.witnesses.entry(owner).or_default();
                    self.parameters.insert(
                        name.span,
                        ValueType::Witness {
                            owner: Box::new(owner.clone()),
                            index: *witness,
                            opening: None,
                        },
                    );
                    *witness += 1;
                    continue;
                }
                let name = match source.nodes[p].kind {
                    NodeKind::TypeParam => {
                        for &attr in &source.nodes[p].children {
                            let prop = data_properties::read(self, attr)?;
                            if prop.carry.is_some() {
                                return Err(TypeError::unsupported(source, attr));
                            }
                            types.bounds.insert((owner, index), prop.multiplicity);
                        }
                        source.nodes[p].words().next()
                    }
                    NodeKind::KindedParam => source.nodes[p].words().next(),
                    NodeKind::ConstParam | NodeKind::MachineParam => source.nodes[p].words().nth(1),
                    NodeKind::LifetimeParam => {
                        types.lifetimes.insert((owner, index));
                        source.nodes[p].words().next()
                    }
                    _ => return Err(TypeError::unsupported(source, p)),
                }
                .ok_or_else(|| TypeError::unsupported(source, p))?;
                self.parameters.insert(
                    name.span,
                    ValueType::Parameter {
                        owner: Box::new(owner.clone()),
                        index,
                    },
                );
                index += 1;
            }
            types.arities.insert(owner, index);
        }
        Ok(())
    }
    pub fn application_segment(
        &self,
        path: usize,
        section: usize,
    ) -> Option<&'a Symbol> {
        let node = &self.file.nodes[path];
        let before = self.file.span(section).get_start_offset();
        let mut prefix = node.words().take_while(|w| w.span.get_start_offset() < before);
        let first = prefix.next()?;
        let last = prefix.last().unwrap_or(first);
        self.names
            .get(&SourceSpan::new(first.span.get_start_offset(), last.span.get_end_offset()))
            .copied()
    }
    pub fn index_enclosing_scopes(
        &self,
        types: &mut TypeCatalog<'a>,
    ) -> Result<(), TypeError> {
        let source = &self.file;
        for (node_index, _) in source
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, d)| d.kind == NodeKind::Data)
        {
            let id = self.declaration(node_index)?;
            let mut chain = source
                .enclosing_contexts(node_index)
                .filter(|&a| source.nodes[a].kind == NodeKind::Data)
                .map(|a| self.declaration(a))
                .collect::<Result<Vec<_>, _>>()?;
            chain.reverse();
            chain.push(id);
            types.scopes.insert(id, chain);
            if let Some(attr) = source.child(node_index, NodeKind::Attributes) {
                types
                    .multiplicities
                    .insert(id, data_properties::read(self, attr)?.multiplicity);
            }
        }
        Ok(())
    }
    pub fn data_application(
        &self,
        node_index: usize,
        id: &DeclarationId,
        types: &TypeCatalog<'_>,
        candidate: Option<&DeclarationId>,
    ) -> Result<ValueType, TypeError> {
        let source = &self.file;
        let node = &source.nodes[node_index];
        let mut groups = HashMap::new();
        for &section in &node.children {
            if !matches!(source.nodes[section].kind, NodeKind::TypeArgs | NodeKind::ExpressionArgs) {
                return Err(TypeError::unsupported(source, section));
            }
            let before = source.span(section).get_start_offset();
            let mut prefix = node.words().take_while(|w| w.span.get_start_offset() < before);
            let first = prefix
                .next()
                .ok_or_else(|| TypeError::unsupported(source, section))?;
            let last = prefix.last().unwrap_or(first);
            let span = SourceSpan::new(first.span.get_start_offset(), last.span.get_end_offset());
            let Some(Symbol::Declaration(owner)) = self.names.get(&span).copied() else {
                return Err(TypeError::at(source, section, "unresolved type application owner"));
            };
            let mut substitutions = types.formals(owner);
            let mut values = Vec::with_capacity(source.nodes[section].children.len());
            for (index, &arg) in source.nodes[section].children.iter().enumerate() {
                if index >= substitutions.len() {
                    return Err(TypeError::at(source, section, "excess generic arguments"));
                }
                let bound = types
                    .bound(owner, index)
                    .map(|t| t.substitute(owner, &substitutions));
                let value = self.raw_bound_argument(arg, types, owner, index, candidate, bound.as_ref())?;
                substitutions[index] = value.clone();
                values.push(value);
            }
            let values = types
                .solve_arguments(owner, values)
                .map_err(|m| TypeError::at(source, section, m))?;
            groups.insert(owner, values);
        }
        let mut args = Vec::new();
        for &owner in &types.scopes[id] {
            if let Some(selected) = groups.remove(owner) {
                for (index, arg) in selected.iter().enumerate() {
                    types
                        .validate_bound(owner, index, arg)
                        .map_err(|m| TypeError::at(source, node_index, m))?;
                }
                args.extend(selected);
            } else if owner != id
                && source
                    .enclosing_contexts(node_index)
                    .filter_map(|i| self.definitions.get(&source.span(i)))
                    .any(|&d| d == owner)
            {
                args.extend(types.formals(owner));
            } else if types.arities[owner] != 0 {
                return Err(TypeError::at(
                    source,
                    node_index,
                    format!("{} expects {} type arguments, found 0", owner.path, types.arities[owner]),
                ));
            }
        }
        if !groups.is_empty() {
            return Err(TypeError::at(source, node_index, "type arguments do not belong to the selected carrier"));
        }
        Ok(if args.is_empty() {
            ValueType::Nominal(Box::new(id.clone()))
        } else {
            ValueType::Applied(Box::new(id.clone()), args)
        })
    }
    pub fn index_const_carriers(
        &self,
        types: &mut TypeCatalog<'a>,
    ) -> Result<(), TypeError> {
        for (node_index, node) in self
            .file
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| matches!(node.kind, NodeKind::ConstParam | NodeKind::KindedParam))
        {
            if node
                .parent
                .is_some_and(|p| self.file.nodes[p].kind == NodeKind::Fragment)
            {
                continue;
            }
            let telescope = node
                .parent
                .ok_or_else(|| TypeError::unsupported(&self.file, node_index))?;
            let owner = self.declaration(
                self.file.nodes[telescope]
                    .parent
                    .ok_or_else(|| TypeError::unsupported(&self.file, node_index))?,
            )?;
            let index = self.file.nodes[telescope]
                .children
                .iter()
                .take_while(|&&i| i != node_index)
                .filter(|&&i| self.file.nodes[i].kind != NodeKind::SomeParam)
                .count();
            let annotation = self.single_child(node_index)?;
            if matches!(self.symbol(annotation),Some(Symbol::Declaration(id)) if id.kind==NodeKind::Machine)
                || self.file.nodes[annotation].kind == NodeKind::PathType && self.file.nodes[annotation].path() == "machine"
            {
                continue;
            }
            let carrier = if let Some(Symbol::Declaration(schema)) = self.symbol(annotation)
                && schema.kind == NodeKind::Trait
            {
                let args = self
                    .file
                    .child(annotation, NodeKind::TypeArgs)
                    .into_iter()
                    .flat_map(|a| self.file.nodes[a].children.iter().copied())
                    .enumerate()
                    .map(|(i, arg)| self.argument(arg, types, schema, i))
                    .collect::<Result<Vec<_>, _>>()?;
                if args.len() != types.arities[schema] {
                    return Err(TypeError::at(&self.file, annotation, "incorrect evidence application arity"));
                }
                ValueType::Evidence(Box::new(crate::RequirementSlot {
                    declaration: schema.clone(),
                    trait_arguments: args,
                }))
            } else {
                self.raw_type(annotation, None, types)?
            };
            if !matches!(
                carrier,
                ValueType::Integer { .. }
                    | ValueType::Callable(_)
                    | ValueType::Evidence(_)
                    | ValueType::LevelKind
                    | ValueType::Universe { .. }
                    | ValueType::Function(..)
            ) {
                return Err(TypeError::unsupported(&self.file, node_index));
            }
            if node.kind == NodeKind::KindedParam && matches!(carrier, ValueType::Integer { .. }) {
                types.runtime_indices.insert((owner, index));
            }
            types.const_carriers.insert((owner, index), carrier);
        }
        Ok(())
    }
    pub fn index_static_equations(
        &self,
        types: &mut TypeCatalog<'a>,
    ) -> Result<(), TypeError> {
        let source = &self.file;
        for (node_index, node) in source
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, d)| d.kind == NodeKind::Data)
        {
            let owner = self.declaration(node_index)?;
            for &clause in &node.children {
                if source.nodes[clause].kind != NodeKind::Requires {
                    continue;
                }
                let term = self.single_child(clause)?;
                let predicate = &source.nodes[term];
                if predicate.kind != NodeKind::Binary || !predicate.leaves.iter().any(|l| l.text == "==") {
                    continue;
                }
                let [l, r] = *predicate.children.as_slice() else {
                    continue;
                };
                let binding = |node_index| match self.symbol(node_index) {
                    Some(Symbol::Binding {
                        span,
                        kind: NodeKind::TypeParam,
                    }) => match self.parameters.get(span) {
                        Some(ValueType::Parameter { owner: o, index }) if o.as_ref() == owner => Some(*index),
                        _ => None,
                    },
                    _ => None,
                };
                if let Some((i, pattern)) = binding(l)
                    .map(|i| (i, r))
                    .or_else(|| binding(r).map(|i| (i, l)))
                {
                    let pattern = self.annotation(pattern, types)?;
                    types.equations.entry(owner).or_default().push((i, pattern));
                }
            }
        }
        Ok(())
    }
    pub fn index_aliases(
        &self,
        types: &mut TypeCatalog<'a>,
    ) -> Result<(), TypeError> {
        let source = &self.file;
        for (node_index, node) in source
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.kind == NodeKind::Data && node.leaves.iter().any(|w| w.text == "="))
        {
            let ty = *node
                .children
                .last()
                .ok_or_else(|| TypeError::unsupported(source, node_index))?;
            types
                .aliases
                .insert(self.declaration(node_index)?, self.raw_type(ty, None, types)?);
        }
        Ok(())
    }
}
