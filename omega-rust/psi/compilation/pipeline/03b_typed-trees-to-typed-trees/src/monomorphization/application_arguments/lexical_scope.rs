//! Tuple identity and caller availability are different judgments. An inner
//! unspecialized declaration can expose its own result binder before its
//! selected application supplies that result. Such a binder has an identity,
//! but cannot become an inferred operand of an unrelated caller. Retain only
//! references to that caller's original telescope while ordinary result
//! elaboration settles. This readiness query admits no carrier, bound or fact.

use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;
use typed_trees::references::{StaticArgumentCall, TypedReferences};
use typed_trees::static_applications::{
    StaticApplicationDeclaration, StaticMachineBinding, StaticMachineBindingValue,
};
use typed_trees::type_identity::AppliedTypeIdentityOwner;
use typed_trees::types::TypeReferenceHandle;

pub(in crate::monomorphization) fn bindings_are_in_scope(
    program: &TypedTrees,
    lexical_owner: AppliedTypeIdentityOwner<'_>,
    bindings: &[StaticMachineBinding],
) -> bool {
    let (scope_owner, declaration) = match lexical_owner {
        AppliedTypeIdentityOwner::Declaration(symbol) => (program, symbol),
        AppliedTypeIdentityOwner::Captured(scope) => (scope.owner(), scope.symbol()),
    };
    let Ok(scope) = StaticApplicationDeclaration::for_symbol(scope_owner, declaration) else {
        return false;
    };
    let mut binders = scope.binders().collect::<Vec<_>>();
    if let Some(machine) = scope_owner
        .machine(declaration)
        .or_else(|| scope_owner.machine_holding_state(declaration))
    {
        // The existing type-scope owner retains attached data parameters in
        // addition to the original callable telescope. Captured Product
        // scopes share their source symbol prefix; only payload arenas move.
        let (parameters, _) = scope_owner
            .machine_type_reference_scope(machine, |left, right| left.symbol == right.symbol);
        binders.extend(parameters.iter().map(|parameter| parameter.symbol));
    }
    let mut references = OperandReferences::default();
    references.bindings(program, bindings);
    references.symbols.into_iter().all(|symbol| {
        !symbol.is_valid()
            || !matches!(
                program.symbols.get(symbol).kind,
                SymbolKind::TypeParameter
                    | SymbolKind::MachineParameter
                    | SymbolKind::ConformanceParameter
            )
            || binders.contains(&symbol)
    })
}

#[derive(Default)]
struct OperandReferences {
    symbols: Vec<SymbolHandle>,
    expressions: Vec<ExpressionHandle>,
    types: Vec<TypeReferenceHandle>,
    applications: Vec<typed_trees::static_applications::StaticMachineApplicationHandle>,
}

impl OperandReferences {
    fn bindings(&mut self, program: &TypedTrees, bindings: &[StaticMachineBinding]) {
        for binding in bindings {
            match &binding.value {
                StaticMachineBindingValue::Type(reference)
                | StaticMachineBindingValue::Const(reference) => {
                    typed_trees::references::visit_type_reference(program, self, *reference);
                }
                StaticMachineBindingValue::RuntimeValue {
                    type_reference,
                    subject,
                } => {
                    typed_trees::references::visit_type_reference(program, self, *type_reference);
                    self.extend([*subject]);
                }
                StaticMachineBindingValue::Machine(argument)
                | StaticMachineBindingValue::Conformance(argument) => {
                    typed_trees::references::visit_static_argument(program, self, argument);
                }
            }
        }
    }
}

impl Extend<SymbolHandle> for OperandReferences {
    fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: T) {
        self.symbols.extend(symbols);
    }
}

impl TypedReferences for OperandReferences {
    fn expression(&mut self, expression: ExpressionHandle) -> bool {
        if self.expressions.contains(&expression) {
            return false;
        }
        self.expressions.push(expression);
        true
    }

    fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
        if self.types.contains(&reference) {
            return false;
        }
        self.types.push(reference);
        true
    }

    fn call_arguments(&mut self, program: &TypedTrees, call: StaticArgumentCall<'_>) -> bool {
        // The operand owns its source arguments. A called signature's own
        // quantified formals are not free binders of this caller's operand.
        for argument in call.arguments(program) {
            typed_trees::references::visit_static_argument(program, self, argument);
        }
        let application = call.application();
        if application.is_valid() && !self.applications.contains(&application) {
            self.applications.push(application);
            self.bindings(
                program,
                program
                    .expression_table
                    .static_machine_application_bindings(application),
            );
            for selected in program
                .expression_table
                .static_machine_application_selected_machines(application)
            {
                typed_trees::references::visit_static_argument(program, self, selected);
            }
        }
        false
    }
}
