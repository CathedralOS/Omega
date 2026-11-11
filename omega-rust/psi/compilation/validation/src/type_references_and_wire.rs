/*
Closed const and static type-argument validation (`type_references`) alongside
wire-schema protocol validation (`wire`).
*/
pub mod type_references {
    pub(crate) mod alias_arguments {
        use super::*;

        pub(crate) fn validate(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
            symbols: &TopLevelSymbols<'_>,
            owner: TypeReferenceOwner<'_>,
            scope: TypeParameterScope<'_>,
            diagnostics: &mut Vec<Diagnostic>,
            obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
        ) {
            let receiving_owner = scope.lexical_owner;
            for application in program
                .type_reference_table
                .type_alias_applications(reference)
            {
                let Some(alias) = program
                    .type_aliases()
                    .iter()
                    .find(|alias| alias.symbol == application.declaration)
                else {
                    diagnostics.push(Diagnostic::error(
                        "type alias application has no retained declaration header",
                    ));
                    continue;
                };
                if program.symbols.symbol_source_span(alias.symbol)
                    != Some(application.declaration_source)
                {
                    diagnostics.push(Diagnostic::error(
                        "type alias application has different declaration source custody",
                    ));
                    continue;
                }
                let supplier = application.supplied_operand_owner;
                let supplied_parameters = supplier
                    .is_valid()
                    .then(|| declaration_parameters(program, supplier));
                let scope = TypeParameterScope {
                    type_parameters: supplied_parameters
                        .as_deref()
                        .unwrap_or(scope.type_parameters),
                    lexical_owner: if supplier.is_valid() {
                        supplier
                    } else {
                        scope.lexical_owner
                    },
                    operand_lifetime_owner: if supplier.is_valid() {
                        supplier
                    } else {
                        scope.operand_lifetime_owner
                    },
                    ..scope
                };
                let arguments = program
                    .type_reference_table
                    .type_reference_handles(application.arguments);
                let parameters = program
                    .tables
                    .data_type_parameters
                    .span_or_empty(alias.type_parameters);
                if parameters.len() != arguments.len()
                    || alias.lifetime_parameters.len() != application.lifetime_arguments.len()
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "type alias `{}` requires its complete declared argument telescope",
                        alias.name
                    )));
                    continue;
                }
                for (slot, lifetime) in application.lifetime_arguments.iter().enumerate() {
                    if !scope.contains_operand_lifetime(
                        program,
                        application.application,
                        slot,
                        lifetime.as_str(),
                    ) {
                        diagnostics.push(Diagnostic::error(format!(
                            "{owner} uses undeclared lifetime argument `'{}' in type alias `{}`",
                            lifetime.as_str(),
                            alias.name
                        )));
                    }
                }
                validate_parameter_bounds(
                    program,
                    alias.name.as_str(),
                    parameters,
                    arguments,
                    scope,
                    diagnostics,
                );
                crate::declarations::traits::validate_implementation_application(
                    program,
                    alias.symbol,
                    parameters,
                    &alias.lifetime_parameters,
                    &alias.conformance_bounds,
                    arguments,
                    &application.lifetime_arguments,
                    if scope.operand_lifetime_owner.is_valid() {
                        scope.operand_lifetime_owner
                    } else {
                        scope.lexical_owner
                    },
                    &format!("type alias `{}`", alias.name),
                    diagnostics,
                );
                for (parameter, argument) in parameters.iter().zip(arguments) {
                    let argument_supplier = program
                        .type_reference_table
                        .operand_lifetime_owner(*argument);
                    let argument_parameters = argument_supplier
                        .is_valid()
                        .then(|| declaration_parameters(program, argument_supplier));
                    let scope = TypeParameterScope {
                        type_parameters: argument_parameters
                            .as_deref()
                            .unwrap_or(scope.type_parameters),
                        lexical_owner: if argument_supplier.is_valid() {
                            argument_supplier
                        } else {
                            scope.lexical_owner
                        },
                        operand_lifetime_owner: if argument_supplier.is_valid() {
                            argument_supplier
                        } else {
                            scope.operand_lifetime_owner
                        },
                        ..scope
                    };
                    if let TypeParameterKind::Const { type_reference } = parameter.kind
                        && let TypeReferenceNode::ConstExpression(expression) =
                            program.type_reference_table.type_reference(*argument)
                        && !program.pending_const_range_endpoints.contains(expression)
                        && typed_trees::domain::index_argument_is_closed(
                            program.typed_program(),
                            *argument,
                        )
                    {
                        match check_closed_const_expression_in_carrier(
                            program,
                            scope.lexical_owner,
                            *expression,
                            AppliedValueCarrier::applied(type_reference, parameters, arguments),
                        ) {
                            Ok(Some(_)) => continue,
                            Err(mut errors) => {
                                diagnostics.append(&mut errors);
                                continue;
                            }
                            Ok(None) => {}
                        }
                    }
                    let argument_owner = if scope.lexical_owner == receiving_owner {
                        owner.generic_argument()
                    } else {
                        TypeReferenceOwner::SuppliedOperand {
                            declaration: program.symbols.name(scope.lexical_owner),
                            generic_depth: 1,
                        }
                    };
                    let prior_facts = argument_owner.value_formation_facts(program);
                    let local_formation = argument_owner.local_value_formation();
                    match &parameter.kind {
                TypeParameterKind::Implementation => implementation_arguments::validate_operand_formation(program, *argument, symbols, argument_owner, scope, diagnostics, obligations),
                TypeParameterKind::Const { type_reference } | TypeParameterKind::Value { type_reference } => validate_const_data_argument(
                    program, alias.name.as_str(), parameter, AppliedValueCarrier::applied(*type_reference, parameters, arguments),
                    *argument, scope, &prior_facts, local_formation, diagnostics, obligations,
                ),
                TypeParameterKind::Machine { contract } => validate_machine_data_argument(program, alias.symbol, alias.name.as_str(), parameter, contract, *argument, arguments, scope, diagnostics),
                TypeParameterKind::Proposition { .. } => diagnostics.push(Diagnostic::error(format!("proposition parameter `{}` of type alias `{}` requires a proposition-family argument", parameter.name, alias.name))),
                TypeParameterKind::Type => {
                    if machine_argument_name(program, *argument, scope).is_some() {
                        diagnostics.push(Diagnostic::error(format!("type parameter `{}` of type alias `{}` requires a type argument, not a machine symbol", parameter.name, alias.name)));
                    } else {
                        validate_type_reference_node(program, *argument, symbols, diagnostics, argument_owner, scope, obligations);
                    }
                }
            }
                }
            }
        }

        fn declaration_parameters(
            program: &dyn crate::CheckingInput,
            owner: symbols::SymbolHandle,
        ) -> Vec<TypeParameter> {
            if let Some(machine) = program.machine_declaration_for_lexical_scope(owner) {
                return program
                    .machine_declaration_scope(machine, |left, right| left.symbol == right.symbol)
                    .0;
            }
            let mut parameters = Vec::new();
            let mut seen = Vec::new();
            let mut declaration = owner;
            while declaration.is_valid() && !seen.contains(&declaration) {
                seen.push(declaration);
                for parameter in program.declaration_type_parameters(declaration) {
                    if !parameters
                        .iter()
                        .any(|known: &TypeParameter| known.symbol == parameter.symbol)
                    {
                        parameters.push(parameter.clone());
                    }
                }
                declaration = program.declaration_scope_parent(declaration);
            }
            parameters
        }
    }
    pub(crate) mod const_predicates {
        use crate::frozen_program::expression_table;
        use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
        use symbols::SymbolHandle;
        use typed_trees::data::{DataDefinition, DataField};
        use typed_trees::domain::ProofFact;
        use typed_trees::expression::{
            BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
        };
        use typed_trees::types::TypeReferenceHandle;

        pub(crate) fn validate(
            program: &dyn crate::CheckingInput,
            definition: &DataDefinition,
            facts: arena::HandleSpan<ProofFact>,
            fields: &[&DataField],
            values: &[(String, DecodedCanonicalConstValue)],
            active_case: SymbolHandle,
        ) -> Result<(), String> {
            for fact in program.proof_facts.span_or_empty(facts) {
                let verdict = fact
                    .as_expression()
                    .ok_or("membership and proposition predicates need their own index-use proof")
                    .and_then(|expression| {
                        evaluate(
                            program,
                            definition.symbol,
                            definition.predicate_subject_symbol,
                            active_case,
                            fields,
                            values,
                            *expression,
                            0,
                        )
                    })
                    .and_then(|(value, _)| match value {
                        Scalar::Boolean(true) => Ok(()),
                        Scalar::Boolean(false) => Err("the required predicate is false"),
                        Scalar::Integer(_) => Err("the required predicate is not Boolean"),
                    });
                if let Err(reason) = verdict {
                    return Err(format!(
                        "canonical value of `{}` does not establish its required default-domain predicate at index use: {reason}",
                        definition.name
                    ));
                }
            }
            Ok(())
        }

        pub(crate) fn closed_boolean(
            program: &dyn crate::CheckingInput,
            owner: symbols::SymbolHandle,
            expression: ExpressionHandle,
        ) -> Option<bool> {
            match evaluate(
                program,
                owner,
                SymbolHandle::invalid(),
                SymbolHandle::invalid(),
                &[],
                &[],
                expression,
                0,
            )
            .ok()?
            .0
            {
                Scalar::Boolean(value) => Some(value),
                Scalar::Integer(_) => None,
            }
        }

        #[derive(Clone, Copy)]
        pub(crate) enum Scalar {
            Integer(i128),
            Boolean(bool),
        }

        fn scalar(value: &DecodedCanonicalConstValue) -> Option<Scalar> {
            match value {
                DecodedCanonicalConstValue::Integer { value, .. } => Some(Scalar::Integer(*value)),
                DecodedCanonicalConstValue::Boolean(value) => Some(Scalar::Boolean(*value)),
                _ => None,
            }
        }

        fn evaluate(
            program: &dyn crate::CheckingInput,
            owner: symbols::SymbolHandle,
            subject: SymbolHandle,
            active_case: SymbolHandle,
            fields: &[&DataField],
            values: &[(String, DecodedCanonicalConstValue)],
            expression: ExpressionHandle,
            depth: usize,
        ) -> Result<(Scalar, Option<TypeReferenceHandle>), &'static str> {
            if depth > 128 {
                return Err("closed scalar predicate exceeds the evaluation depth bound");
            }
            if !expression_table(program).expression_is_valid(expression) {
                return Err("closed scalar predicate has an invalid expression handle");
            }
            let operand = |expression| {
                evaluate(
                    program,
                    owner,
                    subject,
                    active_case,
                    fields,
                    values,
                    expression,
                    depth + 1,
                )
            };
            let boolean = |value| {
                let carrier =
                    crate::proof_contracts::bound_expression_meaning::builtin_type_reference(
                        program,
                        symbols::BuiltinTypeAtom::Bool,
                    )
                    .ok_or("closed scalar predicate has no resolved Boolean carrier")?;
                Ok((Scalar::Boolean(value), Some(carrier)))
            };
            match expression_table(program).expression(expression) {
                ExpressionNode::Integer(value) => Ok((
                    Scalar::Integer(
                        value
                            .decimal_value_i128()
                            .ok_or("integer predicate literal exceeds the evaluation carrier")?,
                    ),
                    None,
                )),
                ExpressionNode::Boolean(value) => boolean(*value),
                ExpressionNode::Name(path) => {
                    if !path.symbol.is_valid()
                        || expression_table(program)
                            .name_path_members(path.members)
                            .len()
                            != 1
                    {
                        return Err("predicate value is not one resolved lexical declaration");
                    }
                    if let Some(field) = fields.iter().find(|field| field.symbol == path.symbol) {
                        let (_, value) = values
                            .iter()
                            .find(|(name, _)| name == field.name.as_str())
                            .ok_or("predicate field has no completed value")?;
                        return Ok((
                            scalar(value).ok_or(
                                "predicate field is outside the closed integer/Boolean fragment",
                            )?,
                            Some(field.type_reference),
                        ));
                    }
                    let declaration = program
                        .const_declaration_by_symbol(path.symbol)
                        .ok_or("predicate name is neither its field nor an evaluated constant")?;
                    let encoding = declaration
                        .evaluated_value
                        .as_ref()
                        .and_then(|value| value.encoding())
                        .ok_or("predicate constant has no completed canonical value")?;
                    let value = CanonicalConstValue::new("", encoding, "")
                        .decode_encoding()
                        .ok_or("predicate constant has a malformed canonical encoding")?;
                    Ok((
                        scalar(&value).ok_or(
                            "predicate constant is outside the closed integer/Boolean fragment",
                        )?,
                        Some(declaration.declared_type),
                    ))
                }
                ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                    let (Scalar::Boolean(value), _) = operand(unary.operand)? else {
                        return Err("logical negation requires a Boolean predicate value");
                    };
                    boolean(!value)
                }
                ExpressionNode::Call(call) => {
                    let arguments = expression_table(program).expression_handles(call.arguments);
                    let values = arguments
                        .iter()
                        .map(|argument| {
                            let (value, carrier) = operand(*argument)?;
                            match value {
                                Scalar::Integer(value) => Ok((value, carrier)),
                                Scalar::Boolean(_) => {
                                    Err("closed predicate application needs integer actuals")
                                }
                            }
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let value =
                crate::proof_contracts::collection_predicates::closed_integer_predicate_value(
                    program, expression, &values,
                )
                .ok_or("closed predicate call has no fully decided admitted integer application")?;
                    boolean(value)
                }
                ExpressionNode::Member(_) => {
                    let (value, reference) = projected_field_value(
                        program, owner, subject, fields, values, expression, depth,
                    )?;
                    Ok((
                        scalar(value).ok_or(
                            "predicate field is outside the closed integer/Boolean fragment",
                        )?,
                        Some(reference),
                    ))
                }
                ExpressionNode::Binary(binary) => {
                    if let Some(matches_case) = case_membership(
                        program,
                        subject,
                        active_case,
                        binary.operator,
                        binary.left,
                        binary.right,
                    ) {
                        return boolean(matches_case);
                    }
                    if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
                        let (left, _) = operand(binary.left)?;
                        if let Scalar::Boolean(value) = left {
                            let decided = match binary.operator {
                                BinaryOperator::Or => value,
                                BinaryOperator::And => !value,
                                _ => false,
                            };
                            if decided {
                                return boolean(value);
                            }
                        }
                    }
                    let (left, left_type) = operand(binary.left)?;
                    let (right, right_type) = operand(binary.right)?;
                    if let Some(spelling) =
                crate::proof_contracts::parameter_expression_meaning::spelled_binary_operator(
                    binary.operator,
                )
                && !typed_trees::operator::has_builtin_spelled_expression_meaning(
                    program,
                    owner,
                    expression,
                    spelling,
                    &[left_type, right_type],
                )
            {
                return Err("predicate operator does not select builtin scalar meaning");
            }
                    let value = match (left, right) {
                        (Scalar::Integer(left), Scalar::Integer(right)) => match binary.operator {
                            BinaryOperator::Equal => left == right,
                            BinaryOperator::NotEqual => left != right,
                            BinaryOperator::Less => left < right,
                            BinaryOperator::LessOrEqual => left <= right,
                            BinaryOperator::Greater => left > right,
                            BinaryOperator::GreaterOrEqual => left >= right,
                            _ => {
                                return Err(
                                    "integer predicate operation is outside the closed comparison fragment",
                                );
                            }
                        },
                        (Scalar::Boolean(left), Scalar::Boolean(right)) => match binary.operator {
                            BinaryOperator::Equal => left == right,
                            BinaryOperator::NotEqual => left != right,
                            BinaryOperator::And => left && right,
                            BinaryOperator::Or => left || right,
                            _ => {
                                return Err(
                                    "Boolean predicate operation is outside the closed logical fragment",
                                );
                            }
                        },
                        _ => {
                            return Err(
                                "predicate operands do not share integer or Boolean meaning",
                            );
                        }
                    };
                    boolean(value)
                }
                _ => Err("predicate expression is outside the closed scalar fragment"),
            }
        }

        fn names_subject(
            program: &dyn crate::CheckingInput,
            subject: SymbolHandle,
            expression: ExpressionHandle,
        ) -> bool {
            let ExpressionNode::Name(path) = expression_table(program).expression(expression)
            else {
                return false;
            };
            subject.is_valid()
                && path.symbol == subject
                && expression_table(program)
                    .name_path_members(path.members)
                    .len()
                    == 1
        }

        fn case_membership(
            program: &dyn crate::CheckingInput,
            subject: SymbolHandle,
            active_case: SymbolHandle,
            operator: BinaryOperator,
            left: ExpressionHandle,
            right: ExpressionHandle,
        ) -> Option<bool> {
            if !matches!(
                operator,
                BinaryOperator::Equal | BinaryOperator::CaseMembership
            ) || !active_case.is_valid()
            {
                return None;
            }
            let case_expression = if names_subject(program, subject, left) {
                right
            } else if names_subject(program, subject, right) {
                left
            } else {
                return None;
            };
            let ExpressionNode::Name(path) = expression_table(program).expression(case_expression)
            else {
                return None;
            };
            if expression_table(program)
                .name_path_members(path.members)
                .len()
                != 2
                || !path.symbol.is_valid()
            {
                return None;
            }
            Some(path.symbol == active_case)
        }

        fn projected_field_value<'value>(
            program: &dyn crate::CheckingInput,
            owner: SymbolHandle,
            subject: SymbolHandle,
            fields: &[&DataField],
            values: &'value [(String, DecodedCanonicalConstValue)],
            expression: ExpressionHandle,
            depth: usize,
        ) -> Result<(&'value DecodedCanonicalConstValue, TypeReferenceHandle), &'static str>
        {
            if depth > 128 {
                return Err("closed predicate projection exceeds the evaluation depth bound");
            }
            let direct = |symbol| {
                let selected = if fields.iter().any(|field| field.symbol == symbol) {
                    symbol
                } else {
                    let definition = program
                        .unique_data_definition_by_symbol(owner)
                        .ok_or("closed predicate has no receiving declaration")?;
                    let (source, _) = program
                        .any_field_owner(symbol)
                        .ok_or("closed predicate projection has no source field")?;
                    program
                        .data_constructor_field(
                            definition
                                .generic_instance
                                .unwrap_or(definition.predicate_subject_type),
                            source.symbol,
                            symbol,
                        )
                        .ok_or("closed predicate projection has no exact applied field")?
                        .symbol
                };
                let field = fields
                    .iter()
                    .find(|field| field.symbol == selected)
                    .ok_or("predicate projection has no exact declared field")?;
                let (_, value) = values
                    .iter()
                    .find(|(name, _)| name == field.name.as_str())
                    .ok_or("predicate field has no completed value")?;
                Ok((value, field.type_reference))
            };
            match expression_table(program).expression(expression) {
                ExpressionNode::Name(name) => direct(name.symbol),
                ExpressionNode::Member(member)
                    if names_subject(program, subject, member.receiver) =>
                {
                    direct(member.member_symbol)
                }
                ExpressionNode::Member(member) => {
                    let (receiver, reference) = projected_field_value(
                        program,
                        owner,
                        subject,
                        fields,
                        values,
                        member.receiver,
                        depth + 1,
                    )?;
                    if crate::reference_shell(program, reference).is_some() {
                        return Err(
                            "closed predicate projection cannot read through a reference descriptor",
                        );
                    }
                    let field = crate::value_custody::places::applied_data_member_field(
                        program, reference, member,
                    )
                    .ok_or("closed predicate projection lost its exact receiving field")?;
                    let values = match receiver {
                        DecodedCanonicalConstValue::Record { fields, .. } => fields,
                        DecodedCanonicalConstValue::Variant {
                            case_name, fields, ..
                        } => {
                            let parent = program.symbols.get(field.symbol).parent;
                            if program.symbols.get(parent).kind == symbols::SymbolKind::Variant
                                && program.symbols.name(parent) != case_name
                            {
                                return Err("closed predicate projection selects an inactive case");
                            }
                            fields
                        }
                        _ => {
                            return Err(
                                "closed predicate projection has no completed record/case contents",
                            );
                        }
                    };
                    let (_, value) = values
                        .iter()
                        .find(|(name, _)| name == field.name.as_str())
                        .ok_or("closed predicate projection has no completed field")?;
                    Ok((value, field.type_reference))
                }
                _ => Err("predicate projection is outside the closed storage fragment"),
            }
        }
    }
    pub(crate) mod constraint_validation {
        use super::open_index_expressions::validate_indexed_domain_arguments;
        use super::{TypeParameterScope, TypeReferenceOwner, type_reference_label};

        use diagnostics::Diagnostic;
        use language_semantics::value_domain::ValueDomain;
        use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

        fn domain_is_declared(
            program: &dyn crate::CheckingInput,
            base_type: TypeReferenceHandle,
            constraint: &typed_trees::types::DomainConstraint,
        ) -> bool {
            constraint.symbol.is_valid()
                && program.domain_definitions().iter().any(|domain| {
                    domain.symbol == constraint.symbol
                        && domain.classification == constraint.classification
                        && domain.predicate_presence == constraint.predicate_presence
                        && domain.semantic_roles.denotation_dimension.is_some()
                            == constraint.semantic_roles.denotation_dimension.is_some()
                        && domain.semantic_roles.arithmetic_policy.is_some()
                            == constraint.semantic_roles.arithmetic_policy.is_some()
                        && domain.establishment_routes == constraint.establishment_routes
                        && constraint.arguments.len()
                            == typed_trees::domain::index_parameters(program, domain).len()
                        && typed_trees::domain::domain_accepts_carrier(
                            program,
                            domain,
                            base_type,
                            &constraint.arguments,
                        )
                })
        }

        pub(crate) fn validate_type_constraints_node(
            program: &dyn crate::CheckingInput,
            base_type: TypeReferenceHandle,
            constraints: arena::HandleSpan<TypeConstraintNode>,
            diagnostics: &mut Vec<Diagnostic>,
            owner: TypeReferenceOwner<'_>,
            type_parameter_scope: TypeParameterScope<'_>,
        ) {
            let primitive_type = program.type_reference_table.primitive_type(base_type);
            let constraints = program.type_reference_table.constraints(constraints);
            validate_semantic_role_composition(constraints, diagnostics, &owner);

            for constraint in constraints {
                match constraint {
                    TypeConstraintNode::Named(name) if name.as_str() == "finite" => {
                        let Some(primitive_type) = primitive_type else {
                            continue;
                        };

                        if !primitive_type.accepts_finite_constraint() {
                            diagnostics.push(Diagnostic::error(format!(
                        "{owner} uses `finite` on `{}`, but `finite` is only valid on floats",
                        primitive_type.name()
                    )));
                        }
                    }
                    TypeConstraintNode::Named(_) => {}
                    TypeConstraintNode::Domain(name)
                        if language_semantics::CarryPermission::from_name(name.as_str())
                            .is_some() => {}
                    TypeConstraintNode::Domain(name) if name.as_str().starts_with("Carry::") => {
                        diagnostics.push(Diagnostic::error(format!(
                            "{owner} uses unknown compiler carry permission `{name}`; expected \
                     `Carry::AcrossSuspend`, `Carry::AnyCpu`, `Carry::AnyThread`, \
                     `Carry::MovableAddress`, or the `Carry::Portable` alias",
                        )));
                    }
                    TypeConstraintNode::Domain(name)
                        if ValueDomain::from_name(name.as_str()).is_some() =>
                    {
                        let value_domain = ValueDomain::from_name(name.as_str())
                            .expect("match guard recognized the core value domain");
                        match value_domain {
                            ValueDomain::Finite
                                if !primitive_type.is_some_and(|primitive| {
                                    primitive.accepts_finite_constraint()
                                }) =>
                            {
                                diagnostics.push(Diagnostic::error(format!(
                            "{owner} uses `in {value_domain_name}` on `{carrier}`, but `{value_domain_name}` is only valid on floats",
                            value_domain_name = value_domain.name(),
                            carrier = type_reference_label(program, base_type),
                        )));
                            }
                            ValueDomain::Finite => {}
                        }
                    }
                    TypeConstraintNode::Domain(name)
                        if typed_trees::wire::is_layout_domain_constraint(name) =>
                    {
                        validate_layout_domain_constraint(
                            program,
                            base_type,
                            name,
                            diagnostics,
                            &owner,
                        );
                    }
                    TypeConstraintNode::Domain(domain) => {
                        if !domain_is_declared(program, base_type, domain) {
                            diagnostics.push(Diagnostic::error(format!(
                        "{owner} uses `in {domain}`, but no `domain` named `{domain}` is declared \
                         for `{}` (domains are bound to their storage type)",
                        type_reference_label(program, base_type)
                    )));
                        } else {
                            validate_indexed_domain_arguments(
                                program,
                                domain,
                                type_parameter_scope,
                                diagnostics,
                                &owner,
                            );
                        }
                    }

                    TypeConstraintNode::ArithmeticDomain(domain) => {
                        use numerics::arithmetic::ArithmeticDomain;
                        if primitive_type
                            .is_some_and(|primitive| primitive.accepts_finite_constraint())
                        {
                            let primitive_name = primitive_type.expect("checked above").name();
                            match domain {
                        ArithmeticDomain::Wrapping => diagnostics.push(Diagnostic::error(format!(
                            "{owner} applies `Wrapping` to `{primitive_name}`, but there \
                                 is no modular reading of a float -- a wrapping policy is only \
                                 meaningful on integers"
                        ))),
                        ArithmeticDomain::Saturating
                        | ArithmeticDomain::Trapping
                        | ArithmeticDomain::Exact => {}
                    }
                        }
                    }
                }
            }
        }

        fn validate_semantic_role_composition(
            constraints: &[TypeConstraintNode],
            diagnostics: &mut Vec<Diagnostic>,
            owner: &TypeReferenceOwner<'_>,
        ) {
            use language_semantics::{DomainSemanticRole, SemanticDomainTable};

            for role in [
                DomainSemanticRole::DenotationDimension,
                DomainSemanticRole::ArithmeticPolicy,
            ] {
                let mut contributions = Vec::new();
                for constraint in constraints {
                    let contribution = match constraint {
                        TypeConstraintNode::Domain(domain) => domain
                            .semantic_roles
                            .contribution(role)
                            .map(|semantic_id| (semantic_id, domain.name.to_string())),
                        TypeConstraintNode::ArithmeticDomain(domain)
                            if role == DomainSemanticRole::ArithmeticPolicy =>
                        {
                            let semantic_id = match domain {
                                numerics::arithmetic::ArithmeticDomain::Exact => continue,
                                numerics::arithmetic::ArithmeticDomain::Wrapping => {
                                    SemanticDomainTable::WRAPPING
                                }
                                numerics::arithmetic::ArithmeticDomain::Saturating => {
                                    SemanticDomainTable::SATURATING
                                }
                                numerics::arithmetic::ArithmeticDomain::Trapping => {
                                    SemanticDomainTable::TRAPPING
                                }
                            };
                            Some((semantic_id, domain.name().to_owned()))
                        }
                        _ => None,
                    };
                    let Some((semantic_id, label)) = contribution else {
                        continue;
                    };
                    if contributions.iter().all(
                        |(prior, _): &(language_semantics::SemanticDomainId, String)| {
                            *prior != semantic_id
                        },
                    ) {
                        contributions.push((semantic_id, label));
                    }
                }
                if contributions.len() <= 1 {
                    continue;
                }
                let rendered = contributions
                    .iter()
                    .map(|(_, label)| format!("`{label}`"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let rule = match role {
                    DomainSemanticRole::ArithmeticPolicy => {
                        "a domain chain may carry at most one policy domain"
                    }
                    DomainSemanticRole::DenotationDimension => {
                        "a domain chain may carry at most one denotation/dimension domain"
                    }
                };
                diagnostics.push(Diagnostic::error(format!(
            "{owner} declares conflicting `{}` semantic-role contributions {rendered}; {rule}",
            role.as_str(),
        )));
            }
        }

        fn validate_layout_domain_constraint(
            program: &dyn crate::CheckingInput,
            base_type: TypeReferenceHandle,
            domain: &typed_trees::types::DomainConstraint,
            diagnostics: &mut Vec<Diagnostic>,
            owner: &TypeReferenceOwner<'_>,
        ) {
            let Some((schema_name, grammar)) =
                typed_trees::wire::layout_domain_constraint_arguments(program, domain)
            else {
                return;
            };
            match program.type_reference_table.type_reference(base_type) {
                TypeReferenceNode::FixedArray { .. } => {
                    diagnostics.push(Diagnostic::error(format!(
                "{owner} declares `in OmegaLayout<{schema_name}>` on owned stored bytes; a layout domain is a \
                 fact about bytes ESTABLISHED by validate/encode, never declared on storage \
                 (a zeroed buffer holds no valid encoding). Hold the value (`{schema_name}`) \
                 and encode at the edge, or carry the fact on a borrowed view \
                 (`&[u8] in OmegaLayout<{schema_name}>`)"
            )));
                    return;
                }
                TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. } => {}
                _ => {
                    diagnostics.push(Diagnostic::error(format!(
                "{owner} uses `in OmegaLayout<{schema_name}>` on `{}`, but a layout domain lives on a borrowed \
                 byte view (`&[u8] in OmegaLayout<{schema_name}>`)",
                type_reference_label(program, base_type)
            )));
                    return;
                }
            }
            if let Some(grammar) = grammar {
                diagnostics.push(Diagnostic::error(format!(
            "{owner} names the `{grammar}` grammar explicitly, but `Derived` is the default \
             and the only implemented grammar of `OmegaLayout` (an explicit `Packed` \
             parameter is not implemented yet)"
        )));
                return;
            }
            if !program
                .wire_schemas()
                .iter()
                .any(|schema| schema.name.as_str() == schema_name.as_str())
            {
                let is_plain_data = program
                    .data_definitions()
                    .iter()
                    .any(|data| data.name.as_str() == schema_name.as_str());
                if is_plain_data {
                    diagnostics.push(Diagnostic::error(format!(
                "{owner} uses `in OmegaLayout<{schema_name}>`, but data `{schema_name}` has \
                 no identity numbers; the packed grammar of an unnumbered schema is not \
                 implemented yet -- number the fields (`1: name: type;`) for the derived \
                 tagged grammar"
            )));
                } else {
                    diagnostics.push(Diagnostic::error(format!(
                "{owner} uses `in OmegaLayout<{schema_name}>`, but no data definition named \
                 `{schema_name}` exists"
            )));
                }
            }
        }
    }
    pub(crate) mod formation {
        use super::*;
        use crate::proof_contracts::arithmetic_domains::LocalValueFormation;
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::signature::SignatureContractKind;

        impl<'program> TypeReferenceOwner<'program> {
            pub(crate) fn local_value_formation(self) -> Option<LocalValueFormation<'program>> {
                match self {
                    Self::Expression {
                        machine,
                        current_state,
                        value_environment,
                        ..
                    }
                    | Self::StateLocalData {
                        machine,
                        current_state,
                        value_environment,
                        ..
                    } => Some(LocalValueFormation {
                        machine,
                        state: current_state,
                        environment: value_environment,
                    }),
                    _ => None,
                }
            }

            pub(crate) fn value_formation_facts(
                self,
                program: &dyn crate::CheckingInput,
            ) -> Vec<ExpressionHandle> {
                let signature = match self {
                    Self::StateParameter { signature, .. }
                    | Self::StateReturn { signature, .. } => signature,
                    _ => return Vec::new(),
                };
                let requires = SignatureContractKind::Requires;
                if let Some(machine) = program.machine_holding_state(signature)
                    && let Some(state) = program.state_in_machine(machine, signature)
                {
                    let mut facts = program
                        .contract_expressions(
                            program.state_contracts(state).iter(),
                            requires.clone(),
                        )
                        .collect::<Vec<_>>();
                    if program
                        .machine_states(machine)
                        .first()
                        .is_some_and(|entry| entry.symbol == signature)
                    {
                        facts.extend(program.contract_expressions(
                            program.machine_contracts(machine).iter(),
                            requires,
                        ));
                    }
                    return facts;
                }
                let signature = program
                    .trait_machine_signature(signature)
                    .map(|(_, signature)| signature)
                    .or_else(|| {
                        program
                            .retained_static_machine_contract(signature)
                            .map(|contract| contract.signature())
                    });
                signature.map_or_else(Vec::new, |signature| {
                    program
                        .contract_expressions(
                            program.state_signature_contracts(signature).iter(),
                            requires,
                        )
                        .collect()
                })
            }
        }

        pub(crate) fn validate_expression_type_application(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            reference: TypeReferenceHandle,
            symbols: &TopLevelSymbols<'_>,
            environment: &crate::proof_contracts::arithmetic_domains::ValueEnvironment,
            diagnostics: &mut Vec<Diagnostic>,
            obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
        ) {
            if !reference.is_valid() {
                return;
            }
            let (parameters, lifetimes) = program
                .machine_type_reference_scope(machine, |left, right| left.symbol == right.symbol);
            validate_type_reference_handle(
                program,
                reference,
                symbols,
                diagnostics,
                TypeReferenceOwner::Expression {
                    machine,
                    current_state: Some(state),
                    value_environment: environment,
                    generic_depth: 0,
                },
                TypeParameterScope::authored(machine.symbol, &parameters, &lifetimes),
                obligations,
            );
        }
    }
    pub(crate) mod generic_arguments {
        use super::{
            TopLevelSymbols, TypeParameterScope, TypeReferenceOwner, type_reference_label,
        };
        use crate::CheckingInput;
        use diagnostics::Diagnostic;
        use language_semantics::const_value::{
            CanonicalConstIdentity, CanonicalConstValue, DecodedCanonicalConstValue,
        };
        use std::collections::HashSet;
        use symbols::SymbolHandle;
        use typed_trees::data::DataDefinition;
        use typed_trees::data::DataShapeKind;
        use typed_trees::data::{
            DataField, DataMember, MachineParameterContract, TypeParameter, TypeParameterKind,
        };
        use typed_trees::expression::StaticMachineArgument;
        use typed_trees::signature::StateParameter;
        use typed_trees::type_identity::IndexIdentityArgument;
        use typed_trees::type_identity::NormalizedTypeIdentity;
        use typed_trees::type_identity::TypeIdentityRequest;
        use typed_trees::types::{
            FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
        };

        pub(crate) mod closed_expressions {
            use super::{
                AppliedValueCarrier, CanonicalConstValue, DataMember, DecodedCanonicalConstValue,
                PrimitiveType, SymbolHandle, TypeReferenceHandle, TypeReferenceNode,
                canonical_carrier_label, exact_const_primitive,
                exact_structured_const_carrier_is_eligible, validate_decoded_structured_const,
            };
            use crate::frozen_program::expression_table;
            use typed_trees::expression::{ExpressionHandle, ExpressionNode};

            pub fn normalize_closed_const_expression(
                program: &dyn crate::CheckingInput,
                owner: SymbolHandle,
                expression: ExpressionHandle,
                carrier: TypeReferenceHandle,
            ) -> Result<Option<CanonicalConstValue>, String> {
                normalize_closed_const_expression_in_carrier(
                    program,
                    owner,
                    expression,
                    AppliedValueCarrier::ordinary(carrier),
                )
            }

            pub(crate) fn normalize_closed_const_expression_in_carrier(
                program: &dyn crate::CheckingInput,
                owner: SymbolHandle,
                expression: ExpressionHandle,
                carrier: AppliedValueCarrier<'_>,
            ) -> Result<Option<CanonicalConstValue>, String> {
                if !exact_structured_const_carrier_is_eligible(program, carrier, &mut Vec::new()) {
                    return Ok(None);
                }
                let mut context = Context {
                    program,
                    owner,
                    active: Vec::new(),
                    nodes: 0,
                };
                let Some(value) = context.value(expression, carrier)? else {
                    return Ok(None);
                };
                let mut canonical = CanonicalConstValue::from_decoded(
                    &value,
                    expression_table(program).display_name(expression),
                )
                .ok_or_else(|| {
                    "completed const recipe exceeds canonical value encoding limits".to_owned()
                })?;
                if !matches!(
                    value,
                    DecodedCanonicalConstValue::Integer { .. }
                        | DecodedCanonicalConstValue::Boolean(_)
                ) {
                    canonical.type_name = carrier.identity_text(program)?;
                }
                Ok(Some(canonical))
            }

            pub(crate) struct Context<'program> {
                program: &'program dyn crate::CheckingInput,
                owner: SymbolHandle,
                active: Vec<ExpressionHandle>,
                nodes: usize,
            }

            impl Context<'_> {
                fn value(
                    &mut self,
                    expression: ExpressionHandle,
                    carrier: AppliedValueCarrier<'_>,
                ) -> Result<Option<DecodedCanonicalConstValue>, String> {
                    if !expression_table(self.program).expression_is_valid(expression)
                        || self.active.contains(&expression)
                    {
                        return Err(
                            "const recipe has an invalid or cyclic expression edge".to_owned()
                        );
                    }
                    if self.active.len() >= CanonicalConstValue::MAXIMUM_DEPTH
                        || self.nodes >= CanonicalConstValue::MAXIMUM_NODES
                    {
                        return Err("const recipe exceeds canonical structural limits".to_owned());
                    }
                    self.nodes += 1;
                    self.active.push(expression);
                    let result = self.node(expression, carrier);
                    self.active.pop();
                    let value = result?;
                    if let Some(value) = &value {
                        validate_decoded_structured_const(
                            self.program,
                            carrier,
                            value,
                            &mut Vec::new(),
                        )?;
                    }
                    Ok(value)
                }

                fn node(
                    &mut self,
                    expression: ExpressionHandle,
                    carrier: AppliedValueCarrier<'_>,
                ) -> Result<Option<DecodedCanonicalConstValue>, String> {
                    let program = self.program;
                    let carrier = carrier.resolved(program);
                    let expected = carrier.reference;
                    let node = expression_table(program).expression(expression);
                    if let ExpressionNode::Boolean(value) = node {
                        return Ok(Some(DecodedCanonicalConstValue::Boolean(*value)));
                    }
                    if let ExpressionNode::Name(path) = node {
                        if program
                            .data_type_parameter_by_symbol(path.symbol)
                            .is_some_and(|parameter| parameter.kind.is_const_or_value())
                        {
                            return Ok(None);
                        }
                        if let Some(declaration) = program.const_declaration_by_symbol(path.symbol)
                        {
                            if !carrier.matches(program, declaration.declared_type) {
                                return Err(
                                    "named const recipe leaf has a different declared carrier"
                                        .to_owned(),
                                );
                            }
                            let encoding = declaration
                                .evaluated_value
                                .as_ref()
                                .and_then(|value| value.encoding())
                                .ok_or("named const recipe leaf has no canonical value")?;
                            return CanonicalConstValue::new("", encoding, "")
                                .decode_encoding()
                                .map(Some)
                                .ok_or_else(|| {
                                    "named const recipe leaf is not a canonical index".to_owned()
                                });
                        }
                    }
                    if let ExpressionNode::Member(member) = node {
                        let receiver = crate::parameter_expression_result_type_reference(
                            program,
                            self.owner,
                            &[],
                            member.receiver,
                        )
                        .ok_or("const field projection has no independently typed receiver")?;
                        let Some(value) =
                            self.value(member.receiver, AppliedValueCarrier::ordinary(receiver))?
                        else {
                            return Ok(None);
                        };
                        let field = crate::value_custody::places::applied_data_member_field(
                            program, receiver, member,
                        )
                        .ok_or("const field projection lost its exact declared field")?;
                        if !carrier.matches(program, field.type_reference) {
                            return Err("const field projection has a different declared carrier"
                                .to_owned());
                        }
                        let fields = match value {
                            DecodedCanonicalConstValue::Record { fields, .. }
                            | DecodedCanonicalConstValue::Variant { fields, .. } => fields,
                            _ => {
                                return Err(
                        "const field projection did not produce its declared record/case"
                            .to_owned(),
                    );
                            }
                        };
                        return fields
                            .into_iter()
                            .find(|(name, _)| name == field.name.as_str())
                            .map(|(_, value)| Some(value))
                            .ok_or_else(|| {
                                "const field projection lost its canonical field value".to_owned()
                            });
                    }
                    if let Some(primitive) = exact_const_primitive(program, expected) {
                        if primitive == PrimitiveType::Bool {
                            return Ok(super::super::const_predicates::closed_boolean(
                                program, self.owner, expression,
                            )
                            .map(DecodedCanonicalConstValue::Boolean));
                        }
                        if primitive.accepts_integer_literal() {
                            let Some(value) =
                                program.closed_integer_value_in(expression, self.owner)
                            else {
                                return Ok(None);
                            };
                            if value.primitive.is_some_and(|actual| actual != primitive)
                                || value
                                    .type_reference
                                    .is_some_and(|actual| !carrier.matches(program, actual))
                            {
                                return Err(
                        "const numeric recipe changes an already selected carrier or policy"
                            .to_owned(),
                    );
                            }
                            let value = value.value.to_string().parse::<i128>().map_err(
                    |_| "const numeric recipe exceeds the exact canonical integer carrier",
                )?;
                            return Ok(Some(DecodedCanonicalConstValue::Integer {
                                type_name: primitive.name().to_owned(),
                                value,
                            }));
                        }
                    }
                    match (program.type_reference_table.type_reference(expected), node) {
                        (
                            TypeReferenceNode::FixedArray {
                                element_type,
                                length,
                            },
                            ExpressionNode::ArrayLiteral(elements),
                        ) => {
                            let length = carrier
                                .array_length(program, length)
                                .ok_or("const array carrier extent is not closed")?;
                            let elements = expression_table(program).expression_handles(*elements);
                            if elements.len() != length {
                                return Err(
                                    "const array recipe has the wrong element count".to_owned()
                                );
                            }
                            let mut values = Vec::with_capacity(length);
                            for element in elements {
                                let Some(value) =
                                    self.value(*element, carrier.child(*element_type))?
                                else {
                                    return Ok(None);
                                };
                                values.push(value);
                            }
                            Ok(Some(DecodedCanonicalConstValue::Array {
                                type_name: canonical_carrier_label(program, carrier),
                                values,
                            }))
                        }
                        (
                            TypeReferenceNode::Named { symbol, .. },
                            ExpressionNode::StructLiteral(literal),
                        ) => {
                            if *symbol != literal.type_symbol {
                                return Err(
                                    "const constructor names another nominal carrier".to_owned()
                                );
                            }
                            let definition = program
                                .data_definition_by_symbol(*symbol)
                                .ok_or("const constructor lost its data owner")?;
                            let (case, fields) = if let Some(case) = literal.case_symbol {
                                let case = program
                                    .data_members(definition)
                                    .iter()
                                    .filter_map(DataMember::as_variant)
                                    .find(|variant| variant.symbol == case)
                                    .ok_or("const constructor lost its exact case owner")?;
                                (
                                    Some(case),
                                    program.data_payload_fields(case).iter().collect::<Vec<_>>(),
                                )
                            } else {
                                if literal.case_name.is_some() {
                                    return Err(
                                        "const constructor has an unbound case spelling".to_owned()
                                    );
                                }
                                (
                                    None,
                                    program
                                        .data_members(definition)
                                        .iter()
                                        .filter_map(DataMember::as_field)
                                        .collect::<Vec<_>>(),
                                )
                            };
                            let supplied = expression_table(program).struct_fields(literal.fields);
                            if fields.len() != supplied.len() {
                                return Err(
                                    "const constructor has the wrong field count".to_owned()
                                );
                            }
                            let mut values = Vec::with_capacity(fields.len());
                            for field in fields {
                                let mut selected = supplied
                                    .iter()
                                    .filter(|value| value.field_symbol == field.symbol);
                                let value = selected.next().ok_or(
                                    "const constructor is missing an exact declared field",
                                )?;
                                if selected.next().is_some() {
                                    return Err(
                                        "const constructor repeats a declared field".to_owned()
                                    );
                                }
                                let field_type =
                        crate::value_custody::struct_literals::selected_construction_field_type(
                            program,
                            expression,
                            literal,
                            field.symbol,
                        )
                        .ok_or("const constructor field lost its selected carrier")?;
                                let Some(value) =
                                    self.value(value.value, carrier.child(field_type))?
                                else {
                                    return Ok(None);
                                };
                                values.push((field.name.as_str().to_owned(), value));
                            }
                            Ok(Some(if let Some(case) = case {
                                DecodedCanonicalConstValue::Variant {
                                    type_name: definition.name.as_str().to_owned(),
                                    case_name: case.name.as_str().to_owned(),
                                    fields: values,
                                }
                            } else {
                                DecodedCanonicalConstValue::Record {
                                    type_name: definition.name.as_str().to_owned(),
                                    fields: values,
                                }
                            }))
                        }
                        _ => Ok(None),
                    }
                }
            }
        }
        pub use closed_expressions::normalize_closed_const_expression;
        pub(crate) use closed_expressions::normalize_closed_const_expression_in_carrier;

        pub fn check_closed_const_expression(
            program: &dyn CheckingInput,
            lexical_owner: SymbolHandle,
            expression: typed_trees::expression::ExpressionHandle,
            carrier: TypeReferenceHandle,
        ) -> Result<Option<CanonicalConstValue>, Vec<Diagnostic>> {
            check_closed_const_expression_in_carrier(
                program,
                lexical_owner,
                expression,
                AppliedValueCarrier::ordinary(carrier),
            )
        }

        pub(crate) fn check_closed_const_expression_in_carrier(
            program: &dyn CheckingInput,
            lexical_owner: SymbolHandle,
            expression: typed_trees::expression::ExpressionHandle,
            carrier: AppliedValueCarrier<'_>,
        ) -> Result<Option<CanonicalConstValue>, Vec<Diagnostic>> {
            let resolved = carrier.resolved(program);
            let mut diagnostics = Vec::new();
            let mut formation_obligations = Vec::new();
            crate::proof_contracts::arithmetic_domains::validate_declaration_value_range(
                program,
                lexical_owner,
                &declaration_parameters(program, lexical_owner),
                &[],
                None,
                expression,
                resolved.reference,
                "closed const argument",
                &mut diagnostics,
                &mut formation_obligations,
            );
            if diagnostics.iter().any(Diagnostic::is_error) {
                return Err(diagnostics);
            }
            if !formation_obligations.is_empty() {
                return Ok(None);
            }
            normalize_closed_const_expression_in_carrier(
                program,
                lexical_owner,
                expression,
                carrier,
            )
            .map_err(|message| vec![Diagnostic::error(message)])
        }

        pub(crate) fn validate_machine_data_argument(
            program: &dyn CheckingInput,
            base_symbol: SymbolHandle,
            base_name: &str,
            parameter: &TypeParameter,
            contract: &MachineParameterContract,
            argument: TypeReferenceHandle,
            arguments: &[TypeReferenceHandle],
            _type_parameter_scope: TypeParameterScope<'_>,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let TypeReferenceNode::Named { symbol, name } =
                program.type_reference_table.type_reference(argument)
            else {
                diagnostics.push(Diagnostic::error(format!(
            "machine parameter `{}` of proof data `{base_name}` requires a static machine symbol argument",
            parameter.name
        )));
                return;
            };

            if *symbol == parameter.symbol {
                return;
            }

            let parameters = program
                .data_definitions()
                .iter()
                .find(|definition| {
                    definition.symbol == base_symbol
                        || (!program.symbols.has_source_metadata()
                            && definition.name.as_str() == base_name)
                })
                .map(|definition| program.data_type_parameters(definition))
                .or_else(|| {
                    program
                        .trait_definition_by_symbol(base_symbol)
                        .map(|definition| program.trait_type_parameters(definition))
                })
                .or_else(|| {
                    program
                        .type_aliases()
                        .iter()
                        .find(|alias| alias.symbol == base_symbol)
                        .map(|alias| {
                            program
                                .tables
                                .data_type_parameters
                                .span_or_empty(alias.type_parameters)
                        })
                })
                .unwrap_or(&[]);
            if parameters.len() != arguments.len() {
                diagnostics.push(Diagnostic::error(format!(
                    "machine parameter `{}` of `{base_name}` lost its complete applied telescope",
                    parameter.name,
                )));
                return;
            }
            let generic_types = parameters
                .iter()
                .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
                .collect::<Vec<_>>();
            let substitutions = parameters
                .iter()
                .zip(arguments)
                .map(|(parameter, argument)| (parameter.symbol, *argument))
                .collect::<Vec<_>>();
            crate::machine_calls::machine_parameters::validate_data_machine_selection(
                program,
                base_name,
                parameter,
                contract,
                *symbol,
                name.as_str(),
                &generic_types,
                &substitutions,
                diagnostics,
            );
        }

        pub(crate) fn machine_argument_name<'program>(
            program: &'program dyn CheckingInput,
            argument: TypeReferenceHandle,
            type_parameter_scope: TypeParameterScope<'program>,
        ) -> Option<&'program str> {
            let TypeReferenceNode::Named { symbol, name } =
                program.type_reference_table.type_reference(argument)
            else {
                return None;
            };
            if type_parameter_scope
                .machine_parameter(*symbol, name.as_str())
                .is_some()
                || program.machines().iter().any(|machine| {
                    machine.symbol == *symbol
                        || (!program.symbols.has_source_metadata()
                            && machine.name.as_str() == name.as_str())
                })
            {
                Some(name.as_str())
            } else {
                None
            }
        }

        pub(crate) fn validate_symbolic_array_length(
            program: &dyn CheckingInput,
            length: &FixedArrayLength,
            type_parameter_scope: TypeParameterScope<'_>,
            diagnostics: &mut Vec<Diagnostic>,
            owner: TypeReferenceOwner<'_>,
            formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
        ) {
            if let FixedArrayLength::Expression(value) = length {
                let TypeReferenceNode::ConstExpression(root) =
                    program.type_reference_table.type_reference(*value)
                else {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} has an unclosed array extent without its expression recipe"
                    )));
                    return;
                };
                let parameters = type_parameter_scope
                    .type_parameters
                    .iter()
                    .filter_map(|parameter| {
                        let type_reference = match parameter.kind {
                            TypeParameterKind::Const { type_reference }
                            | TypeParameterKind::Value { type_reference } => type_reference,
                            _ => return None,
                        };
                        Some(StateParameter {
                            symbol: parameter.symbol,
                            name: parameter.name.clone(),
                            type_reference,
                            is_const: matches!(parameter.kind, TypeParameterKind::Const { .. }),
                            ..Default::default()
                        })
                    })
                    .collect::<Vec<_>>();
                let prior = program
                    .data_definition_by_symbol(type_parameter_scope.lexical_owner)
                    .map(|definition| {
                        program
                            .proof_facts
                            .span_or_empty(definition.requires_facts)
                            .iter()
                            .filter_map(typed_trees::domain::ProofFact::as_expression)
                            .copied()
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                crate::proof_contracts::contract_entailment::specification_calls::validate_declaration_expression_formation(
            program,
            type_parameter_scope.lexical_owner,
            &parameters,
            *root,
            &prior,
            &owner.to_string(),
            diagnostics,
            formation_obligations,
        );
                let natural = crate::parameter_expression_result_type_reference(
                    program,
                    type_parameter_scope.lexical_owner,
                    &[],
                    *root,
                )
                .and_then(|result| program.primitive_type_reference(result));
                let integer = natural.is_some_and(|primitive| primitive.accepts_integer_literal());
                let anonymous = crate::has_anonymous_numeric_results(program, *root)
                    && crate::evaluate_anonymous_numeric_expression(
                        program,
                        *root,
                        &[],
                        |expression| {
                            match program.expression_table.expression(expression) {
                    typed_trees::expression::ExpressionNode::Binary(binary) => {
                        typed_trees::operator::binary_operator_spelling(binary.operator).is_none_or(
                            |spelling| {
                                typed_trees::operator::has_builtin_spelled_expression_meaning(
                                    program,
                                    type_parameter_scope.lexical_owner,
                                    expression,
                                    spelling,
                                    &[None, None],
                                )
                            },
                        )
                    }
                    typed_trees::expression::ExpressionNode::Unary(_) => true,
                    _ => false,
                }
                        },
                    )
                    .and_then(|value| value.to_integer_exact())
                    .and_then(|value| value.to_u64())
                    .is_some();
                if !integer && !anonymous {
                    diagnostics.push(Diagnostic::error(format!("{owner} array extent expression requires a well-typed integer result or exact nonnegative anonymous integer"))
                .with_source_span(program.expression_table.source_span(*root)));
                }
                return;
            }
            let FixedArrayLength::ConstParameter { symbol, name } = length else {
                return;
            };
            let parameter = type_parameter_scope
                .type_parameters
                .iter()
                .find(|parameter| symbol.is_valid() && parameter.symbol == *symbol);
            let Some(parameter) = parameter else {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses unresolved fixed-array length `{name}`; a symbolic array length \
             must name an in-scope `const` parameter"
                )));
                return;
            };
            let (TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference }) = parameter.kind
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses type parameter `{name}` as a fixed-array length; array lengths \
             must name a `const` parameter"
                )));
                return;
            };
            let Some(primitive) = program.type_reference_table.primitive_type(type_reference)
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses const parameter `{name}` with a non-primitive type as a \
             fixed-array length; array lengths require an integer primitive"
                )));
                return;
            };
            if !primitive.accepts_integer_literal() {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses const parameter `{name}` of non-integer type `{}` as a \
             fixed-array length",
                    primitive.name(),
                )));
            }
        }

        pub fn validate_closed_const_argument(
            program: &dyn CheckingInput,
            owner: &str,
            parameter: &TypeParameter,
            argument: TypeReferenceHandle,
            application_parameters: &[TypeParameter],
            application_arguments: &[TypeReferenceHandle],
        ) -> Result<(), Vec<Diagnostic>> {
            let (TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference }) = parameter.kind
            else {
                return Err(vec![Diagnostic::error(format!(
                    "parameter `{}` of `{owner}` is not a const parameter",
                    parameter.name,
                ))]);
            };
            if !argument.is_valid()
                || !matches!(
                    program.type_reference_table.type_reference(argument),
                    TypeReferenceNode::Named { symbol, .. }
                        if !symbol.is_valid() || program.const_declaration_by_symbol(*symbol).is_some()
                )
            {
                return Err(vec![Diagnostic::error(format!(
                    "const parameter `{}` of `{owner}` requires a normalized closed value",
                    parameter.name,
                ))]);
            }
            let mut diagnostics = Vec::new();
            let mut formation_obligations = Vec::new();
            validate_const_data_argument(
                program,
                owner,
                parameter,
                AppliedValueCarrier::applied(
                    type_reference,
                    application_parameters,
                    application_arguments,
                ),
                argument,
                TypeParameterScope::authored(symbols::SymbolHandle::invalid(), &[], &[]),
                &[],
                None,
                &mut diagnostics,
                &mut formation_obligations,
            );
            if !formation_obligations.is_empty() {
                diagnostics.push(Diagnostic::error(format!(
                    "const parameter `{}` of `{owner}` has unresolved formation obligations",
                    parameter.name,
                )));
            }
            crate::program_validation::finish_strict_diagnostics(diagnostics)
        }

        pub(crate) fn const_argument_matches_parameter(
            program: &dyn CheckingInput,
            parameter: &TypeParameter,
            argument: TypeReferenceHandle,
            caller_parameters: &[TypeParameter],
        ) -> bool {
            let TypeParameterKind::Const { type_reference } = parameter.kind else {
                return false;
            };
            if let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(argument)
                && caller_parameters.iter().any(|candidate| {
                    candidate.symbol == *symbol
                        && matches!(candidate.kind, TypeParameterKind::Value { .. })
                })
            {
                return false;
            }
            let mut diagnostics = Vec::new();
            let mut formation_obligations = Vec::new();
            validate_const_data_argument(
                program,
                "call",
                parameter,
                AppliedValueCarrier::ordinary(type_reference),
                argument,
                TypeParameterScope::authored(
                    symbols::SymbolHandle::invalid(),
                    caller_parameters,
                    &[],
                ),
                &[],
                None,
                &mut diagnostics,
                &mut formation_obligations,
            );
            formation_obligations.is_empty() && !diagnostics.iter().any(Diagnostic::is_error)
        }

        #[derive(Clone, Copy)]
        pub(crate) struct AppliedValueCarrier<'application> {
            reference: TypeReferenceHandle,
            parameters: &'application [TypeParameter],
            arguments: &'application [TypeReferenceHandle],
            static_arguments: Option<&'application [StaticMachineArgument]>,
            values: &'application [(SymbolHandle, &'application CanonicalConstIdentity)],
        }

        impl<'application> AppliedValueCarrier<'application> {
            pub(crate) fn ordinary(reference: TypeReferenceHandle) -> Self {
                Self {
                    reference,
                    parameters: &[],
                    arguments: &[],
                    static_arguments: None,
                    values: &[],
                }
            }

            pub(crate) fn applied(
                reference: TypeReferenceHandle,
                parameters: &'application [TypeParameter],
                arguments: &'application [TypeReferenceHandle],
            ) -> Self {
                Self {
                    reference,
                    parameters,
                    arguments,
                    static_arguments: None,
                    values: &[],
                }
            }

            pub(crate) fn applied_operands(
                reference: TypeReferenceHandle,
                parameters: &'application [TypeParameter],
                arguments: &'application [StaticMachineArgument],
                values: &'application [(SymbolHandle, &'application CanonicalConstIdentity)],
            ) -> Self {
                Self {
                    reference,
                    parameters,
                    arguments: &[],
                    static_arguments: Some(arguments),
                    values,
                }
            }

            fn type_argument(
                self,
                program: &dyn CheckingInput,
                position: usize,
            ) -> Option<TypeReferenceHandle> {
                if let Some(arguments) = self.static_arguments {
                    let argument = arguments.get(position)?;
                    if argument.type_reference.is_valid() {
                        return Some(argument.type_reference);
                    }
                    return argument
                        .symbol
                        .is_valid()
                        .then(|| {
                            program
                                .type_reference_table
                                .find_named_type_reference(argument.symbol)
                        })
                        .flatten();
                }
                self.arguments
                    .get(position)
                    .copied()
                    .filter(|argument| argument.is_valid())
            }

            pub(crate) fn resolved(self, program: &dyn CheckingInput) -> Self {
                if !self.reference.is_valid() {
                    return self;
                }
                let TypeReferenceNode::Named { symbol, .. } =
                    program.type_reference_table.type_reference(self.reference)
                else {
                    return self;
                };
                self.parameters
                    .iter()
                    .enumerate()
                    .find_map(|(position, parameter)| {
                        (symbol.is_valid()
                            && parameter.symbol == *symbol
                            && matches!(parameter.kind, TypeParameterKind::Type))
                        .then(|| self.type_argument(program, position).map(Self::ordinary))
                        .flatten()
                    })
                    .unwrap_or(self)
            }

            pub(crate) fn reference(self) -> TypeReferenceHandle {
                self.reference
            }

            fn child(self, reference: TypeReferenceHandle) -> Self {
                Self { reference, ..self }
            }

            pub(crate) fn matches(
                self,
                program: &dyn CheckingInput,
                actual: TypeReferenceHandle,
            ) -> bool {
                self.identity(program)
                    .is_some_and(|expected| expected == program.normalized_type_identity(actual))
            }

            pub(crate) fn identity(
                self,
                program: &dyn CheckingInput,
            ) -> Option<NormalizedTypeIdentity> {
                if let Some(arguments) = self.static_arguments {
                    let mut values = Vec::new();
                    for (position, (parameter, argument)) in
                        self.parameters.iter().zip(arguments).enumerate()
                    {
                        match parameter.kind {
                            TypeParameterKind::Type => {
                                let actual = self.type_argument(program, position)?;
                                values.push((
                                    parameter.symbol,
                                    IndexIdentityArgument::TypeArgument(actual),
                                ));
                            }
                            TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
                                values.push((
                                    parameter.symbol,
                                    IndexIdentityArgument::StaticArgument(argument),
                                ));
                            }
                            _ => {}
                        }
                    }
                    return program.scoped_type_identity_with_index_arguments(
                        TypeIdentityRequest::ordinary(self.reference),
                        &[],
                        &values,
                    );
                }
                let substitutions = self
                    .parameters
                    .iter()
                    .zip(self.arguments)
                    .filter(|(parameter, _)| {
                        matches!(
                            parameter.kind,
                            TypeParameterKind::Type
                                | TypeParameterKind::Const { .. }
                                | TypeParameterKind::Value { .. }
                        )
                    })
                    .map(|(parameter, argument)| (parameter.symbol, *argument))
                    .collect::<Vec<_>>();
                let expected =
                    program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
                        substitutions: &substitutions,
                        ..TypeIdentityRequest::ordinary(self.reference)
                    });
                expected.is_valid().then_some(expected)
            }

            fn identity_text(self, program: &dyn CheckingInput) -> Result<String, String> {
                self.identity(program)
                    .and_then(|identity| program.type_identity_text(identity))
                    .ok_or_else(|| "const carrier lost its complete applied identity".to_owned())
            }

            fn array_length(
                self,
                program: &dyn CheckingInput,
                length: &FixedArrayLength,
            ) -> Option<usize> {
                match length {
                    FixedArrayLength::Literal(length) => Some(*length),
                    FixedArrayLength::ConstParameter { symbol, .. } => {
                        if self.static_arguments.is_some() {
                            let (_, value) = self
                                .values
                                .iter()
                                .find(|(parameter, _)| parameter == symbol)?;
                            return match value.decode_encoding()? {
                                DecodedCanonicalConstValue::Integer { value, .. } => {
                                    usize::try_from(value).ok()
                                }
                                _ => None,
                            };
                        }
                        let argument = self.parameters.iter().zip(self.arguments).find_map(
                            |(parameter, argument)| {
                                (parameter.symbol == *symbol).then_some(*argument)
                            },
                        )?;
                        let TypeReferenceNode::Named { symbol, name } =
                            program.type_reference_table.type_reference(argument)
                        else {
                            return None;
                        };
                        if let Some(declaration) = program.const_declaration_by_symbol(*symbol) {
                            let encoding = declaration.evaluated_value.as_ref()?.encoding()?;
                            return match CanonicalConstValue::new("", encoding, "")
                                .decode_encoding()?
                            {
                                DecodedCanonicalConstValue::Integer { value, .. } => {
                                    usize::try_from(value).ok()
                                }
                                _ => None,
                            };
                        }
                        if symbol.is_valid() {
                            return None;
                        }
                        name.as_str()
                            .parse()
                            .ok()
                            .or_else(|| {
                                match CanonicalConstValue::from_atom(name.as_str())?
                                    .decode_encoding()?
                                {
                                    DecodedCanonicalConstValue::Integer { value, .. } => {
                                        usize::try_from(value).ok()
                                    }
                                    _ => None,
                                }
                            })
                    }
                    FixedArrayLength::Expression(_) | FixedArrayLength::ConstCall { .. } => {
                        program.closed_fixed_array_length(self.reference)
                    }
                }
            }
        }

        pub(crate) fn validate_const_data_argument(
            program: &dyn CheckingInput,
            base_name: &str,
            parameter: &TypeParameter,
            carrier: AppliedValueCarrier<'_>,
            argument: TypeReferenceHandle,
            type_parameter_scope: TypeParameterScope<'_>,
            prior_facts: &[typed_trees::expression::ExpressionHandle],
            local_formation: Option<
                crate::proof_contracts::arithmetic_domains::LocalValueFormation<'_>,
            >,
            diagnostics: &mut Vec<Diagnostic>,
            formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
        ) {
            let carrier = carrier.resolved(program);
            let parameter_type = carrier.reference;
            validate_const_argument_expression_formation(
                program,
                base_name,
                parameter,
                carrier,
                argument,
                type_parameter_scope,
                prior_facts,
                local_formation,
                diagnostics,
                formation_obligations,
            );
            if matches!(parameter.kind, TypeParameterKind::Value { .. })
                && let TypeReferenceNode::Named { symbol, name } =
                    program.type_reference_table.type_reference(argument)
                && symbol.is_valid()
                && (matches!(
                    program.symbols.get(*symbol).kind,
                    symbols::SymbolKind::Local | symbols::SymbolKind::Parameter
                ) || (program.symbols.get(*symbol).kind == symbols::SymbolKind::Field
                    && program.any_field_owner(*symbol).is_some_and(|(owner, _)| {
                        owner.symbol == type_parameter_scope.lexical_owner
                    })))
            {
                let actual =
                    crate::value_custody::places::declaration_type_reference(program, *symbol);
                if !actual.is_some_and(|actual| carrier.matches(program, actual)) {
                    diagnostics.push(Diagnostic::error(format!(
                "value parameter `{}` of `{base_name}` requires `{}`, but runtime subject `{name}` has a different carrier",
                parameter.name, canonical_carrier_label(program, carrier),
            )));
                }
                return;
            }
            if matches!(parameter.kind, TypeParameterKind::Value { .. })
                && let TypeReferenceNode::ConstExpression(expression) =
                    program.type_reference_table.type_reference(argument)
            {
                let parameters =
                    declaration_parameters(program, type_parameter_scope.lexical_owner);
                let actual = crate::parameter_expression_result_type_reference(
                    program,
                    type_parameter_scope.lexical_owner,
                    &parameters,
                    *expression,
                );
                let anonymous = actual.is_none() && crate::has_anonymous_numeric_results(program, *expression)
            && crate::value_custody::expression_types::argument_matches_type_reference_with_substitutions(program, *expression, parameter_type, &[]);
                if !anonymous && !actual.is_some_and(|actual| carrier.matches(program, actual)) {
                    diagnostics.push(Diagnostic::error(format!(
                "value parameter `{}` of `{base_name}` requires `{}`, but its expression has a different or unresolved carrier",
                parameter.name, canonical_carrier_label(program, carrier),
            )));
                }
                return;
            }
            let primitive = program.type_reference_table.primitive_type(parameter_type);
            let is_integer_parameter =
                primitive.is_some_and(PrimitiveType::accepts_integer_literal);
            if let TypeReferenceNode::ConstExpression(root) =
                program.type_reference_table.type_reference(argument)
            {
                if program.pending_const_range_endpoints.contains(root) {
                    return;
                }
                if matches!(parameter.kind, TypeParameterKind::Const { .. })
                    && !typed_trees::domain::index_argument_is_closed(
                        program.typed_program(),
                        argument,
                    )
                    && crate::value_custody::const_expressions::has_carrier(
                        program,
                        *root,
                        &TypeParameterKind::Const {
                            type_reference: parameter_type,
                        },
                        type_parameter_scope.lexical_owner,
                    )
                {
                    return;
                }
                diagnostics.push(Diagnostic::error(format!(
            "const argument `{}` of `{base_name}` must be known during compilation and have type `{}`",
            parameter.name,
            canonical_carrier_label(program, carrier),
        )));
                return;
            }
            let TypeReferenceNode::Named { symbol, name } =
                program.type_reference_table.type_reference(argument)
            else {
                diagnostics.push(Diagnostic::error(if is_integer_parameter {
            format!(
                "const parameter `{}` of `{base_name}` requires an integer literal argument",
                parameter.name
            )
        } else {
            format!(
                "const parameter `{}` of `{base_name}` requires a canonical const value",
                parameter.name
            )
        }));
                return;
            };

            if symbol.is_valid()
                && let Some(declaration) = program.const_declaration_by_symbol(*symbol)
            {
                let admission = if !carrier.matches(program, declaration.declared_type) {
                    Err(format!(
                        "named constant `{name}` has type `{}`, expected `{}`",
                        type_reference_label(program, declaration.declared_type),
                        canonical_carrier_label(program, carrier)
                    ))
                } else if let Some(encoding) = declaration
                    .evaluated_value
                    .as_ref()
                    .and_then(|value| value.encoding())
                {
                    validate_exact_const_value_encoding_in_carrier(program, carrier, encoding)
                } else {
                    Err(format!(
                        "named constant `{name}` has no evaluated const value"
                    ))
                };
                if let Err(reason) = admission {
                    diagnostics.push(Diagnostic::error(format!(
                "const parameter `{}` of `{base_name}` has an invalid named value: {reason}",
                parameter.name
            )));
                }
                return;
            }

            if let Some(value) = CanonicalConstValue::from_atom(name.as_str()) {
                if exact_structured_const_carrier_is_eligible(program, carrier, &mut Vec::new()) {
                    if let Err(reason) = validate_exact_const_identity_in_carrier(
                        program,
                        carrier,
                        &value.identity(),
                    ) {
                        diagnostics.push(Diagnostic::error(format!(
                    "const parameter `{}` of `{base_name}` has an invalid canonical value: {reason}",
                    parameter.name
                )));
                    }
                    return;
                }
                if let Err(reason) =
                    validate_typed_const_index_type(program, parameter_type, &mut HashSet::new())
                {
                    diagnostics.push(Diagnostic::error(format!(
                "const parameter `{}` of `{base_name}` has an ineligible index type: {reason}",
                parameter.name
            )));
                    return;
                }
                let required_type = canonical_carrier_label(program, carrier);
                if value.type_name != required_type {
                    diagnostics.push(Diagnostic::error(format!(
                "const parameter `{}` of `{base_name}` has type `{required_type}`, but its canonical value has type `{}`",
                parameter.name, value.type_name
            )));
                }
                return;
            }

            if let Ok(value) = name.as_str().parse::<i128>() {
                let Some(primitive) =
                    primitive.filter(|primitive| primitive.accepts_integer_literal())
                else {
                    diagnostics.push(Diagnostic::error(format!(
                "const parameter `{}` of `{base_name}` requires a `{}` value, not integer `{value}`",
                parameter.name,
                canonical_carrier_label(program, carrier),
            )));
                    return;
                };
                validate_const_integer_carrier(base_name, parameter, primitive, value, diagnostics);
                return;
            }

            let forwarded = type_parameter_scope
                .type_parameters
                .iter()
                .find(|candidate| {
                    if symbol.is_valid() {
                        candidate.symbol == *symbol
                    } else {
                        candidate.name.as_str() == name.as_str()
                    }
                });
            let Some(forwarded) = forwarded else {
                diagnostics.push(Diagnostic::error(if is_integer_parameter {
            format!(
                "const parameter `{}` of `{base_name}` requires an integer literal argument or an in-scope const parameter, got `{name}`",
                parameter.name,
            )
        } else {
            format!(
                "const parameter `{}` of `{base_name}` requires a canonical const value or an in-scope const parameter, got `{name}`",
                parameter.name,
            )
        }));
                return;
            };
            let (TypeParameterKind::Const {
                type_reference: forwarded_type,
            }
            | TypeParameterKind::Value {
                type_reference: forwarded_type,
            }) = forwarded.kind
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "const parameter `{}` of `{base_name}` requires a value, not a type",
                    parameter.name
                )));
                return;
            };
            if !carrier.matches(program, forwarded_type) {
                diagnostics.push(Diagnostic::error(format!(
            "const parameter `{}` of `{base_name}` has type `{}`, but forwarded const parameter `{}` has type `{}`",
            parameter.name,
            canonical_carrier_label(program, carrier),
            forwarded.name,
            type_reference_label(program, forwarded_type),
        )));
            }
        }

        pub(crate) fn validate_const_argument_expression_formation(
            program: &dyn CheckingInput,
            base_name: &str,
            parameter: &TypeParameter,
            carrier: AppliedValueCarrier<'_>,
            argument: TypeReferenceHandle,
            scope: TypeParameterScope<'_>,
            prior_facts: &[typed_trees::expression::ExpressionHandle],
            local_formation: Option<
                crate::proof_contracts::arithmetic_domains::LocalValueFormation<'_>,
            >,
            diagnostics: &mut Vec<Diagnostic>,
            formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
        ) {
            let TypeReferenceNode::ConstExpression(expression) =
                program.type_reference_table.type_reference(argument)
            else {
                return;
            };
            let carrier = carrier.resolved(program);
            let mut parameters = declaration_parameters(program, scope.lexical_owner);
            for parameter in scope.type_parameters {
                let (type_reference, is_const) = match parameter.kind {
                    TypeParameterKind::Const { type_reference } => (type_reference, true),
                    TypeParameterKind::Value { type_reference } => (type_reference, false),
                    _ => continue,
                };
                if !parameters
                    .iter()
                    .any(|known| known.symbol == parameter.symbol)
                {
                    parameters.push(StateParameter {
                        symbol: parameter.symbol,
                        name: parameter.name.clone(),
                        type_reference,
                        is_const,
                        ..Default::default()
                    });
                }
            }
            crate::proof_contracts::arithmetic_domains::validate_declaration_value_range(
                program,
                scope.lexical_owner,
                &parameters,
                prior_facts,
                local_formation,
                *expression,
                carrier.reference,
                &format!("value argument `{}` of `{base_name}`", parameter.name),
                diagnostics,
                formation_obligations,
            );
        }

        pub(crate) fn validate_exact_typed_structured_const_argument(
            program: &dyn CheckingInput,
            expected_type: TypeReferenceHandle,
            value: &CanonicalConstValue,
        ) -> Result<(), String> {
            if !exact_structured_const_data_carrier_is_eligible(program, expected_type) {
                return Err(format!(
                    "`{}` is outside the exact acyclic structured-data const cohort",
                    type_reference_label(program, expected_type)
                ));
            }
            let expected_name = program.normalized_type_identity_text(expected_type);
            let canonical_label =
                canonical_carrier_label(program, AppliedValueCarrier::ordinary(expected_type));
            if value.type_name != expected_name && value.type_name != canonical_label {
                return Err(format!(
                    "expected carrier `{expected_name}`, but the canonical atom names `{}`",
                    value.type_name
                ));
            }
            let decoded = value
                .decode_encoding()
                .ok_or_else(|| "the canonical const encoding is malformed".to_owned())?;
            validate_decoded_structured_const(
                program,
                AppliedValueCarrier::ordinary(expected_type),
                &decoded,
                &mut Vec::new(),
            )
        }

        pub(crate) fn validate_exact_const_identity(
            program: &dyn CheckingInput,
            expected_type: TypeReferenceHandle,
            identity: &CanonicalConstIdentity,
        ) -> Result<(), String> {
            validate_exact_const_identity_in_carrier(
                program,
                AppliedValueCarrier::ordinary(expected_type),
                identity,
            )
        }

        pub(crate) fn validate_exact_const_identity_in_carrier(
            program: &dyn CheckingInput,
            carrier: AppliedValueCarrier<'_>,
            identity: &CanonicalConstIdentity,
        ) -> Result<(), String> {
            let carrier = carrier.resolved(program);
            let expected_type = carrier.reference;
            let expected_name = if let Some(primitive) =
                exact_const_primitive(program, expected_type)
            {
                primitive.name().to_owned()
            } else if matches!(
                program.type_reference_table.type_reference(expected_type),
                TypeReferenceNode::FixedArray { .. }
            ) && exact_structured_const_carrier_is_eligible(
                program,
                carrier,
                &mut Vec::new(),
            ) {
                let closed = AppliedValueCarrier::ordinary(expected_type);
                if exact_structured_const_carrier_is_eligible(program, closed, &mut Vec::new()) {
                    closed.identity_text(program)?
                } else {
                    carrier.identity_text(program)?
                }
            } else {
                let value = CanonicalConstValue::new(
                    identity.type_name.clone(),
                    identity.encoding.clone(),
                    String::new(),
                );
                return validate_exact_typed_structured_const_argument(
                    program,
                    expected_type,
                    &value,
                );
            };
            if identity.type_name != expected_name {
                return Err(format!(
                    "expected carrier `{expected_name}`, but the canonical value names `{}`",
                    identity.type_name
                ));
            }
            let decoded = identity
                .decode_encoding()
                .ok_or_else(|| "the canonical const encoding is malformed".to_owned())?;
            validate_decoded_structured_const(program, carrier, &decoded, &mut Vec::new())
        }

        pub fn validate_exact_const_value_encoding(
            program: &dyn CheckingInput,
            expected_type: TypeReferenceHandle,
            encoding: &str,
        ) -> Result<(), String> {
            validate_exact_const_value_encoding_in_carrier(
                program,
                AppliedValueCarrier::ordinary(expected_type),
                encoding,
            )
        }

        pub(crate) fn validate_exact_const_value_encoding_in_carrier(
            program: &dyn CheckingInput,
            carrier: AppliedValueCarrier<'_>,
            encoding: &str,
        ) -> Result<(), String> {
            let decoded = CanonicalConstValue::new("", encoding, "")
                .decode_encoding()
                .ok_or_else(|| "the canonical const encoding is malformed".to_owned())?;
            if !exact_structured_const_carrier_is_eligible(program, carrier, &mut Vec::new()) {
                return Err(
                    "the receiving carrier is outside the exact canonical const cohort".to_owned(),
                );
            }
            validate_decoded_structured_const(program, carrier, &decoded, &mut Vec::new())
        }

        fn canonical_carrier_label(
            program: &dyn CheckingInput,
            carrier: AppliedValueCarrier<'_>,
        ) -> String {
            let carrier = carrier.resolved(program);
            let reference = carrier.reference;
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Named { symbol, .. } => {
                    crate::declarations::data_queries::data_definition_by_symbol(program, *symbol)
                        .map_or_else(
                            || type_reference_label(program, reference),
                            |definition| definition.name.as_str().to_owned(),
                        )
                }
                TypeReferenceNode::FixedArray {
                    element_type,
                    length,
                } => format!(
                    "[{}; {}]",
                    canonical_carrier_label(program, carrier.child(*element_type)),
                    carrier
                        .array_length(program, length)
                        .map_or_else(|| "unclosed".to_owned(), |length| length.to_string())
                ),
                _ => type_reference_label(program, reference),
            }
        }

        pub(crate) fn exact_const_carrier_is_eligible(
            program: &dyn CheckingInput,
            reference: TypeReferenceHandle,
        ) -> bool {
            exact_structured_const_carrier_is_eligible(
                program,
                AppliedValueCarrier::ordinary(reference),
                &mut Vec::new(),
            )
        }

        fn exact_structured_const_data_carrier_is_eligible(
            program: &dyn CheckingInput,
            type_reference: TypeReferenceHandle,
        ) -> bool {
            let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(type_reference)
            else {
                return false;
            };
            symbol.is_valid()
                && crate::declarations::data_queries::data_definition_by_symbol(program, *symbol)
                    .is_some()
                && exact_structured_const_carrier_is_eligible(
                    program,
                    AppliedValueCarrier::ordinary(type_reference),
                    &mut Vec::new(),
                )
        }

        fn exact_structured_const_carrier_is_eligible(
            program: &dyn CheckingInput,
            carrier: AppliedValueCarrier<'_>,
            visiting: &mut Vec<symbols::SymbolHandle>,
        ) -> bool {
            let carrier = carrier.resolved(program);
            let type_reference = carrier.reference;
            if let Some(primitive) = exact_const_primitive(program, type_reference) {
                return primitive == PrimitiveType::Bool || primitive.accepts_integer_literal();
            }
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::FixedArray {
                    element_type,
                    length,
                } => {
                    carrier.array_length(program, length).is_some()
                        && exact_structured_const_carrier_is_eligible(
                            program,
                            carrier.child(*element_type),
                            visiting,
                        )
                }
                TypeReferenceNode::Named { symbol, .. } => {
                    if !symbol.is_valid() || visiting.contains(symbol) {
                        return false;
                    }
                    visiting.push(*symbol);
                    let eligible = crate::declarations::data_queries::data_definition_by_symbol(
                        program, *symbol,
                    )
                    .is_some_and(|definition| {
                        let members = program.data_members(definition);
                        definition.supply_mode == language_semantics::DataSupplyMode::CheckedShape
                            && definition.name.as_str() != "Rat"
                            && definition.lifetime_parameters.is_empty()
                            && program.data_type_parameters(definition).is_empty()
                            && definition.generic_instance.is_none()
                            && definition.quotient.is_none()
                            && match DataDefinition::shape_kind_from_members(members) {
                                DataShapeKind::Record => members.iter().all(|member| {
                                    let Some(field) = member.as_field() else {
                                        return false;
                                    };
                                    !field.relevance.is_erased()
                                        && exact_structured_const_carrier_is_eligible(
                                            program,
                                            AppliedValueCarrier::ordinary(field.type_reference),
                                            visiting,
                                        )
                                }),
                                DataShapeKind::Enum => members.iter().all(|member| {
                                    let Some(variant) = member.as_variant() else {
                                        return false;
                                    };
                                    program.data_payload_fields(variant).iter().all(|field| {
                                        !field.relevance.is_erased()
                                            && exact_structured_const_carrier_is_eligible(
                                                program,
                                                AppliedValueCarrier::ordinary(field.type_reference),
                                                visiting,
                                            )
                                    })
                                }),
                                DataShapeKind::Empty | DataShapeKind::Mixed => false,
                            }
                    });
                    visiting.pop();
                    eligible
                }
                _ => false,
            }
        }

        fn structured_const_carrier_definition<'p>(
            program: &'p dyn CheckingInput,
            symbol: &symbols::SymbolHandle,
            type_name: &str,
            node_kind: &str,
            visiting: &[symbols::SymbolHandle],
        ) -> Result<&'p DataDefinition, String> {
            if !symbol.is_valid() || visiting.contains(symbol) {
                return Err("structured const carrier is unresolved or recursive".to_owned());
            }
            let definition =
                crate::declarations::data_queries::data_definition_by_symbol(program, *symbol)
                    .ok_or_else(|| {
                        "structured const carrier symbol has no data definition".to_owned()
                    })?;
            if type_name != definition.name.as_str() {
                return Err(format!(
                    "{node_kind} node names `{type_name}`, expected `{}`",
                    definition.name
                ));
            }
            Ok(definition)
        }

        fn validate_decoded_structured_const(
            program: &dyn CheckingInput,
            carrier: AppliedValueCarrier<'_>,
            value: &DecodedCanonicalConstValue,
            visiting: &mut Vec<symbols::SymbolHandle>,
        ) -> Result<(), String> {
            let carrier = carrier.resolved(program);
            let expected_type = carrier.reference;
            if let Some(primitive) = exact_const_primitive(program, expected_type) {
                return match (primitive, value) {
                    (PrimitiveType::Bool, DecodedCanonicalConstValue::Boolean(_)) => Ok(()),
                    (primitive, DecodedCanonicalConstValue::Integer { type_name, value })
                        if primitive.accepts_integer_literal()
                            && type_name == primitive.name()
                            && const_integer_value_fits_primitive(primitive, *value) =>
                    {
                        Ok(())
                    }
                    (PrimitiveType::Bool, _) => Err("expected a canonical Boolean node".to_owned()),
                    (primitive, DecodedCanonicalConstValue::Integer { type_name, value })
                        if primitive.accepts_integer_literal() =>
                    {
                        Err(format!(
                            "integer node `{type_name}({value})` does not fit exact carrier `{}`",
                            primitive.name()
                        ))
                    }
                    (primitive, _) => Err(format!(
                        "expected a canonical integer node for `{}`",
                        primitive.name()
                    )),
                };
            }

            match (
                program.type_reference_table.type_reference(expected_type),
                value,
            ) {
                (
                    TypeReferenceNode::FixedArray {
                        element_type,
                        length,
                    },
                    DecodedCanonicalConstValue::Array { type_name, values },
                ) => {
                    let length = carrier
                        .array_length(program, length)
                        .ok_or("const carrier array extent is not closed")?;
                    let expected_name = canonical_carrier_label(program, carrier);
                    if type_name != &expected_name {
                        return Err(format!(
                            "array node names `{type_name}`, expected `{expected_name}`"
                        ));
                    }
                    if values.len() != length {
                        return Err(format!(
                            "array node has {} elements, expected {length}",
                            values.len()
                        ));
                    }
                    for value in values {
                        validate_decoded_structured_const(
                            program,
                            carrier.child(*element_type),
                            value,
                            visiting,
                        )?;
                    }
                    Ok(())
                }
                (
                    TypeReferenceNode::Named { symbol, .. },
                    DecodedCanonicalConstValue::Record { type_name, fields },
                ) => {
                    let definition = structured_const_carrier_definition(
                        program,
                        symbol,
                        type_name.as_str(),
                        "record",
                        visiting,
                    )?;
                    let members = program.data_members(definition);
                    if DataDefinition::shape_kind_from_members(members) != DataShapeKind::Record {
                        return Err("canonical record node names non-record data".to_owned());
                    }
                    if fields.len() != members.len() {
                        return Err(format!(
                            "record node has {} fields, expected {}",
                            fields.len(),
                            members.len()
                        ));
                    }
                    let declared_fields = members
                        .iter()
                        .filter_map(DataMember::as_field)
                        .collect::<Vec<_>>();
                    super::const_predicates::validate(
                        program,
                        definition,
                        definition.requires_facts,
                        &declared_fields,
                        fields,
                        SymbolHandle::invalid(),
                    )?;
                    visiting.push(*symbol);
                    let result = members.iter().zip(fields).try_for_each(
                        |(member, (encoded_name, encoded_value))| {
                            let Some(field) = member.as_field() else {
                                return Err("structured const carrier contains a case".to_owned());
                            };
                            validate_named_field_value(
                                program,
                                encoded_name.as_str(),
                                field,
                                encoded_value,
                                "record field",
                                visiting,
                            )
                        },
                    );
                    visiting.pop();
                    result
                }
                (
                    TypeReferenceNode::Named { symbol, .. },
                    DecodedCanonicalConstValue::Variant {
                        type_name,
                        case_name,
                        fields,
                    },
                ) => {
                    let definition = structured_const_carrier_definition(
                        program,
                        symbol,
                        type_name.as_str(),
                        "variant",
                        visiting,
                    )?;
                    let members = program.data_members(definition);
                    if DataDefinition::shape_kind_from_members(members) != DataShapeKind::Enum {
                        return Err("canonical variant node names non-sum data".to_owned());
                    }
                    let mut matching_cases = members.iter().filter_map(|member| {
                        let variant = member.as_variant()?;
                        (variant.name.as_str() == case_name.as_str()).then_some(variant)
                    });
                    let variant = matching_cases.next().ok_or_else(|| {
                        format!(
                            "sum `{}` has no case `{case_name}`",
                            definition.name.as_str()
                        )
                    })?;
                    if matching_cases.next().is_some() {
                        return Err(format!(
                            "sum `{}` has duplicate case name `{case_name}`",
                            definition.name.as_str()
                        ));
                    }
                    let payload = program.data_payload_fields(variant);
                    if fields.len() != payload.len() {
                        return Err(format!(
                            "case `{case_name}` has {} payload fields, expected {}",
                            fields.len(),
                            payload.len()
                        ));
                    }
                    let declared_fields = payload.iter().collect::<Vec<_>>();
                    super::const_predicates::validate(
                        program,
                        definition,
                        definition.requires_facts,
                        &declared_fields,
                        fields,
                        variant.symbol,
                    )?;
                    super::const_predicates::validate(
                        program,
                        definition,
                        variant.requires_facts,
                        &declared_fields,
                        fields,
                        variant.symbol,
                    )?;
                    visiting.push(*symbol);
                    let result = payload.iter().zip(fields).try_for_each(
                        |(field, (encoded_name, encoded_value))| {
                            validate_named_field_value(
                                program,
                                encoded_name.as_str(),
                                field,
                                encoded_value,
                                "case payload field",
                                visiting,
                            )
                        },
                    );
                    visiting.pop();
                    result
                }
                _ => Err(format!(
                    "canonical node does not match exact carrier `{}`",
                    type_reference_label(program, expected_type)
                )),
            }
        }

        fn validate_named_field_value(
            program: &dyn CheckingInput,
            encoded_name: &str,
            field: &DataField,
            encoded_value: &DecodedCanonicalConstValue,
            field_role: &str,
            visiting: &mut Vec<SymbolHandle>,
        ) -> Result<(), String> {
            if encoded_name != field.name.as_str() {
                return Err(format!(
                    "{field_role} `{encoded_name}` appears where `{}` is required",
                    field.name
                ));
            }
            validate_decoded_structured_const(
                program,
                AppliedValueCarrier::ordinary(field.type_reference),
                encoded_value,
                visiting,
            )
        }

        fn exact_const_primitive(
            program: &dyn CheckingInput,
            type_reference: TypeReferenceHandle,
        ) -> Option<PrimitiveType> {
            let TypeReferenceNode::Named { symbol, name } =
                program.type_reference_table.type_reference(type_reference)
            else {
                return None;
            };
            let primitive = match program.symbols.builtin_type_atom(*symbol) {
                Some(symbols::BuiltinTypeAtom::Bool) => PrimitiveType::Bool,
                Some(symbols::BuiltinTypeAtom::I8) => PrimitiveType::I8,
                Some(symbols::BuiltinTypeAtom::I16) => PrimitiveType::I16,
                Some(symbols::BuiltinTypeAtom::I32) => PrimitiveType::I32,
                Some(symbols::BuiltinTypeAtom::I64) => PrimitiveType::I64,
                Some(symbols::BuiltinTypeAtom::U8) => PrimitiveType::U8,
                Some(symbols::BuiltinTypeAtom::U16) => PrimitiveType::U16,
                Some(symbols::BuiltinTypeAtom::U32) => PrimitiveType::U32,
                Some(symbols::BuiltinTypeAtom::U64) => PrimitiveType::U64,
                Some(symbols::BuiltinTypeAtom::Address) => PrimitiveType::Addr,
                Some(_) => return None,
                None if !symbol.is_valid() => PrimitiveType::from_name(name.as_str())?,
                None => return None,
            };
            (name.as_str() == primitive.name()).then_some(primitive)
        }

        fn validate_typed_const_index_type(
            program: &dyn CheckingInput,
            type_reference: TypeReferenceHandle,
            visiting: &mut HashSet<String>,
        ) -> Result<(), String> {
            match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { name, .. } => {
            if matches!(
                name.as_str(),
                "bool" | "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64"
                    | "addr"
            ) {
                return Ok(());
            }
            if matches!(name.as_str(), "f32" | "f64") {
                return Err(format!(
                    "`{name}` does not have canonical structural index identity"
                ));
            }
            if !visiting.insert(name.as_str().to_owned()) {
                return Ok(());
            }
            let definition = program
                .data_definition_by_name_str(name.as_str())
                .ok_or_else(|| format!("`{name}` is not declared data"))?;
            if definition.supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque {
                return Err(format!("boundary-opaque data `{name}` is not structural"));
            }
            if definition.quotient.is_some() {
                return Err(format!(
                    "quotient data `{name}` has no compiler-verified canonical representative"
                ));
            }
            if !definition.requires_facts.is_empty() {
                return Err(format!(
                    "data `{name}` has default-domain facts whose index-site proof is not implemented"
                ));
            }
            for member in program.data_members(definition) {
                match member {
                    DataMember::Field(field) => validate_typed_const_index_type(
                        program,
                        field.type_reference,
                        visiting,
                    )?,
                    DataMember::Variant(variant) => {
                        for field in program.data_payload_fields(variant) {
                            validate_typed_const_index_type(
                                program,
                                field.type_reference,
                                visiting,
                            )?;
                        }
                    }
                }
            }
            visiting.remove(name.as_str());
            Ok(())
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(_),
        } => validate_typed_const_index_type(program, *element_type, visiting),
        TypeReferenceNode::Constrained { base_type, .. } => {
            validate_typed_const_index_type(program, *base_type, visiting)
        }
        TypeReferenceNode::Unit => Ok(()),
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::FixedArray { .. } => Err(
            "indices require finite structural values with decidable equality and one canonical form"
                .to_owned(),
        ),
            }
        }

        fn validate_const_integer_carrier(
            base_name: &str,
            parameter: &TypeParameter,
            primitive: PrimitiveType,
            value: i128,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            if !const_integer_value_fits_primitive(primitive, value) {
                diagnostics.push(Diagnostic::error(format!(
                    "const argument `{value}` for `{base_name}::{}` does not fit `{}`",
                    parameter.name,
                    primitive.name()
                )));
            }
        }

        pub(crate) fn validate_const_binder_carriers(
            program: &dyn CheckingInput,
            symbols: &TopLevelSymbols<'_>,
            owner: &str,
            parameters: &[TypeParameter],
            scope: TypeParameterScope<'_>,
            diagnostics: &mut Vec<Diagnostic>,
            formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
        ) {
            for parameter in parameters {
                let (TypeParameterKind::Const {
                    type_reference: carrier,
                }
                | TypeParameterKind::Value {
                    type_reference: carrier,
                }) = parameter.kind
                else {
                    continue;
                };
                super::validate_type_reference_handle(
                    program,
                    carrier,
                    symbols,
                    diagnostics,
                    TypeReferenceOwner::GenericBinder {
                        owner,
                        parameter: parameter.name.as_str(),
                        generic_depth: 0,
                    },
                    scope,
                    formation_obligations,
                );
            }
        }

        pub(crate) fn const_integer_value_fits_primitive(
            primitive: PrimitiveType,
            value: i128,
        ) -> bool {
            primitive
                .integer_bounds()
                .is_some_and(|(minimum, maximum)| value >= minimum && value <= maximum)
        }

        pub(crate) fn validate_generic_argument_bounds(
            program: &dyn CheckingInput,
            base_symbol: SymbolHandle,
            base_name: &str,
            arguments: arena::HandleSpan<TypeReferenceHandle>,
            type_parameter_scope: TypeParameterScope<'_>,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let Some(definition) = program.data_definitions().iter().find(|definition| {
                definition.symbol == base_symbol
                    || (!program.symbols.has_source_metadata()
                        && definition.name.as_str() == base_name)
            }) else {
                return;
            };

            let parameters = program.data_type_parameters(definition);
            let argument_handles = program
                .type_reference_table
                .type_reference_handles(arguments);
            validate_parameter_bounds(
                program,
                base_name,
                parameters,
                argument_handles,
                type_parameter_scope,
                diagnostics,
            );
        }

        pub(crate) fn validate_parameter_bounds(
            program: &dyn CheckingInput,
            base_name: &str,
            parameters: &[TypeParameter],
            arguments: &[TypeReferenceHandle],
            type_parameter_scope: TypeParameterScope<'_>,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            for (parameter, argument) in parameters.iter().zip(arguments) {
                let bounds = crate::proof_contracts::properties::declared_property_requirements(
                    &parameter.bounds,
                );
                let bound_labels = bounds.iter().map(ToString::to_string).collect::<Vec<_>>();
                for property in bounds {
                    if crate::proof_contracts::properties::type_satisfies_declared_property(
                        program,
                        type_parameter_scope.type_parameters,
                        *argument,
                        property,
                    ) {
                        continue;
                    }
                    diagnostics.push(Diagnostic::error(format!(
                "type parameter `{} [{}]` of `{base_name}` was instantiated with `{}`, which does not satisfy `[{property}]`",
                parameter.name,
                bound_labels.join(", "),
                type_reference_label(program, *argument)
            )));
                }
            }
        }

        fn declaration_parameters(
            program: &dyn CheckingInput,
            owner: SymbolHandle,
        ) -> Vec<StateParameter> {
            if let Some(machine) = program.machine_by_symbol(owner) {
                return program
                    .machine_states(machine)
                    .iter()
                    .flat_map(|state| program.state_parameters(state).iter().cloned())
                    .collect();
            }
            program
                .trait_machine_signature(owner)
                .map(|(_, signature)| signature)
                .or_else(|| {
                    program
                        .retained_static_machine_contract(owner)
                        .map(|contract| contract.signature())
                })
                .map(|signature| program.state_signature_parameters(signature).to_vec())
                .unwrap_or_default()
        }
    }
    pub(crate) mod implementation_arguments {
        use super::{
            TopLevelSymbols, TypeParameterScope, TypeReferenceOwner,
            generic_arguments::{
                AppliedValueCarrier, validate_const_argument_expression_formation,
            },
            validate_type_reference_node,
        };
        use diagnostics::Diagnostic;
        use typed_trees::{
            data::TypeParameterKind,
            types::{TypeReferenceHandle, TypeReferenceNode},
        };

        pub(crate) fn validate_operand_formation(
            program: &dyn crate::CheckingInput,
            argument: TypeReferenceHandle,
            symbols: &TopLevelSymbols<'_>,
            owner: TypeReferenceOwner<'_>,
            scope: TypeParameterScope<'_>,
            diagnostics: &mut Vec<Diagnostic>,
            obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
        ) {
            let reference = program
                .type_reference_table
                .unconstrained_type_reference(argument);
            let TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } = program.type_reference_table.type_reference(reference)
            else {
                return;
            };
            let Some(declaration) = program.conformance_by_symbol(*base_symbol) else {
                return;
            };
            let parameters = program.conformance_type_parameters(declaration);
            let arguments = program
                .type_reference_table
                .type_reference_handles(*arguments);
            for (parameter, argument) in parameters.iter().zip(arguments) {
                match parameter.kind {
                    TypeParameterKind::Const { type_reference }
                    | TypeParameterKind::Value { type_reference } => {
                        validate_const_argument_expression_formation(
                            program,
                            &program.symbols.display_path(declaration.symbol, "::"),
                            parameter,
                            AppliedValueCarrier::applied(type_reference, parameters, arguments),
                            *argument,
                            scope,
                            &owner.value_formation_facts(program),
                            owner.local_value_formation(),
                            diagnostics,
                            obligations,
                        );
                    }
                    TypeParameterKind::Type => validate_type_reference_node(
                        program,
                        *argument,
                        symbols,
                        diagnostics,
                        owner.generic_argument(),
                        scope,
                        obligations,
                    ),
                    TypeParameterKind::Implementation => validate_operand_formation(
                        program,
                        *argument,
                        symbols,
                        owner.generic_argument(),
                        scope,
                        diagnostics,
                        obligations,
                    ),
                    TypeParameterKind::Machine { .. } | TypeParameterKind::Proposition { .. } => {}
                }
            }
        }
    }
    pub(crate) mod open_index_expressions {
        use super::{
            TypeParameterScope, TypeReferenceOwner, type_reference_label, type_references_match,
        };
        use crate::frozen_program::expression_table;
        use diagnostics::Diagnostic;
        use language_semantics::const_value::CanonicalConstValue;
        use std::collections::HashMap;
        use std::fmt;
        use symbols::JoinedPath;
        use symbols::SymbolHandle;
        use typed_trees::TypedTrees;
        use typed_trees::data::TypeParameterKind;
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

        pub(crate) fn validate_indexed_domain_arguments(
            program: &dyn crate::CheckingInput,
            constraint: &typed_trees::types::DomainConstraint,
            scope: TypeParameterScope<'_>,
            diagnostics: &mut Vec<Diagnostic>,
            owner: &TypeReferenceOwner<'_>,
        ) {
            let Some(definition) = program.domain_definition(constraint.symbol) else {
                return;
            };
            validate_indexed_domain_argument_pack(
                program,
                definition,
                constraint.name.as_str(),
                &constraint.arguments,
                scope,
                diagnostics,
                &owner,
            );
        }

        pub(crate) fn validate_indexed_qualification_arguments(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            cast: &typed_trees::expression::TableCastExpression,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            if !cast.semantic_domain_symbol.is_valid() {
                return;
            }
            let Some(definition) = program.domain_definition(cast.semantic_domain_symbol) else {
                return;
            };
            let arguments = program
                .type_reference_table
                .type_reference_handles(cast.semantic_domain_arguments);
            let domain_name = expression_table(program)
                .name_path_members(cast.semantic_domain)
                .joined_path();
            let owner = format!("machine `{}` indexed qualification", machine.name);
            validate_indexed_domain_argument_pack(
                program,
                definition,
                &domain_name,
                arguments,
                TypeParameterScope::authored(
                    machine.symbol,
                    program.machine_type_parameters(machine),
                    &machine.lifetime_parameters,
                ),
                diagnostics,
                &owner,
            );
        }

        fn collect_domain_const_sites<'a>(
            program: &dyn crate::CheckingInput,
            parameters: &[typed_trees::data::TypeParameter],
            arguments: impl Iterator<Item = &'a TypeReferenceHandle>,
            sites: &mut Vec<(ExpressionHandle, TypeReferenceHandle)>,
        ) {
            for (parameter, argument) in parameters.iter().zip(arguments) {
                let TypeParameterKind::Const {
                    type_reference: expected,
                } = parameter.kind
                else {
                    continue;
                };
                let TypeReferenceNode::ConstExpression(expression) =
                    program.type_reference_table.type_reference(*argument)
                else {
                    continue;
                };
                if !sites.iter().any(|(existing, _)| *existing == *expression) {
                    sites.push((*expression, expected));
                }
            }
        }

        pub fn normalize_open_index_expressions(
            program: &mut TypedTrees,
        ) -> Result<(), Vec<Diagnostic>> {
            if program
                .domain_definitions()
                .iter()
                .all(|domain| domain.type_parameters.is_empty())
            {
                return Ok(());
            }
            let mut sites = Vec::new();
            for (_, _, constraints) in program
                .type_reference_table
                .constrained_type_reference_sites()
            {
                for constraint in program.type_reference_table.constraints(constraints) {
                    let TypeConstraintNode::Domain(constraint) = constraint else {
                        continue;
                    };
                    let Some(definition) = program.domain_definition(constraint.symbol) else {
                        continue;
                    };
                    let parameters = typed_trees::domain::index_parameters(program, definition);
                    collect_domain_const_sites(
                        program,
                        parameters,
                        constraint.arguments.iter(),
                        &mut sites,
                    );
                }
            }
            let indexed_qualification_sites = expression_table(program)
                .expression_entries()
                .filter_map(|(_, expression)| {
                    let typed_trees::expression::ExpressionNode::Cast(cast) = expression else {
                        return None;
                    };
                    (cast.semantic_domain_symbol.is_valid()
                        && !cast.semantic_domain_arguments.is_empty())
                    .then_some((cast.semantic_domain_symbol, cast.semantic_domain_arguments))
                })
                .collect::<Vec<_>>();
            for (domain_symbol, arguments) in indexed_qualification_sites {
                let Some(definition) = program.domain_definition(domain_symbol) else {
                    continue;
                };
                let parameters = typed_trees::domain::index_parameters(program, definition);
                collect_domain_const_sites(
                    program,
                    parameters,
                    program
                        .type_reference_table
                        .type_reference_handles(arguments)
                        .iter(),
                    &mut sites,
                );
            }

            let binder_owners = const_binder_owners(program);
            let mut diagnostics = Vec::new();
            let mut normalizations = Vec::new();
            for (expression, index_type) in sites {
                let mut operations = Vec::new();
                let owner = open_index_expression_owner(program, expression, &binder_owners);
                normalize_open_index_expression_operations(
                    program,
                    expression,
                    index_type,
                    owner,
                    &mut operations,
                    &mut diagnostics,
                );
                normalizations.push((
                    expression,
                    typed_trees::expression::OpenIndexNormalization {
                        index_type,
                        normalizer_version: 4,
                    },
                    operations,
                ));
            }
            let fact_operations = bind_machine_index_operations(program);
            if diagnostics.is_empty() {
                for (expression, normalization, operations) in normalizations {
                    program
                        .expression_table
                        .index_expression_authority_mut(expression)
                        .normalization = Some(normalization);
                    for operation in operations {
                        let expression = operation.expression;
                        program
                            .expression_table
                            .index_expression_authority_mut(expression)
                            .operation = Some(operation);
                    }
                }
                for operation in fact_operations {
                    let expression = operation.expression;
                    program
                        .expression_table
                        .index_expression_authority_mut(expression)
                        .operation = Some(operation);
                }
                Ok(())
            } else {
                Err(diagnostics)
            }
        }

        fn const_binder_owners(
            program: &dyn crate::CheckingInput,
        ) -> HashMap<SymbolHandle, SymbolHandle> {
            let mut owners = HashMap::new();
            let mut seed = |parameters: &[typed_trees::data::TypeParameter],
                            owner: SymbolHandle| {
                for parameter in parameters {
                    owners.insert(parameter.symbol, owner);
                }
            };
            for machine in program.machines() {
                seed(program.machine_type_parameters(machine), machine.symbol);
            }
            for data in program.data_definitions() {
                seed(program.data_type_parameters(data), data.symbol);
            }
            for domain in program.domain_definitions() {
                seed(program.domain_type_parameters(domain), domain.symbol);
            }
            for operator in program.operators() {
                seed(program.operator_type_parameters(operator), operator.symbol);
            }
            for trait_definition in program.traits() {
                seed(
                    program.trait_type_parameters(trait_definition),
                    trait_definition.symbol,
                );
                for signature in program.trait_machine_signatures(trait_definition) {
                    seed(
                        program.state_signature_type_parameters(signature),
                        trait_definition.symbol,
                    );
                }
            }
            for conformance in program.conformances() {
                seed(
                    program.conformance_type_parameters(conformance),
                    conformance.symbol,
                );
            }
            owners
        }

        fn open_index_expression_owner(
            program: &dyn crate::CheckingInput,
            expression: typed_trees::expression::ExpressionHandle,
            binder_owners: &HashMap<SymbolHandle, SymbolHandle>,
        ) -> Option<SymbolHandle> {
            use typed_trees::expression::ExpressionNode;

            let mut owner = None;
            let mut leaves = vec![expression];
            while let Some(handle) = leaves.pop() {
                match expression_table(program).expression(handle) {
                    ExpressionNode::Name(path) => {
                        if let Some(candidate) = binder_owners.get(&path.symbol)
                            && let Some(existing) = owner.replace(*candidate)
                            && existing != *candidate
                        {
                            return None;
                        }
                    }
                    ExpressionNode::Binary(binary) => {
                        leaves.push(binary.left);
                        leaves.push(binary.right);
                    }
                    ExpressionNode::Unary(unary) => leaves.push(unary.operand),
                    _ => {}
                }
            }
            owner
        }

        fn normalize_open_index_expression_operations(
            program: &dyn crate::CheckingInput,
            expression: typed_trees::expression::ExpressionHandle,
            index_type: TypeReferenceHandle,
            owner: Option<SymbolHandle>,
            operations: &mut Vec<typed_trees::expression::OpenIndexOperationSelection>,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            use language_core::operator_spelling::OperatorSpelling;
            use typed_trees::expression::BinaryOperator;

            let Some(binary) = expression_table(program).binary_node(expression) else {
                return;
            };
            let spelling = match binary.operator {
                BinaryOperator::Add => OperatorSpelling::Add,
                BinaryOperator::Subtract => OperatorSpelling::Subtract,
                BinaryOperator::Multiply => OperatorSpelling::Multiply,
                BinaryOperator::Divide => OperatorSpelling::Divide,
                _ => return,
            };
            let Some(owner) = owner else {
                return;
            };
            let Some(machine) = program.machine(owner) else {
                diagnostics.push(Diagnostic::error(format!(
            "open index operator `{}` requires an explicit conformance binder on the machine owning the index expression, but the owner is not a machine",
            spelling.symbol(),
        )));
                return;
            };
            let selectable = matches!(
                program.type_reference_table.type_reference(index_type),
                TypeReferenceNode::Named { .. } | TypeReferenceNode::Constrained { .. }
            );
            if !selectable {
                return;
            }
            let candidates =
                bound_index_operations(program, machine, expression, index_type, spelling);
            let [operation] = candidates.as_slice() else {
                diagnostics.push(Diagnostic::error(format!(
            "open index operator `{}` requires one exact conformance bound on machine `{}` supplying a `{}` operation spelled `{}`, but {} were found",
            spelling.symbol(),
            machine.name,
            type_reference_label(program, index_type),
            spelling.symbol(),
            candidates.len()
        )));
                return;
            };
            operations.push(operation.clone());
            normalize_open_index_expression_operations(
                program,
                binary.left,
                index_type,
                Some(owner),
                operations,
                diagnostics,
            );
            normalize_open_index_expression_operations(
                program,
                binary.right,
                index_type,
                Some(owner),
                operations,
                diagnostics,
            );
        }

        fn bound_index_operations(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            expression: ExpressionHandle,
            index_type: TypeReferenceHandle,
            spelling: language_core::operator_spelling::OperatorSpelling,
        ) -> Vec<typed_trees::expression::OpenIndexOperationSelection> {
            typed_trees::operator::bound_trait_operator_meanings(
                program,
                machine.symbol,
                spelling,
                &[Some(index_type), Some(index_type)],
            )
            .into_iter()
            .filter_map(|meaning| {
                if meaning.result_identity(program) != program.normalized_type_identity(index_type)
                {
                    return None;
                }
                let trait_definition = meaning.trait_definition;
                let requirement = meaning.requirement;
                let bound = meaning.bound;
                let laws = crate::proof_contracts::contract_entailment::declared_index_algebra_laws(
                    program,
                    trait_definition,
                    requirement.symbol,
                );
                let identities =
                    crate::proof_contracts::contract_entailment::literal_index_identities(
                        program,
                        bound,
                        requirement.symbol,
                        index_type,
                    );
                Some(typed_trees::expression::OpenIndexOperationSelection {
                    expression,
                    spelling,
                    operator: requirement.symbol,
                    operand_type: index_type,
                    provider: bound.binder,
                    application: None,
                    algebra_trait: trait_definition.symbol,
                    algebra_requirement: requirement.name.as_str().to_owned(),
                    algebra_alias: Some(bound.binder_name.as_str().to_owned()),
                    commutativity_licensed: !laws.commutativity.is_empty(),
                    associativity_licensed: !laws.associativity.is_empty(),
                    distributes_over: laws.distributivity,
                    left_identity_literal: identities.left,
                    right_identity_literal: identities.right,
                })
            })
            .collect()
        }

        fn bind_machine_index_operations(
            program: &dyn crate::CheckingInput,
        ) -> Vec<typed_trees::expression::OpenIndexOperationSelection> {
            let mut operations = Vec::new();
            program.for_each_machine_state_contract_frame(|machine, state, contracts, _| {
                if machine.conformance_bounds.is_empty() {
                    return;
                }
                let mut expressions = Vec::new();
                for contract in contracts {
                    crate::queries::expression_traversal::collect_contract_facts(
                        program,
                        contract.facts,
                        &mut expressions,
                    );
                }
                for statement in program.statement_table.statements(state.statement_nodes) {
                    crate::queries::expression_traversal::collect_statement_expression_trees(
                        program,
                        statement,
                        &mut expressions,
                    );
                }
                for expression in expressions {
                    let Some(binary) = expression_table(program).binary_node(expression) else {
                        continue;
                    };
                    let Some(spelling) =
                        typed_trees::operator::arithmetic_operator_spelling(binary.operator)
                    else {
                        continue;
                    };
                    let operand_type = crate::expression_result_type_reference(
                        program,
                        machine,
                        state,
                        binary.left,
                    )
                    .or_else(|| {
                        crate::expression_result_type_reference(
                            program,
                            machine,
                            state,
                            binary.right,
                        )
                    });
                    let Some(operand_type) = operand_type else {
                        continue;
                    };
                    let candidates = bound_index_operations(
                        program,
                        machine,
                        expression,
                        operand_type,
                        spelling,
                    );
                    if let [operation] = candidates.as_slice() {
                        operations.push(operation.clone());
                    }
                }
            });
            operations
        }

        fn validate_indexed_domain_argument_pack(
            program: &dyn crate::CheckingInput,
            definition: &typed_trees::domain::DomainDefinition,
            domain_name: &str,
            arguments: &[TypeReferenceHandle],
            scope: TypeParameterScope<'_>,
            diagnostics: &mut Vec<Diagnostic>,
            owner: &dyn fmt::Display,
        ) {
            let parameters = typed_trees::domain::index_parameters(program, definition);
            if parameters.is_empty() {
                return;
            }
            let closed_application = arguments
                .iter()
                .all(|argument| typed_trees::domain::index_argument_is_closed(program, *argument));
            for (parameter, argument) in parameters.iter().zip(arguments) {
                let TypeParameterKind::Const {
                    type_reference: expected,
                } = parameter.kind
                else {
                    continue;
                };
                let TypeReferenceNode::Named { symbol, name } =
                    program.type_reference_table.type_reference(*argument)
                else {
                    if let TypeReferenceNode::ConstExpression(expression) =
                        program.type_reference_table.type_reference(*argument)
                    {
                        let prior_diagnostics = diagnostics.len();
                        validate_open_index_expression(
                            program,
                            *expression,
                            expected,
                            scope,
                            domain_name,
                            owner,
                            diagnostics,
                        );
                        if closed_application
                            && typed_trees::domain::index_argument_is_closed(program, expected)
                            && diagnostics.len() == prior_diagnostics
                        {
                            let carrier = super::generic_arguments::AppliedValueCarrier::applied(
                                expected, parameters, arguments,
                            );
                            let completed =
                        super::generic_arguments::normalize_closed_const_expression_in_carrier(
                            program,
                            scope.lexical_owner,
                            *expression,
                            carrier,
                        )
                        .and_then(|value| {
                            if let Some(value) = value {
                                super::generic_arguments::validate_exact_const_identity_in_carrier(
                                    program,
                                    carrier,
                                    &value.identity(),
                                )?;
                            }
                            Ok(())
                        });
                            if let Err(reason) = completed {
                                diagnostics.push(Diagnostic::error(format!(
                            "{owner} supplies an invalid closed argument for indexed domain `{domain_name}`: {reason}"
                        )));
                            }
                        }
                    } else {
                        diagnostics.push(Diagnostic::error(format!(
                            "{owner} supplies a noncanonical argument for indexed domain `{}`",
                            domain_name
                        )));
                    }
                    continue;
                };
                if closed_application {
                    if let Err(mut failures) =
                        super::generic_arguments::validate_closed_const_argument(
                            program,
                            domain_name,
                            parameter,
                            *argument,
                            parameters,
                            arguments,
                        )
                    {
                        diagnostics.append(&mut failures);
                    }
                    continue;
                }
                if let Some(value) =
                    language_semantics::const_value::CanonicalConstValue::from_atom(name.as_str())
                {
                    let expected_name = const_index_type_label(program, expected);
                    if value.type_name != expected_name {
                        diagnostics.push(Diagnostic::error(format!(
                    "{owner} supplies indexed-domain argument type `{}`, but `{}` requires `{expected_name}`",
                    value.type_name, parameter.name
                )));
                    }
                    continue;
                }
                if let Ok(value) = name.as_str().parse::<i128>() {
                    if !integer_const_fits_type(program, expected, value) {
                        diagnostics.push(Diagnostic::error(format!(
                            "{owner} supplies integer index `{value}` outside the declared `{}` type",
                            const_index_type_label(program, expected)
                        )));
                    }
                    continue;
                }
                let binder = scope.type_parameters.iter().find(|candidate| {
                    candidate.kind.is_const_or_value()
                        && if symbol.is_valid() {
                            candidate.symbol == *symbol
                        } else {
                            candidate.name.as_str() == name.as_str()
                        }
                });
                let Some(binder) = binder else {
                    diagnostics.push(Diagnostic::error(format!(
                "{owner} supplies `{name}` as an index for `{}`, but it is neither a canonical named const nor a direct in-scope const binder",
                domain_name
            )));
                    continue;
                };
                let actual = const_binder_type_reference(binder);
                if !type_references_match(program, actual, expected) {
                    diagnostics.push(Diagnostic::error(format!(
                "{owner} forwards const binder `{}` of type `{}` into `{}`, which requires `{}`",
                binder.name,
                type_reference_label(program, actual),
                domain_name,
                type_reference_label(program, expected)
            )));
                }
            }
        }

        fn const_binder_type_reference(
            binder: &typed_trees::data::TypeParameter,
        ) -> TypeReferenceHandle {
            let (TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference }) = binder.kind
            else {
                unreachable!();
            };
            type_reference
        }

        fn validate_open_index_expression(
            program: &dyn crate::CheckingInput,
            expression: typed_trees::expression::ExpressionHandle,
            expected: TypeReferenceHandle,
            scope: TypeParameterScope<'_>,
            domain_name: &str,
            owner: &dyn fmt::Display,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            use typed_trees::expression::{BinaryOperator, ExpressionNode};

            match expression_table(program).expression(expression) {
        ExpressionNode::Name(path) => {
            let name = expression_table(program).name_path_members(path.members).joined_path();
            if let Some(value) = CanonicalConstValue::from_atom(&name) {
                let expected_name = const_index_type_label(program, expected);
                if value.type_name != expected_name {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} uses closed index atom of type `{}` in `{domain_name}`, whose expression requires `{expected_name}`",
                        value.type_name
                    )));
                }
                return;
            }
            if let Ok(value) = name.parse::<i128>() {
                if !integer_const_fits_type(program, expected, value) {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} uses integer index `{value}` outside the declared `{}` type in `{domain_name}`",
                        const_index_type_label(program, expected)
                    )));
                }
                return;
            }
            let symbol = if path.symbol.is_valid() { path.symbol } else { path.head_symbol };
            let binder = scope.type_parameters.iter().find(|candidate| {
                candidate.kind.is_const_or_value()
                    && if symbol.is_valid() {
                    candidate.symbol == symbol
                } else {
                    candidate.name.as_str() == name
                }
            });
            let Some(binder) = binder else {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses `{name}` in open index expression `{}`, but it is not a direct in-scope const binder",
                    expression_table(program).display_name(expression)
                )));
                return;
            };
            let actual = const_binder_type_reference(binder);
            if !type_references_match(program, actual, expected) {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses const binder `{}` of type `{}` in `{domain_name}`, whose index expression requires `{}`",
                    binder.name,
                    type_reference_label(program, actual),
                    type_reference_label(program, expected)
                )));
            }
        }
        ExpressionNode::Integer(value) => {
            let Some(value) = value
                .value_i64()
                .map(i128::from)
                .or_else(|| value.value_u64().map(i128::from))
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses an integer outside the const-index envelope in `{domain_name}`"
                )));
                return;
            };
            if !integer_const_fits_type(program, expected, value) {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses integer index `{value}` outside the declared `{}` type in `{domain_name}`",
                    const_index_type_label(program, expected)
                )));
            }
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
            ) =>
        {
            validate_open_index_expression(
                program,
                binary.left,
                expected,
                scope,
                domain_name,
                owner,
                diagnostics,
            );
            validate_open_index_expression(
                program,
                binary.right,
                expected,
                scope,
                domain_name,
                owner,
                diagnostics,
            );
        }
        _ => diagnostics.push(Diagnostic::error(format!(
            "{owner} uses unsupported open index expression `{}` in `{domain_name}`; only direct const binders combined with `+`, `-`, `*`, or `/` are in the proof-static algebra fragment",
            expression_table(program).display_name(expression)
        ))),
            }
        }

        fn const_index_type_label(
            program: &dyn crate::CheckingInput,
            type_reference: TypeReferenceHandle,
        ) -> String {
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Named { name, .. } => name.as_str().to_owned(),
                TypeReferenceNode::Constrained { base_type, .. } => {
                    const_index_type_label(program, *base_type)
                }
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: typed_trees::types::FixedArrayLength::Literal(length),
                } => format!(
                    "[{}; {length}]",
                    const_index_type_label(program, *element_type)
                ),
                TypeReferenceNode::Unit => "()".to_owned(),
                _ => type_reference_label(program, type_reference),
            }
        }

        fn integer_const_fits_type(
            program: &dyn crate::CheckingInput,
            type_reference: TypeReferenceHandle,
            value: i128,
        ) -> bool {
            let label = const_index_type_label(program, type_reference);
            let (minimum, maximum) = match label.as_str() {
                "i8" => (i128::from(i8::MIN), i128::from(i8::MAX)),
                "i16" => (i128::from(i16::MIN), i128::from(i16::MAX)),
                "i32" => (i128::from(i32::MIN), i128::from(i32::MAX)),
                "i64" => (i128::from(i64::MIN), i128::from(i64::MAX)),
                "u8" => (0, i128::from(u8::MAX)),
                "u16" => (0, i128::from(u16::MAX)),
                "u32" => (0, i128::from(u32::MAX)),
                "u64" | "addr" => (0, i128::from(u64::MAX)),
                _ => return false,
            };
            value >= minimum && value <= maximum
        }
    }
    pub(crate) mod static_argument_scope {
        use super::StaticTypeArgumentOwner;
        use crate::mathematical_applications::{
            FixedCoreVocabulary, MathematicalBinderKind, mathematical_binder_kind,
        };
        use std::borrow::Cow;
        use symbols::{SymbolHandle, SymbolKind};
        use typed_trees::{data::TypeParameter, name::Identifier};

        impl<'program> StaticTypeArgumentOwner<'program> {
            pub fn from_lexical_owner(
                program: &'program dyn crate::CheckingInput,
                owner: SymbolHandle,
            ) -> Option<Self> {
                if let Some(machine) = program.machine_declaration_for_lexical_scope(owner) {
                    return Some(Self::Machine(machine));
                }
                if let Some((declaration, signature)) = program.trait_machine_signature(owner) {
                    return Some(Self::TraitSignature {
                        declaration,
                        signature,
                    });
                }
                if let Some((declaration, signature)) = program.machine_parameter_signature(owner) {
                    return Some(Self::MachineSignature {
                        declaration,
                        signature,
                    });
                }
                if let Some(data) = program.data_definition_by_symbol(owner) {
                    return Some(Self::Data(data));
                }
                if let Some(declaration) = program
                    .mathematical_definitions()
                    .iter()
                    .find(|declaration| declaration.symbol == owner)
                {
                    return Some(Self::Mathematical {
                        declaration,
                        binders: program.data_type_parameters.span(declaration.binders)?,
                    });
                }
                // Const symbols deliberately outlive value substitution and retain
                // source provenance even when an executable projection omits the value.
                (owner.is_valid() && program.symbols.get(owner).kind == SymbolKind::Const).then(
                    || Self::Constant {
                        symbol: owner,
                        name: program.symbols.name(owner),
                    },
                )
            }

            pub fn parameters(
                self,
                program: &'program dyn crate::CheckingInput,
            ) -> (
                Cow<'program, [TypeParameter]>,
                Cow<'program, [Identifier]>,
                SymbolHandle,
            ) {
                match self {
                    Self::Machine(machine) => {
                        let (types, lifetimes) = program
                            .machine_type_reference_scope(machine, |left, right| {
                                left.name == right.name
                            });
                        (Cow::Owned(types), Cow::Owned(lifetimes), machine.symbol)
                    }
                    Self::Data(data) => (
                        Cow::Borrowed(program.data_type_parameters(data)),
                        Cow::Borrowed(data.lifetime_parameters.as_slice()),
                        data.symbol,
                    ),
                    Self::TraitSignature {
                        declaration,
                        signature,
                    } => {
                        let mut types = program.state_signature_type_parameters(signature).to_vec();
                        let mut lifetimes = signature.lifetime_parameters.clone();
                        append_scope(
                            &mut types,
                            &mut lifetimes,
                            program.trait_type_parameters(declaration),
                            &declaration.lifetime_parameters,
                        );
                        (Cow::Owned(types), Cow::Owned(lifetimes), signature.symbol)
                    }
                    Self::MachineSignature {
                        declaration,
                        signature,
                    } => {
                        let mut types = program.state_signature_type_parameters(signature).to_vec();
                        let mut lifetimes = signature.lifetime_parameters.clone();
                        let (outer_types, outer_lifetimes) = program
                            .machine_type_reference_scope(declaration, |left, right| {
                                left.name == right.name
                            });
                        append_scope(&mut types, &mut lifetimes, &outer_types, &outer_lifetimes);
                        (Cow::Owned(types), Cow::Owned(lifetimes), signature.symbol)
                    }
                    Self::Mathematical {
                        declaration,
                        binders,
                    } => {
                        let mut types = binders.to_vec();
                        let vocabulary = FixedCoreVocabulary::resolve(&program.symbols);
                        for parameter in &mut types {
                            if matches!(
                                mathematical_binder_kind(program, parameter, &vocabulary),
                                Some(MathematicalBinderKind::Type)
                            ) {
                                // The mathematical telescope retains A: Type<u>
                                // as a term binder. This type-checking view must
                                // retain its authenticated type-forming role.
                                parameter.kind = typed_trees::data::TypeParameterKind::Type;
                            }
                        }
                        let (outer_types, lifetimes) =
                            enclosing_parameters(program, declaration.symbol);
                        let mut lifetimes = lifetimes;
                        append_scope(&mut types, &mut lifetimes, &outer_types, &[]);
                        (Cow::Owned(types), Cow::Owned(lifetimes), declaration.symbol)
                    }
                    Self::Constant { symbol, .. } => {
                        let (types, lifetimes) = enclosing_parameters(program, symbol);
                        (Cow::Owned(types), Cow::Owned(lifetimes), symbol)
                    }
                }
            }
        }

        fn enclosing_parameters(
            program: &dyn crate::CheckingInput,
            constant: SymbolHandle,
        ) -> (Vec<TypeParameter>, Vec<Identifier>) {
            let mut types = Vec::new();
            let mut lifetimes = Vec::new();
            let mut enclosing = program.declaration_scope_parent(constant);
            while enclosing.is_valid() {
                if let Some(machine) = program.machine_declaration_for_lexical_scope(enclosing) {
                    let (outer_types, outer_lifetimes) = program
                        .machine_type_reference_scope(machine, |left, right| {
                            left.name == right.name
                        });
                    append_scope(&mut types, &mut lifetimes, &outer_types, &outer_lifetimes);
                    break;
                }
                if let Some(data) = program.data_definition_by_symbol(enclosing) {
                    append_scope(
                        &mut types,
                        &mut lifetimes,
                        program.data_type_parameters(data),
                        &data.lifetime_parameters,
                    );
                } else if let Some(conformance) = program.conformance_by_symbol(enclosing) {
                    append_scope(
                        &mut types,
                        &mut lifetimes,
                        program.conformance_type_parameters(conformance),
                        &conformance.lifetime_parameters,
                    );
                } else if let Some(trait_definition) = program.trait_definition_by_symbol(enclosing)
                {
                    append_scope(
                        &mut types,
                        &mut lifetimes,
                        program.trait_type_parameters(trait_definition),
                        &trait_definition.lifetime_parameters,
                    );
                } else if let Some(domain) = program.domain_definition(enclosing) {
                    append_scope(
                        &mut types,
                        &mut lifetimes,
                        program.domain_type_parameters(domain),
                        &[],
                    );
                }
                enclosing = program.declaration_scope_parent(enclosing);
            }
            (types, lifetimes)
        }

        fn append_scope(
            types: &mut Vec<TypeParameter>,
            lifetimes: &mut Vec<Identifier>,
            outer_types: &[TypeParameter],
            outer_lifetimes: &[Identifier],
        ) {
            for parameter in outer_types {
                if !types.iter().any(|existing| existing.name == parameter.name) {
                    types.push(parameter.clone());
                }
            }
            for lifetime in outer_lifetimes {
                if !lifetimes.contains(lifetime) {
                    lifetimes.push(lifetime.clone());
                }
            }
        }
    }

    use constraint_validation::validate_type_constraints_node;
    pub(crate) use generic_arguments::{
        AppliedValueCarrier, check_closed_const_expression_in_carrier,
        const_argument_matches_parameter, const_integer_value_fits_primitive,
        exact_const_carrier_is_eligible, validate_const_argument_expression_formation,
        validate_const_binder_carriers, validate_exact_const_identity,
        validate_exact_const_identity_in_carrier, validate_exact_const_value_encoding_in_carrier,
        validate_exact_typed_structured_const_argument,
    };
    pub use generic_arguments::{check_closed_const_expression, normalize_closed_const_expression};
    use generic_arguments::{
        machine_argument_name, validate_const_data_argument, validate_generic_argument_bounds,
        validate_machine_data_argument, validate_symbolic_array_length,
    };
    pub use generic_arguments::{
        validate_closed_const_argument, validate_exact_const_value_encoding,
    };
    pub(crate) use generic_arguments::{
        validate_const_data_argument as validate_const_application_argument,
        validate_machine_data_argument as validate_machine_application_argument,
        validate_parameter_bounds,
    };
    pub use open_index_expressions::normalize_open_index_expressions;
    pub(crate) mod refinement_predicates {
        use checked_trees::{CheckedBooleanExpression, CheckedScalarExpression};
        use typed_trees::types::{
            DomainConstraintSubject, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
        };

        pub fn scalar_type_refinement_predicates(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
            subject: CheckedScalarExpression,
        ) -> Option<Vec<CheckedBooleanExpression>> {
            Some(scalar_type_refinements(program, reference, subject)?.predicates)
        }

        pub fn scalar_result_contract_is_represented(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> bool {
            let Some(primitive_type) = program.primitive_type_reference(reference) else {
                return false;
            };
            scalar_type_refinements(
                program,
                reference,
                CheckedScalarExpression::Parameter {
                    position: 0,
                    primitive_type,
                },
            )
            .is_some_and(|refinements| {
                let mut represented = crate::scalar_type_tags(program, reference);
                represented.extend(crate::scalar_type_routed_domains(program, reference));
                refinements
                    .semantic_atoms
                    .iter()
                    .all(|atom| represented.contains(atom))
            })
        }

        pub(crate) struct ScalarTypeRefinements {
            predicates: Vec<CheckedBooleanExpression>,
            semantic_atoms: Vec<(symbols::SymbolHandle, language_semantics::SemanticDomainId)>,
        }

        fn scalar_type_refinements(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
            subject: CheckedScalarExpression,
        ) -> Option<ScalarTypeRefinements> {
            let primitive = subject.primitive_type()?;
            if program.primitive_type_reference(reference) != Some(primitive) {
                return None;
            }
            let mut current = reference;
            let mut active = Vec::new();
            let mut predicates = Vec::new();
            let mut semantic_atoms = Vec::new();
            loop {
                if !program
                    .type_reference_table
                    .contains_type_reference(current)
                    || active.contains(&current)
                {
                    return None;
                }
                active.push(current);
                let (base_type, constraints) =
                    match program.type_reference_table.type_reference(current) {
                        TypeReferenceNode::Named { .. }
                        | TypeReferenceNode::DynamicTrait { .. } => {
                            return Some(ScalarTypeRefinements {
                                predicates,
                                semantic_atoms,
                            });
                        }
                        TypeReferenceNode::Constrained {
                            base_type,
                            constraints,
                        } => (*base_type, *constraints),
                        _ => return None,
                    };
                for constraint in program.type_reference_table.constraint_span(constraints)? {
                    let bounds = match constraint {
                        TypeConstraintNode::ArithmeticDomain(_) => continue,
                        TypeConstraintNode::Domain(domain) => match domain.subject {
                            DomainConstraintSubject::Value(domain) => {
                                predicates
                                    .push(scalar_value_domain_predicate(domain, subject.clone())?);
                                continue;
                            }
                            DomainConstraintSubject::Declared => {
                                if !domain.establishment_routes.is_empty() {
                                    predicates.push(crate::scalar_domain_value_predicate(
                                        program,
                                        domain,
                                        subject.clone(),
                                    )?);
                                    semantic_atoms.push((domain.symbol, domain.semantic_id));
                                    continue;
                                }
                                if let Some(bounds) = crate::exact_declared_domain_carrier_interval(
                                    program, primitive, domain,
                                ) {
                                    bounds
                                } else if domain.predicate_presence.is_present() {
                                    predicates.push(crate::scalar_domain_predicate(
                                        program,
                                        domain,
                                        subject.clone(),
                                    )?);
                                    continue;
                                } else {
                                    semantic_atoms.push((domain.symbol, domain.semantic_id));
                                    continue;
                                }
                            }
                            _ => return None,
                        },

                        _ => return None,
                    };
                    let minimum = crate::land_integer_value(&bounds.0, primitive)?;
                    let maximum = crate::land_integer_value(&bounds.1, primitive)?;
                    predicates.push(CheckedBooleanExpression::And {
                        left: Box::new(CheckedBooleanExpression::IntegerComparison {
                            kind: checked_trees::CheckedIntegerComparisonKind::LessOrEqual,
                            left: Box::new(CheckedScalarExpression::IntegerLiteral {
                                literal: minimum,
                            }),
                            right: Box::new(subject.clone()),
                        }),
                        right: Box::new(CheckedBooleanExpression::IntegerComparison {
                            kind: checked_trees::CheckedIntegerComparisonKind::LessOrEqual,
                            left: Box::new(subject.clone()),
                            right: Box::new(CheckedScalarExpression::IntegerLiteral {
                                literal: maximum,
                            }),
                        }),
                    });
                }
                current = base_type;
            }
        }

        pub fn scalar_value_domain_predicate(
            domain: language_semantics::value_domain::ValueDomain,
            subject: checked_trees::CheckedScalarExpression,
        ) -> Option<checked_trees::CheckedBooleanExpression> {
            use checked_trees::{
                CheckedBooleanExpression, CheckedIeeeFloatComparisonKind, CheckedScalarExpression,
            };
            use semantic_vocabulary::IeeeFloatValue;
            use typed_trees::types::PrimitiveType;
            let primitive_type = subject.primitive_type()?;
            match domain {
                language_semantics::value_domain::ValueDomain::Finite => {}
            }
            let (lowest, largest) = match primitive_type {
                PrimitiveType::F32 => (
                    IeeeFloatValue::Binary32((-f32::MAX).to_bits()),
                    IeeeFloatValue::Binary32(f32::MAX.to_bits()),
                ),
                PrimitiveType::F64 => (
                    IeeeFloatValue::Binary64((-f64::MAX).to_bits()),
                    IeeeFloatValue::Binary64(f64::MAX.to_bits()),
                ),
                _ => return None,
            };
            let subject = || Box::new(subject.clone());
            Some(CheckedBooleanExpression::And {
                left: Box::new(CheckedBooleanExpression::ScalarIeeeFloatComparison {
                    kind: CheckedIeeeFloatComparisonKind::LessOrEqual,
                    left: Box::new(CheckedScalarExpression::IeeeFloatLiteral { value: lowest }),
                    right: subject(),
                }),
                right: Box::new(CheckedBooleanExpression::ScalarIeeeFloatComparison {
                    kind: CheckedIeeeFloatComparisonKind::LessOrEqual,
                    left: subject(),
                    right: Box::new(CheckedScalarExpression::IeeeFloatLiteral { value: largest }),
                }),
            })
        }
    }
    pub use refinement_predicates::{
        scalar_result_contract_is_represented, scalar_type_refinement_predicates,
        scalar_value_domain_predicate,
    };
    pub(crate) mod float_ranges {
        use numerics::literals::FloatFormat;
        use semantic_vocabulary::IeeeFloatValue;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::types::PrimitiveType;

        pub fn closed_float_range_endpoint(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            primitive_type: PrimitiveType,
        ) -> Option<IeeeFloatValue> {
            if !program.expression_table.expression_is_valid(expression) {
                return None;
            }
            match program.expression_table.expression(expression) {
                ExpressionNode::Float(literal) => match primitive_type {
                    PrimitiveType::F32 => (literal.landing() != Some(FloatFormat::F64))
                        .then(|| IeeeFloatValue::Binary32(literal.value_f32().to_bits())),
                    PrimitiveType::F64 => {
                        Some(IeeeFloatValue::Binary64(literal.landed_f64().to_bits()))
                    }
                    _ => None,
                },
                ExpressionNode::Integer(_) | ExpressionNode::Name(_) => {
                    let value =
                        super::integer_ranges::closed_integer_range_bound(program, expression)?
                            .to_i64()?;
                    Some(match primitive_type {
                        PrimitiveType::F32 => IeeeFloatValue::Binary32((value as f32).to_bits()),
                        PrimitiveType::F64 => IeeeFloatValue::Binary64((value as f64).to_bits()),
                        _ => return None,
                    })
                }
                _ => None,
            }
        }

        pub fn ieee_float_range_ordered(minimum: IeeeFloatValue, maximum: IeeeFloatValue) -> bool {
            match (minimum, maximum) {
                (IeeeFloatValue::Binary32(low), IeeeFloatValue::Binary32(high)) => {
                    f32::from_bits(low) <= f32::from_bits(high)
                }
                (IeeeFloatValue::Binary64(low), IeeeFloatValue::Binary64(high)) => {
                    f64::from_bits(low) <= f64::from_bits(high)
                }
                _ => false,
            }
        }
    }
    pub use float_ranges::{closed_float_range_endpoint, ieee_float_range_ordered};
    pub(crate) mod integer_ranges {
        use numerics::bignum::BigInt;
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

        pub fn is_plain_scalar_storage(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> bool {
            program
                .type_reference_table
                .contains_type_reference(reference)
                && (matches!(
                    program.type_reference_table.type_reference(reference),
                    TypeReferenceNode::Named { .. } | TypeReferenceNode::DynamicTrait { .. }
                ) || is_arithmetic_policy_only_integer(program, reference))
        }

        pub fn is_arithmetic_policy_only_integer(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> bool {
            let table = &program.type_reference_table;
            if !table.contains_type_reference(reference) {
                return false;
            }
            let TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } = table.type_reference(reference)
            else {
                return false;
            };
            if !table.contains_type_reference(*base_type)
                || !matches!(
                    table.constraint_span(*constraints),
                    Some([TypeConstraintNode::ArithmeticDomain(_)])
                )
            {
                return false;
            }
            let TypeReferenceNode::Named { symbol, name } = table.type_reference(*base_type) else {
                return false;
            };
            program
                .symbols
                .builtin_type_atom(*symbol)
                .is_some_and(|atom| name.as_str() == atom.symbol_name() && atom.is_fixed_integer())
        }

        pub fn unrestricted_builtin_primitive(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> Option<typed_trees::types::PrimitiveType> {
            let table = &program.type_reference_table;
            if !table.contains_type_reference(reference) {
                return None;
            }
            let unrestricted = match table.type_reference(reference) {
                TypeReferenceNode::Named { symbol, name } => program
                    .symbols
                    .builtin_type_atom(*symbol)
                    .is_some_and(|atom| atom.symbol_name() == name.as_str()),
                TypeReferenceNode::DynamicTrait { .. } => true,
                _ => is_arithmetic_policy_only_integer(program, reference),
            };
            unrestricted
                .then(|| program.primitive_type_reference(reference))
                .flatten()
        }

        pub fn closed_integer_range_bound(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
        ) -> Option<BigInt> {
            program.closed_integer_expression_value(expression)
        }

        pub fn closed_integer_range_maximum(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            end_inclusive: bool,
        ) -> Option<BigInt> {
            program.closed_integer_range_endpoint(expression, end_inclusive)
        }
    }
    pub use integer_ranges::{
        closed_integer_range_bound, closed_integer_range_maximum,
        is_arithmetic_policy_only_integer, is_plain_scalar_storage, unrestricted_builtin_primitive,
    };
    pub(crate) use open_index_expressions::validate_indexed_qualification_arguments;

    use crate::StateSignatureOwner;
    use crate::declarations::symbols::TopLevelSymbols;
    use diagnostics::Diagnostic;
    use std::fmt;
    use typed_trees::data::{TypeParameter, TypeParameterKind};
    use typed_trees::type_identity::TypeIdentityRequest;
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

    #[derive(Debug, Clone, Copy)]
    pub enum StaticTypeArgumentOwner<'program> {
        Machine(&'program typed_trees::machine::Machine),
        Data(&'program typed_trees::data::DataDefinition),
        TraitSignature {
            declaration: &'program typed_trees::trait_definition::TraitDefinition,
            signature: &'program typed_trees::signature::StateSignature,
        },
        MachineSignature {
            declaration: &'program typed_trees::machine::Machine,
            signature: &'program typed_trees::signature::StateSignature,
        },
        Mathematical {
            declaration: &'program typed_trees::mathematical::MathematicalDefinition,
            binders: &'program [TypeParameter],
        },
        Constant {
            symbol: symbols::SymbolHandle,
            name: &'program str,
        },
    }

    #[derive(Debug, Clone, Copy)]
    pub(crate) enum TypeReferenceOwner<'program> {
        SuppliedOperand {
            declaration: &'program str,
            generic_depth: usize,
        },
        TypeAlias {
            name: &'program str,
            generic_depth: usize,
        },
        StaticMachineArgument {
            owner: StaticTypeArgumentOwner<'program>,
            generic_depth: usize,
        },
        DomainTarget {
            domain: &'program str,
            generic_depth: usize,
        },
        DataField {
            data: &'program str,
            field: &'program str,
            generic_depth: usize,
        },
        MachineOwnedData {
            machine: &'program str,
            data: &'program str,
            generic_depth: usize,
        },
        Expression {
            machine: &'program typed_trees::machine::Machine,
            current_state: Option<&'program typed_trees::state::State>,
            value_environment:
                &'program crate::proof_contracts::arithmetic_domains::ValueEnvironment,
            generic_depth: usize,
        },
        StateLocalData {
            machine: &'program typed_trees::machine::Machine,
            current_state: Option<&'program typed_trees::state::State>,
            value_environment:
                &'program crate::proof_contracts::arithmetic_domains::ValueEnvironment,
            state: &'program str,
            local: &'program str,
            generic_depth: usize,
        },
        StateParameter {
            signature: symbols::SymbolHandle,
            owner: StateSignatureOwner<'program>,
            state: &'program str,
            parameter: &'program str,
            generic_depth: usize,
        },
        StateReturn {
            signature: symbols::SymbolHandle,
            owner: StateSignatureOwner<'program>,
            state: &'program str,
            generic_depth: usize,
        },
        OperatorParameter {
            operator: &'program str,
            parameter: &'program str,
            generic_depth: usize,
        },
        OperatorReturn {
            operator: &'program str,
            generic_depth: usize,
        },
        TraitParent {
            trait_name: &'program str,
            parent: &'program str,
            generic_depth: usize,
        },
        GenericBinder {
            owner: &'program str,
            parameter: &'program str,
            generic_depth: usize,
        },
    }

    impl TypeReferenceOwner<'_> {
        fn is_runtime_value_position(&self) -> bool {
            matches!(
                self,
                Self::DataField { .. }
                    | Self::MachineOwnedData { .. }
                    | Self::StateLocalData { .. }
                    | Self::StateParameter { .. }
                    | Self::StateReturn { .. }
                    | Self::OperatorParameter { .. }
                    | Self::OperatorReturn { .. }
                    | Self::Expression { .. }
            )
        }

        fn generic_argument(self) -> Self {
            match self {
                Self::SuppliedOperand {
                    declaration,
                    generic_depth,
                } => Self::SuppliedOperand {
                    declaration,
                    generic_depth: generic_depth + 1,
                },
                Self::TypeAlias {
                    name,
                    generic_depth,
                } => Self::TypeAlias {
                    name,
                    generic_depth: generic_depth + 1,
                },
                Self::StaticMachineArgument {
                    owner,
                    generic_depth,
                } => Self::StaticMachineArgument {
                    owner,
                    generic_depth: generic_depth + 1,
                },
                Self::DomainTarget {
                    domain,
                    generic_depth,
                } => Self::DomainTarget {
                    domain,
                    generic_depth: generic_depth + 1,
                },
                Self::DataField {
                    data,
                    field,
                    generic_depth,
                } => Self::DataField {
                    data,
                    field,
                    generic_depth: generic_depth + 1,
                },
                Self::MachineOwnedData {
                    machine,
                    data,
                    generic_depth,
                } => Self::MachineOwnedData {
                    machine,
                    data,
                    generic_depth: generic_depth + 1,
                },
                Self::Expression {
                    machine,
                    current_state,
                    value_environment,
                    generic_depth,
                } => Self::Expression {
                    machine,
                    current_state,
                    value_environment,
                    generic_depth: generic_depth + 1,
                },
                Self::StateLocalData {
                    machine,
                    current_state,
                    value_environment,
                    state,
                    local,
                    generic_depth,
                } => Self::StateLocalData {
                    machine,
                    current_state,
                    value_environment,
                    state,
                    local,
                    generic_depth: generic_depth + 1,
                },
                Self::StateParameter {
                    signature,
                    owner,
                    state,
                    parameter,
                    generic_depth,
                } => Self::StateParameter {
                    signature,
                    owner,
                    state,
                    parameter,
                    generic_depth: generic_depth + 1,
                },
                Self::StateReturn {
                    signature,
                    owner,
                    state,
                    generic_depth,
                } => Self::StateReturn {
                    signature,
                    owner,
                    state,
                    generic_depth: generic_depth + 1,
                },
                Self::OperatorParameter {
                    operator,
                    parameter,
                    generic_depth,
                } => Self::OperatorParameter {
                    operator,
                    parameter,
                    generic_depth: generic_depth + 1,
                },
                Self::OperatorReturn {
                    operator,
                    generic_depth,
                } => Self::OperatorReturn {
                    operator,
                    generic_depth: generic_depth + 1,
                },
                Self::TraitParent {
                    trait_name,
                    parent,
                    generic_depth,
                } => Self::TraitParent {
                    trait_name,
                    parent,
                    generic_depth: generic_depth + 1,
                },
                Self::GenericBinder {
                    owner,
                    parameter,
                    generic_depth,
                } => Self::GenericBinder {
                    owner,
                    parameter,
                    generic_depth: generic_depth + 1,
                },
            }
        }
    }

    impl fmt::Display for TypeReferenceOwner<'_> {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            let generic_depth = match self {
                Self::SuppliedOperand {
                    declaration,
                    generic_depth,
                } => {
                    write!(formatter, "operand supplied by `{declaration}`")?;
                    *generic_depth
                }
                Self::TypeAlias {
                    name,
                    generic_depth,
                } => {
                    write!(formatter, "type alias `{name}` expansion")?;
                    *generic_depth
                }
                Self::StaticMachineArgument {
                    owner,
                    generic_depth,
                } => {
                    match owner {
                        StaticTypeArgumentOwner::Machine(machine) => {
                            write!(formatter, "machine `{}`", machine.name)?;
                        }
                        StaticTypeArgumentOwner::Data(data) => {
                            write!(formatter, "data `{}`", data.name)?;
                        }
                        StaticTypeArgumentOwner::TraitSignature {
                            declaration,
                            signature,
                        } => {
                            write!(
                                formatter,
                                "trait `{}` requirement `{}`",
                                declaration.name, signature.name
                            )?;
                        }
                        StaticTypeArgumentOwner::MachineSignature { signature, .. } => {
                            write!(
                                formatter,
                                "machine-parameter requirement `{}`",
                                signature.name
                            )?;
                        }
                        StaticTypeArgumentOwner::Mathematical { declaration, .. } => {
                            write!(formatter, "mathematical declaration `{}`", declaration.name)?;
                        }
                        StaticTypeArgumentOwner::Constant { name, .. } => {
                            write!(formatter, "constant `{name}`")?;
                        }
                    }
                    formatter.write_str(" static type argument")?;
                    *generic_depth
                }
                Self::DomainTarget {
                    domain,
                    generic_depth,
                } => {
                    write!(formatter, "domain `{domain}` target type")?;
                    *generic_depth
                }
                Self::DataField {
                    data,
                    field,
                    generic_depth,
                } => {
                    write!(formatter, "data `{data}` field `{field}`")?;
                    *generic_depth
                }
                Self::MachineOwnedData {
                    machine,
                    data,
                    generic_depth,
                } => {
                    write!(formatter, "machine `{machine}` owned data `{data}`")?;
                    *generic_depth
                }
                Self::Expression {
                    machine,
                    current_state,
                    generic_depth,
                    ..
                } => {
                    write!(formatter, "machine `{}`", machine.name)?;
                    if let Some(state) = current_state {
                        write!(formatter, " state `{}`", state.name)?;
                    }
                    formatter.write_str(" expression type application")?;
                    *generic_depth
                }
                Self::StateLocalData {
                    machine,
                    current_state: _,
                    value_environment: _,
                    state,
                    local,
                    generic_depth,
                } => {
                    write!(
                        formatter,
                        "machine `{}` state `{state}` local data `{local}`",
                        machine.name
                    )?;
                    *generic_depth
                }
                Self::StateParameter {
                    signature: _,
                    owner,
                    state,
                    parameter,
                    generic_depth,
                } => {
                    write!(formatter, "{owner} state `{state}` parameter `{parameter}`")?;
                    *generic_depth
                }
                Self::StateReturn {
                    signature: _,
                    owner,
                    state,
                    generic_depth,
                } => {
                    write!(formatter, "{owner} state `{state}` return type")?;
                    *generic_depth
                }
                Self::OperatorParameter {
                    operator,
                    parameter,
                    generic_depth,
                } => {
                    write!(formatter, "operator `{operator}` parameter `{parameter}`")?;
                    *generic_depth
                }
                Self::OperatorReturn {
                    operator,
                    generic_depth,
                } => {
                    write!(formatter, "operator `{operator}` return type")?;
                    *generic_depth
                }
                Self::TraitParent {
                    trait_name,
                    parent,
                    generic_depth,
                } => {
                    write!(
                        formatter,
                        "trait `{trait_name}` parent `{parent}` type argument"
                    )?;
                    *generic_depth
                }
                Self::GenericBinder {
                    owner,
                    parameter,
                    generic_depth,
                } => {
                    write!(formatter, "{owner} parameter `{parameter}`")?;
                    *generic_depth
                }
            };

            for _ in 0..generic_depth {
                formatter.write_str(" generic argument")?;
            }

            Ok(())
        }
    }

    pub fn validate_static_type_argument<'program>(
        program: &'program dyn crate::CheckingInput,
        caller: StaticTypeArgumentOwner<'program>,
        type_reference: TypeReferenceHandle,
        symbols: &TopLevelSymbols<'_>,
        diagnostics: &mut Vec<Diagnostic>,
        formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
    ) {
        let (type_parameters, lifetime_parameters, lexical_owner) = caller.parameters(program);
        validate_type_reference_handle(
            program,
            type_reference,
            symbols,
            diagnostics,
            TypeReferenceOwner::StaticMachineArgument {
                owner: caller,
                generic_depth: 0,
            },
            TypeParameterScope::authored(lexical_owner, &type_parameters, &lifetime_parameters),
            formation_obligations,
        );
    }

    pub(crate) fn validate_type_reference_handle(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
        symbols: &TopLevelSymbols<'_>,
        diagnostics: &mut Vec<Diagnostic>,
        owner: TypeReferenceOwner<'_>,
        type_parameter_scope: TypeParameterScope<'_>,
        formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
    ) {
        validate_boundary_trait_value_placement(program, type_reference, owner, diagnostics);

        validate_type_reference_node(
            program,
            type_reference,
            symbols,
            diagnostics,
            owner,
            type_parameter_scope,
            formation_obligations,
        );
    }

    fn validate_boundary_trait_value_placement(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
        owner: TypeReferenceOwner<'_>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. } => {
                validate_boundary_trait_value_placement(program, *referee, owner, diagnostics);
            }
            TypeReferenceNode::Constrained { base_type, .. } => {
                validate_boundary_trait_value_placement(program, *base_type, owner, diagnostics);
            }
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => {
                validate_boundary_trait_value_placement(program, *element_type, owner, diagnostics);
            }
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                arguments,
                ..
            } => {
                if owner.is_runtime_value_position() && is_boundary_trait(program, *base_symbol) {
                    push_boundary_trait_value_diagnostic(owner, base_name, diagnostics);
                    return;
                }
                for argument in program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                {
                    validate_boundary_trait_value_placement(
                        program,
                        *argument,
                        owner.generic_argument(),
                        diagnostics,
                    );
                }
            }
            TypeReferenceNode::Named { symbol, name } => {
                if owner.is_runtime_value_position() && is_boundary_trait(program, *symbol) {
                    push_boundary_trait_value_diagnostic(owner, name, diagnostics);
                    return;
                }
                if let Some(origin) =
                    crate::declarations::data_queries::data_definition_by_symbol(program, *symbol)
                        .and_then(|definition| definition.generic_instance)
                {
                    validate_boundary_trait_value_placement(
                        program,
                        origin,
                        owner.generic_argument(),
                        diagnostics,
                    );
                }
            }
            TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::Unit => {}
        }
    }

    fn is_boundary_trait(
        program: &dyn crate::CheckingInput,
        symbol: symbols::SymbolHandle,
    ) -> bool {
        program
            .traits()
            .iter()
            .any(|definition| definition.symbol == symbol && definition.is_boundary)
    }

    fn push_boundary_trait_value_diagnostic(
        owner: TypeReferenceOwner<'_>,
        name: impl std::fmt::Display,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        diagnostics.push(Diagnostic::error(format!(
        "{owner} uses boundary trait `{name}` as a value type; call a `{name}` requirement directly, or carry an ordinary handle, session, or `dyn` value for runtime multiplicity"
    )));
    }

    pub(crate) fn validate_generic_instance_argument_bounds(
        program: &dyn crate::CheckingInput,
        instance: &typed_trees::data::DataDefinition,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(origin) = instance.generic_instance else {
            return;
        };
        let TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            arguments,
            ..
        } = program.type_reference_table.type_reference(origin)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "generated data instance `{}` has a non-generic derivation origin",
                instance.name
            )));
            return;
        };
        validate_generic_argument_bounds(
            program,
            *base_symbol,
            base_name.as_str(),
            *arguments,
            TypeParameterScope::authored(instance.symbol, &[], &instance.lifetime_parameters),
            diagnostics,
        );
    }

    fn validate_type_reference_node(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
        symbols: &TopLevelSymbols<'_>,
        diagnostics: &mut Vec<Diagnostic>,
        owner: TypeReferenceOwner<'_>,
        mut type_parameter_scope: TypeParameterScope<'_>,
        formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
    ) {
        let supplying_owner = program
            .type_reference_table
            .operand_lifetime_owner(type_reference);
        if supplying_owner.is_valid() {
            type_parameter_scope.operand_lifetime_owner = supplying_owner;
        }
        alias_arguments::validate(
            program,
            type_reference,
            symbols,
            owner,
            type_parameter_scope,
            diagnostics,
            formation_obligations,
        );
        macro_rules! validate_node {
            ($type_reference:expr $(,)?) => {
                validate_type_reference_node(
                    program,
                    $type_reference,
                    symbols,
                    diagnostics,
                    owner,
                    type_parameter_scope,
                    formation_obligations,
                )
            };
        }
        let validate_generic_argument =
            |argument: TypeReferenceHandle,
             diagnostics: &mut Vec<Diagnostic>,
             formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>| {
                validate_type_reference_node(
                    program,
                    argument,
                    symbols,
                    diagnostics,
                    owner.generic_argument(),
                    type_parameter_scope,
                    formation_obligations,
                );
            };
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference {
                referee, lifetime, ..
            } => {
                if let Some(lifetime) = lifetime
                    && !type_parameter_scope.contains_operand_lifetime(
                        program,
                        type_reference,
                        0,
                        lifetime.as_str(),
                    )
                {
                    diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses undeclared lifetime `'{}'; declare it in the owner's generic parameter list",
                    lifetime.as_str()
                )));
                }
                validate_node!(*referee);
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                validate_node!(*base_type);
                validate_type_constraints_node(
                    program,
                    *base_type,
                    *constraints,
                    diagnostics,
                    owner,
                    type_parameter_scope,
                );
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                validate_symbolic_array_length(
                    program,
                    length,
                    type_parameter_scope,
                    diagnostics,
                    owner,
                    formation_obligations,
                );

                validate_node!(*element_type);
            }
            TypeReferenceNode::Slice { element_type } => {
                validate_node!(*element_type);
            }
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                lifetime_arguments,
                arguments,
                ..
            } => {
                if type_parameter_scope
                    .type_parameters
                    .iter()
                    .any(|parameter| {
                        matches!(parameter.kind, TypeParameterKind::Type)
                            && ((base_symbol.is_valid() && parameter.symbol == *base_symbol)
                                || (!program.symbols.has_source_metadata()
                                    && parameter.name.as_str() == base_name.as_str()))
                    })
                {
                    diagnostics.push(Diagnostic::error(format!(
                    "{owner} cannot apply generic parameter `{base_name}` as a generic constructor"
                )));
                }
                let base_resolves = symbols.has_type_symbol(*base_symbol)
                    || PrimitiveType::from_name(base_name.as_str()).is_some()
                    || (!program.symbols.has_source_metadata() && symbols.has_type(base_name));
                if !base_resolves && !type_parameter_scope.contains(base_name.as_str()) {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} references unknown generic type `{base_name}`"
                    )));
                }

                for (slot, lifetime) in lifetime_arguments.iter().enumerate() {
                    if !type_parameter_scope.contains_operand_lifetime(
                        program,
                        type_reference,
                        slot,
                        lifetime.as_str(),
                    ) {
                        diagnostics.push(Diagnostic::error(format!(
                        "{owner} uses undeclared lifetime argument `'{}' in `{base_name}`; declare it in the owner's generic parameter list",
                        lifetime.as_str()
                    )));
                    }
                }

                let argument_handles = program
                    .type_reference_table
                    .type_reference_handles(*arguments);
                let definition = program.data_definitions().iter().find(|definition| {
                    definition.symbol == *base_symbol
                        || (!program.symbols.has_source_metadata()
                            && definition.name.as_str() == base_name.as_str())
                });
                if let Some(definition) = definition {
                    validate_generic_argument_bounds(
                        program,
                        *base_symbol,
                        base_name.as_str(),
                        *arguments,
                        type_parameter_scope,
                        diagnostics,
                    );
                    if definition.lifetime_parameters.len() != lifetime_arguments.len() {
                        diagnostics.push(Diagnostic::error(format!(
                            "data `{base_name}` expected {} lifetime arguments but got {}",
                            definition.lifetime_parameters.len(),
                            lifetime_arguments.len()
                        )));
                    }
                    let parameters = program.data_type_parameters(definition);
                    if parameters.len() != argument_handles.len() {
                        diagnostics.push(Diagnostic::error(format!(
                            "generic data `{base_name}` expected {} arguments but got {}",
                            parameters.len(),
                            argument_handles.len()
                        )));
                    }
                    crate::declarations::traits::validate_implementation_application(
                        program,
                        definition.symbol,
                        parameters,
                        &definition.lifetime_parameters,
                        &definition.conformance_bounds,
                        argument_handles,
                        lifetime_arguments,
                        if type_parameter_scope.operand_lifetime_owner.is_valid() {
                            type_parameter_scope.operand_lifetime_owner
                        } else {
                            type_parameter_scope.lexical_owner
                        },
                        &format!("data `{base_name}`"),
                        diagnostics,
                    );
                    for (parameter, argument) in parameters.iter().zip(argument_handles) {
                        match &parameter.kind {
                            TypeParameterKind::Implementation => {
                                implementation_arguments::validate_operand_formation(
                                    program,
                                    *argument,
                                    symbols,
                                    owner.generic_argument(),
                                    type_parameter_scope,
                                    diagnostics,
                                    formation_obligations,
                                );
                            }
                            TypeParameterKind::Const { type_reference }
                            | TypeParameterKind::Value { type_reference } => {
                                validate_const_data_argument(
                                    program,
                                    base_name,
                                    parameter,
                                    AppliedValueCarrier::applied(
                                        *type_reference,
                                        parameters,
                                        argument_handles,
                                    ),
                                    *argument,
                                    type_parameter_scope,
                                    &owner.value_formation_facts(program),
                                    owner.local_value_formation(),
                                    diagnostics,
                                    formation_obligations,
                                );
                            }
                            TypeParameterKind::Machine { contract } => {
                                validate_machine_data_argument(
                                    program,
                                    *base_symbol,
                                    base_name,
                                    parameter,
                                    contract,
                                    *argument,
                                    argument_handles,
                                    type_parameter_scope,
                                    diagnostics,
                                );
                            }
                            TypeParameterKind::Proposition { .. } => {
                                diagnostics.push(Diagnostic::error(format!(
                                "proposition parameter `{}` of `{base_name}` requires a proposition-family argument, not a type-reference argument",
                                parameter.name
                            )));
                            }
                            TypeParameterKind::Type => {
                                if machine_argument_name(program, *argument, type_parameter_scope)
                                    .is_some()
                                {
                                    diagnostics.push(Diagnostic::error(format!(
                                    "type parameter `{}` of `{base_name}` was instantiated with a machine symbol; declare it as `<machine {}>` with a `requires machine` contract",
                                    parameter.name, parameter.name
                                )));
                                    continue;
                                }
                                validate_generic_argument(
                                    *argument,
                                    diagnostics,
                                    formation_obligations,
                                );
                            }
                        }
                    }
                } else if let Some(definition) = program.trait_definition_by_symbol(*base_symbol) {
                    if definition.lifetime_parameters.len() != lifetime_arguments.len() {
                        diagnostics.push(Diagnostic::error(format!(
                            "trait `{base_name}` expected {} lifetime arguments but got {}",
                            definition.lifetime_parameters.len(),
                            lifetime_arguments.len()
                        )));
                    }
                    crate::declarations::traits::validate_trait_application_obligations(
                        program,
                        definition,
                        argument_handles,
                        lifetime_arguments,
                        base_name.as_str(),
                        symbols,
                        type_parameter_scope,
                        diagnostics,
                        formation_obligations,
                    );
                } else {
                    for argument in argument_handles {
                        validate_generic_argument(*argument, diagnostics, formation_obligations);
                    }
                }
            }
            TypeReferenceNode::DynamicTrait {
                symbol,
                name,
                arguments,
            } => {
                let trait_definition = symbols.trait_definition_by_symbol(*symbol).or_else(|| {
                    (!program.symbols.has_source_metadata())
                        .then(|| symbols.trait_definition(name.as_str()))
                        .flatten()
                });
                let Some(trait_definition) = trait_definition else {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} references unknown dynamic trait `{name}`"
                    )));
                    return;
                };

                if trait_definition.is_boundary {
                    diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses boundary trait `{name}` as a local dynamic value; local dynamic selectors cannot cross a replaceable component boundary"
                )));
                }

                let parameter_count = program.trait_type_parameters(trait_definition).len();
                let arguments = program
                    .type_reference_table
                    .type_reference_handles(*arguments);
                if arguments.len() != parameter_count {
                    diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses `dyn {name}` with {} argument(s); `{name}` declares {parameter_count} trait parameter(s)",
                    arguments.len()
                )));
                }
                crate::declarations::traits::validate_trait_application_obligations(
                    program,
                    trait_definition,
                    arguments,
                    &[],
                    name.as_str(),
                    symbols,
                    type_parameter_scope,
                    diagnostics,
                    formation_obligations,
                );
            }
            TypeReferenceNode::Named { symbol, name } => {
                if type_parameter_scope
                    .type_parameters
                    .iter()
                    .any(|parameter| {
                        matches!(
                            parameter.kind,
                            TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. }
                        ) && ((symbol.is_valid() && parameter.symbol == *symbol)
                            || (!program.symbols.has_source_metadata()
                                && parameter.name.as_str() == name.as_str()))
                    })
                {
                    diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses value parameter `{name}` as a type; const and runtime value parameters supply values, not types"
                )));
                    return;
                }
                if type_parameter_scope
                    .machine_parameter(*symbol, name.as_str())
                    .is_some()
                {
                    diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses machine parameter `{name}` as a stored/runtime type; static machine parameters may only index a generic proof-data family or specialize a direct call"
                )));
                    return;
                }
                if symbol.is_valid()
                    && matches!(
                        program.symbols.get(*symbol).kind,
                        symbols::SymbolKind::Conformance
                            | symbols::SymbolKind::ConformanceParameter
                    )
                {
                    diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses implementation `{name}` as a stored/runtime type; implementations select code, not data"
                )));
                    return;
                }
                let type_resolves = symbols.has_type_symbol(*symbol)
                    || PrimitiveType::from_name(name.as_str()).is_some()
                    || (!program.symbols.has_source_metadata() && symbols.has_type(name));
                if !type_resolves && !type_parameter_scope.contains(name.as_str()) {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} references unknown data type `{name}`"
                    )));
                }
            }
            TypeReferenceNode::ConstExpression(expression) => {
                diagnostics.push(Diagnostic::error(format!(
                "{owner} uses proof-static index expression `{}` as a runtime type; computed const expressions are valid only inside an indexed-domain argument pack",
                program.expression_table.display_name(*expression)
            )));
            }
            TypeReferenceNode::Unit => {}
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub(crate) struct TypeParameterScope<'program> {
        pub(crate) operand_lifetime_owner: symbols::SymbolHandle,
        pub(crate) lexical_owner: symbols::SymbolHandle,
        pub(crate) type_parameters: &'program [TypeParameter],
        pub(crate) lifetime_parameters: &'program [typed_trees::name::Identifier],
    }

    impl<'program> TypeParameterScope<'program> {
        pub(crate) fn authored(
            lexical_owner: symbols::SymbolHandle,
            type_parameters: &'program [TypeParameter],
            lifetime_parameters: &'program [typed_trees::name::Identifier],
        ) -> Self {
            Self {
                operand_lifetime_owner: symbols::SymbolHandle::invalid(),
                lexical_owner,
                type_parameters,
                lifetime_parameters,
            }
        }

        fn contains(self, name: &str) -> bool {
            self.type_parameters
                .iter()
                .any(|parameter| parameter.name.as_str() == name)
        }

        fn contains_operand_lifetime(
            self,
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
            slot: usize,
            name: &str,
        ) -> bool {
            let retained = program.type_reference_table.operand_lifetimes(reference);
            if !retained.is_empty() {
                let expected = match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Generic {
                        lifetime_arguments, ..
                    } => lifetime_arguments.len(),
                    TypeReferenceNode::Reference {
                        lifetime: Some(_), ..
                    } => 1,
                    _ => 0,
                };
                if retained.len() != expected {
                    return false;
                }
                if let Some(region) = retained
                    .get(slot)
                    .filter(|region| region.declaration.is_valid())
                {
                    return typed_trees::type_identity::lifetime_parameter_name(
                        program.typed_program(),
                        *region,
                    )
                    .is_some_and(|parameter| parameter.as_str() == name);
                }
            }
            self.contains_lifetime(program, name)
        }

        fn contains_lifetime(self, program: &dyn crate::CheckingInput, name: &str) -> bool {
            let supplying_owner = self.operand_lifetime_owner;
            if supplying_owner.is_valid() {
                return typed_trees::type_identity::lifetime_parameter_identity(
                    program.typed_program(),
                    supplying_owner,
                    name,
                )
                .is_some();
            }
            self.lifetime_parameters
                .iter()
                .any(|parameter| parameter.as_str() == name)
        }

        fn machine_parameter(
            self,
            symbol: symbols::SymbolHandle,
            name: &str,
        ) -> Option<&'program TypeParameter> {
            self.type_parameters.iter().find(|parameter| {
                matches!(parameter.kind, TypeParameterKind::Machine { .. })
                    && ((symbol.is_valid() && parameter.symbol == symbol)
                        || parameter.name.as_str() == name)
            })
        }
    }

    pub(crate) fn type_references_match(
        program: &dyn crate::CheckingInput,
        actual: TypeReferenceHandle,
        required: TypeReferenceHandle,
    ) -> bool {
        if !actual.is_valid() || !required.is_valid() {
            return actual.is_valid() == required.is_valid();
        }
        if actual == required {
            return true;
        }

        program.normalized_type_identity(actual) == program.normalized_type_identity(required)
    }

    pub(crate) fn substituted_identity(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
        substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
    ) -> typed_trees::type_identity::NormalizedTypeIdentity {
        program.type_identity(TypeIdentityRequest {
            substitutions,
            ..TypeIdentityRequest::ordinary(type_reference)
        })
    }

    pub(crate) fn type_reference_label(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> String {
        if type_reference.is_valid() {
            program.display_type_reference_with_constraints(type_reference)
        } else {
            "()".to_owned()
        }
    }

    pub(crate) use formation::validate_expression_type_application;
}

