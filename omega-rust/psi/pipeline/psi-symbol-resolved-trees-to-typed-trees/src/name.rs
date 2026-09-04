use psi_symbol_resolved_trees as resolved;
use psi_typed_trees as typed;

pub(crate) fn lower_name(name: &resolved::name::DiagnosticName) -> typed::name::Identifier {
    typed::name::Identifier::generated(name.as_str())
}
