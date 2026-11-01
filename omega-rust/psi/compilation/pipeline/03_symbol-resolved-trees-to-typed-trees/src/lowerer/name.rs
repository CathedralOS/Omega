use resolved::name::DiagnosticName;
use symbol_resolved_trees as resolved;
use typed_trees as typed;

pub(crate) fn lower_name(name: &DiagnosticName) -> typed::name::Identifier {
    typed::name::Identifier::generated(name.as_str())
}

pub(crate) fn lower_names(names: &[DiagnosticName]) -> Vec<typed::name::Identifier> {
    names.iter().map(lower_name).collect()
}