pub mod wire {
    pub(crate) mod compatibility {
        use diagnostics::Diagnostic;

        pub(crate) mod demands {
            use super::migration::{MigrationRouteSearch, migration_route, retired_identity_reuse};
            use super::schemas::{SchemaRow, qualified_schema_path};
            use super::{
                WireCompatibilityDemand, WireDemandVerdict, WireFact, WireField, WireShape,
            };

            pub(crate) fn compatibility_demand_verdict(
                typed: &dyn crate::CheckingInput,
                rows: &[SchemaRow],
                demand: &WireCompatibilityDemand,
            ) -> WireDemandVerdict {
                let local = select_era_path(
                    &demand.local_schema,
                    rows.iter().map(|row| (row.qualified_path.as_str(), row)),
                );
                let peer = select_era_path(
                    &demand.peer_schema,
                    rows.iter().map(|row| (row.qualified_path.as_str(), row)),
                );
                let local_row = era_resolved(&local).copied();
                let peer_row = era_resolved(&peer).copied();
                let local_schema = local_row.map(|row| &row.entry);
                let peer_schema = peer_row.map(|row| &row.entry);
                let codec = local_schema
                    .and_then(|schema| schema.encoding.as_deref())
                    .or_else(|| peer_schema.and_then(|schema| schema.encoding.as_deref()))
                    .unwrap_or("compact_binary")
                    .to_owned();
                let compact_binary = codec == "compact_binary";

                let readability_value = local_schema
                    .zip(peer_schema)
                    .is_some_and(|(reader, writer)| schema_accepts(reader, writer));
                let writability_value = local_schema
                    .zip(peer_schema)
                    .is_some_and(|(writer, reader)| schema_accepts(reader, writer));
                let readable_detail = match (local_schema, peer_schema) {
                    (Some(_), Some(_)) if readability_value => {
                        "the local decoder accepts every peer shape".to_owned()
                    }
                    (Some(_), Some(_)) => {
                        "the strict local decoder does not accept every peer shape".to_owned()
                    }
                    _ => schema_selection_detail(&local, &peer, demand),
                };
                let writable_detail = match (local_schema, peer_schema) {
                    (Some(_), Some(_)) if writability_value => {
                        "the peer decoder accepts every local shape".to_owned()
                    }
                    (Some(_), Some(_)) => {
                        "the strict peer decoder does not accept every local shape".to_owned()
                    }
                    _ => schema_selection_detail(&local, &peer, demand),
                };

                let migration_route = migration_route(
                    typed,
                    &demand.lineage,
                    &demand.peer_schema,
                    &demand.local_schema,
                );
                let identity_reuse = match &migration_route {
                    MigrationRouteSearch::Complete(route) => {
                        retired_identity_reuse(typed, &route.eras)
                    }
                    _ => None,
                };
                let migration_value = local_schema.is_some()
                    && peer_schema.is_some()
                    && matches!(migration_route, MigrationRouteSearch::Complete(_))
                    && identity_reuse.is_none();
                let migration_detail = match (local_schema, peer_schema, &migration_route) {
                    (None, _, _) | (_, None, _) => schema_selection_detail(&local, &peer, demand),
                    (Some(_), Some(_), MigrationRouteSearch::Missing) => {
                        format!(
                            "no complete `{}` migration route exists from `{}` to `{}`",
                            demand.lineage, demand.peer_schema, demand.local_schema
                        )
                    }
                    (Some(_), Some(_), MigrationRouteSearch::Ambiguous(detail)) => detail.clone(),
                    (Some(_), Some(_), MigrationRouteSearch::Complete(route)) => {
                        if let Some(reuse) = identity_reuse {
                            format!("the selected route is not a sound migration: {reuse}")
                        } else if route.machines.is_empty() {
                            "peer and local schemas are identical; no migration edge is needed"
                                .to_owned()
                        } else {
                            format!("selected checked route: {}", route.machines.join(" -> "))
                        }
                    }
                };

                let readability = fact(demand.require_readable, readability_value, readable_detail);
                let writability = fact(demand.require_writable, writability_value, writable_detail);
                let preserving_decode = local_row
                    .and_then(|row| row.value_symbol)
                    .and_then(|symbol| published_preserving_decode(typed, symbol));
                let unknown_preservation = fact(
                    demand.require_unknown_preservation,
                    preserving_decode.is_some(),
                    if let Some(detail) = preserving_decode {
                        detail
                    } else if compact_binary {
                        "compact_binary publishes strict unknown-member behavior".to_owned()
                    } else {
                        format!("codec `{codec}` publishes no preserving behavior")
                    },
                );
                let canonical_value =
                    compact_binary && local_schema.is_some() && peer_schema.is_some();
                let canonical_detail = if local_schema.is_none() || peer_schema.is_none() {
                    schema_selection_detail(&local, &peer, demand)
                } else if compact_binary {
                    "compact_binary emits its canonical field order and scalar encodings".to_owned()
                } else {
                    format!("codec `{codec}` publishes no canonicalization guarantee")
                };
                let canonicality =
                    fact(demand.require_canonical, canonical_value, canonical_detail);
                let migration_coverage = fact(
                    demand.require_complete_migration,
                    migration_value,
                    migration_detail,
                );
                let satisfied = [
                    &readability,
                    &writability,
                    &unknown_preservation,
                    &canonicality,
                    &migration_coverage,
                ]
                .into_iter()
                .all(|fact| !fact.required || fact.satisfied);

                WireDemandVerdict {
                    edge: demand.edge.clone(),
                    local_schema: demand.local_schema.clone(),
                    peer_schema: demand.peer_schema.clone(),
                    readability,
                    writability,
                    unknown_preservation,
                    canonicality,
                    migration_coverage,
                    satisfied,
                }
            }

