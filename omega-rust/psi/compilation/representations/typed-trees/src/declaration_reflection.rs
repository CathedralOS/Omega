/*
Source queries need descriptions of selected data declarations before checking
starts. Keep these descriptions and their encoding with typed declarations so
those queries can work without the old checker. Descriptions do not grant
permission; source-bound replay and authorization remain with their receiving
owners.
*/

pub mod reflection {
    #[doc(hidden)]
    pub mod fingerprint {
        pub const OFFSET_BASIS: u64 = semantic_vocabulary::Fnv1a::OFFSET_BASIS;

        pub fn byte(hash: &mut u64, value: u8) {
            *hash ^= u64::from(value);
            *hash = hash.wrapping_mul(semantic_vocabulary::Fnv1a::PRIME);
        }
        pub fn bytes(hash: &mut u64, value: &[u8]) {
            for value in value {
                byte(hash, *value);
            }
        }
        pub fn uint(hash: &mut u64, value: u64) {
            bytes(hash, &value.to_le_bytes());
        }
        pub fn text(hash: &mut u64, value: &str) {
            uint(hash, value.len() as u64);
            bytes(hash, value.as_bytes());
        }
        pub fn optional_text(hash: &mut u64, value: &Option<String>) {
            match value {
                Some(value) => {
                    byte(hash, 1);
                    text(hash, value);
                }
                None => byte(hash, 0),
            }
        }
        pub fn optional_uint(hash: &mut u64, value: Option<u64>) {
            match value {
                Some(value) => {
                    byte(hash, 1);
                    uint(hash, value);
                }
                None => byte(hash, 0),
            }
        }

        pub fn query_authority(hash: &mut u64, authority: &super::SchemaQueryAuthority) {
            match authority {
                super::SchemaQueryAuthority::OwningScope { requester_identity } => {
                    byte(hash, 0);
                    text(hash, requester_identity);
                }
                super::SchemaQueryAuthority::ForeignScope { requester_identity } => {
                    byte(hash, 1);
                    text(hash, requester_identity);
                }
            }
        }
    }

    #[doc(hidden)]
    pub mod construction {
        use crate::TypedTrees;
        use crate::data::{
            DataDefinition, DataField, DataMember, DataShapeKind, TypeParameter, TypeParameterKind,
        };
        use crate::reflection::graph_revision;
        use crate::reflection::{
            CaseDescription, DeclarationDescription, FieldDescription, NominalReferenceDescription,
            SchemaNode, SchemaNodeHandle, SchemaQueryAuthority, SchemaShape, SemanticSchemaGraph,
            TypeParameterDescription,
        };
        use crate::types::{TypeReferenceHandle, TypeReferenceNode};
        use symbols::{SymbolHandle, SymbolKind};

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

        #[derive(Debug, Clone, PartialEq)]
        pub struct NominalExpectation {
            pub owner_identity: String,
            pub owner_kind: String,
            pub name: String,
            pub application_identity: String,
            pub is_self_edge: bool,
        }

        fn nominal_expectations(
            typed: &TypedTrees,
            type_reference: TypeReferenceHandle,
            root_owner_identity: &str,
            root_application_identity: Option<&str>,
        ) -> Result<Vec<NominalExpectation>, String> {
            let mut found = Vec::new();
            collect_nominal_expectations(
                typed,
                type_reference,
                root_owner_identity,
                root_application_identity,
                &mut found,
            )?;
            Ok(found)
        }

