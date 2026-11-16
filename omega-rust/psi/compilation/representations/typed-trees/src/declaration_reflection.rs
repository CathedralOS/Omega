/*
Queries describe selected types using the caller's visibility. Construction
stays here; language semantics owns the shared type descriptions. Consumers
keep the original query and its complete description. Descriptions alone grant
no permission to call a function or access storage.
*/

pub mod reflection {
    pub use language_semantics::reflection::*;

    #[doc(hidden)]
    pub mod construction {
        use crate::TypedTrees;
        use crate::data::{
            DataDefinition, DataField, DataMember, DataShapeKind, TypeParameter, TypeParameterKind,
        };
        use crate::reflection::{
            CaseDescription, DeclarationDescription, FieldDescription, NominalReferenceDescription,
            SchemaNode, SchemaNodeHandle, SchemaQueryAuthority, SchemaShape, SemanticSchemaGraph,
            TypeParameterDescription,
        };
        use crate::types::{TypeReferenceHandle, TypeReferenceNode};
        use symbols::{SymbolHandle, SymbolKind};

        /// Keep an authored declaration operand when constructing the actual
        /// call. Use the existing description role so it cannot become a
        /// runtime value or an execution of the described declaration.
        pub fn declaration_query_argument_role(
            typed: &TypedTrees,
            call: crate::references::StaticArgumentCall<'_>,
        ) -> crate::references::StaticArgumentRole {
            use language_core::core_vocabulary::{CoreVocabulary, ReflectionDeclaration};
            let target = match call {
                crate::references::StaticArgumentCall::Expression(_, call) => call.target_symbol,
                crate::references::StaticArgumentCall::Statement(call) => call.target_symbol,
            };
            let declaration = typed
                .machine_by_symbol(target)
                .or_else(|| typed.machine_holding_state(target));
            if declaration.is_some_and(|declaration| {
                matches!(
                    typed.core_vocabulary(declaration.symbol),
                    Some(CoreVocabulary::Reflection(
                        ReflectionDeclaration::Schema
                            | ReflectionDeclaration::Callable
                            | ReflectionDeclaration::Field
                            | ReflectionDeclaration::Parameter
                    ))
                )
            }) {
                crate::references::StaticArgumentRole::DeclarationDescription
            } else {
                crate::references::StaticArgumentRole::ValueDependency
            }
        }

        /// Describe the original query's author and visibility. Consumers of
        /// a retained schema use the same request, so another policy or helper
        /// cannot substitute a more privileged author when the value is read.
        pub fn schema_query_authority(
            typed: &TypedTrees,
            requester: SymbolHandle,
            subject: SymbolHandle,
        ) -> Result<SchemaQueryAuthority, String> {
            let requester_identity = typed.normalized_hermetic_symbol_identity(requester)?;
            Ok(
                if typed.symbols.same_symbol_source_package(requester, subject) {
                    SchemaQueryAuthority::OwningScope { requester_identity }
                } else {
                    SchemaQueryAuthority::ForeignScope { requester_identity }
                },
            )
        }

        pub fn exact_symbol_identity(
            typed: &TypedTrees,
            symbol: SymbolHandle,
        ) -> Result<(String, bool), String> {
            if !symbol.is_valid() {
                return Err("a schema identity requires a resolved declaration".to_owned());
            }
            if let Ok(identity) = typed.normalized_hermetic_symbol_identity(symbol) {
                return Ok((identity, true));
            }
            let path = typed.symbols.display_path(symbol, "::");
            if path.is_empty() {
                return Err("a schema identity requires a named declaration".to_owned());
            }
            Ok((path, false))
        }

        pub fn declaration_source_revision(
            typed: &TypedTrees,
            declaration: SymbolHandle,
        ) -> Result<Vec<u8>, String> {
            let span = typed
                .symbols
                .symbol_provenance_source_span(declaration)
                .ok_or("a declaration description requires its authored source custody")?;
            let source = typed
                .symbols
                .source_file(span)
                .ok_or("a declaration description requires its immutable source revision")?;
            source::source_identity::canonical_source_entry(source)
        }

