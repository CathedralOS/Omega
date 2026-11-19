use crate::expressions::body_typer::BodyTyper;
use crate::{Access, ObligationKind, Site, TypeError, ValueType};
use omega_parser::NodeKind;
use omega_resolver::Symbol;

impl BodyTyper<'_, '_> {
    pub(super) fn infer_argument(
        param: &ValueType,
        actual: &ValueType,
        id: &omega_resolver::DeclarationId,
        bindings: &mut [Option<ValueType>],
    ) -> bool {
        if let ValueType::Reference(Access::Shared, inner) = param
            && !matches!(actual, ValueType::Reference(..) | ValueType::AtLifetime(..))
        {
            return inner.infer(actual, id, bindings);
        }
        param.infer(actual, id, bindings)
    }
    pub(super) fn admit_argument(
        &mut self,
        node_index: usize,
        actual: &ValueType,
        param: &ValueType,
    ) -> Result<(), TypeError> {
        if let ValueType::Reference(Access::Shared, inner) = param
            && !matches!(actual, ValueType::Reference(..) | ValueType::AtLifetime(..))
        {
            self.check_destination(node_index, actual, inner)?;
            self.record_obligation(
                node_index,
                ObligationKind::Borrow {
                    access: Access::Shared,
                    place: Site::at(&self.unit.file, node_index),
                },
            );
            return Ok(());
        }
        self.check_destination(node_index, actual, param)
    }
    pub(super) fn validate_call_bindings(
        &mut self,
        node_index: usize,
        callee: usize,
        id: &omega_resolver::DeclarationId,
        bindings: &[ValueType],
    ) -> Result<(), TypeError> {
        let source_file = &self.unit.file;
        for (index, ty) in bindings.iter().enumerate() {
            if let Some(sort @ ValueType::Universe { .. }) = self.catalog.types.bound(id, index) {
                let expected = sort.substitute(id, bindings);
                if let ValueType::Universe { level, .. } = ty
                    && let ValueType::Level(actual) = level.as_ref()
                    && let ValueType::Universe { strict, level: outer } = &expected
                    && let ValueType::Level(target) = outer.as_ref()
                    && (*strict || actual + 1u8 != *target)
                {
                    return Err(TypeError::at(source_file, callee, "universe argument requires its exact successor universe"));
                }
                self.record_obligation(
                    node_index,
                    ObligationKind::UniverseMembership {
                        subject: ty.clone(),
                        sort: expected,
                    },
                );
            }
            if let Some(ValueType::LogicalRequirement(site)) = self.catalog.types.bound(id, index) {
                let req = &self.catalog.hypotheses[site];
                if let ValueType::ProofTerm(source) = ty {
                    let argument = (callee..source_file.nodes[callee].end)
                        .find(|&i| {
                            source_file.span(i) == source.span
                                && matches!(
                                    source_file.nodes[i].kind,
                                    NodeKind::FieldAccess | NodeKind::Call | NodeKind::PathType | NodeKind::PathExpression
                                )
                        })
                        .ok_or_else(|| TypeError::at(source_file, callee, "missing logical evidence expression"))?;
                    let enclosing_logical_argument = std::mem::replace(&mut self.logical_argument, true);
                    let actual = if source_file.nodes[argument].kind == NodeKind::PathType
                        && let Some(Symbol::Binding { span, .. }) = self.unit.symbol(argument)
                    {
                        self.locals
                            .get(span)
                            .cloned()
                            .ok_or_else(|| TypeError::at(source_file, argument, "untyped evidence term"))
                    } else {
                        self.type_expression(argument)
                    };
                    self.logical_argument = enclosing_logical_argument;
                    let mut evidence_type = actual?;
                    for (span, parameter) in &req.parameters {
                        let ValueType::Function(input, out) = evidence_type else {
                            return Err(TypeError::at(source_file, argument, "logical evidence must abstract every required subject"));
                        };
                        if *input != parameter.substitute(id, bindings) {
                            return Err(TypeError::at(source_file, argument, "incompatible logical evidence parameter"));
                        }
                        let mut formal = req.site.clone();
                        formal.span = *span;
                        evidence_type = out.apply_subject(&ValueType::Subject {
                            declaration: formal,
                            version: None,
                        });
                    }
                    if req.result != ValueType::NoResult || !matches!(evidence_type, ValueType::FamilyApplication { .. } | ValueType::Squash(_)) {
                        return Err(TypeError::at(source_file, argument, "expected evidence for the logical conclusion"));
                    }
                    self.record_obligation(
                        node_index,
                        ObligationKind::SupplyHypothesis {
                            requirement: site.clone(),
                            evidence: ty.clone(),
                            substitutions: bindings.to_vec(),
                        },
                    );
                    continue;
                }
                let (params, result) = match ty {
                    ValueType::ProofCitation(selected) => {
                        let signature = &self.catalog.signatures[selected.as_ref()];
                        if self.catalog.types.arities[selected.as_ref()] != 0 {
                            return Err(TypeError::at(source_file, callee, "logical citation needs a closed interface"));
                        }
                        (&signature.params, &signature.result)
                    }
                    ValueType::LogicalRequirement(other) => {
                        let supplied = &self.catalog.hypotheses[other];
                        (&supplied.parameters, &supplied.result)
                    }
                    _ => return Err(TypeError::at(source_file, callee, "expected explicit logical evidence")),
                };
                if !req
                    .parameters
                    .iter()
                    .map(|(_, ty)| ty.substitute(id, bindings))
                    .eq(params.iter().map(|(_, ty)| ty.clone()))
                    || req.result.substitute(id, bindings) != *result
                {
                    return Err(TypeError::at(source_file, callee, "incompatible logical evidence signature"));
                }
                self.record_obligation(
                    node_index,
                    ObligationKind::SupplyHypothesis {
                        requirement: site.clone(),
                        evidence: ty.clone(),
                        substitutions: bindings.to_vec(),
                    },
                );
            }
            if let Some(formal @ ValueType::Evidence(_)) = self.catalog.types.bound(id, index)
                && let ValueType::Evidence(slot) = formal.substitute(id, bindings)
            {
                let valid = match ty {
                    ValueType::Implementation(implementation) => self
                        .catalog
                        .implementations
                        .get(implementation.as_ref())
                        .is_some_and(|s| self.catalog.admits_conformance(s, &slot)),
                    ValueType::Parameter { .. } => {
                        matches!(self.catalog.types.parameter_carrier(ty),Some(ValueType::Evidence(other)) if self.catalog.trait_lineage(other).contains(&slot))
                    }
                    _ => false,
                };
                if valid
                    && let ValueType::Implementation(selected) = ty
                    && self
                        .catalog
                        .trait_lineage(&slot)
                        .iter()
                        .any(|s| self.catalog.refinements.contains_key(&s.declaration))
                {
                    self.record_obligation(
                        node_index,
                        ObligationKind::RefineEvidence {
                            binder: (*slot).clone(),
                            implementation: (**selected).clone(),
                        },
                    );
                }
                if !valid {
                    return Err(TypeError::at(source_file, callee, "selected evidence has the wrong trait application"));
                }
            }
            if let Some(formal @ ValueType::Callable(_)) = self.catalog.types.bound(id, index)
                && let ValueType::Callable(c) = formal.substitute(id, bindings)
            {
                match ty {
                    ValueType::MachineSelection(m) => self.refine_callable(node_index, &c, m)?,
                    ValueType::Parameter { .. }
                        if let Some(ValueType::Callable(actual)) = self.catalog.types.parameter_carrier(ty)
                            && c.admits_contract(actual) =>
                    {
                        self.check_destination(callee, &ValueType::Callable(actual.clone()), &ValueType::Callable(c))?;
                    }
                    _ => return Err(TypeError::at(source_file, callee, "forwarded callback has incompatible requirements")),
                }
            }
            self.catalog
                .types
                .validate_bound(id, index, ty)
                .map_err(|m| TypeError::at(source_file, callee, m))?;
        }
        Ok(())
    }
}
