/*
We track typed symbol, declaration, and call-application reference occurrences
(`references`) across `TypedTrees`.
*/

use crate::expression::BuildSelectionRequest;
use crate::machine::Machine;
use crate::mathematical::MathematicalTypeHandle;
use crate::static_applications::StaticMachineApplicationHandle;
use crate::{
    TypedTrees,
    domain::ProofFact,
    expression::{
        ExpressionHandle, ExpressionNode, StaticMachineArgument, TableCallExpression,
        TableMatchArm, TableNamePath,
    },
    machine::TraitConformance,
    proposition::{PropositionApplication, PropositionBinderArgumentKind, PropositionFormula},
    signature::{AuthoredInvocationTarget, SignatureContract},
    statement::{StatementHandle, StatementNode, TransitionTargetNode},
    types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode},
};
use symbols::SymbolHandle;

pub(crate) mod source_occurrences {
    use super::{
        DeclarationSelection, HashSet, StaticArgumentCall, TypeOccurrence, TypeOccurrences,
        TypedReferences, visit_alias, visit_const, visit_data, visit_declarations, visit_machine,
        visit_program, visit_trait,
    };
    use crate::{TypedTrees, expression::ExpressionHandle, types::TypeReferenceHandle};
    use symbols::{SymbolHandle, SymbolKind};

    pub(crate) mod call_applications {
        use crate::{TypedTrees, expression::ExpressionNode, expression::StaticMachineArgument};
        use symbols::SymbolHandle;

        impl TypedTrees {
            pub fn retain_static_argument_contexts(&mut self) -> Result<(), &'static str> {
                use crate::expression::ExpressionHandle;
                use crate::references::{TypedReferences, visit_program};
                use crate::statement::StatementNode;

                #[derive(Default)]
                struct SourceCalls {
                    owner: SymbolHandle,
                    outer: Vec<SymbolHandle>,
                    visited: std::collections::HashSet<(SymbolHandle, ExpressionHandle)>,
                    expressions: Vec<(ExpressionHandle, SymbolHandle)>,
                }
                impl Extend<SymbolHandle> for SourceCalls {
                    fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {}
                }
                impl TypedReferences for SourceCalls {
                    fn derived_declaration_references(&self) -> bool {
                        false
                    }
                    fn declaration(&mut self, owner: SymbolHandle) -> bool {
                        self.owner = owner;
                        self.outer.clear();
                        true
                    }
                    fn enter_lexical_scope(&mut self, owner: SymbolHandle) {
                        self.outer.push(self.owner);
                        if owner.is_valid() {
                            self.owner = owner;
                        }
                    }
                    fn leave_lexical_scope(&mut self) {
                        if let Some(owner) = self.outer.pop() {
                            self.owner = owner;
                        }
                    }
                    fn expression(&mut self, expression: ExpressionHandle) -> bool {
                        if !self.visited.insert((self.owner, expression)) {
                            return false;
                        }
                        self.expressions.push((expression, self.owner));
                        true
                    }
                }
                let mut roots = SourceCalls::default();
                visit_program(self, &mut roots);
                for (expression, owner) in roots.expressions {
                    self.retain_call_static_argument_context(expression, owner)?;
                }
                let statement_calls = self
                    .machines()
                    .iter()
                    .flat_map(|machine| {
                        self.machine_states(machine).iter().flat_map(|state| {
                            self.statement_table
                                .iter_statements(state.statement_nodes)
                                .filter_map(|(handle, statement)| {
                                    let call = statement.as_call()?;
                                    (call.authored_machine_application.is_valid()
                                        || !call.machine_arguments.is_empty()
                                        || has_region_telescope(self, call.target_symbol)
                                        || implementation_namespace(self, call.receiver_symbol)
                                        || (!call.static_machine_parameter.is_valid()
                                            && call.static_requirement_dispatch.is_none()
                                            && direct_declaration(self, call.target_symbol)
                                                .is_some()))
                                    .then(|| (handle, machine.symbol, call.clone()))
                                })
                        })
                    })
                    .collect::<Vec<_>>();
                for (handle, owner, mut call) in statement_calls {
                    let original_arguments = call.machine_arguments.clone();
                    self.retain_static_operand_sources(&mut call.machine_arguments, owner)?;
                    let declaration = (!call.static_machine_parameter.is_valid()
                        && call.static_requirement_dispatch.is_none())
                    .then(|| direct_declaration(self, call.target_symbol))
                    .flatten();
                    let application = if call.authored_machine_application.is_valid() {
                        call.authored_machine_application
                    } else {
                        self.expression_table.retain_static_machine_application(
                            call.target_symbol,
                            call.target.clone(),
                            &call.machine_arguments,
                        )
                    };
                    let retained = self
                        .expression_table
                        .static_machine_application_mut(application);
                    if !retained.lexical_owner.is_valid() {
                        retained.lexical_owner = owner;
                    }
                    if !retained.binding_declaration.is_valid()
                        && let Some(declaration) = declaration
                    {
                        retained.binding_declaration = declaration;
                    }
                    self.retain_application_operand_sources(
                        application,
                        &original_arguments,
                        &call.machine_arguments,
                    )?;
                    let arguments = call.machine_arguments;
                    let StatementNode::Call(call) = self.statement_table.statement_mut(handle)
                    else {
                        unreachable!("retaining a source application does not replace its call");
                    };
                    call.authored_machine_application = application;
                    call.machine_arguments = arguments;
                }
                Ok(())
            }

            /// Gives the call at `expression` the source application an
            /// authored call has: its target, its static arguments and the
            /// declaration `owner` whose names those arguments use.
            ///
            /// The program-wide pass above runs once. A stage that turns an
            /// expression into a call afterwards uses this for that call, so
            /// its result is judged like the same call written by hand.
            pub fn retain_call_static_argument_context(
                &mut self,
                expression: crate::expression::ExpressionHandle,
                owner: SymbolHandle,
            ) -> Result<(), &'static str> {
                let Some(mut call) = self.expression_table.call_node(expression).cloned() else {
                    return Ok(());
                };
                let original_arguments = call.machine_arguments.clone();
                self.retain_static_operand_sources(&mut call.machine_arguments, owner)?;
                // An ordinary call still has a source application when its
                // static tuple is empty. Result evidence needs that exact
                // declaration and lexical owner too.
                let declaration = (!call.static_machine_parameter.is_valid()
                    && !call.requirement_application.is_valid()
                    && call.static_requirement_dispatch.is_none())
                .then(|| direct_declaration(self, call.target_symbol))
                .flatten();
                if call.authored_machine_application.is_valid() {
                    let application = call.authored_machine_application;
                    let retained = self
                        .expression_table
                        .static_machine_application_mut(application);
                    if !retained.lexical_owner.is_valid() {
                        retained.lexical_owner = owner;
                    }
                    if !retained.binding_declaration.is_valid()
                        && let Some(declaration) = declaration
                    {
                        retained.binding_declaration = declaration;
                    }
                    self.retain_application_operand_sources(
                        application,
                        &original_arguments,
                        &call.machine_arguments,
                    )?;
                    let ExpressionNode::Call(destination) =
                        self.expression_table.expression_mut(expression)
                    else {
                        unreachable!("retaining operand sources does not replace its call");
                    };
                    destination.machine_arguments = call.machine_arguments;
                    return Ok(());
                }
                let namespace = self
                    .expression_table
                    .name_path_node(call.receiver)
                    .map_or_else(SymbolHandle::invalid, |path| path.symbol);
                if call.machine_arguments.is_empty()
                    && !has_region_telescope(self, call.target_symbol)
                    && !implementation_namespace(self, namespace)
                    && declaration.is_none()
                {
                    return Ok(());
                }
                let application = self.expression_table.retain_static_machine_application(
                    call.target_symbol,
                    call.target,
                    &call.machine_arguments,
                );
                let retained = self
                    .expression_table
                    .static_machine_application_mut(application);
                retained.lexical_owner = owner;
                if let Some(declaration) = declaration {
                    retained.binding_declaration = declaration;
                }
                self.retain_application_operand_sources(
                    application,
                    &original_arguments,
                    &call.machine_arguments,
                )?;
                let arguments = call.machine_arguments;
                let ExpressionNode::Call(call) = self.expression_table.expression_mut(expression)
                else {
                    unreachable!("retaining a source application does not replace its call");
                };
                call.authored_machine_application = application;
                call.machine_arguments = arguments;
                Ok(())
            }