        pub fn field_description_identity(
            typed: &TypedTrees,
            operand: &crate::expression::StaticMachineArgument,
            author: SymbolHandle,
        ) -> Result<semantic_vocabulary::FieldInfoDescription, String> {
            use crate::expression::StaticMachineArgument;
            use crate::reflection::SchemaNode;
            use crate::type_identity::{IndexIdentityArgument, portable_applied_index_identity};
            let symbols = typed.symbols.symbols().nodes();
            if !symbols.is_valid(author) {
                return Err(String::from("a field query lost its lexical owner"));
            }
            let mut selected = operand;
            let mut declaring_type = None;
            let mut members = Vec::new();
            while let Some(application) = &selected.application
                && application.member.is_valid()
            {
                if members.contains(&application.member) {
                    return Err(String::from("a field query has a cyclic member selection"));
                }
                members.push(application.member);
                if !symbols.is_valid(selected.symbol) {
                    return Err(String::from("a field query lost its supplying declaration"));
                }
                match typed.symbols.get(selected.symbol).kind {
                    SymbolKind::Data | SymbolKind::TypeAlias => {
                        if declaring_type.is_some() {
                            return Err(String::from(
                                "a field query has an unjoined enclosing type application",
                            ));
                        }
                        let mut head = selected.clone();
                        head.application
                            .as_mut()
                            .expect("selected member has an application")
                            .member = arena::Handle::invalid();
                        if !typed
                            .type_reference_table
                            .contains_type_reference(head.type_reference)
                            || !typed
                                .type_reference_table
                                .operand_lifetime_owner(head.type_reference)
                                .is_valid()
                        {
                            return Err(String::from(
                                "a field query lost its prepared supplying type and scope",
                            ));
                        }
                        typed.static_operand_region_supplier(&head, author)?;
                        declaring_type = Some(head.type_reference);
                    }
                    SymbolKind::Variant
                        if application.arguments.is_empty()
                            && application.lifetime_arguments.is_empty() => {}
                    _ => return Err(String::from("a field query has a non-type member supplier")),
                }
                selected = typed
                    .expression_table
                    .static_member(application.member)
                    .ok_or_else(|| String::from("a field query lost its selected member"))?;
            }
            if !symbols.is_valid(selected.symbol) {
                return Err(String::from("a field query lost its selected declaration"));
            }
            if typed.symbols.get(selected.symbol).kind != SymbolKind::Field {
                return Err(String::from(
                    "a field query requires its exact resolved field declaration",
                ));
            }
            if selected.const_literal.is_some() || selected.evidence_projection.is_some() {
                return Err(String::from(
                    "a field query cannot describe a value or evidence projection",
                ));
            }
            if selected.application.is_some() {
                return Err(String::from(
                    "a field query needs its resolved owner application",
                ));
            }
            if selected.type_reference.is_valid() {
                return Err(String::from(
                    "a field query has an unjoined supplying type reference",
                ));
            }
            let parent = typed.symbols.get(selected.symbol).parent;
            if !symbols.is_valid(parent) {
                return Err(String::from("a field query lost its declaring owner"));
            }
            let owner = if typed.symbols.get(parent).kind == SymbolKind::Variant {
                typed.symbols.get(parent).parent
            } else {
                parent
            };
            if !symbols.is_valid(owner) || typed.symbols.get(owner).kind != SymbolKind::Data {
                return Err(String::from("a field query has no data declaration owner"));
            }
            let data = match declaring_type {
                Some(reference) => {
                    if typed
                        .closed_data_constructor_field(reference, owner, selected.symbol)
                        .is_none()
                    {
                        return Err(String::from(
                            "a field query does not belong to its closed owner application",
                        ));
                    }
                    typed.closed_data_definition(reference).ok_or_else(|| {
                        String::from("a field query lost its closed declaring data")
                    })?
                }
                None => typed
                    .unique_data_definition_by_symbol(owner)
                    .ok_or_else(|| String::from("a field query has no unique declaring data"))?,
            };
            let mut occurrences = 0;
            for member in typed.data_members(data) {
                match member {
                    crate::data::DataMember::Field(field)
                        if parent == owner && field.symbol == selected.symbol =>
                    {
                        occurrences += 1
                    }
                    crate::data::DataMember::Variant(case) if case.symbol == parent => {
                        occurrences += typed
                            .data_payload_fields(case)
                            .iter()
                            .filter(|field| field.symbol == selected.symbol)
                            .count();
                    }
                    _ => {}
                }
            }
            if occurrences != 1 {
                return Err(String::from(
                    "a field query is not its declaring owner's exact field row",
                ));
            }
            if data.generic_instance.is_none()
                && (!typed.data_type_parameters(data).is_empty()
                    || !data.lifetime_parameters.is_empty())
            {
                return Err(String::from(
                    "a field query requires its complete owner application",
                ));
            }
            let author_span = typed
                .symbols
                .symbol_provenance_source_span(author)
                .ok_or_else(|| String::from("a field query lost its lexical author"))?;
            if !typed
                .symbols
                .source_reference_may_select_symbol(author_span, selected.symbol)
            {
                return Err(String::from(
                    "a field query is outside its author's declaration scope",
                ));
            }
            let authority = schema_query_authority(typed, author, owner)?;
            let graph = match declaring_type {
                Some(reference) => {
                    construct_semantic_schema_graph_for_type(typed, reference, authority)
                }
                None => construct_semantic_schema_graph(typed, owner, authority),
            }?;
            let member = exact_symbol_identity(typed, selected.symbol)?.0;
            let mut described = graph.nodes().iter().filter_map(|node| match node {
                SchemaNode::Field(field) if field.member_identity == member => Some(field),
                _ => None,
            });
            let field = described
                .next()
                .ok_or_else(|| String::from("field query lost its exact schema member"))?;
            if described.next().is_some() {
                return Err(String::from(
                    "field query has ambiguous schema correspondence",
                ));
            }
            let case_identity = if field.owner_case.is_valid() {
                let SchemaNode::Case(case) = graph.node(field.owner_case) else {
                    return Err(String::from("field query lost its exact case owner"));
                };
                case.member_identity.as_bytes().to_vec()
            } else {
                Vec::new()
            };
            let owner_application = match &graph.declaration().application_identity {
                Some(identity) => identity.clone(),
                None => portable_applied_index_identity(
                    typed,
                    IndexIdentityArgument::StaticArgument(&StaticMachineArgument {
                        symbol: owner,
                        ..Default::default()
                    }),
                    author,
                    &[],
                )
                .and_then(|identity| typed.type_identity_text(identity))
                .ok_or_else(|| String::from("field query lost its complete owner type identity"))?,
            };
            let revision = declaration_source_revision(typed, owner)?;
            Ok(semantic_vocabulary::FieldInfoDescription {
                member_identity: field.member_identity.as_bytes().to_vec(),
                case_identity,
                owner_identity: graph.declaration().owner_identity.as_bytes().to_vec(),
                owner_application: owner_application.into_bytes(),
                name: field.name.as_bytes().to_vec(),
                qualified_type: field.qualified_type.as_bytes().to_vec(),
                declaration_position: u64::from(field.declaration_position),
                has_stable_number: field.stable_number.is_some(),
                is_erased: field.erased,
                stable_number: field.stable_number.unwrap_or(0),
                source_revision: revision,
            })
        }

