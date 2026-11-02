mod machine;
mod top_level;

use symbols::SymbolHandle;

pub use machine::MachineSymbols;
pub use top_level::TopLevelSymbols;
pub(crate) use top_level::{CallerSiteCaches, PrefixSiteEntry};

/// First retained index entry whose authored name matches, projected through
/// `value`. Entries keep authored order, and `build` already rejected a
/// duplicate spelling, so the first match is the only one.
fn named_value<Entry, Value>(
    entries: &[Entry],
    name: &str,
    key: impl Fn(&Entry) -> &str,
    value: impl FnOnce(&Entry) -> Value,
) -> Option<Value> {
    entries.iter().find(|entry| key(entry) == name).map(value)
}

/// The retained entry's own declaration symbol, or an invalid handle when no
/// authored declaration carries that name.
fn named_symbol<Entry>(
    entries: &[Entry],
    name: &str,
    key: impl Fn(&Entry) -> &str,
    symbol: impl FnOnce(&Entry) -> SymbolHandle,
) -> SymbolHandle {
    named_value(entries, name, key, symbol).unwrap_or_else(SymbolHandle::invalid)
}