            fn retain_static_operand_sources(
                &mut self,
                arguments: &mut [StaticMachineArgument],
                owner: SymbolHandle,
            ) -> Result<bool, &'static str> {
                // An unresolved child defers its enclosing application until
                // this same context pass runs after ordinary resolution.
                let mut complete = true;
                for argument in arguments {
                    if argument.type_reference.is_valid() {
                        // A supplied Type already has its representation, but still
                        // needs its original scope before a call is copied.
                        self.type_reference_table
                            .retain_operand_lifetime_owner(argument.type_reference, owner)?;
                        continue;
                    }
                    if argument.evidence_projection.is_some()
                        || argument
                            .application
                            .as_ref()
                            .is_some_and(|application| application.member.is_valid())
                    {
                        complete = false;
                        continue;
                    }
                    if let Some(application) = &mut argument.application
                        && !self.retain_static_operand_sources(&mut application.arguments, owner)?
                    {
                        complete = false;
                        continue;
                    }
                    if argument.names_implementation(&self.symbols) {
                        if self.symbols.get(argument.symbol).kind
                            == symbols::SymbolKind::Conformance
                        {
                            let Some(declaration) = self.conformance_by_symbol(argument.symbol)
                            else {
                                complete = false;
                                continue;
                            };
                            let application = argument.application.as_deref();
                            if self.conformance_type_parameters(declaration).len()
                                != application.map_or(0, |application| application.arguments.len())
                                || declaration.lifetime_parameters.len()
                                    != application.map_or(0, |application| {
                                        application.lifetime_arguments.len()
                                    })
                            {
                                complete = false;
                                continue;
                            }
                        }
                        // Capture the complete operand in its source scope before
                        // a copied call or specialization changes the call owner.
                        argument.type_reference =
                            self.static_operand_type_reference_in_scope(argument, owner)?;
                    } else if !argument.symbol.is_valid()
                        && argument.const_literal.is_none()
                        && crate::static_const_argument_spelling(self, argument).is_none()
                    {
                        complete = false;
                    }
                }
                Ok(complete)
            }

            fn retain_application_operand_sources(
                &mut self,
                application: crate::static_applications::StaticMachineApplicationHandle,
                original: &[StaticMachineArgument],
                supplied: &[StaticMachineArgument],
            ) -> Result<(), &'static str> {
                use crate::static_applications::StaticMachineBindingValue;

                let owner = self.static_machine_application(application).lexical_owner;
                let mut arguments = self
                    .static_machine_application_arguments(application)
                    .to_vec();
                if arguments == original {
                    arguments.clone_from_slice(supplied);
                } else {
                    self.retain_static_operand_sources(&mut arguments, owner)?;
                }
                self.expression_table
                    .static_machine_application_arguments_mut(application)
                    .clone_from_slice(&arguments);
                let original = self.static_machine_application_bindings(application);
                let mut bindings = original.to_vec();
                for binding in &mut bindings {
                    match &mut binding.value {
                        StaticMachineBindingValue::Type(reference)
                        | StaticMachineBindingValue::Const(reference) => {
                            if reference.is_valid() {
                                self.type_reference_table
                                    .retain_operand_lifetime_owner(*reference, owner)?;
                            }
                        }
                        StaticMachineBindingValue::Machine(argument)
                        | StaticMachineBindingValue::Conformance(argument) => {
                            self.retain_static_operand_sources(
                                std::slice::from_mut(argument),
                                owner,
                            )?;
                        }
                        _ => {}
                    }
                }
                if bindings != self.static_machine_application_bindings(application) {
                    self.expression_table
                        .set_static_machine_application_bindings(application, &bindings);
                }
                Ok(())
            }
        }

        fn direct_declaration(program: &TypedTrees, target: SymbolHandle) -> Option<SymbolHandle> {
            program
                .machine_by_symbol(target)
                .or_else(|| program.machine_holding_state(target))
                .map(|machine| machine.symbol)
        }

        fn implementation_namespace(program: &TypedTrees, symbol: SymbolHandle) -> bool {
            symbol.is_valid()
                && (program.conformance_by_symbol(symbol).is_some()
                    || program.symbols.get(symbol).kind
                        == symbols::SymbolKind::ConformanceParameter)
        }

        fn has_region_telescope(program: &TypedTrees, target: SymbolHandle) -> bool {
            program
                .machine_by_symbol(target)
                .or_else(|| program.machine_holding_state(target))
                .is_some_and(|machine| !machine.lifetime_parameters.is_empty())
        }
    }

    #[cfg(test)]
    use super::{ExpressionNode, TypeReferenceNode, conformance_occurrence_tests};

    pub fn collect_authored_program_type_occurrences(program: &TypedTrees) -> Vec<TypeOccurrence> {
        let mut source = AuthoredTypeOccurrences(TypeOccurrences::default());
        visit_program(program, &mut source);
        source.0.types
    }

    pub fn authored_type_occurrences(
        program: &TypedTrees,
        owners: &[SymbolHandle],
    ) -> Vec<TypeOccurrence> {
        let mut requested = HashSet::default();
        let mut ancestors = HashSet::default();
        let mut descriptions = Vec::new();
        let mut description_owners = HashSet::default();
        for &owner in owners {
            if !owner.is_valid() || requested.contains(&owner) {
                continue;
            }
            let mut scope = Vec::new();
            let mut current = owner;
            while current.is_valid() && !scope.contains(&current) {
                scope.push(current);
                current = program.symbols.get(current).parent;
            }
            let mut selected = None;
            let mut ambiguous = false;
            for &ancestor in &scope {
                match lexical_description(program, ancestor) {
                    Ok(Some(description)) => {
                        selected = Some((ancestor, description));
                        break;
                    }
                    Ok(None) => {}
                    Err(()) => {
                        ambiguous = true;
                        break;
                    }
                }
            }
            if ambiguous {
                continue;
            }
            requested.insert(owner);
            ancestors.extend(scope);
            if let Some((symbol, description)) = selected
                && description_owners.insert(symbol)
            {
                descriptions.push(description);
            }
        }
        let mut source = AuthoredTypeOccurrences(TypeOccurrences {
            selection: DeclarationSelection::from_symbols(ancestors),
            ..TypeOccurrences::default()
        });
        visit_declarations(program, &mut source);
        for description in descriptions {
            match description {
                SourceDescription::Machine(machine) => visit_machine(program, &mut source, machine),
                SourceDescription::Data(definition)
                    if program
                        .data_definition_by_symbol(definition.symbol)
                        .is_none()
                        && source.declaration(definition.symbol) =>
                {
                    visit_data(program, &mut source, definition);
                }
                SourceDescription::Trait(definition)
                    if program
                        .trait_definition_by_symbol(definition.symbol)
                        .is_none()
                        && source.declaration(definition.symbol) =>
                {
                    visit_trait(program, &mut source, definition);
                }
                SourceDescription::Const(definition)
                    if program
                        .const_declaration_by_symbol(definition.symbol)
                        .is_none()
                        && source.declaration(definition.symbol) =>
                {
                    visit_const(program, &mut source, definition);
                }
                SourceDescription::Alias(definition)
                    if program
                        .type_aliases()
                        .iter()
                        .all(|alias| alias.symbol != definition.symbol)
                        && source.declaration(definition.symbol) =>
                {
                    visit_alias(program, &mut source, definition);
                }
                _ => {}
            }
        }
        source
            .0
            .types
            .into_iter()
            .filter(|occurrence| requested.contains(&occurrence.lexical_owner))
            .collect()
    }

    enum SourceDescription<'program> {
        Machine(&'program crate::machine::Machine),
        Data(&'program crate::data::DataDefinition),
        Trait(&'program crate::trait_definition::TraitDefinition),
        Const(&'program crate::constant::ConstDeclaration),
        Alias(&'program crate::type_aliases::TypeAliasDefinition),
    }

    fn lexical_description(
        program: &TypedTrees,
        symbol: SymbolHandle,
    ) -> Result<Option<SourceDescription<'_>>, ()> {
        macro_rules! header {
            ($current:expr, $table:ident, $variant:ident) => {
                consistent_header(
                    $current,
                    program
                        .tables
                        .$table
                        .iter()
                        .map(|(_, declaration)| declaration)
                        .filter(|declaration| declaration.symbol == symbol),
                    PartialEq::eq,
                )
                .map(|description| description.map(SourceDescription::$variant))
            };
        }
        match program.symbols.get(symbol).kind {
            SymbolKind::Machine => header!(
                program.machine_declaration_for_lexical_scope(symbol),
                machines,
                Machine
            ),
            SymbolKind::Data => header!(
                program.data_definition_by_symbol(symbol),
                data_definitions,
                Data
            ),
            SymbolKind::Trait => header!(program.trait_definition_by_symbol(symbol), traits, Trait),
            SymbolKind::TypeAlias => header!(
                program
                    .type_aliases()
                    .iter()
                    .find(|alias| alias.symbol == symbol),
                type_aliases,
                Alias
            ),
            SymbolKind::Const => consistent_header(
                program.const_declaration_by_symbol(symbol),
                program
                    .tables
                    .const_declarations
                    .iter()
                    .map(|(_, declaration)| declaration)
                    .filter(|declaration| declaration.symbol == symbol),
                |left, right| {
                    left.symbol == right.symbol
                        && left.is_public == right.is_public
                        && left.declared_type == right.declared_type
                        && left.initializer_source_span == right.initializer_source_span
                        && left.authored_initializer == right.authored_initializer
                },
            )
            .map(|description| description.map(SourceDescription::Const)),
            _ => Ok(None),
        }
    }

    fn consistent_header<'program, Declaration: 'program>(
        current: Option<&'program Declaration>,
        mut copies: impl Iterator<Item = &'program Declaration>,
        agrees: impl Fn(&Declaration, &Declaration) -> bool,
    ) -> Result<Option<&'program Declaration>, ()> {
        if current.is_some() {
            return Ok(current);
        }
        let Some(description) = copies.next() else {
            return Ok(None);
        };
        if copies.all(|copy| agrees(copy, description)) {
            Ok(Some(description))
        } else {
            Err(())
        }
    }

    struct AuthoredTypeOccurrences(TypeOccurrences);

    impl Extend<SymbolHandle> for AuthoredTypeOccurrences {
        fn extend<Symbols: IntoIterator<Item = SymbolHandle>>(&mut self, _: Symbols) {}
    }

    impl TypedReferences for AuthoredTypeOccurrences {
        fn derived_declaration_references(&self) -> bool {
            false
        }
        fn declaration(&mut self, symbol: SymbolHandle) -> bool {
            self.0.declaration(symbol)
        }
        fn enter_lexical_scope(&mut self, owner: SymbolHandle) {
            self.0.enter_lexical_scope(owner);
        }
        fn leave_lexical_scope(&mut self) {
            self.0.leave_lexical_scope();
        }
        fn call_arguments(&mut self, program: &TypedTrees, call: StaticArgumentCall<'_>) -> bool {
            self.0.call_arguments(program, call)
        }
        fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
            self.0.type_reference(reference)
        }
        fn expression(&mut self, expression: ExpressionHandle) -> bool {
            self.0.expression(expression)
        }
        fn mathematical_type(
            &mut self,
            reference: crate::mathematical::MathematicalTypeHandle,
        ) -> bool {
            self.0.mathematical_type(reference)
        }
    }

    #[cfg(test)]
    mod authored_type_occurrence_tests {
        use super::*;

        fn contains(
            program: &TypedTrees,
            owner: SymbolHandle,
            reference: TypeReferenceHandle,
        ) -> bool {
            authored_type_occurrences(program, &[owner]).contains(&TypeOccurrence {
                lexical_owner: owner,
                reference,
            })
        }

        #[test]
        fn authored_call_actuals_and_cast_targets_exclude_computed_expression_types() {
            use crate::expression::{
                BinaryOperator, OpenIndexOperationSelection, StaticMachineArgument,
                TableBinaryExpression, TableCastExpression,
            };
            let mut symbols = symbols::SymbolTableBuilder::default();
            let owner = symbols.insert_root(
                symbols::SymbolKind::Machine,
                symbols::SymbolNameRef::Static("owner"),
            );
            let mut program = TypedTrees {
                symbols: symbols.finish(),
                ..Default::default()
            };
            let mut ty = |name: &str| {
                program
                    .type_reference_table
                    .insert(TypeReferenceNode::Named {
                        symbol: SymbolHandle::invalid(),
                        name: crate::name::Identifier::from(name),
                    })
            };
            let authored_call = ty("AuthoredCallArgument");
            let authored_cast = ty("AuthoredCastTarget");
            let computed = ty("ComputedTypeOnly");
            let (call, _) =
                super::conformance_occurrence_tests::occurrence_call(&mut program, owner);
            let application = program.expression_table.retain_static_machine_application(
                SymbolHandle::invalid(),
                Default::default(),
                &[StaticMachineArgument {
                    type_reference: authored_call,
                    ..Default::default()
                }],
            );
            program
                .expression_table
                .static_machine_application_mut(application)
                .lexical_owner = owner;
            if let ExpressionNode::Call(call) = program.expression_table.expression_mut(call) {
                call.machine_arguments = Box::new([]);
                call.authored_machine_application = application;
            }
            let leaf = program
                .expression_table
                .insert(ExpressionNode::Boolean(true));
            let binary =
                program
                    .expression_table
                    .insert(ExpressionNode::Binary(TableBinaryExpression {
                        policy: numerics::arithmetic::ArithmeticDomain::Exact,
                        left: leaf,
                        right: leaf,
                        operator: BinaryOperator::Add,
                    }));
            program
                .expression_table
                .index_expression_authority_mut(binary)
                .operation = Some(OpenIndexOperationSelection {
                expression: binary,
                spelling: language_core::OperatorSpelling::Add,
                operator: SymbolHandle::invalid(),
                operand_type: computed,
                provider: SymbolHandle::invalid(),
                application: Some(StaticMachineArgument {
                    type_reference: computed,
                    ..Default::default()
                }),
                algebra_trait: SymbolHandle::invalid(),
                algebra_requirement: String::new(),
                algebra_alias: None,
                commutativity_licensed: false,
                associativity_licensed: false,
                distributes_over: Vec::new(),
                left_identity_literal: String::new(),
                right_identity_literal: String::new(),
            });
            let cast = program
                .expression_table
                .insert(ExpressionNode::Cast(TableCastExpression {
                    value: leaf,
                    target_type: authored_cast,
                    result_type: computed,
                    target_label: Default::default(),
                    domain: numerics::arithmetic::ArithmeticDomain::Exact,
                    semantic_domain: Default::default(),
                    semantic_domain_arguments: Default::default(),
                    semantic_domain_symbol: SymbolHandle::invalid(),
                    semantic_domain_id: language_semantics::SemanticDomainId::NULL,
                    form: language_core::cast_form::CastForm::Value,
                }));
            let elements = program
                .expression_table
                .insert_expression_handles([call, binary, cast]);
            let expression = program
                .expression_table
                .insert(ExpressionNode::ArrayLiteral(elements));
            let mut machine = crate::machine::Machine {
                symbol: owner,
                ..Default::default()
            };
            program.push_machine_owned_data(
                &mut machine,
                crate::machine::OwnedData {
                    initial_value: expression,
                    ..Default::default()
                },
            );
            program.push_machine(machine);
            for projected in [false, true] {
                if projected {
                    program.roots.machines = Default::default();
                }
                assert!(contains(&program, owner, authored_call));
                assert!(contains(&program, owner, authored_cast));
                assert!(!contains(&program, owner, computed));
                let current = collect_authored_program_type_occurrences(&program);
                assert_eq!(
                    current.contains(&TypeOccurrence {
                        lexical_owner: owner,
                        reference: authored_call
                    }),
                    !projected,
                );
                assert_eq!(
                    current.contains(&TypeOccurrence {
                        lexical_owner: owner,
                        reference: authored_cast
                    }),
                    !projected,
                );
                assert!(
                    !current
                        .iter()
                        .any(|occurrence| occurrence.reference == computed)
                );
            }
        }

        #[test]
        fn source_occurrences_survive_projection_without_derived_self_attestation() {
            let mut symbols = symbols::SymbolTableBuilder::default();
            let owner = symbols.insert_root(
                symbols::SymbolKind::Machine,
                symbols::SymbolNameRef::Static("source"),
            );
            let signature = symbols
                .insert_children(
                    owner,
                    [(
                        symbols::SymbolKind::Machine,
                        symbols::SymbolNameRef::Static("callback"),
                    )],
                )
                .handle_at(0);
            let constant = symbols.insert_root(
                symbols::SymbolKind::Const,
                symbols::SymbolNameRef::Static("constant"),
            );
            let missing = symbols.insert_root(
                symbols::SymbolKind::Machine,
                symbols::SymbolNameRef::Static("missing"),
            );
            let mut program = TypedTrees {
                symbols: symbols.finish(),
                ..Default::default()
            };
            let reference = program.type_reference_table.insert(TypeReferenceNode::Unit);
            let forged = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: SymbolHandle::invalid(),
                    name: crate::name::Identifier::from("sidecar_only"),
                });
            let mut machine = crate::machine::Machine {
                symbol: owner,
                ..Default::default()
            };
            program.push_machine_owned_data(
                &mut machine,
                crate::machine::OwnedData {
                    type_reference: reference,
                    ..Default::default()
                },
            );
            machine.type_parameters =
                program
                    .data_type_parameters
                    .insert_many([crate::data::TypeParameter {
                        kind: crate::data::TypeParameterKind::Machine {
                            contract: crate::data::MachineParameterContract::Structural(
                                crate::signature::StateSignature {
                                    symbol: signature,
                                    return_type: reference,
                                    ..Default::default()
                                },
                            ),
                        },
                        ..Default::default()
                    }]);
            program.push_machine(machine.clone());
            program.push_const_declaration(crate::constant::ConstDeclaration {
                symbol: constant,
                declared_type: reference,
                ..Default::default()
            });
            program
                .data_implementation_applications
                .insert(crate::DataImplementationApplication {
                    lexical_owner: owner,
                    source_application: forged,
                    ..Default::default()
                });
            assert!(contains(&program, owner, reference));
            assert!(contains(&program, signature, reference));
            assert!(contains(&program, constant, reference));
            let batch = authored_type_occurrences(&program, &[owner, signature, owner]);
            assert_eq!(batch.len(), 2);
            assert!(batch.contains(&TypeOccurrence {
                lexical_owner: owner,
                reference
            }));
            assert!(batch.contains(&TypeOccurrence {
                lexical_owner: signature,
                reference
            }));
            assert!(!contains(&program, owner, forged));
            let current = collect_authored_program_type_occurrences(&program);
            for lexical_owner in [owner, signature, constant] {
                assert!(current.contains(&TypeOccurrence {
                    lexical_owner,
                    reference
                }));
            }
            assert!(
                !current
                    .iter()
                    .any(|occurrence| occurrence.reference == forged)
            );
            assert!(!contains(&program, missing, reference));
            assert!(!contains(
                &program,
                owner,
                TypeReferenceHandle::from_parts(
                    reference.arena_index(),
                    reference.generation() + 1
                ),
            ));

            program.roots.machines = Default::default();
            assert!(program.machine_by_symbol(owner).is_none());
            assert!(
                collect_authored_program_type_occurrences(&program)
                    == vec![TypeOccurrence {
                        lexical_owner: constant,
                        reference,
                    }]
            );
            assert!(contains(&program, owner, reference));
            assert!(contains(&program, signature, reference));
            assert!(!contains(&program, owner, forged));
            assert!(program.machine_by_symbol(owner).is_none());
            let duplicate = program.tables.machines.insert(machine);
            assert!(contains(&program, owner, reference));
            program
                .tables
                .machines
                .get_mut(duplicate)
                .lifetime_parameters
                .push(crate::name::Identifier::from("conflicting"));
            assert!(!contains(&program, owner, reference));
            assert!(!contains(&program, signature, reference));
            assert!(
                authored_type_occurrences(&program, &[owner, signature, constant])
                    == vec![TypeOccurrence {
                        lexical_owner: constant,
                        reference
                    }]
            );
            let declaration = program
                .const_declaration_by_symbol(constant)
                .unwrap()
                .clone();
            program.roots.const_declarations = Default::default();
            assert!(contains(&program, constant, reference));
            let mut copy = declaration;
            copy.materialized_initializer = program
                .expression_table
                .insert(ExpressionNode::Boolean(true));
            let duplicate = program.tables.const_declarations.insert(copy);
            assert!(contains(&program, constant, reference));
            program
                .tables
                .const_declarations
                .get_mut(duplicate)
                .declared_type = forged;
            assert!(!contains(&program, constant, reference));
            assert!(!contains(&program, constant, forged));
        }
    }
}
pub use source_occurrences::{
    authored_type_occurrences, collect_authored_program_type_occurrences,
};

