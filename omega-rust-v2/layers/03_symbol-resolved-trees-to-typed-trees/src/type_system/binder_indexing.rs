use crate::declarations::data_properties;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::type_system::type_catalog::TypeCatalog;
use crate::{TypeError, ValueType};
use omega_parser::NodeKind;
use omega_resolver::{DeclarationId, Symbol};
use std::collections::HashMap;

fn has_generic_arguments(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Machine | NodeKind::AsmMachine | NodeKind::MathLet | NodeKind::Data | NodeKind::Trait | NodeKind::Conformance | NodeKind::Domain
    )
}

impl<'a> ResolvedSourceFile<'a> {
    /// Fields and cases share the enclosing declaration's specialization.
    pub(crate) fn specialization_owner(
        &self,
        node: usize,
    ) -> Option<&'a DeclarationId> {
        self.file
            .ancestors(node)
            .filter_map(|n| self.declaration_at(n))
            .find(|id| has_generic_arguments(id.kind))
    }

    pub fn index_declaration_headers(
        &mut self,
        types: &mut TypeCatalog<'a>,
    ) -> Result<(), TypeError> {
        let source = self.file;
        for (node, id) in self.tree.declaration_nodes() {
            if id.kind == NodeKind::Trait && source.nodes[node].has_boundary_modifier() {
                types.service_traits.insert(id);
            }
            if has_generic_arguments(id.kind) {
                types.generics.entry(id).or_default();
            }
            if matches!(id.kind, NodeKind::Data | NodeKind::Trait) {
                let mut scopes = source
                    .ancestors(node)
                    .find(|&parent| source.nodes[parent].kind == NodeKind::Data)
                    .and_then(|parent| self.declaration_at(parent))
                    .map(|parent| types.scopes[parent].clone())
                    .unwrap_or_default();
                scopes.push(id);
                types.scopes.insert(id, scopes);
                if let Some(attr) = source.child(node, NodeKind::Attributes) {
                    types
                        .data_properties
                        .insert(id, data_properties::read(self, attr)?);
                }
            }
        }
        for node in source
            .nodes
            .iter()
            .filter(|node| node.kind == NodeKind::GenericParams)
        {
            let owner = self.declaration(node.parent.unwrap())?;
            let generic = types.generics.entry(owner).or_default();
            for (name, p, _) in source.generic_parameters(node.parent.unwrap()) {
                if source.nodes[p].kind == NodeKind::SomeParam {
                    if owner.kind != NodeKind::Data {
                        return Err(TypeError::unsupported(source, p));
                    }
                    let witness = &mut generic.witness_count;
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
                let mut multiplicity = None;
                match source.nodes[p].kind {
                    NodeKind::TypeParam => {
                        for &attr in &source.nodes[p].children {
                            let prop = data_properties::read(self, attr)?;
                            if prop.carry.is_some() {
                                return Err(TypeError::unsupported(source, attr));
                            }
                            multiplicity = Some(prop.multiplicity);
                        }
                    }
                    NodeKind::KindedParam | NodeKind::ConstParam | NodeKind::MachineParam | NodeKind::LifetimeParam => {}
                    _ => return Err(TypeError::unsupported(source, p)),
                }
                let index = generic.parameters.len();
                generic.parameters.push(super::type_catalog::GenericParameter {
                    kind: source.nodes[p].kind,
                    multiplicity,
                    carrier: None,
                });
                self.parameters.insert(
                    name.span,
                    ValueType::Parameter {
                        owner: Box::new(owner.clone()),
                        index,
                    },
                );
            }
        }
        Ok(())
    }
    pub fn data_application(
        &self,
        node_index: usize,
        id: &DeclarationId,
        types: &TypeCatalog<'_>,
    ) -> Result<ValueType, TypeError> {
        let source = self.file;
        let mut groups = HashMap::new();
        for &section in self.file.reference_sections(node_index) {
            if !matches!(source.nodes[section].kind, NodeKind::TypeArgs | NodeKind::ExpressionArgs) {
                return Err(TypeError::unsupported(source, section));
            }
            let application = self.application_name(section).map(|name| &name.target);
            if application.is_some_and(Symbol::names_domain)
                && self
                    .name_in_role(node_index, true)
                    .is_some_and(Symbol::names_domain)
            {
                // Domain indices belong to the domain application, including on an ancestor segment.
                continue;
            }
            let Some(Symbol::Declaration(owner)) = application else {
                return Err(TypeError::at(source, section, "unresolved type application owner"));
            };
            groups.insert(owner, section);
        }
        let mut args = Vec::new();
        for (depth, &owner) in types.scopes[id].iter().enumerate() {
            if let Some(section) = groups.remove(owner) {
                let arguments = &source.nodes[section].children;
                let capture = |ty: &ValueType| {
                    types
                        .scoped_arguments(id, &args)
                        .take(depth)
                        .fold(ty.clone(), |bound, (outer, selected)| bound.substitute(outer, selected))
                };
                let mut selected = self.bind_formals(section, owner, arguments, types, capture, |index, arg, bound| {
                    self.raw_bound_argument(arg, types, owner, index, bound)
                })?;
                selected.truncate(arguments.len());
                let selected = types
                    .solve_arguments(owner, selected)
                    .map_err(|m| TypeError::at(source, section, m))?;
                for (index, arg) in selected.iter().enumerate() {
                    types
                        .validate_bound(owner, index, arg)
                        .map_err(|m| TypeError::at(source, node_index, m))?;
                }
                args.extend(selected);
            } else if owner != id
                && source
                    .ancestors(node_index)
                    .filter_map(|i| self.declaration_at(i))
                    .any(|d| d == owner)
            {
                args.extend(types.formals(owner));
            } else if types.arity(owner) != 0 {
                return Err(TypeError::at(
                    source,
                    node_index,
                    format!("{} expects {} type arguments, found 0", owner.path, types.arity(owner)),
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
    pub(super) fn parameter_slot(
        &self,
        span: omega_lexer::SourceSpan,
        types: &TypeCatalog<'a>,
    ) -> Option<(&'a DeclarationId, usize)> {
        let ValueType::Parameter { owner, index } = self.parameters.get(&span)? else {
            return None;
        };
        let (&declaration, _) = types.generics.get_key_value(owner.as_ref())?;
        Some((declaration, *index))
    }

    pub(super) fn index_binder_carrier(
        &mut self,
        node_index: usize,
        annotation: usize,
        types: &mut TypeCatalog<'a>,
    ) -> Result<(), TypeError> {
        let node = &self.file.nodes[node_index];
        let name = node
            .binder_name()
            .expect("a binder annotation belongs to a named parameter");
        let (owner, index) = self
            .parameter_slot(name.span, types)
            .ok_or_else(|| TypeError::unsupported(self.file, node_index))?;
        match self.symbol(annotation) {
            Some(Symbol::Builtin(name)) if name == "machine" => {
                self.inferred_callbacks.insert(node_index, (owner, index));
                return Ok(());
            }
            Some(Symbol::Declaration(id)) if id.kind == NodeKind::Machine => {
                self.nominal_callbacks.push((annotation, (owner, index)));
                return Ok(());
            }
            _ => {}
        }
        let carrier = self.annotation(annotation, types)?;
        if !matches!(
            carrier,
            ValueType::Integer { .. }
                | ValueType::Callable(_)
                | ValueType::Evidence(_)
                | ValueType::LevelKind
                | ValueType::Universe { .. }
                | ValueType::Function(..)
        ) {
            return Err(TypeError::unsupported(self.file, node_index));
        }
        if matches!(carrier, ValueType::Universe { .. }) {
            self.universe_binders.insert(node_index);
            if omega_resolver::is_builtin_type_name(name.text) {
                return Err(TypeError {
                    span: name.span,
                    ..TypeError::at(self.file, node_index, format!("builtin type {} cannot be redeclared", name.text))
                });
            }
        }
        types.set_carrier(owner, index, carrier);
        Ok(())
    }
    pub fn index_static_equations(
        &self,
        types: &mut TypeCatalog<'a>,
    ) -> Result<(), TypeError> {
        let source = self.file;
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
                let Some((l, r)) = self.equality_operands(term)? else {
                    continue;
                };
                let binding = |node_index| match self.symbol(node_index) {
                    Some(Symbol::Binding {
                        span,
                        kind: NodeKind::TypeParam,
                        ..
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
                    types
                        .generics
                        .get_mut(owner)
                        .expect("indexed data")
                        .equations
                        .push((i, pattern));
                }
            }
        }
        Ok(())
    }
}