        pub fn parameter_description_identity(
            typed: &TypedTrees,
            formal: SymbolHandle,
            author: SymbolHandle,
        ) -> Result<semantic_vocabulary::ParameterDescription, String> {
            let symbols = typed.symbols.symbols().nodes();
            if !symbols.is_valid(formal) || !symbols.is_valid(author) {
                return Err("a parameter query lost its exact formal or lexical author".into());
            }
            let requirement = typed.symbols.get(formal).parent;
            let (owner, signature) = typed
                .trait_machine_signature(requirement)
                .filter(|(owner, _)| owner.is_boundary)
                .ok_or("a parameter query selects a boundary requirement's declared formal")?;
            if typed.closed_requirement_application(requirement).is_none()
                && (!typed.trait_type_parameters(owner).is_empty()
                    || !owner.lifetime_parameters.is_empty()
                    || !typed.state_signature_type_parameters(signature).is_empty())
            {
                return Err(
                    "a parameter query requires its complete requirement application".into(),
                );
            }
            let mut parameters = typed
                .state_signature_parameters(signature)
                .iter()
                .enumerate()
                .filter(|(_, parameter)| parameter.symbol == formal);
            let (position, parameter) = parameters
                .next()
                .ok_or("a parameter query is not its requirement's exact formal row")?;
            if parameters.next().is_some() || parameter.relevance.is_erased() {
                return Err("a parameter query requires one live semantic formal".into());
            }
            let author_span = typed
                .symbols
                .symbol_provenance_source_span(author)
                .ok_or("a parameter query lost its retained lexical source")?;
            if !typed
                .symbols
                .source_reference_may_select_symbol(author_span, requirement)
            {
                return Err("a parameter query is outside its author's declaration scope".into());
            }
            let qualified_type = crate::type_identity::descriptive_type_identity_in_region_scope(
                typed,
                crate::type_identity::TypeIdentityRequest::package_qualified(
                    parameter.type_reference,
                ),
                requirement,
                requirement,
                &[],
                &[],
            )
            .and_then(|identity| typed.type_identity_text(identity))
            .ok_or(
                "a parameter query lost its complete declared type and lifetime relationships",
            )?;
            let declaration = typed
                .closed_requirement_application(requirement)
                .map_or(requirement, |application| application.template);
            Ok(semantic_vocabulary::ParameterDescription {
                requirement_identity: typed
                    .normalized_hermetic_symbol_identity(declaration)?
                    .into_bytes(),
                requirement_application: typed
                    .trait_requirement_callable_identity_text(owner, signature)
                    .into_bytes(),
                parameter_identity: typed
                    .normalized_hermetic_symbol_identity(formal)?
                    .into_bytes(),
                qualified_type: qualified_type.into_bytes(),
                declaration_position: u64::try_from(position)
                    .map_err(|_| "a parameter position exceeds its description carrier")?,
                source_revision: declaration_source_revision(typed, declaration)?,
            })
        }

        /// Descriptions retain the selected declaration and all of its arguments
        /// without making an executable instance. The caller prepares arguments
        /// through their ordinary binding owner; these bytes supply no permission.
        pub fn callable_description_identity(
            typed: &TypedTrees,
            selected: &crate::expression::StaticMachineArgument,
            lexical_owner: SymbolHandle,
        ) -> Result<crate::reflection::CallableDescription, String> {
            if selected.const_literal.is_some() || selected.evidence_projection.is_some() {
                return Err("a callable description requires one declaration operand".into());
            }
            if selected.type_reference.is_valid()
                && (!typed.symbols.symbols().nodes().is_valid(selected.symbol)
                    || !matches!(
                        typed.symbols.get(selected.symbol).kind,
                        SymbolKind::Machine | SymbolKind::State
                    ))
            {
                return Err("a callable description requires one declaration operand".into());
            }
            let supplier = typed
                .static_operand_region_supplier(selected, lexical_owner)
                .map_err(str::to_owned)?;
            let (requirement, declaration, callable) = if let Some((owner, signature)) =
                typed.trait_machine_signature(selected.symbol)
            {
                if selected.application.is_some()
                    || (typed
                        .closed_requirement_application(selected.symbol)
                        .is_none()
                        && (!typed.trait_type_parameters(owner).is_empty()
                            || !owner.lifetime_parameters.is_empty()
                            || !typed.state_signature_type_parameters(signature).is_empty()
                            || !signature.lifetime_parameters.is_empty()))
                {
                    return Err(
                        "a callable description requires its complete requirement application"
                            .into(),
                    );
                }
                let declaration = typed
                    .closed_requirement_application(signature.symbol)
                    .map_or(signature.symbol, |application| application.template);
                (
                    true,
                    declaration,
                    typed.trait_requirement_callable_identity_text(owner, signature),
                )
            } else {
                let machine = typed
                    .machine_by_symbol(selected.symbol)
                    .or_else(|| typed.machine_holding_state(selected.symbol))
                    .ok_or(
                        "a callable description selects a concrete machine or trait requirement",
                    )?;
                let entry = typed
                    .machine_states(machine)
                    .first()
                    .ok_or("a callable description requires its declared entry")?;
                if selected.symbol != machine.symbol && selected.symbol != entry.symbol {
                    return Err("a callable description cannot select a continuation state".into());
                }
                if !selected.type_reference.is_valid()
                    || !typed
                        .type_reference_table
                        .contains_type_reference(selected.type_reference)
                {
                    return Err(
                        "a callable description requires its prepared declaration application"
                            .into(),
                    );
                }
                let declaration = match typed.type_reference_node(selected.type_reference) {
                    TypeReferenceNode::Named { symbol, .. } => *symbol,
                    TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
                    _ => return Err("a callable description lost its selected declaration".into()),
                };
                if declaration != machine.symbol {
                    return Err(
                        "a callable description has another declaration's application".into(),
                    );
                }
                let identity = crate::type_identity::portable_applied_index_identity(
                    typed,
                    crate::type_identity::IndexIdentityArgument::StaticArgument(selected),
                    supplier,
                    &[],
                )
                .and_then(|identity| typed.type_identity_text(identity))
                .ok_or("a callable description lost its exact callable application identity")?;
                (false, machine.symbol, identity)
            };
            Ok((
                requirement,
                [
                    typed
                        .normalized_hermetic_symbol_identity(declaration)?
                        .into_bytes(),
                    callable.into_bytes(),
                    declaration_source_revision(typed, declaration)?,
                ],
            ))
        }

        /// Match complete descriptive values to the existing declaration
        /// owner. This checks data correspondence, never permission to invoke.
        /// Scratch is sized to the actual requested descriptions, and each
        /// source declaration/query is visited once rather than per caller row.
        pub fn authenticate_callable_descriptions(
            typed: &TypedTrees,
            values: &[crate::reflection::CallableDescription],
        ) -> Result<(), String> {
            callable_description_sources(typed, values).map(|_| ())
        }

