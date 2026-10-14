//! Apply types and expression payloads together; neither arena alone owns a
//! complete signature graph. Substitutions are simultaneous: selected arguments
//! retain their caller scope instead of being rebound in the declaration scope.

use super::CallableApplication;
use crate::{TypedTrees, expression, static_applications, types};
use arena::HandleSpan;

impl CallableApplication {
    pub(super) fn type_reference(
        &mut self,
        program: &mut TypedTrees,
        original: types::TypeReferenceHandle,
    ) -> Result<types::TypeReferenceHandle, &'static str> {
        if !original.is_valid() {
            return Ok(original);
        }
        let owner = program
            .type_reference_table
            .operand_lifetime_owner(original);
        if owner.is_valid()
            && program.lifetime_declaration_owner(owner)
                != program.lifetime_declaration_owner(self.region_scope)
        {
            // Region custody does not imply closed Type/Const payloads.
            // Keep symbol-identified substitutions, but do not rebind the
            // supplying declaration's region names in a foreign telescope.
            let lifetimes = std::mem::take(&mut self.lifetimes);
            let elided_lifetimes = std::mem::take(&mut self.elided_lifetimes);
            let applied = self.type_payload(program, original);
            self.lifetimes = lifetimes;
            self.elided_lifetimes = elided_lifetimes;
            return applied;
        }
        self.type_payload(program, original)
    }

    fn type_payload(
        &mut self,
        program: &mut TypedTrees,
        original: types::TypeReferenceHandle,
    ) -> Result<types::TypeReferenceHandle, &'static str> {
        let mut applied_owner = program
            .type_reference_table
            .operand_lifetime_owner(original);
        let node = match program
            .type_reference_table
            .type_reference(original)
            .clone()
        {
            types::TypeReferenceNode::Named { symbol, name } => {
                if let Some((_, argument)) = self
                    .static_arguments
                    .iter()
                    .rev()
                    .find(|(binder, _)| *binder == symbol)
                {
                    // This operand belongs to the outer caller. Its own
                    // nested application must not be rebound by this view.
                    return Self::selected_argument_reference(program, argument);
                }
                if let Some((_, argument)) = self
                    .types
                    .iter()
                    .rev()
                    .find(|(binder, _)| *binder == symbol)
                {
                    return Ok(*argument);
                }
                types::TypeReferenceNode::Named {
                    symbol: self.symbol(symbol),
                    name,
                }
            }
            types::TypeReferenceNode::Reference {
                referee,
                access,
                lifetime,
            } => {
                let lifetime = lifetime
                    .as_ref()
                    .filter(|lifetime| !self.elided_lifetimes.contains(lifetime))
                    .map(|lifetime| self.scoped_lifetime(program, original, lifetime))
                    .transpose()?;
                if let Some((_, region, _)) = &lifetime
                    && region.declaration.is_valid()
                {
                    applied_owner = region.declaration;
                }
                types::TypeReferenceNode::Reference {
                    referee: self.type_reference(program, referee)?,
                    access,
                    lifetime: lifetime.map(|(name, _, _)| name),
                }
            }
            types::TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let base_type = self.type_reference(program, base_type)?;
                let original_constraints = program
                    .type_reference_table
                    .constraints(constraints)
                    .to_vec();
                let mut constraints = Vec::with_capacity(original_constraints.len());
                for constraint in original_constraints {
                    constraints.push(match constraint {
                        types::TypeConstraintNode::Domain(mut domain) => {
                            domain.arguments = domain
                                .arguments
                                .into_iter()
                                .map(|argument| self.type_reference(program, argument))
                                .collect::<Result<Vec<_>, _>>()?;
                            types::TypeConstraintNode::Domain(domain)
                        }
                        types::TypeConstraintNode::Range {
                            minimum,
                            maximum,
                            end_inclusive,
                        } => types::TypeConstraintNode::Range {
                            minimum: self.expression(program, minimum)?,
                            maximum: self.expression(program, maximum)?,
                            end_inclusive,
                        },
                        constraint => constraint,
                    });
                }
                let constraints = program.type_reference_table.insert_constraints(constraints);
                types::TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                }
            }
            types::TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                let length = match length {
                    types::FixedArrayLength::ConstParameter { symbol, name } => {
                        if let Some((_, argument)) = self
                            .types
                            .iter()
                            .rev()
                            .find(|(binder, _)| *binder == symbol)
                        {
                            match program.type_reference_table.type_reference(*argument) {
                                types::TypeReferenceNode::Named { symbol, name } => {
                                    if let Some(value) = program
                                        .type_reference_table
                                        .anonymous_numeric_literal_value(*argument)
                                    {
                                        types::FixedArrayLength::Literal(value)
                                    } else {
                                        types::FixedArrayLength::ConstParameter {
                                            symbol: *symbol,
                                            name: name.clone(),
                                        }
                                    }
                                }
                                types::TypeReferenceNode::ConstExpression(expression) => {
                                    let expression::ExpressionNode::Integer(value) =
                                        program.expression_table.expression(*expression)
                                    else {
                                        return Err(
                                            "nominal callable array extent application needs a normalized integer argument",
                                        );
                                    };
                                    let value = value.value_u64().and_then(|value| usize::try_from(value).ok())
                                        .ok_or("nominal callable array extent is outside the host extent range")?;
                                    types::FixedArrayLength::Literal(value)
                                }
                                _ => {
                                    return Err(
                                        "nominal callable array extent argument is not a const index",
                                    );
                                }
                            }
                        } else {
                            types::FixedArrayLength::ConstParameter {
                                symbol: self.symbol(symbol),
                                name,
                            }
                        }
                    }
                    length => length,
                };
                types::TypeReferenceNode::FixedArray {
                    element_type: self.type_reference(program, element_type)?,
                    length,
                }
            }
            types::TypeReferenceNode::Slice { element_type } => types::TypeReferenceNode::Slice {
                element_type: self.type_reference(program, element_type)?,
            },
            types::TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                lifetime_arguments,
                arguments,
            } => {
                if lifetime_arguments
                    .iter()
                    .any(|lifetime| self.elided_lifetimes.contains(lifetime))
                {
                    return Err(
                        "elided result region requires an anonymous nominal lifetime representation",
                    );
                }
                let lifetimes = lifetime_arguments
                    .iter()
                    .map(|lifetime| self.scoped_lifetime(program, original, lifetime))
                    .collect::<Result<Vec<_>, _>>()?;
                if let Some((environment, _)) = &self.trait_region_application {
                    let captured = lifetimes.iter().any(|(_, _, substituted)| *substituted);
                    if captured
                        && lifetimes
                            .iter()
                            .any(|(_, region, _)| !region.declaration.is_valid())
                    {
                        return Err(
                            "applied nominal regions have no complete represented supplying telescope",
                        );
                    }
                    // A proven receiving-frame substitution moves the whole
                    // nominal operand to its exact supplier. Unrelated owned
                    // operands retain their original scope. Every region must
                    // still rejoin its declaration/ordinal under that scope.
                    let owner = if captured {
                        environment.argument_owner()
                    } else if applied_owner.is_valid() {
                        applied_owner
                    } else {
                        environment.argument_owner()
                    };
                    for (name, region, _) in &lifetimes {
                        if region.declaration.is_valid()
                            && crate::type_identity::lifetime_parameter_identity(
                                program,
                                owner,
                                name.as_str(),
                            ) != Some(*region)
                        {
                            return Err(
                                "applied nominal regions have no complete represented supplying telescope",
                            );
                        }
                    }
                    if lifetimes
                        .iter()
                        .any(|(_, region, _)| region.declaration.is_valid())
                    {
                        applied_owner = owner;
                    }
                }
                types::TypeReferenceNode::Generic {
                    base_symbol: self.symbol(base_symbol),
                    base_name,
                    lifetime_arguments: lifetimes.into_iter().map(|(name, _, _)| name).collect(),
                    arguments: self.type_arguments(program, arguments)?,
                }
            }
            types::TypeReferenceNode::DynamicTrait {
                symbol,
                name,
                arguments,
            } => types::TypeReferenceNode::DynamicTrait {
                symbol: self.symbol(symbol),
                name,
                arguments: self.type_arguments(program, arguments)?,
            },
            types::TypeReferenceNode::ConstExpression(expression) => {
                types::TypeReferenceNode::ConstExpression(self.expression(program, expression)?)
            }
            types::TypeReferenceNode::Unit => types::TypeReferenceNode::Unit,
        };
        let origins = program
            .type_reference_table
            .constant_origins(original)?
            .to_vec();
        let applied = program.type_reference_table.insert(node);
        program
            .type_reference_table
            .attach_constant_origins(applied, origins)?;
        program
            .type_reference_table
            .retain_operand_lifetime_owner(applied, applied_owner)?;
        Ok(applied)
    }

    pub(super) fn type_arguments(
        &mut self,
        program: &mut TypedTrees,
        original: HandleSpan<types::TypeReferenceHandle>,
    ) -> Result<HandleSpan<types::TypeReferenceHandle>, &'static str> {
        let arguments = program
            .type_reference_table
            .type_reference_handles(original)
            .to_vec();
        if arguments.len() != original.len() {
            return Err("applied callable has a stale type-argument span");
        }
        let applied = arguments
            .into_iter()
            .map(|argument| self.type_reference(program, argument))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(program
            .type_reference_table
            .insert_type_reference_handles(applied))
    }

    pub(super) fn expression(
        &mut self,
        program: &mut TypedTrees,
        original: expression::ExpressionHandle,
    ) -> Result<expression::ExpressionHandle, &'static str> {
        if !original.is_valid() {
            return Ok(original);
        }
        let first = program.expression_table.iter_expressions().count() + 1;
        let copied = program.expression_table.insert_copy(original);
        // Snapshot only this copy's rows. Applying a type payload can append
        // another expression graph; that graph is applied by its own invocation.
        let rows = program
            .expression_table
            .iter_expressions()
            .filter(|(handle, _)| handle.arena_index() as usize >= first)
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>();
        // Signature predicates can retain unresolved local names. Bind only
        // this copied graph against its declaration's lexical telescope,
        // before freshening or substitution; never search global spellings.
        for handle in &rows {
            let expression::ExpressionNode::Name(mut path) =
                program.expression_table.expression(*handle).clone()
            else {
                continue;
            };
            if path.head_symbol.is_valid() || path.symbol.is_valid() {
                continue;
            }
            let [member] = program.expression_table.name_path_members(path.members) else {
                continue;
            };
            if let Some((_, symbol)) = self
                .source_binders
                .iter()
                .rev()
                .find(|(name, _)| name == member)
            {
                path.head_symbol = *symbol;
                path.symbol = *symbol;
                *program.expression_table.expression_mut(*handle) =
                    expression::ExpressionNode::Name(path);
            }
        }
        program
            .expression_table
            .remap_symbols_in(copied, &self.symbols);
        for handle in rows {
            let node = program.expression_table.expression(handle).clone();
            let applied = match node {
                expression::ExpressionNode::Name(path) => {
                    if self
                        .static_arguments
                        .iter()
                        .any(|(binder, _)| *binder == path.head_symbol)
                    {
                        // Evidence is a selected namespace, not a runtime value.
                        // Copying its old child symbols would retain authority
                        // in the provider scope. An applied requirement/member
                        // tuple join must precede support for expression names.
                        return Err(
                            "applied evidence expression requires an exact requirement namespace mapping",
                        );
                    }
                    if let Some((_, argument)) = self
                        .types
                        .iter()
                        .rev()
                        .find(|(binder, _)| *binder == path.head_symbol)
                    {
                        // The replacement belongs to the caller, not the
                        // declaration: copy it without applying this telescope.
                        let replacement = Self::selected_value_expression(program, *argument)?;
                        Some(program.expression_table.expression(replacement).clone())
                    } else {
                        None
                    }
                }
                expression::ExpressionNode::Cast(mut cast) => {
                    cast.target_type = self.type_reference(program, cast.target_type)?;
                    cast.result_type = self.type_reference(program, cast.result_type)?;
                    cast.semantic_domain_arguments =
                        self.type_arguments(program, cast.semantic_domain_arguments)?;
                    Some(expression::ExpressionNode::Cast(cast))
                }
                expression::ExpressionNode::ZeroValue(reference) => Some(
                    expression::ExpressionNode::ZeroValue(self.type_reference(program, reference)?),
                ),
                expression::ExpressionNode::Call(mut call) => {
                    self.static_arguments(program, &mut call.machine_arguments)?;
                    let application = call.authored_machine_application;
                    if application.is_valid() {
                        let lifetimes = program
                            .expression_table
                            .static_machine_application_lifetimes(application)
                            .iter()
                            .map(
                                |binding| static_applications::StaticMachineLifetimeBinding {
                                    parameter: binding.parameter.clone(),
                                    argument: self.lifetime(&binding.argument),
                                },
                            )
                            .collect::<Vec<_>>();
                        program
                            .expression_table
                            .set_static_machine_application_lifetimes(application, &lifetimes);
                        let mut arguments = program
                            .expression_table
                            .static_machine_application_arguments(application)
                            .to_vec();
                        self.static_arguments(program, &mut arguments)?;
                        program
                            .expression_table
                            .static_machine_application_arguments_mut(application)
                            .clone_from_slice(&arguments);
                        let mut selected = program
                            .expression_table
                            .static_machine_application_selected_machines(application)
                            .to_vec();
                        self.static_arguments(program, &mut selected)?;
                        program
                            .expression_table
                            .static_machine_application_selected_machines_mut(application)
                            .clone_from_slice(&selected);
                        let mut bindings = program
                            .expression_table
                            .static_machine_application_bindings(application)
                            .to_vec();
                        for binding in &mut bindings {
                            match &mut binding.value {
                                static_applications::StaticMachineBindingValue::Type(reference)
                                | static_applications::StaticMachineBindingValue::Const(
                                    reference,
                                ) => *reference = self.type_reference(program, *reference)?,
                                static_applications::StaticMachineBindingValue::RuntimeValue {
                                    type_reference,
                                    ..
                                } => {
                                    *type_reference =
                                        self.type_reference(program, *type_reference)?
                                }
                                static_applications::StaticMachineBindingValue::Machine(
                                    argument,
                                )
                                | static_applications::StaticMachineBindingValue::Conformance(
                                    argument,
                                ) => {
                                    self.static_arguments(program, std::slice::from_mut(argument))?
                                }
                            }
                        }
                        program
                            .expression_table
                            .static_machine_application_bindings_mut(application)
                            .clone_from_slice(&bindings);
                    }
                    Some(expression::ExpressionNode::Call(call))
                }
                _ => None,
            };
            if let Some(applied) = applied {
                *program.expression_table.expression_mut(handle) = applied;
            }
            if let Some(mut authority) = program
                .expression_table
                .index_expression_authority(handle)
                .cloned()
            {
                if let Some(normalization) = &mut authority.normalization {
                    normalization.index_type =
                        self.type_reference(program, normalization.index_type)?;
                }
                if let Some(operation) = &mut authority.operation {
                    operation.operand_type =
                        self.type_reference(program, operation.operand_type)?;
                    if let Some(application) = &mut operation.application {
                        self.static_arguments(program, std::slice::from_mut(application))?;
                    }
                    if let Some((_, selected)) = self
                        .static_arguments
                        .iter()
                        .rev()
                        .find(|(binder, _)| *binder == operation.provider)
                    {
                        let alias = if program.symbols.get(selected.symbol).kind
                            == symbols::SymbolKind::ConformanceParameter
                        {
                            program.symbols.name(selected.symbol).to_owned()
                        } else {
                            let conformance = program.conformance_by_symbol(selected.symbol)
                                .ok_or("applied index authority lost its selected evidence declaration")?;
                            conformance
                                .alias
                                .as_ref()
                                .map_or("<unnamed-conformance>", |alias| alias.as_str())
                                .to_owned()
                        };
                        operation.apply_provider(selected, &alias);
                    }
                }
                *program
                    .expression_table
                    .index_expression_authority_mut(handle) = authority;
            }
        }
        Ok(copied)
    }

    fn static_arguments(
        &mut self,
        program: &mut TypedTrees,
        arguments: &mut [expression::StaticMachineArgument],
    ) -> Result<(), &'static str> {
        for argument in arguments {
            if let Some((_, replacement)) = self
                .static_arguments
                .iter()
                .rev()
                .find(|(symbol, _)| *symbol == argument.symbol)
            {
                // A selected operand is already in the caller's scope. Do not
                // recurse into it under the provider's telescope a second time.
                *argument = replacement.clone();
                continue;
            }
            if let Some((_, replacement)) = self
                .types
                .iter()
                .rev()
                .find(|(symbol, _)| *symbol == argument.symbol)
            {
                argument.type_reference = *replacement;
                match program.type_reference_table.type_reference(*replacement) {
                    types::TypeReferenceNode::Named { symbol, name } => {
                        argument.symbol = *symbol;
                        argument.path = vec![name.clone()].into_boxed_slice();
                    }
                    types::TypeReferenceNode::ConstExpression(value) => {
                        if let expression::ExpressionNode::Integer(value) =
                            program.expression_table.expression(*value)
                        {
                            argument.const_literal = Some(value.clone());
                        }
                        argument.symbol = symbols::SymbolHandle::invalid();
                        argument.path = Box::new([]);
                    }
                    _ => {
                        // The complete compound operand is retained above;
                        // the old binder path is diagnostic text, not identity.
                        argument.symbol = symbols::SymbolHandle::invalid();
                        argument.path = Box::new([]);
                    }
                }
            } else {
                argument.type_reference = self.type_reference(program, argument.type_reference)?;
            }
            if let Some(application) = &mut argument.application {
                for lifetime in &mut application.lifetime_arguments {
                    *lifetime = self.lifetime(lifetime);
                }
                self.static_arguments(program, &mut application.arguments)?;
            }
        }
        Ok(())
    }
}
