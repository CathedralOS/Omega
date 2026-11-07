//! Closed machine applications discharge the same structural equations as data.
//!
//! Ordinary provisional resolution supplies exact selected declaration handles.
//! The lowering pass records private source links while producing those handles;
//! source coordinates and leaf spellings never reconstruct call identity. This
//! preparation route combines fixed static arguments with declaration-backed
//! ordinary argument/result types before solving structural equations. It never
//! observes initializer contents or flow bounds. Open applications remain
//! obligations and reject rather than entering later inference unchecked.
//! Selected scalar constants share declaration canonicalization across direct
//! and nested arguments. Nested constructor slots consume the selected declared
//! carrier before scalar normalization erases it; direct arguments retain their
//! authored syntax for the ordinary immutable static-argument checker.

use super::{
    declaration_selection::ConstantSelection,
    generic_data::module_constants,
    type_equations::{self, EquationTemplate},
};
use crate::tables::syntax_items;
use crate::tables::syntax_type_references;
use arena::Handle;
use diagnostics::Diagnostic;
use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
use numerics::literals::{IntegerLiteral, IntegerRadix};
use symbol_resolved_trees::SymbolResolvedTrees;
use syntax_trees::identifier::JoinedSyntaxPath;
use syntax_trees::{
    SyntaxTrees,
    expression::StaticMachineArgument,
    item::{Machine, TypeParameterKind},
    types::{TypeReferenceHandle, TypeReferenceNode},
};

mod argument_types;
mod completion;
use super::generic_data::canonicalize_selected_declared_const_definition;
use super::generic_data::closed_argument_identity;
pub(crate) use argument_types::DeclaredTypeOrigin;
use argument_types::call_constraints;
use completion::complete_arguments;
use numerics::arithmetic::ArithmeticDomain;
use std::iter::once;
use symbol_resolved_trees::expression::ExpressionHandle;
use symbol_resolved_trees::expression::ExpressionNode;
use symbol_resolved_trees::statement::Statement;
use symbol_resolved_trees::types::TypeConstraint;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::ItemHandle;
use syntax_trees::statement::StatementHandle;
use syntax_trees::statement::StatementNode;
use type_equations::const_binder_envelope;

#[derive(Debug, Clone, Default, compiler_storage::Record)]
pub(crate) struct SourceLinks {
    pub(crate) declared_types: Vec<(DeclaredTypeOrigin, TypeReferenceHandle)>,
    pub(crate) declarations: Vec<(usize, Machine)>,
    pub(crate) data_declarations: Vec<(ItemHandle, usize)>,
    pub(crate) expressions: Vec<(syntax_trees::expression::ExpressionHandle, ExpressionHandle)>,
    pub(crate) statements: Vec<(StatementHandle, Handle<Statement>)>,
}

impl SourceLinks {
    pub(crate) fn same_syntax(
        &self,
        other: &Self,
        left: &syntax_trees::SyntaxTextOwner,
        right: &syntax_trees::SyntaxTextOwner,
    ) -> bool {
        use syntax_trees::SyntaxEquivalent;
        let Self {
            declared_types,
            declarations,
            data_declarations,
            expressions,
            statements,
        } = self;
        *declared_types == other.declared_types
            && *data_declarations == other.data_declarations
            && *expressions == other.expressions
            && *statements == other.statements
            && declarations.same_syntax(&other.declarations, left, right)
    }
}

pub(crate) fn required(syntax: &SyntaxTrees, base: Option<&SymbolResolvedTrees>) -> bool {
    syntax.root_items().any(|item| {
        item.as_machine()
            .is_some_and(|machine| !requirement_equations(syntax, machine).is_empty())
    }) || base.is_some_and(|base| {
        base.machines
            .iter()
            .any(|machine| machine.has_structural_type_equations)
    })
}

/// The type equations among a machine's `requires` facts. Other facts remain
/// ordinary entry obligations.
pub(crate) fn machine_type_equations(
    syntax: &SyntaxTrees,
    machine: &Machine,
) -> Vec<type_equations::TypeEquation> {
    type_equations::classify_machine_equations(
        syntax,
        syntax.items.type_parameters(machine.type_parameters),
        &machine.lifetime_parameters,
        &syntax.items.requirement_facts(machine.contracts),
    )
}

