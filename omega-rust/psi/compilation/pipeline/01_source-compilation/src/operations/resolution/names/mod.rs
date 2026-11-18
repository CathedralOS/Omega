/*
    Symbol table construction and identity assignment.

    `assign_symbols` builds the table from every declaration, then assigns
    symbols to top-level references, type references, propositions, measures,
    contracts, domain facts, and statements. The seeded variant extends a
    retained base table with a later stratum in place. Static module calls
    normalize here once module namespaces exist.
*/

pub(crate) mod runtime_callable_scopes;

use std::sync::Arc;
use syntax_trees::identifier::JoinedSyntaxPath;

use crate::source_inputs::SourceMap;
use crate::source_resolved::SymbolResolvedTrees;

mod runtime_callable_types;
mod symbol_table;

use symbol_table::build_symbol_table;
use type_references::assign_type_reference_symbols;

mod contracts;
mod domain_facts;
mod expression_paths;
mod expressions;
mod lookup;
mod mathematical;
mod measures;
mod repeated_locals;

mod private_nested_calls;
mod propositions;
pub(crate) mod recursive_tail_calls;
pub(crate) mod requirement_namespaces;
mod scopes;
pub(crate) use scopes::{owned_conformance_namespace, owned_conformance_realization};
mod paths;
mod statements;
pub(crate) mod target_machine_families;
pub(crate) mod target_siblings;
mod targets;
mod top_level;
pub(crate) mod type_predicates;
mod type_references;

use contracts::assign_contract_reference_symbols;
use domain_facts::assign_domain_fact_symbols;
use statements::{assign_machine_signature_reference_symbols, assign_statement_reference_symbols};
use top_level::assign_top_level_symbols;

// The named entry takes precedence over the parser's implicit entry. Both the
// symbol table and the provisional conformance catalog use this selection rule.
pub(crate) fn select_machine_entry<T>(
    name: &str,
    mut state_named: impl FnMut(&str) -> Option<T>,
) -> Option<T> {
    state_named(name).or_else(|| state_named("entry"))
}

#[derive(Default)]
pub(crate) struct NamespaceDeclarations {
    text: syntax_trees::SyntaxTextOwner,
    modules: Vec<Vec<syntax_trees::identifier::Identifier>>,
    imports: Vec<Vec<syntax_trees::identifier::Identifier>>,
}

impl syntax_trees::SyntaxTextContext for NamespaceDeclarations {
    fn source_text(&self, source: arena::Handle<syntax_trees::RetainedSource>) -> &str {
        self.text.source_text(source)
    }
}

impl NamespaceDeclarations {
    pub(crate) fn record_module(
        &mut self,
        syntax: &syntax_trees::SyntaxTrees,
        path: arena::HandleSpan<syntax_trees::identifier::Identifier>,
    ) {
        self.text.retain_from(&syntax.text);
        self.modules
            .push(syntax.items.identifier_path_members(path).to_vec());
    }

    pub(crate) fn record_import(
        &mut self,
        syntax: &syntax_trees::SyntaxTrees,
        path: arena::HandleSpan<syntax_trees::identifier::Identifier>,
    ) {
        self.text.retain_from(&syntax.text);
        self.imports
            .push(syntax.items.identifier_path_members(path).to_vec());
    }

    fn record_import_selections(
        &self,
        program: &mut SymbolResolvedTrees,
    ) -> Result<(), Vec<Diagnostic>> {
        use crate::declaration_selection::{
            AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
        };
        for path in &self.imports {
            let (Some(first), Some(last)) = (path.first(), path.last()) else {
                continue;
            };
            let name = path.joined_path_in(&self.text);
            let targets = program
                .symbols
                .source_module_import_targets(first.source_span().source_id, &name);
            let span = source_text::SourceSpan::new(
                first.source_span().source_id,
                source_text::Span::new(first.source_span().span.start, last.source_span().span.end),
            );
            for target in targets {
                program
                    .record_resolved_authored_declaration_selection(
                        span,
                        AuthoredDeclarationSelectionExposure::PrivateImplementation,
                        AuthoredDeclarationSelectionKind::StaticPathSegment,
                        target,
                    )
                    .map_err(|error| {
                        vec![
                            Diagnostic::error(format!(
                                "failed to retain module import selection: {error:?}"
                            ))
                            .with_source_span(span),
                        ]
                    })?;
            }
        }
        Ok(())
    }

