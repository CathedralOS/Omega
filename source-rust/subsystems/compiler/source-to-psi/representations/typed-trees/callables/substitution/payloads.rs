use super::{CallableApplication, CallableLifetimeOrigin};
use crate::source_state::{TypedTrees, expression, static_applications, types};
use arena::HandleSpan;

impl CallableApplication {
    pub fn type_reference(
        &mut self,
        program: &mut TypedTrees,
        original: types::TypeReferenceHandle,
    ) -> Result<types::TypeReferenceHandle, &'static str> {
        let obligations = program
            .type_reference_table
            .type_alias_applications(original)
            .cloned()
            .collect::<Vec<_>>();
        let applied = self.type_reference_with_owner(program, original)?;
        if obligations.is_empty() {
            return Ok(applied);
        }
        let applied = program.type_reference_table.fresh_occurrence(applied);
        for obligation in obligations {
            let application = self.type_reference(program, obligation.application)?;
            let (declaration, arguments, lifetime_arguments) =
                match program.type_reference_node(application) {
                    types::TypeReferenceNode::Generic {
                        base_symbol,
                        arguments,
                        lifetime_arguments,
                        ..
                    } => (*base_symbol, *arguments, lifetime_arguments.clone()),
                    types::TypeReferenceNode::Named { symbol, .. } => {
                        (*symbol, HandleSpan::empty(), Vec::new())
                    }
                    _ => return Err("applied alias obligation lost its declaration head"),
                };
            if declaration != obligation.declaration {
                return Err("applied alias obligation changed its declaration");
            }
            let supplied_operand_owner = program
                .type_reference_table
                .operand_lifetime_owner(application);
            program.type_reference_table.retain_type_alias_application(
                crate::source_state::type_aliases::TypeAliasApplication {
                    instance: applied,
                    application,
                    arguments,
                    lifetime_arguments,
                    supplied_operand_owner,
                    ..obligation
                },
            );
        }
        Ok(applied)
    }

    fn type_reference_with_owner(
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
            && !program
                .type_reference_table
                .has_resolved_operand_lifetimes(original)
            && program.lifetime_declaration_owner(owner)
                != program.lifetime_declaration_owner(self.region_scope)
        {
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
        let mut applied_lifetimes = Vec::new();
        let mut supplied_origins = Vec::new();
        let mut supplied_extent_argument = None;
        let node = match program.type_reference_node(original).clone() {
            types::TypeReferenceNode::Named { symbol, name } => {
                if let Some((_, argument)) = self
                    .static_arguments
                    .iter()
                    .rev()
                    .find(|(binder, _)| *binder == symbol)
                {
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
                let count = program
                    .unique_data_definition_by_symbol(symbol)
                    .map_or(0, |data| data.lifetime_parameters.len());
                if self.invocation_elision.is_some() && count != 0 {
                    let regions = (0..count)
                        .map(|_| self.invocation_region(program))
                        .collect::<Result<Vec<_>, _>>()?;
                    applied_owner = regions[0].region.declaration;
                    applied_lifetimes.extend(regions.iter().map(|region| region.region));
                    types::TypeReferenceNode::Generic {
                        base_symbol: self.symbol(symbol),
                        base_name: name,
                        lifetime_arguments: regions.into_iter().map(|region| region.name).collect(),
                        arguments: Default::default(),
                    }
                } else {
                    types::TypeReferenceNode::Named {
                        symbol: self.symbol(symbol),
                        name,
                    }
                }
            }
            types::TypeReferenceNode::Reference {
                referee,
                access,
                lifetime,
            } => {
                // Reserve the outer slot before traversing its referent,
                // exactly as signature identity and stage 05 read inputs.
                let lifetime = if lifetime.is_none() && self.invocation_elision.is_some() {
                    Some(self.invocation_region(program)?)
                } else {
                    lifetime
                        .as_ref()
                        .filter(|lifetime| !self.elided_lifetimes.contains(lifetime))
                        .map(|lifetime| self.scoped_lifetime(program, original, 0, lifetime))
                        .transpose()?
                };
                if let Some(lifetime) = &lifetime {
                    applied_lifetimes.push(lifetime.region);
                    if lifetime.region.declaration.is_valid() {
                        applied_owner = lifetime.region.declaration;
                    }
                }
                types::TypeReferenceNode::Reference {
                    referee: self.type_reference(program, referee)?,
                    access,
                    lifetime: lifetime.map(|lifetime| lifetime.name),
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
                                if domain.subject == types::DomainConstraintSubject::Declared
                                    && program.domain_definition(domain.symbol).is_some()
                                {
                                    domain.semantic_id = program.retain_domain_application(
                                        domain.symbol, base_type, &domain.arguments,
                                    ).map_err(|_| "copied qualification lost its complete domain application")?;
                                }
                                domain.semantic_roles.denotation_dimension = domain
                                    .semantic_roles
                                    .denotation_dimension
                                    .map(|_| domain.semantic_id);
                                domain.semantic_roles.arithmetic_policy = domain
                                    .semantic_roles
                                    .arithmetic_policy
                                    .map(|_| domain.semantic_id);
                                types::TypeConstraintNode::Domain(domain)
                            }

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
                            supplied_extent_argument = Some(*argument);
                            match program.type_reference_node(*argument) {
                                types::TypeReferenceNode::Named { symbol, name } => {
                                    if let Some(value) = program
                                        .type_reference_table
                                        .anonymous_numeric_literal_value(*argument)
                                    {
                                        types::FixedArrayLength::Literal(value)
                                    } else if let Some(value) =
                                        crate::source_state::integer_const_declaration_extent(
                                            program, *symbol,
                                        )
                                    {
                                        supplied_origins.extend(
                                            program
                                                .type_reference_table
                                                .constant_origins(*argument)?
                                                .iter()
                                                .copied(),
                                        );
                                        types::FixedArrayLength::Literal(value)
                                    } else {
                                        types::FixedArrayLength::ConstParameter {
                                            symbol: *symbol,
                                            name: name.clone(),
                                        }
                                    }
                                }
                                types::TypeReferenceNode::ConstExpression(expression) => {
                                    match program.expression_table.expression(*expression) {
                                        expression::ExpressionNode::Integer(value) => {
                                            let value = value.value_u64().and_then(|value| usize::try_from(value).ok())
                                                .ok_or("nominal callable array extent is outside the host extent range")?;
                                            types::FixedArrayLength::Literal(value)
                                        }
                                        _ => types::FixedArrayLength::Expression(*argument),
                                    }
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
                    types::FixedArrayLength::Expression(reference) => {
                        types::FixedArrayLength::Expression(
                            self.type_reference(program, reference)?,
                        )
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
                let nominal_owner = self.symbol(base_symbol);
                if !applied_owner.is_valid()
                    && !lifetime_arguments.is_empty()
                    && lifetime_arguments.iter().all(|lifetime| {
                        !self
                            .lifetimes
                            .iter()
                            .any(|binding| binding.parameter == *lifetime)
                            && crate::source_state::type_identity::lifetime_parameter_identity(
                                program,
                                self.region_scope,
                                lifetime.as_str(),
                            )
                            .is_some()
                    })
                {
                    applied_owner = self.region_scope;
                }
                if !applied_owner.is_valid()
                    && lifetime_arguments
                        .iter()
                        .any(|lifetime| self.elided_lifetimes.contains(lifetime))
                {
                    applied_owner = self.region_scope;
                }
                let lifetimes = if lifetime_arguments.is_empty()
                    && self.invocation_elision.is_some()
                {
                    let count = program
                        .unique_data_definition_by_symbol(base_symbol)
                        .map_or(0, |data| data.lifetime_parameters.len());
                    let regions = (0..count)
                        .map(|_| self.invocation_region(program))
                        .collect::<Result<Vec<_>, _>>()?;
                    if let Some(first) = regions.first() {
                        applied_owner = first.region.declaration;
                    }
                    regions
                } else {
                    lifetime_arguments
                    .iter()
                    .enumerate()
                    .map(|(slot, lifetime)| {
                        if self.elided_lifetimes.contains(lifetime) {
                            if crate::source_state::type_identity::lifetime_parameter_identity(
                                program,
                                applied_owner,
                                lifetime.as_str(),
                            )
                            .is_none()
                            {
                                return Err(
                                    "elided nominal result lost its supplying lifetime telescope",
                                );
                            }
                            return Ok(super::ScopedCallableLifetime {
                                name: lifetime.clone(),
                                region: Default::default(),
                                origin: super::CallableLifetimeOrigin::Lexical,
                            });
                        }
                        if self.invocation_elision.is_some() && program.type_reference_table.operand_lifetimes(original).get(slot).is_some_and(|region| !region.declaration.is_valid()) {
                            return self.invocation_region(program);
                        }
                        self.scoped_lifetime(program, original, slot, lifetime)
                    })
                    .collect::<Result<Vec<_>, _>>()?
                };
                for lifetime in &lifetimes {
                    if lifetime.origin == CallableLifetimeOrigin::Specialized {
                        applied_owner = lifetime.region.declaration;
                        continue;
                    }
                    if lifetime.region.declaration.is_valid()
                        && crate::source_state::type_identity::lifetime_parameter_name(
                            program,
                            lifetime.region,
                        ) != Some(&lifetime.name)
                    {
                        return Err("applied nominal region lost its exact declaration ordinal");
                    }
                }
                if let Some((environment, _)) = &self.trait_region_application {
                    let supplier = environment.argument_owner();
                    if lifetimes.iter().all(|lifetime| {
                        lifetime.origin == CallableLifetimeOrigin::Captured
                            && crate::source_state::type_identity::lifetime_parameter_identity(
                                program,
                                supplier,
                                lifetime.name.as_str(),
                            ) == Some(lifetime.region)
                    }) {
                        applied_owner = supplier;
                    } else if !applied_owner.is_valid() {
                        applied_owner = self.region_scope;
                    }
                }
                applied_lifetimes.extend(lifetimes.iter().map(|lifetime| lifetime.region));
                types::TypeReferenceNode::Generic {
                    base_symbol: nominal_owner,
                    base_name,
                    lifetime_arguments: lifetimes
                        .into_iter()
                        .map(|lifetime| lifetime.name)
                        .collect(),
                    arguments: self.type_arguments(program, arguments)?,
                }
            }
            types::TypeReferenceNode::ConstExpression(expression) => {
                types::TypeReferenceNode::ConstExpression(self.expression(program, expression)?)
            }
            types::TypeReferenceNode::Callable { contract } => types::TypeReferenceNode::Callable {
                contract: self.runtime_contract(program, contract)?,
            },
            types::TypeReferenceNode::Unit => types::TypeReferenceNode::Unit,
        };
        let mut origins = program
            .type_reference_table
            .constant_origins(original)?
            .to_vec();
        origins.extend(supplied_origins);
        let extent_argument = match (&node, supplied_extent_argument) {
            (types::TypeReferenceNode::FixedArray { .. }, Some(argument)) => Some(argument),
            (
                types::TypeReferenceNode::FixedArray {
                    length: types::FixedArrayLength::Expression(argument),
                    ..
                },
                None,
            ) => Some(*argument),
            (types::TypeReferenceNode::FixedArray { .. }, None) => {
                let argument = program.type_reference_table.array_extent_argument(original);
                if argument.is_valid() {
                    Some(self.type_reference(program, argument)?)
                } else {
                    None
                }
            }
            _ => None,
        };
        let applied = program.type_reference_table.insert(node);
        if let Some(argument) = extent_argument {
            program
                .type_reference_table
                .retain_array_extent_argument(applied, argument)?;
        }
        let origin = program.type_reference_table.operand_origin(original);
        program
            .type_reference_table
            .set_operand_origin(applied, origin);
        program
            .type_reference_table
            .attach_constant_origins(applied, origins)?;
        program
            .type_reference_table
            .retain_operand_lifetime_owner(applied, applied_owner)?;
        program
            .type_reference_table
            .retain_operand_lifetimes(applied, applied_lifetimes)?;
        Ok(applied)
    }

    pub(crate) fn type_arguments(
        &mut self,
        program: &mut TypedTrees,
        original: HandleSpan<types::TypeReferenceHandle>,
    ) -> Result<HandleSpan<types::TypeReferenceHandle>, &'static str> {
        let arguments = program.type_reference_handles(original).to_vec();
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

    pub fn expression(
        &mut self,
        program: &mut TypedTrees,
        original: expression::ExpressionHandle,
    ) -> Result<expression::ExpressionHandle, &'static str> {
        if !original.is_valid() {
            return Ok(original);
        }
        // The copy appends its rows after every existing one, so start at the
        // first new slot instead of scanning the whole table on each copy.
        let first = program.expression_table.expression_count() + 1;
        let copied = program.expression_table.insert_copy(original);
        let rows = program
            .expression_table
            .iter_expressions_from(first)
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>();
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
                let mut member_symbols = HandleSpan::empty();
                program
                    .expression_table
                    .push_name_path_member_symbol(&mut member_symbols, *symbol);
                path.member_symbols = member_symbols;
                path.head_symbol = *symbol;
                path.symbol = *symbol;
                *program.expression_table.expression_mut(*handle) =
                    expression::ExpressionNode::Name(path);
            }
        }
        let mut symbols = self.symbols.clone();
        symbols.extend(self.declaration_symbols.rebinding().iter().copied());
        for handle in &rows {
            if let expression::ExpressionNode::Name(path) =
                program.expression_table.expression(*handle)
            {
                let binder = path.head_symbol;
                if binder.is_valid()
                    && program.symbols.get(binder).kind == symbols::SymbolKind::ConformanceParameter
                    && !symbols.iter().any(|(source, _)| *source == binder)
                {
                    symbols.extend(self.namespace_symbols(program, binder, &rows)?);
                }
            }
        }
        let call_targets = self.expression_call_targets(program, &rows)?;
        program.expression_table.remap_symbols_in(copied, &symbols);
        self.apply_expression_rows(program, &rows, call_targets)?;
        Ok(copied)
    }

    pub(crate) fn expression_call_targets(
        &self,
        program: &TypedTrees,
        rows: &[expression::ExpressionHandle],
    ) -> Result<Vec<Option<super::call_targets::AppliedCallTarget>>, &'static str> {
        rows.iter()
            .map(
                |handle| match program.expression_table.expression(*handle) {
                    expression::ExpressionNode::Call(call) => {
                        self.selected_call_target(program, call.target_symbol)
                    }
                    _ => Ok(None),
                },
            )
            .collect()
    }

    /// Specialization and contract application share the ownership of all
    /// types retained by copied expressions. Keep this exhaustive over node
    /// kinds so a new node cannot silently retain a template's type rows.
    pub(crate) fn apply_expression_rows(
        &mut self,
        program: &mut TypedTrees,
        rows: &[expression::ExpressionHandle],
        call_targets: Vec<Option<super::call_targets::AppliedCallTarget>>,
    ) -> Result<(), &'static str> {
        for (&handle, call_target) in rows.iter().zip(call_targets) {
            let tests = program.expression_table.executable_domain_tests(handle);
            if tests.iter().any(|test| test.application.is_valid()) {
                let mut tests = tests.to_vec();
                for test in tests.iter_mut().filter(|test| test.application.is_valid()) {
                    let original = program
                        .domain_arguments_for_instance(test.domain, test.application)
                        .ok_or("copied membership lost its applied domain")?
                        .to_vec();
                    let arguments = original
                        .into_iter()
                        .map(|argument| self.type_reference(program, argument))
                        .collect::<Result<Vec<_>, _>>()?;
                    test.application = program
                        .retain_domain_arguments(test.domain, &arguments)
                        .map_err(|_| "copied membership lost its substituted application")?;
                }
                program
                    .expression_table
                    .replace_executable_domain_tests(handle, tests);
            }
            let carrier = program
                .expression_table
                .constructed_value_type_reference(handle);
            if carrier.is_valid() {
                let carrier = self.type_reference(program, carrier)?;
                program
                    .expression_table
                    .set_constructed_value_type_reference(handle, carrier);
            }
            let required = program
                .expression_table
                .construction_required_type_reference(handle);
            if required.is_valid() {
                let required = self.type_reference(program, required)?;
                program
                    .expression_table
                    .set_construction_required_type_reference(handle, required);
            }
            let node = program.expression_table.expression(handle).clone();
            let applied = match node {
                expression::ExpressionNode::Name(path) => {
                    if path.static_application.is_valid() {
                        let mut argument = program
                            .expression_table
                            .static_member(path.static_application)
                            .ok_or("copied name lost its retained static operand")?
                            .clone();
                        self.static_arguments(program, std::slice::from_mut(&mut argument))?;
                        *program
                            .expression_table
                            .static_member_mut(path.static_application)
                            .ok_or("copied name lost its retained static operand")? = argument;
                    }
                    if path.head_symbol.is_valid()
                        && program.symbols.get(path.head_symbol).kind
                            == symbols::SymbolKind::ConformanceParameter
                    {
                        continue;
                    }
                    if self
                        .static_arguments
                        .iter()
                        .any(|(binder, _)| *binder == path.head_symbol)
                    {
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
                        let argument = *argument;
                        let carrier = program
                            .data_type_parameter_by_symbol(path.head_symbol)
                            .and_then(|parameter| match parameter.kind {
                                crate::source_state::data::TypeParameterKind::Const {
                                    type_reference,
                                }
                                | crate::source_state::data::TypeParameterKind::Value {
                                    type_reference,
                                } => Some(type_reference),
                                _ => None,
                            });
                        let carrier = match carrier {
                            Some(carrier) => self.type_reference(program, carrier)?,
                            None => types::TypeReferenceHandle::invalid(),
                        };
                        let replacement = Self::selected_value_expression(
                            program,
                            argument,
                            carrier,
                            self.region_scope,
                        )?;
                        Some(program.expression_table.expression(replacement).clone())
                    } else {
                        None
                    }
                }
                expression::ExpressionNode::StructLiteral(mut literal) => {
                    literal.application_type =
                        self.type_reference(program, literal.application_type)?;
                    for reference in literal
                        .package
                        .iter_mut()
                        .flat_map(|package| package.type_references_mut())
                    {
                        *reference = self.type_reference(program, *reference)?;
                    }
                    Some(expression::ExpressionNode::StructLiteral(literal))
                }
                // The block's statements are applied with the body's.
                expression::ExpressionNode::Block(_) => None,
                expression::ExpressionNode::Match(mut dispatch) => {
                    let mut arms = program.expression_table.match_arms(dispatch.arms).to_vec();
                    for arm in &mut arms {
                        if let expression::MatchPattern::Destructure(ref mut pattern) = arm.pattern
                        {
                            pattern.application_type =
                                self.type_reference(program, pattern.application_type)?;
                        }
                    }
                    dispatch.arms = program.expression_table.insert_match_arms(arms);
                    Some(expression::ExpressionNode::Match(dispatch))
                }
                expression::ExpressionNode::Cast(mut cast) => {
                    cast.target_type = self.type_reference(program, cast.target_type)?;
                    cast.operand_type = self.type_reference(program, cast.operand_type)?;
                    cast.result_type = self.type_reference(program, cast.result_type)?;
                    cast.recast_storage_type =
                        self.type_reference(program, cast.recast_storage_type)?;
                    cast.semantic_domain_arguments =
                        self.type_arguments(program, cast.semantic_domain_arguments)?;
                    let arguments = program
                        .type_reference_table
                        .type_reference_handles(cast.semantic_domain_arguments)
                        .to_vec();
                    cast.semantic_domain_id = self.domain_identity(
                        program,
                        cast.semantic_domain_symbol,
                        &arguments,
                        cast.semantic_domain_id,
                    )?;
                    Some(expression::ExpressionNode::Cast(cast))
                }
                expression::ExpressionNode::TypeExpression(reference) => {
                    Some(expression::ExpressionNode::TypeExpression(
                        self.type_reference(program, reference)?,
                    ))
                }
                expression::ExpressionNode::ZeroValue(reference) => Some(
                    expression::ExpressionNode::ZeroValue(self.type_reference(program, reference)?),
                ),
                expression::ExpressionNode::Call(mut call) => {
                    if let Some(runtime) = &mut call.runtime_call {
                        runtime.contract = self.runtime_contract(program, runtime.contract)?;
                    }
                    if let Some(request) = &mut call.installed_interrupt_operation {
                        request.prepared = None;
                    }
                    call.requirement_application =
                        self.type_reference(program, call.requirement_application)?;
                    call.exact_coercion_target =
                        self.type_reference(program, call.exact_coercion_target)?;
                    self.static_arguments(program, &mut call.machine_arguments)?;
                    for argument in call.request_static_arguments_mut() {
                        self.static_arguments(program, std::slice::from_mut(argument))?;
                    }
                    self.apply_call_application(
                        program,
                        crate::source_state::references::StaticArgumentCall::Expression(
                            handle, &call,
                        ),
                    )?;
                    if let Some(target) = call_target {
                        self.apply_call_target(program, &mut call, target)?;
                    }
                    Some(expression::ExpressionNode::Call(call))
                }
                expression::ExpressionNode::MachineValue(mut value) => {
                    value.type_reference = self.type_reference(program, value.type_reference)?;
                    self.static_arguments(program, std::slice::from_mut(&mut value.selected))?;
                    self.apply_static_machine_application(program, value.application)?;
                    value.conformance = self.symbol(value.conformance);
                    value.requirement = self.symbol(value.requirement);
                    Some(expression::ExpressionNode::MachineValue(value))
                }
                expression::ExpressionNode::SavedSubject(_)
                | expression::ExpressionNode::ArrayLiteral(_)
                | expression::ExpressionNode::Atomic(_)
                | expression::ExpressionNode::Binary(_)
                | expression::ExpressionNode::Boolean(_)
                | expression::ExpressionNode::Float(_)
                | expression::ExpressionNode::UndeterminedNaN(_)
                | expression::ExpressionNode::Indexed(_)
                | expression::ExpressionNode::Integer(_)
                | expression::ExpressionNode::Member(_)
                | expression::ExpressionNode::Borrow(_)
                | expression::ExpressionNode::Range(_)
                | expression::ExpressionNode::String(_)
                | expression::ExpressionNode::Unary(_) => None,
            };
            if let Some(applied) = applied {
                *program.expression_table.expression_mut(handle) = applied;
            }
        }
        Ok(())
    }

    /// Copied statements, expressions and stored selections retain the same
    /// complete invocation, including runtime-value expression payloads.
    pub(crate) fn apply_call_application(
        &mut self,
        program: &mut TypedTrees,
        call: crate::source_state::references::StaticArgumentCall<'_>,
    ) -> Result<(), &'static str> {
        let handle = call.application();
        if !handle.is_valid() {
            return Ok(());
        }
        // Build selections and reflection describe authored declarations.
        // Executable operands enter the lexical declarations copied here.
        let arguments = program.static_machine_application(handle).arguments;
        let symbols = self.declaration_symbols.rebinding();
        if !arguments.is_empty()
            && !symbols.is_empty()
            && !matches!(call, crate::source_state::references::StaticArgumentCall::Expression(_, call)
                    if call.build_selection.is_valid())
            && crate::source_state::reflection::construction::declaration_query_argument_role(
                program, call,
            ) == crate::source_state::references::StaticArgumentRole::ValueDependency
        {
            program
                .expression_table
                .remap_static_arguments(arguments, symbols);
        }
        self.apply_static_machine_application(program, handle)
    }

    pub(crate) fn apply_static_machine_application(
        &mut self,
        program: &mut TypedTrees,
        application: static_applications::StaticMachineApplicationHandle,
    ) -> Result<(), &'static str> {
        let lifetimes = program
            .static_machine_application_lifetimes(application)
            .iter()
            .map(
                |binding| static_applications::StaticMachineLifetimeBinding {
                    parameter: binding.parameter.clone(),
                    argument: self.lifetime(&binding.argument),
                },
            )
            .collect::<Vec<_>>();
        let declaration = program
            .static_machine_application(application)
            .lifetime_binding_declaration;
        program
            .expression_table
            .set_static_machine_application_lifetimes(application, declaration, &lifetimes);
        let mut arguments = program
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
            .static_machine_application_bindings(application)
            .to_vec();
        for binding in &mut bindings {
            match &mut binding.value {
                static_applications::StaticMachineBindingValue::Type(reference)
                | static_applications::StaticMachineBindingValue::Const(reference) => {
                    *reference = self.type_reference(program, *reference)?
                }
                static_applications::StaticMachineBindingValue::RuntimeValue {
                    type_reference,
                    expression,
                    ..
                } => {
                    *type_reference = self.type_reference(program, *type_reference)?;
                    *expression = self.expression(program, *expression)?;
                }
                static_applications::StaticMachineBindingValue::Machine(argument)
                | static_applications::StaticMachineBindingValue::Proof(argument)
                | static_applications::StaticMachineBindingValue::Conformance(argument) => {
                    self.static_arguments(program, std::slice::from_mut(argument))?
                }
            }
        }
        program
            .expression_table
            .static_machine_application_bindings_mut(application)
            .clone_from_slice(&bindings);

        Ok(())
    }

    pub fn static_arguments(
        &mut self,
        program: &mut TypedTrees,
        arguments: &mut [expression::StaticMachineArgument],
    ) -> Result<(), &'static str> {
        for argument in arguments {
            let selected = self
                .static_arguments
                .iter()
                .rev()
                .find(|(symbol, _)| *symbol == argument.symbol)
                .map(|(_, replacement)| Ok(replacement.clone()))
                .or_else(|| {
                    self.types
                        .iter()
                        .rev()
                        .find(|(symbol, _)| *symbol == argument.symbol)
                        .map(|(_, replacement)| {
                            program.static_operand_from_type_reference(*replacement)
                        })
                });
            if let Some(selected) = selected {
                if argument.application.is_some() {
                    return Err(
                        "an applied callable binder cannot be replaced as a bare static operand",
                    );
                }
                let mut selected = selected?;
                // A named replacement may omit the type already retained
                // on this occurrence. Keep its substituted type so later
                // consumers do not have to reconstruct it from the name.
                if argument.type_reference.is_valid() {
                    selected.type_reference =
                        self.type_reference(program, argument.type_reference)?;
                }
                *argument = selected;
                continue;
            }
            argument.type_reference = self.type_reference(program, argument.type_reference)?;
            if let Some(application) = &mut argument.application {
                for lifetime in &mut application.lifetime_arguments {
                    *lifetime = self.lifetime(lifetime);
                }
                self.static_arguments(program, &mut application.arguments)?;
                if application.member.is_valid() {
                    let mut member = program
                        .expression_table
                        .static_member(application.member)
                        .ok_or("qualified static member has no owning expression record")?
                        .clone();
                    self.static_arguments(program, std::slice::from_mut(&mut member))?;
                    *program
                        .expression_table
                        .static_member_mut(application.member)
                        .ok_or("qualified static member lost its owning expression record")? =
                        member;
                }
            }
        }
        Ok(())
    }
}