pub(crate) fn validate_declarations(
    syntax: &SyntaxTrees,
    selection: Option<&ConstantSelection<'_>>,
) -> Result<(), Vec<Diagnostic>> {
    for item in syntax.root_items() {
        let Some(machine) = item.as_machine() else {
            continue;
        };
        let equations = machine_type_equations(syntax, machine);
        if !equations.is_empty() {
            // Explicit attached calls have the same selected declaration and
            // source links as free calls, so their tuples use `complete` below.
            // Implicit operator/conformance selection still needs its own complete
            // application context; an attachment alone is not such a selection.
            if (machine.spelling.is_some() || !machine.satisfies.is_empty())
                && equations
                    .iter()
                    .any(type_equations::TypeEquation::requires_argument_recovery)
            {
                return Err(vec![Diagnostic::error("machine structural equations currently require explicit calls; operator and conformance supplies retain undisclosed equations").with_source_span(machine.name.source_span())]);
            }
            for equation in equations {
                equation
                    .validate_kind("machine")
                    .map_err(|error| vec![error])?;
            }
        }
        // Closed definitions are entry predicates, so the recovery filter
        // above does not see them. A const operand still has to fail the
        // closed-type fence before operand retention reports a mix.
        for equation in type_equations::classify_type_equations(
            syntax,
            syntax.items.type_parameters(machine.type_parameters),
            &syntax.items.requirement_facts(machine.contracts),
        ) {
            equation
                .reject_unclosed_names(syntax, selection)
                .map_err(|error| {
                    let mut diagnostic = Diagnostic::error(format!(
                        "generic machine `{}` {}",
                        machine.name.as_str(syntax),
                        error.message
                    ));
                    if let Some(span) = error.source_span {
                        diagnostic = diagnostic.with_source_span(span);
                    }
                    vec![diagnostic]
                })?;
        }
    }
    Ok(())
}

fn selected_machine(trees: &SymbolResolvedTrees, symbol: symbols::SymbolHandle) -> Option<usize> {
    let declaration = trees.symbols.get(symbol);
    let owner = match declaration.kind {
        symbols::SymbolKind::Machine => symbol,
        symbols::SymbolKind::State => declaration.parent,
        _ => return None,
    };
    trees
        .machines
        .iter()
        .position(|machine| machine.symbol == owner)
}