    /*
        Register namespace custody before all nonconstant headers are available.
        Import-target validation still runs when the complete table is installed.
    */
    pub(crate) fn register(&self, symbols: &mut SymbolTable) -> Result<(), Vec<Diagnostic>> {
        for path in &self.modules {
            let Some(first) = path.first() else {
                continue;
            };
            symbols
                .register_source_module(
                    first.source_span().source_id,
                    path.iter()
                        .map(|member| (member.as_str(&self.text), member.source_span())),
                )
                .map_err(|message| {
                    vec![Diagnostic::error(message).with_source_span(first.source_span())]
                })?;
        }
        for path in &self.imports {
            let Some(first) = path.first() else {
                continue;
            };
            let name = path.joined_path_in(&self.text);
            symbols.register_source_import(first.source_span().source_id, &name);
        }
        Ok(())
    }

    fn install(&self, symbols: &mut SymbolTable) -> Result<(), Vec<Diagnostic>> {
        self.register(symbols)?;
        for path in &self.imports {
            let Some(first) = path.first() else {
                continue;
            };
            let name = path.joined_path_in(&self.text);
            symbols
                .validate_source_module_import(first.source_span().source_id, &name)
                .map_err(|message| {
                    vec![Diagnostic::error(message).with_source_span(first.source_span())]
                })?;
        }
        Ok(())
    }
}

/*
    The full symbol-assignment orchestration over a ready table: top-level
    identity first, then generic origins, then the typed reference,
    proposition, mathematical, measure, contract, domain-fact and signature
    passes in dependency order. Both the initial and extension drivers run
    the same sequence.
*/
fn assign_program_symbols(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
    generic_origin_frontier: usize,
    runtime_callable_scopes: &mut crate::resolution::names::runtime_callable_scopes::RuntimeCallableScopes,
    runtime_callable_contracts: &[crate::source_resolved::callable_values::RuntimeCallableContractHandle],
) -> Vec<Diagnostic> {
    let mut diagnostics = assign_top_level_symbols(program, symbols);
    if let Err(error) = runtime_callable_scopes.bind_program(program) {
        diagnostics.push(error);
        return diagnostics;
    }
    if let Err(errors) =
        runtime_callable_types::assign_signatures(program, symbols, runtime_callable_contracts)
    {
        diagnostics.extend(errors);
        return diagnostics;
    }
    assign_generic_application_origin_symbols(program, symbols, generic_origin_frontier);
    assign_type_reference_symbols(program, symbols);
    type_references::assign_declaration_type_value_expression_symbols(program, symbols);
    propositions::assign_proposition_expression_symbols(program, symbols);
    mathematical::assign_mathematical_expression_symbols(program, symbols);
    measures::assign_measure_expression_symbols(program, symbols);
    if let Err(errors) = assign_contract_reference_symbols(program, symbols) {
        diagnostics.extend(errors);
    }
    assign_domain_fact_symbols(program, symbols);
    if let Err(errors) = assign_machine_signature_reference_symbols(program, symbols) {
        diagnostics.extend(errors);
    }
    diagnostics
}