        /// Keep the selected operand behind each description when a later
        /// source consumer binds it to a module declaration. The same query
        /// checks every static argument and source revision; the description
        /// is never parsed back into an operand or treated as a call grant.
        pub fn callable_description_sources<'values>(
            typed: &TypedTrees,
            values: &'values [crate::reflection::CallableDescription],
        ) -> Result<
            Vec<(
                &'values crate::reflection::CallableDescription,
                crate::expression::StaticMachineArgument,
                SymbolHandle,
            )>,
            String,
        > {
            use crate::expression::{ExpressionNode, StaticMachineArgument};
            use crate::references::StaticArgumentCall;
            use language_core::core_vocabulary::{CoreVocabulary, ReflectionDeclaration};

            let mut requested: Vec<_> = values.iter().collect();
            requested.sort_unstable_by(|left, right| {
                left.1[0]
                    .cmp(&right.1[0])
                    .then_with(|| left.0.cmp(&right.0))
                    .then_with(|| left.1.cmp(&right.1))
            });
            requested.dedup();
            if requested.is_empty() {
                return Ok(Vec::new());
            }
            let mut matched = vec![None; requested.len()];
            let mut failures = vec![None; requested.len()];
            let mut consider = |selected: &StaticMachineArgument, author: SymbolHandle| {
                if !typed.symbols.symbols().nodes().is_valid(selected.symbol) {
                    return;
                }
                // This is only a cheap work-domain filter. The existing
                // canonical query below checks the complete kind/application.
                let declaration =
                    if let Some((_, signature)) = typed.trait_machine_signature(selected.symbol) {
                        typed
                            .closed_requirement_application(signature.symbol)
                            .map_or(signature.symbol, |application| application.template)
                    } else if let Some(machine) = typed
                        .machine_by_symbol(selected.symbol)
                        .or_else(|| typed.machine_holding_state(selected.symbol))
                    {
                        machine.symbol
                    } else {
                        return;
                    };
                let Ok(identity) = typed.normalized_hermetic_symbol_identity(declaration) else {
                    return;
                };
                let first =
                    requested.partition_point(|value| value.1[0].as_slice() < identity.as_bytes());
                let last =
                    requested.partition_point(|value| value.1[0].as_slice() <= identity.as_bytes());
                if first == last {
                    return;
                }
                match callable_description_identity(typed, selected, author) {
                    Ok(actual) => {
                        for index in first..last {
                            if &actual == requested[index] && matched[index].is_none() {
                                matched[index] = Some((selected.clone(), author));
                            }
                        }
                    }
                    Err(reason) => {
                        for failure in &mut failures[first..last] {
                            *failure = Some(reason.clone());
                        }
                    }
                }
            };
            // A real reflected application keeps its complete raw tuple and
            // lexical owner. No descriptor string is parsed back into types.
            for (_, node) in typed.expression_table.iter_expressions() {
                let ExpressionNode::Call(call) = node else {
                    continue;
                };
                let Some(declaration) = typed
                    .machine_by_symbol(call.target_symbol)
                    .or_else(|| typed.machine_holding_state(call.target_symbol))
                else {
                    continue;
                };
                if typed.core_vocabulary(declaration.symbol)
                    != Some(CoreVocabulary::Reflection(ReflectionDeclaration::Callable))
                {
                    continue;
                }
                if call.authored_machine_application.is_valid()
                    && !typed
                        .expression_table
                        .static_machine_application_is_valid(call.authored_machine_application)
                {
                    continue;
                }
                let [expression] = typed.expression_table.expression_handles(call.arguments) else {
                    continue;
                };
                let Ok(selected) = typed.declaration_operand_from_expression(*expression) else {
                    continue;
                };
                let author = StaticArgumentCall::Expression(
                    crate::expression::ExpressionHandle::invalid(),
                    call,
                )
                .lexical_owner(typed);
                if !typed.symbols.symbols().nodes().is_valid(author) {
                    continue;
                }
                let Some(span) = typed.symbols.symbol_provenance_source_span(author) else {
                    continue;
                };
                if !typed
                    .symbols
                    .source_reference_may_select_symbol(span, selected.symbol)
                {
                    continue;
                }
                consider(&selected, author);
            }
            // Ordinary closed declarations need no fabricated query to give
            // an identical descriptive value its existing canonical meaning.
            for machine in typed.machines() {
                consider(
                    &StaticMachineArgument {
                        symbol: machine.symbol,
                        ..Default::default()
                    },
                    machine.symbol,
                );
            }
            for (_, signature) in typed.trait_machine_signatures.iter() {
                consider(
                    &StaticMachineArgument {
                        symbol: signature.symbol,
                        ..Default::default()
                    },
                    signature.symbol,
                );
            }
            if let Some(index) = matched.iter().position(Option::is_none) {
                let reason = failures[index]
                    .as_deref()
                    .unwrap_or("no exact declaration/application/source revision matches");
                return Err(format!(
                    "callable description is not bound to the actual source: {reason}"
                ));
            }
            requested
                .into_iter()
                .zip(matched)
                .map(|(description, selected)| {
                    let (selected, author) =
                        selected.ok_or("callable source correspondence is incomplete")?;
                    Ok((description, selected, author))
                })
                .collect()
        }

        #[derive(Debug, Clone, PartialEq)]
        pub struct NominalExpectation {
            pub owner_identity: String,
            pub owner_kind: String,
            pub name: String,
            pub application_identity: String,
            pub is_self_edge: bool,
        }

        /// The exact type identity retained by a schema query, including its
        /// original region supplier. Consumers bind the published image to this
        /// selected application without rebuilding its graph from source.
        pub fn schema_type_identity(
            typed: &TypedTrees,
            reference: TypeReferenceHandle,
            declaration: SymbolHandle,
        ) -> Result<String, String> {
            if !typed
                .type_reference_table
                .contains_type_reference(reference)
            {
                return Err("a schema type lost its retained reference".into());
            }
            let retained = typed.type_reference_table.operand_lifetime_owner(reference);
            let owner = if retained.is_valid() {
                retained
            } else {
                declaration
            };
            crate::type_identity::descriptive_type_identity_in_region_scope(
                typed,
                crate::type_identity::TypeIdentityRequest::package_qualified(reference),
                owner,
                owner,
                &[],
                &[],
            )
            .and_then(|identity| typed.type_identity_text(identity))
            .filter(|identity| !identity.is_empty())
            .ok_or_else(|| "a schema type has no complete owned identity".into())
        }

        fn nominal_expectations(
            typed: &TypedTrees,
            type_reference: TypeReferenceHandle,
            argument_owner: SymbolHandle,
            root_owner_identity: &str,
            root_application_identity: Option<&str>,
        ) -> Result<Vec<NominalExpectation>, String> {
            let mut found = Vec::new();
            collect_nominal_expectations(
                typed,
                type_reference,
                argument_owner,
                root_owner_identity,
                root_application_identity,
                &[],
                &mut Vec::new(),
                &mut found,
            )?;
            Ok(found)
        }

