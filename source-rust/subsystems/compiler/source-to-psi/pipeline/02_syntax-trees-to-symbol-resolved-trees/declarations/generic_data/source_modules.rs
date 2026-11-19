use crate::resolution::declarations::declaration_selection::ConstantSelection;
use source_text::SourceId;

pub(crate) fn module_path(selection: Option<&ConstantSelection>, source: SourceId) -> Option<String> {
    let symbols = selection?.header_symbols();
    let module = symbols.source_module(source);
    module.is_valid().then(|| symbols.display_path(module, "::"))
}