type HashSet<T> = std::collections::HashSet<T, symbols::BuildSymbolHasher>;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StaticArgumentRole {
    ValueDependency,
    DeclarationDescription,
}

#[derive(Clone, Copy)]
pub enum StaticArgumentCall<'call> {
    Expression(ExpressionHandle, &'call TableCallExpression),
    Statement(&'call crate::statement::TableCall),
}

impl<'call> StaticArgumentCall<'call> {
    pub fn applications(&self) -> impl Iterator<Item = StaticMachineApplicationHandle> + use<> {
        let invocation = self.application();
        let selection = self.selection_application();
        [invocation, selection]
            .into_iter()
            .enumerate()
            .filter_map(move |(index, application)| {
                (application.is_valid() && (index == 0 || application != invocation))
                    .then_some(application)
            })
    }

    pub fn selection_application(&self) -> StaticMachineApplicationHandle {
        match self {
            Self::Expression(_, call) => call.static_requirement_dispatch.as_ref(),
            Self::Statement(call) => call.static_requirement_dispatch.as_ref(),
        }
        .map(|dispatch| dispatch.selection_application)
        .unwrap_or_default()
    }

    pub fn application(&self) -> StaticMachineApplicationHandle {
        match self {
            Self::Expression(_, call) => call.authored_machine_application,
            Self::Statement(call) => call.authored_machine_application,
        }
    }

    pub fn registration_operation(&self, program: &TypedTrees) -> SymbolHandle {
        let application = self.application();
        if application.is_valid() {
            return program
                .static_machine_application(application)
                .registration_operation;
        }
        match self {
            Self::Expression(_, call) => call.target_symbol,
            Self::Statement(call) => call.target_symbol,
        }
    }

    pub fn lexical_owner(&self, program: &TypedTrees) -> SymbolHandle {
        let application = self.application();
        if application.is_valid() {
            program
                .static_machine_application(application)
                .lexical_owner
        } else {
            SymbolHandle::invalid()
        }
    }

    pub fn arguments(&self, program: &'call TypedTrees) -> &'call [StaticMachineArgument] {
        let application = self.application();
        if application.is_valid() {
            return program.static_machine_application_arguments(application);
        }
        match self {
            Self::Expression(_, call) => &call.machine_arguments,
            Self::Statement(call) => &call.machine_arguments,
        }
    }
}

pub type StaticArgumentClassifier = fn(&TypedTrees, StaticArgumentCall<'_>) -> StaticArgumentRole;

pub trait TypedReferences: Extend<SymbolHandle> {
    fn callable_use(&mut self, _: SymbolHandle) {}
    fn callable_signature(&mut self, _: &crate::signature::StateSignature) {}
    fn enter_lexical_scope(&mut self, _: SymbolHandle) {}
    fn leave_lexical_scope(&mut self) {}
    fn authored_selection(&mut self, _: crate::AuthoredDeclarationSelectionOccurrenceId) {}
    fn static_application(&mut self, _: StaticMachineApplicationHandle) {}
    fn declaration_description_dependency(&mut self, _: SymbolHandle) {}
    fn layout_realization_dependency(&mut self, symbol: SymbolHandle) {
        self.extend([symbol]);
    }
    fn derived_declaration_references(&self) -> bool {
        true
    }
    fn declaration(&mut self, _: SymbolHandle) -> bool {
        true
    }
    fn expression(&mut self, _: ExpressionHandle) -> bool {
        true
    }
    fn statement_handle(&mut self, _: StatementHandle) {}
    fn statement(&mut self, _: &StatementNode) -> bool {
        true
    }
    fn call_arguments(&mut self, _: &TypedTrees, _: StaticArgumentCall<'_>) -> bool {
        true
    }
    fn type_reference(&mut self, _: TypeReferenceHandle) -> bool {
        true
    }
    fn type_alias_application(
        &mut self,
        _: &TypedTrees,
        _: &crate::type_aliases::TypeAliasApplication,
    ) -> bool {
        true
    }
    fn type_expression(&mut self, _: ExpressionHandle) -> bool {
        true
    }
    fn proof_fact(&mut self, _: arena::Handle<ProofFact>) -> bool {
        true
    }
    fn mathematical_type(&mut self, _: MathematicalTypeHandle) -> bool {
        true
    }
}

pub struct ProgramReferences {
    pub declarations: Vec<SymbolHandle>,
    pub symbols: HashSet<SymbolHandle>,
    pub callable_uses: HashSet<SymbolHandle>,
    pub descriptor_symbols: HashSet<SymbolHandle>,
    pub expressions: HashSet<ExpressionHandle>,
    pub statements: HashSet<StatementHandle>,
    pub type_references: Vec<TypeReferenceHandle>,
    pub proof_facts: Vec<arena::Handle<ProofFact>>,
    pub authored_selection_occurrences: HashSet<crate::AuthoredDeclarationSelectionOccurrenceId>,
}
#[derive(Default)]
struct ReferenceCollector {
    selection: DeclarationSelection,
    declarations: Vec<SymbolHandle>,
    symbols: HashSet<SymbolHandle>,
    callable_uses: HashSet<SymbolHandle>,
    descriptor_symbols: HashSet<SymbolHandle>,
    classify_static_arguments: Option<StaticArgumentClassifier>,
    statements: HashSet<StatementHandle>,
    expressions: HashSet<ExpressionHandle>,
    track_new_expressions: bool,
    new_expressions: HashSet<ExpressionHandle>,
    types: HashSet<TypeReferenceHandle>,
    describing_alias_arguments: bool,
    alias_argument_types: HashSet<TypeReferenceHandle>,
    alias_argument_expressions: HashSet<ExpressionHandle>,
    facts: HashSet<arena::Handle<ProofFact>>,
    mathematical_types: HashSet<MathematicalTypeHandle>,
    authored_selection_occurrences: HashSet<crate::AuthoredDeclarationSelectionOccurrenceId>,
}
impl Extend<SymbolHandle> for ReferenceCollector {
    fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: T) {
        self.symbols.extend(symbols);
    }
}
impl TypedReferences for ReferenceCollector {
    fn type_alias_application(
        &mut self,
        program: &TypedTrees,
        application: &crate::type_aliases::TypeAliasApplication,
    ) -> bool {
        if self.describing_alias_arguments {
            return true;
        }
        let mut description = ReferenceCollector {
            describing_alias_arguments: true,
            ..ReferenceCollector::default()
        };
        description.declaration_description_dependency(application.supplied_operand_owner);
        visit_type_reference(program, &mut description, application.application);
        let description = finish_references(description);
        self.descriptor_symbols.extend(description.symbols);
        self.descriptor_symbols
            .extend(description.descriptor_symbols);
        self.authored_selection_occurrences
            .extend(description.authored_selection_occurrences);
        self.alias_argument_types
            .extend(description.type_references);
        self.alias_argument_expressions
            .extend(description.expressions);
        false
    }
    fn statement_handle(&mut self, handle: StatementHandle) {
        self.statements.insert(handle);
    }
    fn callable_use(&mut self, symbol: SymbolHandle) {
        if symbol.is_valid() {
            self.callable_uses.insert(symbol);
        }
    }
    fn declaration_description_dependency(&mut self, symbol: SymbolHandle) {
        self.descriptor_symbols.insert(symbol);
    }
    fn call_arguments(&mut self, program: &TypedTrees, call: StaticArgumentCall<'_>) -> bool {
        if !self.classify_static_arguments.is_some_and(|classify| {
            classify(program, call) == StaticArgumentRole::DeclarationDescription
        }) {
            return true;
        }
        let mut description = ReferenceCollector::default();
        for argument in call.arguments(program) {
            visit_static_argument(program, &mut description, argument);
        }
        visit_static_machine_bindings(program, &mut description, call.application());
        self.descriptor_symbols.extend(description.symbols);
        self.authored_selection_occurrences
            .extend(description.authored_selection_occurrences);
        false
    }
    fn authored_selection(&mut self, occurrence: crate::AuthoredDeclarationSelectionOccurrenceId) {
        self.authored_selection_occurrences.insert(occurrence);
    }
    fn declaration(&mut self, symbol: SymbolHandle) -> bool {
        if !self.selection.contains(symbol) {
            return false;
        }
        self.declarations.push(symbol);
        true
    }
    fn expression(&mut self, handle: ExpressionHandle) -> bool {
        let inserted = self.expressions.insert(handle);
        if inserted && self.track_new_expressions {
            self.new_expressions.insert(handle);
        }
        inserted
    }
    fn type_reference(&mut self, handle: TypeReferenceHandle) -> bool {
        self.types.insert(handle)
    }
    fn proof_fact(&mut self, handle: arena::Handle<ProofFact>) -> bool {
        self.facts.insert(handle)
    }
    fn mathematical_type(&mut self, handle: MathematicalTypeHandle) -> bool {
        self.mathematical_types.insert(handle)
    }
}
pub fn collect_expression_symbol_dependencies(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Vec<SymbolHandle> {
    let mut references = ReferenceCollector::default();
    visit_expression(program, &mut references, expression);
    let mut symbols = references.symbols.into_iter().collect::<Vec<_>>();
    symbols.sort_unstable_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
    symbols
}

pub fn collect_type_symbol_dependencies(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Vec<SymbolHandle> {
    let mut references = ReferenceCollector::default();
    visit_type_reference(program, &mut references, reference);
    let mut symbols = references.symbols.into_iter().collect::<Vec<_>>();
    symbols.sort_unstable_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
    symbols
}

pub fn collect_program_references(program: &TypedTrees) -> ProgramReferences {
    collect_selected_program_references(program, &DeclarationSelection::default())
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct TypeOccurrence {
    pub lexical_owner: SymbolHandle,
    pub reference: TypeReferenceHandle,
}

pub fn collect_program_type_occurrences(program: &TypedTrees) -> Vec<TypeOccurrence> {
    collect_selected_program_type_occurrences(program, &DeclarationSelection::default())
}

pub fn collect_selected_program_type_occurrences(
    program: &TypedTrees,
    selection: &DeclarationSelection,
) -> Vec<TypeOccurrence> {
    let mut occurrences = TypeOccurrences {
        selection: selection.clone(),
        ..TypeOccurrences::default()
    };
    visit_program(program, &mut occurrences);
    occurrences.types
}

pub struct ProgramOperandOwners {
    types: Vec<Vec<(SymbolHandle, TypeReferenceHandle)>>,
    expressions: Vec<Vec<(SymbolHandle, ExpressionHandle)>>,
}

impl ProgramOperandOwners {
    pub fn type_owners(
        &self,
        reference: TypeReferenceHandle,
    ) -> impl Iterator<Item = SymbolHandle> + '_ {
        self.types
            .get(reference.arena_index() as usize)
            .into_iter()
            .flatten()
            .filter(move |(_, retained)| *retained == reference)
            .map(|(owner, _)| *owner)
    }

    pub fn expression_owners(
        &self,
        expression: ExpressionHandle,
    ) -> impl Iterator<Item = SymbolHandle> + '_ {
        self.expressions
            .get(expression.arena_index() as usize)
            .into_iter()
            .flatten()
            .filter(move |(_, retained)| *retained == expression)
            .map(|(owner, _)| *owner)
    }
}

pub fn collect_program_operand_owners(program: &TypedTrees) -> ProgramOperandOwners {
    let mut occurrences = TypeOccurrences::default();
    visit_program(program, &mut occurrences);
    ProgramOperandOwners {
        types: occurrences.seen_types,
        expressions: occurrences.seen_expressions,
    }
}

pub fn collect_program_conformance_occurrences(
    program: &TypedTrees,
) -> Vec<StaticMachineApplicationHandle> {
    let mut occurrences = TypeOccurrences::default();
    visit_program(program, &mut occurrences);
    occurrences.conformance_applications(program)
}

pub(crate) struct ProgramApplicationOccurrences {
    pub types: Vec<TypeOccurrence>,
    pub requirement_applications: Vec<TypeOccurrence>,
    pub conformance_applications: Vec<StaticMachineApplicationHandle>,
}

pub(crate) fn collect_program_application_occurrences(
    program: &TypedTrees,
) -> ProgramApplicationOccurrences {
    let mut occurrences = TypeOccurrences::default();
    visit_program(program, &mut occurrences);
    let conformance_applications = occurrences.conformance_applications(program);
    ProgramApplicationOccurrences {
        types: occurrences.types,
        requirement_applications: occurrences.requirement_applications,
        conformance_applications,
    }
}

#[derive(Default)]
struct TypeOccurrences {
    selection: DeclarationSelection,
    owner: SymbolHandle,
    outer_owners: Vec<SymbolHandle>,
    types: Vec<TypeOccurrence>,
    requirement_applications: Vec<TypeOccurrence>,
    applications: HashSet<StaticMachineApplicationHandle>,
    seen_types: Vec<Vec<(SymbolHandle, TypeReferenceHandle)>>,
    seen_expressions: Vec<Vec<(SymbolHandle, ExpressionHandle)>>,
    seen_mathematical_types: Vec<Vec<(SymbolHandle, MathematicalTypeHandle)>>,
}

fn retain_scoped_occurrence<Element>(
    seen: &mut Vec<Vec<(SymbolHandle, arena::Handle<Element>)>>,
    owner: SymbolHandle,
    reference: arena::Handle<Element>,
) -> bool {
    let slot = reference.arena_index() as usize;
    if seen.len() <= slot {
        seen.resize_with(slot + 1, Vec::new);
    }
    let occurrence = (owner, reference);
    if seen[slot].contains(&occurrence) {
        return false;
    }
    seen[slot].push(occurrence);
    true
}

impl TypeOccurrences {
    fn conformance_applications(
        &self,
        program: &TypedTrees,
    ) -> Vec<StaticMachineApplicationHandle> {
        program
            .expression_table
            .static_machine_applications()
            .filter_map(|(handle, _)| self.applications.contains(&handle).then_some(handle))
            .collect()
    }
}

impl Extend<SymbolHandle> for TypeOccurrences {
    fn extend<Symbols: IntoIterator<Item = SymbolHandle>>(&mut self, _: Symbols) {}
}

impl TypedReferences for TypeOccurrences {
    fn call_arguments(&mut self, program: &TypedTrees, call: StaticArgumentCall<'_>) -> bool {
        if let StaticArgumentCall::Expression(_, call) = call {
            let readiness = match program
                .expression_table
                .build_selection_request(call.build_selection)
            {
                Some(BuildSelectionRequest::ProviderReadiness(request)) => request.requirement,
                _ => Default::default(),
            };
            for reference in [call.requirement_application, readiness] {
                if !reference.is_valid() {
                    continue;
                }
                let occurrence = TypeOccurrence {
                    lexical_owner: self.owner,
                    reference,
                };
                if !self.requirement_applications.contains(&occurrence) {
                    self.requirement_applications.push(occurrence);
                }
            }
        }
        for handle in [call.application(), call.selection_application()] {
            if handle.is_valid() {
                let source = program.static_machine_application(handle);
                if source.lexical_owner == self.owner && source.conformance_selection.is_some() {
                    self.applications.insert(handle);
                }
            }
        }
        true
    }

    fn declaration(&mut self, symbol: SymbolHandle) -> bool {
        if !self.selection.contains(symbol) {
            return false;
        }
        self.owner = symbol;
        true
    }

    fn enter_lexical_scope(&mut self, owner: SymbolHandle) {
        self.outer_owners.push(self.owner);
        if owner.is_valid() {
            self.owner = owner;
        }
    }

    fn leave_lexical_scope(&mut self) {
        if let Some(owner) = self.outer_owners.pop() {
            self.owner = owner;
        }
    }

    fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
        let occurrence = TypeOccurrence {
            lexical_owner: self.owner,
            reference,
        };
        if !retain_scoped_occurrence(&mut self.seen_types, self.owner, reference) {
            return false;
        }
        self.types.push(occurrence);
        true
    }

    fn expression(&mut self, expression: ExpressionHandle) -> bool {
        retain_scoped_occurrence(&mut self.seen_expressions, self.owner, expression)
    }

    fn mathematical_type(&mut self, reference: MathematicalTypeHandle) -> bool {
        retain_scoped_occurrence(&mut self.seen_mathematical_types, self.owner, reference)
    }
}

pub fn collect_program_declarations(program: &TypedTrees) -> Vec<SymbolHandle> {
    struct Declarations(Vec<SymbolHandle>);
    impl Extend<SymbolHandle> for Declarations {
        fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {}
    }
    impl TypedReferences for Declarations {
        fn declaration(&mut self, symbol: SymbolHandle) -> bool {
            self.0.push(symbol);
            false
        }
    }
    let mut declarations = Declarations(Vec::new());
    visit_program(program, &mut declarations);
    declarations.0
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct DeclarationSelection {
    excluded: bool,
    symbols: Vec<SymbolHandle>,
}

impl Default for DeclarationSelection {
    fn default() -> Self {
        Self::all()
    }
}

impl DeclarationSelection {
    pub fn all() -> Self {
        Self {
            excluded: true,
            symbols: Vec::new(),
        }
    }

    pub fn contains(&self, symbol: SymbolHandle) -> bool {
        self.symbols
            .binary_search_by_key(&(symbol.arena_index(), symbol.generation()), |symbol| {
                (symbol.arena_index(), symbol.generation())
            })
            .is_ok()
            != self.excluded
    }

    pub fn is_all(&self) -> bool {
        self.excluded && self.symbols.is_empty()
    }

    pub fn from_program(program: &TypedTrees) -> Self {
        Self::from_symbols(collect_program_declarations(program))
    }

    pub fn from_symbols(symbols: impl IntoIterator<Item = SymbolHandle>) -> Self {
        let mut symbols = symbols.into_iter().collect::<Vec<_>>();
        symbols.sort_unstable_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
        symbols.dedup();
        Self {
            excluded: false,
            symbols,
        }
    }

    pub fn intersect(&self, other: &Self) -> Self {
        if other.excluded {
            if self.excluded {
                let mut result = self.clone();
                result.symbols.extend(other.symbols.iter().copied());
                result
                    .symbols
                    .sort_unstable_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
                result.symbols.dedup();
                return result;
            }
            return other.intersect(self);
        }
        let mut result = other.clone();
        result.symbols.retain(|symbol| self.contains(*symbol));
        result
    }

    pub fn excluding(&self, other: &Self) -> Self {
        if other.excluded {
            let mut complement = other.clone();
            complement.excluded = false;
            return self.intersect(&complement);
        }
        if self.excluded {
            let mut result = self.clone();
            result.symbols.extend(other.symbols.iter().copied());
            result
                .symbols
                .sort_unstable_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
            result.symbols.dedup();
            result
        } else {
            let mut result = self.clone();
            result.symbols.retain(|symbol| !other.contains(*symbol));
            result
        }
    }

    pub fn include(&mut self, symbol: SymbolHandle) {
        let position = self
            .symbols
            .binary_search_by_key(&(symbol.arena_index(), symbol.generation()), |symbol| {
                (symbol.arena_index(), symbol.generation())
            });
        match (self.excluded, position) {
            (false, Err(position)) => self.symbols.insert(position, symbol),
            (true, Ok(position)) => {
                self.symbols.remove(position);
            }
            _ => {}
        }
    }
}

pub fn collect_selected_program_references(
    program: &TypedTrees,
    selection: &DeclarationSelection,
) -> ProgramReferences {
    collect_program_references_with_roles(program, selection, None)
}

pub fn collect_declaration_symbol_dependencies(
    program: &TypedTrees,
) -> Vec<(SymbolHandle, Vec<SymbolHandle>)> {
    #[derive(Default)]
    struct Declarations {
        rows: Vec<(SymbolHandle, Vec<SymbolHandle>)>,
        owner: SymbolHandle,
        current: ReferenceCollector,
    }

    impl Declarations {
        fn finish(&mut self) {
            let references = std::mem::take(&mut self.current);
            if !self.owner.is_valid() {
                return;
            }
            let mut dependencies: Vec<_> = references
                .symbols
                .into_iter()
                .chain(references.descriptor_symbols)
                .collect();
            dependencies.sort_unstable_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
            dependencies.dedup();
            self.rows.push((self.owner, dependencies));
        }
    }

    impl Extend<SymbolHandle> for Declarations {
        fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: T) {
            self.current.extend(symbols);
        }
    }

    impl TypedReferences for Declarations {
        fn declaration(&mut self, symbol: SymbolHandle) -> bool {
            self.finish();
            self.owner = symbol;
            true
        }

        fn declaration_description_dependency(&mut self, symbol: SymbolHandle) {
            self.current.declaration_description_dependency(symbol);
        }

        fn expression(&mut self, expression: ExpressionHandle) -> bool {
            self.current.expression(expression)
        }

        fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
            self.current.type_reference(reference)
        }

        fn proof_fact(&mut self, fact: arena::Handle<ProofFact>) -> bool {
            self.current.proof_fact(fact)
        }

        fn mathematical_type(&mut self, ty: MathematicalTypeHandle) -> bool {
            self.current.mathematical_type(ty)
        }
    }

    let mut declarations = Declarations::default();
    visit_program(program, &mut declarations);
    declarations.finish();
    declarations.rows
}

pub fn collect_program_references_with_roles(
    program: &TypedTrees,
    selection: &DeclarationSelection,
    classify_static_arguments: Option<StaticArgumentClassifier>,
) -> ProgramReferences {
    let mut roots = ReferenceCollector {
        selection: selection.clone(),
        classify_static_arguments,
        ..ReferenceCollector::default()
    };
    visit_program(program, &mut roots);
    finish_references(roots)
}

pub fn collect_signature_references(
    program: &TypedTrees,
    signature: &crate::signature::StateSignature,
) -> ProgramReferences {
    let mut roots = ReferenceCollector::default();
    visit_signature(program, &mut roots, signature);
    finish_references(roots)
}

fn finish_references(roots: ReferenceCollector) -> ProgramReferences {
    let mut type_references = roots
        .types
        .into_iter()
        .chain(roots.alias_argument_types)
        .collect::<Vec<_>>();
    type_references.sort_unstable_by_key(|handle| (handle.arena_index(), handle.generation()));
    type_references.dedup();
    let mut proof_facts = roots.facts.into_iter().collect::<Vec<_>>();
    proof_facts.sort_unstable_by_key(|handle| (handle.arena_index(), handle.generation()));
    ProgramReferences {
        callable_uses: roots.callable_uses,
        declarations: roots.declarations,
        symbols: roots.symbols,
        descriptor_symbols: roots.descriptor_symbols,
        expressions: roots
            .expressions
            .into_iter()
            .chain(roots.alias_argument_expressions)
            .collect(),
        statements: roots.statements,
        type_references,
        proof_facts,
        authored_selection_occurrences: roots.authored_selection_occurrences,
    }
}

pub struct MachineSidecarIndex {
    specializations: Vec<(SymbolHandle, u32)>,
    service_reaches: Vec<(SymbolHandle, u32)>,
}

fn sidecar_symbol_key(symbol: SymbolHandle) -> u64 {
    (u64::from(symbol.arena_index()) << 32) | u64::from(symbol.generation())
}

fn sidecar_symbol_rows(
    rows: &[(SymbolHandle, u32)],
    symbol: SymbolHandle,
) -> &[(SymbolHandle, u32)] {
    if rows.is_empty() {
        return &[];
    }
    let key = sidecar_symbol_key(symbol);
    let start = rows.partition_point(|(candidate, _)| sidecar_symbol_key(*candidate) < key);
    let end = start
        + rows[start..].partition_point(|(candidate, _)| sidecar_symbol_key(*candidate) <= key);
    &rows[start..end]
}

impl MachineSidecarIndex {
    pub fn build(program: &TypedTrees) -> Self {
        let mut specializations: Vec<(SymbolHandle, u32)> = program
            .machine_specializations
            .iter()
            .enumerate()
            .map(|(index, row)| (row.instance, index as u32))
            .collect();
        specializations.sort_by_key(|(symbol, _)| sidecar_symbol_key(*symbol));
        let mut service_reaches: Vec<(SymbolHandle, u32)> = program
            .authored_service_reach_rows
            .iter()
            .enumerate()
            .map(|(index, row)| (row.owner, index as u32))
            .collect();
        service_reaches.sort_by_key(|(symbol, _)| sidecar_symbol_key(*symbol));
        Self {
            specializations,
            service_reaches,
        }
    }
}

pub fn collect_machine_references(
    program: &TypedTrees,
    machine: &Machine,
    sidecars: &MachineSidecarIndex,
) -> ProgramReferences {
    let mut roots = ReferenceCollector::default();
    if roots.declaration(machine.symbol) {
        visit_machine_with_sidecars(program, &mut roots, machine, Some(sidecars));
    }
    finish_references(roots)
}

pub fn visit_program(program: &TypedTrees, roots: &mut impl TypedReferences) {
    visit_declarations(program, roots);
    let mut sidecars = None;
    for machine in program.machines() {
        if roots.declaration(machine.symbol) {
            let sidecars = sidecars.get_or_insert_with(|| MachineSidecarIndex::build(program));
            visit_machine_with_sidecars(program, roots, machine, Some(sidecars));
        }
    }
}

pub fn visit_product_operand_inputs(
    program: &TypedTrees,
    roots: &mut impl TypedReferences,
    owner: SymbolHandle,
) {
    let pending = program
        .product_operand_applications
        .iter()
        .filter_map(|(handle, application)| {
            let source = program.static_machine_application(application.supplying_application);
            (source.lexical_owner == owner).then_some(handle)
        })
        .collect::<Vec<_>>();
    if pending.is_empty() {
        return;
    }
    let live = program.live_product_operand_applications();
    for handle in pending.into_iter().filter(|handle| live.contains(handle)) {
        let application = program.product_operand_applications.get(handle);
        let source = program.static_machine_application(application.supplying_application);
        roots.enter_lexical_scope(application.declaration);
        roots.declaration_description_dependency(application.declaration);
        visit_type_parameters(program, roots, application.declaration_parameters);
        roots.leave_lexical_scope();
        roots.enter_lexical_scope(source.lexical_owner);
        for selection in program
            .product_input_selections
            .span_or_empty(application.input_selections)
        {
            if let Some(occurrence) = selection.occurrence {
                roots.authored_selection(occurrence);
            }
        }
        if let Some(arguments) = &application.argument.application {
            for argument in arguments.arguments.iter() {
                visit_static_argument(program, roots, argument);
            }
        }
        application.scope.visit_references(program, roots);
        roots.leave_lexical_scope();
    }
}

pub fn collect_expression_source_owners(
    program: &TypedTrees,
    expression_sources: &[source::SourceSpan],
) -> DeclarationSelection {
    if expression_sources.is_empty() {
        return DeclarationSelection::from_symbols([]);
    }
    struct OwnerVisitor<'a> {
        program: &'a TypedTrees,
        sources: &'a [source::SourceSpan],
        current: SymbolHandle,
        matched: bool,
        owners: DeclarationSelection,
        expressions: HashSet<ExpressionHandle>,
        types: HashSet<TypeReferenceHandle>,
        facts: HashSet<arena::Handle<ProofFact>>,
        mathematical_types: HashSet<MathematicalTypeHandle>,
    }
    impl Extend<SymbolHandle> for OwnerVisitor<'_> {
        fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {}
    }
    impl TypedReferences for OwnerVisitor<'_> {
        fn declaration(&mut self, symbol: SymbolHandle) -> bool {
            if self.owners.contains(symbol) {
                self.matched = true;
                return false;
            }
            if self.current != symbol {
                self.current = symbol;
                self.expressions.clear();
                self.types.clear();
                self.facts.clear();
                self.mathematical_types.clear();
            }
            self.matched = false;
            true
        }
        fn expression(&mut self, handle: ExpressionHandle) -> bool {
            if self.matched || !self.expressions.insert(handle) {
                return false;
            }
            if self
                .sources
                .contains(&self.program.expression_table.source_span(handle))
            {
                self.owners.include(self.current);
                self.matched = true;
                return false;
            }
            true
        }
        fn type_reference(&mut self, handle: TypeReferenceHandle) -> bool {
            !self.matched && self.types.insert(handle)
        }
        fn proof_fact(&mut self, handle: arena::Handle<ProofFact>) -> bool {
            !self.matched && self.facts.insert(handle)
        }
        fn mathematical_type(&mut self, handle: MathematicalTypeHandle) -> bool {
            !self.matched && self.mathematical_types.insert(handle)
        }
    }
    let mut visitor = OwnerVisitor {
        program,
        sources: expression_sources,
        current: SymbolHandle::invalid(),
        matched: false,
        owners: DeclarationSelection::from_symbols([]),
        expressions: HashSet::default(),
        types: HashSet::default(),
        facts: HashSet::default(),
        mathematical_types: HashSet::default(),
    };
    visit_program(program, &mut visitor);
    visitor.owners
}