/*
    The integer carriers this forest spells that have no fixed root slot:
    `u13` as a type, `7u13` as a literal suffix, `u13` qualifying a path.
    Names can only be bound to symbols that exist, so the table is given one
    builtin type per carrier before any reference is resolved. Whether a
    spelling is used as a type is not decided here: an unused symbol binds
    nothing, and a carrier this scan missed stays unresolved and is refused
    where it is used.

    An application `u<N>` or `i<N>` that the width pass left as written (its
    width is a const parameter or must be computed) names the family head,
    which is installed for the same reason.
*/
pub(crate) struct SpelledIntegers {
    carriers: Vec<numerics::integer_carrier::FixedInteger>,
    family_heads: bool,
}

impl SpelledIntegers {
    pub(crate) fn install(&self, symbols: &mut SymbolTable) {
        symbols.install_integer_types(self.carriers.iter().copied());
        if self.family_heads {
            symbols.install_integer_families();
        }
    }
}

pub(crate) fn spelled_integers(syntax: &syntax_trees::SyntaxTrees) -> SpelledIntegers {
    use syntax_trees::expression::ExpressionNode;
    use syntax_trees::types::TypeReferenceNode;
    let mut carriers = Vec::new();
    let mut spelled = |carrier: Option<numerics::integer_carrier::FixedInteger>| {
        if let Some(carrier) = carrier
            && symbols::BuiltinTypeAtom::Integer(carrier)
                .roster_ordinal()
                .is_none()
        {
            carriers.push(carrier);
        }
    };
    let mut family_heads = false;
    for (_, node) in syntax.type_references.iter_type_references() {
        match node {
            TypeReferenceNode::Named(name) => spelled(
                numerics::integer_carrier::FixedInteger::from_decimal_spelling(name.as_str(syntax)),
            ),
            TypeReferenceNode::Generic { base_name, .. } => {
                family_heads |= matches!(base_name.as_str(syntax), "u" | "i");
            }
            _ => {}
        }
    }
    for (_, node) in syntax.expressions.iter_expressions() {
        match node {
            ExpressionNode::Integer(literal) => {
                spelled(
                    literal
                        .landing()
                        .and_then(|landing| match landing.landed_type {
                            numeric_literals::LandedIntegerType::Fixed(carrier) => Some(carrier),
                            numeric_literals::LandedIntegerType::Addr => None,
                        }),
                );
            }
            ExpressionNode::Name { members, .. } => {
                /* Only a qualifier can be a type; a name alone is a value such as a local `i1`. */
                let members = syntax.expressions.identifier_path_members(*members);
                for qualifier in members.split_last().map_or(&[][..], |(_, head)| head) {
                    spelled(
                        numerics::integer_carrier::FixedInteger::from_decimal_spelling(
                            qualifier.as_str(syntax),
                        ),
                    );
                }
            }
            _ => {}
        }
    }
    carriers.sort_unstable();
    carriers.dedup();
    SpelledIntegers {
        carriers,
        family_heads,
    }
}

