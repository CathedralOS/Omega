use super::call_arguments::{CallOperand, PendingComparisons, SelectedMachine};
use super::destination::Destination;
use crate::declarations::declaration_catalog::{PendingParameter, PendingSignature};
use crate::evaluation::check_static_indices::IndexEvaluations;
use crate::expressions::body_typer::BodyTyper;
use crate::type_error::SourceTypeError;
use crate::type_system::dependent_subjects::DependentSubjects;
use crate::type_system::domains::InferResultDomains;
use crate::type_system::substitution::TypeInference;
use crate::{TypeError, TypedSignature, ValueType};
use omega_resolved::{DeclarationId, Symbol};
use omega_syntax::NodeKind;

struct CallBindings {
    arguments: Vec<ValueType>,
    known: Vec<Option<ValueType>>,
}

impl CallBindings {
    fn from_explicit(
        arguments: Vec<ValueType>,
        explicit: usize,
    ) -> Self {
        let mut known: Vec<_> = arguments[..explicit].iter().cloned().map(Some).collect();
        known.resize(arguments.len(), None);
        Self { arguments, known }
    }

    /// Commit only bindings recovered by a successful structural match.
    fn with_result(
        &self,
        result: &ValueType,
        destination: &ValueType,
        owner: &DeclarationId,
    ) -> Option<Self> {
        if self.known.iter().all(Option::is_some) {
            return None;
        }
        let mut known = self.known.clone();
        if !result.infer(destination, owner, &mut known) || known == self.known {
            return None;
        }
        let arguments = known
            .iter()
            .zip(&self.arguments)
            .map(|(bound, formal)| bound.as_ref().unwrap_or(formal).clone())
            .collect();
        Some(Self { arguments, known })
    }

    fn operand_type(
        &self,
        formal: &ValueType,
        owner: &DeclarationId,
    ) -> Option<ValueType> {
        // In a recursive call a supplied argument can name a binder of this same
        // declaration. Check availability before substituting caller types.
        if !self.known.is_empty() && formal.depends_on_unsupplied_parameter(owner, &self.known) {
            return None;
        }
        Some(formal.substitute(owner, &self.arguments))
    }
}

struct CallCandidate<'signature, 'expected> {
    declaration: &'signature DeclarationId,
    signature: &'signature TypedSignature,
    bindings: CallBindings,
    possible_bindings: Vec<CallBindings>,
    result_destination: Option<&'expected ValueType>,
    ordinary_lookup: bool,
    prospective_lookup: bool,
}

struct UnfinishedCall<'a> {
    declaration: &'a DeclarationId,
    signature: &'a PendingSignature,
    bindings: CallBindings,
    supplies_context: bool,
}

/// Determine the common destination using only facts the final signatures must preserve.
/// An unfinished candidate can supply an operand's context even if another operand later excludes it.
fn unfinished_operand_destination<'a>(
    unfinished: &[UnfinishedCall<'_>],
    position: usize,
    capture: impl Fn(&ValueType) -> ValueType,
    types: &'a mut Vec<ValueType>,
    indices: &IndexEvaluations<'_>,
) -> Option<Destination<'a>> {
    let mut undecided = false;
    let mut qualified = false;
    let mut qualified_carrier: Option<ValueType> = None;
    let distinct = |left: &ValueType, right: &ValueType| left != right && !indices.can_wait_for_equality(left, right);
    let (certain, possible) = (unfinished.iter().filter(|c| c.supplies_context), unfinished.iter().filter(|c| !c.supplies_context));
    for candidate in certain.chain(possible) {
        let parameter = &candidate.signature.parameters[position];
        match parameter {
            PendingParameter::Complete(ty) => {
                let ty = capture(ty);
                if ty.components().any(|t| matches!(t, ValueType::Subject { .. } | ValueType::BoundSubject(_))) {
                    undecided = true;
                    continue;
                }
                let Some(ty) = candidate.bindings.operand_type(&ty, candidate.declaration) else {
                    if candidate.supplies_context {
                        return Some(Destination::Unspecified);
                    }
                    undecided = true;
                    continue;
                };
                if candidate.supplies_context {
                    types.push(ty);
                } else if !types.contains(&ty) {
                    undecided = true;
                }
            }
            PendingParameter::QualifiedCarrier(ty) => {
                if candidate.supplies_context {
                    qualified = true;
                    let ty = capture(ty.carrier());
                    if !ty.components().any(|t| matches!(t, ValueType::Subject { .. } | ValueType::BoundSubject(_)))
                        && let Some(carrier) = candidate.bindings.operand_type(&ty, candidate.declaration)
                    {
                        if qualified_carrier.as_ref().is_some_and(|previous| distinct(previous.carrier(), carrier.carrier())) {
                            return Some(Destination::Unspecified);
                        }
                        qualified_carrier = Some(carrier);
                    }
                }
                undecided = true;
            }
            PendingParameter::Unknown => undecided = true,
        }
    }
    // A qualified type cannot become equal to an unqualified one, even when
    // both have the same integer width.
    if qualified && types.iter().any(|ty| ty.domains().is_empty() || qualified_carrier.as_ref().is_some_and(|carrier| distinct(ty.carrier(), carrier.carrier())))
        || !types.is_empty() && matches!(Destination::common(types, indices), Destination::Unspecified)
    {
        return Some(Destination::Unspecified);
    }
    (!undecided).then(|| Destination::common(types, indices))
}

