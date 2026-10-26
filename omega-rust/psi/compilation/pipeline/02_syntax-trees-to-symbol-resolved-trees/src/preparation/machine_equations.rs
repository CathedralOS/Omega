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
use arena::Handle;
use diagnostics::Diagnostic;
use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
use numerics::literals::{IntegerLiteral, IntegerRadix};
use symbol_resolved_trees::SymbolResolvedTrees;
use syntax_trees::identifier::JoinedSyntaxPath;
use syntax_trees::{
    SyntaxTrees,
    expression::StaticMachineArgument,
    item::{Item, Machine, TypeParameterKind},
    types::{TypeReferenceHandle, TypeReferenceNode},
};

mod argument_types;
mod completion;
pub(crate) use argument_types::DeclaredTypeOrigin;
use completion::complete_arguments;

#[derive(Debug, Clone, Default)]
pub(crate) struct SourceLinks {
    pub(crate) declared_types: Vec<(DeclaredTypeOrigin, TypeReferenceHandle)>,
    pub(crate) declarations: Vec<(usize, Machine)>,
    pub(crate) data_declarations: Vec<(syntax_trees::item::ItemHandle, usize)>,
    pub(crate) expressions: Vec<(
        syntax_trees::expression::ExpressionHandle,
        symbol_resolved_trees::expression::ExpressionHandle,
    )>,
    pub(crate) statements: Vec<(
        syntax_trees::statement::StatementHandle,
        Handle<symbol_resolved_trees::statement::Statement>,
    )>,
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
    syntax
        .root_items()
        .any(|item| matches!(item, Item::Machine(machine) if !machine.where_facts.is_empty()))
        || base.is_some_and(|base| {
            base.machines
                .iter()
                .any(|machine| machine.has_structural_type_equations)
        })
}