pub(crate) fn complete(
    syntax: &mut SyntaxTrees,
    trees: &SymbolResolvedTrees,
    sources: &SourceLinks,
    selection: &ConstantSelection<'_>,
) -> Result<(), Vec<Diagnostic>> {
    let mut completed_type_roots = Vec::new();
    for conformance in &trees.conformances {
        for row in &conformance.rows {
            if [row.realization_machine, row.realization_state]
                .into_iter()
                .any(|symbol| {
                    selected_machine(trees, symbol)
                        .is_some_and(|index| trees.machines[index].has_structural_type_equations)
                })
            {
                return Err(vec![Diagnostic::error(
                    "conformance realization retains an undisclosed machine structural equation",
                )]);
            }
        }
    }
    let binders = BinderClosures::build(trees);
    for (original, resolved) in &sources.expressions {
        match trees.tables.bodies.expressions.expression(*resolved) {
            ExpressionNode::Call(call) => {
                reject_machine_arguments(trees, &call.machine_arguments)?;
                let Some(index) = selected_machine(trees, call.target_symbol) else {
                    continue;
                };
                if !completes_call(
                    syntax,
                    trees,
                    sources,
                    &binders,
                    index,
                    &call.machine_arguments,
                )? {
                    continue;
                }
                require_closed_selected_arguments(&binders, trees, &call.machine_arguments)?;
                let syntax_trees::expression::ExpressionNode::Call(mut authored) =
                    syntax.expressions.expression(*original).clone()
                else {
                    return Err(vec![Diagnostic::error(
                        "machine equation application lost its authored call",
                    )]);
                };
                let ordinary_types =
                    call_constraints(syntax, trees, sources, call, Some(*resolved), selection);
                complete_authored_arguments(
                    syntax,
                    index,
                    &authored.target,
                    &mut authored.machine_arguments,
                    &ordinary_types.types,
                    &ordinary_types.pending_diagnostics,
                    sources,
                    selection,
                    &mut completed_type_roots,
                )?;
                syntax.expressions.replace_expression(
                    *original,
                    syntax_trees::expression::ExpressionNode::Call(authored),
                );
            }
            ExpressionNode::Name(path)
                if selected_machine(trees, path.symbol)
                    .is_some_and(|index| trees.machines[index].has_structural_type_equations) =>
            {
                return Err(vec![Diagnostic::error(
                    "machine with structural equations requires a closed explicit call; noncall selection retains an unsolved equation",
                )]);
            }
            _ => {}
        }
    }
    for (original, resolved) in &sources.statements {
        let Statement::Call(call) = trees.tables.declarations.state_statements.get(*resolved)
        else {
            continue;
        };
        reject_machine_arguments(trees, &call.machine_arguments)?;
        let Some(index) = selected_machine(trees, call.target_symbol) else {
            continue;
        };
        if !completes_call(
            syntax,
            trees,
            sources,
            &binders,
            index,
            &call.machine_arguments,
        )? {
            continue;
        }
        require_closed_selected_arguments(&binders, trees, &call.machine_arguments)?;
        let StatementNode::Call(mut authored) = syntax.statements.statement(*original).clone()
        else {
            return Err(vec![Diagnostic::error(
                "machine equation application lost its authored statement",
            )]);
        };
        let ordinary_types =
            argument_types::statement_constraints(syntax, trees, sources, call, selection);
        complete_authored_arguments(
            syntax,
            index,
            &authored.target,
            &mut authored.machine_arguments,
            &ordinary_types.types,
            &ordinary_types.pending_diagnostics,
            sources,
            selection,
            &mut completed_type_roots,
        )?;
        syntax
            .statements
            .replace_statement(*original, StatementNode::Call(authored));
    }
    // Completion may introduce nominal type applications after the compiler's
    // earlier data-normalization pass. Rejoin that same owner only after all
    // provisional source links have been consumed: matching a constructor does
    // not discharge the constructor's own facts or nested equations.
    // Retain descendants before synthesis rewrites parents to Named. A
    // materialized outer record must not conceal an unsupported inner type.
    let mut positions = Vec::new();
    for &root in &completed_type_roots {
        super::generic_data::collect_type_positions(syntax, root, &mut positions, false);
    }
    let mut warnings = Vec::new();
    super::generic_data::desugar_generic_data_instances_in(
        syntax,
        &mut warnings,
        Some(selection),
        super::generic_data::GenericDataWorkDomain::Selected {
            type_roots: &completed_type_roots,
            declarations: &[],
        },
    )?;
    // Synthesis deliberately leaves unsupported applications untouched.
    // Matching one does not establish its formation obligations.
    if positions.iter().any(|reference| {
        matches!(
            syntax.type_references.type_reference(*reference),
            TypeReferenceNode::Generic { .. }
        )
    }) {
        return Err(vec![Diagnostic::error(
            "machine equation type argument retains an unnormalized data application",
        )]);
    }
    for warning in warnings {
        eprintln!("{warning}");
    }
    Ok(())
}

/// Whether a call's static arguments go through equation completion.
///
/// A machine whose `requires` facts define a type binder completes every
/// call whose selected arguments are closed, including a partial tuple that
/// still has to recover an omitted binder. A complete tuple that only
/// forwards open type binders stays with the later type-predicate check:
/// `fixed_vec_field<Kind, i32, 4>` is valid on the arm that already has
/// `Kind == FixedVec<i32, 4>`. A complete tuple that forwards an open const
/// or value binder is rejected here. Structural matching must not read that
/// binder as a same-named closed declaration (`const Count` beside
/// `relay<const Count>`).
fn completes_call(
    syntax: &SyntaxTrees,
    trees: &SymbolResolvedTrees,
    sources: &SourceLinks,
    binders: &BinderClosures,
    index: usize,
    arguments: &[symbol_resolved_trees::expression::StaticMachineArgument],
) -> Result<bool, Vec<Diagnostic>> {
    let declares = trees.machines[index].has_structural_type_equations
        || sources
            .declarations
            .iter()
            .find(|(declaration, _)| *declaration == index)
            .is_some_and(|(_, machine)| {
                requirement_equations(syntax, machine)
                    .iter()
                    .any(type_equations::TypeEquation::requires_argument_recovery)
            });
    if !declares {
        return Ok(false);
    }
    let closure = classify_selected_arguments(binders, trees, arguments);
    if supplies_complete_tuple(trees, index, arguments) && closure == ArgumentClosure::ConstBinder {
        return Err(open_caller_binder());
    }
    Ok(closure != ArgumentClosure::TypeBinder || !supplies_complete_tuple(trees, index, arguments))
}

