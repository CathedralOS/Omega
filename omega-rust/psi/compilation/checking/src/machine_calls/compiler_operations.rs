//! Exact compiler operations and the static-operand authority they grant.
//!
//! Preparation, dependency traversal, checking and Build admission share this
//! classification. An ordinary resolved declaration never gains compiler-operation
//! authority by spelling; Build execution still checks the original activation cell.

mod build_declarations;
pub use build_declarations::{
    build_operation, is_build_declaration_selection, is_build_provider_selection,
    provider_selection_expressions, resolve_build_declaration_operand, resolve_product_operand,
};

use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionIntrinsic, BuildOperation,
};
use symbols::SymbolHandle;
use typed_trees::references::{StaticArgumentCall, StaticArgumentRole};

/// Only an exact compiler operation grants declaration-description operands.
/// The same classification drives dependency traversal and static conformance
/// validation; an ordinary same-named callable keeps host value dependencies.
pub fn static_argument_role(
    program: &typed_trees::TypedTrees,
    call: StaticArgumentCall<'_>,
) -> StaticArgumentRole {
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic as Intrinsic;

    let intrinsic = match call {
        StaticArgumentCall::Expression(call) => classify_expression_call_intrinsic(program, call),
        StaticArgumentCall::Statement(call) => classify_statement_call_intrinsic(program, call),
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

pub fn classify_statement_call_intrinsic(
    program: &dyn crate::CheckingInput,
    call: &typed_trees::statement::TableCall,
) -> Option<AuthoredDeclarationSelectionIntrinsic> {
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic as Intrinsic;

    if call.target_symbol.is_valid() {
        return None;
    }
    if program.wire_encode_call_schema(call).is_some() {
        return Some(Intrinsic::WireEncode);
    }
    if program.wire_decode_call_schema(call).is_some() {
        return Some(Intrinsic::WireDecode);
    }
    checked_call_intrinsic(
        call.target.as_str(),
        call.target_symbol,
        typed_trees::expression::ExpressionHandle::invalid(),
    )
}

pub fn classify_expression_call_intrinsic(
    program: &dyn crate::CheckingInput,
    call: &typed_trees::expression::TableCallExpression,
) -> Option<AuthoredDeclarationSelectionIntrinsic> {
    if build_declarations::is_provider_selection_call(program, call) {
        return Some(AuthoredDeclarationSelectionIntrinsic::BuildProviderSelection);
    }
    if let Some(
        operation @ (BuildOperation::ServiceExclusion
        | BuildOperation::RepresentationSelection
        | BuildOperation::BoundaryAcceptance
        | BuildOperation::WireCompatibilityRequest
        | BuildOperation::AssumedBindingContract),
    ) = BuildOperation::from_call_target(call.target.as_str())
        && build_declarations::has_explicit_build_argument(program, call)
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
    checked_call_intrinsic(call.target.as_str(), call.target_symbol, call.receiver)
}

fn checked_call_intrinsic(
    target: &str,
    target_symbol: SymbolHandle,
    receiver: typed_trees::expression::ExpressionHandle,
) -> Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic> {
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic as Intrinsic;

    // A resolved declaration always wins over compiler vocabulary with the
    // same spelling, and a receiver place selects nothing here: the only
    // intrinsics left are free calls. Every toolchain Build facet operation
    // carries its facet as an explicit first parameter of its prelude
    // declaration, so an authored use resolves that declaration and the
    // first arm answers it; no build operation is recognized from the shape
    // of the place it was called on. This keeps intrinsic recognition from
    // becoming a source-text fallback for an ordinary package declaration.
    if target_symbol.is_valid() || receiver.is_valid() {
        None
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

pub fn type_reference_names_exact_prelude_data(
    program: &dyn crate::CheckingInput,
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

pub fn exact_build_prelude_data(
    program: &dyn crate::CheckingInput,
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
