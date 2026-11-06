//! Exact compiler operations and the static-operand authority they grant.
//!
//! Preparation, dependency traversal, checking and Build admission share this
//! classification. An ordinary resolved declaration never gains compiler-operation
//! authority by spelling; Build execution still checks the original activation cell.

mod build_declarations;
mod descriptor_declarations;
mod segment_registers;
mod static_interrupts;
pub use static_interrupts::{
    StaticInterruptContext, StaticInterruptDeclaration, static_interrupt_context,
    static_interrupt_declaration,
};
mod task_register;
pub use build_declarations::{
    build_operation, build_product_request_intrinsic, is_build_declaration_selection,
    resolve_build_static_operand,
};
use language_semantics::byte_predicates::ByteSequencePredicate;
/// Admit only the exact private core declaration before its extent is closed.
pub fn descriptor_table_declaration(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> Result<Option<language_semantics::ServiceReachId>, &'static str> {
    if let Some(declaration) = task_register::task_register_declaration(program, machine)? {
        return Ok(Some(declaration.service()));
    }
    Ok(segment_registers::declaration(program, machine)?.map(|declaration| declaration.service()))
}

/// Retain a closed operation so later lowering does not reopen its declaration.
pub fn descriptor_table_operation(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> Result<
    Option<(
        language_core::descriptor_tables::DescriptorTableOperation,
        language_semantics::ServiceReachId,
    )>,
    &'static str,
> {
    if let Some(operation) = task_register::task_register_operation(program, machine)? {
        return Ok(Some(operation));
    }
    segment_registers::operation(program, machine)
}
use typed_trees::types::TypeReferenceNode;

use crate::frozen_program::expression_table;
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

    match static_call_intrinsic(program, call) {
        Some(
            Intrinsic::BuildProviderSelection
            | Intrinsic::BuildBoundaryAcceptance
            | Intrinsic::BuildWireCompatibilityRequest
            | Intrinsic::BuildRepresentationSelection
            | Intrinsic::BuildServiceExclusion
            | Intrinsic::BuildServiceAudit
            | Intrinsic::BuildAssumedBindingContract,
        ) => StaticArgumentRole::DeclarationDescription,
        _ => StaticArgumentRole::ValueDependency,
    }
}

fn static_call_intrinsic(
    program: &dyn crate::CheckingInput,
    call: StaticArgumentCall<'_>,
) -> Option<AuthoredDeclarationSelectionIntrinsic> {
    authored_call_intrinsic(program, call).or_else(|| match call {
        StaticArgumentCall::Expression(_, call) => {
            classify_expression_call_intrinsic(program, call)
        }
        StaticArgumentCall::Statement(call) => classify_statement_call_intrinsic(program, call),
    })
}

/// Select the operand contract from the exact operation, independently of an
/// operand's representation category. Applied maps can arrive as structural
/// type references, so testing only for a Conformance symbol loses this rule.
pub(crate) fn validate_declaration_operands(
    program: &dyn crate::CheckingInput,
    call: StaticArgumentCall<'_>,
) -> Result<(), diagnostics::Diagnostic> {
    if static_call_intrinsic(program, call)
        == Some(AuthoredDeclarationSelectionIntrinsic::BuildProviderSelection)
        && call
            .arguments(program.typed_program())
            .iter()
            .any(|argument| !typed_trees::product_operand::is_declaration_operand(argument))
    {
        return Err(diagnostics::Diagnostic::error(
            "provider selection operands must name bare declarations, not applied types, conformance applications, values or projections",
        ));
    }
    Ok(())
}

/// Typing can finalize a declaration operation before executable checking,
/// notably target-provider defaults. Consume that exact occurrence rather
/// than rediscovering its enclosing declaration or receiver spelling. This
/// identity describes operands; Build activation still owes its own admission.
pub fn authored_call_intrinsic(
    program: &dyn crate::CheckingInput,
    call: StaticArgumentCall<'_>,
) -> Option<AuthoredDeclarationSelectionIntrinsic> {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget,
    };
    let selections = program.authored_declaration_selections();
    let selection = match call {
        StaticArgumentCall::Expression(expression, _) => expression_table(program)
            .authored_selection_occurrences(expression)
            .filter_map(|occurrence| selections.get(occurrence))
            .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::Call),
        StaticArgumentCall::Statement(call) => {
            return call
                .authored_call_selection
                .and_then(|occurrence| selections.get(occurrence))
                .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::Call)
                .and_then(|selection| match selection.target() {
                    AuthoredDeclarationSelectionTarget::Intrinsic(intrinsic) => Some(intrinsic),
                    _ => None,
                });
        }
    };
    let mut occurrences = selection;
    let selection = occurrences.next()?;
    if occurrences.next().is_some() {
        return None;
    }
    match selection.target() {
        AuthoredDeclarationSelectionTarget::Intrinsic(intrinsic) => Some(intrinsic),
        _ => None,
    }
}