/// Every type equation among a machine's `requires` facts, closed definitions
/// included; completion solves omitted arguments from all of them.
fn requirement_equations(
    syntax: &SyntaxTrees,
    machine: &Machine,
) -> Vec<type_equations::TypeEquation> {
    type_equations::classify_type_equations(
        syntax,
        syntax.items.type_parameters(machine.type_parameters),
        &syntax.items.requirement_facts(machine.contracts),
    )
}

fn supplies_complete_tuple(
    trees: &SymbolResolvedTrees,
    index: usize,
    arguments: &[symbol_resolved_trees::expression::StaticMachineArgument],
) -> bool {
    let machine = &trees.machines[index];
    arguments.len() == machine.type_parameters.count() as usize + machine.conformance_bounds.len()
}

fn reject_machine_arguments(
    trees: &SymbolResolvedTrees,
    arguments: &[symbol_resolved_trees::expression::StaticMachineArgument],
) -> Result<(), Vec<Diagnostic>> {
    for argument in arguments {
        if selected_machine(trees, argument.symbol)
            .is_some_and(|index| trees.machines[index].has_structural_type_equations)
        {
            return Err(vec![Diagnostic::error(
                "machine structural equation remains unsolved in a noncall static argument",
            )]);
        }
        if let Some(application) = &argument.application {
            reject_machine_arguments(trees, &application.arguments)?;
        }
    }
    Ok(())
}

/// Syntax identity must not reinterpret an exact caller binder as a same-named
/// global declaration. Inspect the selected type tree before structural matching;
/// this is a closedness fence, not a second equation or type inference solver.
fn require_closed_selected_arguments(
    binders: &BinderClosures,
    trees: &SymbolResolvedTrees,
    arguments: &[symbol_resolved_trees::expression::StaticMachineArgument],
) -> Result<(), Vec<Diagnostic>> {
    if classify_selected_arguments(binders, trees, arguments) == ArgumentClosure::Closed {
        Ok(())
    } else {
        Err(open_caller_binder())
    }
}

fn open_caller_binder() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "machine equation retains an open caller binder; supply a closed explicit static argument",
    )]
}

/// How open a selected static tuple is. A type binder can still be discharged
/// by a later predicate or guard. A const or value binder cannot: matching
/// would treat its spelling as a closed declaration of the same name.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ArgumentClosure {
    Closed,
    TypeBinder,
    ConstBinder,
}

impl ArgumentClosure {
    fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::ConstBinder, _) | (_, Self::ConstBinder) => Self::ConstBinder,
            (Self::TypeBinder, _) | (_, Self::TypeBinder) => Self::TypeBinder,
            (Self::Closed, Self::Closed) => Self::Closed,
        }
    }
}

/// Type-parameter closure keyed by symbol arena index. Built once per
/// completion so each selected argument does not rescan the parameter arena.
struct BinderClosures {
    by_symbol: Vec<Option<ArgumentClosure>>,
}

impl BinderClosures {
    fn build(trees: &SymbolResolvedTrees) -> Self {
        let mut by_symbol = vec![None; trees.symbols.symbols().len()];
        for (_, parameter) in trees.tables.declarations.data_type_parameters.iter() {
            let index = parameter.symbol.arena_index() as usize;
            if index >= by_symbol.len() {
                continue;
            }
            by_symbol[index] = Some(match &parameter.kind {
                symbol_resolved_trees::data::TypeParameterKind::Type
                | symbol_resolved_trees::data::TypeParameterKind::Implementation => {
                    ArgumentClosure::TypeBinder
                }
                _ => ArgumentClosure::ConstBinder,
            });
        }
        Self { by_symbol }
    }

