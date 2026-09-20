//! Intrinsic call targets and exact build receivers.

use checked_trees::CheckFacts;
use language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

pub(crate) fn checked_statement_call_intrinsic(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    call: &typed_trees::statement::TableCall,
) -> Option<AuthoredDeclarationSelectionIntrinsic> {
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic as Intrinsic;

    if call.target_symbol.is_valid() {
        return None;
    }
    if call.target.as_str() == "select_provider"
        && super::provider_selection::exact_mutable_build_statement_receiver(program, call)
    {
        return Some(Intrinsic::BuildProviderSelection);
    }
    if program.wire_encode_call_schema(call).is_some() {
        return Some(Intrinsic::WireEncode);
    }
    if program.wire_decode_call_schema(call).is_some() {
        return Some(Intrinsic::WireDecode);
    }
    if exact_statement_build_output_receiver(program, state, call) {
        return Some(Intrinsic::BuildIncludedSourceHandoff);
    }
    if exact_statement_build_log_receiver(program, state, call) {
        return Some(Intrinsic::BuildLogWriteLine);
    }
    if exact_statement_build_optimization_receiver(program, state, call) {
        return Some(Intrinsic::BuildOptimizationSelection);
    }
    if exact_statement_build_optimization_report_receiver(program, state, call) {
        return Some(Intrinsic::BuildOptimizationReportRequest);
    }
    checked_call_intrinsic(
        program,
        call.target.as_str(),
        call.target_symbol,
        typed_trees::expression::ExpressionHandle::invalid(),
    )
}

