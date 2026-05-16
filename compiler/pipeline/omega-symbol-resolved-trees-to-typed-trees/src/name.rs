use omega_symbol_resolved_trees as resolved;
use omega_typed_trees as typed;

pub(crate) fn lower_name(name: &resolved::name::DiagnosticName) -> typed::name::ProgramName {
    typed::name::ProgramName::generated(name.as_str())
}