            fn published_preserving_decode(
                typed: &dyn crate::CheckingInput,
                value_symbol: symbols::SymbolHandle,
            ) -> Option<String> {
                typed
                    .machines()
                    .iter()
                    .flat_map(|machine| {
            typed
                .machine_trait_conformances(machine)
                .iter()
                .map(move |conformance| (machine, conformance))
                    })
                    .filter(|(machine, conformance)| {
            let Some(typed_trees::machine::SatisfiedDeclaration::Trait {
                definition,
                requirement,
            }) = typed_trees::machine::resolve_satisfied_declaration(
                typed, machine, conformance,
            )
            else {
                return false;
            };
            if definition.name.as_str() != "PreservingDecode"
                || requirement.name.as_str() != "decode_preserving"
            {
                return false;
            }
            typed
                .type_reference_table
                .type_reference_handles(conformance.arguments)
                .get(1)
                .is_some_and(|value| {
                    typed.type_reference_table.type_symbol(*value) == value_symbol
                })
                    })
                    .map(|(machine, _)| {
            format!(
                "codec publishes preserving decode: `{}` satisfies `PreservingDecode::decode_preserving` for the local schema's value type",
                qualified_schema_path(typed, machine.symbol, machine.name.as_str())
            )
                    })
                    .next()
            }

