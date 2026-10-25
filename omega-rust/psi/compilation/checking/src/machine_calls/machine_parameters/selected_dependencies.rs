//! Machine dependencies inside retained static argument values.
//!
//! Telescope ordinals use only the top-level argument roster. Dependency
//! closure also consumes nested application arguments and type-held selections,
//! through the representation's reference walk rather than a source-shape walk.

use symbols::SymbolHandle;
use typed_trees::expression::StaticMachineArgument;
use typed_trees::references::TypedReferences;
use typed_trees::types::TypeReferenceHandle;

pub(crate) fn machine_selection_dependency_symbols(
    program: &dyn crate::CheckingInput,
    arguments: &[StaticMachineArgument],
) -> Vec<SymbolHandle> {
    let mut dependencies = MachineSelectionDependencies {
        program,
        symbols: Vec::new(),
        types: Vec::new(),
    };
    for argument in arguments {
        typed_trees::references::visit_static_argument(program, &mut dependencies, argument);
    }
    dependencies.symbols
}

/// A completed/specification call retains source, contextual and executable
/// selections independently. Specialization may empty the live argument list;
/// the ordinary reference walk still owns every callback dependency in the
/// invocation and qualifier applications. This classifies dependencies only.
pub(crate) fn call_selection_dependency_symbols<'call>(
    program: &'call dyn crate::CheckingInput,
    call: typed_trees::references::StaticArgumentCall<'call>,
) -> Vec<SymbolHandle> {
    let mut dependencies = MachineSelectionDependencies {
        program,
        symbols: Vec::new(),
        types: Vec::new(),
    };
    let live = match call {
        typed_trees::references::StaticArgumentCall::Expression(_, call) => &call.machine_arguments,
        typed_trees::references::StaticArgumentCall::Statement(call) => &call.machine_arguments,
    };
    for argument in live.iter().chain(call.arguments(program)) {
        typed_trees::references::visit_static_argument(program, &mut dependencies, argument);
    }
    for application in call.applications() {
        typed_trees::references::visit_static_machine_bindings(
            program,
            &mut dependencies,
            application,
        );
    }
    dependencies.symbols
}

/// Specialization erases executable static operands, not proof dependencies.
/// The declaration receipt retains their complete typed binding application,
/// including unused selections, nested applications and type-held machines.
pub(crate) fn specialization_selection_dependency_symbols(
    program: &dyn crate::CheckingInput,
    target: SymbolHandle,
) -> Option<Vec<SymbolHandle>> {
    let mut dependencies = MachineSelectionDependencies {
        program,
        symbols: Vec::new(),
        types: Vec::new(),
    };
    let owner = program.machine_by_symbol(target)?;
    program.executable_application_template(owner.symbol).ok()?;
    let mut applications = Vec::new();
    if let Some(application) = program.machine_specialization(owner.symbol) {
        applications.push(application.binding_application);
    }
    for (_, captured) in program
        .captured_machine_applications
        .iter()
        .filter(|(_, captured)| captured.instance == owner.symbol)
    {
        applications.push(captured.binding_application);
        applications.push(captured.supplying_application);
        typed_trees::references::visit_closed_conformance(
            program,
            &mut dependencies,
            &captured.conformance_application,
        );
    }
    for application in applications {
        if !application.is_valid() {
            return None;
        }
        for argument in program
            .expression_table
            .static_machine_application_arguments(application)
        {
            typed_trees::references::visit_static_argument(program, &mut dependencies, argument);
        }
        typed_trees::references::visit_static_machine_bindings(
            program,
            &mut dependencies,
            application,
        );
    }
    Some(dependencies.symbols)
}

struct MachineSelectionDependencies<'program> {
    program: &'program dyn crate::CheckingInput,
    symbols: Vec<SymbolHandle>,
    types: Vec<TypeReferenceHandle>,
}

impl Extend<SymbolHandle> for MachineSelectionDependencies<'_> {
    fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: T) {
        for symbol in symbols {
            if super::is_machine_selection_symbol(self.program, symbol)
                && !self.symbols.contains(&symbol)
            {
                self.symbols.push(symbol);
            }
        }
    }
}

impl TypedReferences for MachineSelectionDependencies<'_> {
    fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
        if self.types.contains(&reference) {
            return false;
        }
        self.types.push(reference);
        true
    }
}