pub struct ReferenceClosure {
    sidecars: Option<MachineSidecarIndex>,
    visited_declarations: DeclarationSelection,
    collector: ReferenceCollector,
}

impl Default for ReferenceClosure {
    fn default() -> Self {
        Self::new()
    }
}

impl ReferenceClosure {
    pub fn new() -> Self {
        Self {
            sidecars: None,
            visited_declarations: DeclarationSelection::from_symbols([]),
            collector: ReferenceCollector {
                track_new_expressions: true,
                ..ReferenceCollector::default()
            },
        }
    }

    pub fn extend(
        &mut self,
        program: &TypedTrees,
        selection: &DeclarationSelection,
    ) -> (
        std::collections::HashSet<SymbolHandle, symbols::BuildSymbolHasher>,
        std::collections::HashSet<ExpressionHandle, symbols::BuildSymbolHasher>,
    ) {
        let delta = selection.excluding(&self.visited_declarations);
        if delta.symbols.is_empty() && !delta.excluded {
            return (HashSet::default(), HashSet::default());
        }
        for &symbol in &delta.symbols {
            self.visited_declarations.include(symbol);
        }
        self.collector.selection = delta;
        visit_declarations(program, &mut self.collector);
        for machine in program.machines() {
            if self.collector.declaration(machine.symbol) {
                let sidecars = self
                    .sidecars
                    .get_or_insert_with(|| MachineSidecarIndex::build(program));
                visit_machine_with_sidecars(program, &mut self.collector, machine, Some(sidecars));
            }
        }
        (
            std::mem::take(&mut self.collector.symbols),
            std::mem::take(&mut self.collector.new_expressions),
        )
    }
}