            fn fact(required: bool, satisfied: bool, detail: String) -> WireFact {
                WireFact {
                    required,
                    satisfied,
                    detail,
                }
            }

            pub(crate) enum EraSelection<T> {
                Resolved(T),
                Ambiguous(Vec<String>),
                Missing,
            }

            fn era_resolved<T>(selection: &EraSelection<T>) -> Option<&T> {
                match selection {
                    EraSelection::Resolved(item) => Some(item),
                    _ => None,
                }
            }

            pub(crate) fn select_era_path<'a, T>(
                requested: &str,
                candidates: impl Iterator<Item = (&'a str, T)>,
            ) -> EraSelection<T> {
                let nested_suffix = format!("::{requested}");
                let mut exact = Vec::new();
                let mut nested = Vec::new();
                for (path, item) in candidates {
                    if path == requested {
                        exact.push((path, item));
                    } else if path.ends_with(&nested_suffix) {
                        nested.push((path, item));
                    }
                }
                let (paths, items): (Vec<&str>, Vec<T>) = if exact.is_empty() {
                    nested.into_iter().unzip()
                } else {
                    exact.into_iter().unzip()
                };
                match items.len() {
                    0 => EraSelection::Missing,
                    1 => EraSelection::Resolved(items.into_iter().next().expect("one item")),
                    _ => EraSelection::Ambiguous(
                        paths
                            .into_iter()
                            .map(str::to_owned)
                            .collect::<Vec<String>>(),
                    ),
                }
            }

