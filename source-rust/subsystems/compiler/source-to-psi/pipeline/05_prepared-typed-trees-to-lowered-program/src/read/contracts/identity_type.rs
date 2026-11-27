//! The explicit type of Identity retains the same nested declarations that
//! name resolution attached to the call. Its arguments select the carrier
//! of both subjects, even when two record applications share their head.
use super::*;
use crate::read::refuse_at;
use crate::source_state::data::TypeParameterKind;
use source_text::SourceSpan;
use symbols::SymbolKind;

impl Program<'_> {
    pub(super) fn check_identity_type_application(
        &self,
        id: MachineId,
        reference: TypeReferenceHandle,
        call: SourceSpan,
        head: SourceSpan,
        mut selections: Vec<(SourceSpan, SymbolHandle)>,
    ) -> Checked<()> {
        let unavailable = |reason| refuse_at(id, Unserved::Contract(reason));
        let typed = self.typed;
        selections.sort_unstable_by_key(|(source, _)| source.span.start);
        let mut selections = selections.into_iter();
        let mut previous_end = head.span.end;
        let mut first = true;
        let mut pending = vec![reference];
        while let Some(next) = pending.pop() {
            if !typed.contains_type_reference(next) {
                return Err(unavailable("Identity has an unavailable nested type"));
            }
            let symbol = match typed.type_reference_table.type_reference(next) {
                TypeReferenceNode::Named { symbol, .. } => *symbol,
                TypeReferenceNode::Generic {
                    base_symbol,
                    lifetime_arguments,
                    arguments,
                    ..
                } => {
                    let definition = typed
                        .unique_data_definition_by_symbol(*base_symbol)
                        .ok_or_else(|| unavailable("Identity needs a nominal data application"))?;
                    let parameters = typed.data_type_parameters(definition);
                    let arguments = typed
                        .type_reference_table
                        .type_reference_handles(*arguments);
                    if !lifetime_arguments.is_empty()
                        || !definition.lifetime_parameters.is_empty()
                        || arguments.is_empty()
                        || parameters.len() != arguments.len()
                        || parameters
                            .iter()
                            .any(|parameter| parameter.kind != TypeParameterKind::Type)
                    {
                        return Err(unavailable(
                            "Identity needs the complete declared type argument tuple",
                        ));
                    }
                    pending.extend(arguments.iter().rev().copied());
                    *base_symbol
                }
                _ => {
                    return Err(unavailable(
                        "Identity has an unsupported nested type argument",
                    ));
                }
            };
            if !typed.symbols.symbols().nodes().is_valid(symbol) {
                return Err(unavailable(
                    "Identity has an unavailable nested declaration",
                ));
            }
            if std::mem::take(&mut first) {
                continue;
            }
            if typed.symbols.get(symbol).kind == SymbolKind::TypeParameter {
                if !self.scope.iter().any(|(parameter, _)| *parameter == symbol) {
                    return Err(unavailable(
                        "Identity has a type parameter outside its written scope",
                    ));
                }
                continue;
            }
            let Some((source, selected)) = selections.next() else {
                return Err(unavailable("Identity is missing a nested type selection"));
            };
            if source.source_id != call.source_id
                || source.span.start < previous_end
                || source.span.start >= source.span.end
                || source.span.end > call.span.end
                || selected != symbol
            {
                return Err(unavailable("Identity changed a nested type selection"));
            }
            previous_end = source.span.end;
        }
        if selections.next().is_some() {
            return Err(unavailable("Identity has an extra nested type selection"));
        }
        Ok(())
    }
}