pub(crate) fn validate_declarations(syntax: &SyntaxTrees) -> Result<(), Vec<Diagnostic>> {
    for item in syntax.root_items() {
        let Some(machine) = item.as_machine() else {
            continue;
        };
        if machine.where_facts.is_empty() {
            continue;
        }
        // Explicit attached calls have the same selected declaration and
        // source links as free calls, so their tuples use `complete` below.
        // Implicit operator/conformance selection still needs its own complete
        // application context; an attachment alone is not such a selection.
        if machine.spelling.is_some() || !machine.satisfies.is_empty() {
            return Err(vec![Diagnostic::error("machine structural equations currently require explicit calls; operator and conformance supplies retain undisclosed equations").with_source_span(machine.name.source_span())]);
        }
        if machine.type_parameters.is_empty() {
            return Err(vec![
                Diagnostic::error("machine structural equations require a generic declaration")
                    .with_source_span(machine.name.source_span()),
            ]);
        }
        let equations = type_equations::classify_type_equations(
            syntax,
            syntax.items.type_parameters(machine.type_parameters),
            syntax.items.proof_facts(machine.where_facts),
        );
        if equations.len() != machine.where_facts.count() as usize {
            return Err(vec![Diagnostic::error("machine where clause requires a structural type equation or a named conformance bound").with_source_span(machine.name.source_span())]);
        }
        for equation in equations {
            equation
                .validate_kind("machine")
                .map_err(|error| vec![error])?;
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
    for (original, resolved) in &sources.expressions {
        match trees.tables.bodies.expressions.expression(*resolved) {
            symbol_resolved_trees::expression::ExpressionNode::Call(call) => {
                reject_machine_arguments(trees, &call.machine_arguments)?;
                let Some(index) = selected_machine(trees, call.target_symbol) else {
                    continue;
                };
                if !trees.machines[index].has_structural_type_equations {
                    continue;
                }
                require_closed_selected_arguments(trees, &call.machine_arguments)?;
                let syntax_trees::expression::ExpressionNode::Call(mut authored) =
                    syntax.expressions.expression(*original).clone()
                else {
                    return Err(vec![Diagnostic::error(
                        "machine equation application lost its authored call",
                    )]);
                };
                let ordinary_types = argument_types::call_constraints(
                    syntax,
                    trees,
                    sources,
                    call,
                    Some(*resolved),
                    selection,
                );
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
            symbol_resolved_trees::expression::ExpressionNode::Name(path)
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
        let symbol_resolved_trees::statement::Statement::Call(call) =
            trees.tables.declarations.state_statements.get(*resolved)
        else {
            continue;
        };
        reject_machine_arguments(trees, &call.machine_arguments)?;
        let Some(index) = selected_machine(trees, call.target_symbol) else {
            continue;
        };
        if !trees.machines[index].has_structural_type_equations {
            continue;
        }
        require_closed_selected_arguments(trees, &call.machine_arguments)?;
        let syntax_trees::statement::StatementNode::Call(mut authored) =
            syntax.statements.statement(*original).clone()
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
        syntax.statements.replace_statement(
            *original,
            syntax_trees::statement::StatementNode::Call(authored),
        );
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
    trees: &SymbolResolvedTrees,
    arguments: &[symbol_resolved_trees::expression::StaticMachineArgument],
) -> Result<(), Vec<Diagnostic>> {
    use symbol_resolved_trees::types::{FixedArrayLength, TypeReference};
    let is_binder = |symbol| {
        matches!(
            trees.symbols.get(symbol).kind,
            symbols::SymbolKind::TypeParameter
                | symbols::SymbolKind::Parameter
                | symbols::SymbolKind::MachineParameter
                | symbols::SymbolKind::ConformanceParameter
        )
    };
    let reject = || {
        vec![Diagnostic::error(
            "machine equation retains an open caller binder; supply a closed explicit static argument",
        )]
    };
    let mut pending = Vec::new();
    for argument in arguments {
        if is_binder(argument.symbol) {
            return Err(reject());
        }
        if let Some(application) = &argument.application {
            require_closed_selected_arguments(trees, &application.arguments)?;
        }
        if argument.type_reference.is_valid() {
            pending.push(argument.type_reference);
        }
    }
    let mut visited = Vec::new();
    while let Some(handle) = pending.pop() {
        if visited.contains(&handle) {
            continue;
        }
        visited.push(handle);
        match trees.child_type_reference(handle) {
            TypeReference::Named { symbol, .. } | TypeReference::DynamicTrait { symbol, .. } => {
                if is_binder(*symbol) {
                    return Err(reject());
                }
            }
            TypeReference::Reference(reference) => pending.push(reference.referee),
            TypeReference::Slice(slice) => pending.push(slice.element_type),
            TypeReference::FixedArray(array) => {
                // A named extent can already select a closed declaration.
                // Preserve that selection for the shared extent reader, but
                // never reselect an open caller binder by its spelling.
                match &array.length {
                    FixedArrayLength::Literal(_) => {}
                    FixedArrayLength::ConstParameter { symbol, .. }
                        if symbol.is_valid()
                            && trees.symbols.get(*symbol).kind == symbols::SymbolKind::Const => {}
                    _ => return Err(reject()),
                }
                pending.push(array.element_type);
            }
            TypeReference::Constrained(constrained) => {
                pending.push(constrained.base_type);
                for constraint in trees
                    .tables
                    .types
                    .constraints
                    .span_or_empty(constrained.constraints)
                {
                    if let symbol_resolved_trees::types::TypeConstraint::Domain(domain) = constraint
                    {
                        append_type_children(domain.arguments, &mut pending);
                    }
                }
            }
            TypeReference::Generic(generic) => {
                if is_binder(generic.base_symbol) {
                    return Err(reject());
                }
                append_type_children(generic.arguments, &mut pending);
            }
            TypeReference::ConstExpression(_) => return Err(reject()),
            TypeReference::Unit => {}
        }
    }
    Ok(())
}

fn append_type_children(
    span: arena::HandleSpan<symbol_resolved_trees::types::TypeReference>,
    pending: &mut Vec<Handle<symbol_resolved_trees::types::TypeReference>>,
) {
    pending.extend(span.handles());
}

/// Complete one authored call's machine-argument list and record the type
/// roots the completion minted.
fn complete_authored_arguments(
    syntax: &mut SyntaxTrees,
    index: usize,
    target: &syntax_trees::identifier::Identifier,
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
        std::iter::once(error)
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
    let normalization = syntax
        .type_references
        .const_argument_normalization(reference)
        .ok_or_else(reject)?
        .clone();
    if normalization.authored_expression.is_valid() {
        let type_reference = syntax
            .type_references
            .insert(TypeReferenceNode::ConstExpression(
                normalization.authored_expression,
            ));
        return Ok(StaticMachineArgument {
            type_reference,
            path: Box::default(),
            application: None,
            const_literal: None,
            evidence_projection: None,
        });
    }
    let mut origins = syntax
        .type_references
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
        let name = syntax_trees::identifier::Identifier::new(
            argument.path.joined_path_in(syntax),
            first.source_span(),
        );
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
    let name = syntax_trees::identifier::Identifier::new(
        argument.path.joined_path_in(syntax),
        first.source_span(),
    );
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
    let value = super::generic_data::canonicalize_selected_declared_const_definition(
        syntax,
        &definition,
        Some(selection),
    )
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
    Ok(syntax.type_references.insert(TypeReferenceNode::Named(
        syntax_trees::identifier::Identifier::generated(spelling),
    )))
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
            let Some(carrier) = syntax
                .type_references
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
                    && super::generic_data::closed_argument_identity(
                        syntax,
                        Some(selection),
                        declared_type,
                        false,
                    ) != required_carrier
                {
                    return Err(reject(
                        "has a constructor argument with a different declared carrier",
                    ));
                }
                match value.decode_encoding() {
                    Some(DecodedCanonicalConstValue::Boolean(_)) if boolean => {}
                    Some(DecodedCanonicalConstValue::Integer { .. })
                        if !boolean
                            && type_equations::const_binder_envelope(carrier.as_str(syntax))
                                .is_some() => {}
                    Some(
                        DecodedCanonicalConstValue::Array { .. }
                        | DecodedCanonicalConstValue::Record { .. }
                        | DecodedCanonicalConstValue::Variant { .. },
                    ) if declared_type.is_valid() && required_carrier.is_some() => {
                        let first = argument.path.first().ok_or_else(|| {
                            reject("lost its structural constructor argument selection")
                        })?;
                        let name = syntax_trees::identifier::Identifier::new(
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
                    || landing.domain != numerics::arithmetic::ArithmeticDomain::Exact
            }) {
                return Err(reject(
                    "has a constructor argument with a different integer meaning",
                ));
            }
            let (minimum, maximum) = type_equations::const_binder_envelope(carrier.as_str(syntax))
                .ok_or_else(|| reject("requires a builtin integer constructor parameter"))?;
            let value = literal
                .value_bignum()
                .ok_or_else(|| reject("requires an exact integer"))?;
            if value < minimum || value > maximum {
                return Err(reject(
                    "has a constructor constant outside its declared carrier",
                ));
            }
            Ok(syntax.type_references.insert(TypeReferenceNode::Named(
                syntax_trees::identifier::Identifier::generated(value.to_string()),
            )))
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
            let name = syntax_trees::identifier::Identifier::new(
                argument.path.joined_path_in(syntax),
                first.source_span(),
            );
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
            let parameters = syntax
                .items
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