impl CallCandidate<'_, '_> {
    fn available_operand_type(
        &self,
        position: usize,
        capture: impl Fn(&ValueType) -> ValueType,
        subjects: &[ValueType],
    ) -> Option<ValueType> {
        self.bindings
            .operand_type(&capture(&self.signature.parameters[position].1), self.declaration)
            .map(|ty| ty.instantiate_subjects(self.signature, subjects))
    }

    fn prospective_subjects(
        &self,
        body: &BodyTyper<'_, '_>,
        capture: impl Fn(&ValueType) -> ValueType + Copy,
        operands: &[CallOperand],
    ) -> Vec<ValueType> {
        // A possible context can give an anonymous operand its carrier.
        // Test subjects using this signature and the recorded operand versions.
        // Only the ordinary match can publish subjects or select a declaration.
        let mut subjects = Vec::with_capacity(operands.len());
        for (position, operand) in operands.iter().enumerate() {
            let formal = self.available_operand_type(position, capture, &subjects);
            subjects.push(operand.subject(body.unit, formal.as_ref().unwrap_or(&operand.value)));
        }
        subjects
    }

    fn match_operands(
        &self,
        body: &BodyTyper<'_, '_>,
        capture: impl Fn(&ValueType) -> ValueType,
        operands: &[CallOperand],
        subjects: &[ValueType],
    ) -> Option<(Vec<Option<ValueType>>, PendingComparisons)> {
        let mut bindings = self.bindings.known.clone();
        let indices = &body.catalog.types.indices;
        let mut pending = PendingComparisons::default();
        for (position, ((_, parameter), operand)) in self.signature.parameters.iter().zip(operands).enumerate() {
            let formal = capture(parameter)
                .substitute(self.declaration, &self.bindings.arguments)
                .instantiate_subjects(self.signature, subjects);
            if !body.infer_operand_with(NodeKind::Call, position, operand, &formal, self.declaration, &mut bindings, &mut |a, b| {
                pending.compare(indices, a, b)
            }) {
                return None;
            }
        }
        Some((bindings, pending))
    }
}

impl BodyTyper<'_, '_> {
    /// An unfinished overload may keep a call waiting but can never select a machine.
    fn unfinished_call_may_match(
        &self,
        declaration: &DeclarationId,
        signature: &PendingSignature,
        formals: &[ValueType],
        operands: &[CallOperand],
        capture: impl Fn(&ValueType) -> ValueType,
    ) -> bool {
        let indices = &self.catalog.types.indices;
        let mut bindings = vec![None; formals.len()];
        signature.parameters.iter().zip(operands).enumerate().all(|(position, (parameter, operand))| {
            let carrier_operand;
            let (formal, operand) = match parameter {
                PendingParameter::Complete(ty) => (ty, operand),
                PendingParameter::QualifiedCarrier(ty) => {
                    if operand.value.domains().is_empty() {
                        return false;
                    }
                    carrier_operand = self.call_operand(operand.node, operand.value.carrier().clone());
                    (ty.carrier(), &carrier_operand)
                }
                PendingParameter::Unknown => return true,
            };
            let formal = capture(formal).substitute(declaration, formals);
            // Earlier operand subjects become available with the full signature.
            formal.components().any(|ty| matches!(ty, ValueType::Subject { .. } | ValueType::BoundSubject(_)))
                || self.infer_operand_with(NodeKind::Call, position, operand, &formal, declaration, &mut bindings, &mut |left, right| {
                    indices.can_wait_for_equality(left, right)
                })
        })
    }

