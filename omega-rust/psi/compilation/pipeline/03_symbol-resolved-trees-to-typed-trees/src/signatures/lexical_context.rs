//! Turn a lexical static environment into explicit ordinary generic operands.
//!
//! The original binder identities survive. No enclosing runtime parameters,
//! locals, or entry facts enter the lifted machine. Captured statics precede
//! authored statics both in the declaration telescope and at every call site.

use diagnostics::Diagnostic;
use symbol_resolved_trees as resolved;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees as typed;

pub(crate) fn enclosing_parameters<'program>(
    program: &'program resolved::SymbolResolvedTrees,
    machine: &resolved::machine::Machine,
) -> Vec<&'program resolved::data::TypeParameter> {
    let mut owners = Vec::new();
    let mut parent = machine.lexical_parent;
    while parent.is_valid() {
        let owner = program.machines.get(parent);
        owners.push(owner);
        parent = owner.lexical_parent;
    }
    owners
        .into_iter()
        .rev()
        .flat_map(|owner| program.machine_type_parameters(owner))
        .collect()
}

pub(crate) fn call_arguments(
    program: Option<&resolved::SymbolResolvedTrees>,
    target: &mut typed::TypedTrees,
    exposure: Option<
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
    >,
    callee: SymbolHandle,
    authored: &[resolved::expression::StaticMachineArgument],
) -> Result<Box<[typed::expression::StaticMachineArgument]>, Diagnostic> {
    let mut arguments = captured_arguments(program, callee);
    for argument in authored {
        arguments.push(
            crate::expressions::expression::lower_static_machine_argument(
                program, target, exposure, argument,
            )?,
        );
    }
    Ok(arguments.into_boxed_slice())
}

pub(crate) fn captured_arguments(
    program: Option<&resolved::SymbolResolvedTrees>,
    callee: SymbolHandle,
) -> Vec<typed::expression::StaticMachineArgument> {
    let Some(program) = program.filter(|_| callee.is_valid()) else {
        return Vec::new();
    };
    let owner = match program.symbols.get(callee).kind {
        SymbolKind::Machine => callee,
        SymbolKind::State => program.symbols.get(callee).parent,
        _ => return Vec::new(),
    };
    if !owner.is_valid() {
        return Vec::new();
    }
    let enclosing = program.symbols.get(owner).parent;
    if !enclosing.is_valid() || program.symbols.get(enclosing).kind != SymbolKind::Machine {
        return Vec::new();
    }
    let machine = program.machine(owner);
    machine.map_or_else(Vec::new, |machine| {
        enclosing_parameters(program, machine)
            .into_iter()
            .map(|parameter| typed::expression::StaticMachineArgument {
                symbol: parameter.symbol,
                path: vec![crate::lowerer::name::lower_name(&parameter.name)].into_boxed_slice(),
                ..Default::default()
            })
            .collect()
    })
}
