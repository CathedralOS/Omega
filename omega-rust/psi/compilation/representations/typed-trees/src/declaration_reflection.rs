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
                    ))
                )
            }) {
                crate::references::StaticArgumentRole::DeclarationDescription
            } else {
                crate::references::StaticArgumentRole::ValueDependency
            }
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
                            if &actual == requested[index] {
                                if matched[index].is_none() {
                                    matched[index] = Some((selected.clone(), author));
                                }
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
                &mut found,
            )?;
            Ok(found)
        }

        fn collect_nominal_expectations(
            typed: &TypedTrees,
            type_reference: TypeReferenceHandle,
            argument_owner: SymbolHandle,
            root_owner_identity: &str,
            root_application_identity: Option<&str>,
            found: &mut Vec<NominalExpectation>,
        ) -> Result<(), String> {
            let mut intern = |symbol: SymbolHandle,
                              name: &str,
                              application: TypeReferenceHandle,
                              bare_named: bool|
             -> Result<(), String> {
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

        pub fn resolve_subject<'program>(
            typed: &'program TypedTrees,
            owner_identity: &str,
        ) -> Result<&'program DataDefinition, String> {
            let mut matches = typed.data_definitions().iter().filter(|data| {
                exact_symbol_identity(typed, data.symbol)
                    .is_ok_and(|(identity, _)| identity == owner_identity)
            });
            let Some(data) = matches.next() else {
                return Err(format!(
                    "schema owner `{owner_identity}` does not resolve to a data declaration in this program"
                ));
            };
            if matches.next().is_some() {
                return Err(format!(
                    "schema owner `{owner_identity}` resolves to more than one data declaration"
                ));
            }
            Ok(data)
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