    pub(super) fn type_call(
        &mut self,
        node_index: usize,
        destination: Destination<'_>,
    ) -> Result<ValueType, TypeError> {
        let expected = destination.exact();
        let pending_destination = destination.pending();
        let source = self.unit.file;
        let [callee, args] = *source.nodes[node_index].children.as_slice() else {
            return Err(TypeError::unsupported(source, node_index));
        };
        if let Some(Symbol::DeclarationQuery { package, path }) = self.unit.symbol(callee) {
            return self.declaration_query(node_index, callee, args, package, path);
        }
        if let Some(Symbol::Declaration(id)) = self.unit.symbol(callee)
            && id.kind == NodeKind::MathLet
        {
            return self.mathematical_application(
                node_index,
                callee,
                Some(id),
                self.catalog.types.mathematical[id].clone(),
                &source.nodes[args].children,
                expected,
            );
        }
        if let Some(Symbol::Binding { span, .. }) = self.unit.symbol(callee)
            && let Some(ty @ ValueType::Function(..)) = self
                .locals
                .get(span)
                .or_else(|| {
                    self.unit
                        .parameters
                        .get(span)
                        .and_then(|p| self.catalog.types.parameter_carrier(p))
                })
                .cloned()
        {
            return self.mathematical_application(node_index, callee, None, ty, &source.nodes[args].children, expected);
        }
        if source.nodes[callee].kind != NodeKind::PathExpression {
            return match self.type_expression(callee)? {
                ty @ ValueType::Function(..) => self.mathematical_application(node_index, callee, None, ty, &source.nodes[args].children, expected),
                ValueType::Callable(c) => self.indirect(node_index, callee, args, &c),
                _ => Err(TypeError::at(source, callee, "indirect call requires a callable value")),
            };
        }
        match self.unit.symbol(callee) {
            Some(Symbol::Binding {
                span,
                kind: NodeKind::ConstParam,
                ..
            }) => {
                let c = self.static_callable(*span, callee)?;
                return self.indirect(node_index, callee, args, &c);
            }
            Some(Symbol::Binding {
                kind: NodeKind::Hypothesis | NodeKind::MachineParam,
                ..
            }) => return Err(TypeError::at(source, callee, "logical evidence cannot be executed")),
            Some(Symbol::Binding { span, .. }) => {
                let Some(ValueType::Callable(c)) = self.locals.get(span).cloned() else {
                    return Err(TypeError::at(source, callee, "indirect call requires a callable value"));
                };
                return self.indirect(node_index, callee, args, &c);
            }
            _ => {}
        }
        let mut evidence = None;
        let mut evidence_lineage = None;
        let mut requirements = Vec::new();
        let declarations = match self.unit.symbol(callee) {
            Some(Symbol::StaticMember { binder, member }) => {
                evidence = self.unit.parameters.get(binder).cloned();
                let carrier = evidence
                    .as_ref()
                    .and_then(|parameter| self.catalog.types.parameter_carrier(parameter));
                let Some(ValueType::Evidence(slot)) = carrier else {
                    if member == "ensures" {
                        return self.project_guarantee(node_index, callee, args, *binder);
                    }
                    return Err(TypeError::at(source, callee, "member selection requires a trait evidence binder"));
                };
                let lineage = self.catalog.trait_lineage(slot);
                requirements.extend(
                    lineage
                        .iter()
                        .flat_map(|owner| &self.catalog.requirements[&owner.declaration])
                        .filter(|id| id.path.rsplit([':', '\\']).next() == Some(member.as_str()))
                        .copied(),
                );
                if requirements.is_empty() {
                    return Err(TypeError::at(source, callee, format!("{} has no requirement {member}", slot.declaration.path)));
                }
                evidence_lineage = Some(lineage);
                &[][..]
            }
            Some(Symbol::Declaration(id)) if matches!(id.kind, NodeKind::Machine | NodeKind::AsmMachine) => std::slice::from_ref(id),
            Some(Symbol::Candidates(ids)) => ids.as_slice(),
            _ => return Err(TypeError::at(source, callee, "unsupported indirect call or non-machine callee")),
        };
        let mut explicit = &[][..];
        for &section in &source.nodes[callee].children {
            let arguments = source.nodes[section].children.as_slice();
            match self.unit.application_name(section).map(|name| &name.target) {
                Some(Symbol::Declaration(id)) if matches!(id.kind, NodeKind::Trait | NodeKind::Conformance) => {}
                Some(Symbol::Binding { .. }) if evidence.is_some() => {
                    return Err(TypeError::at(source, section, "type arguments cannot be applied to the evidence binder"));
                }
                Some(_) => explicit = arguments,
                None => return Err(TypeError::unsupported(source, section)),
            }
        }
        let enclosing_arguments = if let Some(lineage) = evidence_lineage {
            lineage
                .into_iter()
                .map(|slot| (slot.declaration, slot.trait_arguments))
                .collect()
        } else {
            self.unit.receiver_arguments(callee, self.catalog)?
        };
        let capture = |t: &ValueType| t.in_enclosing_scopes(&enclosing_arguments);
        let indices = &self.catalog.types.indices;
        let mut prepared = Vec::new();
        let mut unfinished = Vec::new();
        let mut argument_error = None;
        let candidate_count = declarations.len() + requirements.len();
        for id in declarations.iter().chain(requirements) {
            let partial = self.catalog.pending_signatures.get(id);
            let signature = self.catalog.signatures.get(id);
            if partial.is_none() && indices.declaration_is_pending(id) {
                return Err(indices.wait(source, node_index));
            }
            let arity = signature.map(|s| s.parameters.len()).or_else(|| partial.map(|s| s.parameters.len()));
            if arity != Some(source.nodes[args].children.len()) {
                continue;
            }
            let formals = match self
                .unit
                .applied_formals(callee, id, explicit, &self.catalog.types, capture)
            {
                Ok(formals) => formals,
                Err(e) => {
                    argument_error = Some(e);
                    continue;
                }
            };
            let declared_result = signature.map(|s| &s.result).or_else(|| partial?.result.as_ref());
            let Some(declared_result) = declared_result else {
                unfinished.push(UnfinishedCall {
                    declaration: id,
                    signature: partial.expect("the result belongs to a pending signature"),
                    bindings: CallBindings::from_explicit(formals, explicit.len()),
                    supplies_context: false,
                });
                continue;
            };
            let declared_result = capture(declared_result);
            let result = declared_result.substitute(id, &formals);
            let wanted = expected.map_or(&[][..], ValueType::domains);
            // An operand can determine the entire return type or an index in a
            // declared result domain. Keep that candidate until its operands bind.
            let domain_destination = expected.filter(|ty| result.can_infer_result_domains(id, ty) || indices.can_wait_for_result_domains(&result, ty));
            let ordinary_lookup = domain_destination.is_some() || result.domains() == wanted;
            let prospective_lookup = pending_destination.is_some_and(|pending| {
                pending.permits(|ty| {
                    result.domains() == ty.domains() || result.can_infer_result_domains(id, ty) || indices.can_wait_for_result_domains(&result, ty)
                })
            });
            if ordinary_lookup || prospective_lookup {
                let mut bindings = CallBindings::from_explicit(formals, explicit.len());
                if let Some(inferred) = expected.and_then(|ty| bindings.with_result(&declared_result, ty, id)) {
                    bindings = inferred;
                }
                if let Some(signature) = partial {
                    unfinished.push(UnfinishedCall {
                        declaration: id,
                        signature,
                        bindings,
                        supplies_context: result.domains() == wanted && pending_destination.is_none(),
                    });
                    continue;
                }
                let signature = signature.expect("a completed signature");
                // A pending index can change which result type is expected. These
                // alternatives only carry that dependency to nested calls.
                let possible_bindings = pending_destination
                    .into_iter()
                    .filter(|_| prospective_lookup)
                    .flat_map(|pending| pending.types())
                    .filter_map(|ty| bindings.with_result(&declared_result, ty, id))
                    .collect();
                prepared.push(CallCandidate {
                    declaration: id,
                    signature,
                    bindings,
                    possible_bindings,
                    result_destination: domain_destination,
                    ordinary_lookup,
                    prospective_lookup,
                });
            }
        }
        let conditional_inputs = pending_destination.filter(|pending| {
            pending.has_domains()
                && prepared
                    .iter()
                    .any(|candidate| !candidate.ordinary_lookup || !candidate.prospective_lookup)
        });
        let mut operands = Vec::new();
        let mut subjects = Vec::new();
        let mut available_types = Vec::new();
        for (position, &arg) in source.nodes[args].children.iter().enumerate() {
            available_types.clear();
            let mut all_parameters_available = true;
            let mut pending_inputs = conditional_inputs;
            for candidate in &prepared {
                if let Some(ty) = candidate.available_operand_type(position, capture, &subjects) {
                    available_types.push(ty);
                } else {
                    all_parameters_available = false;
                    let formal = capture(&candidate.signature.parameters[position].1);
                    for bindings in &candidate.possible_bindings {
                        if let Some(ty) = bindings.operand_type(&formal, candidate.declaration) {
                            available_types.push(ty.instantiate_subjects(candidate.signature, &subjects));
                            pending_inputs = pending_destination;
                        }
                    }
                }
            }
            let argument_destination = if !unfinished.is_empty() && all_parameters_available {
                if pending_destination.is_some() || prepared.iter().any(|candidate| candidate.result_destination.is_some()) {
                    Destination::Unresolved
                } else {
                    unfinished_operand_destination(&unfinished, position, capture, &mut available_types, indices)
                        .unwrap_or(Destination::Unresolved)
                }
            } else if let Some(pending) = pending_inputs {
                pending.conditional(&available_types)
            } else if all_parameters_available {
                Destination::common(&available_types, indices)
            } else {
                Destination::Unspecified
            };
            if argument_destination.pending().is_some()
                && !operands.is_empty()
                && prepared.iter().all(|candidate| {
                    candidate
                        .match_operands(self, capture, &operands, &subjects)
                        .is_none()
                        && !(conditional_inputs.is_some() && candidate.prospective_lookup && {
                            let possible_subjects = candidate.prospective_subjects(self, capture, &operands);
                            candidate
                                .match_operands(self, capture, &operands, &possible_subjects)
                                .is_some()
                        })
                })
            {
                let name = source.nodes[callee].path();
                let types = operands
                    .iter()
                    .map(|operand| operand.value.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(TypeError::at(
                    source,
                    callee,
                    format!("no matching overload for {name} with leading operand types ({types})"),
                ));
            }
            let operand = self.argument_operand(arg, argument_destination)?;
            subjects.push(operand.subject(self.unit, argument_destination.exact().unwrap_or(&operand.value)));
            operands.push(operand);
        }
        let mut matches = Vec::new();
        let mut waiting = unfinished.into_iter().any(|candidate| {
            self.unfinished_call_may_match(candidate.declaration, candidate.signature, &candidate.bindings.arguments, &operands, capture)
        });
        for candidate in prepared {
            let matched = candidate.match_operands(self, capture, &operands, &subjects);
            if let Some(destination) = pending_destination {
                if destination.has_domains()
                    && candidate.ordinary_lookup
                    && let Some((_, pending)) = &matched
                {
                    destination.retain_demands(indices);
                    pending.retain_demands(indices);
                    waiting = true;
                    continue;
                }
                if candidate.prospective_lookup {
                    let possible_subjects;
                    let possible_match;
                    let (result_subjects, possible_operands) = if conditional_inputs.is_some() {
                        possible_subjects = candidate.prospective_subjects(self, capture, &operands);
                        possible_match = candidate.match_operands(self, capture, &operands, &possible_subjects);
                        (&possible_subjects, possible_match.as_ref())
                    } else {
                        (&subjects, matched.as_ref())
                    };
                    if let Some((bindings, pending)) = possible_operands
                        && (destination.has_domains() || bindings.iter().any(Option::is_none))
                    {
                        let result = capture(&candidate.signature.result).instantiate_subjects(candidate.signature, result_subjects);
                        if destination.permits(|ty| {
                            let mut hypothetical_bindings = bindings.clone();
                            let inferred = infer_result(&result, ty, candidate.declaration, &candidate.bindings.arguments, &mut hypothetical_bindings);
                            (inferred.domains() == ty.domains() || indices.can_wait_for_result_domains(&inferred, ty))
                                && (destination.has_domains() || hypothetical_bindings.iter().all(Option::is_some))
                        }) {
                            destination.retain_demands(indices);
                            pending.retain_demands(indices);
                            waiting = true;
                            continue;
                        }
                    }
                }
            }
            let CallCandidate {
                declaration: id,
                signature,
                bindings: CallBindings { arguments: formals, .. },
                result_destination: domain_destination,
                ordinary_lookup,
                ..
            } = candidate;
            if let Some((mut bindings, mut pending)) = matched {
                if let Some(destination) = domain_destination {
                    let result = capture(&signature.result).instantiate_subjects(signature, &subjects);
                    let result = infer_result(&result, destination, id, &formals, &mut bindings);
                    if result.domains() != destination.domains() {
                        if !indices.can_wait_for_result_domains(&result, destination) {
                            continue;
                        }
                        pending.record(&result, destination);
                    }
                }
                if ordinary_lookup {
                    if pending.retain_demands(indices) {
                        waiting = true;
                    } else {
                        matches.push((id, signature, bindings));
                    }
                }
            } else if candidate_count == 1 {
                let mut inferred = vec![None; formals.len()];
                if signature
                    .parameters
                    .iter()
                    .zip(&operands)
                    .enumerate()
                    .all(|(i, ((_, p), op))| self.infer_operand(NodeKind::Call, i, op, &capture(p), id, &mut inferred))
                {
                    for (i, ty) in formals[..explicit.len()].iter().enumerate() {
                        if inferred[i].as_ref().is_some_and(|v| v != ty) {
                            argument_error = Some(TypeError::at(source, explicit[i], format!("conflicting type argument {ty}")));
                            break;
                        }
                    }
                }
            }
        }
        if waiting {
            return Err(indices.wait(source, callee));
        }
        let mut matches = matches.into_iter();
        let name = source.nodes[callee].path();
        let Some((id, signature, mut bindings)) = matches.next() else {
            if let Some(e) = argument_error {
                return Err(e);
            }
            let types = operands
                .iter()
                .map(|op| op.value.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(TypeError::at(source, callee, format!("no matching overload for {name}({types})")));
        };
        if matches.next().is_some() {
            return Err(TypeError::at(source, callee, format!("ambiguous call to {name}")));
        }
        self.unit.check_signature_selection(callee, id)?;
        if let Some(expected) = expected {
            capture(&signature.result).infer(expected, id, &mut bindings);
        }
        let bindings = bindings
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| TypeError::at(source, callee, "cannot infer all generic arguments"))?;
        self.apply_machine(
            node_index,
            SelectedMachine {
                machine: id,
                explicit_arguments: explicit,
                arguments: bindings,
                enclosing: enclosing_arguments,
                evidence,
            },
            &operands,
        )
    }
}

fn infer_result(
    result: &ValueType,
    destination: &ValueType,
    declaration: &DeclarationId,
    formals: &[ValueType],
    bindings: &mut [Option<ValueType>],
) -> ValueType {
    result.infer(destination, declaration, bindings);
    let arguments: Vec<_> = formals
        .iter()
        .zip(bindings)
        .map(|(formal, binding)| binding.as_ref().unwrap_or(formal).clone())
        .collect();
    result.substitute(declaration, &arguments)
}
