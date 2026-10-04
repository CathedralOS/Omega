/// Canonical authored operand kind and order for one `FilesystemHost`
/// requirement. This is evaluator ABI schema, not provider behavior: even an
/// operand a modeled provider does not use must be prepared exactly once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::interpreter::evaluator) enum FilesystemHostOperandKind {
    PathBytes,
    Bytes,
    I32,
    U32,
    I64,
    U64,
    MutableBytes,
    MutableI64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::interpreter::evaluator) enum FilesystemHostResultKind {
    I32,
    I64,
}

// The enum, declaration-order transcript tags, canonical names, and exact
// operand/result schemas are generated from the one Omega boundary declaration.
include!(concat!(env!("OUT_DIR"), "/filesystem_host_operations.rs"));

impl FilesystemHostOperation {}

impl std::fmt::Display for FilesystemHostOperation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.canonical_name())
    }
}
