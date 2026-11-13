/*
Data application and projected-receiver elaboration (`data_applications`) and
product-operand requirement preparation (`product_operands`).
*/

pub(crate) mod data_applications {
    use crate::expression_roots::{append_statement_roots, visit_post_order};
    use arena::HandleSpan;
    use symbols::SymbolHandle;
    use symbols::SymbolKind;
    use typed_trees::TypedTrees;
    use typed_trees::data::DataDefinition;
    use typed_trees::data::DataMember;
    use typed_trees::data::TypeParameterKind;
    use typed_trees::name::Identifier;
    use typed_trees::type_identity::applied_type_identity;
    use typed_trees::types::TypeConstraintNode;
    use typed_trees::types::TypeReferenceHandle;
    use typed_trees::types::TypeReferenceNode;

    pub(crate) mod field_scopes {
        use arena::Handle;
        use diagnostics::Diagnostic;
        use symbols::SymbolHandle;
        use typed_trees::TypedTrees;
        use typed_trees::data::{DataField, DataMember};
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        enum FieldLocation {
            Member(Handle<DataMember>),
            Payload(Handle<DataField>),
        }

        pub(crate) fn retain(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
            let mut fields = Vec::new();
            for definition in program.data_definitions() {
                for (position, member) in
                    (0..definition.members.count()).zip(program.data_members(definition))
                {
                    match member {
                        DataMember::Field(field) => fields.push((
                            FieldLocation::Member(definition.members.handle_at(position)),
                            definition.symbol,
                            field.type_reference,
                        )),
                        DataMember::Variant(variant) => {
                            for (position, field) in (0..variant.payload.count())
                                .zip(program.data_payload_fields(variant))
                            {
                                fields.push((
                                    FieldLocation::Payload(variant.payload.handle_at(position)),
                                    definition.symbol,
                                    field.type_reference,
                                ));
                            }
                        }
                    }
                }
            }
            for (location, owner, reference) in fields {
                if program
                    .type_reference_table
                    .operand_lifetime_owner(reference)
                    .is_valid()
                    || !contains_regions(program, reference)
                {
                    continue;
                }
                let scoped = scoped_field_type(program, owner, reference)
                    .map_err(|reason| vec![Diagnostic::error(reason)])?;
                match location {
                    FieldLocation::Member(handle) => {
                        if let DataMember::Field(field) = program.data_members.get_mut(handle) {
                            field.type_reference = scoped;
                        }
                    }
                    FieldLocation::Payload(handle) => {
                        program.data_payload_fields.get_mut(handle).type_reference = scoped
                    }
                }
            }
            Ok(())
        }

        fn contains_regions(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
            crate::monomorphization::for_each_type_reference(program, &[reference], |_, node| {
                matches!(
                    node,
                    TypeReferenceNode::Reference {
                        lifetime: Some(_),
                        ..
                    }
                ) || matches!(node, TypeReferenceNode::Generic { lifetime_arguments, .. } if !lifetime_arguments.is_empty())
            })
        }

