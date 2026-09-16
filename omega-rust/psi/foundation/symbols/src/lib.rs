#![forbid(unsafe_code)]

//! Target-neutral symbol identities, names, paths, and hierarchy storage.
//!
//! A `SymbolHandle` is the durable identity of one declaration; names and paths
//! are lookup and display metadata around it. `builtin` seeds the language's
//! builtin types and functions, and `table` owns the hierarchy arena that scoped
//! lookup walks. String names never stand in for identity after resolution.
//!
//! Start at `table.rs`; `symbol` holds the handle, name, path and kind types
//! it stores, and `builtin` the seeded builtin symbols.

mod builtin;
mod symbol;
mod table;

pub use builtin::{
    BUILTIN_TYPE_COUNT, BuiltinFunction, BuiltinType, BuiltinTypeAtom, builtin_function_symbols,
    builtin_type_member_symbols, builtin_type_symbols,
};
pub use symbol::kind::SymbolKind;
pub use symbol::name::{SymbolName, SymbolNameRef, SymbolNameStorageKind};
pub use symbol::path::SymbolPath;
pub use symbol::{Symbol, SymbolHandle, SymbolNameHandle, SymbolSpan};
pub use table::{
    SourceScopedTopLevelBinding, SymbolLookup, SymbolNameStorageCounts, SymbolTable,
    SymbolTableAppender, SymbolTableBuilder, SymbolTableExtension,
};

#[cfg(test)]
mod tests;
