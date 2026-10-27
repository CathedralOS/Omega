//! Rejoin original binder scopes through a live specialization's exact tuple.
//! Descriptions contain no executable declaration and never enter root lookup.

use super::StaticDeclaration;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::references::{StaticArgumentCall, TypedReferences};
use typed_trees::static_applications::StaticMachineApplicationHandle;

#[derive(Default)]
struct ApplicationOwner {
    application: StaticMachineApplicationHandle,
    found: bool,
    expressions: std::collections::HashSet<typed_trees::expression::ExpressionHandle>,
    types: std::collections::HashSet<typed_trees::types::TypeReferenceHandle>,
}

impl Extend<SymbolHandle> for ApplicationOwner {
    fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {}
}

impl TypedReferences for ApplicationOwner {
    fn expression(&mut self, expression: typed_trees::expression::ExpressionHandle) -> bool {
        self.expressions.insert(expression)
    }
    fn type_reference(&mut self, reference: typed_trees::types::TypeReferenceHandle) -> bool {
        self.types.insert(reference)
    }
    fn call_arguments(
        &mut self,
        _: &typed_trees::TypedTrees,
        call: StaticArgumentCall<'_>,
    ) -> bool {
        self.found |= call.application() == self.application;
        true
    }
}

pub(super) fn find<'program>(
    program: &'program dyn crate::CheckingInput,
    symbol: SymbolHandle,
    application: StaticMachineApplicationHandle,
) -> Result<Option<StaticDeclaration<'program>>, Diagnostic> {
    let typed = program.typed_program();
    let current = typed
        .expression_table
        .static_machine_application(application);
    let mut result: Option<(
        &typed_trees::typed_trees::StaticMachineDeclarationDescription,
        arena::HandleSpan<typed_trees::data::TypeParameter>,
    )> = None;
    for receipt in typed
        .machine_specializations
        .iter()
        .filter(|receipt| receipt.template == symbol)
    {
        let Some(instance) = typed.machine_by_symbol(receipt.instance) else {
            continue;
        };
        if !typed
            .expression_table
            .static_machine_application_is_valid(receipt.binding_application)
        {
            return Err(Diagnostic::error(
                "retained static declaration lost its exact specialization tuple",
            ));
        }
        let original = typed
            .expression_table
            .static_machine_application(receipt.binding_application);
        // Separate retained call/instance handles may own the same exact tuple.
        // Binder symbols and lexical ownership are part of that comparison.
        let same_tuple = current.binding_declaration == original.binding_declaration
            && current.registration_operation == original.registration_operation
            && current.lexical_owner == original.lexical_owner
            && typed
                .expression_table
                .static_machine_application_bindings(application)
                == typed
                    .expression_table
                    .static_machine_application_bindings(receipt.binding_application)
            && typed
                .expression_table
                .static_machine_application_lifetimes(application)
                == typed
                    .expression_table
                    .static_machine_application_lifetimes(receipt.binding_application);
        let binder = current.binding_declaration == symbol && same_tuple;
        let mut owner = ApplicationOwner {
            application,
            ..Default::default()
        };
        if current.lexical_owner == symbol {
            typed_trees::references::visit_machine(typed, &mut owner, instance);
        }
        if !binder && !owner.found {
            continue;
        }
        if receipt.instance == receipt.template {
            return Err(Diagnostic::error(
                "static declaration description aliases its executable instance with the authored template",
            ));
        }
        if typed
            .machine_specializations
            .iter()
            .filter(|other| other.instance == receipt.instance)
            .count()
            != 1
        {
            return Err(Diagnostic::error(
                "retained static declaration has ambiguous specialization ownership",
            ));
        }
        let Some(description) = &receipt.template_declaration else {
            return Err(Diagnostic::error(
                "live static tuple has no original declaration description",
            ));
        };
        let parameters = typed
            .data_type_parameters
            .span_or_empty(receipt.template_parameters);
        if !symbol.is_valid()
            || description.symbol != symbol
            || description.source_span != typed.symbols.symbol_source_span(symbol)
            || parameters.len() != receipt.template_parameters.len()
            || original.binding_declaration != symbol
            || receipt.normalized_template_identity.is_empty()
        {
            return Err(Diagnostic::error(
                "retained static declaration changed its template, telescope or source owner",
            ));
        }
        if let Some((previous, parameters)) = result {
            if previous != description || parameters != receipt.template_parameters {
                return Err(Diagnostic::error(
                    "live static tuples retain incompatible original declaration descriptions",
                ));
            }
        }
        result = Some((description, receipt.template_parameters));
    }
    Ok(result.map(|(description, parameters)| StaticDeclaration {
        symbol: description.symbol,
        name: &description.name,
        parameters: typed.data_type_parameters.span_or_empty(parameters),
        lifetime_parameters: &description.lifetime_parameters,
        conformance_bounds: &description.conformance_bounds,
    }))
}
