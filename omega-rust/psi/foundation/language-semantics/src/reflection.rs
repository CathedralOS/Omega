//! Target-neutral semantic schema descriptions shared by source queries and
//! policy normalization. These are descriptive data, never access authority.
//! Typed construction retains source ownership in the frontend.

/// The existing CallableInfo query result: requirement versus machine, then
/// complete declaration identity, applied callable identity and source revision.
/// Keeping all three payloads preserves its meaning without granting invocation.
pub type CallableDescription = (bool, [Vec<u8>; 3]);

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

    fn image_word(bytes: &[u8], slot: usize) -> Result<u64, String> {
        let start = slot
            .checked_mul(8)
            .ok_or("schema image word offset overflows")?;
        let end = start
            .checked_add(8)
            .ok_or("schema image word extent overflows")?;
        let value = bytes
            .get(start..end)
            .ok_or("schema image lost a complete word")?;
        Ok(u64::from_le_bytes(
            value.try_into().expect("eight-byte word"),
        ))
    }

    fn image_span(bytes: &[u8], slot: usize) -> Result<&[u8], String> {
        let start = usize::try_from(image_word(bytes, slot)?)
            .map_err(|_| "schema image span offset exceeds this host")?;
        let length = usize::try_from(image_word(bytes, slot + 1)?)
            .map_err(|_| "schema image span length exceeds this host")?;
        let end = start
            .checked_add(length)
            .ok_or("schema image span overflows")?;
        bytes
            .get(start..end)
            .ok_or_else(|| "schema image span lies outside its owner".into())
    }

    fn image_text(bytes: &[u8], slot: usize) -> Result<String, String> {
        std::str::from_utf8(image_span(bytes, slot)?)
            .map(str::to_owned)
            .map_err(|error| format!("schema image text is invalid: {error}"))
    }

    fn image_numbers(bytes: &[u8], slot: usize) -> Result<Vec<u64>, String> {
        let values = image_span(bytes, slot)?;
        if !values.len().is_multiple_of(8) {
            return Err("schema image number sequence has a partial word".into());
        }
        values
            .as_chunks::<8>()
            .0
            .iter()
            .map(|word| image_word(word, 0))
            .collect()
    }

    fn image_handles(bytes: &[u8], slot: usize) -> Result<Vec<super::SchemaNodeHandle>, String> {
        image_numbers(bytes, slot)?
            .into_iter()
            .map(|value| {
                u32::try_from(value)
                    .map(super::SchemaNodeHandle::from_index)
                    .map_err(|_| "schema image node coordinate exceeds its representation".into())
            })
            .collect()
    }

    fn image_strings(bytes: &[u8], slot: usize) -> Result<Vec<String>, String> {
        let mut remaining = image_span(bytes, slot)?;
        let mut values = Vec::new();
        while !remaining.is_empty() {
            let length = usize::try_from(image_word(remaining, 0)?)
                .map_err(|_| "schema image text length exceeds this host")?;
            let end = 8usize
                .checked_add(length)
                .ok_or("schema image text extent overflows")?;
            let text = remaining
                .get(8..end)
                .ok_or("schema image lost its complete text")?;
            values.push(
                std::str::from_utf8(text)
                    .map(str::to_owned)
                    .map_err(|error| format!("schema image text is invalid: {error}"))?,
            );
            remaining = &remaining[end..];
        }
        Ok(values)
    }

    fn image_boolean(bytes: &[u8], slot: usize) -> Result<bool, String> {
        match image_word(bytes, slot)? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err("schema image Boolean is not canonical".into()),
        }
    }

    fn image_node(bytes: &[u8]) -> Result<SchemaNode, String> {
        use super::{
            CaseDescription, DeclarationDescription, FieldDescription, NominalReferenceDescription,
            SchemaNodeHandle, TypeParameterDescription,
        };
        if bytes.len() < NODE_WORDS * 8 {
            return Err("schema image lost a complete node".into());
        }
        let optional = |text: String| (!text.is_empty()).then_some(text);
        let position = || -> Result<u32, String> {
            u32::try_from(image_word(bytes, 1)?)
                .map_err(|_| "schema image declaration position exceeds its representation".into())
        };
        let stable = || {
            if image_boolean(bytes, 2)? {
                image_word(bytes, 3).map(Some)
            } else {
                Ok(None)
            }
        };
        Ok(match image_word(bytes, 0)? {
            0 => SchemaNode::Invalid,
            1 => {
                let parameters = image_strings(bytes, 30)?;
                if !parameters.len().is_multiple_of(4) {
                    return Err("schema image lost a complete type parameter".into());
                }
                let parameters = parameters
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|parts| {
                        let kind = match parts[1].as_str() {
                            "type" => "type",
                            "const" => "const",
                            "value" => "value",
                            "machine" => "machine",
                            "implementation" => "implementation",
                            "proposition" => "proposition",
                            _ => {
                                return Err("schema image has an unknown parameter kind".to_owned());
                            }
                        };
                        Ok(TypeParameterDescription {
                            name: parts[0].clone(),
                            kind,
                            type_identity: optional(parts[2].clone()),
                            bounds: parts[3].clone(),
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                SchemaNode::Declaration(DeclarationDescription {
                    owner_identity: image_text(bytes, 8)?,
                    identity_is_hermetic: image_boolean(bytes, 7)?,
                    name: image_text(bytes, 10)?,
                    shape: match image_word(bytes, 6)? {
                        0 => SchemaShape::Empty,
                        1 => SchemaShape::Record,
                        2 => SchemaShape::Sum,
                        3 => SchemaShape::Mixed,
                        _ => return Err("schema image has an unknown declaration shape".into()),
                    },
                    supply: image_text(bytes, 18)?,
                    multiplicity: image_text(bytes, 20)?,
                    carry: optional(image_text(bytes, 22)?),
                    type_parameters: parameters,
                    lifetime_parameters: image_strings(bytes, 32)?,
                    application_identity: optional(image_text(bytes, 16)?),
                    members: image_handles(bytes, 24)?,
                    retired_identities: image_numbers(bytes, 26)?,
                })
            }
            2 => SchemaNode::Field(FieldDescription {
                member_identity: image_text(bytes, 8)?,
                name: image_text(bytes, 10)?,
                declaration_position: position()?,
                stable_number: stable()?,
                erased: image_boolean(bytes, 4)?,
                qualified_type: image_text(bytes, 12)?,
                nominal_references: image_handles(bytes, 28)?,
                owner_case: SchemaNodeHandle::from_index(
                    u32::try_from(image_word(bytes, 5)?)
                        .map_err(|_| "schema image case coordinate exceeds its representation")?,
                ),
            }),
            3 => SchemaNode::Case(CaseDescription {
                member_identity: image_text(bytes, 8)?,
                name: image_text(bytes, 10)?,
                declaration_position: position()?,
                stable_number: stable()?,
                payload_fields: image_handles(bytes, 24)?,
                retired_payload_identities: image_numbers(bytes, 26)?,
            }),
            4 => SchemaNode::NominalReference(NominalReferenceDescription {
                owner_identity: image_text(bytes, 8)?,
                owner_kind: image_text(bytes, 14)?,
                name: image_text(bytes, 10)?,
                application_identity: image_text(bytes, 16)?,
            }),
            _ => return Err("schema image has an unknown node kind".into()),
        })
    }

    impl SemanticSchemaGraph {
        /// Decode the existing complete image as descriptive data. The source
        /// query/receipt owner authenticates where it came from; this operation
        /// neither reconstructs typed declarations nor grants query authority.
        pub fn from_runtime_image(bytes: &[u8]) -> Result<Self, String> {
            if image_word(bytes, 0)? != 1 {
                return Err("schema image has another version".into());
            }
            let count = usize::try_from(image_word(bytes, 1)?)
                .map_err(|_| "schema image node count exceeds this host")?;
            let available = bytes.len().saturating_sub(HEADER_WORDS * 8) / 16;
            if count == 0 || count > available {
                return Err("schema image node count exceeds its actual table".into());
            }
            let root = super::SchemaNodeHandle::from_index(
                u32::try_from(image_word(bytes, 2)?)
                    .map_err(|_| "schema image root exceeds its representation")?,
            );
            let requester_identity = image_text(bytes, 5)?;
            let authority = match image_word(bytes, 4)? {
                0 => SchemaQueryAuthority::OwningScope { requester_identity },
                1 => SchemaQueryAuthority::ForeignScope { requester_identity },
                _ => return Err("schema image has another query scope".into()),
            };
            let nodes = (0..count)
                .map(|index| image_node(image_span(bytes, HEADER_WORDS + index * 2)?))
                .collect::<Result<Vec<_>, String>>()?;
            let graph = Self::from_nodes(nodes, root, authority)?;
            // Compare every encoded field, the root and the entire image,
            // including canonical offsets and the original change fingerprint.
            if graph.revision != image_word(bytes, 3)? || graph.runtime_image() != bytes {
                return Err("schema image differs from its complete canonical graph".into());
            }
            Ok(graph)
        }

        pub fn runtime_image(&self) -> Vec<u8> {
            let mut bytes = vec![0; HEADER_WORDS * 8 + self.nodes.len() * 16];
            word(&mut bytes, 0, 1);
            word(&mut bytes, 1, self.nodes.len() as u64);
            word(&mut bytes, 2, self.root.index() as u64);
            word(&mut bytes, 3, self.revision);
            let (authority, requester) = match &self.authority {
                SchemaQueryAuthority::OwningScope { requester_identity } => (0, requester_identity),
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

        /// A coordinate in this schema's own node table. It describes data;
        /// callers still authenticate the complete owning schema before use.
        pub const fn from_index(index: u32) -> Self {
            Self(index)
        }

        pub const fn is_valid(self) -> bool {
            self.0 != 0
        }

        pub const fn index(self) -> usize {
            self.0 as usize
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum SchemaNode {
        Invalid,
        Declaration(DeclarationDescription),
        Field(FieldDescription),
        Case(CaseDescription),
        NominalReference(NominalReferenceDescription),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
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

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TypeParameterDescription {
        pub name: String,
        pub kind: &'static str,
        pub type_identity: Option<String>,
        pub bounds: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
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

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CaseDescription {
        pub member_identity: String,
        pub name: String,
        pub declaration_position: u32,
        pub stable_number: Option<u64>,
        pub payload_fields: Vec<SchemaNodeHandle>,
        pub retired_payload_identities: Vec<u64>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
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

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum SchemaQueryAuthority {
        OwningScope { requester_identity: String },
        ForeignScope { requester_identity: String },
    }
}

pub use schema_nodes::{
    CaseDescription, DeclarationDescription, FieldDescription, NominalReferenceDescription,
    SchemaNode, SchemaNodeHandle, SchemaQueryAuthority, SchemaShape, TypeParameterDescription,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticSchemaGraph {
    nodes: Vec<SchemaNode>,
    root: SchemaNodeHandle,
    authority: SchemaQueryAuthority,
    revision: u64,
}

impl SemanticSchemaGraph {
    /// Construction supplies declaration nodes; keeping the root check here
    /// lets all readers borrow the same immutable graph representation.
    pub fn from_nodes(
        nodes: Vec<SchemaNode>,
        root: SchemaNodeHandle,
        authority: SchemaQueryAuthority,
    ) -> Result<Self, String> {
        if !root.is_valid() || !matches!(nodes.get(root.index()), Some(SchemaNode::Declaration(_)))
        {
            return Err("semantic schema root is not its declaration node".into());
        }
        let revision = graph_revision(&authority, &nodes);
        Ok(Self {
            nodes,
            root,
            authority,
            revision,
        })
    }

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

    /// Borrow the fields owned by this subject, including its own case
    /// payloads. Detached fields and payloads of another case are not regions.
    pub fn governed_fields(&self) -> Result<Vec<(SchemaNodeHandle, &FieldDescription)>, String> {
        let mut fields = Vec::new();
        for member in &self.declaration().members {
            match self.node(*member) {
                SchemaNode::Field(field) if !field.owner_case.is_valid() => {
                    fields.push((*member, field))
                }
                SchemaNode::Case(case) => {
                    for payload in &case.payload_fields {
                        let SchemaNode::Field(field) = self.node(*payload) else {
                            return Err("schema case lost its declared payload field".into());
                        };
                        if field.owner_case != *member {
                            return Err("schema payload belongs to another case".into());
                        }
                        fields.push((*payload, field));
                    }
                }
                _ => return Err("schema root member has another owner or kind".into()),
            }
        }
        fields.sort_unstable_by_key(|(handle, _)| *handle);
        if fields.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err("schema repeats a governed field".into());
        }
        Ok(fields)
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
