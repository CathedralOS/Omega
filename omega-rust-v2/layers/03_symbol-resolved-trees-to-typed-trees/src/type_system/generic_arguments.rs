use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::evaluation::numeric_literals;
use crate::type_system::type_catalog::TypeCatalog;
use crate::{TypeError, ValueType};
use omega_parser::NodeKind;
use omega_resolver::{DeclarationId, Symbol};

impl<'a> ResolvedSourceFile<'a> {
    /// Later binders can mention earlier arguments. Keep the identity of each
    /// unfilled slot so callers can infer the rest of a partial application.
    pub(crate) fn bind_formals(
        &self,
        node: usize,
        owner: &DeclarationId,
        actual: &[usize],
        types: &TypeCatalog<'_>,
        capture: impl Fn(&ValueType) -> ValueType,
        mut read: impl FnMut(usize, usize, Option<&ValueType>) -> Result<ValueType, TypeError>,
    ) -> Result<Vec<ValueType>, TypeError> {
        let mut formals = types.formals(owner);
        if actual.len() > formals.len() {
            return Err(TypeError::at(self.file, node, "excess generic arguments"));
        }
        for (index, &arg) in actual.iter().enumerate() {
            let bound = types
                .bound(owner, index)
                .map(|ty| capture(ty).substitute(owner, &formals));
            formals[index] = read(index, arg, bound.as_ref())?;
        }
        Ok(formals)
    }

    pub(crate) fn applied_formals(
        &self,
        node: usize,
        owner: &DeclarationId,
        actual: &[usize],
        types: &TypeCatalog<'_>,
        capture: impl Fn(&ValueType) -> ValueType,
    ) -> Result<Vec<ValueType>, TypeError> {
        self.bind_formals(node, owner, actual, types, capture, |index, arg, bound| {
            self.argument(arg, types, owner, index, bound)
        })
    }

