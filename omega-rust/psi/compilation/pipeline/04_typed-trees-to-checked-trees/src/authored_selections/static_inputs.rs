//! Finalize source-issued static input occurrences after independent ordinary
//! formation checks. Product root descriptors grant no authority here.
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionKind as Kind, AuthoredDeclarationSelectionLateBinding as Binding,
    AuthoredDeclarationSelectionTarget as Target,
};
use validation::selection_resolution::{
    AuthoredSelectionResolution, AuthoredSelectionResolutionTarget, push_consistent_resolution,
};

pub(super) fn collect(
    program: &dyn validation::CheckingInput,
    resolutions: &mut Vec<AuthoredSelectionResolution>,
) -> Result<(), Diagnostic> {
    let typed = program.typed_program();
    for handle in typed.live_product_operand_applications() {
        let application = typed.product_operand_applications.get(handle);
        collect_inputs(
            typed,
            application.input_selections,
            application
                .argument
                .application
                .as_ref()
                .map_or(&[][..], |application| application.arguments.as_ref()),
            resolutions,
        )?;
    }
    #[derive(Default)]
    struct Applications {
        handles: Vec<typed_trees::static_applications::StaticMachineApplicationHandle>,
        expressions: Vec<typed_trees::expression::ExpressionHandle>,
    }
    impl Extend<symbols::SymbolHandle> for Applications {
        fn extend<Symbols: IntoIterator<Item = symbols::SymbolHandle>>(&mut self, _: Symbols) {}
    }
    impl typed_trees::references::TypedReferences for Applications {
        fn static_application(
            &mut self,
            handle: typed_trees::static_applications::StaticMachineApplicationHandle,
        ) {
            if !self.handles.contains(&handle) {
                self.handles.push(handle);
            }
        }
        fn expression(&mut self, handle: typed_trees::expression::ExpressionHandle) -> bool {
            if self.expressions.contains(&handle) {
                return false;
            }
            self.expressions.push(handle);
            true
        }
    }
    let mut applications = Applications::default();
    typed_trees::references::visit_program(typed, &mut applications);
    for handle in applications.handles {
        let application = typed.expression_table.static_machine_application(handle);
        collect_inputs(
            typed,
            application.input_selections,
            typed
                .expression_table
                .static_machine_application_arguments(handle),
            resolutions,
        )?;
    }
    Ok(())
}

fn collect_inputs(
    typed: &typed_trees::TypedTrees,
    inputs: arena::HandleSpan<typed_trees::product_operand::ProductInputSelection>,
    arguments: &[typed_trees::expression::StaticMachineArgument],
    resolutions: &mut Vec<AuthoredSelectionResolution>,
) -> Result<(), Diagnostic> {
    for retained in typed.product_input_selections.span_or_empty(inputs) {
        let occurrence = retained.occurrence.ok_or_else(|| {
            Diagnostic::error("static input retains an absent selection occurrence")
        })?;
        let selection = typed
            .authored_declaration_selections()
            .get(occurrence)
            .ok_or_else(|| {
                Diagnostic::error("Product input retains an unknown authored selection occurrence")
            })?;
        if selection.compiler_partition().is_some() {
            return Err(Diagnostic::error(
                "Product input source copy requires its exact semantic owner",
            )
            .with_source_span(selection.source_span()));
        }
        if !matches!(
            selection.kind(),
            Kind::StaticArgument
                | Kind::Conformance
                | Kind::TypeReference
                | Kind::StaticPathSegment
        ) {
            return Err(Diagnostic::error(
                "Product input retains a non-input declaration selection",
            )
            .with_source_span(selection.source_span()));
        }
        let selected = retained
            .selected_symbol(typed, arguments)
            .map_err(|reason| {
                Diagnostic::error(reason).with_source_span(selection.source_span())
            })?;
        match selection.target() {
            Target::LateBound(
                binding @ (Binding::CheckedStaticArgument
                | Binding::CheckedStaticPathSegment
                | Binding::CheckedConformance),
            ) => {
                push_consistent_resolution(
                    resolutions,
                    AuthoredSelectionResolution {
                        occurrence,
                        binding,
                        target: AuthoredSelectionResolutionTarget::Declaration(selected),
                    },
                )?;
            }
            Target::Resolved(target)
                if typed.symbols.authored_origin(target.selected_symbol()) == selected => {}
            _ => {
                return Err(Diagnostic::error(
                    "Product input disagrees with its original authored declaration selection",
                )
                .with_source_span(selection.source_span()));
            }
        }
    }
    Ok(())
}
