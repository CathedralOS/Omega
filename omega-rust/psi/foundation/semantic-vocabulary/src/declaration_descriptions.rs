//! Declaration descriptions preserve the selected owner, application and
//! source revision across Build execution and retained package boundaries.
//! Their contents describe declarations and grant no permission to call or
//! read the storage those declarations describe.

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct ParameterDescription {
    pub requirement_identity: Vec<u8>,
    pub requirement_application: Vec<u8>,
    pub parameter_identity: Vec<u8>,
    pub qualified_type: Vec<u8>,
    pub declaration_position: u64,
    pub source_revision: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
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
