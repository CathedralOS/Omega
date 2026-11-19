//! Type descriptions shared by source queries and access-rule validation.
//! The frontend uses the caller's visibility. Descriptions alone grant no
//! permission to call functions or access storage.

/// The existing CallableInfo query result: requirement versus machine, then
/// complete declaration identity, applied callable identity and source revision.
/// Keeping all three payloads preserves its meaning without granting invocation.
pub type CallableDescription = (bool, [Vec<u8>; 3]);

mod nodes;
mod revision;
mod runtime_image;

#[cfg(test)]
mod tests;

pub use nodes::{
    CaseDescription, DeclarationDescription, FieldDescription, NominalReferenceDescription,
    SchemaNode, SchemaNodeHandle, SchemaQueryAuthority, SchemaShape, TypeParameterDescription,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SemanticSchemaGraph {
    nodes: Vec<SchemaNode>,
    root: SchemaNodeHandle,
    authority: SchemaQueryAuthority,
    revision: u64,
}

// Compiler snapshots use the same complete image as ordinary schema values.
// Reusing its decoder preserves root and visibility checks without maintaining
// another serialization of the graph or its static parameter-kind labels.
impl record_storage::Record for SemanticSchemaGraph {
    fn write(
        &self,
        output: &mut record_storage::RecordWriter,
    ) -> Result<(), record_storage::RecordError> {
        record_storage::Record::write(&self.runtime_image(), output)
    }

    fn read<Context>(
        input: &mut record_storage::RecordReader<'_, Context>,
    ) -> Result<Self, record_storage::RecordError> {
        let image = <Vec<u8> as record_storage::Record>::read(input)?;
        Self::from_runtime_image(&image).map_err(|reason| input.error(reason))
    }
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
        for node in &nodes {
            let SchemaNode::Declaration(declaration) = node else {
                continue;
            };
            let SchemaShape::Primitive(atom) = declaration.shape else {
                continue;
            };
            let identity =
                application_identity::type_identity::compiler_type_name(&atom.identity());
            let application =
                application_identity::type_identity::compiler_type_identity(&atom.identity());
            if symbols::PrimitiveType::from_builtin_atom(atom).is_none() {
                return Err("primitive schema names a non-scalar carrier".into());
            }
            if declaration.owner_identity != identity
                || declaration.application_identity.as_deref() != Some(application.as_str())
                || !declaration.identity_is_hermetic
            {
                return Err(format!(
                    "primitive schema changes its exact scalar identity: owner {:?}, type {:?}; expected {:?} and {:?}",
                    declaration.owner_identity,
                    declaration.application_identity,
                    identity,
                    application,
                ));
            }
            if !declaration.members.is_empty()
                || !declaration.retired_identities.is_empty()
                || !declaration.type_parameters.is_empty()
                || !declaration.lifetime_parameters.is_empty()
                || !declaration.supply.is_empty()
                || declaration.multiplicity != "Copy"
                || declaration.carry.is_some()
            {
                return Err(
                    "primitive schema adds data members or changes scalar properties".into(),
                );
            }
        }
        let revision = revision::graph_revision(&authority, &nodes);
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