pub fn program_proof_facts(
    program: &TypedTrees,
) -> impl Iterator<Item = (arena::Handle<ProofFact>, &ProofFact)> {
    let mut facts = program_scoped_proof_facts(program)
        .map(|(_, handle, _)| handle)
        .collect::<Vec<_>>();
    facts.sort_unstable_by_key(|handle| (handle.arena_index(), handle.generation()));
    facts.dedup();
    facts
        .into_iter()
        .map(move |handle| (handle, program.proof_facts.get(handle)))
}

pub fn program_scoped_proof_facts(
    program: &TypedTrees,
) -> impl Iterator<Item = (SymbolHandle, arena::Handle<ProofFact>, &ProofFact)> {
    let mut roots = ProofFactCollector::default();
    visit_program(program, &mut roots);
    let mut facts = roots.facts.into_iter().collect::<Vec<_>>();
    facts.sort_unstable_by_key(|(owner, handle)| {
        (
            owner.arena_index(),
            owner.generation(),
            handle.arena_index(),
            handle.generation(),
        )
    });
    facts
        .into_iter()
        .map(move |(owner, handle)| (owner, handle, program.proof_facts.get(handle)))
}

#[derive(Default)]
struct ProofFactCollector {
    declaration: SymbolHandle,
    scopes: Vec<SymbolHandle>,
    facts: HashSet<(SymbolHandle, arena::Handle<ProofFact>)>,
}
impl Extend<SymbolHandle> for ProofFactCollector {
    fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {}
}
impl TypedReferences for ProofFactCollector {
    fn declaration(&mut self, symbol: SymbolHandle) -> bool {
        self.declaration = symbol;
        true
    }
    fn enter_lexical_scope(&mut self, symbol: SymbolHandle) {
        self.scopes.push(symbol);
    }
    fn leave_lexical_scope(&mut self) {
        self.scopes.pop();
    }
    fn statement(&mut self, _: &StatementNode) -> bool {
        false
    }
    fn expression(&mut self, _: ExpressionHandle) -> bool {
        false
    }
    fn type_reference(&mut self, _: TypeReferenceHandle) -> bool {
        false
    }
    fn mathematical_type(&mut self, _: MathematicalTypeHandle) -> bool {
        false
    }
    fn proof_fact(&mut self, handle: arena::Handle<ProofFact>) -> bool {
        self.facts.insert((
            self.scopes.last().copied().unwrap_or(self.declaration),
            handle,
        ));
        false
    }
}

fn visit_proof_facts(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    facts: arena::HandleSpan<ProofFact>,
) {
    for (offset, fact) in program.proof_facts.span_or_empty(facts).iter().enumerate() {
        let handle = facts.handle_at(offset as u32);
        if retained.proof_fact(handle) {
            visit_proof_fact(program, retained, fact);
        }
    }
}

fn visit_symbol(retained: &mut impl TypedReferences, symbol: SymbolHandle) {
    retained.extend([symbol]);
}

/// A domain's establishment routes name the trait requirement or exact machine
/// that may issue a value into the domain. Lowering and the checker resolve
/// those names from the domain itself, not from any expression, so a reached
/// domain reaches its issuers: a program narrowed to what it reaches must
/// still hold the trait a route names.
fn visit_domain_establishment_routes(
    retained: &mut impl TypedReferences,
    routes: &[language_semantics::DomainEstablishmentRoute],
) {
    for route in routes {
        match *route {
            language_semantics::DomainEstablishmentRoute::CheckedRequirement {
                trait_definition: owner,
                requirement,
            }
            | language_semantics::DomainEstablishmentRoute::BoundaryRequirement {
                boundary_trait: owner,
                requirement,
            } => retained.extend([owner, requirement]),
            language_semantics::DomainEstablishmentRoute::ExactMachine { machine } => {
                retained.extend([machine]);
            }
        }
    }
}

fn visit_callable_symbol(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    symbol: SymbolHandle,
) {
    retained.callable_use(symbol);
    visit_symbol(retained, symbol);
    if !retained.derived_declaration_references() {
        return;
    }
    for (_, plan) in program.access_lifecycle_plans.iter() {
        if let crate::AccessLifecyclePlan::Evaluated {
            callable,
            policy,
            schema,
            policy_machine,
            storage,
            profile_carrier,
            ..
        } = plan
            && *callable == symbol
        {
            visit_symbol(retained, *policy);
            visit_symbol(retained, *schema);
            visit_symbol(retained, *policy_machine);
            visit_type_reference(program, retained, storage.carrier);
            if let Some(profile) = profile_carrier {
                visit_type_reference(program, retained, *profile);
            }
            for field in [&storage.base, &storage.length] {
                visit_symbol(retained, field.symbol);
                visit_type_reference(program, retained, field.type_reference);
            }
            if let crate::AccessLifecycleOwner::Run {
                carrier,
                storage,
                count,
                ..
            } = &storage.owner
            {
                visit_type_reference(program, retained, *carrier);
                for field in [storage, count] {
                    visit_symbol(retained, field.symbol);
                    visit_type_reference(program, retained, field.type_reference);
                }
            }
        }
    }
    let Some(application) = program.closed_requirement_application(symbol) else {
        return;
    };
    visit_symbol(retained, application.declaring_trait);
    visit_symbol(retained, application.template);
    application.trait_scope.visit_references(program, retained);
    for reference in application.trait_arguments.iter().copied().chain(
        application
            .local_arguments
            .iter()
            .map(|argument| argument.type_reference()),
    ) {
        visit_type_reference(program, retained, reference);
    }
    let source = program.static_machine_application(application.application);
    visit_symbol(retained, source.binding_declaration);
    retained.declaration_description_dependency(source.lexical_owner);
    visit_static_machine_bindings(program, retained, application.application);
}

fn visit_authored_selection(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    occurrence: crate::AuthoredDeclarationSelectionOccurrenceId,
) {
    retained.authored_selection(occurrence);
    if let Some(selection) = program.authored_declaration_selections().get(occurrence)
        && let crate::AuthoredDeclarationSelectionTarget::Resolved(target) = selection.target()
        && program.symbols.get(target.selected_symbol()).kind == symbols::SymbolKind::Const
    {
        visit_symbol(retained, target.selected_symbol());
    }
}

pub fn visit_static_argument(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    argument: &StaticMachineArgument,
) {
    visit_callable_symbol(program, retained, argument.symbol);
    visit_type_reference(program, retained, argument.type_reference);
    if let Some(application) = &argument.application {
        for nested in application.arguments.iter() {
            visit_static_argument(program, retained, nested);
        }
        if let Some(member) = program.expression_table.static_member(application.member) {
            visit_static_argument(program, retained, member);
        }
    }
}

pub fn visit_static_machine_bindings(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    application: StaticMachineApplicationHandle,
) {
    visit_static_machine_bindings_inner(program, retained, application, &mut HashSet::default());
}

fn visit_static_machine_bindings_inner(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    application: StaticMachineApplicationHandle,
    visited: &mut HashSet<StaticMachineApplicationHandle>,
) {
    if !retained.derived_declaration_references()
        || !application.is_valid()
        || !visited.insert(application)
    {
        return;
    }
    retained.static_application(application);
    let source = program.static_machine_application(application);
    visit_static_machine_bindings_inner(
        program,
        retained,
        source.callable_contract_source,
        visited,
    );
    if !program
        .static_machine_application_lifetimes(application)
        .is_empty()
    {
        retained.declaration_description_dependency(source.lifetime_binding_declaration);
        retained.declaration_description_dependency(source.lexical_owner);
    }
    for selection in program
        .product_input_selections
        .span_or_empty(source.input_selections)
    {
        if let Some(occurrence) = selection.occurrence {
            retained.authored_selection(occurrence);
        }
    }
    if let Some(selection) = &program
        .static_machine_application(application)
        .conformance_selection
        && let Some(selected) = selection.application.as_deref()
    {
        visit_closed_conformance_inner(program, retained, selected, visited);
    }
    for applied in program.applied_machine_callables(application) {
        visit_static_machine_bindings_inner(program, retained, applied.provider, visited);
        visit_signature(program, retained, applied.required);
        visit_signature(program, retained, applied.selected);
    }
    for selected in program
        .expression_table
        .static_machine_application_selected_machines(application)
    {
        visit_static_argument(program, retained, selected);
    }
    for binding in program.static_machine_application_bindings(application) {
        use crate::static_applications::StaticMachineBindingValue;
        match &binding.value {
            StaticMachineBindingValue::Type(reference)
            | StaticMachineBindingValue::Const(reference) => {
                visit_type_reference(program, retained, *reference)
            }
            StaticMachineBindingValue::RuntimeValue {
                type_reference,
                subject,
                expression,
            } => {
                visit_type_reference(program, retained, *type_reference);
                visit_symbol(retained, *subject);
                visit_expression(program, retained, *expression);
            }
            StaticMachineBindingValue::Machine(argument)
            | StaticMachineBindingValue::Conformance(argument) => {
                visit_static_argument(program, retained, argument)
            }
        }
    }
}

pub fn visit_closed_conformance(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    application: &crate::ClosedConformanceApplication,
) {
    visit_closed_conformance_inner(program, retained, application, &mut HashSet::default());
}

fn visit_closed_conformance_inner(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    application: &crate::ClosedConformanceApplication,
    visited: &mut HashSet<StaticMachineApplicationHandle>,
) {
    if !retained.derived_declaration_references() {
        return;
    }
    application.trait_scope.visit_references(program, retained);
    visit_static_machine_bindings_inner(
        program,
        retained,
        application.callable_application,
        visited,
    );
    for argument in &application.const_arguments {
        use crate::ClosedConformanceConstArgument;
        let (parameter, argument) = match argument {
            ClosedConformanceConstArgument::Evaluated {
                parameter_carrier,
                declared_carrier,
                ..
            } => (*parameter_carrier, *declared_carrier),
            ClosedConformanceConstArgument::CallerExpression {
                parameter_carrier,
                expression,
            } => (*parameter_carrier, *expression),
            ClosedConformanceConstArgument::CallerBinder {
                parameter_carrier,
                binder_carrier,
                ..
            } => (*parameter_carrier, *binder_carrier),
        };
        visit_type_reference(program, retained, parameter);
        visit_type_reference(program, retained, argument);
    }
    for machine in application.machine_arguments.iter() {
        visit_symbol(retained, *machine);
    }
    for argument in application.arguments.iter() {
        visit_static_argument(program, retained, argument);
    }
    for row in &application.rows {
        visit_symbol(retained, row.realization_machine);
        visit_symbol(retained, row.realization_state);
    }
}

fn visit_declaration_dependencies(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    owner: SymbolHandle,
) {
    if !retained.derived_declaration_references() {
        return;
    }
    visit_constant_evaluation_entries(program, retained, owner);
    visit_product_operand_inputs(program, retained, owner);
    for (_, selected) in program
        .data_implementation_applications
        .iter()
        .filter(|(_, selected)| selected.lexical_owner == owner)
    {
        visit_type_reference(program, retained, selected.source_application);
        visit_type_reference(program, retained, selected.argument);
        visit_static_machine_bindings(program, retained, selected.supplying_application);
        visit_closed_conformance(program, retained, &selected.application);
        for (_, capture) in program
            .captured_machine_applications
            .iter()
            .filter(|(_, capture)| {
                capture.conformance_application == selected.application
                    && program
                        .static_machine_application(capture.supplying_application)
                        .lexical_owner
                        == owner
            })
        {
            visit_symbol(retained, capture.instance);
        }
    }
    if program.trait_implementation_applications.is_empty() {
        return;
    }
    for (_, selected) in program
        .trait_implementation_applications
        .iter()
        .filter(|(_, selected)| {
            program
                .static_machine_application(selected.supplying_application)
                .lexical_owner
                == owner
        })
    {
        selected.source_scope.visit_references(program, retained);
        visit_type_reference(program, retained, selected.argument);
        visit_static_machine_bindings(program, retained, selected.supplying_application);
        visit_closed_conformance(program, retained, &selected.application);
        if !program.captured_machine_applications.is_empty() {
            for (_, capture) in
                program
                    .captured_machine_applications
                    .iter()
                    .filter(|(_, capture)| {
                        capture.conformance_application == selected.application
                            && program
                                .static_machine_application(capture.supplying_application)
                                .lexical_owner
                                == owner
                    })
            {
                visit_symbol(retained, capture.instance);
            }
        }
    }
}

