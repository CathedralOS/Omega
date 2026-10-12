//! Typed authority for Build operations selecting product declarations.
//!
//! `Build::` identifies the toolchain namespace; it supplies no receiver value.
//! The first ordinary argument must be a mutable Build borrow. Execution checks
//! that borrow against the current activation's original cell, so a fabricated
//! Build cannot acquire authority merely by passing this type-level check.

use super::exact_build_prelude_data;
use language_semantics::declaration_selection::BuildOperation;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
use typed_trees::references::StaticArgumentCall;

/// The declaration category owned by one designated, non-executing operand.
/// Each designated operand keeps its original Product declaration role.
/// A callable requirement and a whole interface have different telescopes.
pub use typed_trees::typed_trees::product_applications::ProductDeclarationKind;

/// The category of one original operand of an already admitted Build operation.
/// Tuple descriptions and closed provider/service descriptions share selection
/// custody; forming a requirement tuple remains a separate admission duty.
pub fn build_declaration_operand_kind(
    operation: BuildOperation,
    argument_index: usize,
) -> Option<ProductDeclarationKind> {
    match (operation, argument_index) {
        (BuildOperation::AssumedBindingContract, 0 | 1) => {
            Some(ProductDeclarationKind::BoundaryRequirement)
        }
        (BuildOperation::ProviderSelection, 0) | (BuildOperation::ServiceExclusion, 0) => {
            Some(ProductDeclarationKind::ProviderSubject)
        }
        (BuildOperation::ProviderSelection, 1) => Some(ProductDeclarationKind::ProviderType),
        _ => None,
    }
}

pub fn is_build_declaration_selection(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    program
        .expression_table
        .call_node(expression)
        .is_some_and(|call| {
            build_operation_for_call(program, StaticArgumentCall::Expression(expression, call))
                .and_then(|operation| build_declaration_operand_kind(operation, 0))
                .is_some()
        })
}

/// Consume the original selected declaration, never a normalized type graph or
/// another resolution of its spelling. Both expression and statement finalizers
/// use the same call/application coordinate. Ordinary calls retain their original
/// operand symbol in the source application; Build descriptions use its Product
/// payload owner. A missing Product selection never falls through to that symbol.
pub fn resolve_static_argument_selection(
    program: &dyn crate::CheckingInput,
    call: StaticArgumentCall<'_>,
    position: language_semantics::declaration_selection::StaticArgumentPosition,
) -> Option<SymbolHandle> {
    if let Some(operation) = build_operation_for_call(program, call)
        && build_declaration_operand_kind(operation, position.argument_index).is_some()
    {
        let operand =
            program.product_declaration_operand(call.application(), position.argument_index)?;
        if position.descendant_ordinal == 0 {
            return operand
                .declaration
                .is_valid()
                .then_some(operand.declaration);
        }
        let mut selections = operand
            .owner
            .product_static_argument_selections
            .span_or_empty(operand.declaration_selections)
            .iter()
            .filter(|selection| selection.descendant_ordinal == position.descendant_ordinal);
        let selected = selections.next()?.declaration;
        return (selected.is_valid() && selections.next().is_none()).then_some(selected);
    }
    let argument = call.argument_at(program.typed_program(), position)?;
    argument.symbol.is_valid().then_some(argument.symbol)
}

