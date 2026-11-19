use crate::evaluation::check_static_indices::IndexEvaluations;
use crate::expressions::body_typer::{BodyTyper, ExpressionContext};
use crate::{Access, ObligationKind, Site, TypeError, ValueType};
use omega_parser::NodeKind;
use omega_resolver::{DeclarationId, Symbol};

/// The operand's storage version is captured when it is evaluated. A later
/// argument may call a machine that changes the current version.
pub(super) struct CallOperand {
    pub node: usize,
    pub value: ValueType,
    version: Option<Site>,
    pub(super) read_from: Option<Access>,
}

/// A candidate must satisfy all known obligations before these differences
/// can justify postponing the selection.
#[derive(Default)]
pub(super) struct PendingComparisons(Vec<ValueType>);

impl PendingComparisons {
    pub(super) fn compare(
        &mut self,
        indices: &IndexEvaluations<'_>,
        actual: &ValueType,
        expected: &ValueType,
    ) -> bool {
        if !indices.can_wait_for_equality(actual, expected) {
            return false;
        }
        self.record(actual, expected);
        true
    }

    pub(super) fn record(
        &mut self,
        actual: &ValueType,
        expected: &ValueType,
    ) {
        self.0.extend([actual.clone(), expected.clone()]);
    }

    pub(super) fn retain_demands(
        &self,
        indices: &IndexEvaluations<'_>,
    ) -> bool {
        let mut waiting = false;
        for value in &self.0 {
            waiting |= indices.retain_demands(value);
        }
        waiting
    }
}

pub(super) struct SelectedMachine<'a> {
    pub machine: &'a DeclarationId,
    pub explicit_arguments: &'a [usize],
    pub arguments: Vec<ValueType>,
    pub enclosing: Vec<(DeclarationId, Vec<ValueType>)>,
    pub evidence: Option<ValueType>,
}

impl CallOperand {
    pub(super) fn subject(
        &self,
        unit: &crate::declarations::resolved_source_file::ResolvedSourceFile<'_>,
        formal: &ValueType,
    ) -> ValueType {
        unit.term_subject(self.node, formal, self.version.as_ref())
    }
}

impl BodyTyper<'_, '_> {
    pub(super) fn call_operand(
        &self,
        node: usize,
        value: ValueType,
    ) -> CallOperand {
        let version = match self.unit.symbol(self.unit.file.ungroup(node)) {
            Some(Symbol::Binding { span, .. }) if self.writable.contains(span) => self.version.clone(),
            _ => None,
        };
        CallOperand {
            node,
            value,
            version,
            read_from: None,
        }
    }

    /// A borrowed field supplies a place until the selected parameter determines
    /// whether to borrow its storage or read a value from it.
    pub(super) fn argument_operand(
        &mut self,
        node: usize,
        expected: Option<&ValueType>,
    ) -> Result<CallOperand, TypeError> {
        let root = self.unit.file.ungroup(node);
        if self.unit.file.nodes[root].kind == NodeKind::FieldAccess
            || matches!(self.unit.symbol(root), Some(Symbol::Binding {span,..}) if self.borrowed_places.contains_key(span))
        {
            let (ty, place) = self.place_operand(node)?;
            let mut operand = self.call_operand(node, ty);
            operand.read_from = place;
            Ok(operand)
        } else {
            let value = self.type_expression_with_expected_type(node, expected)?;
            Ok(self.call_operand(node, value))
        }
    }

