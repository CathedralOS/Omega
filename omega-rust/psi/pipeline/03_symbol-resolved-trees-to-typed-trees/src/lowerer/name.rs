use crate::typed_trees as typed;
use syntax_trees_to_symbol_resolved_trees::symbol_resolved_trees as resolved;

pub(crate) fn lower_name(name: &resolved::name::DiagnosticName) -> typed::name::Identifier {
    typed::name::Identifier::generated(name.as_str())
}