pub fn classify_statement_call_intrinsic(
    _program: &dyn crate::CheckingInput,
    call: &typed_trees::statement::TableCall,
) -> Option<AuthoredDeclarationSelectionIntrinsic> {
    if call.target_symbol.is_valid() {
        return None;
    }
    checked_call_intrinsic(
        call.target.as_str(),
        call.target_symbol,
        typed_trees::expression::ExpressionHandle::invalid(),
    )
}

/// `Schema::encode` / `Schema::decode` on a numbered data declaration.
/// Synthesis no longer installs a callable, and the schema symbol is not a
/// machine. The leaf still names that declaration's derived wire schema.
pub fn statement_wire_codec_intrinsic(
    program: &dyn crate::CheckingInput,
    call: &typed_trees::statement::TableCall,
) -> Option<AuthoredDeclarationSelectionIntrinsic> {
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic as Intrinsic;

    if call.target_symbol.is_valid() {
        return None;
    }
    let intrinsic = match call.target.as_str() {
        "encode" => Intrinsic::WireEncode,
        "decode" => Intrinsic::WireDecode,
        _ => return None,
    };
    let members = program.statement_table.name_path_members(call.receiver);
    let leaf = members.last()?;
    program
        .wire_schemas()
        .iter()
        .any(|schema| schema.name.as_str() == leaf.as_str())
        .then_some(intrinsic)
}