    /// Named arguments admit a shared loan. Only bracket syntax also admits a
    /// mutable loan of its collection. Other operator operands match as written.
    fn argument_loan<'p>(
        kind: NodeKind,
        position: usize,
        actual: &ValueType,
        formal: &'p ValueType,
    ) -> Option<(&'p Access, &'p ValueType)> {
        let ValueType::Reference(access, inner) = formal else { return None };
        if matches!(actual, ValueType::Reference(..) | ValueType::AtLifetime(..)) {
            return None;
        }
        match (kind, position, access) {
            (NodeKind::Call, _, Access::Shared) | (NodeKind::Index, 0, Access::Shared | Access::Mut) => Some((access, inner)),
            _ => None,
        }
    }

    pub(super) fn infer_operand(
        &self,
        kind: NodeKind,
        position: usize,
        operand: &CallOperand,
        formal: &ValueType,
        id: &DeclarationId,
        bindings: &mut [Option<ValueType>],
    ) -> bool {
        self.infer_operand_with(kind, position, operand, formal, id, bindings, &mut |_, _| false)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn infer_operand_with(
        &self,
        kind: NodeKind,
        position: usize,
        operand: &CallOperand,
        formal: &ValueType,
        id: &DeclarationId,
        bindings: &mut [Option<ValueType>],
        compare_pending: &mut impl FnMut(&ValueType, &ValueType) -> bool,
    ) -> bool {
        if let Some((access, inner)) = Self::argument_loan(kind, position, &operand.value, formal) {
            return (*access != Access::Mut || self.require_place_access(operand.node, access).is_ok())
                && inner.infer_with(&operand.value, id, bindings, compare_pending);
        }
        formal.infer_with(&operand.value, id, bindings, compare_pending)
    }

    pub(super) fn admit_argument(
        &mut self,
        call: usize,
        position: usize,
        operand: &CallOperand,
        formal: &ValueType,
    ) -> Result<(), TypeError> {
        let source = self.unit.file;
        let node = operand.node;
        let actual = &operand.value;
        if let Some((access, inner)) = Self::argument_loan(source.nodes[call].kind, position, actual, formal) {
            self.require_place_access(node, access)?;
            self.check_destination(node, actual, inner)?;
            let site = if source.nodes[call].kind == NodeKind::Index { call } else { node };
            self.record_obligation(
                site,
                ObligationKind::Borrow {
                    access: access.clone(),
                    place: Site::at(source, node),
                },
            );
            return Ok(());
        }
        if let Some(access) = &operand.read_from {
            self.read_borrowed(node, access, actual)?;
        }
        self.check_destination(node, actual, formal)
    }

    pub(super) fn apply_machine(
        &mut self,
        node: usize,
        target: SelectedMachine<'_>,
        operands: &[CallOperand],
    ) -> Result<ValueType, TypeError> {
        let source = self.unit.file;
        let callee = if source.nodes[node].kind == NodeKind::Call {
            source.nodes[node].children[0]
        } else {
            node
        };
        let signature = &self.catalog.signatures[target.machine];
        self.validate_call_bindings(node, callee, target.machine, target.explicit_arguments, &target.arguments, &target.enclosing)?;
        let instantiate =
            |ty: &ValueType, applied: &[crate::AppliedOperand]| ty.at_machine_application(target.machine, &target.arguments, &target.enclosing, applied);
        let mut applied = Vec::with_capacity(operands.len());
        for (position, (op, (span, formal))) in operands.iter().zip(&signature.parameters).enumerate() {
            let expected = instantiate(formal, &applied);
            self.admit_argument(node, position, op, &expected)?;
            let mut parameter = signature.site.clone();
            parameter.span = *span;
            // A const argument keeps its selected generic identity so preparation
            // can substitute the owning declaration's closed argument.
            let subject = if self.context == ExpressionContext::StaticArgument
                && let Some(Symbol::Binding {
                    span,
                    kind: NodeKind::ConstParam,
                    ..
                }) = self.unit.symbol(source.ungroup(op.node))
            {
                self.unit
                    .parameters
                    .get(span)
                    .cloned()
                    .ok_or_else(|| TypeError::at(source, op.node, "missing type of static argument binder"))?
            } else {
                op.subject(self.unit, &expected)
            };
            applied.push(crate::AppliedOperand {
                node: source.node_id(op.node),
                site: Site::at(source, op.node),
                parameter,
                subject,
                callable: matches!(op.value, ValueType::Callable(_)).then(|| self.invocation_target(op.node)),
            });
        }
        let result = instantiate(&signature.result, &applied);
        if signature.requires {
            self.record_obligation(node, ObligationKind::CallPrecondition(target.machine.clone()));
        }
        let use_site = self.record_call_use(node);
        self.output.calls.push(crate::TypedCall {
            node: source.node_id(node),
            use_site,
            site: Site::at(source, node),
            machine: target.machine.clone(),
            result: result.clone(),
            operands: applied,
            type_arguments: target.arguments,
            enclosing_arguments: target.enclosing,
            caller: self.caller.clone(),
            evidence: target.evidence,
            acknowledgements: crate::contracts::effect_clauses::markers(self.unit, node)?,
        });
        Ok(result)
    }

    pub(super) fn validate_call_bindings(
        &mut self,
        node_index: usize,
        callee: usize,
        id: &omega_resolver::DeclarationId,
        explicit_arguments: &[usize],
        bindings: &[ValueType],
        enclosing: &[(DeclarationId, Vec<ValueType>)],
    ) -> Result<(), TypeError> {
        let source_file = self.unit.file;
        let instantiate = |ty: &ValueType| ty.at_machine_application(id, bindings, enclosing, &[]);
        self.catalog
            .types
            .check_universe_arguments(id, bindings, instantiate, |subject, sort| {
                self.record_obligation(node_index, ObligationKind::UniverseMembership { subject, sort });
            })
            .map_err(|message| TypeError::at(source_file, callee, message))?;
        for (index, ty) in bindings.iter().enumerate() {
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
                    let enclosing_context = std::mem::replace(&mut self.context, super::body_typer::ExpressionContext::Proof);
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
                    self.context = enclosing_context;
                    let mut evidence_type = actual?;
                    for (span, parameter) in &req.parameters {
                        let ValueType::Function(input, out) = evidence_type else {
                            return Err(TypeError::at(source_file, argument, "logical evidence must abstract every required subject"));
                        };
                        if *input != instantiate(parameter) {
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
                } else {
                    let (params, result) = match ty {
                        ValueType::ProofCitation(selected) => {
                            let signature = &self.catalog.signatures[selected.as_ref()];
                            if self.catalog.types.arity(selected.as_ref()) != 0 {
                                return Err(TypeError::at(source_file, callee, "logical citation needs a closed interface"));
                            }
                            (&signature.parameters, &signature.result)
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
                        .map(|(_, ty)| instantiate(ty))
                        .eq(params.iter().map(|(_, ty)| ty.clone()))
                        || instantiate(&req.result) != *result
                    {
                        return Err(TypeError::at(source_file, callee, "incompatible logical evidence signature"));
                    }
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
                && let ValueType::Evidence(slot) = instantiate(formal)
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
                && let ValueType::Callable(c) = instantiate(formal)
            {
                match ty {
                    ValueType::MachineSelection(machine, arguments) => {
                        let selection = explicit_arguments
                            .get(index)
                            .map(|&argument| source_file.ungroup(argument));
                        let selected_arguments = selection.map_or(&[][..], |node| self.unit.arguments_for(node, machine));
                        self.validate_call_bindings(
                            selection.unwrap_or(node_index),
                            selection.unwrap_or(callee),
                            machine,
                            selected_arguments,
                            arguments,
                            &[],
                        )?;
                        self.refine_callable(node_index, &c, machine, arguments)?;
                    }
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