    fn get(&self, symbol: symbols::SymbolHandle) -> Option<ArgumentClosure> {
        self.by_symbol
            .get(symbol.arena_index() as usize)
            .copied()
            .flatten()
    }
}

fn classify_selected_arguments(
    binders: &BinderClosures,
    trees: &SymbolResolvedTrees,
    arguments: &[symbol_resolved_trees::expression::StaticMachineArgument],
) -> ArgumentClosure {
    let mut closure = ArgumentClosure::Closed;
    let mut pending = Vec::new();
    for argument in arguments {
        closure = closure.join(binder_closure(binders, trees, argument.symbol));
        if let Some(application) = &argument.application {
            closure = closure.join(classify_selected_arguments(
                binders,
                trees,
                &application.arguments,
            ));
        }
        if argument.type_reference.is_valid() {
            pending.push(argument.type_reference);
        }
    }
    for handle in pending {
        closure = closure.join(classify_type_reference(
            binders,
            trees,
            trees.child_type_reference(handle),
            0,
        ));
    }
    closure
}

fn classify_type_reference(
    binders: &BinderClosures,
    trees: &SymbolResolvedTrees,
    reference: &symbol_resolved_trees::types::TypeReference,
    depth: usize,
) -> ArgumentClosure {
    use symbol_resolved_trees::types::{FixedArrayLength, TypeReference};
    if depth >= 128 {
        return ArgumentClosure::ConstBinder;
    }
    match reference {
        TypeReference::AliasUse { expansion, .. } => {
            classify_type_reference(binders, trees, expansion, depth + 1)
        }
        TypeReference::Named { symbol, .. } | TypeReference::DynamicTrait { symbol, .. } => {
            binder_closure(binders, trees, *symbol)
        }
        TypeReference::Reference(reference) => classify_type_reference(
            binders,
            trees,
            trees.child_type_reference(reference.referee),
            depth + 1,
        ),
        TypeReference::Slice(slice) => classify_type_reference(
            binders,
            trees,
            trees.child_type_reference(slice.element_type),
            depth + 1,
        ),
        TypeReference::FixedArray(array) => {
            // A named extent can already select a closed declaration.
            // Preserve that selection for the shared extent reader, but
            // never reselect an open caller binder by its spelling.
            let extent = match &array.length {
                FixedArrayLength::Literal(_) => ArgumentClosure::Closed,
                FixedArrayLength::ConstParameter { symbol, .. }
                    if symbol.is_valid()
                        && trees.symbols.get(*symbol).kind == symbols::SymbolKind::Const =>
                {
                    ArgumentClosure::Closed
                }
                FixedArrayLength::ConstParameter { symbol, .. } => {
                    let extent = binder_closure(binders, trees, *symbol);
                    if extent == ArgumentClosure::Closed {
                        ArgumentClosure::ConstBinder
                    } else {
                        extent
                    }
                }
                FixedArrayLength::Expression(_) | FixedArrayLength::ConstCall { .. } => {
                    ArgumentClosure::ConstBinder
                }
            };
            extent.join(classify_type_reference(
                binders,
                trees,
                trees.child_type_reference(array.element_type),
                depth + 1,
            ))
        }
        TypeReference::Constrained(constrained) => {
            let mut closure = classify_type_reference(
                binders,
                trees,
                trees.child_type_reference(constrained.base_type),
                depth + 1,
            );
            for constraint in trees
                .tables
                .types
                .constraints
                .span_or_empty(constrained.constraints)
            {
                if let TypeConstraint::Domain(domain) = constraint {
                    for handle in domain.arguments.handles() {
                        closure = closure.join(classify_type_reference(
                            binders,
                            trees,
                            trees.child_type_reference(handle),
                            depth + 1,
                        ));
                    }
                }
            }
            closure
        }
        TypeReference::Generic(generic) => {
            let mut closure = binder_closure(binders, trees, generic.base_symbol);
            for handle in generic.arguments.handles() {
                closure = closure.join(classify_type_reference(
                    binders,
                    trees,
                    trees.child_type_reference(handle),
                    depth + 1,
                ));
            }
            closure
        }
        TypeReference::ConstExpression(_) => ArgumentClosure::ConstBinder,
        TypeReference::Unit => ArgumentClosure::Closed,
    }
}