        fn collect_nominal_expectations(
            typed: &TypedTrees,
            type_reference: TypeReferenceHandle,
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
                let application_identity = typed.package_qualified_type_identity_text(application);
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
                            root_owner_identity,
                            root_application_identity,
                            found,
                        )?;
                    }
                }
                TypeReferenceNode::DynamicTrait { symbol, name, .. } => {
                    intern(*symbol, name.as_str(), type_reference, false)?;
                }
                TypeReferenceNode::Constrained { base_type, .. } => {
                    collect_nominal_expectations(
                        typed,
                        *base_type,
                        root_owner_identity,
                        root_application_identity,
                        found,
                    )?;
                }
                TypeReferenceNode::Reference { referee, .. } => {
                    collect_nominal_expectations(
                        typed,
                        *referee,
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
                    qualified_type: typed
                        .package_qualified_type_identity_text(field.type_reference),
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
            let application_identity = data
                .generic_instance
                .map(|instance| typed.package_qualified_type_identity_text(instance));

            let mut nodes = vec![SchemaNode::Invalid, SchemaNode::Invalid];
            let root = SchemaNodeHandle(1);
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
                        let case_handle = SchemaNodeHandle(case_handle);
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
            let revision = graph_revision(&authority, &nodes);
            Ok(SemanticSchemaGraph {
                nodes,
                root,
                authority,
                revision,
            })
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
                    let handle = SchemaNodeHandle(nodes.len() as u32);
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
            let handle = SchemaNodeHandle(nodes.len() as u32);
            nodes.push(SchemaNode::Field(description));
            Ok(handle)
        }

        pub fn sorted(mut identities: Vec<u64>) -> Vec<u64> {
            identities.sort_unstable();
            identities
        }
    }

    pub fn graph_revision(authority: &SchemaQueryAuthority, nodes: &[SchemaNode]) -> u64 {
        use crate::reflection::fingerprint::{
            byte, bytes, optional_text, optional_uint, query_authority, text, uint,
        };
        fn handle(hash: &mut u64, value: SchemaNodeHandle) {
            uint(hash, value.0 as u64);
        }
        fn handles(hash: &mut u64, values: &[SchemaNodeHandle]) {
            uint(hash, values.len() as u64);
            for value in values {
                handle(hash, *value);
            }
        }
        fn identities(hash: &mut u64, values: &[u64]) {
            uint(hash, values.len() as u64);
            for value in values {
                uint(hash, *value);
            }
        }

        let mut hash = crate::reflection::fingerprint::OFFSET_BASIS;
        bytes(&mut hash, b"omega.reflect.schema.v1");
        query_authority(&mut hash, authority);
        uint(&mut hash, nodes.len() as u64);
        for node in nodes {
            match node {
                SchemaNode::Invalid => byte(&mut hash, 0),
                SchemaNode::Declaration(declaration) => {
                    byte(&mut hash, 1);
                    text(&mut hash, &declaration.owner_identity);
                    byte(&mut hash, declaration.identity_is_hermetic as u8);
                    text(&mut hash, &declaration.name);
                    byte(
                        &mut hash,
                        match declaration.shape {
                            SchemaShape::Empty => 0,
                            SchemaShape::Record => 1,
                            SchemaShape::Sum => 2,
                            SchemaShape::Mixed => 3,
                        },
                    );
                    text(&mut hash, &declaration.supply);
                    text(&mut hash, &declaration.multiplicity);
                    optional_text(&mut hash, &declaration.carry);
                    uint(&mut hash, declaration.type_parameters.len() as u64);
                    for parameter in &declaration.type_parameters {
                        text(&mut hash, parameter.name.as_str());
                        text(&mut hash, parameter.kind);
                        optional_text(&mut hash, &parameter.type_identity);
                        text(&mut hash, &parameter.bounds);
                    }
                    uint(&mut hash, declaration.lifetime_parameters.len() as u64);
                    for name in &declaration.lifetime_parameters {
                        text(&mut hash, name);
                    }
                    optional_text(&mut hash, &declaration.application_identity);
                    handles(&mut hash, &declaration.members);
                    identities(&mut hash, &declaration.retired_identities);
                }
                SchemaNode::Field(field) => {
                    byte(&mut hash, 2);
                    text(&mut hash, &field.member_identity);
                    text(&mut hash, &field.name);
                    uint(&mut hash, u64::from(field.declaration_position));
                    optional_uint(&mut hash, field.stable_number);
                    byte(&mut hash, field.erased as u8);
                    text(&mut hash, &field.qualified_type);
                    handles(&mut hash, &field.nominal_references);
                    handle(&mut hash, field.owner_case);
                }
                SchemaNode::Case(case) => {
                    byte(&mut hash, 3);
                    text(&mut hash, &case.member_identity);
                    text(&mut hash, &case.name);
                    uint(&mut hash, u64::from(case.declaration_position));
                    optional_uint(&mut hash, case.stable_number);
                    handles(&mut hash, &case.payload_fields);
                    identities(&mut hash, &case.retired_payload_identities);
                }
                SchemaNode::NominalReference(reference) => {
                    byte(&mut hash, 4);
                    text(&mut hash, &reference.owner_identity);
                    text(&mut hash, &reference.owner_kind);
                    text(&mut hash, &reference.name);
                    text(&mut hash, &reference.application_identity);
                }
            }
        }
        if hash == 0 { 1 } else { hash }
    }

    pub(crate) mod runtime_image {
        use super::{SchemaNode, SchemaQueryAuthority, SchemaShape, SemanticSchemaGraph};

        const HEADER_WORDS: usize = 8;
        const NODE_WORDS: usize = 34;

        fn word(bytes: &mut [u8], slot: usize, value: u64) {
            bytes[slot * 8..slot * 8 + 8].copy_from_slice(&value.to_le_bytes());
        }

        fn span(bytes: &mut Vec<u8>, slot: usize, value: &[u8]) {
            let start = bytes.len() as u64;
            bytes.extend_from_slice(value);
            word(bytes, slot, start);
            word(bytes, slot + 1, value.len() as u64);
        }

        fn numbers(values: impl IntoIterator<Item = u64>) -> Vec<u8> {
            values.into_iter().flat_map(u64::to_le_bytes).collect()
        }

        fn strings(values: impl IntoIterator<Item = impl AsRef<str>>) -> Vec<u8> {
            let mut bytes = Vec::new();
            for value in values {
                let value = value.as_ref().as_bytes();
                bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
                bytes.extend_from_slice(value);
            }
            bytes
        }

        fn record(node: &SchemaNode) -> Vec<u8> {
            let mut bytes = vec![0; NODE_WORDS * 8];
            match node {
                SchemaNode::Invalid => {}
                SchemaNode::Declaration(value) => {
                    word(&mut bytes, 0, 1);
                    word(
                        &mut bytes,
                        6,
                        match value.shape {
                            SchemaShape::Empty => 0,
                            SchemaShape::Record => 1,
                            SchemaShape::Sum => 2,
                            SchemaShape::Mixed => 3,
                        },
                    );
                    word(&mut bytes, 7, u64::from(value.identity_is_hermetic));
                    span(&mut bytes, 8, value.owner_identity.as_bytes());
                    span(&mut bytes, 10, value.name.as_bytes());
                    span(
                        &mut bytes,
                        16,
                        value
                            .application_identity
                            .as_deref()
                            .unwrap_or("")
                            .as_bytes(),
                    );
                    span(&mut bytes, 18, value.supply.as_bytes());
                    span(&mut bytes, 20, value.multiplicity.as_bytes());
                    span(
                        &mut bytes,
                        22,
                        value.carry.as_deref().unwrap_or("").as_bytes(),
                    );
                    span(
                        &mut bytes,
                        24,
                        &numbers(value.members.iter().map(|h| h.index() as u64)),
                    );
                    span(
                        &mut bytes,
                        26,
                        &numbers(value.retired_identities.iter().copied()),
                    );
                    let mut parameters = Vec::new();
                    for parameter in &value.type_parameters {
                        parameters.extend(strings([
                            parameter.name.as_str(),
                            parameter.kind,
                            parameter.type_identity.as_deref().unwrap_or(""),
                            parameter.bounds.as_str(),
                        ]));
                    }
                    span(&mut bytes, 30, &parameters);
                    span(&mut bytes, 32, &strings(&value.lifetime_parameters));
                }
                SchemaNode::Field(value) => {
                    word(&mut bytes, 0, 2);
                    word(&mut bytes, 1, u64::from(value.declaration_position));
                    word(&mut bytes, 2, u64::from(value.stable_number.is_some()));
                    word(&mut bytes, 3, value.stable_number.unwrap_or(0));
                    word(&mut bytes, 4, u64::from(value.erased));
                    word(&mut bytes, 5, value.owner_case.index() as u64);
                    span(&mut bytes, 8, value.member_identity.as_bytes());
                    span(&mut bytes, 10, value.name.as_bytes());
                    span(&mut bytes, 12, value.qualified_type.as_bytes());
                    span(
                        &mut bytes,
                        28,
                        &numbers(value.nominal_references.iter().map(|h| h.index() as u64)),
                    );
                }
                SchemaNode::Case(value) => {
                    word(&mut bytes, 0, 3);
                    word(&mut bytes, 1, u64::from(value.declaration_position));
                    word(&mut bytes, 2, u64::from(value.stable_number.is_some()));
                    word(&mut bytes, 3, value.stable_number.unwrap_or(0));
                    span(&mut bytes, 8, value.member_identity.as_bytes());
                    span(&mut bytes, 10, value.name.as_bytes());
                    span(
                        &mut bytes,
                        24,
                        &numbers(value.payload_fields.iter().map(|h| h.index() as u64)),
                    );
                    span(
                        &mut bytes,
                        26,
                        &numbers(value.retired_payload_identities.iter().copied()),
                    );
                }
                SchemaNode::NominalReference(value) => {
                    word(&mut bytes, 0, 4);
                    span(&mut bytes, 8, value.owner_identity.as_bytes());
                    span(&mut bytes, 10, value.name.as_bytes());
                    span(&mut bytes, 14, value.owner_kind.as_bytes());
                    span(&mut bytes, 16, value.application_identity.as_bytes());
                }
            }
            bytes
        }

        impl SemanticSchemaGraph {
            pub fn runtime_image(&self) -> Vec<u8> {
                let mut bytes = vec![0; HEADER_WORDS * 8 + self.nodes.len() * 16];
                word(&mut bytes, 0, 1);
                word(&mut bytes, 1, self.nodes.len() as u64);
                word(&mut bytes, 2, self.root.index() as u64);
                word(&mut bytes, 3, self.revision);
                let (authority, requester) = match &self.authority {
                    SchemaQueryAuthority::OwningScope { requester_identity } => {
                        (0, requester_identity)
                    }
                    SchemaQueryAuthority::ForeignScope { requester_identity } => {
                        (1, requester_identity)
                    }
                };
                word(&mut bytes, 4, authority);
                span(&mut bytes, 5, requester.as_bytes());
                for (index, node) in self.nodes.iter().enumerate() {
                    span(&mut bytes, HEADER_WORDS + index * 2, &record(node));
                }
                bytes
            }
        }
    }

    pub(crate) mod schema_nodes {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct SchemaNodeHandle(pub(crate) u32);

        impl SchemaNodeHandle {
            pub const INVALID: Self = Self(0);

            pub const fn invalid() -> Self {
                Self::INVALID
            }

            pub const fn is_valid(self) -> bool {
                self.0 != 0
            }

            pub const fn index(self) -> usize {
                self.0 as usize
            }
        }

        #[derive(Debug, Clone, PartialEq)]
        pub enum SchemaNode {
            Invalid,
            Declaration(DeclarationDescription),
            Field(FieldDescription),
            Case(CaseDescription),
            NominalReference(NominalReferenceDescription),
        }

        #[derive(Debug, Clone, PartialEq)]
        pub struct DeclarationDescription {
            pub owner_identity: String,
            pub identity_is_hermetic: bool,
            pub name: String,
            pub shape: SchemaShape,
            pub supply: String,
            pub multiplicity: String,
            pub carry: Option<String>,
            pub type_parameters: Vec<TypeParameterDescription>,
            pub lifetime_parameters: Vec<String>,
            pub application_identity: Option<String>,
            pub members: Vec<SchemaNodeHandle>,
            pub retired_identities: Vec<u64>,
        }

        #[derive(Debug, Clone, PartialEq)]
        pub struct TypeParameterDescription {
            pub name: String,
            pub kind: &'static str,
            pub type_identity: Option<String>,
            pub bounds: String,
        }

        #[derive(Debug, Clone, PartialEq)]
        pub struct FieldDescription {
            pub member_identity: String,
            pub name: String,
            pub declaration_position: u32,
            pub stable_number: Option<u64>,
            pub erased: bool,
            pub qualified_type: String,
            pub nominal_references: Vec<SchemaNodeHandle>,
            pub owner_case: SchemaNodeHandle,
        }

        #[derive(Debug, Clone, PartialEq)]
        pub struct CaseDescription {
            pub member_identity: String,
            pub name: String,
            pub declaration_position: u32,
            pub stable_number: Option<u64>,
            pub payload_fields: Vec<SchemaNodeHandle>,
            pub retired_payload_identities: Vec<u64>,
        }

        #[derive(Debug, Clone, PartialEq)]
        pub struct NominalReferenceDescription {
            pub owner_identity: String,
            pub owner_kind: String,
            pub name: String,
            pub application_identity: String,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum SchemaShape {
            Empty,
            Record,
            Sum,
            Mixed,
        }

        #[derive(Debug, Clone, PartialEq)]
        pub enum SchemaQueryAuthority {
            OwningScope { requester_identity: String },
            ForeignScope { requester_identity: String },
        }
    }

    pub use construction::construct_semantic_schema_graph;
    pub use schema_nodes::{
        CaseDescription, DeclarationDescription, FieldDescription, NominalReferenceDescription,
        SchemaNode, SchemaNodeHandle, SchemaQueryAuthority, SchemaShape, TypeParameterDescription,
    };

    #[derive(Debug, Clone, PartialEq)]
    pub struct SemanticSchemaGraph {
        nodes: Vec<SchemaNode>,
        root: SchemaNodeHandle,
        authority: SchemaQueryAuthority,
        revision: u64,
    }

    impl SemanticSchemaGraph {
        pub fn nodes(&self) -> &[SchemaNode] {
            &self.nodes
        }

        pub fn root(&self) -> SchemaNodeHandle {
            self.root
        }

        pub fn authority(&self) -> &SchemaQueryAuthority {
            &self.authority
        }

        pub fn revision(&self) -> u64 {
            self.revision
        }

        pub fn node(&self, handle: SchemaNodeHandle) -> &SchemaNode {
            self.nodes
                .get(handle.index())
                .unwrap_or(&SchemaNode::Invalid)
        }

        pub fn declaration(&self) -> &DeclarationDescription {
            let SchemaNode::Declaration(declaration) = self.node(self.root) else {
                unreachable!("the schema root is always a declaration node");
            };
            declaration
        }

        pub fn fields(&self) -> impl Iterator<Item = (SchemaNodeHandle, &FieldDescription)> {
            self.declaration()
                .members
                .iter()
                .copied()
                .filter_map(|handle| match self.node(handle) {
                    SchemaNode::Field(field) => Some((handle, field)),
                    _ => None,
                })
        }

        pub fn cases(&self) -> impl Iterator<Item = (SchemaNodeHandle, &CaseDescription)> {
            self.declaration()
                .members
                .iter()
                .copied()
                .filter_map(|handle| match self.node(handle) {
                    SchemaNode::Case(case) => Some((handle, case)),
                    _ => None,
                })
        }
    }

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