pub(crate) fn assign_symbols(
    program: &mut SymbolResolvedTrees,
    sources: Option<Arc<SourceMap>>,
    source_scoped_top_level_bindings: Vec<crate::source_symbols::SourceScopedTopLevelBinding>,
    const_declarations: &[crate::resolution::lowering::lowerer::PendingConstDeclaration],
    namespace_declarations: &NamespaceDeclarations,
    generic_origin_frontier: usize,
    operand_bindings: &[(
        crate::source_resolved::types::OperandLifetimeOwnerHandle,
        crate::resolution::lowering::operand_owners::DeclarationInput,
    )],
    runtime_callable_scopes: &mut crate::resolution::names::runtime_callable_scopes::RuntimeCallableScopes,
    integers: &SpelledIntegers,
) -> Result<(), Vec<Diagnostic>> {
    let mut symbols = build_symbol_table(
        program,
        sources.clone(),
        source_scoped_top_level_bindings,
        const_declarations,
        !namespace_declarations.modules.is_empty(),
    );
    integers.install(&mut symbols);
    namespace_declarations.install(&mut symbols)?;
    let has_sources = sources.is_some() || !namespace_declarations.modules.is_empty();
    let mut symbols = insert_selected_machine_children(program, symbols, sources, 0, has_sources);
    scopes::record_declaration_owners(program, &mut symbols)?;
    program.symbols = symbols;
    let runtime_callable_contracts = symbol_table::append_runtime_callable_symbols(program);
    crate::resolution::lowering::operand_owners::bind_program(program, operand_bindings)?;
    let symbols = program.symbols.clone();
    let diagnostics = assign_program_symbols(
        program,
        &symbols,
        generic_origin_frontier,
        runtime_callable_scopes,
        &runtime_callable_contracts,
    );
    namespace_declarations.record_import_selections(program)?;
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

pub(crate) fn assign_symbols_against_resolved_base(
    program: &mut SymbolResolvedTrees,
    sources: Arc<SourceMap>,
    source_scoped_top_level_bindings: Vec<crate::source_symbols::SourceScopedTopLevelBinding>,
    roots: crate::resolution::lowering::lowerer::RootWatermarks,
    const_declarations: &[crate::resolution::lowering::lowerer::PendingConstDeclaration],
    namespace_declarations: &NamespaceDeclarations,
    generic_origin_frontier: usize,
    operand_bindings: &[(
        crate::source_resolved::types::OperandLifetimeOwnerHandle,
        crate::resolution::lowering::operand_owners::DeclarationInput,
    )],
    runtime_callable_scopes: &mut crate::resolution::names::runtime_callable_scopes::RuntimeCallableScopes,
    integers: &SpelledIntegers,
) -> Result<(), Vec<Diagnostic>> {
    symbol_table::extend_symbol_table(
        program,
        sources.clone(),
        source_scoped_top_level_bindings,
        roots,
        const_declarations,
    );
    integers.install(&mut program.symbols);
    namespace_declarations.install(&mut program.symbols)?;
    let table = std::mem::take(&mut program.symbols);
    program.symbols =
        insert_selected_machine_children(program, table, Some(sources), roots.machines, true);
    let mut symbols = program.symbols.clone();
    scopes::record_declaration_owners(program, &mut symbols)?;
    program.symbols = symbols;
    let runtime_callable_contracts = symbol_table::append_runtime_callable_symbols(program);
    crate::resolution::lowering::operand_owners::bind_program(program, operand_bindings)?;
    let symbols = program.symbols.clone();
    let diagnostics = assign_program_symbols(
        program,
        &symbols,
        generic_origin_frontier,
        runtime_callable_scopes,
        &runtime_callable_contracts,
    );
    namespace_declarations.record_import_selections(program)?;
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/*
    Constant initializers have declaration-source lookup but no caller locals or
    receiver. Reuse the ordinary expression resolver before copying a selected
    initializer into any body; use-site namespaces never resolve its constructors.
*/
pub(crate) fn assign_constant_expression_symbols(
    program: &mut SymbolResolvedTrees,
    initializers: impl IntoIterator<Item = crate::source_resolved::expression::ExpressionHandle>,
) {
    let attached_machines = scopes::attached_machines(program);
    let binder_roles = scopes::BinderRoles::of(program, &program.symbols);
    let declarations = &mut program.tables.declarations;
    let mut scope = scopes::MachineScope {
        attached_machines: &attached_machines,
        binder_roles: &binder_roles,
        resolution_indexes: &program.resolution_indexes,
        symbol: symbols::SymbolHandle::invalid(),
        lexical_requirements: scopes::LexicalRequirements::none(),
        type_parameters: &[],
        attached_data_symbol: symbols::SymbolHandle::invalid(),
        inherited_data_members: None,
        owned_data: &[],
        prior_statements: &[],
        contract_result_data: symbols::SymbolHandle::invalid(),
        data_definitions: &program.roots.data_definitions,
        type_aliases: &program.roots.type_aliases,
        conformances: &program.roots.conformances,
        traits: &program.roots.traits,
        operand_owners: &program.tables.types.operand_lifetime_owners,
        data_members: &declarations.data_members,
        data_type_parameters: &declarations.data_type_parameters,
        data_payload_fields: &declarations.data_payload_fields,
        type_constraints: &program.tables.types.constraints,
    };
    for initializer in initializers {
        scope.symbol = program
            .roots
            .const_declarations
            .iter()
            .find(|declaration| {
                declaration.initializer == initializer
                    || declaration.authored_initializer == initializer
            })
            .map_or_else(symbols::SymbolHandle::invalid, |declaration| {
                declaration.symbol
            });
        expressions::assign_expression_table_symbols(
            &program.symbols,
            &scope,
            &[],
            symbols::SymbolHandle::invalid(),
            &mut program.tables.bodies.expressions,
            &mut declarations.child_type_references,
            initializer,
        );
    }
}

pub(crate) use lookup::{
    DomainMembership, MembershipSelection, builtin_type_atom_by_symbol_name,
    constructor_prefix_is_value, constructor_type, domain_name_reaches, membership_domain_lookup,
    membership_selection, qualified_payload_free_case_type,
};

/*
    Build the symbol table and give every reference its identity, then settle
    the static module calls and outcome-specific contracts that need it. A
    seeded lowerer extends its base's table in place.
*/
pub(crate) fn assign(
    lowerer: &mut Lowerer,
    syntax: &syntax_trees::SyntaxTrees,
) -> Result<(), Vec<Diagnostic>> {
    assign_declarations(lowerer, syntax)?;
    let machines = lowerer
        .symbol_resolved_trees
        .machines
        .iter()
        .map(|machine| machine.symbol)
        .collect::<Vec<_>>();
    assign_body_symbols(lowerer, &machines)
}

/*
    Allocate and resolve declaration identities before any body-local symbol
    batch. Both eager resolution and retained-body completion use this boundary.
*/
pub(crate) fn assign_declarations(
    lowerer: &mut Lowerer,
    syntax: &syntax_trees::SyntaxTrees,
) -> Result<(), Vec<Diagnostic>> {
    let integers = spelled_integers(syntax);
    let operand_bindings = crate::resolution::lowering::operand_owners::bindings(lowerer)?;
    let seeded_roots = lowerer.seed.as_ref().map(|seed| seed.roots);
    match seeded_roots {
        None => assign_symbols(
            &mut lowerer.symbol_resolved_trees,
            lowerer.sources.take(),
            std::mem::take(&mut lowerer.source_scoped_top_level_bindings),
            &lowerer.pending_const_declarations,
            &lowerer.namespace_declarations,
            lowerer.bound_generic_application_origins,
            &operand_bindings,
            &mut lowerer.runtime_callable_scopes,
            &integers,
        )?,
        Some(roots) => {
            let sources = lowerer.sources.take().ok_or_else(|| {
                vec![Diagnostic::error(
                    "seeded symbol resolution requires retained source custody",
                )]
            })?;
            assign_symbols_against_resolved_base(
                &mut lowerer.symbol_resolved_trees,
                sources,
                std::mem::take(&mut lowerer.source_scoped_top_level_bindings),
                roots,
                &lowerer.pending_const_declarations,
                &lowerer.namespace_declarations,
                lowerer.bound_generic_application_origins,
                &operand_bindings,
                &mut lowerer.runtime_callable_scopes,
                &integers,
            )?;
        }
    }
    lowerer.bound_generic_application_origins = lowerer
        .symbol_resolved_trees
        .tables
        .types
        .generic_application_origins
        .len();
    contracts::finalize_outcome_specific_contract_symbols(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_outcome_specific_contracts,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    Ok(())
}

/*
    Complete selected bodies against their existing declaration symbols. New
    locals and synthetic state signatures append; retained bodies and headers
    never have their symbols reallocated.
*/
pub(crate) fn assign_body_symbols(
    lowerer: &mut Lowerer,
    selected_machines: &[symbols::SymbolHandle],
) -> Result<(), Vec<Diagnostic>> {
    symbol_table::append_body_symbols(&mut lowerer.symbol_resolved_trees, selected_machines)?;
    let runtime_callable_contracts =
        symbol_table::append_runtime_callable_symbols(&mut lowerer.symbol_resolved_trees);
    lowerer
        .runtime_callable_scopes
        .bind_program(&mut lowerer.symbol_resolved_trees)
        .map_err(|error| vec![error])?;
    let symbols = lowerer.symbol_resolved_trees.symbols.clone();
    runtime_callable_types::assign_signatures(
        &mut lowerer.symbol_resolved_trees,
        &symbols,
        &runtime_callable_contracts,
    )?;
    assign_generic_application_origin_symbols(
        &mut lowerer.symbol_resolved_trees,
        &symbols,
        lowerer.bound_generic_application_origins,
    );
    lowerer.bound_generic_application_origins = lowerer
        .symbol_resolved_trees
        .tables
        .types
        .generic_application_origins
        .len();
    assign_statement_reference_symbols(
        &mut lowerer.symbol_resolved_trees,
        &symbols,
        selected_machines,
    )?;
    assign_contract_reference_symbols(&mut lowerer.symbol_resolved_trees, &symbols)?;
    contracts::finalize_outcome_specific_contract_symbols(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_outcome_specific_contracts,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    private_nested_calls::reject_private_nested_calls(&lowerer.symbol_resolved_trees)?;
    repeated_locals::reject_repeated_local_names(&lowerer.symbol_resolved_trees)
}

#[cfg(any(test, feature = "test-support"))]
pub(crate) fn assign_lexical_inputs(
    lowerer: &mut Lowerer,
    syntax: &syntax_trees::SyntaxTrees,
    demand: crate::resolution::lowering::state::LexicalInputDemand,
) -> Result<Vec<crate::resolution::selections::body_calls::CallSelectionInput>, Vec<Diagnostic>> {
    let mut inputs =
        crate::resolution::lowering::machine::lower_pending_lexical_inputs(lowerer, syntax, demand)
            .map_err(|diagnostic| vec![diagnostic])?;
    for input in &mut inputs {
        let state = lowerer
            .symbol_resolved_trees
            .machine_state(input.state)
            .symbol;
        symbol_table::append_local_symbols(
            &mut lowerer.symbol_resolved_trees.symbols,
            state,
            &mut input.locals,
        );
    }
    let runtime_callable_contracts =
        symbol_table::append_runtime_callable_symbols(&mut lowerer.symbol_resolved_trees);
    lowerer
        .runtime_callable_scopes
        .bind_program(&mut lowerer.symbol_resolved_trees)
        .map_err(|error| vec![error])?;
    let symbols = lowerer.symbol_resolved_trees.symbols.clone();
    runtime_callable_types::assign_signatures(
        &mut lowerer.symbol_resolved_trees,
        &symbols,
        &runtime_callable_contracts,
    )?;
    statements::assign_lexical_input_reference_symbols(
        &mut lowerer.symbol_resolved_trees,
        &symbols,
        &mut inputs,
    )?;
    let mut calls = Vec::new();
    for input in &mut inputs {
        for (_, expression) in input.calls.drain(..) {
            calls.push(
                crate::resolution::selections::body_calls::CallSelectionInput {
                    machine: input.machine,
                    expression,
                },
            );
        }
    }
    lowerer.resolved_lexical_inputs.extend(inputs);
    Ok(calls)
}

use crate::resolution::lowering::lowerer::Lowerer;
use crate::source_symbols::SymbolTable;
use diagnostics::Diagnostic;
use symbol_table::insert_selected_machine_children;
use type_references::assign_generic_application_origin_symbols;
pub(crate) use type_references::assign_nominal_machine_requirement_arguments;
