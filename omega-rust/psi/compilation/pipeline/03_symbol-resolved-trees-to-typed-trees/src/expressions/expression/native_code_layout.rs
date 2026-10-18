//! Admit the core query's syntax without evaluating or invoking its selection.
//! The canonical counts-and-addresses contract makes native geometry late
//! artifact metadata; source typing retains only its ordinary record carrier.

use diagnostics::Diagnostic;
use symbol_resolved_trees as resolved;
use typed_trees as typed;

pub(crate) fn lower_request(
    program: Option<&resolved::SymbolResolvedTrees>,
    target: &mut typed::TypedTrees,
    exposure: Option<
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
    >,
    operands: &[resolved::expression::StaticMachineArgument],
    value_count: usize,
    evidence_count: usize,
) -> Result<typed::expression::NativeCodeLayoutRequest, Diagnostic> {
    if value_count != 0 || evidence_count != 0 || operands.len() != 1 {
        return Err(Diagnostic::error(
            "`code_layout` requires exactly one selected machine application and no value or evidence arguments",
        ));
    }
    let operand = super::lower_static_machine_argument(program, target, exposure, &operands[0])?;
    let valid = program.is_some_and(|program| {
        operand.symbol.is_valid()
            && matches!(
                program.symbols.get(operand.symbol).kind,
                symbols::SymbolKind::Machine
                    | symbols::SymbolKind::State
                    | symbols::SymbolKind::MachineParameter
            )
    });
    if !valid || operand.const_literal.is_some() || operand.evidence_projection.is_some() {
        return Err(Diagnostic::error(
            "`code_layout` requires one exact executable machine application; data and proof-only declarations have no native realization",
        ));
    }
    Ok(typed::expression::NativeCodeLayoutRequest {
        operand,
        discards_result: false,
    })
}

pub(crate) fn result_type(
    program: Option<&resolved::SymbolResolvedTrees>,
    target: &mut typed::TypedTrees,
) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
    let program = program.ok_or_else(|| {
        Diagnostic::error("native layout query requires its resolved core declarations")
    })?;
    let symbol = program
        .data_definitions
        .iter()
        .find(|data| {
            data.name.as_str() == "CodeLayout"
                && program.symbols.symbol_source_origin(data.symbol)
                    == Some(source::SourceOrigin::Toolchain)
                && program
                    .symbols
                    .symbol_source_span(data.symbol)
                    .and_then(|span| program.symbols.source_file(span))
                    .is_some_and(|file| file.path.ends_with("core/code_layout.omg"))
        })
        .map(|data| data.symbol)
        .ok_or_else(|| {
            Diagnostic::error("native layout query lost its canonical core result declaration")
        })?;
    Ok(target
        .type_reference_table
        .find_named_type_reference(symbol)
        .unwrap_or_else(|| {
            target
                .type_reference_table
                .insert(typed::types::TypeReferenceNode::Named {
                    symbol,
                    name: typed::name::Identifier::generated_static("CodeLayout"),
                })
        }))
}