            fn schema_selection_detail<T>(
                local: &EraSelection<T>,
                peer: &EraSelection<T>,
                demand: &WireCompatibilityDemand,
            ) -> String {
                if matches!(local, EraSelection::Missing) && matches!(peer, EraSelection::Missing) {
                    return format!(
                        "neither local schema `{}` nor peer schema `{}` is published",
                        demand.local_schema, demand.peer_schema
                    );
                }
                [
                    ("local", demand.local_schema.as_str(), local),
                    ("peer", demand.peer_schema.as_str(), peer),
                ]
                .into_iter()
                .filter_map(|(role, authored, selection)| match selection {
                    EraSelection::Resolved(_) => None,
                    EraSelection::Missing => {
            Some(format!("{role} schema `{authored}` is not published"))
                    }
                    EraSelection::Ambiguous(candidates) => Some(format!(
            "{role} schema `{authored}` is ambiguous across published eras {}; qualify the era path in the demand",
            candidates
                .iter()
                .map(|candidate| format!("`{candidate}`"))
                .collect::<Vec<_>>()
                .join(", ")
                    )),
                })
                .collect::<Vec<_>>()
                .join("; ")
            }

            pub(crate) fn name_leaf(name: &str) -> &str {
                name.rsplit("::").next().unwrap_or(name)
            }

