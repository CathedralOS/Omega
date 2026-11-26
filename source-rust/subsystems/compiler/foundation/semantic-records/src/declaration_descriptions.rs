//! Declaration descriptions preserve the selected owner, application and
//! source revision across Build execution and retained package boundaries.
//! Their contents describe declarations and grant no permission to call or
//! read the storage those declarations describe.

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub struct ParameterDescription {
    pub requirement_identity: Vec<u8>,
    pub requirement_application: Vec<u8>,
    pub parameter_identity: Vec<u8>,
    pub qualified_type: Vec<u8>,
    pub declaration_position: u64,
    pub source_revision: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub struct FieldInfoDescription {
    pub member_identity: Vec<u8>,
    pub case_identity: Vec<u8>,
    pub owner_identity: Vec<u8>,
    pub owner_application: Vec<u8>,
    pub name: Vec<u8>,
    pub qualified_type: Vec<u8>,
    pub declaration_position: u64,
    pub has_stable_number: bool,
    pub is_erased: bool,
    pub stable_number: u64,
    pub source_revision: Vec<u8>,
}

/// One Build's selection of a function pointer field and the requirement
/// formal that lends the table containing it.
#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub struct FunctionFieldBinding {
    pub table: ParameterDescription,
    pub field: FieldInfoDescription,
}

impl FunctionFieldBinding {
    pub fn identity_digest(&self) -> [u8; 32] {
        let mut digest =
            wire::digest_encoder::DomainDigestEncoder::new(b"omega-function-field-binding-v1\0");
        let table = &self.table;
        digest.bytes(&table.requirement_identity);
        digest.bytes(&table.requirement_application);
        digest.bytes(&table.parameter_identity);
        digest.bytes(&table.qualified_type);
        digest.u64(table.declaration_position);
        digest.bytes(&table.source_revision);
        let field = &self.field;
        digest.bytes(&field.member_identity);
        digest.bytes(&field.case_identity);
        digest.bytes(&field.owner_identity);
        digest.bytes(&field.owner_application);
        digest.bytes(&field.name);
        digest.bytes(&field.qualified_type);
        digest.u64(field.declaration_position);
        digest.byte(u8::from(field.has_stable_number));
        digest.byte(u8::from(field.is_erased));
        digest.u64(field.stable_number);
        digest.bytes(&field.source_revision);
        digest.finish()
    }
}