pub fn provider_selection_expressions(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> Vec<ExpressionHandle> {
    let mut expressions = Vec::new();
    for state in program.machine_states(machine) {
        for statement in program.statement_table.statements(state.statement_nodes) {
            crate::expression_traversal::collect_statement_expression_trees(
                program,
                statement,
                &mut expressions,
            );
        }
    }
    expressions.retain(|expression| {
        matches!(program.expression_table.expression(*expression), ExpressionNode::Call(call)
            if BuildOperation::from_call_target(call.target.as_str())
                == Some(BuildOperation::ProviderSelection)
                && !call.target_symbol.is_valid())
    });
    expressions
}

pub fn is_build_provider_selection(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    let Some(call) = program.expression_table.call_node(expression) else {
        return false;
    };
    is_provider_selection_call(program, call)
}

pub fn is_provider_selection_call(
    program: &dyn crate::CheckingInput,
    call: &typed_trees::expression::TableCallExpression,
) -> bool {
    BuildOperation::from_call_target(call.target.as_str())
        == Some(BuildOperation::ProviderSelection)
        && has_explicit_build_argument(program, call)
        && (1..=2).contains(
            &program
                .expression_table
                .expression_handles(call.arguments)
                .len(),
        )
        && call.machine_arguments.len() == 2
}

/// Namespace identity selects the operation; its first value operand carries
/// the activation. Never infer that operand from a receiver or a local name.
pub(super) fn has_explicit_build_argument(
    program: &dyn crate::CheckingInput,
    call: &typed_trees::expression::TableCallExpression,
) -> bool {
    !call.target_symbol.is_valid()
        && call.receiver.is_valid()
        && matches!(program.expression_table.expression(call.receiver), ExpressionNode::Name(path)
            if exact_build_prelude_data(program, path.symbol, "Build"))
        && program
            .expression_table
            .expression_handles(call.arguments)
            .first()
            .is_some_and(|builder| exact_mutable_build_argument(program, *builder))
        && call.evidence_arguments.is_empty()
}

fn exact_mutable_build_argument(
    program: &dyn crate::CheckingInput,
    receiver: ExpressionHandle,
) -> bool {
    if !receiver.is_valid() {
        return false;
    }
    if let Some(borrow) = program.expression_table.borrow_expression(receiver) {
        return borrow.access == language_semantics::ReferenceAccess::Mutable
            && crate::expression_type_symbol(program, borrow.target)
                .is_some_and(|symbol| exact_build_prelude_data(program, symbol, "Build"));
    }
    crate::expression_place_type_reference(program, receiver, &[]).is_some_and(|reference| {
        matches!(program.type_reference_table.type_reference(reference),
            typed_trees::types::TypeReferenceNode::Reference { access, referee, .. }
                if *access == language_semantics::ReferenceAccess::Mutable
                    && exact_build_prelude_data(program, program.type_reference_table.type_symbol(*referee), "Build"))
    })
}

/// Resolve an already classified product operand, never an ordinary static use.
/// The collector of an executed selection independently rejoins the same path.
pub fn resolve_product_declaration(
    program: &dyn crate::CheckingInput,
    argument: &StaticMachineArgument,
    occurrence: source::SourceSpan,
    kind: ProductDeclarationKind,
) -> Option<SymbolHandle> {
    if matches!(kind, ProductDeclarationKind::BoundaryRequirement) {
        return super::resolve_product_requirement(program, argument, occurrence)
            .map(|selection| selection.declaration);
    }
    typed_trees::product_operand::resolve_product_operand(
        program.typed_program(),
        argument,
        occurrence,
        matches!(kind, ProductDeclarationKind::ProviderSubject),
    )
}

/// Classify the exact compiler Build operation, not an ordinary same-named call.
/// Statement and expression forms retain their existing admission boundaries.
pub fn build_operation_for_call(
    program: &dyn crate::CheckingInput,
    call: StaticArgumentCall<'_>,
) -> Option<BuildOperation> {
    let (target, intrinsic) = match call {
        StaticArgumentCall::Expression(_, call) => (
            call.target.as_str(),
            super::classify_expression_call_intrinsic(program, call),
        ),
        StaticArgumentCall::Statement(call) => (
            call.target.as_str(),
            super::classify_statement_call_intrinsic(program, call),
        ),
    };
    let intrinsic = super::authored_call_intrinsic(program, call).or(intrinsic);
    let operation = BuildOperation::from_call_target(target)?;
    (intrinsic == Some(operation.intrinsic())).then_some(operation)
}

pub fn build_operation(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<BuildOperation> {
    build_operation_for_call(
        program,
        StaticArgumentCall::Expression(expression, program.expression_table.call_node(expression)?),
    )
}

use typed_trees::product_requirements::product_requirement_schema as requirement_schema;
/// An exact interface view for ordinary static tuple formation. Trait-owner
/// binders precede member binders; invocation lifetime families remain separate
/// from the captured regions of a generic trait owner.
pub use typed_trees::product_requirements::{
    ProductDeclarationTelescope, product_requirement_application_declaration,
};

/// The exact designated Product role determines the original telescope.
/// A boundary trait subject carries only its outer interface tuple; a callable
/// requirement carries its outer and member tuple. Neither is executable here.
pub fn product_application_declaration<'program>(
    program: &'program typed_trees::TypedTrees,
    application: &typed_trees::static_applications::StaticMachineApplication,
    kind: ProductDeclarationKind,
) -> Option<ProductDeclarationTelescope<'program>> {
    match kind {
        ProductDeclarationKind::BoundaryRequirement => {
            product_requirement_application_declaration(program, application)
        }
        ProductDeclarationKind::ProviderSubject => {
            let declaration =
                program.trait_definition_by_symbol(application.binding_declaration)?;
            if !declaration.is_boundary
                || application.registration_operation != declaration.symbol
                || application.selected_interface != declaration.symbol
            {
                return None;
            }
            Some(ProductDeclarationTelescope {
                symbol: declaration.symbol,
                name: &declaration.name,
                parameters: program.trait_type_parameters(declaration).iter().collect(),
                conformance_bounds: &declaration.conformance_bounds,
                captured_lifetimes: &declaration.lifetime_parameters,
                invocation_lifetimes: &[],
            })
        }
        ProductDeclarationKind::ProviderType => None,
    }
}