pub(crate) fn checked_intrinsic_call_target(
    facts: &CheckFacts,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<AuthoredDeclarationSelectionIntrinsic> {
    let mut matching = facts
        .intrinsic_calls
        .iter()
        .filter(|fact| fact.expression == expression)
        .map(|fact| fact.intrinsic);
    let selected = matching.next()?;
    matching
        .all(|candidate| candidate == selected)
        .then_some(selected)
}

pub(crate) fn checked_call_intrinsic(
    program: &TypedTrees,
    target: &str,
    target_symbol: SymbolHandle,
    receiver: typed_trees::expression::ExpressionHandle,
) -> Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic> {
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic as Intrinsic;

    // A resolved declaration always wins over compiler vocabulary with the
    // same spelling. Receiver calls likewise cannot select these free-call
    // intrinsics. This keeps intrinsic recognition from becoming a
    // source-text fallback for an ordinary package declaration.
    if target_symbol.is_valid() {
        None
    } else if receiver.is_valid() {
        if target == "select_provider"
            && super::provider_selection::exact_mutable_build_receiver(program, receiver)
        {
            Some(Intrinsic::BuildProviderSelection)
        } else if exact_build_output_receiver(program, receiver, target) {
            Some(Intrinsic::BuildIncludedSourceHandoff)
        } else if exact_build_log_receiver(program, receiver, target) {
            Some(Intrinsic::BuildLogWriteLine)
        } else if exact_build_optimization_receiver(program, receiver, target) {
            Some(Intrinsic::BuildOptimizationSelection)
        } else if exact_build_optimization_report_receiver(program, receiver, target) {
            Some(Intrinsic::BuildOptimizationReportRequest)
        } else {
            None
        }
    } else if let Some(predicate) =
        language_semantics::byte_predicates::ByteSequencePredicate::from_name(target)
    {
        Some(Intrinsic::ByteSequencePredicate(predicate))
    } else if target == "select_representation" {
        Some(Intrinsic::BuildRepresentationSelection)
    } else if target == "exclude_service" {
        Some(Intrinsic::BuildServiceExclusion)
    } else if target.starts_with("accept_boundary#") {
        Some(Intrinsic::BuildBoundaryAcceptance)
    } else if target.starts_with("wire_compatibility#") {
        Some(Intrinsic::BuildWireCompatibilityRequest)
    } else if target.starts_with("asm#") {
        Some(Intrinsic::InlineAssemblyOperation)
    } else {
        None
    }
}

fn exact_build_output_receiver(
    program: &TypedTrees,
    receiver: typed_trees::expression::ExpressionHandle,
    target: &str,
) -> bool {
    if target != "include_source" {
        return false;
    }
    crate::flow::expression_type_symbol(program, receiver)
        .is_some_and(|type_symbol| exact_build_prelude_data(program, type_symbol, "BuildOutput"))
}

fn exact_build_log_receiver(
    program: &TypedTrees,
    receiver: typed_trees::expression::ExpressionHandle,
    target: &str,
) -> bool {
    if target != "write_line" {
        return false;
    }
    crate::flow::expression_type_symbol(program, receiver)
        .is_some_and(|type_symbol| exact_build_prelude_data(program, type_symbol, "BuildLog"))
}

fn exact_build_optimization_receiver(
    program: &TypedTrees,
    receiver: typed_trees::expression::ExpressionHandle,
    target: &str,
) -> bool {
    if target != "enable" {
        return false;
    }
    crate::flow::expression_type_symbol(program, receiver)
        .is_some_and(|type_symbol| exact_build_prelude_data(program, type_symbol, "Optimizations"))
}

fn exact_build_optimization_report_receiver(
    program: &TypedTrees,
    receiver: typed_trees::expression::ExpressionHandle,
    target: &str,
) -> bool {
    if target != "emit_report" {
        return false;
    }
    crate::flow::expression_type_symbol(program, receiver)
        .is_some_and(|type_symbol| exact_build_prelude_data(program, type_symbol, "Optimizations"))
}

fn exact_statement_build_output_receiver(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    call: &typed_trees::statement::TableCall,
) -> bool {
    call.target.as_str() == "include_source"
        && exact_statement_build_member_receiver(program, state, call, "BuildOutput")
}

fn exact_statement_build_log_receiver(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    call: &typed_trees::statement::TableCall,
) -> bool {
    call.target.as_str() == "write_line"
        && exact_statement_build_member_receiver(program, state, call, "BuildLog")
}

fn exact_statement_build_optimization_receiver(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    call: &typed_trees::statement::TableCall,
) -> bool {
    call.target.as_str() == "enable"
        && exact_statement_build_member_receiver(program, state, call, "Optimizations")
}

fn exact_statement_build_optimization_report_receiver(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    call: &typed_trees::statement::TableCall,
) -> bool {
    call.target.as_str() == "emit_report"
        && exact_statement_build_member_receiver(program, state, call, "Optimizations")
}

fn exact_statement_build_member_receiver(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    call: &typed_trees::statement::TableCall,
    expected_receiver: &str,
) -> bool {
    let [root, members @ ..] = program.statement_table.name_path_members(call.receiver) else {
        return false;
    };
    let Some(parameter) = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.name.as_str() == root.as_str())
    else {
        return false;
    };
    let mut type_symbol = program
        .type_reference_table
        .type_symbol(parameter.type_reference);
    if !exact_build_prelude_data(program, type_symbol, "Build") {
        return false;
    }
    for member in members {
        let Some(selected) = crate::flow::resolve_member_symbol_from_type_symbol(
            program,
            type_symbol,
            member.as_str(),
        ) else {
            return false;
        };
        let Some(selected_type) = crate::flow::symbol_type_symbol(program, selected) else {
            return false;
        };
        type_symbol = selected_type;
    }
    exact_build_prelude_data(program, type_symbol, expected_receiver)
}

pub(crate) fn type_reference_names_exact_prelude_data(
    program: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
    name: &str,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            type_reference_names_exact_prelude_data(program, *referee, name)
        }
        typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_names_exact_prelude_data(program, *base_type, name)
        }
        typed_trees::types::TypeReferenceNode::Named { symbol, .. } => {
            exact_build_prelude_data(program, *symbol, name)
        }
        _ => false,
    }
}

pub(crate) fn exact_build_prelude_data(
    program: &TypedTrees,
    type_symbol: SymbolHandle,
    name: &str,
) -> bool {
    program
        .symbols
        .symbol_source_span(type_symbol)
        .and_then(|span| program.symbols.source_file(span))
        .is_some_and(|source| {
            source.origin == source::SourceOrigin::Toolchain
                && source.path == std::path::Path::new("<build-prelude>")
        })
        && program
            .data_definitions()
            .iter()
            .any(|data| data.symbol == type_symbol && data.name.as_str() == name)
}