pub fn visit_declarations(program: &TypedTrees, retained: &mut impl TypedReferences) {
    for alias in program.type_aliases() {
        if !retained.declaration(alias.symbol) {
            continue;
        }
        visit_alias(program, retained, alias);
    }

    for data in program.data_definitions() {
        if !retained.declaration(data.symbol) {
            continue;
        }
        visit_data(program, retained, data);
    }

    for definition in program.traits() {
        if !retained.declaration(definition.symbol) {
            continue;
        }
        visit_trait(program, retained, definition);
    }

    for conformance in program.conformances() {
        if !retained.declaration(conformance.symbol) {
            continue;
        }
        visit_declaration_dependencies(program, retained, conformance.symbol);
        visit_symbol(retained, conformance.symbol);
        visit_type_parameters(program, retained, conformance.type_parameters);
        visit_symbol(retained, conformance.trait_symbol);
        for row in &conformance.rows {
            visit_symbol(retained, row.declaring_trait);
            visit_symbol(retained, row.requirement);
            visit_symbol(retained, row.realization_machine);
            visit_symbol(retained, row.realization_state);
        }
        for argument in program.type_reference_handles(conformance.arguments) {
            visit_type_reference(program, retained, *argument);
        }
    }

    for proposition in program.propositions() {
        if !retained.declaration(proposition.symbol) {
            continue;
        }
        visit_declaration_dependencies(program, retained, proposition.symbol);
        visit_symbol(retained, proposition.symbol);
        for binder in program.proposition_binders(proposition) {
            visit_symbol(retained, binder.symbol);
            if let crate::proposition::PropositionBinderKind::Const { type_reference } = binder.kind
            {
                visit_type_reference(program, retained, type_reference);
            }
        }
        for parameter in program.proposition_parameters(proposition) {
            visit_type_reference(program, retained, parameter.type_reference);
        }
        match &proposition.body {
            crate::proposition::PropositionBody::Witness { evidence } => {
                visit_type_reference(program, retained, *evidence);
            }
            crate::proposition::PropositionBody::Transparent { proposition } => {
                visit_proposition_formula(program, retained, proposition);
            }
            crate::proposition::PropositionBody::Primitive => {}
        }
    }

    for definition in program.mathematical_definitions() {
        if !retained.declaration(definition.symbol) {
            continue;
        }
        visit_declaration_dependencies(program, retained, definition.symbol);
        visit_type_parameters(program, retained, definition.binders);
        for parameter in program.mathematical_parameters(definition.parameters) {
            visit_mathematical_type(program, retained, parameter.ty);
        }
        visit_mathematical_type(program, retained, definition.result);
        if let crate::mathematical::MathematicalBody::Definition(expression) = definition.body {
            visit_expression(program, retained, expression);
        }
    }

    for measure in program.measures() {
        if !retained.declaration(measure.symbol) {
            continue;
        }
        visit_declaration_dependencies(program, retained, measure.symbol);
        visit_symbol(retained, measure.symbol);
        if let Some(parameter) = &measure.parameter {
            visit_type_reference(program, retained, parameter.type_reference);
        }
        visit_type_reference(program, retained, measure.return_type);
        for expression in program.expression_table.expression_handles(measure.body) {
            visit_expression(program, retained, *expression);
        }
    }

    for declaration in program.const_declarations() {
        if !retained.declaration(declaration.symbol) {
            continue;
        }
        visit_const(program, retained, declaration);
    }

    for operator in program
        .operators()
        .iter()
        .chain(program.machine_token_bindings())
        .chain(
            program
                .domain_definitions()
                .iter()
                .flat_map(|domain| program.domain_operators(domain)),
        )
    {
        if !retained.declaration(operator.symbol) {
            continue;
        }
        visit_declaration_dependencies(program, retained, operator.symbol);
        visit_symbol(retained, operator.symbol);
        visit_type_parameters(program, retained, operator.type_parameters);
        for parameter in program
            .tables
            .state_parameters
            .span_or_empty(operator.parameters)
        {
            visit_type_reference(program, retained, parameter.type_reference);
        }
        visit_type_reference(program, retained, operator.return_type);
        for contract in program
            .tables
            .signature_contracts
            .span_or_empty(operator.contracts)
        {
            visit_contract(program, retained, contract);
        }
    }

    for domain in program.domain_definitions() {
        if !retained.declaration(domain.symbol) {
            continue;
        }
        visit_declaration_dependencies(program, retained, domain.symbol);
        visit_symbol(retained, domain.symbol);
        visit_type_parameters(program, retained, domain.type_parameters);
        if let Some(selection) = &domain.cleanup_selection {
            visit_cleanup_selection(program, retained, selection);
        }
        visit_type_reference(program, retained, domain.target_type);
        for argument in &domain.index_arguments {
            visit_type_reference(program, retained, *argument);
        }
        visit_proof_facts(program, retained, domain.facts);
        visit_domain_establishment_routes(retained, &domain.establishment_routes);
    }

    for application in &program.applied_domain_applications {
        if !retained.derived_declaration_references()
            || !retained.declaration(application.declaration)
        {
            continue;
        }
        for argument in &application.arguments {
            visit_type_reference(program, retained, *argument);
        }
        visit_type_reference(program, retained, application.theory.target_type);
        visit_proof_facts(program, retained, application.theory.facts);
        visit_domain_establishment_routes(retained, &application.theory.establishment_routes);
        if let Some(selection) = &application.theory.cleanup_selection {
            visit_cleanup_selection(program, retained, selection);
        }
    }

    for application in &program.applied_data_cleanup_selections {
        if retained.declaration(application.declaration) {
            visit_cleanup_selection(program, retained, &application.selection);
        }
    }

    for schema in program.wire_schemas() {
        if !retained.declaration(schema.symbol) {
            continue;
        }
        visit_declaration_dependencies(program, retained, schema.symbol);
        visit_symbol(retained, schema.symbol);
        for member in program.tables.wire_members.span_or_empty(schema.members) {
            if let crate::wire::WireMember::Field(field) = member {
                visit_type_reference(program, retained, field.type_reference);
            }
        }
    }
}

fn visit_cleanup_selection(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    selection: &crate::domain::CleanupSelection,
) {
    if !retained.derived_declaration_references() {
        return;
    }
    let application = program.static_machine_application(selection.application);
    if let Some(occurrence) = selection.authorization {
        retained.authored_selection(occurrence);
    }
    visit_symbol(retained, application.registration_operation);
    visit_symbol(retained, selection.executable);
    visit_type_reference(program, retained, selection.subject);
    for argument in program.static_machine_application_arguments(selection.application) {
        visit_static_argument(program, retained, argument);
    }
    visit_static_machine_bindings(program, retained, selection.application);
}

pub fn visit_proof_fact(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    fact: &ProofFact,
) {
    match fact {
        ProofFact::Expression(expression) => {
            visit_expression(program, retained, *expression);
        }
        ProofFact::Membership(membership) => {
            if let Some(occurrence) = membership.authored_domain_selection {
                visit_authored_selection(program, retained, occurrence);
            }
            visit_expression(program, retained, membership.value);
            visit_symbol(retained, membership.domain_symbol);
            for argument in program.type_reference_handles(membership.domain_arguments) {
                visit_type_reference(program, retained, *argument);
            }
        }
        ProofFact::Proposition(application) => {
            visit_proposition_application(program, retained, application);
        }
    }
}

fn visit_proposition_application(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    application: &PropositionApplication,
) {
    visit_symbol(retained, application.proposition);
    for binder in application.binder_arguments.iter() {
        if binder.kind == PropositionBinderArgumentKind::Machine {
            visit_symbol(retained, binder.symbol);
        }
    }
    for argument in program
        .expression_table
        .expression_handles(application.arguments)
    {
        visit_expression(program, retained, *argument);
    }
}

fn visit_proposition_formula(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    formula: &PropositionFormula,
) {
    match formula {
        PropositionFormula::Application(application) => {
            visit_proposition_application(program, retained, application);
        }
        PropositionFormula::BooleanExpression(expression) => {
            visit_expression(program, retained, *expression);
        }
    }
}

fn visit_alias(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    alias: &crate::type_aliases::TypeAliasDefinition,
) {
    visit_symbol(retained, alias.symbol);
    visit_type_parameters(program, retained, alias.type_parameters);
    for bound in &alias.conformance_bounds {
        visit_symbol(retained, bound.trait_symbol);
        for argument in &bound.arguments {
            visit_type_reference(program, retained, *argument);
        }
    }
    visit_type_reference(program, retained, alias.expansion);
}

fn visit_const(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    declaration: &crate::constant::ConstDeclaration,
) {
    visit_declaration_dependencies(program, retained, declaration.symbol);
    visit_symbol(retained, declaration.symbol);
    visit_type_reference(program, retained, declaration.declared_type);
    visit_expression(program, retained, declaration.authored_initializer);
    if retained.derived_declaration_references() {
        visit_expression(program, retained, declaration.materialized_initializer);
    }
    if retained.derived_declaration_references()
        && declaration.evaluated_value.is_none()
        && program
            .tables
            .machines
            .is_valid(declaration.evaluation_entry)
    {
        visit_expression(program, retained, declaration.evaluation_expression);
        visit_symbol(
            retained,
            program
                .tables
                .machines
                .get(declaration.evaluation_entry)
                .symbol,
        );
    }
}

fn visit_data(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    data: &crate::data::DataDefinition,
) {
    visit_declaration_dependencies(program, retained, data.symbol);
    visit_symbol(retained, data.symbol);
    if retained.derived_declaration_references() && !program.plan_laid_layouts.is_empty() {
        for plan in program
            .plan_laid_layouts
            .iter()
            .filter(|plan| plan.data_symbol == data.symbol)
        {
            visit_type_reference(program, retained, plan.application);
            visit_type_reference(program, retained, plan.semantic_type);
            visit_closed_conformance(program, retained, &plan.policy_application);
            visit_symbol(retained, plan.schema_symbol);
            visit_symbol(retained, plan.policy_symbol);
            visit_symbol(retained, plan.policy_plan_machine_symbol);
            for alternative in plan.target_layouts(program) {
                retained.layout_realization_dependency(alternative.machine);
            }
            for geometry in plan.geometries(program) {
                for demand in &geometry.private_callback_demands {
                    visit_closed_conformance(
                        program,
                        retained,
                        &demand.slot_application.application,
                    );
                }
            }
        }
    }
    if retained.derived_declaration_references() && !program.access_view_plans.is_empty() {
        for plan in program
            .access_view_plans
            .iter()
            .filter(|plan| plan.data_symbol == data.symbol)
        {
            visit_symbol(retained, plan.schema_symbol);
            visit_symbol(retained, plan.policy_symbol);
            visit_symbol(retained, plan.policy_plan_machine_symbol);
            for field in &plan.fields {
                visit_type_reference(program, retained, field.accessor_type);
                visit_type_reference(program, retained, field.value_type);
                if let Some(resident) = &field.atomic_resident {
                    visit_type_reference(program, retained, resident.resident_type);
                }
            }
        }
    }
    visit_type_parameters(program, retained, data.type_parameters);
    for bound in &data.conformance_bounds {
        visit_symbol(retained, bound.trait_symbol);
        for argument in &bound.arguments {
            visit_type_reference(program, retained, *argument);
        }
    }
    if data.predicate_subject_symbol.is_valid() {
        visit_symbol(retained, data.predicate_subject_symbol);
        visit_type_reference(program, retained, data.predicate_subject_type);
    }
    if let Some(instance) = data.generic_instance {
        visit_type_reference(program, retained, instance);
    }
    if let Some(quotient) = &data.quotient {
        visit_type_reference(program, retained, quotient.carrier);
        visit_symbol(retained, quotient.relation_symbol);
    }
    if let Some(selection) = &data.cleanup_selection {
        visit_cleanup_selection(program, retained, selection);
    }
    visit_proof_facts(program, retained, data.requires_facts);
    for member in program.data_members(data) {
        match member {
            crate::data::DataMember::Field(field) => {
                visit_type_reference(program, retained, field.type_reference);
            }
            crate::data::DataMember::Variant(variant) => {
                for field in program.data_payload_fields(variant) {
                    visit_type_reference(program, retained, field.type_reference);
                }
                visit_proof_facts(program, retained, variant.requires_facts);
            }
        }
    }
}

fn visit_trait(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    definition: &crate::trait_definition::TraitDefinition,
) {
    visit_declaration_dependencies(program, retained, definition.symbol);
    visit_symbol(retained, definition.symbol);
    visit_type_parameters(program, retained, definition.type_parameters);

    for requirement in program
        .trait_requirements(definition)
        .iter()
        .chain(definition.refines.iter())
    {
        visit_symbol(retained, requirement.symbol);
        for argument in program.type_reference_handles(requirement.arguments) {
            visit_type_reference(program, retained, *argument);
        }
    }
    for refinement in &definition.refinement_clauses {
        visit_signature(program, retained, &refinement.signature);
    }
    for bound in &definition.conformance_bounds {
        for argument in &bound.arguments {
            visit_type_reference(program, retained, *argument);
        }
    }
    let include_derived = retained.derived_declaration_references();
    for signature in program.trait_machine_signatures(definition) {
        visit_signature(program, retained, signature);
    }
    if include_derived {
        for contract in program.applied_trait_contracts(definition.symbol) {
            if let Some(scope) = contract.trait_scope {
                scope.visit_references(program, retained);
            }
            visit_signature(program, retained, contract.signature);
        }
    }
}

pub fn visit_signature(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    signature: &crate::signature::StateSignature,
) {
    retained.callable_signature(signature);
    retained.enter_lexical_scope(signature.symbol);
    visit_constant_evaluation_entries(program, retained, signature.symbol);
    visit_type_parameters(program, retained, signature.type_parameters);
    for bound in &signature.conformance_bounds {
        visit_symbol(retained, bound.binder);
        visit_symbol(retained, bound.trait_symbol);
        for argument in &bound.arguments {
            visit_type_reference(program, retained, *argument);
        }
    }
    for parameter in program
        .tables
        .state_parameters
        .span_or_empty(signature.parameters)
    {
        visit_type_reference(program, retained, parameter.type_reference);
    }
    visit_type_reference(program, retained, signature.return_type);
    for invocation in program
        .tables
        .signature_invokes
        .span_or_empty(signature.invokes)
    {
        if let AuthoredInvocationTarget::Service(target) = invocation.target {
            visit_symbol(retained, target);
        }
    }
    for contract in program
        .tables
        .signature_contracts
        .span_or_empty(signature.contracts)
    {
        visit_contract(program, retained, contract);
    }
    retained.leave_lexical_scope();
}