/// A caller binder's closure. Closed names, including a selected `const`
/// declaration, are not binders. A type or implementation parameter can be
/// refined later; every other binder is a const or value the equation must
/// not recover by spelling.
fn binder_closure(
    binders: &BinderClosures,
    trees: &SymbolResolvedTrees,
    symbol: symbols::SymbolHandle,
) -> ArgumentClosure {
    if !symbol.is_valid() {
        return ArgumentClosure::Closed;
    }
    match trees.symbols.get(symbol).kind {
        symbols::SymbolKind::TypeParameter => {
            binders.get(symbol).unwrap_or(ArgumentClosure::ConstBinder)
        }
        symbols::SymbolKind::Parameter
        | symbols::SymbolKind::MachineParameter
        | symbols::SymbolKind::ConformanceParameter => ArgumentClosure::ConstBinder,
        _ => ArgumentClosure::Closed,
    }
}

/// Complete one authored call's machine-argument list and record the type
/// roots the completion minted.
fn complete_authored_arguments(
    syntax: &mut SyntaxTrees,
    index: usize,
    target: &Identifier,
    machine_arguments: &mut Box<[StaticMachineArgument]>,
    ordinary_types: &[(TypeReferenceHandle, TypeReferenceHandle)],
    pending_diagnostics: &[Diagnostic],
    sources: &SourceLinks,
    selection: &ConstantSelection<'_>,
    completed_type_roots: &mut Vec<syntax_trees::types::TypeReferenceHandle>,
) -> Result<(), Vec<Diagnostic>> {
    *machine_arguments = complete_arguments(
        syntax,
        index,
        target,
        machine_arguments,
        None,
        ordinary_types,
        sources,
        selection,
    )
    .map_err(|error| {
        once(error)
            .chain(pending_diagnostics.iter().cloned())
            .collect::<Vec<_>>()
    })?
    .0;
    completed_type_roots.extend(
        machine_arguments
            .iter()
            .map(|argument| argument.type_reference)
            .filter(|reference| reference.is_valid()),
    );
    Ok(())
}

/// A syntax-stage atom cannot grant resolved nominal carrier authority.
/// Restore its retained source operand so ordinary static-argument admission
/// checks the exact declaration and carrier. The original occurrence remains
/// the lexical selection scope even when a type reaches a different caller.
fn inferred_const_argument(
    syntax: &mut SyntaxTrees,
    reference: TypeReferenceHandle,
    selection: &ConstantSelection<'_>,
) -> Result<StaticMachineArgument, Diagnostic> {
    let reject =
        || Diagnostic::error("inferred structural constant lost its authored operand custody");
    let normalization = syntax_type_references(syntax)
        .const_argument_normalization(reference)
        .ok_or_else(reject)?
        .clone();
    if normalization.authored_expression.is_valid() {
        // A produced value retains the original normalization and its exact
        // computation. The shared type-reference lowerer owns that join.
        let type_reference = if normalization.materialized_expression.is_valid() {
            reference
        } else {
            syntax
                .type_references
                .insert(TypeReferenceNode::ConstExpression(
                    normalization.authored_expression,
                ))
        };
        return Ok(StaticMachineArgument {
            type_reference,
            path: Box::default(),
            application: None,
            const_literal: None,
            evidence_projection: None,
        });
    }
    let mut origins = syntax_type_references(syntax)
        .const_argument_origins(normalization.selections)
        .iter()
        .filter(|origin| origin.reference == normalization.reference);
    let origin = origins.next().ok_or_else(reject)?;
    if origins.next().is_some() {
        return Err(reject());
    }
    let name = selection.retained_const_name(syntax, origin)?;
    Ok(StaticMachineArgument {
        type_reference: TypeReferenceHandle::invalid(),
        path: vec![name].into_boxed_slice(),
        application: None,
        const_literal: None,
        evidence_projection: None,
    })
}

