use super::AppliedTraitScope;
use crate::source_state::{TypedTrees, machine::Machine, signature::StateSignature};
use symbols::SymbolHandle;

#[derive(Clone)]
pub struct GenericBoundRequirement<'program> {
    pub signature: &'program StateSignature,
    pub application: AppliedTraitScope<'program>,
}

impl TypedTrees {
    /// A const record retains its declared schema and original lexical binder.
    /// Naming a requirement supplies no record-construction or refinement proof.
    pub fn static_record_parameter_schema(
        &self,
        owner: SymbolHandle,
        binder: SymbolHandle,
    ) -> Result<Option<AppliedTraitScope<'_>>, String> {
        if !binder.is_valid() || !self.symbols.symbols().nodes().is_valid(binder) {
            return Ok(None);
        }
        if self.symbols.get(binder).kind != symbols::SymbolKind::TypeParameter {
            return Ok(None);
        }
        if !owner.is_valid() || !self.symbols.symbols().nodes().is_valid(owner) {
            return Err("static record caller has no current lexical owner".into());
        }
        let declared_owner = self.symbols.get(binder).parent;
        if !declared_owner.is_valid() || !self.symbols.symbols().nodes().is_valid(declared_owner) {
            return Err("static record binder has no current declaration owner".into());
        }
        let declared = self.declaration_type_parameters(declared_owner);
        let mut candidates = declared
            .iter()
            .filter(|parameter| parameter.symbol == binder);
        let Some(parameter) = candidates.next() else {
            return Ok(None);
        };
        if candidates.next().is_some() {
            return Err("static record binder has repeated declaring rows".into());
        }
        let crate::source_state::data::TypeParameterKind::Const { type_reference } = parameter.kind
        else {
            return Ok(None);
        };
        let Some(schema) = self.static_record_schema(type_reference, declared_owner) else {
            return Ok(None);
        };
        let visible = self.declaration_type_parameters(owner);
        let mut occurrences = visible.iter().filter(|entry| entry.symbol == binder);
        if occurrences.next() != Some(parameter) || occurrences.next().is_some() {
            return Err("static record binder is outside its exact retained telescope".into());
        }
        let mut lexical = owner;
        let mut visited = Vec::new();
        while lexical != declared_owner {
            if !lexical.is_valid()
                || !self.symbols.symbols().nodes().is_valid(lexical)
                || visited.contains(&lexical)
            {
                return Err("static record binder has no actual lexical inheritance path".into());
            }
            visited.push(lexical);
            lexical = self.symbols.get(lexical).parent;
        }
        Ok(Some(schema))
    }

    /// Instantiate a const-record binder's declared schema at an enclosing
    /// specialization frame's retained static bindings.
    pub(crate) fn static_record_schema_in_frame(
        &self,
        frame: &crate::callables::machine_instances::EnclosingInvocationFrame<'_>,
        binder: SymbolHandle,
    ) -> Result<Option<AppliedTraitScope<'_>>, String> {
        let Some(root) = self.static_record_parameter_schema(frame.declaration, binder)? else {
            return Ok(None);
        };
        let frame_bindings = frame
            .bindings
            .iter()
            .filter_map(|binding| match binding.value {
                crate::source_state::static_applications::StaticMachineBindingValue::Type(
                    reference,
                )
                | crate::source_state::static_applications::StaticMachineBindingValue::Const(
                    reference,
                ) => reference
                    .is_valid()
                    .then_some((binding.parameter, reference)),
                _ => None,
            })
            .collect::<Vec<_>>();
        root.under_invocation(
            self,
            frame.declaration,
            frame.argument_owner,
            &frame_bindings,
            &[],
        )
        .map(Some)
        .ok_or_else(|| "static record receiver lost its instantiated schema".into())
    }

    pub fn static_record_bound_requirement<'program>(
        &'program self,
        owner: SymbolHandle,
        binder: SymbolHandle,
        target: SymbolHandle,
    ) -> Result<Option<GenericBoundRequirement<'program>>, String> {
        let Some(root) = self.static_record_parameter_schema(owner, binder)? else {
            return Ok(None);
        };
        if !target.is_valid() || !self.symbols.symbols().nodes().is_valid(target) {
            return Err("static record call lost its selected requirement".into());
        }
        let prepared = self.closed_requirement_application(target);
        let mut selected: Option<GenericBoundRequirement<'program>> = None;
        root.try_visit(self, &mut |scope: &AppliedTraitScope<'_>| -> Result<(), &'static str> {
            for signature in self.trait_machine_signatures(scope.declaration) {
                let matches = prepared.as_ref().map_or(signature.symbol == target, |closed| {
                    closed.template == signature.symbol
                        && self.requirement_application_matches_scope(target, scope)
                });
                if !matches { continue; }
                let signature = if let Some(closed) = &prepared {
                    closed.signature
                } else if let Some(applied) = self.applied_callable_signature(scope, signature.symbol) {
                    applied
                } else if scope.keeps_declared_signatures(self) {
                    signature
                } else {
                    return Err("static record requirement has no prepared applied signature");
                };
                if let Some(known) = &selected {
                    if known.signature.symbol != signature.symbol
                        || !known.application.same_application(self, scope) {
                        return Err("static record call has competing complete requirement applications");
                    }
                } else {
                    selected = Some(GenericBoundRequirement { signature, application: scope.clone() });
                }
            }
            Ok(())
        }).map_err(str::to_owned)?;
        selected.map(Some).ok_or_else(|| {
            "static record call selects a requirement outside its declared schema".into()
        })
    }

    /// Authenticate copied receiver metadata before a visitor enters its source scope.
    pub fn static_record_source_receiver(
        &self,
        application: crate::source_state::static_applications::StaticMachineApplicationHandle,
    ) -> Result<Option<(SymbolHandle, SymbolHandle)>, String> {
        if !self
            .expression_table
            .static_machine_application_is_valid(application)
        {
            return Err("static record receiver has a stale source application".into());
        }
        let source = self.static_machine_application(application);
        let Some(binder) = source.source_record_receiver else {
            return Ok(None);
        };
        if !binder.is_valid()
            || !self.symbols.symbols().nodes().is_valid(binder)
            || !source.lexical_owner.is_valid()
            || !self
                .symbols
                .symbols()
                .nodes()
                .is_valid(source.lexical_owner)
        {
            return Err("static record receiver lost its current binder or caller".into());
        }
        let owner = self
            .machine_by_own_or_state_symbol(source.lexical_owner)
            .map(|machine| machine.symbol)
            .ok_or("static record receiver caller has no actual machine")?;
        let frames = self.enclosing_invocation_frames(owner)?;
        for frame in frames {
            let mut matches = frame
                .bindings
                .iter()
                .filter(|binding| binding.parameter == binder);
            let Some(binding) = matches.next() else {
                continue;
            };
            if matches.next().is_some() {
                return Err("static record receiver has repeated retained bindings".into());
            }
            let crate::source_state::static_applications::StaticMachineBindingValue::Const(
                reference,
            ) = binding.value
            else {
                return Err("static record receiver has another retained binder kind".into());
            };
            let declared = self
                .static_record_bound_requirement(
                    frame.declaration,
                    binder,
                    source.registration_operation,
                )?
                .ok_or("static record receiver has no exact declared requirement")?;
            let root = self
                .static_record_schema_in_frame(&frame, binder)?
                .ok_or("static record receiver has no exact declared schema")?;
            if !self.type_reference_table.contains_type_reference(reference)
                || !self
                    .symbols
                    .symbols()
                    .nodes()
                    .is_valid(frame.argument_owner)
            {
                return Err("static record receiver has stale actual operand custody".into());
            }
            let operand = self.static_operand_from_type_reference(reference)?;
            let actual = self
                .conformance_by_symbol(operand.symbol)
                .and_then(|value| AppliedTraitScope::from_conformance(self, value))
                .ok_or("static record receiver has no exact supplying record schema")?;
            if !root.same_application(self, &actual)
                || !declared.application.declaration.symbol.is_valid()
            {
                return Err("static record receiver has a different supplying schema".into());
            }
            return Ok(Some((binder, self.symbols.get(binder).parent)));
        }
        Err("static record receiver has no exact retained supplying application".into())
    }

    pub fn evidence_requirement_declaration(
        &self,
        target: SymbolHandle,
    ) -> Result<Option<&StateSignature>, String> {
        let selected = self.symbols.get(target);
        if selected.kind != symbols::SymbolKind::State
            || !selected.parent.is_valid()
            || self.symbols.get(selected.parent).kind != symbols::SymbolKind::ConformanceParameter
        {
            return Ok(None);
        }
        let binder = selected.parent;
        let owner = self.symbols.get(binder).parent;
        let bounds = self
            .declaration_conformance_bounds(owner)
            .ok_or("evidence child lost its declaring bound owner")?;
        let mut bounds = bounds.iter().filter(|bound| bound.binder == binder);
        let bound = bounds
            .next()
            .ok_or("evidence child lost its exact authored bound")?;
        if bounds.next().is_some() {
            return Err("evidence child has multiple authored bounds".into());
        }
        let mut requirements = Vec::new();
        collect_requirement_namespace(
            self,
            bound.trait_symbol,
            &mut Vec::new(),
            &mut requirements,
        )?;
        let children = self
            .symbols
            .child_handles(binder)
            .ok_or("evidence binder lost its requirement children")?
            .collect::<Vec<_>>();
        if children.len() != requirements.len()
            || children
                .iter()
                .any(|child| self.symbols.get(*child).kind != symbols::SymbolKind::State)
        {
            return Err("evidence namespace disagrees with its declared requirements".into());
        }
        let ordinal = children
            .iter()
            .position(|child| *child == target)
            .ok_or("evidence requirement is not an exact binder child")?;
        Ok(Some(requirements[ordinal]))
    }

    pub fn machine_callable_parameters(
        &self,
        machine: &Machine,
    ) -> Vec<crate::source_state::data::TypeParameter> {
        let mut parameters = self.machine_type_parameters(machine).to_vec();
        for bound in &machine.conformance_bounds {
            if !parameters
                .iter()
                .any(|parameter| parameter.symbol == bound.binder)
            {
                parameters.push(crate::source_state::data::TypeParameter {
                    symbol: bound.binder,
                    name: bound.binder_name.clone(),
                    kind: crate::source_state::data::TypeParameterKind::Implementation,
                    ..Default::default()
                });
            }
        }
        parameters
    }

    pub fn callable_conformance_bounds(
        &self,
        owner: SymbolHandle,
    ) -> Option<&[crate::source_state::machine::GenericConformanceBound]> {
        if let Some(application) = self.closed_requirement_application(owner) {
            return Some(&application.signature.conformance_bounds);
        }
        if let Some(machine) = self.machine_by_symbol(owner) {
            return Some(&machine.conformance_bounds);
        }
        if let Some((_, signature)) = self.trait_machine_signature(owner) {
            return Some(&signature.conformance_bounds);
        }
        self.retained_static_machine_contract(owner)
            .map(|contract| contract.signature().conformance_bounds.as_slice())
    }

    fn declaration_conformance_bounds(
        &self,
        owner: SymbolHandle,
    ) -> Option<&[crate::source_state::machine::GenericConformanceBound]> {
        self.callable_conformance_bounds(owner)
            .or_else(|| {
                self.trait_definition_by_symbol(owner)
                    .map(|declaration| declaration.conformance_bounds.as_slice())
            })
            .or_else(|| {
                self.unique_data_definition_by_symbol(owner)
                    .map(|declaration| declaration.conformance_bounds.as_slice())
            })
            .or_else(|| {
                self.type_aliases()
                    .iter()
                    .find(|declaration| declaration.symbol == owner)
                    .map(|declaration| declaration.conformance_bounds.as_slice())
            })
    }

    pub fn evidence_bound_requirement<'program>(
        &'program self,
        owner: SymbolHandle,
        receiver: SymbolHandle,
        target: SymbolHandle,
    ) -> Result<Option<GenericBoundRequirement<'program>>, String> {
        let Some(bounds) = self.declaration_conformance_bounds(owner) else {
            return Ok(None);
        };
        if !receiver.is_valid()
            || self.symbols.get(receiver).kind != symbols::SymbolKind::ConformanceParameter
            || !bounds.iter().any(|bound| bound.binder == receiver)
        {
            return Ok(None);
        }
        let failure = || {
            format!(
                "declaration `{}` conformance-evidence call has no unique exact requirement",
                self.symbols.name(owner)
            )
        };
        let mut bounds = bounds.iter().filter(|bound| bound.binder == receiver);
        let bound = bounds.next().ok_or_else(failure)?;
        if bounds.next().is_some() {
            return Err(format!(
                "{}: its receiver has multiple authored bounds",
                failure()
            ));
        }
        if !target.is_valid() || self.symbols.get(target).kind != symbols::SymbolKind::State {
            return Err(format!(
                "{}: its selected target is not a requirement state",
                failure()
            ));
        }
        let prepared = self.closed_requirement_application(target);
        if prepared.is_none()
            && !self
                .symbols
                .child_handles(receiver)
                .is_some_and(|mut children| children.any(|child| child == target))
        {
            return Err(format!(
                "{}: its selected target is not owned by the receiver",
                failure()
            ));
        }
        let authored_requirement = if prepared.is_none() {
            Some(
                self.evidence_requirement_declaration(target)?
                    .ok_or_else(failure)?
                    .symbol,
            )
        } else {
            None
        };
        let carrier = self
            .trait_definition_by_symbol(bound.trait_symbol)
            .ok_or_else(failure)?;
        let root = crate::source_state::trait_definition::AppliedTraitScope::new(
            self,
            carrier,
            crate::source_state::trait_definition::TraitApplicationArguments {
                lexical_owner: owner,
                enclosing_data_applications: &[],
                arguments: &bound.arguments,
                lifetimes: &bound.lifetime_arguments,
            },
        )
        .ok_or_else(|| {
            format!(
                "{}: its complete trait application cannot be represented",
                failure()
            )
        })?;
        let mut selected: Option<GenericBoundRequirement<'program>> = None;
        let mut ambiguous = false;
        root.visit(self, &mut |application| {
            for signature in self.trait_machine_signatures(application.declaration) {
                let matches = if let Some(prepared) = &prepared {
                    prepared.template == signature.symbol
                        && self.requirement_application_matches_scope(target, application)
                } else {
                    Some(signature.symbol) == authored_requirement
                };
                if !matches {
                    continue;
                }
                let signature = prepared
                    .as_ref()
                    .map_or(signature, |prepared| prepared.signature);
                if let Some(known) = &selected {
                    ambiguous |= known.signature.symbol != signature.symbol
                        || !known.application.same_application(self, application);
                    continue;
                }
                selected = Some(GenericBoundRequirement {
                    signature,
                    application: application.clone(),
                });
            }
        });
        if ambiguous {
            return Err(format!(
                "{}: reachable declarations disagree on the requirement application",
                failure()
            ));
        }
        selected.map(Some).ok_or_else(|| {
            format!(
                "{}: its selected child has no reachable authored requirement",
                failure()
            )
        })
    }
}

fn collect_requirement_namespace<'program>(
    program: &'program TypedTrees,
    trait_symbol: SymbolHandle,
    visited: &mut Vec<SymbolHandle>,
    output: &mut Vec<&'program StateSignature>,
) -> Result<(), String> {
    if visited.contains(&trait_symbol) {
        return Ok(());
    }
    let declaration = program
        .trait_definition_by_symbol(trait_symbol)
        .ok_or("evidence namespace lost a resolved trait declaration")?;
    visited.push(trait_symbol);
    output.extend(program.trait_machine_signatures(declaration));
    for parent in declaration
        .refines
        .iter()
        .chain(program.trait_requirements(declaration))
    {
        collect_requirement_namespace(program, parent.symbol, visited, output)?;
    }
    Ok(())
}
