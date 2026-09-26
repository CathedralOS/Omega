//! Intrinsic call targets and exact build receivers.

use checked_trees::CheckFacts;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionIntrinsic, BuildOperation,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

/// How checking tells a toolchain-owned build operation apart from an
/// ordinary call which happens to share its spelling.
///
/// [`BuildOperation::from_call_target`] owns the spelling; this form owns the
/// receiver or call shape each spelling still needs before checking records
/// the operation's intrinsic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BuildOperationForm {
    /// Selected on the mutable root `Build` receiver.
    MutableBuildReceiver,
    /// Selected on a receiver of one exact build-prelude data type.
    PreludeDataReceiver(&'static str),
    /// A parser-carved marker with no declared target and no receiver
    /// custody at this stage: the spelling alone selects it.
    Marker,
}

fn build_operation_form(operation: BuildOperation) -> BuildOperationForm {
    match operation {
        BuildOperation::ProviderSelection => BuildOperationForm::MutableBuildReceiver,
        BuildOperation::IncludedSourceHandoff => {
            BuildOperationForm::PreludeDataReceiver("BuildOutput")
        }
        BuildOperation::LogWriteLine => BuildOperationForm::PreludeDataReceiver("BuildLog"),
        BuildOperation::OptimizationSelection | BuildOperation::OptimizationReportRequest => {
            BuildOperationForm::PreludeDataReceiver("Optimizations")
        }
        BuildOperation::RepresentationSelection
        | BuildOperation::ServiceExclusion
        | BuildOperation::BoundaryAcceptance
        | BuildOperation::WireCompatibilityRequest => BuildOperationForm::Marker,
    }
}

/// Whether a call target is the parser-carved boundary-acceptance marker
/// (`b.accept_boundary<path>()` is retained as `accept_boundary#<path>`): a
/// statically harvested build declaration, never an operational machine
/// call, so the borrow, flow and ownership lanes mint no call evidence for
/// it.
pub(crate) fn is_boundary_acceptance_marker(call_target: &str) -> bool {
    BuildOperation::from_call_target(call_target) == Some(BuildOperation::BoundaryAcceptance)
}

pub(crate) fn checked_statement_call_intrinsic(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    call: &typed_trees::statement::TableCall,
) -> Option<AuthoredDeclarationSelectionIntrinsic> {
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic as Intrinsic;

    if call.target_symbol.is_valid() {
        return None;
    }
    if let Some(operation) = BuildOperation::from_call_target(call.target.as_str())
        && match build_operation_form(operation) {
            BuildOperationForm::MutableBuildReceiver => {
                super::provider_selection::exact_mutable_build_statement_receiver(program, call)
            }
            BuildOperationForm::PreludeDataReceiver(expected_receiver) => {
                exact_statement_build_member_receiver(program, state, call, expected_receiver)
            }
            BuildOperationForm::Marker => false,
        }
    {
        return Some(operation.intrinsic());
    }
    if program.wire_encode_call_schema(call).is_some() {
        return Some(Intrinsic::WireEncode);
    }
    if program.wire_decode_call_schema(call).is_some() {
        return Some(Intrinsic::WireDecode);
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
        let operation = BuildOperation::from_call_target(target)?;
        let receiver_selects = match build_operation_form(operation) {
            BuildOperationForm::MutableBuildReceiver => {
                super::provider_selection::exact_mutable_build_receiver(program, receiver)
            }
            BuildOperationForm::PreludeDataReceiver(expected_receiver) => {
                exact_build_prelude_receiver(program, receiver, expected_receiver)
            }
            BuildOperationForm::Marker => false,
        };
        receiver_selects.then(|| operation.intrinsic())
    } else if let Some(predicate) =
        language_semantics::byte_predicates::ByteSequencePredicate::from_name(target)
    {
        Some(Intrinsic::ByteSequencePredicate(predicate))
    } else if let Some(operation) = BuildOperation::from_call_target(target)
        && build_operation_form(operation) == BuildOperationForm::Marker
    {
        Some(operation.intrinsic())
    } else if target.starts_with("asm#") {
        Some(Intrinsic::InlineAssemblyOperation)
    } else {
        None
    }
}

fn exact_build_prelude_receiver(
    program: &TypedTrees,
    receiver: typed_trees::expression::ExpressionHandle,
    expected_receiver: &str,
) -> bool {
    crate::flow::expression_type_symbol(program, receiver).is_some_and(|type_symbol| {
        exact_build_prelude_data(program, type_symbol, expected_receiver)
    })
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