pub fn product_requirement_declaration(
    program: &typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<ProductDeclarationTelescope<'_>> {
    let declaration =
        typed_trees::static_applications::StaticApplicationDeclaration::for_symbol(program, symbol)
            .ok()?;
    requirement_schema(program, symbol, SymbolHandle::invalid(), declaration)
}

pub fn product_requirement_selection_declaration(
    program: &typed_trees::TypedTrees,
    selection: super::ProductRequirementSelection,
) -> Option<ProductDeclarationTelescope<'_>> {
    let declaration =
        typed_trees::static_applications::StaticApplicationDeclaration::for_selected_interface(
            program,
            selection.declaration,
            selection.selected_interface,
        )
        .ok()?;
    requirement_schema(
        program,
        selection.declaration,
        selection.selected_interface,
        declaration,
    )
}

/// Closed declaration-only consumers retain their original absence semantics.
/// Applied requirement consumers use the explicitly owned application row.
pub fn resolve_product_operand(
    program: &dyn crate::CheckingInput,
    argument: &StaticMachineArgument,
    occurrence: source::SourceSpan,
    kind: ProductDeclarationKind,
) -> Option<SymbolHandle> {
    if argument.application.is_some() {
        return None;
    }
    if matches!(kind, ProductDeclarationKind::BoundaryRequirement) {
        let selection = super::resolve_product_requirement(program, argument, occurrence)?;
        let declaration =
            product_requirement_selection_declaration(program.product_declarations(), selection)?;
        if !declaration.parameters.is_empty()
            || !declaration.conformance_bounds.is_empty()
            || !declaration.captured_lifetimes.is_empty()
        {
            return None;
        }
        Some(selection.declaration)
    } else {
        resolve_product_declaration(program, argument, occurrence, kind)
    }
}

/// The authored call occurrence keeps Product package authority even when a
/// synthetic enclosing expression span follows receiver or statement lowering.
pub fn build_declaration_expression_source_span(
    program: &typed_trees::TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> source::SourceSpan {
    program
        .expression_table
        .authored_selection_occurrences(expression)
        .filter_map(|occurrence| program.authored_declaration_selections().get(occurrence))
        .find(|selection| {
            selection.kind()
                == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Call
        })
        .map_or_else(
            || program.expression_table.source_span(expression),
            |selection| selection.source_span(),
        )
}