        fn scoped_field_type(
            program: &mut TypedTrees,
            owner: SymbolHandle,
            reference: TypeReferenceHandle,
        ) -> Result<TypeReferenceHandle, &'static str> {
            let scoped = program.apply_callable_type(reference, &[], &[], owner)?;
            program
                .type_reference_table
                .retain_operand_lifetime_owner(scoped, owner)?;
            Ok(scoped)
        }
    }
    pub(crate) use field_scopes::retain as retain_declared_field_scopes;

    pub(crate) fn elaborate(program: &mut TypedTrees) -> Result<(), Vec<diagnostics::Diagnostic>> {
        let type_start = program.type_reference_table.type_reference_count();
        let applications = program
            .type_reference_table
            .type_reference_rows()
            .map(|(reference, _)| reference)
            .collect::<Vec<_>>();
        for reference in applications {
            elaborate_value_fields(program, reference, &mut Vec::new())
                .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])?;
        }
        elaborate_projected_fields(program)
            .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])?;
        finalize_produced_types(program, type_start)
    }

    pub(crate) fn elaborate_new_types(
        program: &mut TypedTrees,
        first_type: usize,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        let type_start = program.type_reference_table.type_reference_count();
        let applications = program
            .type_reference_table
            .references_from(first_type.saturating_add(1))
            .map(|(reference, _)| reference)
            .collect::<Vec<_>>();
        for reference in applications {
            elaborate_value_fields(program, reference, &mut Vec::new())
                .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])?;
        }
        finalize_produced_types(program, type_start)
    }

    pub(crate) fn elaborate_projected_receivers(
        program: &mut TypedTrees,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        let type_start = program.type_reference_table.type_reference_count();
        elaborate_projected_fields(program)
            .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])?;
        finalize_produced_types(program, type_start)
    }

    fn finalize_produced_types(
        program: &mut TypedTrees,
        type_start: usize,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        let produced = program
            .type_reference_table
            .references_from(type_start.saturating_add(1))
            .map(|(reference, _)| reference)
            .collect::<Vec<_>>();
        crate::monomorphization::finalize_type_domain_instance_identities(program, produced)
            .map_err(|diagnostic| vec![diagnostic])
    }

    fn elaborate_projected_fields(program: &mut TypedTrees) -> Result<(), &'static str> {
        let mut projections = Vec::new();
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                let mut expressions = Vec::new();
                append_statement_roots(program, state, &mut expressions);
                visit_post_order(program, expressions, |_, node| {
                    if let typed_trees::expression::ExpressionNode::Member(member) = node {
                        projections.push((machine.symbol, state.symbol, member.receiver));
                    }
                });
            }
        }
        for (machine_symbol, state_symbol, receiver) in projections {
            let Some(machine) = program.machine_by_symbol(machine_symbol) else {
                continue;
            };
            let Some(state) = program.state_by_symbol(state_symbol) else {
                continue;
            };
            let Some(reference) =
                validation::expression_result_type_reference(program, machine, state, receiver)
            else {
                continue;
            };
            let reference = program
                .type_reference_table
                .peeled_type_reference(reference);
            elaborate_value_fields(program, reference, &mut Vec::new())?;
        }
        Ok(())
    }

    fn elaborate_value_fields(
        program: &mut TypedTrees,
        reference: TypeReferenceHandle,
        active: &mut Vec<(SymbolHandle, TypeReferenceHandle)>,
    ) -> Result<(), &'static str> {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. } => return Ok(()),
            TypeReferenceNode::Constrained { base_type, .. } => {
                let base_type = *base_type;
                elaborate_value_fields(program, base_type, active)?;
                return Ok(());
            }
            TypeReferenceNode::FixedArray { element_type, .. } => {
                let element_type = *element_type;
                elaborate_value_fields(program, element_type, active)?;
                return Ok(());
            }
            _ => {}
        }
        let owner = match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
            TypeReferenceNode::Named { symbol, .. } => *symbol,
            _ => return Ok(()),
        };
        let ancestors = std::mem::take(active);
        active.extend(ancestors.iter().copied().filter(|(_, parent)| {
            let TypeReferenceNode::Generic { arguments, .. } =
                program.type_reference_table.type_reference(*parent)
            else {
                return true;
            };
            !contains_actual_type(
                program,
                program
                    .type_reference_table
                    .type_reference_handles(*arguments),
                reference,
            )
        }));
        if active.iter().any(|(ancestor, _)| *ancestor == owner) {
            *active = ancestors;
            return Ok(());
        }
        if program.data_application_definition(reference).is_some() {
            *active = ancestors;
            return Ok(());
        }
        elaborate_application(program, reference)?;
        let Some(data) = program
            .data_application_definition(reference)
            .or_else(|| program.closed_data_definition(reference))
        else {
            *active = ancestors;
            return Ok(());
        };
        let mut children = Vec::new();
        for member in program.data_members(data) {
            match member {
                DataMember::Field(field) => children.push(field.type_reference),
                DataMember::Variant(case) => children.extend(
                    program
                        .data_payload_fields(case)
                        .iter()
                        .map(|field| field.type_reference),
                ),
            }
        }
        active.push((owner, reference));
        for child in children {
            elaborate_value_fields(program, child, active)?;
        }
        *active = ancestors;
        Ok(())
    }

    fn contains_actual_type(
        program: &TypedTrees,
        arguments: &[TypeReferenceHandle],
        selected: TypeReferenceHandle,
    ) -> bool {
        let mut pending = arguments.to_vec();
        let mut visited = Vec::new();
        while let Some(reference) = pending.pop() {
            if reference == selected {
                return true;
            }
            if visited.contains(&reference) {
                continue;
            }
            visited.push(reference);
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Generic { arguments, .. } => pending.extend(
                    program
                        .type_reference_table
                        .type_reference_handles(*arguments)
                        .iter()
                        .copied(),
                ),
                TypeReferenceNode::Constrained { base_type, .. } => pending.push(*base_type),
                TypeReferenceNode::FixedArray { element_type, .. } => pending.push(*element_type),
                _ => {}
            }
        }
        false
    }

    fn elaborate_application(
        program: &mut TypedTrees,
        reference: TypeReferenceHandle,
    ) -> Result<(), &'static str> {
        if program.data_application_definition(reference).is_some() {
            return Ok(());
        }
        let TypeReferenceNode::Generic {
            base_symbol,
            lifetime_arguments,
            arguments,
            ..
        } = program
            .type_reference_table
            .type_reference(reference)
            .clone()
        else {
            return Ok(());
        };
        if let Some(storage) = program.layout_storage_arguments(reference) {
            elaborate_value_fields(program, storage.semantic_type, &mut Vec::new())?;
            let is_closed = program
                .closed_data_field_definition(storage.semantic_type)
                .is_some();
            if let Some(mut semantic) = program
                .data_field_definition(storage.semantic_type)
                .cloned()
            {
                semantic.generic_instance = Some(reference);
                program.retain_data_application_definition(reference, semantic, is_closed);
            }
            return Ok(());
        }
        let Some(template) = program
            .unique_data_definition_by_symbol(base_symbol)
            .cloned()
        else {
            return Ok(());
        };
        let parameters = program.data_type_parameters(&template).to_vec();
        let arguments = program
            .type_reference_table
            .type_reference_handles(arguments)
            .to_vec();
        if parameters.len() != arguments.len()
            || template.lifetime_parameters.len() != lifetime_arguments.len()
        {
            return Ok(());
        }
        let mut is_closed = parameters
            .iter()
            .zip(&arguments)
            .all(|(parameter, argument)| {
                matches!(parameter.kind, TypeParameterKind::Value { .. })
                    || closed_type(program, *argument)
            });
        if let Some(instance) = program.closed_data_definition(reference).cloned()
            && let Some(origin) = instance.generic_instance
            && let Some(identity) =
                applied_type_identity(program, reference, SymbolHandle::invalid(), &[])
            && applied_type_identity(program, origin, SymbolHandle::invalid(), &[])
                .is_some_and(|retained| retained == identity)
        {
            retain_collection_results(program, &instance);
            program.retain_data_application_definition(reference, instance, true);
            return Ok(());
        }
        let substitutions = parameters
            .iter()
            .zip(arguments)
            .map(|(parameter, argument)| (parameter.symbol, argument))
            .collect::<Vec<_>>();
        let lifetimes = template
            .lifetime_parameters
            .iter()
            .cloned()
            .zip(lifetime_arguments)
            .collect::<Vec<_>>();
        let members = program.data_members(&template).to_vec();
        let scope = FieldApplicationScope {
            application: reference,
            declaration: template.symbol,
            supplier: program
                .type_reference_table
                .operand_lifetime_owner(reference),
        };
        let mut instantiated = Vec::with_capacity(members.len());
        for member in members {
            let member = match member {
                DataMember::Field(mut field) => {
                    field.type_reference = instantiate_type(
                        program,
                        field.type_reference,
                        &substitutions,
                        &lifetimes,
                        scope,
                    )?;
                    is_closed &= closed_type(program, field.type_reference);
                    DataMember::Field(field)
                }
                DataMember::Variant(mut variant) => {
                    let mut fields = program.data_payload_fields(&variant).to_vec();
                    for field in &mut fields {
                        field.type_reference = instantiate_type(
                            program,
                            field.type_reference,
                            &substitutions,
                            &lifetimes,
                            scope,
                        )?;
                        is_closed &= closed_type(program, field.type_reference);
                    }
                    variant.payload = program.data_payload_fields.insert_many(fields);
                    DataMember::Variant(variant)
                }
            };
            instantiated.push(member);
        }
        let mut application = template;
        if application.cleanup_selection.is_some()
            && let Some(selection) = program
                .applied_data_cleanup_selection(application.symbol, reference)
                .copied()
        {
            application.cleanup_selection = Some(selection);
        }
        if application.predicate_subject_type.is_valid() {
            application.predicate_subject_type = instantiate_type(
                program,
                application.predicate_subject_type,
                &substitutions,
                &lifetimes,
                scope,
            )?;
        }
        application.members = program.data_members.insert_many(instantiated);
        application.type_parameters = HandleSpan::empty();
        application.lifetime_parameters.clear();
        application.generic_instance = Some(reference);
        retain_collection_results(program, &application);
        program.retain_data_application_definition(reference, application, is_closed);
        Ok(())
    }

    fn retain_collection_results(program: &mut TypedTrees, definition: &DataDefinition) {
        let mut references = Vec::new();
        for member in program.data_members(definition) {
            match member {
                DataMember::Field(field) => references.push(field.type_reference),
                DataMember::Variant(variant) => references.extend(
                    program
                        .data_payload_fields(variant)
                        .iter()
                        .map(|field| field.type_reference),
                ),
            }
        }
        for reference in references {
            program.retain_collection_result_types(reference);
        }
    }

    fn closed_type(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
        let mut pending = vec![reference];
        while let Some(reference) = pending.pop() {
            if !program
                .type_reference_table
                .contains_type_reference(reference)
            {
                return false;
            }
            if program.closed_data_definition(reference).is_some() {
                continue;
            }
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Callable { contract } => {
                    if !program.runtime_callable_contracts.is_valid(*contract)
                        || program
                            .runtime_callable_contract_identity(*contract, &[])
                            .is_err()
                    {
                        return false;
                    }
                    let inputs = typed_trees::references::type_reference_inputs(program, reference);
                    let free_parameters = inputs
                        .symbols
                        .into_iter()
                        .filter(|symbol| {
                            matches!(
                                program.symbols.get(*symbol).kind,
                                SymbolKind::TypeParameter
                                    | SymbolKind::Parameter
                                    | SymbolKind::Local
                                    | SymbolKind::MachineParameter
                                    | SymbolKind::ConformanceParameter
                                    | SymbolKind::PropositionParameter
                                    | SymbolKind::PropositionMachineParameter
                            )
                        })
                        .collect::<Vec<_>>();
                    if program.type_reference_mentions_any(reference, &free_parameters) {
                        return false;
                    }
                }
                TypeReferenceNode::Named { symbol, .. } => {
                    if symbol.is_valid()
                        && matches!(
                            program.symbols.get(*symbol).kind,
                            SymbolKind::TypeParameter | SymbolKind::Parameter | SymbolKind::Local
                        )
                    {
                        return false;
                    }
                    if let Some(data) = program.unique_data_definition_by_symbol(*symbol)
                        && !program.data_type_parameters(data).is_empty()
                    {
                        return false;
                    }
                    if let Some(declaration) = program.trait_definition_by_symbol(*symbol)
                        && !program.trait_type_parameters(declaration).is_empty()
                    {
                        return false;
                    }
                }
                TypeReferenceNode::Reference { referee, .. } => pending.push(*referee),
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    pending.push(*base_type);
                    for constraint in program.type_reference_table.constraints(*constraints) {
                        if let TypeConstraintNode::Domain(domain) = constraint {
                            pending.extend(domain.arguments.iter().copied())
                        }
                    }
                }
                TypeReferenceNode::FixedArray { element_type, .. } => {
                    if program.declared_array_extent(reference).is_none() {
                        return false;
                    }
                    pending.push(*element_type);
                }
                TypeReferenceNode::Slice { element_type } => pending.push(*element_type),
                TypeReferenceNode::Generic {
                    base_symbol,
                    lifetime_arguments,
                    arguments,
                    ..
                } => {
                    let telescope = program
                        .unique_data_definition_by_symbol(*base_symbol)
                        .map(|data| {
                            (
                                program.data_type_parameters(data).len(),
                                data.lifetime_parameters.len(),
                            )
                        })
                        .or_else(|| {
                            program
                                .trait_definition_by_symbol(*base_symbol)
                                .map(|declaration| {
                                    (
                                        program.trait_type_parameters(declaration).len(),
                                        declaration.lifetime_parameters.len(),
                                    )
                                })
                        })
                        .or_else(|| {
                            program
                                .conformance_by_symbol(*base_symbol)
                                .map(|declaration| {
                                    (
                                        program.conformance_type_parameters(declaration).len(),
                                        declaration.lifetime_parameters.len(),
                                    )
                                })
                        });
                    let Some((parameter_count, lifetime_count)) = telescope else {
                        return false;
                    };
                    let arguments = program
                        .type_reference_table
                        .type_reference_handles(*arguments);
                    if parameter_count != arguments.len()
                        || lifetime_count != lifetime_arguments.len()
                    {
                        return false;
                    }
                    if let Some(data) = program.unique_data_definition_by_symbol(*base_symbol) {
                        let parameters = program.data_type_parameters(data);
                        for (param, &arg) in parameters.iter().zip(arguments.iter()) {
                            if !matches!(param.kind, TypeParameterKind::Value { .. }) {
                                pending.push(arg);
                            }
                        }
                    } else {
                        pending.extend(arguments.iter().copied());
                    }
                }
                TypeReferenceNode::DynamicTrait {
                    symbol, arguments, ..
                } => {
                    let Some(declaration) = program.trait_definition_by_symbol(*symbol) else {
                        return false;
                    };
                    let arguments = program
                        .type_reference_table
                        .type_reference_handles(*arguments);
                    if program.trait_type_parameters(declaration).len() != arguments.len() {
                        return false;
                    }
                    pending.extend(arguments.iter().copied());
                }
                TypeReferenceNode::ConstExpression(_) => {
                    if program.declared_constant_extent(reference).is_none() {
                        return false;
                    }
                }
                TypeReferenceNode::Unit => {}
            }
        }
        true
    }

    #[derive(Clone, Copy)]
    struct FieldApplicationScope {
        application: TypeReferenceHandle,
        declaration: SymbolHandle,
        supplier: SymbolHandle,
    }

    fn instantiate_type(
        program: &mut TypedTrees,
        reference: TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        lifetimes: &[(Identifier, Identifier)],
        scope: FieldApplicationScope,
    ) -> Result<TypeReferenceHandle, &'static str> {
        if !program
            .type_reference_table
            .contains_type_reference(reference)
        {
            return Err("applied data member has no current type occurrence");
        }
        let regions = program
            .type_reference_table
            .operand_lifetimes(scope.application)
            .to_vec();
        program.apply_callable_type_in_scope(
            reference,
            substitutions,
            lifetimes,
            scope.declaration,
            scope.supplier,
            &regions,
        )
    }
}

pub(crate) mod product_operands {
    use crate::monomorphization::normalize_const_arguments;
    use crate::tables::expressions_of_mut;
    use arena::{Handle, HandleSpan};
    use diagnostics::Diagnostic;
    use expr::CompletedValueCapture;
    use expr::EvidenceProjection;
    use language_semantics::declaration_selection::{
        ProductDeclarationOperand, ProductOperandRole, product_operands::*,
    };
    use symbols::SymbolHandle;
    use typed_trees::product_operand::resolve_product_path;
    use typed_trees::static_applications::StaticMachineLifetimeBinding;
    use typed_trees::{
        TypedTrees, expression as expr,
        name::Identifier,
        trait_definition::{AppliedTraitScope, RetainedTraitScope},
        types::{self, TypeReferenceHandle, TypeReferenceNode},
    };
    use types::FixedArrayLength;

    macro_rules! public_symbols {
        ($table:expr) => {
            $table.iter().map(|value| (value.symbol, value.is_public))
        };
    }

    pub(crate) mod nominal {
        use super::*;
        use crate::tables::{expressions_of, expressions_of_mut};
        use typed_trees::data::TypeParameterKind;
        use typed_trees::product_operand::{ProductApplicationDemand, ProductOperandApplication};
        use typed_trees::static_applications::StaticConformanceSelection;
        use typed_trees::trait_definition::TraitApplicationArguments;
        use validation::conformance_applications::close_conformance_application;

        #[derive(Clone, Copy)]
        enum NominalDeclaration {
            Authored,
            Retained(SymbolHandle),
        }

