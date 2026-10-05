//! Intrinsic call targets and exact build receivers.

use checked_trees::CheckFacts;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionIntrinsic, BuildOperation,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::references::{StaticArgumentCall, StaticArgumentRole};

/// Only an exact compiler operation grants declaration-description operands.
/// The same classification drives dependency traversal and static conformance
/// validation; an ordinary same-named callable keeps host value dependencies.
pub(crate) fn static_argument_role(
    program: &TypedTrees,
    call: StaticArgumentCall<'_>,
) -> StaticArgumentRole {
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic as Intrinsic;

    let intrinsic = match call {
        StaticArgumentCall::Expression(call) => checked_expression_call_intrinsic(program, call),
        StaticArgumentCall::Statement(state, call) => {
            checked_statement_call_intrinsic(program, state, call)
        }
    };
    match intrinsic {
        Some(
            Intrinsic::BuildProviderSelection
            | Intrinsic::BuildBoundaryAcceptance
            | Intrinsic::BuildWireCompatibilityRequest
            | Intrinsic::BuildRepresentationSelection
            | Intrinsic::BuildServiceExclusion
            | Intrinsic::BuildAssumedBindingContract,
        ) => StaticArgumentRole::DeclarationDescription,
        _ => StaticArgumentRole::ValueDependency,
    }
}

/// How checking tells a toolchain-owned build operation apart from an
/// ordinary call which happens to share its spelling.
///
/// [`BuildOperation::from_call_target`] owns the spelling; this form owns the
/// receiver or call shape each spelling still needs before checking records
/// the operation's intrinsic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BuildOperationForm {
    /// Namespace-qualified operation with an explicit Build argument.
    StaticBuildArgument,
    /// Selected on a receiver of one exact build-prelude data type.
    PreludeDataReceiver(&'static str),
}

fn build_operation_form(operation: BuildOperation) -> BuildOperationForm {
    match operation {
        BuildOperation::ProviderSelection
        | BuildOperation::ServiceExclusion
        | BuildOperation::RepresentationSelection
        | BuildOperation::BoundaryAcceptance
        | BuildOperation::WireCompatibilityRequest
        | BuildOperation::AssumedBindingContract => BuildOperationForm::StaticBuildArgument,
        BuildOperation::IncludedSourceHandoff => {
            BuildOperationForm::PreludeDataReceiver("BuildOutput")
        }
        BuildOperation::LogWriteLine => BuildOperationForm::PreludeDataReceiver("BuildLog"),
        BuildOperation::OptimizationSelection | BuildOperation::OptimizationReportRequest => {
            BuildOperationForm::PreludeDataReceiver("Optimizations")
        }
    }
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
            BuildOperationForm::PreludeDataReceiver(expected_receiver) => {
                exact_statement_build_member_receiver(program, state, call, expected_receiver)
            }
            BuildOperationForm::StaticBuildArgument => false,
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

pub(crate) fn checked_expression_call_intrinsic(
    program: &TypedTrees,
    call: &typed_trees::expression::TableCallExpression,
) -> Option<AuthoredDeclarationSelectionIntrinsic> {
    if super::provider_selection::is_provider_selection_call(program, call) {
        return Some(AuthoredDeclarationSelectionIntrinsic::BuildProviderSelection);
    }
    if let Some(
        operation @ (BuildOperation::ServiceExclusion
        | BuildOperation::RepresentationSelection
        | BuildOperation::BoundaryAcceptance
        | BuildOperation::WireCompatibilityRequest
        | BuildOperation::AssumedBindingContract),
    ) = BuildOperation::from_call_target(call.target.as_str())
        && super::provider_selection::has_explicit_build_argument(program, call)
        && program
            .expression_table
            .expression_handles(call.arguments)
            .len()
            == 1
        && match operation {
            BuildOperation::WireCompatibilityRequest => call.machine_arguments.len() >= 5,
            BuildOperation::RepresentationSelection | BuildOperation::AssumedBindingContract => {
                call.machine_arguments.len() == 2
            }
            _ => call.machine_arguments.len() == 1,
        }
    {
        return Some(operation.intrinsic());
    }
    checked_call_intrinsic(
        program,
        call.target.as_str(),
        call.target_symbol,
        call.receiver,
    )
}

fn checked_call_intrinsic(
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
            BuildOperationForm::PreludeDataReceiver(expected_receiver) => {
                exact_build_prelude_receiver(program, receiver, expected_receiver)
            }
            BuildOperationForm::StaticBuildArgument => false,
        };
        receiver_selects.then(|| operation.intrinsic())
    } else if let Some(predicate) =
        language_semantics::byte_predicates::ByteSequencePredicate::from_name(target)
    {
        Some(Intrinsic::ByteSequencePredicate(predicate))
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