/// Expose an authored scalar const's value to equation matching without
/// replacing its source argument. The immutable static-argument checker still
/// owns the selected declaration's exact carrier and policy. Constructor slots
/// below have different custody: normalization erases their nested arguments,
/// so they must validate those arguments before constructing the type.
fn supplied_const_argument_reference(
    syntax: &mut SyntaxTrees,
    argument: &StaticMachineArgument,
    selection: &ConstantSelection<'_>,
) -> Result<TypeReferenceHandle, Diagnostic> {
    let (value, _) = scalar_const_argument(syntax, argument, selection)?;
    if matches!(
        value.decode_encoding(),
        Some(
            DecodedCanonicalConstValue::Array { .. }
                | DecodedCanonicalConstValue::Record { .. }
                | DecodedCanonicalConstValue::Variant { .. }
        )
    ) {
        let first = argument.path.first().ok_or_else(|| {
            Diagnostic::error("structural const argument lost its source selection")
        })?;
        let name = Identifier::new(argument.path.joined_path_in(syntax), first.source_span());
        return Ok(syntax
            .type_references
            .insert(TypeReferenceNode::Named(name)));
    }
    scalar_const_argument_reference(syntax, value)
}

/// Select one scalar value while retaining its declaration's type separately.
/// A Boolean keyword/atom has known Boolean authority and no declaration type.
/// Direct machine arguments keep their original syntax for immutable checking;
/// constructor normalization must consume the declared type before erasure.
fn scalar_const_argument(
    syntax: &SyntaxTrees,
    argument: &StaticMachineArgument,
    selection: &ConstantSelection<'_>,
) -> Result<(CanonicalConstValue, TypeReferenceHandle), Diagnostic> {
    let reject = |reason: &str| Diagnostic::error(format!("machine equation application {reason}"));
    if argument.type_reference.is_valid()
        || argument.application.is_some()
        || argument.evidence_projection.is_some()
        || argument.const_literal.is_some()
    {
        return Err(reject("requires a closed scalar const argument"));
    }
    let Some(first) = argument.path.first() else {
        return Err(reject("requires a selected scalar const argument"));
    };
    let name = Identifier::new(argument.path.joined_path_in(syntax), first.source_span());
    if let Some(value) = type_equations::normalized_boolean_argument(name.as_str(syntax)) {
        return Ok((
            CanonicalConstValue::boolean(value),
            TypeReferenceHandle::default(),
        ));
    }
    let definition = selection.select(syntax, &name)?.ok_or_else(|| {
        reject("requires an unambiguous selected scalar const argument")
            .with_source_span(name.source_span())
    })?;
    let value =
        canonicalize_selected_declared_const_definition(syntax, &definition, Some(selection))
            .map_err(|reason| reject(&reason).with_source_span(name.source_span()))?;
    Ok((value, definition.type_reference))
}

fn scalar_const_argument_reference(
    syntax: &mut SyntaxTrees,
    value: CanonicalConstValue,
) -> Result<TypeReferenceHandle, Diagnostic> {
    let spelling = match value.decode_encoding() {
        Some(DecodedCanonicalConstValue::Integer { value, .. }) => value.to_string(),
        Some(DecodedCanonicalConstValue::Boolean(value)) => {
            CanonicalConstValue::boolean(value).atom()
        }
        _ => {
            return Err(Diagnostic::error(
                "machine equation application requires an integer or Boolean const argument",
            ));
        }
    };
    Ok(syntax
        .type_references
        .insert(TypeReferenceNode::Named(Identifier::generated(spelling))))
}