        fn require_application_inputs(
            program: &mut TypedTrees,
            arguments: &[expr::StaticMachineArgument],
            lexical_owner: SymbolHandle,
            demand: ProductApplicationDemand,
        ) -> Result<(), Vec<Diagnostic>> {
            let references = arguments
                .iter()
                .map(|argument| {
                    program.static_operand_type_reference_in_scope(argument, lexical_owner)
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            let result = match demand {
                ProductApplicationDemand::FormationOnly => {
                    program.require_declared_application_operands(&references, lexical_owner)
                }
                ProductApplicationDemand::Selected => {
                    program.require_owned_application_operands(&references, lexical_owner)
                }
            };
            result.map_err(|reason| vec![Diagnostic::error(reason)])
        }

        fn require_trait_inputs(
            program: &TypedTrees,
            scope: &AppliedTraitScope<'_>,
            demand: ProductApplicationDemand,
        ) -> Result<(), &'static str> {
            match demand {
                ProductApplicationDemand::FormationOnly => {
                    program.require_declared_trait_operands(scope)
                }
                ProductApplicationDemand::Selected => {
                    program.require_receiving_trait_operands(scope)
                }
            }
        }

        pub fn prepare_retained_product_application(
            program: &mut TypedTrees,
            operand: &ProductDeclarationOperand,
            role: ProductOperandRole,
            declaration: SymbolHandle,
        ) -> Result<Handle<ProductOperandApplication>, Vec<Diagnostic>> {
            let handle = prepare(
                program,
                operand,
                role,
                declaration,
                None,
                ProductApplicationDemand::Selected,
                NominalDeclaration::Retained(declaration),
            )?
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "retained Product choice lost its closed application",
                )]
            })?;
            program.retain_dependency_product_application(handle);
            Ok(handle)
        }

        pub fn prepare_product_operand_application(
            program: &mut TypedTrees,
            operand: &ProductDeclarationOperand,
            role: ProductOperandRole,
            author: SymbolHandle,
            static_environment: Option<&ProductStaticEnvironment>,
            demand: ProductApplicationDemand,
        ) -> Result<Option<Handle<ProductOperandApplication>>, Vec<Diagnostic>> {
            prepare(
                program,
                operand,
                role,
                author,
                static_environment,
                demand,
                NominalDeclaration::Authored,
            )
        }

        fn prepare(
            program: &mut TypedTrees,
            operand: &ProductDeclarationOperand,
            role: ProductOperandRole,
            author: SymbolHandle,
            static_environment: Option<&ProductStaticEnvironment>,
            demand: ProductApplicationDemand,
            binding: NominalDeclaration,
        ) -> Result<Option<Handle<ProductOperandApplication>>, Vec<Diagnostic>> {
            let Some(graph) = operand.application.as_deref() else {
                return Ok(None);
            };
            let refuse = |reason: &str| {
                vec![Diagnostic::error(reason).with_source_span(operand.source_span)]
            };
            if !matches!(
                role,
                ProductOperandRole::BoundarySubject | ProductOperandRole::Provider
            ) {
                return Err(refuse(
                    "nominal Product application has a non-nominal declaration role",
                ));
            }
            if let Some(existing) =
                program.prepared_product_operand(operand, static_environment, author, role)
            {
                if demand == ProductApplicationDemand::Selected {
                    let retained = program.product_operand_applications.get(existing);
                    let supplying = retained.supplying_application;
                    let lexical_owner = expressions_of(program)
                        .static_machine_application(supplying)
                        .lexical_owner;
                    let [whole] =
                        expressions_of(program).static_machine_application_arguments(supplying)
                    else {
                        return Err(refuse("Product application lost its whole closed operand"));
                    };
                    let arguments = whole
                        .application
                        .as_ref()
                        .ok_or_else(|| refuse("Product application lost its static telescope"))?
                        .arguments
                        .clone();
                    let retained_scope = retained.scope;
                    require_application_inputs(program, &arguments, lexical_owner, demand)?;
                    let scope = retained_scope
                        .scope(program)
                        .ok_or_else(|| refuse("Product application lost its receiving scope"))?;
                    require_trait_inputs(program, &scope, demand).map_err(refuse)?;
                    program
                        .product_operand_applications
                        .get_mut(existing)
                        .demand = demand;
                }
                return Ok(Some(existing));
            }
            let lexical_owner = environment::supplier_owner(program, static_environment, author)?;
            let mut lower = Lower::new(program, graph, lexical_owner, static_environment);
            if matches!(binding, NominalDeclaration::Retained(_)) {
                lower.naming = OperandNaming::RetainedDeclarations;
            }
            if !graph.operands.is_valid(graph.root) {
                return Err(refuse("Product application lost its owned root"));
            }
            let root = graph.operands.get(graph.root).clone();
            if !graph.applications.is_valid(root.application) {
                return Err(refuse("Product application lost its explicit telescope"));
            }
            if graph.applications.get(root.application).member.is_valid() {
                return Err(refuse(
                    "a boundary or implementation application cannot select a callable member",
                ));
            }
            let path = lower.path(root.path)?;
            if path != operand.path {
                return Err(refuse(
                    "Product application changed its source-issued declaration path",
                ));
            }
            let declaration = match binding {
        NominalDeclaration::Retained(declaration) => declaration,
        NominalDeclaration::Authored => typed_trees::product_operand::resolve_product_path(
        lower.program,
        &path,
        operand.source_span,
        role,
    )
    .ok_or_else(|| {
        refuse(
            "Product application does not name one exact visible declaration of its required role",
        )
    })?,
    };
            let source_argument = expr::StaticMachineArgument {
                symbol: declaration,
                path: Box::from([Identifier::generated(path)]),
                application: Some(Box::new(lower.application_arguments(root.application)?)),
                ..Default::default()
            };
            let (declaration_parameters, declaration_lifetime_count) = match role {
                ProductOperandRole::BoundarySubject => {
                    let definition = lower
                        .program
                        .trait_definition_by_symbol(declaration)
                        .ok_or_else(|| refuse("an applied boundary lost its trait declaration"))?;
                    (
                        definition.type_parameters,
                        definition.lifetime_parameters.len(),
                    )
                }
                ProductOperandRole::Provider => {
                    let definition = lower
                        .program
                        .conformance_by_symbol(declaration)
                        .ok_or_else(|| {
                            refuse("an applied provider must name an ordinary named implementation")
                        })?;
                    (
                        definition.type_parameters,
                        definition.lifetime_parameters.len(),
                    )
                }
                _ => unreachable!("nominal roles checked above"),
            };
            let declaration_lifetime_count = u32::try_from(declaration_lifetime_count)
                .map_err(|_| refuse("Product declaration lifetime telescope exceeds u32"))?;
            let parameters = lower
                .program
                .data_type_parameters
                .span(declaration_parameters)
                .ok_or_else(|| refuse("Product declaration has a stale formal table"))?
                .to_vec();
            let arguments = &source_argument
                .application
                .as_ref()
                .expect("created above")
                .arguments;
            if parameters.len() != arguments.len() {
                return Err(refuse(
                    "Product application must supply its complete declaration telescope",
                ));
            }
            for (parameter, argument) in parameters.iter().zip(arguments.iter()) {
                if matches!(binding, NominalDeclaration::Authored)
                    && matches!(parameter.kind, TypeParameterKind::Type)
                {
                    let reference = lower
                        .program
                        .static_operand_type_reference_in_scope(argument, lexical_owner)
                        .map_err(refuse)?;
                    lower.program.retain_designated_static_type_argument(
                        lexical_owner,
                        reference,
                        operand.source_span,
                    );
                }
            }
            let mut closed = source_argument.clone();
            crate::monomorphization::normalize_const_arguments(
                lower.program,
                lexical_owner,
                std::slice::from_mut(&mut closed),
            )
            .map_err(|reason| refuse(&reason))?;
            let arguments = closed
                .application
                .as_ref()
                .expect("normalization retains a nominal application");
            require_application_inputs(lower.program, &arguments.arguments, lexical_owner, demand)?;
            let (scope, implementation) = match role {
                ProductOperandRole::BoundarySubject => {
                    let reference = lower
                        .program
                        .static_operand_type_reference_in_scope(&closed, lexical_owner)
                        .map_err(refuse)?;
                    let scope = AppliedTraitScope::from_type_reference(
                        lower.program,
                        reference,
                        lexical_owner,
                    )
                    .ok_or_else(|| {
                        refuse("boundary application has an invalid or incomplete telescope")
                    })?;
                    require_trait_inputs(lower.program, &scope, demand).map_err(refuse)?;
                    (
                        scope.environment().retain(lower.program).map_err(refuse)?,
                        None,
                    )
                }
                ProductOperandRole::Provider => {
                    let application = close_conformance_application(lower.program, &closed)
                        .map_err(|diagnostic| {
                            vec![diagnostic.with_source_span(operand.source_span)]
                        })?;
                    let references = arguments
                        .arguments
                        .iter()
                        .map(|argument| {
                            lower
                                .program
                                .static_operand_type_reference_in_scope(argument, lexical_owner)
                        })
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(refuse)?;
                    let definition = lower
                        .program
                        .conformance_by_symbol(declaration)
                        .expect("checked above");
                    let scope = AppliedTraitScope::from_conformance_application(
                        lower.program,
                        definition,
                        TraitApplicationArguments {
                            lexical_owner,
                            enclosing_data_applications: &[],
                            arguments: &references,
                            lifetimes: &arguments.lifetime_arguments,
                        },
                    )
                    .ok_or_else(|| {
                        refuse("implementation application lost its exact satisfied boundary scope")
                    })?;
                    require_trait_inputs(lower.program, &scope, demand).map_err(refuse)?;
                    (
                        scope.environment().retain(lower.program).map_err(refuse)?,
                        Some(application),
                    )
                }
                _ => unreachable!("nominal roles checked above"),
            };
            let supplying = expressions_of_mut(lower.program).retain_static_machine_application(
                declaration,
                Identifier::generated(&operand.path),
                std::slice::from_ref(&closed),
            );
            let application =
                expressions_of_mut(lower.program).static_machine_application_mut(supplying);
            application.binding_declaration = declaration;
            application.lexical_owner = lexical_owner;
            if let Some(implementation) = &implementation {
                application.conformance_selection = Some(Box::new(StaticConformanceSelection {
                    operand_ordinal: 0,
                    application: Some(Box::new(implementation.clone())),
                }));
            }
            let input_selections = lower
                .program
                .product_input_selections
                .insert_many(std::mem::take(&mut lower.input_selections));
            let retained = ProductOperandApplication {
                input_selections,
                operand: operand.clone(),
                environment: static_environment.cloned().map(std::sync::Arc::new),
                owner: author,
                role: Some(role),
                declaration,
                declaration_parameters,
                declaration_lifetime_count,
                demand,
                argument: source_argument,
                scope,
                supplying_application: supplying,
                implementation,
            };
            Ok(Some(
                lower.program.retain_product_operand_application(retained),
            ))
        }
    }
    pub use nominal::{prepare_product_operand_application, prepare_retained_product_application};

    pub(crate) mod environment {
        use super::*;
        use source::SourceSpan;

        fn rejoin_owner(
            program: &TypedTrees,
            owner: SourceSpan,
        ) -> Result<SymbolHandle, Vec<Diagnostic>> {
            typed_trees::product_operand::resolve_build_request_author(program, owner)
                .map_err(|reason| vec![Diagnostic::error(reason)])
        }

        pub(crate) fn supplier_owner(
            program: &TypedTrees,
            environment: Option<&ProductStaticEnvironment>,
            author: SymbolHandle,
        ) -> Result<SymbolHandle, Vec<Diagnostic>> {
            match environment {
                None => Ok(author),
                Some(environment) => {
                    let template = environment.template.ok_or_else(|| {
                        vec![Diagnostic::error(
                            "static helper environment lost its exact original template",
                        )]
                    })?;
                    if program
                        .symbols
                        .symbol_provenance_source_span(author)
                        .map(|source| program.symbols.rejoin_product_occurrence(source))
                        != Some(program.symbols.rejoin_product_occurrence(template))
                    {
                        return Err(vec![Diagnostic::error(
                            "static helper environment does not belong to the exact selecting helper",
                        )]);
                    }
                    rejoin_owner(
                        program,
                        environment.lexical_owner.ok_or_else(|| {
                            vec![Diagnostic::error(
                                "static helper environment lost its supplying source scope",
                            )]
                        })?,
                    )
                }
            }
        }

        pub(crate) struct SuppliedArgument {
            pub argument: expr::StaticMachineArgument,
            pub owner: SymbolHandle,
            pub declared_type: Option<TypeReferenceHandle>,
        }

        impl Lower<'_> {
            pub(crate) fn lexical_static_parameter(
                &self,
                owner: SymbolHandle,
                name: &ProductOperandName,
                path: &str,
            ) -> Result<Option<SymbolHandle>, Vec<Diagnostic>> {
                let Some(machine) = self.program.machine_declaration_for_lexical_scope(owner)
                else {
                    return Ok(None);
                };
                let (parameters, _) = self
                    .program
                    .machine_type_reference_scope(machine, |left, right| left.name == right.name);
                let mut matching = parameters
                    .iter()
                    .filter(|parameter| {
                        parameter.name.as_str() == path
                            && name.declaration.is_none_or(|declaration| {
                                self.program
                                    .symbols
                                    .symbol_provenance_source_span(parameter.symbol)
                                    == Some(
                                        self.program.symbols.rejoin_product_occurrence(declaration),
                                    )
                            })
                    })
                    .map(|parameter| parameter.symbol);
                let selected = matching.next();
                if matching.next().is_some() {
                    return Err(self.error("designated static input has ambiguous lexical binders"));
                }
                Ok(selected)
            }

            pub(crate) fn environment_argument(
                &mut self,
                name: Handle<ProductOperandName>,
            ) -> Result<Option<SuppliedArgument>, Vec<Diagnostic>> {
                let Some(environment) = self.static_environment else {
                    return Ok(None);
                };
                let source_name = self.graph.names.get(name);
                let parameter =
                    match source_name.declaration {
                        Some(parameter) => parameter,
                        None if !std::ptr::eq(self.graph, &environment.graph) => {
                            let template = rejoin_owner(
                    self.program,
                    environment.template.ok_or_else(|| {
                        self.error("static helper environment lost its original source template")
                    })?,
                )?;
                            let Some(parameter) = self.lexical_static_parameter(
                                template,
                                source_name,
                                &source_name.text,
                            )?
                            else {
                                return Ok(None);
                            };
                            self.program
                    .symbols
                    .symbol_provenance_source_span(parameter)
                    .ok_or_else(|| {
                        self.error("static helper binder lost its original declaration coordinate")
                    })?
                        }
                        None => return Ok(None),
                    };
                let parameter = self.program.symbols.rejoin_product_occurrence(parameter);
                let mut selected = environment
                    .bindings
                    .iter()
                    .map(|(_, binding)| binding)
                    .filter(|binding| {
                        self.program
                            .symbols
                            .rejoin_product_occurrence(binding.parameter)
                            == parameter
                    });
                let Some(binding) = selected.next().cloned() else {
                    return Ok(None);
                };
                if selected.next().is_some() {
                    return Err(self.error(
                        "static helper environment binds the same exact source parameter twice",
                    ));
                }
                if self.active_bindings.contains(&binding.parameter) {
                    return Err(self.error("static helper environment has a cyclic substitution"));
                }
                let owner = rejoin_owner(self.program, binding.lexical_owner)?;
                let mut active_bindings = self.active_bindings.clone();
                active_bindings.push(binding.parameter);
                let mut actual =
                    Lower::new(self.program, &environment.graph, owner, Some(environment));
                actual.active_bindings = active_bindings;
                let argument = actual.operand(binding.argument)?;
                if argument.type_reference.is_valid() {
                    actual
                        .program
                        .type_reference_table
                        .retain_operand_lifetime_owner(argument.type_reference, owner)
                        .map_err(|reason| actual.error(reason))?;
                }
                let declared_type = if binding.declared_type.is_valid() {
                    Some(actual.ty(binding.declared_type)?)
                } else {
                    None
                };
                Ok(Some(SuppliedArgument {
                    argument,
                    owner,
                    declared_type,
                }))
            }

            pub(crate) fn lifetime_identifier(
                &self,
                name: Handle<ProductOperandName>,
            ) -> Result<Identifier, Vec<Diagnostic>> {
                let Some(environment) = self.static_environment else {
                    return Ok(self.identifier(name));
                };
                if std::ptr::eq(self.graph, &environment.graph) {
                    return Ok(self.identifier(name));
                }
                let named = self.graph.names.get(name);
                let mut matched = None;
                for (_, binding) in environment.lifetimes.iter() {
                    let owner = rejoin_owner(self.program, binding.parameter_owner)?;
                    let machine = self.program.machine_by_symbol(owner).ok_or_else(|| {
                        self.error("static helper lifetime owner is not a machine")
                    })?;
                    let parameter = machine
                        .lifetime_parameters
                        .get(binding.parameter_ordinal as usize)
                        .ok_or_else(|| {
                            self.error("static helper lifetime lost its original binder ordinal")
                        })?;
                    let source = self
                        .program
                        .symbols
                        .symbol_provenance_source_span(owner)
                        .ok_or_else(|| {
                            self.error("static helper lifetime owner lost source custody")
                        })?;
                    let occurrence = self
                        .program
                        .symbols
                        .rejoin_product_occurrence(named.source_span);
                    if source.source_id == occurrence.source_id && parameter.as_str() == named.text
                    {
                        if matched.is_some() {
                            return Err(
                                self.error("static helper lifetime has multiple supplying frames")
                            );
                        }
                        let actual = environment.graph.names.get(binding.argument);
                        matched = Some(Identifier::generated(&actual.text));
                    }
                }
                Ok(matched.unwrap_or_else(|| self.identifier(name)))
            }
            pub(crate) fn lifetime_identifiers(
                &self,
                names: HandleSpan<ProductOperandName>,
            ) -> Result<Vec<Identifier>, Vec<Diagnostic>> {
                names
                    .handles()
                    .map(|name| self.lifetime_identifier(name))
                    .collect()
            }
        }
    }
    pub(crate) mod expressions {

        use super::*;
        use crate::tables::expressions_of_mut;
        use expr::MatchPattern;
        use expr::{ExpressionHandle, ExpressionNode as Node};
        use language_semantics::declaration_selection::AuthoredDeclarationSelectionKind;
        use language_semantics::declaration_selection::ResolvedAuthoredCallBinding;
        impl Lower<'_> {
            fn retain_expression_source(
                &mut self,
                expression: ExpressionHandle,
                source_span: source::SourceSpan,
                selections: HandleSpan<ProductOperandSelection>,
                call_target_span: Option<source::SourceSpan>,
            ) -> Result<(), Vec<Diagnostic>> {
                let retained = if selections.is_empty() {
                    &[][..]
                } else {
                    self.graph.selections.span(selections).ok_or_else(|| {
                        self.error("designated expression has a foreign authored-selection span")
                    })?
                };
                let mut occurrences = Vec::with_capacity(retained.len());
                let call_target_span = call_target_span
                    .map(|span| self.program.symbols.rejoin_product_occurrence(span));
                let mut call_selection = None;
                for selection in retained {
                    let kind = selection.kind.ok_or_else(|| {
                        self.error("designated expression lost its authored selection kind")
                    })?;
                    let span = self
                        .program
                        .symbols
                        .rejoin_product_occurrence(selection.source_span);
                    let occurrence = self
                .program
                .authored_declaration_selections()
                .source_coordinate_occurrence(span, selection.exposure, kind)
                .map_err(|reason| {
                    vec![Diagnostic::error(format!("designated expression cannot rejoin its {kind:?} selection ({:?}): {reason}", selection.exposure)).with_source_span(span)]
                })?;
                    if kind == AuthoredDeclarationSelectionKind::Call
                        && call_target_span == Some(span)
                    {
                        if call_selection.is_some_and(|previous| previous != occurrence) {
                            return Err(self.error(
                                "designated call has conflicting source-owned Call occurrences",
                            ));
                        }
                        call_selection = Some(occurrence);
                    }
                    occurrences.push(occurrence);
                }
                let span = self.program.symbols.rejoin_product_occurrence(source_span);
                expressions_of_mut(self.program).set_source_span(expression, span);
                expressions_of_mut(self.program)
                    .attach_authored_selection_occurrences(expression, occurrences);
                if let Node::Call(call) =
                    expressions_of_mut(self.program).expression_mut(expression)
                {
                    call.authored_call_selection = call_selection;
                }
                Ok(())
            }
            fn expression_optional(
                &mut self,
                handle: Handle<ProductOperandExpression>,
            ) -> Result<ExpressionHandle, Vec<Diagnostic>> {
                if handle.is_valid() {
                    self.expression(handle)
                } else {
                    Ok(Handle::invalid())
                }
            }
            fn expression_arguments(
                &mut self,
                span: HandleSpan<Handle<ProductOperandExpression>>,
            ) -> Result<HandleSpan<ExpressionHandle>, Vec<Diagnostic>> {
                let values = self
                    .graph
                    .expression_arguments
                    .span_or_empty(span)
                    .iter()
                    .copied()
                    .map(|value| self.expression(value))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(expressions_of_mut(self.program).insert_expression_handles(values))
            }
            fn source_subject(
                &mut self,
                handle: Handle<ProductMatchSubject>,
            ) -> Result<Handle<expr::CompletedValueCapture>, Vec<Diagnostic>> {
                if let Some((_, capture)) =
                    self.subjects.iter().find(|(source, _)| *source == handle)
                {
                    return Ok(*capture);
                }
                let subject = self.expression(self.graph.match_subjects.get(handle).subject)?;
                let capture = expressions_of_mut(self.program).insert_match_subject(subject);
                self.subjects.push((handle, capture));
                Ok(capture)
            }
            pub(crate) fn expression(
                &mut self,
                handle: Handle<ProductOperandExpression>,
            ) -> Result<ExpressionHandle, Vec<Diagnostic>> {
                if let Some((_, target)) = self
                    .expressions
                    .iter()
                    .find(|(source, _)| *source == handle)
                {
                    return Ok(*target);
                }
                if self.active_expressions.contains(&handle) {
                    return Err(self.error("designated constant expression graph contains a cycle"));
                }
                self.active_expressions.push(handle);
                let value = self.graph.expressions.get(handle).clone();
                if let ProductExpressionKind::Name(names) = &value.kind
                    && names.count() == 1
                    && let Some(name) = names.handles().next()
                    && let Some(actual) = self.environment_argument(name)?
                {
                    let declared = actual.declared_type.ok_or_else(|| {
                        self.error("a static type or callable actual cannot become a const value")
                    })?;
                    let reference = self
                        .program
                        .static_operand_type_reference_in_scope(&actual.argument, actual.owner)
                        .map_err(|reason| self.error(reason))?;
                    let node = crate::monomorphization::materialize_static_value(
                        self.program,
                        reference,
                        declared,
                    )
                    .ok_or_else(|| {
                        self.error("completed static actual has no ordinary const value")
                    })?;
                    let result = expressions_of_mut(self.program).insert(node);
                    self.retain_expression_source(
                        result,
                        value.source_span,
                        value.authored_selections,
                        None,
                    )?;
                    self.expressions.push((handle, result));
                    self.active_expressions.pop();
                    return Ok(result);
                }
                let call_target_span = match &value.kind {
                    ProductExpressionKind::Call { target, .. } => {
                        Some(self.graph.names.get(*target).source_span)
                    }
                    _ => None,
                };
                let node = match value.kind {
            ProductExpressionKind::Invalid => {
                return Err(self.error("designated constant expression has an invalid node"));
            }
            ProductExpressionKind::Integer(value) => Node::Integer(value),
            ProductExpressionKind::Boolean(value) => Node::Boolean(value),
            ProductExpressionKind::Float(value) => Node::Float(value),
            ProductExpressionKind::UndeterminedNaN(value) => Node::UndeterminedNaN(value),
            ProductExpressionKind::Text(value) => Node::String(value.into()),
            ProductExpressionKind::Name(names) => {
                let path = self.path(names)?;
                let last = names
                    .handles()
                    .last()
                    .ok_or_else(|| self.error("designated name has no source path"))?;
                let name = self.graph.names.get(last);
                let target = if name.declaration.is_some()
                    || name.builtin_type.is_some()
                    || name.builtin_function.is_some()
                {
                    self.symbol(last)?
                } else {
                    self.program
                        .symbols
                        .find_build_subject_declaration_from_source(
                            &path,
                            name.source_span,
                            self.candidates(),
                            &[],
                        )
                        .ok_or_else(|| {
                            self.error("designated constant name has no exact visible declaration")
                        })?
                };
                let owned = expr::Expression::Name(expr::NamePath::resolved(
                    self.identifiers(names),
                    target,
                    target,
                ));
                let target = expressions_of_mut(self.program).insert_tree(&owned);
                self.retain_expression_source(
                    target,
                    value.source_span,
                    value.authored_selections,
                    None,
                )?;
                self.active_expressions.pop();
                self.expressions.push((handle, target));
                return Ok(target);
            }
            ProductExpressionKind::Binary {
                left,
                operator,
                right,
            } => Node::Binary(expr::TableBinaryExpression {
                policy: numerics::arithmetic::ArithmeticDomain::Exact,
                left: self.expression(left)?,
                operator: binary(operator),
                right: self.expression(right)?,
            }),
            ProductExpressionKind::Unary { operator, operand } => {
                Node::Unary(expr::TableUnaryExpression {
                    operator: match operator {
                        ProductUnaryOperator::BitwiseNot => expr::UnaryOperator::BitwiseNot,
                        ProductUnaryOperator::LogicalNot => expr::UnaryOperator::LogicalNot,
                    },
                    operand: self.expression(operand)?,
                })
            }
            ProductExpressionKind::Array(values) => {
                Node::ArrayLiteral(self.expression_arguments(values)?)
            }
            ProductExpressionKind::Indexed { receiver, index } => {
                Node::Indexed(expr::TableIndexedExpression {
                    collection: self.expression(receiver)?,
                    index: self.expression(index)?,
                })
            }
            ProductExpressionKind::Member { receiver, member } => {
                Node::Member(expr::TableMemberExpression {
                    receiver: self.expression(receiver)?,
                    member_symbol: if self.graph.names.get(member).declaration.is_some() {
                        self.symbol(member)?
                    } else {
                        SymbolHandle::invalid()
                    },
                    member: self.identifier(member),
                    case_variant: None,
                })
            }
            ProductExpressionKind::Borrow { target, access } => {
                Node::Borrow(expr::TableBorrowExpression {
                    target: self.expression(target)?,
                    access,
                })
            }
            ProductExpressionKind::Atomic {
                value,
                result,
                ordering,
                result_custody,
            } => Node::Atomic(expr::TableAtomicExpression {
                value: self.expression(value)?,
                result: self.expression_optional(result)?,
                ordering,
                result_custody,
            }),
            ProductExpressionKind::Range {
                start,
                end,
                inclusive,
            } => Node::Range(expr::TableRangeExpression {
                start: self.expression_optional(start)?,
                end: self.expression_optional(end)?,
                end_inclusive: inclusive,
            }),
            ProductExpressionKind::ZeroValue(target) => Node::ZeroValue(self.ty(target)?),
            ProductExpressionKind::TypeExpression(target) => Node::TypeExpression(self.ty(target)?),
            ProductExpressionKind::Cast {
                value,
                target,
                policy,
                domain,
                domain_arguments,
                form,
            } => {
                let mut semantic_domain = HandleSpan::empty();
                for name in self.identifiers(domain) {
                    expressions_of_mut(self.program)
                        .push_name_path_member(&mut semantic_domain, name);
                }
                Node::Cast(expr::TableCastExpression {
                    value: self.expression(value)?,
                    target_type: self.ty(target)?,
                    result_type: Handle::invalid(),
                    target_label: HandleSpan::empty(),
                    domain: policy,
                    semantic_domain,
                    semantic_domain_arguments: self.type_arguments(domain_arguments)?,
                    semantic_domain_symbol: domain
                        .handles()
                        .last()
                        .filter(|name| self.graph.names.get(*name).declaration.is_some())
                        .map(|name| self.symbol(name))
                        .transpose()?
                        .unwrap_or_default(),
                    semantic_domain_id: Default::default(),
                    form,
                })
            }
            ProductExpressionKind::Call {
                receiver,
                target,
                owner,
                static_arguments,
                arguments,
                evidence,
                acknowledgement,
            } => {
                let requirement_application = self.ty_optional(owner)?;
                let target_symbol = self.symbol(target)?;
                let outer_path = std::mem::take(&mut self.input_path);
                let outer_selections = std::mem::take(&mut self.input_selections);
                let machine_arguments = self
                    .graph
                    .static_arguments
                    .span_or_empty(static_arguments)
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(ordinal, value)| self.argument(ordinal, value))
                    .collect::<Result<Vec<_>, _>>();
                let input_selections =
                    self.program
                        .product_input_selections
                        .insert_many(std::mem::replace(
                            &mut self.input_selections,
                            outer_selections,
                        ));
                self.input_path = outer_path;
                let machine_arguments = machine_arguments?.into_boxed_slice();
                let target_name = self.identifier(target);
                let authored_application = expressions_of_mut(self.program)
                    .retain_static_machine_application(
                        target_symbol,
                        target_name,
                        &machine_arguments,
                    );
                let source_application = expressions_of_mut(self.program)
                    .static_machine_application_mut(authored_application);
                source_application.lexical_owner = self.author;
                source_application.binding_declaration = target_symbol;
                source_application.input_selections = input_selections;
                Node::Call(expr::TableCallExpression {
                    runtime_call: None,
                    authored_call_selection: None,
                    authored_call_binding: Some(ResolvedAuthoredCallBinding {
                        selected: target_symbol,
                        original: target_symbol,
                    }),
                    exact_coercion_target: Handle::invalid(),
                    requirement_application,
                    receiver: self.expression_optional(receiver)?,
                    target_symbol,
                    target: self.identifier(target),
                    arguments: self.expression_arguments(arguments)?,
                    evidence_arguments: self.identifiers(evidence).into_boxed_slice(),
                    machine_arguments,
                    operational_acknowledgement: acknowledgement,
                    build_selection: Handle::invalid(),
                    authored_machine_application: authored_application,
                    static_machine_parameter: SymbolHandle::invalid(),
                    static_requirement_dispatch: None,
                    quotient_operation: None,
                    private_layout_operation: None,
                    installed_interrupt_operation: None,
                    layout_query: None,
                    native_code_layout: None,
                })
            }
            ProductExpressionKind::Record { name, case, fields } => {
                let type_symbol = self.symbol(name)?;
                let fields = self
                    .graph
                    .fields
                    .span_or_empty(fields)
                    .iter()
                    .map(|field| {
                        Ok(expr::TableStructLiteralField {
                            name: self.identifier(field.name),
                            field_symbol: self
                                .graph
                                .names
                                .get(field.name)
                                .declaration
                                .is_some()
                                .then(|| self.symbol(field.name))
                                .transpose()?
                                .unwrap_or_default(),
                            value: self.expression(field.value)?,
                        })
                    })
                    .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
                Node::StructLiteral(expr::TableStructLiteral {
                    application_type: Default::default(),
                    type_name: self.identifier(name),
                    type_symbol,
                    case_name: case.map(|name| self.identifier(name)),
                    case_symbol: case
                        .filter(|name| self.graph.names.get(*name).declaration.is_some())
                        .map(|name| self.symbol(name))
                        .transpose()?,
                    fields: expressions_of_mut(self.program).insert_struct_fields(fields),
                })
            }
            ProductExpressionKind::SavedSubject {
                component,
                capture,
                pattern_head,
            } => Node::SavedSubject(expr::TableSavedSubject {
                component,
                capture: self.source_subject(capture)?,
                pattern_head: self.expression_optional(pattern_head)?,
            }),
            ProductExpressionKind::Match {
                subject,
                capture,
                arms,
            } => {
                let subject = self.expression(subject)?;
                let capture = self.source_subject(capture)?;
                let arms = self
                    .graph
                    .match_arms
                    .span_or_empty(arms)
                    .iter()
                    .cloned()
                    .map(|arm| {
                        Ok(expr::TableMatchArm {
                            pattern: self.pattern(arm.pattern)?,
                            value: self.expression(arm.value)?,
                            source_span: self
                                .program
                                .symbols
                                .rejoin_product_occurrence(arm.source_span),
                        })
                    })
                    .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
                Node::Match(expr::TableMatchExpression {
                    subject,
                    capture,
                    arms: expressions_of_mut(self.program).insert_match_arms(arms),
                })
            }
            ProductExpressionKind::Membership { .. } => return Err(self.error(
                "designated constant membership must be normalized by its source operation owner",
            )),
        };
                let target = expressions_of_mut(self.program).insert(node);
                self.retain_expression_source(
                    target,
                    value.source_span,
                    value.authored_selections,
                    call_target_span,
                )?;
                self.active_expressions.pop();
                self.expressions.push((handle, target));
                Ok(target)
            }
            fn pattern(
                &mut self,
                pattern: ProductMatchPattern,
            ) -> Result<MatchPattern, Vec<Diagnostic>> {
                Ok(match pattern {
                    ProductMatchPattern::Wildcard => MatchPattern::Wildcard,
                    ProductMatchPattern::Value(value) => {
                        MatchPattern::Value(self.expression(value)?)
                    }
                    ProductMatchPattern::Destructure {
                        head,
                        fields,
                        has_rest,
                        guard,
                        condition,
                        proof_selectors,
                    } => {
                        let fields = self
                            .graph
                            .match_fields
                            .span_or_empty(fields)
                            .iter()
                            .map(|field| {
                                Ok(expr::TableMatchField {
                                    component: field.component,
                                    member: self.identifier(field.member),
                                    field_symbol: SymbolHandle::invalid(),
                                    action: match field.action {
                                        ProductMatchFieldAction::Bind(name) => {
                                            expr::MatchFieldAction::Bind(self.identifier(name))
                                        }
                                        ProductMatchFieldAction::Equal(value) => {
                                            expr::MatchFieldAction::Equal(self.expression(value)?)
                                        }
                                        ProductMatchFieldAction::Waive => {
                                            expr::MatchFieldAction::Waive
                                        }
                                    },
                                })
                            })
                            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
                        let selectors = self
                            .graph
                            .proof_selectors
                            .span_or_empty(proof_selectors)
                            .iter()
                            .map(|selector| typed_trees::statement::OutcomeProofSelector {
                                output_field: self.identifier(selector.output),
                                binding: self.identifier(selector.binding),
                            })
                            .collect::<Vec<_>>();
                        MatchPattern::Destructure(expr::TableDestructurePattern {
                            application_type: Default::default(),
                            head: self.expression(head)?,
                            fields: expressions_of_mut(self.program).insert_match_fields(fields),
                            has_rest,
                            guard: self.expression_optional(guard)?,
                            condition: self.expression_optional(condition)?,
                            proof_selectors: expressions_of_mut(self.program)
                                .insert_match_proof_selectors(selectors),
                        })
                    }
                })
            }
        }
        fn binary(value: ProductBinaryOperator) -> expr::BinaryOperator {
            match value {
                ProductBinaryOperator::CaseMembership => expr::BinaryOperator::CaseMembership,
                ProductBinaryOperator::Add => expr::BinaryOperator::Add,
                ProductBinaryOperator::And => expr::BinaryOperator::And,
                ProductBinaryOperator::BitwiseAnd => expr::BinaryOperator::BitwiseAnd,
                ProductBinaryOperator::BitwiseOr => expr::BinaryOperator::BitwiseOr,
                ProductBinaryOperator::BitwiseXor => expr::BinaryOperator::BitwiseXor,
                ProductBinaryOperator::Divide => expr::BinaryOperator::Divide,
                ProductBinaryOperator::Equal => expr::BinaryOperator::Equal,
                ProductBinaryOperator::Greater => expr::BinaryOperator::Greater,
                ProductBinaryOperator::GreaterOrEqual => expr::BinaryOperator::GreaterOrEqual,
                ProductBinaryOperator::Less => expr::BinaryOperator::Less,
                ProductBinaryOperator::LessOrEqual => expr::BinaryOperator::LessOrEqual,
                ProductBinaryOperator::Modulo => expr::BinaryOperator::Modulo,
                ProductBinaryOperator::Multiply => expr::BinaryOperator::Multiply,
                ProductBinaryOperator::NotEqual => expr::BinaryOperator::NotEqual,
                ProductBinaryOperator::Or => expr::BinaryOperator::Or,
                ProductBinaryOperator::ShiftLeft => expr::BinaryOperator::ShiftLeft,
                ProductBinaryOperator::ShiftRight => expr::BinaryOperator::ShiftRight,
                ProductBinaryOperator::Subtract => expr::BinaryOperator::Subtract,
            }
        }
    }
    pub(crate) mod input_selections {
        use super::*;
        use language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId;
        use typed_trees::product_operand::{
            ProductArgumentStep, ProductInputSelection, ProductInputSelectionTarget,
            ProductTypeSelectionSlot,
        };
        impl Lower<'_> {
            pub(crate) fn argument(
                &mut self,
                ordinal: usize,
                operand: Handle<ProductStaticOperand>,
            ) -> Result<expr::StaticMachineArgument, Vec<Diagnostic>> {
                let ordinal = u32::try_from(ordinal).map_err(|_| {
                    self.error("static input argument ordinal exceeds its representation")
                })?;
                self.input_path.push(ProductArgumentStep::Argument(ordinal));
                let result = self.operand(operand);
                self.input_path.pop();
                result
            }

            fn input_occurrence(
                &self,
                coordinate: Handle<ProductOperandSelection>,
            ) -> Result<AuthoredDeclarationSelectionOccurrenceId, Vec<Diagnostic>> {
                if !self.graph.selections.is_valid(coordinate) {
                    return Err(
                        self.error("static input has a foreign source-selection coordinate")
                    );
                }
                let selection = self.graph.selections.get(coordinate);
                let kind = selection
                    .kind
                    .ok_or_else(|| self.error("static input lost its source-selection kind"))?;
                let span = self
                    .program
                    .symbols
                    .rejoin_product_occurrence(selection.source_span);
                self.program
                    .authored_declaration_selections()
                    .source_coordinate_occurrence(span, selection.exposure, kind)
                    .map_err(|reason| {
                        vec![
                    Diagnostic::error(format!(
                        "static input cannot rejoin its {kind:?} selection ({:?}): {reason}",
                        selection.exposure
                    ))
                    .with_source_span(span),
                ]
                    })
            }

            fn input_ordinal(&self) -> Result<u32, Vec<Diagnostic>> {
                match self.input_path.first() {
                    Some(ProductArgumentStep::Argument(ordinal)) => Ok(*ordinal),
                    _ => Err(self.error("static input has no enclosing actual argument")),
                }
            }

            pub(crate) fn retain_operand_input(
                &mut self,
                coordinate: Handle<ProductOperandSelection>,
            ) -> Result<(), Vec<Diagnostic>> {
                if !coordinate.is_valid() {
                    return Ok(());
                }
                let occurrence = self.input_occurrence(coordinate)?;
                let argument_ordinal = self.input_ordinal()?;
                let path = self
                    .program
                    .product_input_argument_steps
                    .insert_many(self.input_path.iter().copied());
                self.input_selections.push(ProductInputSelection {
                    occurrence: Some(occurrence),
                    argument_ordinal,
                    target: ProductInputSelectionTarget::Argument(path),
                });
                Ok(())
            }

            pub(crate) fn retain_type_input(
                &mut self,
                source: Handle<ProductOperandType>,
                reference: TypeReferenceHandle,
            ) -> Result<(), Vec<Diagnostic>> {
                let (name, slot) = match self.graph.types.get(source) {
                    ProductOperandType::Named(name)
                    | ProductOperandType::Generic { name, .. }
                    | ProductOperandType::Dynamic { name, .. } => {
                        (*name, ProductTypeSelectionSlot::Declaration)
                    }
                    ProductOperandType::Array {
                        length: ProductArrayLength::Parameter(name) | ProductArrayLength::Call(name),
                        ..
                    } => (*name, ProductTypeSelectionSlot::ArrayExtent),
                    _ => return Ok(()),
                };
                let coordinate = self.graph.names.get(name).authored_selection;
                if !coordinate.is_valid() {
                    return Ok(());
                }
                let occurrence = self.input_occurrence(coordinate)?;
                let argument_ordinal = self.input_ordinal()?;
                let selection = ProductInputSelection {
                    occurrence: Some(occurrence),
                    argument_ordinal,
                    target: ProductInputSelectionTarget::Type { reference, slot },
                };
                if !self.input_selections.contains(&selection) {
                    self.input_selections.push(selection);
                }
                Ok(())
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PreparedProductRequirement {
        pub requirement: SymbolHandle,
        pub closed_requirement: Option<SymbolHandle>,
        pub trait_scope: Option<RetainedTraitScope>,
    }

    pub fn prepare_product_requirement_operand(
        program: &mut TypedTrees,
        operand: &ProductDeclarationOperand,
        author: SymbolHandle,
        static_environment: Option<&ProductStaticEnvironment>,
    ) -> Result<PreparedProductRequirement, Vec<Diagnostic>> {
        let refuse =
            |reason: &str| vec![Diagnostic::error(reason).with_source_span(operand.source_span)];
        let Some(graph) = operand.application.as_deref() else {
            return resolve_product_path(
                program,
                &operand.path,
                operand.source_span,
                ProductOperandRole::Requirement,
            )
            .map(|requirement| PreparedProductRequirement {
                requirement,
                closed_requirement: None,
                trait_scope: None,
            })
            .ok_or_else(|| {
                refuse("native binding must name one exact visible package-owned requirement")
            });
        };
        let author = environment::supplier_owner(program, static_environment, author)?;
        let mut lower = Lower::new(program, graph, author, static_environment);
        if !graph.operands.is_valid(graph.root) {
            return Err(refuse("designated requirement has no owned root operand"));
        }
        let root = lower.graph.operands.get(graph.root).clone();
        if !graph.applications.is_valid(root.application) {
            return Err(refuse("designated requirement has no owned application"));
        }
        let root_application = graph.applications.get(root.application).clone();
        let (requirement, environment, method) = if root_application.member.is_valid() {
            let path = lower.path(root.path)?;
            let owner = resolve_product_path(
                lower.program,
                &path,
                operand.source_span,
                ProductOperandRole::BoundarySubject,
            )
            .ok_or_else(|| {
                refuse("applied native requirement owner is not one visible boundary declaration")
            })?;
            let application = lower.application_arguments(root.application)?;
            let mut owner_argument = expr::StaticMachineArgument {
                path: Box::from([Identifier::generated(path)]),
                symbol: owner,
                application: Some(Box::new(application)),
                ..Default::default()
            };
            owner_argument
                .application
                .as_mut()
                .expect("created above")
                .member = Handle::invalid();
            let reference = lower
                .program
                .static_operand_type_reference_in_scope(&owner_argument, author)
                .map_err(refuse)?;
            lower.program.retain_designated_static_type_argument(
                author,
                reference,
                operand.source_span,
            );
            normalize_const_arguments(
                lower.program,
                author,
                std::slice::from_mut(&mut owner_argument),
            )
            .map_err(|reason| refuse(&reason))?;
            let reference = lower
                .program
                .static_operand_type_reference_in_scope(&owner_argument, author)
                .map_err(refuse)?;
            lower.program.retain_designated_static_type_argument(
                author,
                reference,
                operand.source_span,
            );
            let scope = AppliedTraitScope::from_type_reference(lower.program, reference, author)
                .ok_or_else(|| {
                    refuse("native requirement has an invalid or incomplete outer application")
                })?;
            lower
                .program
                .require_receiving_trait_operands(&scope)
                .map_err(refuse)?;
            let environment = scope.environment();
            let member = lower.graph.operands.get(root_application.member).clone();
            let member_path = lower.path(member.path)?;
            let joined = format!(
                "{}::{member_path}",
                lower.program.symbols.display_path(owner, "::")
            );
            let requirement = resolve_product_path(
                lower.program,
                &joined,
                operand.source_span,
                ProductOperandRole::Requirement,
            )
            .ok_or_else(|| {
                refuse(
                    "applied native requirement has no exact visible member in its selected owner",
                )
            })?;
            if lower
                .program
                .trait_machine_signature(requirement)
                .is_none_or(|(definition, _)| definition.symbol != owner)
            {
                return Err(refuse(
                    "applied requirement member does not belong to its exact selected owner",
                ));
            }
            (requirement, Some(environment), member.application)
        } else {
            let requirement = resolve_product_path(
                lower.program,
                &operand.path,
                operand.source_span,
                ProductOperandRole::Requirement,
            )
            .ok_or_else(|| {
                refuse("native binding must name one exact visible package-owned requirement")
            })?;
            (requirement, None, root.application)
        };
        let signatures = lower
            .program
            .prepare_applied_callable_signatures()
            .map_err(&refuse)?;
        crate::monomorphization::finalize_callable_signature_identities(lower.program, signatures)
            .map_err(|diagnostic| vec![diagnostic])?;
        let Some((owner, signature)) = lower.program.trait_machine_signature(requirement) else {
            return Err(refuse(
                "an applied native declaration must be a public requirement member",
            ));
        };
        let environment =
            environment.unwrap_or_else(|| AppliedTraitScope::abstract_scope(owner).environment());
        let parameters = lower
            .program
            .state_signature_type_parameters(signature)
            .to_vec();
        let lifetime_parameters = signature.lifetime_parameters.to_vec();
        let name = signature.name.clone();
        let trait_arguments = lower
            .program
            .materialize_trait_application_arguments(&environment)
            .map_err(refuse)?;

        let supplied = if method.is_valid() {
            Some(lower.application(method)?)
        } else {
            None
        };
        let has_outer = root_application.member.is_valid();
        if let Some(supplied) = &supplied
            && (supplied.member.is_valid()
                || supplied.arguments.len() != parameters.len()
                || supplied.lifetime_arguments.len() != lifetime_parameters.len())
        {
            return Err(refuse(
                "native requirement application requires its complete exact member telescope",
            ));
        }
        let retained_scope = has_outer
            .then(|| environment.retain(lower.program))
            .transpose()
            .map_err(refuse)?;
        if supplied.is_some() || (has_outer && parameters.is_empty()) {
            let mut supplied = supplied.unwrap_or(expr::StaticSymbolApplication {
                lifetime_arguments: Box::new([]),
                arguments: Box::new([]),
                member: Handle::invalid(),
            });
            let authored_arguments = supplied.arguments.clone();
            normalize_const_arguments(lower.program, author, &mut supplied.arguments)
                .map_err(|reason| refuse(&reason))?;
            let bindings = crate::monomorphization::requirement_applications::invocation_bindings(
                lower.program,
                author,
                &parameters,
                &supplied.arguments,
            )?;
            let lifetimes = lifetime_parameters
                .iter()
                .zip(supplied.lifetime_arguments.iter())
                .map(|(parameter, argument)| StaticMachineLifetimeBinding {
                    parameter: parameter.clone(),
                    argument: argument.clone(),
                })
                .collect::<Vec<_>>();
            let application = expressions_of_mut(lower.program).retain_static_machine_application(
                requirement,
                name,
                &authored_arguments,
            );
            let source =
                expressions_of_mut(lower.program).static_machine_application_mut(application);
            source.lexical_owner = author;
            source.binding_declaration = requirement;
            expressions_of_mut(lower.program)
                .set_static_machine_application_bindings(application, &bindings);
            expressions_of_mut(lower.program).set_static_machine_application_lifetimes(
                application,
                requirement,
                &lifetimes,
            );
            let instance = lower
                .program
                .instantiate_requirement_application(
                    &environment,
                    requirement,
                    &trait_arguments,
                    application,
                )
                .map_err(refuse)?;
            return Ok(PreparedProductRequirement {
                requirement,
                closed_requirement: Some(instance),
                trait_scope: retained_scope,
            });
        }
        Ok(PreparedProductRequirement {
            requirement,
            closed_requirement: None,
            trait_scope: retained_scope,
        })
    }

    #[derive(Clone, Copy)]
    enum OperandNaming {
        AuthoredScope,
        RetainedDeclarations,
    }

    struct Lower<'a> {
        program: &'a mut TypedTrees,
        graph: &'a ProductStaticOperandGraph,
        author: SymbolHandle,
        static_environment: Option<&'a ProductStaticEnvironment>,
        naming: OperandNaming,
        active_bindings: Vec<source::SourceSpan>,
        input_path: Vec<typed_trees::product_operand::ProductArgumentStep>,
        input_selections: Vec<typed_trees::product_operand::ProductInputSelection>,
        types: Vec<(Handle<ProductOperandType>, TypeReferenceHandle)>,
        expressions: Vec<(Handle<ProductOperandExpression>, expr::ExpressionHandle)>,
        subjects: Vec<(Handle<ProductMatchSubject>, Handle<CompletedValueCapture>)>,
        active_types: Vec<Handle<ProductOperandType>>,
        active_expressions: Vec<Handle<ProductOperandExpression>>,
        active_operands: Vec<Handle<ProductStaticOperand>>,
    }
    impl<'a> Lower<'a> {
        fn new(
            program: &'a mut TypedTrees,
            graph: &'a ProductStaticOperandGraph,
            author: SymbolHandle,
            static_environment: Option<&'a ProductStaticEnvironment>,
        ) -> Self {
            Self {
                program,
                graph,
                author,
                static_environment,
                naming: OperandNaming::AuthoredScope,
                active_bindings: Vec::new(),
                input_path: Vec::new(),
                input_selections: Vec::new(),
                types: Vec::new(),
                expressions: Vec::new(),
                subjects: Vec::new(),
                active_types: Vec::new(),
                active_expressions: Vec::new(),
                active_operands: Vec::new(),
            }
        }
    }
    impl Lower<'_> {
        fn error(&self, reason: &str) -> Vec<Diagnostic> {
            vec![Diagnostic::error(reason)]
        }
        fn identifier(&self, name: Handle<ProductOperandName>) -> Identifier {
            Identifier::generated(&self.graph.names.get(name).text)
        }
        fn identifiers(&self, span: HandleSpan<ProductOperandName>) -> Vec<Identifier> {
            self.graph
                .names
                .span_or_empty(span)
                .iter()
                .map(|name| Identifier::generated(&name.text))
                .collect()
        }
        fn path(&self, span: HandleSpan<ProductOperandName>) -> Result<String, Vec<Diagnostic>> {
            let names = self.graph.names.span_or_empty(span);
            if names.is_empty() || names.len() != span.len() {
                return Err(self.error("designated operand has no complete source path"));
            }
            Ok(names
                .iter()
                .map(|name| name.text.as_str())
                .collect::<Vec<_>>()
                .join("::"))
        }
        fn candidates(&self) -> Vec<(SymbolHandle, bool)> {
            let program = self.program.product_declarations();
            public_symbols!(program.data_definitions())
                .chain(program.data_definitions().iter().flat_map(|owner| {
                    program
                        .data_members(owner)
                        .iter()
                        .flat_map(move |member| match member {
                            typed_trees::data::DataMember::Field(field) => {
                                vec![(field.symbol, owner.is_public)]
                            }
                            typed_trees::data::DataMember::Variant(case) => {
                                std::iter::once((case.symbol, owner.is_public))
                                    .chain(
                                        program
                                            .data_payload_fields(case)
                                            .iter()
                                            .map(|field| (field.symbol, owner.is_public)),
                                    )
                                    .collect()
                            }
                        })
                }))
                .chain(public_symbols!(program.traits()))
                .chain(public_symbols!(program.conformances()))
                .chain(public_symbols!(program.const_declarations()))
                .chain(public_symbols!(program.domain_definitions()))
                .chain(public_symbols!(program.machines()))
                .chain(program.traits().iter().flat_map(|owner| {
                    program
                        .trait_machine_signatures(owner)
                        .iter()
                        .map(move |value| (value.symbol, owner.is_public))
                }))
                .collect()
        }
        fn symbol(
            &self,
            name: Handle<ProductOperandName>,
        ) -> Result<SymbolHandle, Vec<Diagnostic>> {
            self.symbol_named(name, &self.graph.names.get(name).text)
        }
        fn symbol_named(
            &self,
            name: Handle<ProductOperandName>,
            path: &str,
        ) -> Result<SymbolHandle, Vec<Diagnostic>> {
            let name = self.graph.names.get(name);
            if let Some(atom) = name.builtin_type {
                return self
                    .program
                    .symbols
                    .child_handles(self.program.symbols.root())
                    .and_then(|mut children| {
                        children.find(|symbol| {
                            self.program.symbols.builtin_type_atom(*symbol) == Some(atom)
                        })
                    })
                    .ok_or_else(|| self.error("designated type lost its certified builtin atom"));
            }
            if let Some(function) = name.builtin_function {
                return self
                    .program
                    .symbols
                    .builtin_function_symbol(function)
                    .ok_or_else(|| {
                        self.error("designated expression lost its certified builtin function")
                    });
            }
            if matches!(self.naming, OperandNaming::RetainedDeclarations) {
                let declaration = name.declaration.ok_or_else(|| self.error("retained static input has no exact declaration anchor or certified compiler atom"))?;
                let mut matching = self
                    .candidates()
                    .into_iter()
                    .map(|(symbol, _)| symbol)
                    .filter(|symbol| {
                        self.program.symbols.symbol_provenance_source_span(*symbol)
                            == Some(declaration)
                    });
                let selected = matching.next().ok_or_else(|| {
                    self.error(
                        "retained static input declaration is outside the admitted Product closure",
                    )
                })?;
                if matching.any(|other| other != selected) {
                    return Err(self.error("retained static input declaration anchor is ambiguous"));
                }
                return Ok(selected);
            }
            if let Some(parameter) = self.lexical_static_parameter(self.author, name, path)? {
                return Ok(parameter);
            }
            if name.declaration.is_none()
                && let Some(symbol) = self
                    .program
                    .symbols
                    .find_top_level_by_name_and_kinds_from_source(
                        path,
                        &[symbols::SymbolKind::BuiltinType, symbols::SymbolKind::Data],
                        self.program
                            .symbols
                            .rejoin_product_occurrence(name.source_span),
                    )
                && self.program.symbols.builtin_type_atom(symbol).is_some()
            {
                return Ok(symbol);
            }
            let candidates = self
                .candidates()
                .into_iter()
                .filter(|(symbol, _)| {
                    name.declaration.is_none_or(|declaration| {
                        self.program.symbols.symbol_provenance_source_span(*symbol)
                            == Some(self.program.symbols.rejoin_product_occurrence(declaration))
                    })
                })
                .collect::<Vec<_>>();
            self.program
                .symbols
                .find_build_subject_declaration_from_source(path, name.source_span, candidates, &[])
                .ok_or_else(|| {
                    vec![
                    Diagnostic::error(format!(
                        "designated static operand `{}` has no exact visible source declaration",
                        path
                    ))
                    .with_source_span(name.source_span),
                ]
                })
        }
        fn application_arguments(
            &mut self,
            handle: Handle<ProductStaticApplication>,
        ) -> Result<expr::StaticSymbolApplication, Vec<Diagnostic>> {
            let value = self.graph.applications.get(handle).clone();
            let arguments = self
                .graph
                .static_arguments
                .span_or_empty(value.arguments)
                .iter()
                .copied()
                .enumerate()
                .map(|(ordinal, value)| self.argument(ordinal, value))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(expr::StaticSymbolApplication {
                arguments: arguments.into_boxed_slice(),
                lifetime_arguments: self
                    .lifetime_identifiers(value.lifetimes)?
                    .into_boxed_slice(),
                member: Handle::invalid(),
            })
        }
        fn application(
            &mut self,
            handle: Handle<ProductStaticApplication>,
        ) -> Result<expr::StaticSymbolApplication, Vec<Diagnostic>> {
            let value = self.graph.applications.get(handle).clone();
            let arguments = self
                .graph
                .static_arguments
                .span_or_empty(value.arguments)
                .iter()
                .copied()
                .enumerate()
                .map(|(ordinal, value)| self.argument(ordinal, value))
                .collect::<Result<Vec<_>, _>>()?;
            let member = if value.member.is_valid() {
                self.input_path
                    .push(typed_trees::product_operand::ProductArgumentStep::Member);
                let argument = self.operand(value.member);
                self.input_path.pop();
                let argument = argument?;
                expressions_of_mut(self.program).insert_static_member(argument)
            } else {
                Handle::invalid()
            };
            Ok(expr::StaticSymbolApplication {
                arguments: arguments.into_boxed_slice(),
                lifetime_arguments: self
                    .lifetime_identifiers(value.lifetimes)?
                    .into_boxed_slice(),
                member,
            })
        }
        fn operand(
            &mut self,
            handle: Handle<ProductStaticOperand>,
        ) -> Result<expr::StaticMachineArgument, Vec<Diagnostic>> {
            if self.active_operands.contains(&handle) {
                return Err(self.error("designated static operand graph contains a cycle"));
            }
            self.active_operands.push(handle);
            let value = self.graph.operands.get(handle).clone();
            if value.path.count() == 1
                && !value.application.is_valid()
                && !value.structural_type.is_valid()
                && let Some(name) = value.path.handles().next()
                && let Some(actual) = self.environment_argument(name)?
            {
                self.active_operands.pop();
                return Ok(actual.argument);
            }
            let type_reference = self.ty_optional(value.structural_type)?;
            let application = value
                .application
                .is_valid()
                .then(|| self.application(value.application))
                .transpose()?
                .map(Box::new);
            let symbol = if let Some(last) = value.path.handles().last() {
                self.symbol_named(last, &self.path(value.path)?)?
            } else {
                SymbolHandle::invalid()
            };
            let evidence_projection = value.projection.map(|(term, member)| EvidenceProjection {
                term: self.identifier(term),
                member: self.identifier(member),
            });
            self.active_operands.pop();
            self.retain_operand_input(value.authored_selection)?;
            Ok(expr::StaticMachineArgument {
                type_reference,
                application,
                symbol,
                path: self.identifiers(value.path).into_boxed_slice(),
                const_literal: value.integer,
                evidence_projection,
            })
        }
        fn ty_optional(
            &mut self,
            handle: Handle<ProductOperandType>,
        ) -> Result<TypeReferenceHandle, Vec<Diagnostic>> {
            if handle.is_valid() {
                self.ty(handle)
            } else {
                Ok(Handle::invalid())
            }
        }
        fn type_arguments(
            &mut self,
            span: HandleSpan<Handle<ProductOperandType>>,
        ) -> Result<HandleSpan<TypeReferenceHandle>, Vec<Diagnostic>> {
            let values = self
                .graph
                .type_arguments
                .span_or_empty(span)
                .iter()
                .copied()
                .map(|value| self.ty(value))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(self
                .program
                .type_reference_table
                .insert_type_reference_handles(values))
        }
        fn ty(
            &mut self,
            handle: Handle<ProductOperandType>,
        ) -> Result<TypeReferenceHandle, Vec<Diagnostic>> {
            if let Some((_, target)) = self.types.iter().find(|(source, _)| *source == handle) {
                let target = *target;
                self.retain_type_input(handle, target)?;
                return Ok(target);
            }
            if self.active_types.contains(&handle) {
                return Err(self.error("designated type graph contains a cycle"));
            }
            self.active_types.push(handle);
            if let ProductOperandType::Named(name) = self.graph.types.get(handle)
                && let Some(actual) = self.environment_argument(*name)?
            {
                let reference = self
                    .program
                    .static_operand_type_reference_in_scope(&actual.argument, actual.owner)
                    .map_err(|reason| self.error(reason))?;
                self.program
                    .type_reference_table
                    .retain_operand_lifetime_owner(reference, actual.owner)
                    .map_err(|reason| self.error(reason))?;
                self.types.push((handle, reference));
                self.active_types.pop();
                return Ok(reference);
            }
            let node = match self.graph.types.get(handle).clone() {
                ProductOperandType::CanonicalConst(value) => TypeReferenceNode::Named {
                    symbol: SymbolHandle::invalid(),
                    name: Identifier::generated(value.atom()),
                },
                ProductOperandType::Invalid => {
                    return Err(self.error("designated type graph contains an invalid node"));
                }
                ProductOperandType::Unit => TypeReferenceNode::Unit,
                ProductOperandType::Named(name) => TypeReferenceNode::Named {
                    symbol: self.symbol(name)?,
                    name: self.identifier(name),
                },
                ProductOperandType::Generic {
                    name,
                    lifetimes,
                    arguments,
                } => TypeReferenceNode::Generic {
                    base_symbol: self.symbol(name)?,
                    base_name: self.identifier(name),
                    lifetime_arguments: self.lifetime_identifiers(lifetimes)?,
                    arguments: self.type_arguments(arguments)?,
                },
                ProductOperandType::Dynamic { name, arguments } => {
                    TypeReferenceNode::DynamicTrait {
                        symbol: self.symbol(name)?,
                        name: self.identifier(name),
                        arguments: self.type_arguments(arguments)?,
                    }
                }
                ProductOperandType::Reference {
                    referee,
                    access,
                    lifetime,
                } => TypeReferenceNode::Reference {
                    referee: self.ty(referee)?,
                    access,
                    lifetime: lifetime
                        .map(|name| self.lifetime_identifier(name))
                        .transpose()?,
                },
                ProductOperandType::Array { element, length } => TypeReferenceNode::FixedArray {
                    element_type: self.ty(element)?,
                    length: match length {
                        ProductArrayLength::Literal(value) => FixedArrayLength::Literal(value),
                        ProductArrayLength::Expression(value) => {
                            FixedArrayLength::Expression(self.ty(value)?)
                        }
                        ProductArrayLength::Parameter(name) => FixedArrayLength::ConstParameter {
                            symbol: self.symbol(name)?,
                            name: self.identifier(name),
                        },
                        ProductArrayLength::Call(name) => FixedArrayLength::ConstCall {
                            machine: self.symbol(name)?,
                            name: self.identifier(name),
                            source_span: self.graph.names.get(name).source_span,
                        },
                    },
                },
                ProductOperandType::Slice(element) => TypeReferenceNode::Slice {
                    element_type: self.ty(element)?,
                },
                ProductOperandType::ConstExpression(expression) => {
                    TypeReferenceNode::ConstExpression(self.expression(expression)?)
                }
                ProductOperandType::Constrained { base, constraints } => {
                    let base_type = self.ty(base)?;
                    let values = self
                        .graph
                        .constraints
                        .span_or_empty(constraints)
                        .iter()
                        .cloned()
                        .map(|constraint| self.constraint(constraint))
                        .collect::<Result<Vec<_>, _>>()?;
                    TypeReferenceNode::Constrained {
                        base_type,
                        constraints: self.program.type_reference_table.insert_constraints(values),
                    }
                }
            };
            let target = self.program.type_reference_table.insert(node);
            self.program
                .type_reference_table
                .retain_operand_lifetime_owner(target, self.author)
                .map_err(|reason| self.error(reason))?;
            self.active_types.pop();
            self.types.push((handle, target));
            self.retain_type_input(handle, target)?;
            Ok(target)
        }
        fn constraint(
            &mut self,
            constraint: ProductOperandConstraint,
        ) -> Result<types::TypeConstraintNode, Vec<Diagnostic>> {
            Ok(match constraint {
                ProductOperandConstraint::Invalid => {
                    return Err(self.error("designated constraint graph contains an invalid node"));
                }
                ProductOperandConstraint::Named(name) => {
                    types::TypeConstraintNode::Named(self.identifier(name))
                }
                ProductOperandConstraint::Arithmetic(policy) => {
                    types::TypeConstraintNode::ArithmeticDomain(policy)
                }

                ProductOperandConstraint::Domain { name, arguments } => {
                    let span = self.type_arguments(arguments)?;
                    types::TypeConstraintNode::Domain(types::DomainConstraint { name: self.identifier(name), symbol: self.symbol(name)?, arguments: self.program.type_reference_table.type_reference_handles(span).to_vec(), source_selection: Some(types::DomainConstraintSource { source_span: self.graph.names.get(name).source_span, authored_exposure: Some(language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation) }), ..Default::default() })
                }
            })
        }
    }
}