            pub(crate) fn schema_accepts(reader: &WireShape, writer: &WireShape) -> bool {
                if !reader.cases.is_empty() || !writer.cases.is_empty() {
                    return writer.cases.iter().all(|writer_case| {
                        reader.cases.iter().any(|reader_case| {
                            reader_case.number == writer_case.number
                                && fields_equal(
                                    &reader_case.payload_fields,
                                    &writer_case.payload_fields,
                                )
                        })
                    });
                }
                fields_equal(&reader.fields, &writer.fields)
            }

            pub(crate) fn fields_equal(left: &[WireField], right: &[WireField]) -> bool {
                let left_relevant = left
                    .iter()
                    .filter(|field| !field.relevance.is_erased())
                    .collect::<Vec<_>>();
                let right_relevant = right
                    .iter()
                    .filter(|field| !field.relevance.is_erased())
                    .collect::<Vec<_>>();
                left_relevant.len() == right_relevant.len()
                    && left_relevant.iter().all(|left_field| {
                        right_relevant.iter().any(|right_field| {
                            left_field.number == right_field.number
                                && left_field.type_display == right_field.type_display
                        })
                    })
            }
        }
        pub(crate) mod migration {
            use super::demands::{EraSelection, name_leaf, select_era_path};
            use super::schemas::qualified_schema_path;
            use symbols::SymbolHandle;

