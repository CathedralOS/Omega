//! Runtime value indices do not provide inline fixed-array storage.
//!
//! Follow declaration-owned generic applications, not a producing call's
//! spelling or body. A reported symbol is a runtime extent in by-value
//! storage. Absence is not layout authority: ordinary shape, placement and
//! provider checks remain responsible for complete representability.

use symbols::{SymbolHandle, SymbolKind};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

#[derive(Clone, PartialEq, Eq)]
struct Binding {
    parameter: SymbolHandle,
    argument: TypeReferenceHandle,
    outer: usize,
}

pub fn runtime_inline_array_extent(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    Search {
        program,
        remaining: 4096,
        visited: Vec::new(),
    }
    .ty(reference, &[])
}

struct Search<'a> {
    program: &'a dyn crate::CheckingInput,
    remaining: usize,
    visited: Vec<(TypeReferenceHandle, Vec<Binding>)>,
}
impl Search<'_> {
    fn ty(&mut self, reference: TypeReferenceHandle, bindings: &[Binding]) -> Option<SymbolHandle> {
        self.remaining = self.remaining.checked_sub(1)?;
        if !reference.is_valid() || self.visited.contains(&(reference, bindings.to_vec())) {
            return None;
        }
        self.visited.push((reference, bindings.to_vec()));
        let program = self.program;
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. } => None,
            TypeReferenceNode::Constrained { base_type, .. } => self.ty(*base_type, bindings),
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                let extent = if program.closed_fixed_array_length(reference).is_some() {
                    None
                } else {
                    match length {
                        FixedArrayLength::ConstParameter { symbol, .. } => {
                            self.value_symbol(*symbol, bindings)
                        }
                        FixedArrayLength::Expression(value) => self.value(*value, bindings),
                        FixedArrayLength::Literal(_) | FixedArrayLength::ConstCall { .. } => None,
                    }
                };
                extent.or_else(|| self.ty(*element_type, bindings))
            }
            TypeReferenceNode::Named { symbol, .. } => {
                if let Some(binding) = bindings
                    .iter()
                    .rev()
                    .find(|binding| binding.parameter == *symbol)
                {
                    return self.ty(binding.argument, &bindings[..binding.outer]);
                }
                let data = program.data_definition_by_symbol(*symbol)?;
                if self.independent_storage(data.symbol) {
                    return None;
                }
                if let Some(origin) = data.generic_instance.filter(|origin| *origin != reference) {
                    return self.ty(origin, bindings);
                }
                self.fields(data, bindings)
            }
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } => {
                let data = program.data_definition_by_symbol(*base_symbol)?;
                if self.independent_storage(data.symbol) {
                    return None;
                }
                let parameters = program.data_type_parameters(data);
                let arguments = program
                    .type_reference_table
                    .type_reference_handles(*arguments);
                if parameters.len() != arguments.len() {
                    return None;
                }
                let mut applied = bindings.to_vec();
                let outer = applied.len();
                applied.extend(
                    parameters
                        .iter()
                        .zip(arguments)
                        .map(|(parameter, argument)| Binding {
                            parameter: parameter.symbol,
                            argument: *argument,
                            outer,
                        }),
                );
                self.fields(data, &applied)
            }
            _ => None,
        }
    }

    fn independent_storage(&self, data: SymbolHandle) -> bool {
        self.program.plan_laid_layout(data).is_some()
            || self
                .program
                .placed_view_plans
                .iter()
                .any(|view| view.data_symbol == data)
    }

    fn fields(
        &mut self,
        data: &typed_trees::data::DataDefinition,
        bindings: &[Binding],
    ) -> Option<SymbolHandle> {
        let fields = self
            .program
            .data_member_fields(self.program.data_members(data))
            .filter(|field| !field.relevance.is_erased())
            .map(|field| field.type_reference)
            .collect::<Vec<_>>();
        fields
            .into_iter()
            .find_map(|field| self.ty(field, bindings))
    }

    fn value(
        &mut self,
        reference: TypeReferenceHandle,
        bindings: &[Binding],
    ) -> Option<SymbolHandle> {
        self.remaining = self.remaining.checked_sub(1)?;
        if self
            .program
            .static_const_type_reference_identity(reference)
            .is_some()
        {
            return None;
        }
        match self.program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } => self.value_symbol(*symbol, bindings),
            TypeReferenceNode::ConstExpression(expression) => {
                self.expression(*expression, bindings)
            }
            TypeReferenceNode::Constrained { base_type, .. } => self.value(*base_type, bindings),
            _ => None,
        }
    }

    fn value_symbol(&mut self, symbol: SymbolHandle, bindings: &[Binding]) -> Option<SymbolHandle> {
        if let Some(binding) = bindings
            .iter()
            .rev()
            .find(|binding| binding.parameter == symbol)
        {
            return self.value(binding.argument, &bindings[..binding.outer]);
        }
        (symbol.is_valid()
            && matches!(
                self.program.symbols.get(symbol).kind,
                SymbolKind::Parameter | SymbolKind::Local | SymbolKind::Field
            ))
        .then_some(symbol)
    }

    fn expression(
        &mut self,
        expression: ExpressionHandle,
        bindings: &[Binding],
    ) -> Option<SymbolHandle> {
        self.remaining = self.remaining.checked_sub(1)?;
        if !expression.is_valid()
            || self
                .program
                .expression_table
                .constant_integer_value(expression)
                .is_some()
        {
            return None;
        }
        let node = self.program.expression_table.expression(expression);
        if let ExpressionNode::Name(path) = node {
            return self.value_symbol(path.symbol, bindings);
        }
        let mut children = Vec::new();
        crate::expression_traversal::children(self.program, node, |child| children.push(child));
        children
            .into_iter()
            .find_map(|child| self.expression(child, bindings))
    }
}

/// Check fully prepared executable storage. Templates retain their universal
/// source contract; each concrete specialization is checked in its own scope.
pub fn validate_prepared_inline_storage(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) {
    let proof = crate::proof_only_classification(program);
    for machine in program.machines() {
        if !program.machine_type_parameters(machine).is_empty()
            || proof.is_proof_machine(program, machine)
            || crate::mathematical_applications::is_indexed_logical_machine(program, machine)
            || crate::value_custody::content_projections::is_content_projection_machine(
                program, machine,
            )
        {
            continue;
        }
        for state in program.machine_states(machine) {
            let mut roots = vec![(state.return_type, "result".to_owned())];
            roots.extend(
                program
                    .state_parameters(state)
                    .iter()
                    .filter(|parameter| !parameter.is_const && !parameter.relevance.is_erased())
                    .map(|parameter| {
                        (
                            parameter.type_reference,
                            format!("parameter `{}`", parameter.name),
                        )
                    }),
            );
            roots.extend(
                program
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .filter_map(|statement| statement.as_local_data())
                    .filter(|local| !local.relevance.is_erased())
                    .map(|local| (local.type_reference, format!("local `{}`", local.name))),
            );
            for (reference, role) in roots {
                if let Some(extent) = runtime_inline_array_extent(program, reference) {
                    diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "machine `{}` state `{}` {role} uses runtime value `{}` for an inline fixed-array extent; a value index cannot determine a static inline layout; use a const extent, an authored sum or explicit storage/descriptor",
                        machine.name, state.name, program.symbols.name(extent),
                    )));
                }
            }
        }
    }
}