/// The selected type slot determines how a static application is read.
/// Reuse the declaration's telescope rather than treating every static path as
/// a type, and retain each occurrence's source context for normal selection.
fn application_argument_reference(
    syntax: &mut SyntaxTrees,
    argument: &StaticMachineArgument,
    kind: &TypeParameterKind,
    selection: &ConstantSelection<'_>,
) -> Result<TypeReferenceHandle, Diagnostic> {
    let reject = |reason: &str| Diagnostic::error(format!("machine equation application {reason}"));
    if argument.evidence_projection.is_some() {
        return Err(reject("cannot use evidence as a type or integer argument"));
    }
    match kind {
        TypeParameterKind::Const { type_reference } => {
            let Some(carrier) = syntax_type_references(syntax)
                .type_reference(*type_reference)
                .as_named()
            else {
                return Err(reject("requires a builtin integer constructor parameter"));
            };
            let required_carrier =
                super::generic_data::closed_name_identity(syntax, Some(selection), carrier);
            let boolean = required_carrier
                == Some(super::generic_data::ClosedArgumentIdentity::Builtin(
                    symbols::BuiltinTypeAtom::Bool,
                ));
            if argument.const_literal.is_none() {
                let (value, declared_type) = scalar_const_argument(syntax, argument, selection)?;
                // The nested argument will become a mathematical integer or
                // canonical Boolean leaf. Its encoded payload is no authority
                // to change carriers: compare the selected declaration first.
                if declared_type.is_valid()
                    && closed_argument_identity(syntax, Some(selection), declared_type, false)
                        != required_carrier
                {
                    return Err(reject(
                        "has a constructor argument with a different declared carrier",
                    ));
                }
                match value.decode_encoding() {
                    Some(DecodedCanonicalConstValue::Boolean(_)) if boolean => {}
                    Some(DecodedCanonicalConstValue::Integer { .. })
                        if !boolean && const_binder_envelope(carrier.as_str(syntax)).is_some() => {}
                    Some(
                        DecodedCanonicalConstValue::Array { .. }
                        | DecodedCanonicalConstValue::Record { .. }
                        | DecodedCanonicalConstValue::Variant { .. },
                    ) if declared_type.is_valid() && required_carrier.is_some() => {
                        let first = argument.path.first().ok_or_else(|| {
                            reject("lost its structural constructor argument selection")
                        })?;
                        let name = Identifier::new(
                            argument.path.joined_path_in(syntax),
                            first.source_span(),
                        );
                        return Ok(syntax
                            .type_references
                            .insert(TypeReferenceNode::Named(name)));
                    }
                    _ => {
                        return Err(reject(
                            "has a constructor argument with a different scalar kind",
                        ));
                    }
                }
                return scalar_const_argument_reference(syntax, value);
            }
            if boolean {
                return Err(reject("requires a closed Boolean constructor argument"));
            }
            let literal = argument
                .const_literal
                .as_ref()
                .ok_or_else(|| reject("requires a closed integer constructor argument"))?;
            if literal.landing().is_some_and(|landing| {
                landing.landed_type.name() != carrier.as_str(syntax)
                    || landing.domain != ArithmeticDomain::Exact
            }) {
                return Err(reject(
                    "has a constructor argument with a different integer meaning",
                ));
            }
            let (minimum, maximum) = const_binder_envelope(carrier.as_str(syntax))
                .ok_or_else(|| reject("requires a builtin integer constructor parameter"))?;
            let value = literal
                .value_bignum()
                .ok_or_else(|| reject("requires an exact integer"))?;
            if value < minimum || value > maximum {
                return Err(reject(
                    "has a constructor constant outside its declared carrier",
                ));
            }
            Ok(syntax
                .type_references
                .insert(TypeReferenceNode::Named(Identifier::generated(
                    value.to_string(),
                ))))
        }
        TypeParameterKind::Type => {
            if argument.const_literal.is_some() {
                return Err(reject("mixes type and value kinds"));
            }
            if argument.type_reference.is_valid() {
                return Ok(argument.type_reference);
            }
            let Some(first) = argument.path.first() else {
                return Err(reject("requires a selected type name"));
            };
            let name = Identifier::new(argument.path.joined_path_in(syntax), first.source_span());
            let Some(application) = &argument.application else {
                return Ok(syntax
                    .type_references
                    .insert(TypeReferenceNode::Named(name)));
            };
            let definition = selection.data(syntax, &name).map_err(Diagnostic::error)?;
            if !application.lifetime_arguments.is_empty()
                || !definition.lifetime_parameters.is_empty()
            {
                return Err(reject("requires lifetime-free constructor arguments"));
            }
            let parameters = syntax_items(syntax)
                .type_parameters(definition.type_parameters)
                .to_vec();
            if parameters.len() != application.arguments.len() {
                return Err(reject("requires a complete constructor argument tuple"));
            }
            let arguments = application
                .arguments
                .iter()
                .zip(parameters)
                .map(|(argument, parameter)| {
                    application_argument_reference(syntax, argument, &parameter.kind, selection)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let arguments = syntax
                .type_references
                .insert_type_reference_handles(arguments);
            Ok(syntax.type_references.insert(TypeReferenceNode::Generic {
                base_name: name,
                lifetime_arguments: Default::default(),
                arguments,
            }))
        }
        _ => Err(reject(
            "requires type and integer const constructor parameters",
        )),
    }
}
