//! Declaration identities, kinds and retained spellings. Resolution belongs to source-compiler.
#![forbid(unsafe_code)]
mod builtin;
mod fast_map;
mod primitive;
mod symbol;
pub use primitive::PrimitiveType;
pub use builtin::{BUILTIN_TYPE_COUNT, BuiltinFunction, BuiltinType, BuiltinTypeAtom, BuiltinTypeMember, FixedCoreDeclaration};
pub use fast_map::{BuildSymbolHasher, SymbolHasher, SymbolKeyMap, SymbolMap};
pub use symbol::kind::SymbolKind;
pub use symbol::name::{SymbolName, SymbolNameRef};
pub use symbol::path::SymbolPath;
pub use symbol::{Symbol, SymbolHandle, SymbolNameHandle, SymbolSpan};
/// One exact formal occurrence within its source callable's ordered telescope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParameterOccurrence { pub ordinal: usize, pub symbol: SymbolHandle }