    pub(crate) fn argument(
        &self,
        node_index: usize,
        types: &TypeCatalog<'_>,
        owner: &DeclarationId,
        index: usize,
        bound: Option<&ValueType>,
    ) -> Result<ValueType, TypeError> {
        let arg = self.raw_bound_argument(node_index, types, owner, index, bound)?;
        types
            .normalize_at(arg, self, node_index)
            .map_err(|m| TypeError::at(self.file, node_index, m))
    }
    pub(crate) fn raw_argument(
        &self,
        node_index: usize,
        types: &TypeCatalog<'_>,
        owner: &DeclarationId,
        index: usize,
    ) -> Result<ValueType, TypeError> {
        self.raw_bound_argument(node_index, types, owner, index, types.bound(owner, index))
    }
    pub(crate) fn raw_bound_argument(
        &self,
        node_index: usize,
        types: &TypeCatalog<'_>,
        owner: &DeclarationId,
        index: usize,
        bound: Option<&ValueType>,
    ) -> Result<ValueType, TypeError> {
        if types.lifetime_slot(owner, index) {
            if self.file.nodes[node_index].kind != NodeKind::Lifetime {
                return Err(TypeError::at(self.file, node_index, "expected a lifetime argument"));
            }
            return self.raw_type(node_index, types);
        }
        if self.file.nodes[node_index].kind == NodeKind::Lifetime {
            return Err(TypeError::at(self.file, node_index, "unexpected lifetime argument"));
        }
        let Some(carrier) = bound else {
            return self.raw_type(node_index, types);
        };
        if carrier == &ValueType::LevelKind {
            return self.universe_level(node_index, types);
        }
        if matches!(carrier, ValueType::Universe { .. }) {
            return self.raw_type(node_index, types);
        }
        if matches!(carrier, ValueType::Integer { .. }) && !types.admits_runtime_index(owner, index) {
            self.require_static_inputs(types, node_index)?;
        }
        let value_node = self.file.ungroup(node_index);
        let symbol = self.name_in_role(value_node, false);
        if matches!(self.file.nodes[node_index].kind, NodeKind::PathExpression | NodeKind::PathType) && symbol.is_none_or(|name| self.names_type(name)) {
            return Err(TypeError::at(
                self.file,
                node_index,
                format!("unresolved value name {}", self.file.nodes[node_index].path()),
            ));
        }
        if matches!(carrier, ValueType::LogicalRequirement(_)) {
            if matches!(self.file.nodes[node_index].kind, NodeKind::FieldAccess | NodeKind::Call)
                || matches!(
                    symbol,
                    Some(Symbol::Binding {
                        kind: NodeKind::Param | NodeKind::BindingPattern,
                        ..
                    })
                )
            {
                return Ok(ValueType::ProofTerm(crate::Site::at(self.file, node_index)));
            }
            return match symbol {
                Some(Symbol::Declaration(id)) if id.kind == NodeKind::Machine => Ok(ValueType::ProofCitation(Box::new(id.clone()))),
                Some(Symbol::Binding {
                    span,
                    kind: NodeKind::MachineParam | NodeKind::Hypothesis,
                    ..
                }) => self
                    .hypotheses
                    .get(span)
                    .cloned()
                    .map(ValueType::LogicalRequirement)
                    .ok_or_else(|| TypeError::at(self.file, node_index, "unbound logical hypothesis")),
                _ => Err(TypeError::at(self.file, node_index, "expected explicitly supplied logical evidence")),
            };
        }
        if let Some(Symbol::Binding {
            binder_node,
            kind: NodeKind::Param,
            span,
        }) = symbol
            && self.file.nodes[node_index].kind != NodeKind::Call
        {
            if !types.admits_runtime_index(owner, index) && !matches!(carrier, ValueType::Function(..)) {
                return Err(TypeError::at(self.file, node_index, "const index needs a static argument"));
            }
            let annotation = self
                .parameter_annotation(*binder_node)
                .ok_or_else(|| TypeError::unsupported(self.file, node_index))?;
            let expected = types
                .normalize_at(carrier.clone(), self, node_index)
                .map_err(|message| TypeError::at(self.file, node_index, message))?;
            if self.annotation(annotation, types)? != expected {
                return Err(TypeError::at(self.file, node_index, "index subject has the wrong carrier"));
            }
            return Ok(ValueType::Subject {
                declaration: self.binder_site(*span),
                version: None,
            });
        }
        if matches!(carrier, ValueType::Evidence(_))
            && let Some(Symbol::Declaration(id)) = symbol
            && id.kind == NodeKind::Conformance
        {
            return Ok(ValueType::Implementation(Box::new(id.clone())));
        }
        if matches!(carrier, ValueType::Callable(_))
            && let Some(Symbol::Declaration(id)) = symbol
            && id.kind == NodeKind::Machine
        {
            self.check_signature_selection(value_node, id)?;
            let actual = self.arguments_for(value_node, id);
            if actual.len() != types.arity(id) {
                return Err(TypeError::at(self.file, value_node, "machine selection requires all generic arguments"));
            }
            let arguments = self.applied_formals(value_node, id, actual, types, ValueType::clone)?;
            return Ok(ValueType::MachineSelection(Box::new(id.clone()), arguments));
        }
        if let Some(Symbol::Binding {
            span,
            kind: NodeKind::ConstParam | NodeKind::KindedParam,
            ..
        }) = symbol
        {
            let value = self
                .parameters
                .get(span)
                .ok_or_else(|| TypeError::unsupported(self.file, node_index))?;
            if types.parameter_carrier(value) != Some(carrier)
                && !matches!(
                    (types.parameter_carrier(value), carrier),
                    (Some(ValueType::Callable(_)), ValueType::Callable(_)) | (Some(ValueType::Evidence(_)), ValueType::Evidence(_))
                )
            {
                return Err(TypeError::at(self.file, node_index, "const parameter carrier mismatch"));
            }
            return Ok(value.clone());
        }
        if carrier.supports_closed_snapshot() {
            self.require_static_inputs(types, node_index)?;
            let carrier = types
                .normalize_at(carrier.clone(), self, node_index)
                .map_err(|message| TypeError::at(self.file, node_index, message))?;
            return self.closed_static_value(types, node_index, &carrier);
        }
        if let Some(term) = self.open_index(types, node_index, carrier)? {
            return Ok(term);
        }
        let (value, suffix) = self.closed_number(types, node_index, carrier)?;
        if matches!(value, ValueType::StaticFormula { .. }) {
            return Ok(value);
        }
        if suffix.as_ref().is_some_and(|t| t != carrier)
            || !value.fits(carrier)
            || matches!(
                value,
                ValueType::Exact {
                    fractional_origin: Some(_),
                    ..
                }
            )
        {
            return Err(TypeError::at(
                self.file,
                node_index,
                format!("const argument {value} does not land in {carrier}"),
            ));
        }
        let ValueType::Exact { value, .. } = value else {
            return Err(TypeError::unsupported(self.file, node_index));
        };
        Ok(ValueType::StaticInteger {
            carrier: Box::new(carrier.clone()),
            value: value.to_integer(),
        })
    }
    fn require_static_inputs(
        &self,
        types: &TypeCatalog<'_>,
        node: usize,
    ) -> Result<(), TypeError> {
        for part in node..self.file.nodes[node].end {
            let name = if part == node { self.name_in_role(part, false) } else { self.symbol(part) };
            if let Some(Symbol::Binding { span, .. }) = name
                && let Some(ValueType::Parameter { owner, index }) = self.parameters.get(span)
                && types.admits_runtime_index(owner, *index)
            {
                return Err(TypeError::at(self.file, part, "static index cannot depend on a runtime parameter"));
            }
        }
        Ok(())
    }
    /// Array extents and integer widths may depend on a static binder. A
    /// runtime integer binder cannot become a layout index by occurring in arithmetic.
    pub(crate) fn open_size(
        &self,
        types: &TypeCatalog<'_>,
        node: usize,
    ) -> Result<Option<ValueType>, TypeError> {
        let required = ValueType::integer("u64").unwrap();
        self.require_static_inputs(types, node)?;
        if let Some(Symbol::Binding {
            kind: NodeKind::ConstParam,
            span,
            ..
        }) = self.name_in_role(node, false)
        {
            let value = self
                .parameters
                .get(span)
                .ok_or_else(|| TypeError::unsupported(self.file, node))?;
            if types.parameter_carrier(value) != Some(&required) {
                return Err(TypeError::at(self.file, node, "size requires a static u64"));
            }
            return Ok(Some(value.clone()));
        }
        self.open_index(types, node, &required)
    }
    pub(crate) fn closed_size(
        &self,
        types: &TypeCatalog<'_>,
        node_index: usize,
    ) -> Result<ValueType, TypeError> {
        let (value, suffix) = self.closed_number(types, node_index, &ValueType::integer("u64").unwrap())?;
        if matches!(value, ValueType::StaticFormula { .. }) {
            return Ok(value);
        }
        numeric_literals::array_length(self.file, node_index, value, suffix).map(ValueType::static_size)
    }
}