        /// Signature roots retain the binders in scope at each type operand.
        /// Their canonical type identity describes the binders themselves;
        /// nominal graph edges name declarations outside that namespace.
        struct SignatureSchemaTypes<'typed> {
            typed: &'typed TypedTrees,
            roots: Vec<(TypeReferenceHandle, Vec<SymbolHandle>)>,
            scopes: Vec<Vec<SymbolHandle>>,
            pending_binders: Vec<SymbolHandle>,
            expressions: Vec<(crate::expression::ExpressionHandle, Vec<SymbolHandle>)>,
        }

        impl Extend<SymbolHandle> for SignatureSchemaTypes<'_> {
            fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {}
        }

        impl crate::references::TypedReferences for SignatureSchemaTypes<'_> {
            fn derived_declaration_references(&self) -> bool {
                false
            }

            fn callable_signature(&mut self, signature: &crate::signature::StateSignature) {
                self.pending_binders = self
                    .typed
                    .state_signature_type_parameters(signature)
                    .iter()
                    .map(|parameter| parameter.symbol)
                    .chain(
                        self.typed
                            .state_signature_parameters(signature)
                            .iter()
                            .map(|parameter| parameter.symbol),
                    )
                    .collect();
                self.pending_binders.push(signature.symbol);
                if let Some(children) = self.typed.symbols.child_handles(signature.symbol) {
                    self.pending_binders.extend(children);
                }
            }

            fn enter_lexical_scope(&mut self, _: SymbolHandle) {
                self.scopes.push(std::mem::take(&mut self.pending_binders));
            }

            fn leave_lexical_scope(&mut self) {
                self.scopes.pop();
            }

            fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
                let bound = self.scopes.iter().flatten().copied().collect();
                self.roots.push((reference, bound));
                false
            }

            fn expression(&mut self, expression: crate::expression::ExpressionHandle) -> bool {
                let context = self.scopes.iter().flatten().copied().collect();
                let occurrence = (expression, context);
                if self.expressions.contains(&occurrence) {
                    return false;
                }
                self.expressions.push(occurrence);
                true
            }
        }

        fn collect_nominal_expectations(
            typed: &TypedTrees,
            type_reference: TypeReferenceHandle,
            argument_owner: SymbolHandle,
            root_owner_identity: &str,
            root_application_identity: Option<&str>,
            bound: &[SymbolHandle],
            active_contracts: &mut Vec<crate::callable_values::RuntimeCallableContractHandle>,
            found: &mut Vec<NominalExpectation>,
        ) -> Result<(), String> {
            let mut intern = |symbol: SymbolHandle,
                              name: &str,
                              application: TypeReferenceHandle,
                              bare_named: bool|
             -> Result<(), String> {
                if bound.contains(&symbol) {
                    return Ok(());
                }
                let (owner_identity, _) =
                    exact_symbol_identity(typed, symbol).map_err(|message| {
                        format!(
                            "{message}: nominal reference `{name}` in `{}`",
                            typed.package_qualified_type_identity_text(application)
                        )
                    })?;
                let application_identity =
                    schema_type_identity(typed, application, argument_owner)?;
                let is_self_edge = owner_identity == root_owner_identity
                    && match root_application_identity {
                        Some(root_application) => application_identity == root_application,
                        None => bare_named,
                    };
                found.push(NominalExpectation {
                    owner_identity,
                    owner_kind: format!("{:?}", typed.symbols.get(symbol).kind),
                    name: name.to_owned(),
                    application_identity,
                    is_self_edge,
                });
                Ok(())
            };

            match typed.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Callable { contract } => {
                    if !typed.runtime_callable_contracts.is_valid(*contract) {
                        return Err("schema callable type names a stale contract".to_owned());
                    }
                    // Recursive signatures retain their exact row in the type
                    // identity; expand each dependency cycle only once.
                    if active_contracts.contains(contract) {
                        return Ok(());
                    }
                    let row = typed.runtime_callable_contracts.get(*contract);
                    if !typed.symbols.symbols().nodes().is_valid(row.lexical_owner)
                        || !typed
                            .symbols
                            .symbols()
                            .nodes()
                            .is_valid(row.signature.symbol)
                    {
                        return Err(
                            "schema callable contract has no declaration namespace".to_owned()
                        );
                    }
                    typed
                        .runtime_callable_contract_identity(*contract, &[])
                        .map_err(|error| {
                            format!("schema callable contract cannot be described: {error}")
                        })?;
                    active_contracts.push(*contract);
                    let mut inputs = SignatureSchemaTypes {
                        typed,
                        roots: Vec::new(),
                        scopes: vec![bound.to_vec()],
                        pending_binders: Vec::new(),
                        expressions: Vec::new(),
                    };
                    crate::references::visit_signature(typed, &mut inputs, &row.signature);
                    for (reference, own_bound) in inputs.roots {
                        // Own signature binders are masked separately; free
                        // applications keep the enclosing operand's owner.
                        collect_nominal_expectations(
                            typed,
                            reference,
                            argument_owner,
                            root_owner_identity,
                            root_application_identity,
                            &own_bound,
                            active_contracts,
                            found,
                        )?;
                    }
                    active_contracts.pop();
                }
                TypeReferenceNode::Named { symbol, name } => {
                    intern(*symbol, name.as_str(), type_reference, true)?;
                }
                TypeReferenceNode::Generic {
                    base_symbol,
                    base_name,
                    arguments,
                    ..
                } => {
                    intern(*base_symbol, base_name.as_str(), type_reference, false)?;
                    let parameters = typed.declaration_type_parameters(*base_symbol);
                    for (index, argument) in typed
                        .type_reference_table
                        .type_reference_handles(*arguments)
                        .iter()
                        .enumerate()
                    {
                        if parameters
                            .get(index)
                            .is_some_and(|parameter| parameter.kind.is_const_or_value())
                        {
                            continue;
                        }
                        collect_nominal_expectations(
                            typed,
                            *argument,
                            argument_owner,
                            root_owner_identity,
                            root_application_identity,
                            bound,
                            active_contracts,
                            found,
                        )?;
                    }
                }
                TypeReferenceNode::Constrained { base_type, .. } => {
                    collect_nominal_expectations(
                        typed,
                        *base_type,
                        argument_owner,
                        root_owner_identity,
                        root_application_identity,
                        bound,
                        active_contracts,
                        found,
                    )?;
                }
                TypeReferenceNode::Reference { referee, .. } => {
                    collect_nominal_expectations(
                        typed,
                        *referee,
                        argument_owner,
                        root_owner_identity,
                        root_application_identity,
                        bound,
                        active_contracts,
                        found,
                    )?;
                }
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => {
                    collect_nominal_expectations(
                        typed,
                        *element_type,
                        argument_owner,
                        root_owner_identity,
                        root_application_identity,
                        bound,
                        active_contracts,
                        found,
                    )?;
                }
                TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Unit => {}
            }
            Ok(())
        }

        pub fn describe_field(
            typed: &TypedTrees,
            field: &DataField,
            owner_identity: &str,
            declaration_position: u32,
            owner_case: SchemaNodeHandle,
            root_application_identity: Option<&str>,
        ) -> Result<(FieldDescription, Vec<NominalExpectation>), String> {
            let member_identity = if field.symbol.is_valid() {
                exact_symbol_identity(typed, field.symbol)?.0
            } else {
                format!("{owner_identity}::{}", field.name.as_str())
            };
            let expectations = nominal_expectations(
                typed,
                field.type_reference,
                field.symbol,
                owner_identity,
                root_application_identity,
            )?;
            Ok((
                FieldDescription {
                    member_identity,
                    name: field.name.as_str().to_owned(),
                    declaration_position,
                    stable_number: field.identity,
                    erased: field.relevance.is_erased(),
                    qualified_type: schema_type_identity(
                        typed,
                        field.type_reference,
                        field.symbol,
                    )?,
                    nominal_references: Vec::new(),
                    owner_case,
                },
                expectations,
            ))
        }

        pub fn describe_type_parameter(
            typed: &TypedTrees,
            parameter: &TypeParameter,
        ) -> TypeParameterDescription {
            let (kind, type_reference) = match &parameter.kind {
                TypeParameterKind::Type => ("type", None),
                TypeParameterKind::Const { type_reference } => ("const", Some(*type_reference)),
                TypeParameterKind::Value { type_reference } => ("value", Some(*type_reference)),
                TypeParameterKind::Machine { .. } => ("machine", None),
                TypeParameterKind::Implementation => ("implementation", None),
                TypeParameterKind::Proposition { .. } => ("proposition", None),
            };
            TypeParameterDescription {
                name: parameter.name.as_str().to_owned(),
                kind,
                type_identity: type_reference
                    .map(|reference| typed.package_qualified_type_identity_text(reference)),
                bounds: format!("{:?}", parameter.bounds),
            }
        }

        pub fn construct_semantic_schema_graph(
            typed: &TypedTrees,
            subject: SymbolHandle,
            authority: SchemaQueryAuthority,
        ) -> Result<SemanticSchemaGraph, String> {
            if !subject.is_valid() || typed.symbols.get(subject).kind != SymbolKind::Data {
                return Err(
        "a semantic schema query selects an exact data declaration, not another symbol kind"
            .to_owned(),
    );
            }
            let data = typed.data_definition_by_symbol(subject).ok_or_else(|| {
                format!(
                    "schema subject `{}` has no retained data definition",
                    typed.symbols.display_path(subject, "::")
                )
            })?;
            construct_data_schema_graph(typed, data, None, authority)
        }

        /// The selected reference keeps the actual arguments; its declaration
        /// alone cannot describe substituted fields or lifetime relationships.
        pub fn construct_semantic_schema_graph_for_type(
            typed: &TypedTrees,
            reference: TypeReferenceHandle,
            authority: SchemaQueryAuthority,
        ) -> Result<SemanticSchemaGraph, String> {
            if !typed
                .type_reference_table
                .contains_type_reference(reference)
            {
                return Err("a schema query lost its selected type reference".into());
            }
            let data = typed
                .closed_data_definition(reference)
                .ok_or("a schema query requires its exact retained data application")?;
            construct_data_schema_graph(typed, data, Some(reference), authority)
        }

        fn construct_data_schema_graph(
            typed: &TypedTrees,
            data: &DataDefinition,
            reference: Option<TypeReferenceHandle>,
            authority: SchemaQueryAuthority,
        ) -> Result<SemanticSchemaGraph, String> {
            let subject = data.symbol;
            if data.quotient.is_some() {
                return Err(format!(
                    "schema reflection cannot observe quotient `{}`: a quotient does not expose its representative outside its checked quotient contract",
                    data.name.as_str()
                ));
            }
            let (owner_identity, identity_is_hermetic) = exact_symbol_identity(typed, subject)?;
            if let SchemaQueryAuthority::ForeignScope { .. } = &authority {
                let visible = crate::visibility::declaration_visibility(typed, subject)
                    .is_some_and(|visibility| visibility.is_public());
                if !visible {
                    return Err(format!(
                        "schema subject `{}` is not public to a foreign package; a query may not hide inaccessible members while claiming complete coverage",
                        data.name.as_str()
                    ));
                }
            }
            let application_identity = reference
                .or(data.generic_instance)
                .map(|reference| schema_type_identity(typed, reference, data.symbol))
                .transpose()?;

            let mut nodes = vec![SchemaNode::Invalid, SchemaNode::Invalid];
            let root = SchemaNodeHandle::from_index(1);
            let mut interned: Vec<(String, String, SchemaNodeHandle)> = Vec::new();
            let mut member_handles = Vec::new();
            for (position, member) in typed.data_members(data).iter().enumerate() {
                match member {
                    DataMember::Field(field) => {
                        let handle = push_field_node(
                            typed,
                            field,
                            &owner_identity,
                            position as u32,
                            SchemaNodeHandle::INVALID,
                            application_identity.as_deref(),
                            root,
                            &mut interned,
                            &mut nodes,
                        )?;
                        member_handles.push(handle);
                    }
                    DataMember::Variant(variant) => {
                        let case_handle = nodes.len() as u32;
                        nodes.push(SchemaNode::Invalid);
                        let case_handle = SchemaNodeHandle::from_index(case_handle);
                        let mut payload_handles = Vec::new();
                        for (payload_position, payload_field) in
                            typed.data_payload_fields(variant).iter().enumerate()
                        {
                            payload_handles.push(push_field_node(
                                typed,
                                payload_field,
                                &owner_identity,
                                payload_position as u32,
                                case_handle,
                                application_identity.as_deref(),
                                root,
                                &mut interned,
                                &mut nodes,
                            )?);
                        }
                        let (member_identity, _) = exact_symbol_identity(typed, variant.symbol)
                            .unwrap_or_else(|_| {
                                (
                                    format!("{owner_identity}::{}", variant.name.as_str()),
                                    false,
                                )
                            });
                        nodes[case_handle.index()] = SchemaNode::Case(CaseDescription {
                            member_identity,
                            name: variant.name.as_str().to_owned(),
                            declaration_position: position as u32,
                            stable_number: variant.identity,
                            payload_fields: payload_handles,
                            retired_payload_identities: sorted(
                                variant.retired_payload_identities.clone(),
                            ),
                        });
                        member_handles.push(case_handle);
                    }
                }
            }

            nodes[root.index()] = SchemaNode::Declaration(DeclarationDescription {
                owner_identity,
                identity_is_hermetic,
                name: data.name.as_str().to_owned(),
                shape: match DataDefinition::shape_kind_from_members(typed.data_members(data)) {
                    DataShapeKind::Empty => SchemaShape::Empty,
                    DataShapeKind::Record => SchemaShape::Record,
                    DataShapeKind::Enum => SchemaShape::Sum,
                    DataShapeKind::Mixed => SchemaShape::Mixed,
                },
                supply: format!("{:?}", data.supply_mode),
                multiplicity: format!("{:?}", data.properties.multiplicity),
                carry: data.properties.carry.map(|carry| format!("{carry:?}")),
                type_parameters: typed
                    .data_type_parameters(data)
                    .iter()
                    .map(|parameter| describe_type_parameter(typed, parameter))
                    .collect(),
                lifetime_parameters: data
                    .lifetime_parameters
                    .iter()
                    .map(|name| name.as_str().to_owned())
                    .collect(),
                application_identity,
                members: member_handles,
                retired_identities: sorted(data.retired_identities.clone()),
            });
            SemanticSchemaGraph::from_nodes(nodes, root, authority)
        }

        #[allow(clippy::too_many_arguments)]
        fn push_field_node(
            typed: &TypedTrees,
            field: &DataField,
            owner_identity: &str,
            declaration_position: u32,
            owner_case: SchemaNodeHandle,
            root_application_identity: Option<&str>,
            root: SchemaNodeHandle,
            interned: &mut Vec<(String, String, SchemaNodeHandle)>,
            nodes: &mut Vec<SchemaNode>,
        ) -> Result<SchemaNodeHandle, String> {
            let (mut description, expectations) = describe_field(
                typed,
                field,
                owner_identity,
                declaration_position,
                owner_case,
                root_application_identity,
            )?;
            description.nominal_references = expectations
                .iter()
                .map(|expectation| {
                    if expectation.is_self_edge {
                        return Ok(root);
                    }
                    if let Some((_, _, handle)) = interned.iter().find(|(owner, application, _)| {
                        owner == &expectation.owner_identity
                            && application == &expectation.application_identity
                    }) {
                        return Ok(*handle);
                    }
                    let handle = SchemaNodeHandle::from_index(nodes.len() as u32);
                    nodes.push(SchemaNode::NominalReference(NominalReferenceDescription {
                        owner_identity: expectation.owner_identity.clone(),
                        owner_kind: expectation.owner_kind.clone(),
                        name: expectation.name.clone(),
                        application_identity: expectation.application_identity.clone(),
                    }));
                    interned.push((
                        expectation.owner_identity.clone(),
                        expectation.application_identity.clone(),
                        handle,
                    ));
                    Ok(handle)
                })
                .collect::<Result<_, String>>()?;
            let handle = SchemaNodeHandle::from_index(nodes.len() as u32);
            nodes.push(SchemaNode::Field(description));
            Ok(handle)
        }

        pub fn sorted(mut identities: Vec<u64>) -> Vec<u64> {
            identities.sort_unstable();
            identities
        }
    }

    pub use construction::construct_semantic_schema_graph;

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::TypedTrees;
        use crate::data::{DataDefinition, DataField, DataMember, TypeParameter};
        use crate::name::Identifier;
        use crate::types::{TypeReferenceHandle, TypeReferenceNode};
        use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};

        fn named(
            program: &mut TypedTrees,
            symbol: SymbolHandle,
            name: &str,
        ) -> TypeReferenceHandle {
            program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol,
                    name: Identifier::generated(name),
                })
        }

        #[test]
        fn schema_construction_retains_selected_application_and_nominal_edges() {
            let mut symbols = SymbolTableBuilder::default();
            let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static(""));
            let declarations = symbols.insert_children(
                root,
                [
                    (SymbolKind::Data, SymbolNameRef::Static("Cell")),
                    (SymbolKind::Data, SymbolNameRef::Static("Left")),
                    (SymbolKind::Data, SymbolNameRef::Static("Right")),
                ],
            );
            let mut declarations = declarations.handles();
            let cell = declarations.next().unwrap();
            let left = declarations.next().unwrap();
            let right = declarations.next().unwrap();
            let parameter = symbols
                .insert_children(
                    cell,
                    [(SymbolKind::TypeParameter, SymbolNameRef::Static("T"))],
                )
                .handles()
                .next()
                .unwrap();
            let mut program = TypedTrees {
                symbols: symbols.finish(),
                ..TypedTrees::default()
            };
            let parameters = program.data_type_parameters.insert_many([TypeParameter {
                symbol: parameter,
                name: Identifier::generated("T"),
                ..TypeParameter::default()
            }]);
            program.push_data_definition(DataDefinition {
                symbol: cell,
                name: Identifier::generated("Cell"),
                type_parameters: parameters,
                ..DataDefinition::default()
            });
            for (symbol, name) in [(left, "Left"), (right, "Right")] {
                program.push_data_definition(DataDefinition {
                    symbol,
                    name: Identifier::generated(name),
                    ..DataDefinition::default()
                });
            }
            let mut graphs = Vec::new();
            for (actual, name) in [(left, "CellLeft"), (right, "CellRight")] {
                let actual_name = program.symbols.name(actual).to_owned();
                let actual = named(&mut program, actual, &actual_name);
                let arguments = program
                    .type_reference_table
                    .insert_type_reference_handles([actual]);
                let application = program
                    .type_reference_table
                    .insert(TypeReferenceNode::Generic {
                        base_symbol: cell,
                        base_name: Identifier::generated("Cell"),
                        lifetime_arguments: Vec::new(),
                        arguments,
                    });
                let selected =
                    program
                        .symbols
                        .insert_generated_root_from(cell, SymbolKind::Data, name);
                let fields = program.symbols.insert_generated_children(
                    selected,
                    [(SymbolKind::Field, "first"), (SymbolKind::Field, "second")],
                );
                let members = program
                    .data_members
                    .insert_many(fields.handles().enumerate().map(|(position, symbol)| {
                        DataMember::Field(DataField {
                            symbol,
                            name: Identifier::generated(if position == 0 {
                                "first"
                            } else {
                                "second"
                            }),
                            identity: Some(position as u64 + 7),
                            type_reference: actual,
                            ..DataField::default()
                        })
                    }));
                program.push_data_definition(DataDefinition {
                    symbol: selected,
                    name: Identifier::generated("Cell"),
                    is_public: true,
                    generic_instance: Some(application),
                    members,
                    ..DataDefinition::default()
                });
                let graph = construct_semantic_schema_graph(
                    &program,
                    selected,
                    SchemaQueryAuthority::ForeignScope {
                        requester_identity: "consumer".to_owned(),
                    },
                )
                .unwrap();
                assert_eq!(
                    graph.declaration().application_identity.as_deref(),
                    Some(
                        program
                            .package_qualified_type_identity_text(application)
                            .as_str()
                    )
                );
                let fields: Vec<_> = graph.fields().map(|(_, field)| field).collect();
                assert_eq!(
                    fields
                        .iter()
                        .map(|field| field.name.as_str())
                        .collect::<Vec<_>>(),
                    ["first", "second"]
                );
                assert_eq!(fields[0].stable_number, Some(7));
                assert_eq!(fields[1].stable_number, Some(8));
                assert_eq!(fields[0].nominal_references, fields[1].nominal_references);
                let [reference] = fields[0].nominal_references.as_slice() else {
                    panic!("one nominal edge is required")
                };
                assert!(matches!(
                    graph.node(*reference),
                    SchemaNode::NominalReference(_)
                ));
                let image = graph.runtime_image();
                assert_eq!(
                    SemanticSchemaGraph::from_runtime_image(&image).unwrap(),
                    graph,
                    "the published image retains the entire selected application and nominal graph"
                );
                let mut wrong_root = image.clone();
                wrong_root[16..24].copy_from_slice(&0u64.to_le_bytes());
                assert!(
                    SemanticSchemaGraph::from_runtime_image(&wrong_root).is_err(),
                    "the graph fingerprint alone does not authenticate its root"
                );
                let mut overflow = image.clone();
                overflow[8..16].copy_from_slice(&u64::MAX.to_le_bytes());
                assert!(
                    SemanticSchemaGraph::from_runtime_image(&overflow).is_err(),
                    "scratch is bounded by the actual encoded node table"
                );
                let mut extra = image.clone();
                extra.push(0);
                assert!(
                    SemanticSchemaGraph::from_runtime_image(&extra).is_err(),
                    "all bytes belong to the same canonical image"
                );
                assert!(
                    SemanticSchemaGraph::from_runtime_image(&image[..image.len() - 1]).is_err()
                );
                graphs.push(graph);
            }
            assert_ne!(
                graphs[0].declaration().application_identity,
                graphs[1].declaration().application_identity
            );
            assert_ne!(graphs[0].revision(), graphs[1].revision());
            assert_ne!(graphs[0].runtime_image(), graphs[1].runtime_image());
            assert_eq!(graphs[0].runtime_image(), graphs[0].clone().runtime_image());
        }

        #[test]
        fn schema_construction_preserves_self_edge_and_visibility_refusal() {
            let mut symbols = SymbolTableBuilder::default();
            let recursive =
                symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Recursive"));
            let field = symbols
                .insert_children(
                    recursive,
                    [(SymbolKind::Field, SymbolNameRef::Static("next"))],
                )
                .handles()
                .next()
                .unwrap();
            let machine = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("run"));
            let mut program = TypedTrees {
                symbols: symbols.finish(),
                ..TypedTrees::default()
            };
            let referent = named(&mut program, recursive, "Recursive");
            let reference = program
                .type_reference_table
                .insert(TypeReferenceNode::Reference {
                    referee: referent,
                    access: language_core::ReferenceAccess::Shared,
                    lifetime: None,
                });
            let members = program
                .data_members
                .insert_many([DataMember::Field(DataField {
                    symbol: field,
                    name: Identifier::generated("next"),
                    type_reference: reference,
                    ..DataField::default()
                })]);
            program.push_data_definition(DataDefinition {
                symbol: recursive,
                name: Identifier::generated("Recursive"),
                members,
                ..DataDefinition::default()
            });
            let owning = SchemaQueryAuthority::OwningScope {
                requester_identity: "owner".to_owned(),
            };
            let graph =
                construct_semantic_schema_graph(&program, recursive, owning.clone()).unwrap();
            assert_eq!(
                graph.fields().next().unwrap().1.nominal_references,
                [graph.root()]
            );
            assert_eq!(graph.nodes().len(), 3);
            assert!(
                graph
                    .fields()
                    .next()
                    .unwrap()
                    .1
                    .qualified_type
                    .contains("elided-region")
            );
            assert!(
                crate::type_identity::portable_applied_type_identity(
                    &program,
                    reference,
                    field,
                    &[],
                )
                .is_none(),
                "a declared description does not establish a portable reference region"
            );
            let foreign = SchemaQueryAuthority::ForeignScope {
                requester_identity: "consumer".to_owned(),
            };
            assert!(
                construct_semantic_schema_graph(&program, recursive, foreign)
                    .unwrap_err()
                    .contains("not public")
            );
            for invalid in [SymbolHandle::invalid(), machine] {
                assert!(
                    construct_semantic_schema_graph(&program, invalid, owning.clone())
                        .unwrap_err()
                        .contains("exact data declaration")
                );
            }
        }
    }
}