/// The one caller place an exclusive reference argument denotes.
pub fn single_exclusive_reference_storage(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    argument: typed_trees::expression::ExpressionHandle,
) -> Option<(symbols::SymbolHandle, Vec<facts::PlaceSegment>)> {
    crate::machine_calls::calls::single_exclusive_reference_storage(program, machine, argument)
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
        | BuildOperation::ServiceAudit
        | BuildOperation::RepresentationSelection
        | BuildOperation::BoundaryAcceptance
        | BuildOperation::WireCompatibilityRequest
        | BuildOperation::AssumedBindingContract),
    ) = BuildOperation::from_call_target(call.target.as_str())
        && build_declarations::has_explicit_build_argument(program, call)
        && expression_table(program)
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
    } else if let Some(predicate) = ByteSequencePredicate::from_name(target) {
        Some(Intrinsic::ByteSequencePredicate(predicate))
    } else if target == "address_of" {
        Some(Intrinsic::StorageAddress)
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
        TypeReferenceNode::Reference { referee, .. } => {
            type_reference_names_exact_prelude_data(program, *referee, name)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_names_exact_prelude_data(program, *base_type, name)
        }
        TypeReferenceNode::Named { symbol, .. } => exact_build_prelude_data(program, *symbol, name),
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

/// The storage form has one runtime reference operand. A selected ordinary
/// declaration wins over the core spelling; downstream consumers retain this
/// intrinsic selection rather than treating arbitrary calls as address queries.
pub fn storage_address_operand(
    program: &dyn crate::CheckingInput,
    call: &typed_trees::expression::TableCallExpression,
) -> Option<typed_trees::expression::ExpressionHandle> {
    (classify_expression_call_intrinsic(program, call)
        == Some(AuthoredDeclarationSelectionIntrinsic::StorageAddress)
        && call.machine_arguments.is_empty())
    .then(|| expression_table(program).expression_handles(call.arguments))
    .filter(|arguments| arguments.len() == 1)
    .map(|arguments| arguments[0])
}

/// Provider selection records a build choice without replacing references in
/// its value arguments. Its operands still execute first, and their effects
/// remain visible. This is only the descriptor effect of the exact retained
/// intrinsic; declaration compatibility and Build activation remain separate.
pub(crate) fn call_preserves_reference_slots(
    program: &dyn crate::CheckingInput,
    call: StaticArgumentCall<'_>,
) -> bool {
    authored_call_intrinsic(program, call)
        == Some(AuthoredDeclarationSelectionIntrinsic::BuildProviderSelection)
}

/// These resolved observations neither write caller storage nor install
/// reference slots. Their runtime operands still execute with their ordinary
/// effects; an ordinary same-spelled declaration does not select this rule.
///
/// `size_of` and `align_of` are the same kind of observation: the call node
/// retains a layout query, and the answer is a `u64` closed by the selected
/// target. They are not invocations, so a missing callee body is not an
/// incomplete reference relation.
pub(crate) fn expression_call_only_observes_operands(
    program: &dyn crate::CheckingInput,
    call: &typed_trees::expression::TableCallExpression,
) -> bool {
    call.layout_query.is_some()
        || matches!(
            classify_expression_call_intrinsic(program, call),
            Some(
                AuthoredDeclarationSelectionIntrinsic::ByteSequencePredicate(_)
                    | AuthoredDeclarationSelectionIntrinsic::StorageAddress
            )
        )
}

/// A descriptor instruction borrows exactly ten bytes. Hardware-written bytes
/// cannot establish a reference, a refinement or any other typed value.
pub fn global_descriptor_table_operand(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: Option<&typed_trees::state::State>,
    operand: typed_trees::expression::ExpressionHandle,
    operation: language_core::inline_assembly::AsmGlobalDescriptorTableOperation,
) -> Result<typed_trees::types::TypeReferenceHandle, &'static str> {
    use language_core::inline_assembly::AsmGlobalDescriptorTableOperation as Operation;
    use language_semantics::ReferenceAccess;
    let reference = crate::expression_result_type_reference(
        program,
        machine,
        state.ok_or("descriptor instruction has no enclosing machine body")?,
        operand,
    )
    .ok_or("operand has no checked reference type")?;
    let shell = crate::reference_shell(program, reference)
        .ok_or("operand must be a borrowed ten-byte array, not a numeric address")?;
    match (operation, shell.access) {
        (Operation::Load, ReferenceAccess::WriteOnly) => return Err("lgdt needs readable bytes"),
        (Operation::Store, ReferenceAccess::Shared) => return Err("sgdt needs writable bytes"),
        _ => {}
    }
    let TypeReferenceNode::FixedArray {
        element_type: element,
        ..
    } = program.type_reference_table.type_reference(shell.referee)
    else {
        return Err("descriptor must be an unqualified fixed byte array");
    };
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(*element)
    else {
        return Err("descriptor bytes must be unqualified u8 values");
    };
    if program.symbols.builtin_type_atom(*symbol) != Some(symbols::BuiltinTypeAtom::U8)
        || program.closed_fixed_array_length(shell.referee) != Some(10)
    {
        return Err("descriptor must be exactly [u8; 10]");
    }
    Ok(reference)
}

/// Identify descriptor instructions by their resolved builtin, never by source spelling.
pub fn global_descriptor_table_operation(
    program: &dyn crate::CheckingInput,
    target: symbols::SymbolHandle,
) -> Option<language_core::inline_assembly::AsmGlobalDescriptorTableOperation> {
    use language_core::inline_assembly::AsmGlobalDescriptorTableOperation as Operation;
    match program.symbols.builtin_function_for_symbol(target)? {
        symbols::BuiltinFunction::AsmStoreGdtr => Some(Operation::Store),
        symbols::BuiltinFunction::AsmLoadGdtr => Some(Operation::Load),
        _ => None,
    }
}

/// Return the checked receiver types used by ordinary argument evaluation.
/// A missing or invalid instruction contract grants no forwarding authority.
pub fn global_descriptor_table_operand_types(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    target: symbols::SymbolHandle,
    arguments: &[typed_trees::expression::ExpressionHandle],
) -> Option<[typed_trees::types::TypeReferenceHandle; 1]> {
    let operation = global_descriptor_table_operation(program, target)?;
    let [argument] = arguments else {
        return None;
    };
    Some([
        global_descriptor_table_operand(program, machine, Some(state), *argument, operation)
            .ok()?,
    ])
}