fn visit_contract(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    contract: &SignatureContract,
) {
    visit_proof_facts(program, retained, contract.facts);
}

pub fn visit_type_reference(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    type_reference: TypeReferenceHandle,
) {
    if !type_reference.is_valid() || !retained.type_reference(type_reference) {
        return;
    }
    let extent_argument = program
        .type_reference_table
        .array_extent_argument(type_reference);
    if extent_argument.is_valid() {
        visit_type_reference(program, retained, extent_argument);
    }
    for application in program
        .type_reference_table
        .type_alias_applications(type_reference)
    {
        retained.declaration_description_dependency(application.declaration);
        if retained.type_alias_application(program, application) {
            for argument in program.type_reference_handles(application.arguments) {
                visit_type_reference(program, retained, *argument);
            }
        }
    }
    for origin in program
        .type_reference_table
        .constant_origins(type_reference)
        .into_iter()
        .flatten()
    {
        visit_symbol(retained, origin.declaration);
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            visit_type_reference(program, retained, *referee);
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            visit_type_reference(program, retained, *base_type);
            for constraint in program.type_reference_table.constraints(*constraints) {
                match constraint {
                    crate::types::TypeConstraintNode::Domain(domain) => {
                        visit_symbol(retained, domain.symbol);
                        for argument in &domain.arguments {
                            visit_type_reference(program, retained, *argument);
                        }
                    }
                    crate::types::TypeConstraintNode::Named(_)
                    | crate::types::TypeConstraintNode::ArithmeticDomain(_) => {}
                }
            }
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            visit_type_reference(program, retained, *element_type);
            match length {
                FixedArrayLength::ConstParameter { symbol, .. } => {
                    visit_symbol(retained, *symbol);
                }
                FixedArrayLength::ConstCall { machine, .. } => visit_symbol(retained, *machine),
                FixedArrayLength::Expression(value) => {
                    visit_type_reference(program, retained, *value)
                }
                FixedArrayLength::Literal(_) => {}
            }
        }
        TypeReferenceNode::Slice { element_type } => {
            visit_type_reference(program, retained, *element_type);
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            visit_symbol(retained, *base_symbol);
            for argument in program.type_reference_handles(*arguments) {
                visit_type_reference(program, retained, *argument);
            }
        }
        TypeReferenceNode::ConstExpression(expression) => {
            if retained.type_expression(*expression) {
                visit_expression(program, retained, *expression);
            }
        }
        TypeReferenceNode::DynamicTrait {
            symbol, arguments, ..
        } => {
            visit_symbol(retained, *symbol);
            for argument in program.type_reference_handles(*arguments) {
                visit_type_reference(program, retained, *argument);
            }
        }
        TypeReferenceNode::Named { symbol, .. } => {
            visit_symbol(retained, *symbol);
        }
        TypeReferenceNode::Unit => {}
    }
}

pub fn visit_expression(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    expression: ExpressionHandle,
) {
    if !expression.is_valid() || !retained.expression(expression) {
        return;
    }
    let authored = program.expression_table.materialization_source(expression);
    if authored.is_valid() && retained.type_expression(authored) {
        visit_expression(program, retained, authored);
    }
    if retained.derived_declaration_references()
        && let Some(operation) = program.expression_table.open_index_operation(expression)
    {
        visit_symbol(retained, operation.provider);
        visit_symbol(retained, operation.operator);
        visit_symbol(retained, operation.algebra_trait);
        visit_type_reference(program, retained, operation.operand_type);
        if let Some(application) = &operation.application {
            visit_static_argument(program, retained, application);
        }
    }
    for occurrence in program
        .expression_table
        .authored_selection_occurrences(expression)
    {
        visit_authored_selection(program, retained, occurrence);
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(match_expression) => {
            visit_expression(program, retained, match_expression.subject);
            for arm in program.expression_table.match_arms(match_expression.arms) {
                visit_match_arm(program, retained, arm);
            }
        }
        ExpressionNode::SavedSubject(saved) => {
            visit_expression(
                program,
                retained,
                program
                    .expression_table
                    .value_capture(saved.capture)
                    .subject,
            );
            visit_expression(program, retained, saved.pattern_head);
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in program.expression_table.expression_handles(*elements) {
                visit_expression(program, retained, *element);
            }
        }
        ExpressionNode::Atomic(atomic) => {
            visit_expression(program, retained, atomic.value);
            visit_expression(program, retained, atomic.result);
        }
        ExpressionNode::Binary(binary) => {
            visit_expression(program, retained, binary.left);
            visit_expression(program, retained, binary.right);
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => {}
        ExpressionNode::Cast(cast) => {
            visit_expression(program, retained, cast.value);
            visit_type_reference(program, retained, cast.target_type);
            if retained.derived_declaration_references() {
                visit_type_reference(program, retained, cast.result_type);
            }
            visit_symbol(retained, cast.semantic_domain_symbol);
            for argument in program.type_reference_handles(cast.semantic_domain_arguments) {
                visit_type_reference(program, retained, *argument);
            }
        }
        ExpressionNode::Call(call) => {
            visit_call_expression(program, retained, expression, call);
        }
        ExpressionNode::Indexed(indexed) => {
            visit_expression(program, retained, indexed.collection);
            visit_expression(program, retained, indexed.index);
        }
        ExpressionNode::Member(member) => {
            visit_expression(program, retained, member.receiver);
            visit_symbol(retained, member.member_symbol);
        }
        ExpressionNode::Borrow(borrow) => {
            visit_expression(program, retained, borrow.target);
        }
        ExpressionNode::Name(path) => {
            visit_name_path(program, retained, path);
        }
        ExpressionNode::Range(range) => {
            visit_expression(program, retained, range.start);
            visit_expression(program, retained, range.end);
        }
        ExpressionNode::StructLiteral(literal) => {
            visit_type_reference(program, retained, literal.application_type);

            visit_symbol(retained, literal.type_symbol);
            if let Some(case_symbol) = literal.case_symbol {
                visit_symbol(retained, case_symbol);
            }
            for field in program.expression_table.struct_fields(literal.fields) {
                visit_expression(program, retained, field.value);
            }
        }
        ExpressionNode::Unary(unary) => {
            visit_expression(program, retained, unary.operand);
        }
        ExpressionNode::TypeExpression(type_reference)
        | ExpressionNode::ZeroValue(type_reference) => {
            visit_type_reference(program, retained, *type_reference);
        }
    }
}

fn visit_match_arm(program: &TypedTrees, retained: &mut impl TypedReferences, arm: &TableMatchArm) {
    visit_match_pattern(program, retained, arm.pattern);
    visit_expression(program, retained, arm.value);
}

fn visit_match_pattern(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    pattern: crate::expression::MatchPattern,
) {
    for expression in program.expression_table.match_pattern_dependencies(pattern) {
        visit_expression(program, retained, expression);
    }
    if let crate::expression::MatchPattern::Destructure(pattern) = pattern {
        visit_type_reference(program, retained, pattern.application_type);
        for field in program.expression_table.match_fields(pattern.fields) {
            visit_symbol(retained, field.field_symbol);
        }
    }
}

fn visit_name_path(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    path: &TableNamePath,
) {
    visit_symbol(retained, path.head_symbol);
    visit_symbol(retained, path.symbol);
    for member in program
        .expression_table
        .name_path_member_symbols(path.member_symbols)
    {
        visit_symbol(retained, *member);
    }
}

fn visit_static_requirement_dispatch(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    dispatch: &crate::StaticRequirementDispatch,
    invocation: StaticMachineApplicationHandle,
) {
    visit_symbol(retained, dispatch.realization_machine);
    visit_symbol(retained, dispatch.realization_state);
    visit_symbol(retained, dispatch.requirement);
    visit_symbol(retained, dispatch.contract_signature);
    visit_symbol(retained, dispatch.declaring_trait);
    let selected = dispatch.selection_application;
    if selected.is_valid() && selected != invocation {
        let application = program.static_machine_application(selected);
        visit_symbol(retained, application.registration_operation);
        for argument in program.static_machine_application_arguments(selected) {
            visit_static_argument(program, retained, argument);
        }
        visit_static_machine_bindings(program, retained, selected);
    }
}

fn visit_call_expression(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    expression: ExpressionHandle,
    call: &TableCallExpression,
) {
    visit_callable_symbol(program, retained, call.target_symbol);
    for operator in crate::operator::named_expression_call_candidates(program, call) {
        visit_symbol(retained, operator.symbol);
    }
    // Overload selection runs after the walk, so the walk keeps every member
    // of a named machine family the call may select.
    if let Some(path) = crate::operator::named_expression_call_family_path(program, call) {
        let reference = program.expression_table.source_span(expression);
        for member in crate::operator::named_call_family_members(program, &path, reference) {
            visit_symbol(retained, member);
        }
    }
    visit_symbol(retained, call.static_machine_parameter);
    if call.authored_machine_application.is_valid() {
        visit_symbol(
            retained,
            program
                .static_machine_application(call.authored_machine_application)
                .registration_operation,
        );
    }
    visit_expression(program, retained, call.receiver);
    if call.requirement_application.is_valid() {
        visit_type_reference(program, retained, call.requirement_application);
    }
    if let Some(BuildSelectionRequest::ProviderReadiness(request)) = program
        .expression_table
        .build_selection_request(call.build_selection)
    {
        visit_type_reference(program, retained, request.requirement);
    }
    if let Some(dispatch) = &call.static_requirement_dispatch {
        visit_static_requirement_dispatch(
            program,
            retained,
            dispatch,
            call.authored_machine_application,
        );
    }
    if retained.call_arguments(program, StaticArgumentCall::Expression(expression, call)) {
        for argument in StaticArgumentCall::Expression(expression, call).arguments(program) {
            visit_static_argument(program, retained, argument);
        }
        visit_static_machine_bindings(program, retained, call.authored_machine_application);
    }
    if let Some(quotient) = &call.quotient_operation {
        visit_static_argument(program, retained, &quotient.representative_operation);
        for theorem in quotient.theorem_evidence.iter() {
            visit_static_argument(program, retained, &theorem.application);
        }
    }
    if let Some(installation) = &call.installed_interrupt_operation {
        visit_symbol(retained, installation.registrar_trait);
        if retained.derived_declaration_references() {
            visit_callable_symbol(program, retained, installation.registrar_target);
            visit_static_machine_bindings(program, retained, installation.registrar_application);
            installation
                .registrar_receiving_scope
                .visit_references(program, retained);
        }
        visit_static_argument(program, retained, &installation.registrar);
        visit_static_argument(program, retained, &installation.timer_slot);
    }
    if let Some(layout) = &call.private_layout_operation {
        visit_static_argument(program, retained, &layout.selected_slot);
    }
    if let Some(query) = &call.native_code_layout {
        visit_static_argument(program, retained, &query.operand);
    }
    if let Some(query) = &call.layout_query {
        visit_static_argument(program, retained, &query.operand);
    }
    for argument in program.expression_table.expression_handles(call.arguments) {
        visit_expression(program, retained, *argument);
    }
}

fn visit_conformance(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    conformance: &TraitConformance,
) {
    visit_symbol(retained, conformance.symbol);
    visit_symbol(retained, conformance.requirement_symbol);
    visit_expression(program, retained, conformance.via_expression);
    for argument in program.type_reference_handles(conformance.arguments) {
        visit_type_reference(program, retained, *argument);
    }
}

fn visit_statement(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    statement: &StatementNode,
) {
    if !retained.statement(statement) {
        return;
    }
    for (_, expression) in program.tables.expression_roots(statement) {
        visit_expression(program, retained, expression);
    }
    match statement {
        StatementNode::Assembly(_) => {}
        StatementNode::Call(call) => {
            if let Some(occurrence) = call.authored_call_selection {
                visit_authored_selection(program, retained, occurrence);
            }
            visit_callable_symbol(program, retained, call.target_symbol);
            for operator in crate::operator::named_statement_call_candidates(program, call) {
                visit_symbol(retained, operator.symbol);
            }
            if let Some(path) = crate::operator::named_statement_call_family_path(program, call) {
                for member in
                    crate::operator::named_call_family_members(program, &path, call.source_span)
                {
                    visit_symbol(retained, member);
                }
            }
            visit_symbol(retained, call.static_machine_parameter);
            if call.authored_machine_application.is_valid() {
                visit_symbol(
                    retained,
                    program
                        .static_machine_application(call.authored_machine_application)
                        .registration_operation,
                );
            }
            if let Some(dispatch) = &call.static_requirement_dispatch {
                visit_static_requirement_dispatch(
                    program,
                    retained,
                    dispatch,
                    call.authored_machine_application,
                );
            }
            if let Some(request) = &call.installed_interrupt_operation {
                visit_symbol(retained, request.registrar_trait);
                if retained.derived_declaration_references() {
                    visit_callable_symbol(program, retained, request.registrar_target);
                    visit_static_machine_bindings(program, retained, request.registrar_application);
                    request
                        .registrar_receiving_scope
                        .visit_references(program, retained);
                }
                visit_static_argument(program, retained, &request.registrar);
                visit_static_argument(program, retained, &request.timer_slot);
            }
            if retained.call_arguments(program, StaticArgumentCall::Statement(call)) {
                for argument in StaticArgumentCall::Statement(call).arguments(program) {
                    visit_static_argument(program, retained, argument);
                }
                visit_static_machine_bindings(program, retained, call.authored_machine_application);
            }
        }
        StatementNode::LocalData(local) => {
            visit_type_reference(program, retained, local.type_reference);
            for field in program.expression_table.match_fields(local.pattern_fields) {
                if field.field_symbol.is_valid() {
                    visit_symbol(retained, field.field_symbol);
                }
            }
            if local.pattern_subject.is_valid() {
                visit_symbol(retained, local.pattern_subject);
            }
        }
        StatementNode::Assignment(assignment) => {
            if retained.derived_declaration_references() {
                visit_type_reference(program, retained, assignment.receiving_type);
            }
        }
        StatementNode::Transition(transition) => {
            for pattern in program
                .statement_table
                .transition_patterns(transition.patterns)
            {
                visit_expression(program, retained, pattern.subject);
                visit_match_pattern(
                    program,
                    retained,
                    crate::expression::MatchPattern::Destructure(pattern.pattern),
                );
            }

            for target in transition.targets() {
                if let TransitionTargetNode::Named {
                    path,
                    authored_call_selection,
                    ..
                } = program.statement_table.transition_target(target)
                {
                    if let Some(occurrence) = authored_call_selection {
                        visit_authored_selection(program, retained, *occurrence);
                    }
                    visit_symbol(retained, path.symbol);
                    visit_symbol(retained, path.receiver_symbol);
                    visit_symbol(retained, path.head_symbol);
                }
                if let Some((name, reference)) =
                    crate::operator::named_transition_family_path(program, target)
                {
                    for member in
                        crate::operator::named_call_family_members(program, name, reference)
                    {
                        visit_symbol(retained, member);
                    }
                }
            }
        }
        StatementNode::AssemblyFact(fact) => visit_proof_facts(
            program,
            retained,
            arena::HandleSpan::from_parts(fact.fact, 1),
        ),
        StatementNode::Expression(_) => {}
    }
}

fn visit_type_parameters(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    parameters: arena::HandleSpan<crate::data::TypeParameter>,
) {
    use crate::data::TypeParameterKind;
    for parameter in program.data_type_parameters.span_or_empty(parameters) {
        match &parameter.kind {
            TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference } => {
                visit_type_reference(program, retained, *type_reference)
            }
            TypeParameterKind::Machine { contract } => {
                let signature = match contract {
                    crate::data::MachineParameterContract::Structural(signature) => Some(signature),
                    crate::data::MachineParameterContract::Nominal {
                        receiving_trait,
                        receiving_arguments,
                        trait_definition,
                        trait_arguments,
                        instantiated,
                        ..
                    } => {
                        visit_symbol(retained, *receiving_trait);
                        visit_symbol(retained, *trait_definition);
                        for argument in receiving_arguments.iter().chain(trait_arguments) {
                            visit_type_reference(program, retained, *argument);
                        }
                        instantiated.as_deref()
                    }
                    crate::data::MachineParameterContract::RequirementIdentity => None,
                };
                if let Some(signature) = signature {
                    visit_signature(program, retained, signature);
                }
            }
            TypeParameterKind::Proposition { contract } => {
                for parameter in program
                    .tables
                    .state_parameters
                    .span_or_empty(contract.parameters)
                {
                    visit_type_reference(program, retained, parameter.type_reference);
                }
            }
            TypeParameterKind::Type => {}
            TypeParameterKind::Implementation => visit_symbol(retained, parameter.symbol),
        }
    }
}