            fn resolve_declared_era(
                typed: &dyn crate::CheckingInput,
                requested: &str,
            ) -> Option<SymbolHandle> {
                let candidates = typed
                    .data_definitions()
                    .iter()
                    .map(|data| (typed.symbols.display_path(data.symbol, "::"), data.symbol))
                    .collect::<Vec<_>>();
                match select_era_path(
                    requested,
                    candidates
                        .iter()
                        .map(|(path, symbol)| (path.as_str(), *symbol)),
                ) {
                    EraSelection::Resolved(symbol) => Some(symbol),
                    _ => None,
                }
            }

            pub(crate) struct MigrationRoute {
                pub(crate) machines: Vec<String>,
                pub(crate) eras: Vec<SymbolHandle>,
            }

            pub(crate) enum MigrationRouteSearch {
                Complete(MigrationRoute),
                Ambiguous(String),
                Missing,
            }

            pub(crate) fn migration_route(
                typed: &dyn crate::CheckingInput,
                lineage: &str,
                peer: &str,
                local: &str,
            ) -> MigrationRouteSearch {
                let (Some(local_symbol), Some(peer_symbol), Some(lineage_symbol)) = (
                    resolve_declared_era(typed, local),
                    resolve_declared_era(typed, peer),
                    resolve_declared_era(typed, lineage),
                ) else {
                    return MigrationRouteSearch::Missing;
                };
                if peer_symbol == local_symbol {
                    return MigrationRouteSearch::Complete(MigrationRoute {
                        machines: Vec::new(),
                        eras: vec![local_symbol],
                    });
                }
                let mut edges = Vec::new();
                for machine in typed.machines() {
                    for conformance in typed.machine_trait_conformances(machine) {
                        if name_leaf(conformance.name.as_str()) != "FormatMigration"
                            || conformance.requirement.as_ref().map(|name| name.as_str())
                                != Some("migrate")
                        {
                            continue;
                        }
                        let arguments = typed
                            .type_reference_table
                            .type_reference_handles(conformance.arguments);
                        if arguments.len() != 3 {
                            continue;
                        }
                        let symbol =
                            |index: usize| typed.type_reference_table.type_symbol(arguments[index]);
                        if symbol(0) != lineage_symbol {
                            continue;
                        }
                        let (old, new) = (symbol(1), symbol(2));
                        if !old.is_valid() || !new.is_valid() {
                            continue;
                        }
                        edges.push((old, new, machine.name.as_str().to_owned()));
                    }
                }

                let uniquely_bound = |(old, new): &(SymbolHandle, SymbolHandle)| {
                    edges
                        .iter()
                        .filter(|edge| (edge.0, edge.1) == (*old, *new))
                        .count()
                        == 1
                };
                let certified_edges = edges
                    .iter()
                    .filter(|(old, new, _)| uniquely_bound(&(*old, *new)))
                    .cloned()
                    .collect::<Vec<_>>();
                if let Some(route) = search_route(&certified_edges, peer_symbol, local_symbol) {
                    return MigrationRouteSearch::Complete(route);
                }
                if let Some(route) = search_route(&edges, peer_symbol, local_symbol) {
                    let hops = route
                        .eras
                        .windows(2)
                        .filter_map(|hop| {
                            let machines = edges
                                .iter()
                                .filter(|edge| (edge.0, edge.1) == (hop[0], hop[1]))
                                .map(|edge| format!("`{}`", edge.2))
                                .collect::<Vec<_>>();
                            (machines.len() > 1).then(|| {
                                format!(
                                    "edge `{}` -> `{}` is bound by {}",
                                    typed.symbols.display_path(hop[0], "::"),
                                    typed.symbols.display_path(hop[1], "::"),
                                    machines.join(", ")
                                )
                            })
                        })
                        .collect::<Vec<_>>()
                        .join("; ");
                    return MigrationRouteSearch::Ambiguous(format!(
                        "no uniquely bound `{lineage}` migration route exists from `{peer}` to `{local}`: \
             {hops}; bind exactly one machine to each checked conversion edge"
                    ));
                }
                MigrationRouteSearch::Missing
            }