impl ResolvedSourceFile<'_> {
    /// A trait denotes the same applied schema in a type annotation, an evidence
    /// bound, a parent declaration, a service row or a requirement selection.
    pub(crate) fn trait_application(
        &self,
        node: usize,
        schema: &DeclarationId,
        types: &TypeCatalog<'_>,
    ) -> Result<crate::RequirementSlot, TypeError> {
        if schema.kind != NodeKind::Trait {
            return Err(TypeError::at(self.file, node, format!("{} is not a trait", schema.path)));
        }
        Ok(crate::RequirementSlot {
            declaration: schema.clone(),
            trait_arguments: self.application_arguments(node, schema, types)?,
        })
    }

    pub(crate) fn receiver_arguments(
        &self,
        path: usize,
        catalog: &crate::declarations::declaration_catalog::DeclarationCatalog<'_>,
    ) -> Result<Vec<(DeclarationId, Vec<ValueType>)>, TypeError> {
        let Some(Symbol::Declaration(owner)) = self.qualifier(path) else {
            return Ok(Vec::new());
        };
        if !matches!(owner.kind, NodeKind::Trait | NodeKind::Conformance) {
            return Ok(Vec::new());
        }
        let args = self.checked_arguments(path, owner, self.arguments_for(path, owner), &catalog.types)?;
        if owner.kind == NodeKind::Conformance {
            return Ok(vec![(owner.clone(), args)]);
        }
        Ok(catalog
            .trait_lineage(&crate::RequirementSlot {
                declaration: owner.clone(),
                trait_arguments: args,
            })
            .into_iter()
            .map(|slot| (slot.declaration, slot.trait_arguments))
            .collect())
    }

    pub(crate) fn application_arguments(
        &self,
        node: usize,
        schema: &DeclarationId,
        types: &TypeCatalog<'_>,
    ) -> Result<Vec<ValueType>, TypeError> {
        let actual: &[usize] = match self.file.nodes[node].children.as_slice() {
            [] => &[],
            [section]
                if self
                    .application_name(*section)
                    .is_some_and(|name| matches!(&name.target,omega_resolver::Symbol::Declaration(owner) if owner == schema)) =>
            {
                &self.file.nodes[*section].children
            }
            _ => return Err(TypeError::at(self.file, node, format!("type arguments do not belong to {}", schema.path))),
        };
        self.checked_arguments(node, schema, actual, types)
    }

    /// A qualified call may apply both the enclosing declaration and the member.
    /// Check each argument against the preceding substitutions of its own binder.
    pub(crate) fn checked_arguments(
        &self,
        node: usize,
        schema: &DeclarationId,
        actual: &[usize],
        types: &TypeCatalog<'_>,
    ) -> Result<Vec<ValueType>, TypeError> {
        let at = |message| TypeError::at(self.file, node, message);
        let kind = match schema.kind {
            NodeKind::Trait => "trait",
            NodeKind::Conformance => "conformance",
            NodeKind::Machine | NodeKind::AsmMachine => "machine",
            _ => return Err(at(format!("{} has no selectable generic arguments", schema.path))),
        };
        let arity = types
            .generics
            .get(schema)
            .map(|g| g.parameters.len())
            .ok_or_else(|| at(format!("missing {kind} declaration for {}", schema.path)))?;
        if actual.len() != arity {
            return Err(at(format!("{} expects {arity} {kind} arguments, found {}", schema.path, actual.len())));
        }
        let arguments = self.applied_formals(node, schema, actual, types, Clone::clone)?;
        for (index, (&arg, value)) in actual.iter().zip(&arguments).enumerate() {
            types
                .validate_bound(schema, index, value)
                .map_err(|m| TypeError::at(self.file, arg, m))?;
        }
        let mut requirements = Vec::new();
        types
            .require_type_application(schema, &arguments, &mut requirements)
            .map_err(at)?;
        types.formations.retain(self, node, requirements);
        Ok(arguments)
    }
}