pub fn visit_machine(program: &TypedTrees, retained: &mut impl TypedReferences, machine: &Machine) {
    visit_machine_with_sidecars(program, retained, machine, None);
}

fn visit_machine_with_sidecars(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    machine: &Machine,
    sidecars: Option<&MachineSidecarIndex>,
) {
    retained.enter_lexical_scope(machine.symbol);
    if retained.derived_declaration_references() {
        if let Some(declaration) = program.const_declaration_for_evaluation_entry(machine.symbol) {
            visit_symbol(retained, declaration.symbol);
        }
        if let Some(entry) = program.constant_evaluation_entry(machine.symbol) {
            retained.declaration_description_dependency(entry.owner);
        }
        visit_declaration_dependencies(program, retained, machine.symbol);
        for signature in program.requirement_invocation_signatures(machine.symbol) {
            visit_signature(program, retained, signature);
        }
        if let Some(origin) = machine.lexical_region_origin {
            retained.declaration_description_dependency(origin.machine);
            retained.declaration_description_dependency(origin.state);
        }
        if !program.captured_machine_applications.is_empty() {
            for (_, capture) in program
                .captured_machine_applications
                .iter()
                .filter(|(_, capture)| capture.instance == machine.symbol)
            {
                visit_symbol(retained, capture.template);
                let source = program.static_machine_application(capture.supplying_application);
                retained.declaration_description_dependency(source.lexical_owner);
                for (_, selected) in
                    program
                        .data_implementation_applications
                        .iter()
                        .filter(|(_, selected)| {
                            selected.supplying_application == capture.supplying_application
                                && selected.application == capture.conformance_application
                        })
                {
                    visit_type_reference(program, retained, selected.source_application);
                }
                visit_symbol(retained, capture.conformance_application.declaration);
                visit_static_machine_bindings(program, retained, capture.binding_application);
                visit_static_machine_bindings(program, retained, capture.supplying_application);
                visit_closed_conformance(program, retained, &capture.conformance_application);
            }
        }
        let mut visit_specialization = |specialization: &crate::MachineSpecialization| {
            visit_symbol(retained, specialization.template);
            visit_type_parameters(program, retained, specialization.template_parameters);
            visit_static_machine_bindings(program, retained, specialization.binding_application);
            for realization in &specialization.operator_realizations {
                for argument in &realization.arguments {
                    use crate::operator::ClosedOperatorApplicationArgument;
                    let reference = match argument {
                        ClosedOperatorApplicationArgument::Type { type_reference, .. } => {
                            *type_reference
                        }
                        ClosedOperatorApplicationArgument::Const {
                            declared_carrier, ..
                        } => *declared_carrier,
                    };
                    visit_type_reference(program, retained, reference);
                }
            }
            for application in &specialization.conformance_applications {
                visit_closed_conformance(program, retained, application);
            }
        };
        if let Some(sidecars) = sidecars {
            for &(_, index) in sidecar_symbol_rows(&sidecars.specializations, machine.symbol) {
                visit_specialization(&program.machine_specializations[index as usize]);
            }
        } else if !program.machine_specializations.is_empty() {
            for specialization in program
                .machine_specializations
                .iter()
                .filter(|row| row.instance == machine.symbol)
            {
                visit_specialization(specialization);
            }
        }
        if !program.ranking_expression_custody.is_empty() {
            for custody in program
                .ranking_expression_custody
                .iter()
                .filter(|row| row.machine == machine.symbol)
            {
                for subject in &custody.subjects {
                    visit_expression(program, retained, *subject);
                }
                for argument in &custody.view_arguments {
                    visit_expression(program, retained, *argument);
                }
                if let Some(rank_range) = custody.rank_range {
                    visit_expression(program, retained, rank_range);
                }
            }
        }
        if !program.proof_output_calls.is_empty() {
            for output in program
                .proof_output_calls
                .iter()
                .filter(|row| row.machine_symbol == machine.symbol)
            {
                visit_expression(program, retained, output.call);
            }
        }
    }
    let from = machine.symbol;
    visit_type_parameters(program, retained, machine.type_parameters);
    for equation in program
        .declared_type_equations
        .span_or_empty(machine.declared_type_equations)
    {
        visit_type_reference(program, retained, equation.parameter);
        visit_type_reference(program, retained, equation.definition);
    }
    visit_type_reference(program, retained, machine.attached_data_application);
    for contract in program
        .tables
        .signature_contracts
        .span_or_empty(machine.contracts)
    {
        visit_contract(program, retained, contract);
    }

    for invocation in program.machine_invokes(machine) {
        if let AuthoredInvocationTarget::Service(target) = invocation.target {
            visit_symbol(retained, target);
        }
    }

    if retained.derived_declaration_references() {
        if let Some(sidecars) = sidecars {
            for &(_, index) in sidecar_symbol_rows(&sidecars.service_reaches, from) {
                for target in &program.authored_service_reach_rows[index as usize].targets {
                    visit_symbol(retained, target.service);
                }
            }
        } else if !program.authored_service_reach_rows.is_empty() {
            for row in program.authored_service_reach_rows_for(from) {
                for target in &row.targets {
                    visit_symbol(retained, target.service);
                }
            }
        }
    }
    for conformance in program.machine_trait_conformances(machine) {
        visit_conformance(program, retained, conformance);
    }

    for owned in program.machine_owned_data(machine) {
        visit_type_reference(program, retained, owned.type_reference);
        visit_expression(program, retained, owned.initial_value);
    }

    for bound in &machine.conformance_bounds {
        for argument in &bound.arguments {
            visit_type_reference(program, retained, *argument);
        }
    }

    for state in program.machine_states(machine) {
        visit_constant_evaluation_entries(program, retained, state.symbol);
        for parameter in program.state_parameters(state) {
            visit_type_reference(program, retained, parameter.type_reference);
        }
        visit_type_reference(program, retained, state.return_type);
        for contract in program.state_contracts(state) {
            visit_proof_facts(program, retained, contract.facts);
        }
        for (handle, statement) in program
            .statement_table
            .iter_statements(state.statement_nodes)
        {
            retained.statement_handle(handle);
            visit_statement(program, retained, statement);
        }
    }
    retained.leave_lexical_scope();
}

fn visit_constant_evaluation_entries(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    owner: SymbolHandle,
) {
    for (_, recipe) in program.constant_recipes.iter() {
        for context in program
            .constant_expression_contexts
            .span_or_empty(recipe.contexts)
        {
            if context.owner == owner && context.evaluation_entry.is_valid() {
                visit_symbol(retained, context.evaluation_entry);
            }
        }
    }
}

fn visit_mathematical_type(
    program: &TypedTrees,
    retained: &mut impl TypedReferences,
    handle: MathematicalTypeHandle,
) {
    use crate::mathematical::MathematicalType;
    if !handle.is_valid() || !retained.mathematical_type(handle) {
        return;
    }
    match program.mathematical_type(handle) {
        MathematicalType::Ordinary(reference) => {
            visit_type_reference(program, retained, *reference)
        }
        MathematicalType::Arrow {
            domain, codomain, ..
        } => {
            visit_mathematical_type(program, retained, *domain);
            visit_mathematical_type(program, retained, *codomain);
        }
        MathematicalType::Application { callee, arguments } => {
            visit_mathematical_type(program, retained, *callee);
            for argument in program.expression_table.expression_handles(*arguments) {
                visit_expression(program, retained, *argument);
            }
        }
    }
}

#[cfg(test)]
mod conformance_occurrence_tests {
    use super::*;
    use crate::static_applications::StaticConformanceSelection;
    #[derive(Default)]
    struct Sink(Vec<SymbolHandle>);
    impl Extend<SymbolHandle> for Sink {
        fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, iter: T) {
            self.0.extend(iter);
        }
    }
    impl TypedReferences for Sink {}

    pub(crate) fn occurrence_call(
        program: &mut TypedTrees,
        owner: SymbolHandle,
    ) -> (ExpressionHandle, StaticMachineApplicationHandle) {
        let application = program.expression_table.retain_static_machine_application(
            Default::default(),
            Default::default(),
            &[],
        );
        let source = program
            .expression_table
            .static_machine_application_mut(application);
        source.lexical_owner = owner;
        source.conformance_selection = Some(Box::new(StaticConformanceSelection::default()));
        let expression =
            program
                .expression_table
                .insert(ExpressionNode::Call(TableCallExpression {
                    authored_call_selection: None,
                    authored_call_binding: None,
                    authored_machine_application: application,
                    requirement_application: Default::default(),
                    exact_coercion_target: Default::default(),
                    receiver: Default::default(),
                    target_symbol: Default::default(),
                    static_machine_parameter: Default::default(),
                    target: Default::default(),
                    static_requirement_dispatch: None,
                    machine_arguments: Box::new([]),
                    quotient_operation: None,
                    private_layout_operation: None,
                    installed_interrupt_operation: None,
                    layout_query: None,
                    native_code_layout: None,
                    build_selection: Default::default(),
                    arguments: Default::default(),
                    evidence_arguments: Box::new([]),
                    operational_acknowledgement: Default::default(),
                }));
        (expression, application)
    }

    #[test]
    fn conformance_census_excludes_removed_same_owner_expression() {
        let mut program = TypedTrees::default();
        let owner = SymbolHandle::from_parts(1, 1);
        let (removed, application) = occurrence_call(&mut program, owner);
        let mut machine = Machine {
            symbol: owner,
            ..Default::default()
        };
        program.push_machine_owned_data(
            &mut machine,
            crate::machine::OwnedData {
                initial_value: removed,
                ..Default::default()
            },
        );
        machine.owned_data = Default::default();
        program.push_machine(machine);
        assert!(
            program
                .expression_table
                .iter_expressions()
                .any(|(h, _)| h == removed)
        );
        assert!(
            !program
                .live_conformance_occurrence_applications()
                .contains(&application)
        );
    }

    #[test]
    fn conformance_census_keeps_uncalled_retained_body() {
        let mut program = TypedTrees::default();
        let owner = SymbolHandle::from_parts(1, 1);
        let (expression, application) = occurrence_call(&mut program, owner);
        let mut machine = Machine {
            symbol: owner,
            ..Default::default()
        };
        program.push_machine_owned_data(
            &mut machine,
            crate::machine::OwnedData {
                initial_value: expression,
                ..Default::default()
            },
        );
        program.push_machine(machine);
        assert_eq!(
            program.live_conformance_occurrence_applications(),
            vec![application]
        );
    }

    #[test]
    fn conformance_census_keeps_non_machine_declaration_expression() {
        let mut program = TypedTrees::default();
        let owner = SymbolHandle::from_parts(1, 1);
        let (expression, application) = occurrence_call(&mut program, owner);
        program.push_const_declaration(crate::constant::ConstDeclaration {
            symbol: owner,
            authored_initializer: expression,
            ..Default::default()
        });
        assert_eq!(
            program.live_conformance_occurrence_applications(),
            vec![application]
        );
    }

    #[test]
    fn cyclic_derived_occurrence_views_do_not_recurse_or_admit() {
        let mut program = TypedTrees::default();
        let a = program.expression_table.retain_static_machine_application(
            SymbolHandle::invalid(),
            Default::default(),
            &[],
        );
        let b = program.expression_table.retain_static_machine_application(
            SymbolHandle::invalid(),
            Default::default(),
            &[],
        );
        for (handle, next) in [(a, b), (b, a)] {
            program
                .expression_table
                .static_machine_application_mut(handle)
                .conformance_selection = Some(Box::new(StaticConformanceSelection {
                operand_ordinal: 0,
                application: Some(Box::new(crate::ClosedConformanceApplication {
                    callable_application: next,
                    ..Default::default()
                })),
            }));
        }
        let mut sink = Sink::default();
        visit_static_machine_bindings(&program, &mut sink, a);
        assert!(sink.0.is_empty());
        assert_eq!(
            program
                .expression_table
                .static_machine_applications()
                .count(),
            2
        );
    }
}