            pub(crate) fn search_route(
                edges: &[(SymbolHandle, SymbolHandle, String)],
                peer: SymbolHandle,
                local: SymbolHandle,
            ) -> Option<MigrationRoute> {
                let mut frontier = vec![(peer, Vec::<String>::new(), vec![peer])];
                let mut visited = vec![peer];
                while let Some((current, machines, eras)) = frontier.pop() {
                    for (old, new, machine) in edges {
                        if *old != current {
                            continue;
                        }
                        let mut next_machines = machines.clone();
                        next_machines.push(machine.clone());
                        let mut next_eras = eras.clone();
                        next_eras.push(*new);
                        if *new == local {
                            return Some(MigrationRoute {
                                machines: next_machines,
                                eras: next_eras,
                            });
                        }
                        if !visited.contains(new) {
                            visited.push(*new);
                            frontier.push((*new, next_machines, next_eras));
                        }
                    }
                }
                None
            }

            #[derive(Clone, Copy, PartialEq, Eq)]
            pub(crate) enum IdentityScope {
                Fields,
                Cases,
                Payload(u64),
            }

            pub(crate) struct NumberScope {
                key: IdentityScope,
                live: Vec<u64>,
                retired: Vec<u64>,
            }

            fn era_number_scopes(
                typed: &dyn crate::CheckingInput,
                era: SymbolHandle,
            ) -> Option<(String, Vec<NumberScope>)> {
                let definition = typed.data_definition_by_symbol(era)?;
                let members = typed.data_members(definition);
                let has_fields = members
                    .iter()
                    .any(|member| matches!(member, typed_trees::data::DataMember::Field(_)));
                let has_cases = members
                    .iter()
                    .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_)));
                let mut scopes = vec![
                    NumberScope {
                        key: IdentityScope::Fields,
                        live: Vec::new(),
                        retired: if has_cases {
                            Vec::new()
                        } else {
                            definition.retired_identities.clone()
                        },
                    },
                    NumberScope {
                        key: IdentityScope::Cases,
                        live: Vec::new(),
                        retired: if has_fields {
                            Vec::new()
                        } else {
                            definition.retired_identities.clone()
                        },
                    },
                ];
                for member in members {
                    match member {
                        typed_trees::data::DataMember::Field(field) => {
                            if let Some(identity) = field.identity {
                                scopes[0].live.push(identity);
                            }
                        }
                        typed_trees::data::DataMember::Variant(variant) => {
                            if let Some(identity) = variant.identity {
                                scopes[1].live.push(identity);
                                scopes.push(NumberScope {
                                    key: IdentityScope::Payload(identity),
                                    live: typed
                                        .data_payload_fields(variant)
                                        .iter()
                                        .filter_map(|field| field.identity)
                                        .collect(),
                                    retired: variant.retired_payload_identities.clone(),
                                });
                            }
                        }
                    }
                }
                Some((
                    qualified_schema_path(typed, era, definition.name.as_str()),
                    scopes,
                ))
            }

            pub(crate) fn retired_identity_reuse(
                typed: &dyn crate::CheckingInput,
                eras: &[SymbolHandle],
            ) -> Option<String> {
                let mut retired: Vec<(IdentityScope, u64, String)> = Vec::new();
                let mut previous_live: Vec<(IdentityScope, Vec<u64>)> = Vec::new();
                for era in eras {
                    let Some((era_name, scopes)) = era_number_scopes(typed, *era) else {
                        continue;
                    };
                    for scope in &scopes {
                        for identity in &scope.live {
                            if let Some((_, _, retiring)) =
                                retired.iter().find(|(key, candidate, _)| {
                                    *key == scope.key && candidate == identity
                                })
                            {
                                return Some(match scope.key {
                                    IdentityScope::Fields => format!(
                                        "era `{era_name}` redeclares stable identity #{identity} retired in era `{retiring}`"
                                    ),
                                    IdentityScope::Cases => format!(
                                        "era `{era_name}` redeclares stable case identity #{identity} retired in era `{retiring}`"
                                    ),
                                    IdentityScope::Payload(case) => format!(
                                        "era `{era_name}` redeclares stable payload identity #{identity} of case #{case} retired in era `{retiring}`"
                                    ),
                                });
                            }
                        }
                    }
                    for scope in &scopes {
                        for identity in &scope.retired {
                            if !retired.iter().any(|(key, candidate, _)| {
                                *key == scope.key && candidate == identity
                            }) {
                                retired.push((scope.key, *identity, era_name.clone()));
                            }
                        }
                        if let Some((_, dropped)) =
                            previous_live.iter().find(|(key, _)| *key == scope.key)
                        {
                            for identity in dropped {
                                if !scope.live.contains(identity)
                                    && !retired.iter().any(|(key, candidate, _)| {
                                        *key == scope.key && candidate == identity
                                    })
                                {
                                    retired.push((scope.key, *identity, era_name.clone()));
                                }
                            }
                        }
                    }
                    for scope in scopes {
                        match previous_live.iter_mut().find(|(key, _)| *key == scope.key) {
                            Some((_, live)) => *live = scope.live,
                            None => previous_live.push((scope.key, scope.live)),
                        }
                    }
                }
                None
            }
        }
        pub(crate) mod observations {
            #[derive(Debug, Clone, PartialEq, Eq, Default)]
            pub(crate) struct WireShape {
                pub name: String,
                pub encoding: Option<String>,
                pub fields: Vec<WireField>,
                pub cases: Vec<WireCase>,
            }

            #[derive(Debug, Clone, PartialEq, Eq)]
            pub(crate) struct WireField {
                pub number: u64,
                pub relevance: language_core::BindingRelevance,
                pub type_display: String,
            }

            #[derive(Debug, Clone, PartialEq, Eq)]
            pub(crate) struct WireCase {
                pub number: u64,
                pub payload_fields: Vec<WireField>,
            }

            pub(crate) struct WireDemandVerdict {
                pub edge: String,
                pub local_schema: String,
                pub peer_schema: String,
                pub readability: WireFact,
                pub writability: WireFact,
                pub unknown_preservation: WireFact,
                pub canonicality: WireFact,
                pub migration_coverage: WireFact,
                pub satisfied: bool,
            }

            pub(crate) struct WireFact {
                pub required: bool,
                pub satisfied: bool,
                pub detail: String,
            }
        }
        pub(crate) mod schemas {
            use super::demands::compatibility_demand_verdict;
            use super::{
                WireCase, WireCompatibilityDemand, WireDemandVerdict, WireField, WireShape,
            };
            use crate::CheckingInput;
            use arena::HandleSpan;
            use typed_trees::data::DataDefinition;
            use typed_trees::wire::{WireMember, WireSchema};

            pub(crate) struct SchemaRow {
                pub(crate) qualified_path: String,
                pub(crate) entry: WireShape,
                pub(crate) value_symbol: Option<symbols::SymbolHandle>,
            }

            pub(crate) fn check_wire_demands(
                typed: &dyn CheckingInput,
                compatibility_demands: &[WireCompatibilityDemand],
            ) -> Vec<WireDemandVerdict> {
                if compatibility_demands.is_empty() {
                    return Vec::new();
                }
                let mut rows = typed
                    .wire_schemas()
                    .iter()
                    .map(|schema| SchemaRow {
                        qualified_path: qualified_schema_path(
                            typed,
                            schema.symbol,
                            schema.name.as_str(),
                        ),
                        entry: schema_shape(typed, schema),
                        value_symbol: None,
                    })
                    .collect::<Vec<_>>();
                for (ordinary_path, ordinary, value_symbol) in
                    typed.data_definitions().iter().filter_map(|data| {
                        ordinary_data_shape(typed, data).map(|entry| {
                            (
                                qualified_schema_path(typed, data.symbol, data.name.as_str()),
                                entry,
                                data.symbol,
                            )
                        })
                    })
                {
                    if let Some(generated) = rows
                        .iter_mut()
                        .find(|row| row.qualified_path == ordinary_path)
                    {
                        generated.value_symbol = Some(value_symbol);
                        if generated.entry.fields.is_empty() {
                            generated.entry.fields = ordinary.fields;
                        }
                        generated.entry.cases = ordinary.cases;
                    } else {
                        rows.push(SchemaRow {
                            qualified_path: ordinary_path,
                            entry: ordinary,
                            value_symbol: Some(value_symbol),
                        });
                    }
                }
                rows.sort_by(|left, right| left.entry.name.cmp(&right.entry.name));
                compatibility_demands
                    .iter()
                    .map(|demand| compatibility_demand_verdict(typed, &rows, demand))
                    .collect()
            }

            pub(crate) fn qualified_schema_path(
                typed: &dyn CheckingInput,
                symbol: symbols::SymbolHandle,
                fallback_name: &str,
            ) -> String {
                let path = typed.symbols.display_path(symbol, "::");
                if path.is_empty() {
                    fallback_name.to_owned()
                } else {
                    path
                }
            }

            fn ordinary_data_shape(
                typed: &dyn CheckingInput,
                data: &DataDefinition,
            ) -> Option<WireShape> {
                use typed_trees::data::DataMember;

                let members = typed.data_members(data);
                let has_identity_metadata = !data.retired_identities.is_empty()
                    || members.iter().any(|member| match member {
                        DataMember::Field(field) => field.identity.is_some(),
                        DataMember::Variant(variant) => {
                            variant.identity.is_some()
                                || !variant.retired_payload_identities.is_empty()
                                || typed
                                    .data_payload_fields(variant)
                                    .iter()
                                    .any(|field| field.identity.is_some())
                        }
                    });
                if !has_identity_metadata {
                    return None;
                }
                let fields = members
                    .iter()
                    .filter_map(|member| match member {
                        DataMember::Field(field) => Some(WireField {
                            number: field.identity?,
                            relevance: field.relevance,
                            type_display: typed.display_type_reference(field.type_reference),
                        }),
                        DataMember::Variant(_) => None,
                    })
                    .collect();
                let cases = members
                    .iter()
                    .filter_map(|member| match member {
                        DataMember::Variant(variant) => Some(WireCase {
                            number: variant.identity?,
                            payload_fields: typed
                                .data_payload_fields(variant)
                                .iter()
                                .filter_map(|field| {
                                    Some(WireField {
                                        number: field.identity?,
                                        relevance: field.relevance,
                                        type_display: typed
                                            .display_type_reference(field.type_reference),
                                    })
                                })
                                .collect(),
                        }),
                        DataMember::Field(_) => None,
                    })
                    .collect();
                Some(WireShape {
                    name: data.name.to_string(),
                    encoding: None,
                    fields,
                    cases,
                })
            }

            fn collect_fields(
                typed: &dyn CheckingInput,
                members: HandleSpan<WireMember>,
            ) -> Vec<WireField> {
                typed
                    .wire_members(members)
                    .iter()
                    .filter_map(|member| match member {
                        WireMember::Field(field) => Some(WireField {
                            number: field.number,
                            relevance: field.relevance,
                            type_display: typed.display_type_reference(field.type_reference),
                        }),
                        WireMember::Reserved(_) | WireMember::Version(_) => None,
                    })
                    .collect()
            }

            fn schema_shape(typed: &dyn CheckingInput, schema: &WireSchema) -> WireShape {
                WireShape {
                    name: schema.name.to_string(),
                    encoding: schema
                        .encoding
                        .as_ref()
                        .map(|encoding| encoding.to_string()),
                    fields: collect_fields(typed, schema.members),
                    cases: Vec::new(),
                }
            }
        }

        use language_semantics::wire::WireCompatibilityDemand;
        use observations::{WireCase, WireDemandVerdict, WireFact, WireField, WireShape};
        use schemas::check_wire_demands;

        pub fn validate_wire_protocol(
            typed: &dyn crate::CheckingInput,
            compatibility_demands: &[WireCompatibilityDemand],
        ) -> Result<(), Vec<Diagnostic>> {
            validate_wire_demands(&check_wire_demands(typed, compatibility_demands))
        }

        fn validate_wire_demands(demands: &[WireDemandVerdict]) -> Result<(), Vec<Diagnostic>> {
            let diagnostics = demands
        .iter()
        .filter(|demand| !demand.satisfied)
        .map(|demand| {
            let failed = [
                ("readability", &demand.readability),
                ("writability", &demand.writability),
                ("unknown preservation", &demand.unknown_preservation),
                ("canonicality", &demand.canonicality),
                ("migration coverage", &demand.migration_coverage),
            ]
            .into_iter()
            .filter(|(_, fact)| fact.required && !fact.satisfied)
            .map(|(name, fact)| format!("{name} ({})", fact.detail))
            .collect::<Vec<_>>()
            .join(", ");
            Diagnostic::error(format!(
                "wire compatibility demand `{}` is unsatisfied for local schema `{}` and peer \
                 schema `{}`: {}",
                demand.edge, demand.local_schema, demand.peer_schema, failed
            ))
        })
        .collect::<Vec<_>>();
            crate::program_validation::finish_strict_diagnostics(diagnostics)
        }
    }
    pub(crate) mod schema_validation {
        use crate::declarations::symbols::TopLevelSymbols;
        use arena::HandleSpan;
        use diagnostics::Diagnostic;
        use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
        use typed_trees::wire::{WireField, WireMember, WireReserved, WireSchema, WireVersion};

        pub(crate) fn validate_wire_schemas(
            program: &dyn crate::CheckingInput,
            symbols: &TopLevelSymbols<'_>,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            for (schema_index, schema) in program.wire_schemas().iter().enumerate() {
                if program.wire_schemas()[..schema_index]
                    .iter()
                    .any(|previous| {
                        previous.name == schema.name
                            && !program
                                .symbols
                                .source_scopes_separate(previous.symbol, schema.symbol)
                    })
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "duplicate data `{}`",
                        schema.name
                    )));
                }

                validate_schema(program, symbols, schema, diagnostics);
            }
        }

        pub(crate) struct WireScope<'program> {
            version: Option<&'program str>,
            fields: Vec<&'program WireField>,
            reserved: Vec<&'program WireReserved>,
            versions: Vec<&'program WireVersion>,
        }

        fn collect_scope<'program>(
            program: &'program dyn crate::CheckingInput,
            version: Option<&'program str>,
            members: HandleSpan<WireMember>,
        ) -> WireScope<'program> {
            let mut scope = WireScope {
                version,
                fields: Vec::new(),
                reserved: Vec::new(),
                versions: Vec::new(),
            };

            for member in program.wire_members(members) {
                match member {
                    WireMember::Field(field) => scope.fields.push(field),
                    WireMember::Reserved(reserved) => scope.reserved.push(reserved),
                    WireMember::Version(nested) => scope.versions.push(nested),
                }
            }

            scope
        }

        fn scope_label(schema: &WireSchema, scope: &WireScope<'_>) -> String {
            match scope.version {
                Some(version) => format!("data `{}` version `{version}`", schema.name),
                None => format!("data `{}`", schema.name),
            }
        }

        fn validate_schema(
            program: &dyn crate::CheckingInput,
            symbols: &TopLevelSymbols<'_>,
            schema: &WireSchema,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let current = collect_scope(program, None, schema.members);

            for (version_index, version) in current.versions.iter().enumerate() {
                if current.versions[..version_index]
                    .iter()
                    .any(|previous| previous.name == version.name)
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "data `{}` declares duplicate version `{}`",
                        schema.name, version.name
                    )));
                }
            }

            let version_scopes = current
                .versions
                .iter()
                .map(|version| collect_scope(program, Some(version.name.as_str()), version.members))
                .collect::<Vec<_>>();

            for scope in version_scopes.iter().chain(std::iter::once(&current)) {
                validate_scope_numbering(schema, scope, diagnostics);
                validate_scope_field_types(program, symbols, schema, scope, diagnostics);

                if scope.version.is_some() && !scope.versions.is_empty() {
                    diagnostics.push(Diagnostic::error(format!(
                "{} nests a version block inside a version block; protocol eras must be declared at the schema level",
                scope_label(schema, scope)
            )));
                }
            }

            let mut chain: Vec<&WireScope<'_>> = version_scopes.iter().collect();
            chain.push(&current);

            for pair in chain.windows(2) {
                validate_adjacent_eras(schema, pair[0], pair[1], diagnostics);
            }
        }

        fn validate_scope_numbering(
            schema: &WireSchema,
            scope: &WireScope<'_>,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            for (field_index, field) in scope.fields.iter().enumerate() {
                if scope.fields[..field_index]
                    .iter()
                    .any(|previous| previous.number == field.number)
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "{} declares duplicate field number {} for field `{}`",
                        scope_label(schema, scope),
                        field.number,
                        field.name
                    )));
                }

                if scope
                    .reserved
                    .iter()
                    .any(|reserved| reserved.number == field.number)
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "{} field `{}` reuses retired identity number {}",
                        scope_label(schema, scope),
                        field.name,
                        field.number
                    )));
                }
            }

            for (reserved_index, reserved) in scope.reserved.iter().enumerate() {
                if scope.reserved[..reserved_index]
                    .iter()
                    .any(|previous| previous.number == reserved.number)
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "{} retires identity number {} more than once",
                        scope_label(schema, scope),
                        reserved.number
                    )));
                }
            }
        }

        fn validate_scope_field_types(
            program: &dyn crate::CheckingInput,
            symbols: &TopLevelSymbols<'_>,
            schema: &WireSchema,
            scope: &WireScope<'_>,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            for field in &scope.fields {
                validate_field_type_reference(
                    program,
                    symbols,
                    schema,
                    scope,
                    field,
                    field.type_reference,
                    diagnostics,
                );
                if field.relevance.is_erased() {
                    continue;
                }
            }
        }

        fn validate_field_type_reference(
            program: &dyn crate::CheckingInput,
            symbols: &TopLevelSymbols<'_>,
            schema: &WireSchema,
            scope: &WireScope<'_>,
            field: &WireField,
            type_reference: TypeReferenceHandle,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Reference { referee, .. } => validate_field_type_reference(
                    program,
                    symbols,
                    schema,
                    scope,
                    field,
                    *referee,
                    diagnostics,
                ),
                TypeReferenceNode::Constrained { base_type, .. } => validate_field_type_reference(
                    program,
                    symbols,
                    schema,
                    scope,
                    field,
                    *base_type,
                    diagnostics,
                ),
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => validate_field_type_reference(
                    program,
                    symbols,
                    schema,
                    scope,
                    field,
                    *element_type,
                    diagnostics,
                ),
                TypeReferenceNode::Generic {
                    base_symbol,
                    base_name,
                    arguments,
                    ..
                } => {
                    if !wire_type_resolves(program, symbols, *base_symbol, base_name.as_str()) {
                        push_unknown_field_type(
                            schema,
                            scope,
                            field,
                            base_name.as_str(),
                            diagnostics,
                        );
                    }

                    for argument in program
                        .type_reference_table
                        .type_reference_handles(*arguments)
                    {
                        if matches!(
                            program.type_reference_table.type_reference(*argument),
                            TypeReferenceNode::Named { name, .. }
                                if name.as_str().parse::<u64>().is_ok()
                        ) {
                            continue;
                        }
                        validate_field_type_reference(
                            program,
                            symbols,
                            schema,
                            scope,
                            field,
                            *argument,
                            diagnostics,
                        );
                    }
                }
                TypeReferenceNode::DynamicTrait { symbol, name, .. }
                | TypeReferenceNode::Named { symbol, name } => {
                    if !wire_type_resolves(program, symbols, *symbol, name.as_str()) {
                        push_unknown_field_type(schema, scope, field, name.as_str(), diagnostics);
                    }
                }
                TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Unit => {}
            }
        }

        fn wire_type_resolves(
            program: &dyn crate::CheckingInput,
            symbols: &TopLevelSymbols<'_>,
            symbol: symbols::SymbolHandle,
            name: &str,
        ) -> bool {
            symbols.has_type_symbol(symbol)
                || PrimitiveType::from_name(name).is_some()
                || program.wire_schemas().iter().any(|schema| {
                    schema.symbol == symbol
                        || (!program.symbols.has_source_metadata() && schema.name.as_str() == name)
                })
                || (!program.symbols.has_source_metadata() && symbols.has_type(name))
        }

        fn push_unknown_field_type(
            schema: &WireSchema,
            scope: &WireScope<'_>,
            field: &WireField,
            type_name: &str,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            diagnostics.push(Diagnostic::error(format!(
                "{} field `{}` references unknown type `{type_name}`",
                scope_label(schema, scope),
                field.name
            )));
        }

        fn validate_adjacent_eras(
            schema: &WireSchema,
            predecessor: &WireScope<'_>,
            successor: &WireScope<'_>,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let predecessor_name = predecessor.version.unwrap_or_default();

            for field in &predecessor.fields {
                let successor_field = successor
                    .fields
                    .iter()
                    .find(|candidate| candidate.number == field.number);

                if successor_field.is_none() {
                    let is_reserved = successor
                        .reserved
                        .iter()
                        .any(|reserved| reserved.number == field.number);

                    if !is_reserved {
                        diagnostics.push(Diagnostic::error(format!(
                    "{} drops field number {} (`{}` in version `{predecessor_name}`) without tombstoning it; add `retired {};`",
                    scope_label(schema, successor),
                    field.number,
                    field.name,
                    field.number
                )));
                    }
                }
            }
        }
    }
    pub use compatibility::validate_wire_protocol;
    pub(crate) use schema_validation::validate_wire_schemas;
}
