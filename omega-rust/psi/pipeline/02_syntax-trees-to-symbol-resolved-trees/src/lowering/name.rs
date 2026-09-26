//! Identifier to diagnostic name.

use crate::symbol_resolved_trees::name::DiagnosticName;
use tokens_to_syntax_trees::syntax_trees as syntax;

pub(crate) fn lower_name(name: &syntax::identifier::Identifier) -> DiagnosticName {
    DiagnosticName::from_str(name.as_str(), name.source_span())
}
