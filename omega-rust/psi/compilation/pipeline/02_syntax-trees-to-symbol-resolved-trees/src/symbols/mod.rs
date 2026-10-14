//! Symbol table construction and identity assignment.
//!
//! `assign_symbols` builds the table from every declaration, then assigns
//! symbols to top-level references, type references, propositions, measures,
//! contracts, domain facts, and statements. The seeded variant extends a
//! retained base table with a later stratum in place. Static module calls
//! normalize here once module namespaces exist.

use std::sync::Arc;

use source::SourceMap;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::JoinedPath;

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
mod propositions;
mod scope;
mod scoped_paths;
mod statements;
pub(crate) mod target_machine_families;
pub(crate) mod target_siblings;
mod targets;
mod top_level;
mod type_references;

use contracts::assign_contract_reference_symbols;
use domain_facts::assign_domain_fact_symbols;
use statements::{assign_machine_signature_reference_symbols, assign_statement_reference_symbols};
use top_level::assign_top_level_symbols;

pub(crate) fn normalize_static_module_calls(
    program: &mut SymbolResolvedTrees,
    calls: &[(
        symbol_resolved_trees::expression::ExpressionHandle,
        Vec<symbol_resolved_trees::name::DiagnosticName>,
    )],
    statement_calls: &[(
        source::SourceSpan,
        Vec<symbol_resolved_trees::name::DiagnosticName>,
    )],
) {
    use symbol_resolved_trees::expression::{ExpressionHandle, ExpressionNode};
    use symbols::{JoinedPath, SymbolHandle, SymbolKind};
    for (expression, path) in calls {
        let Some((target, receiver)) = path.split_last() else {
            continue;
        };
        let receiver_name = receiver.joined_path();
        let reference = lookup::diagnostic_path_source_span(path);
        let name = format!("{receiver_name}::{}", target.as_str());
        // The fixed `core::*` mathematical declarations own their exact
        // spellings ahead of any authored module lookup: `core` is the
        // vocabulary's fixed namespace — the declarations install as root
        // children, never under a `Module` symbol — so an authored module
        // cannot claim one of these names at a call site, the same law the
        // type-reference resolver applies. This arm is also what reaches
        // proposition-body and mathematical-term calls: their resolver
        // leaves receiver-qualified targets for this pass.
        let fixed = symbols::FixedCoreDeclaration::from_name(&name)
            .and_then(|declaration| program.symbols.fixed_core_declaration_symbol(declaration))
            .unwrap_or_else(SymbolHandle::invalid);
        if fixed.is_valid() {
            if let ExpressionNode::Call(call) = program
                .tables
                .bodies
                .expressions
                .expression_mut(*expression)
            {
                call.receiver = ExpressionHandle::invalid();
                call.target =
                    symbol_resolved_trees::name::DiagnosticName::from_str(&name, reference);
                call.target_symbol = fixed;
            }
            continue;
        }
        let namespace = program
            .symbols
            .find_top_level_by_name_and_kinds_from_source(
                &receiver_name,
                &[SymbolKind::Module],
                reference,
            );
        if !namespace.is_some_and(|symbol| program.symbols.get(symbol).kind == SymbolKind::Module) {
            continue;
        }
        let selected = program
            .symbols
            .find_top_level_by_name_and_kinds_from_source(
                &name,
                targets::SOURCE_CALL_KINDS,
                reference,
            );
        let selected = selected
            .map(|symbol| {
                if program.symbols.get(symbol).kind == SymbolKind::Machine {
                    program
                        .symbols
                        .find_child_by_name_and_kind(symbol, "entry", SymbolKind::State)
                        .unwrap_or_else(SymbolHandle::invalid)
                } else {
                    symbol
                }
            })
            .unwrap_or_else(SymbolHandle::invalid);
        let original_receiver = match program.tables.bodies.expressions.expression(*expression) {
            ExpressionNode::Call(call) => call.receiver,
            _ => continue,
        };
        // Declaration indexing can already have normalized this occurrence.
        // A later body batch must not dereference its now-absent receiver.
        if original_receiver.is_valid()
            && let ExpressionNode::Name(original_path) = program
                .tables
                .bodies
                .expressions
                .expression(original_receiver)
                .clone()
        {
            let mut prefix = String::new();
            for (offset, member) in receiver.iter().enumerate() {
                if !prefix.is_empty() {
                    prefix.push_str("::");
                }
                prefix.push_str(member.as_str());
                let selected_module = program
                    .symbols
                    .find_top_level_by_name_and_kinds_from_source(
                        &prefix,
                        &[SymbolKind::Module],
                        reference,
                    )
                    .unwrap_or_else(SymbolHandle::invalid);
                program
                    .tables
                    .bodies
                    .expressions
                    .set_name_path_member_symbol_at_offset(
                        original_path.member_symbols,
                        offset as u32,
                        selected_module,
                    );
            }
        }
        if let ExpressionNode::Call(call) = program
            .tables
            .bodies
            .expressions
            .expression_mut(*expression)
        {
            call.receiver = ExpressionHandle::invalid();
            call.target = symbol_resolved_trees::name::DiagnosticName::from_str(&name, reference);
            call.target_symbol = selected;
        }
    }
    let symbols = &program.symbols;
    program
        .tables
        .declarations
        .state_statements
        .for_each_mut(|_, statement| {
            let symbol_resolved_trees::statement::Statement::Call(call) = statement else {
                return;
            };
            let Some((_, path)) = statement_calls
                .iter()
                .find(|(span, _)| *span == call.target.source_span())
            else {
                return;
            };
            let Some((target, receiver)) = path.split_last() else {
                return;
            };
            let receiver_name = receiver.joined_path();
            let reference = lookup::diagnostic_path_source_span(path);
            let namespace = symbols.find_top_level_by_name_and_kinds_from_source(
                &receiver_name,
                &[SymbolKind::Module],
                reference,
            );
            if !namespace.is_some_and(|symbol| symbols.get(symbol).kind == SymbolKind::Module) {
                return;
            }
            let name = format!("{receiver_name}::{}", target.as_str());
            let selected = symbols
                .find_top_level_by_name_and_kinds_from_source(
                    &name,
                    targets::SOURCE_CALL_KINDS,
                    reference,
                )
                .map(|symbol| {
                    if symbols.get(symbol).kind == SymbolKind::Machine {
                        symbols
                            .find_child_by_name_and_kind(symbol, "entry", SymbolKind::State)
                            .unwrap_or_else(SymbolHandle::invalid)
                    } else {
                        symbol
                    }
                })
                .unwrap_or_else(SymbolHandle::invalid);
            call.receiver = arena::HandleSpan::empty();
            call.receiver_symbol = SymbolHandle::invalid();
            call.receiver_root_symbol = SymbolHandle::invalid();
            call.target = symbol_resolved_trees::name::DiagnosticName::from_str(&name, reference);
            call.target_symbol = selected;
        });
}

#[derive(Default)]
pub(crate) struct NamespaceDeclarations {
    pub(crate) modules: Vec<Vec<syntax_trees::identifier::Identifier>>,
    pub(crate) imports: Vec<Vec<syntax_trees::identifier::Identifier>>,
}

impl NamespaceDeclarations {
    fn record_import_selections(
        &self,
        program: &mut SymbolResolvedTrees,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        use language_semantics::declaration_selection::{
            AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
        };
        for path in &self.imports {
            let (Some(first), Some(last)) = (path.first(), path.last()) else {
                continue;
            };
            let name = path.joined_path();
            let Some(target) = program
                .symbols
                .source_module_import_target(first.source_span().source_id, &name)
            else {
                continue;
            };
            let span = source::SourceSpan::new(
                first.source_span().source_id,
                source::Span::new(first.source_span().span.start, last.source_span().span.end),
            );
            program
                .record_resolved_authored_declaration_selection(
                    span,
                    AuthoredDeclarationSelectionExposure::PrivateImplementation,
                    AuthoredDeclarationSelectionKind::StaticPathSegment,
                    target,
                )
                .map_err(|error| {
                    vec![
                        diagnostics::Diagnostic::error(format!(
                            "failed to retain module import selection: {error:?}"
                        ))
                        .with_source_span(span),
                    ]
                })?;
        }
        Ok(())
    }

    /// Register namespace custody before all nonconstant headers are available.
    /// Import-target validation still runs when the complete table is installed.
    pub(crate) fn register(
        &self,
        symbols: &mut symbols::SymbolTable,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        for path in &self.modules {
            let Some(first) = path.first() else {
                continue;
            };
            symbols
                .register_source_module(
                    first.source_span().source_id,
                    path.iter()
                        .map(|member| (member.as_str(), member.source_span())),
                )
                .map_err(|message| {
                    vec![
                        diagnostics::Diagnostic::error(message)
                            .with_source_span(first.source_span()),
                    ]
                })?;
        }
        for path in &self.imports {
            let Some(first) = path.first() else {
                continue;
            };
            let name = path.joined_path();
            symbols.register_source_import(first.source_span().source_id, &name);
        }
        Ok(())
    }

    fn install(
        &self,
        symbols: &mut symbols::SymbolTable,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        self.register(symbols)?;
        for path in &self.imports {
            let Some(first) = path.first() else {
                continue;
            };
            let name = path.joined_path();
            symbols
                .validate_source_module_import(first.source_span().source_id, &name)
                .map_err(|message| {
                    vec![
                        diagnostics::Diagnostic::error(message)
                            .with_source_span(first.source_span()),
                    ]
                })?;
        }
        Ok(())
    }
}

/// The full symbol-assignment orchestration over a ready table: top-level
/// identity first, then generic origins, then the typed reference,
/// proposition, mathematical, measure, contract, domain-fact and signature
/// passes in dependency order. Both the initial and extension drivers run
/// the same sequence.
fn assign_program_symbols(
    program: &mut SymbolResolvedTrees,
    symbols: &symbols::SymbolTable,
    generic_origin_frontier: usize,
) -> Vec<diagnostics::Diagnostic> {
    let diagnostics = assign_top_level_symbols(program, symbols);
    type_references::assign_generic_application_origin_symbols(
        program,
        symbols,
        generic_origin_frontier,
    );
    assign_type_reference_symbols(program, symbols);
    type_references::assign_declaration_type_value_expression_symbols(program, symbols);
    propositions::assign_proposition_expression_symbols(program, symbols);
    mathematical::assign_mathematical_expression_symbols(program, symbols);
    measures::assign_measure_expression_symbols(program, symbols);
    assign_contract_reference_symbols(program, symbols);
    assign_domain_fact_symbols(program, symbols);
    assign_machine_signature_reference_symbols(program, symbols);
    diagnostics
}

pub(crate) fn assign_symbols(
    program: &mut SymbolResolvedTrees,
    sources: Option<Arc<SourceMap>>,
    source_scoped_top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    const_declarations: &[crate::resolution::lowerer::PendingConstDeclaration],
    namespace_declarations: &NamespaceDeclarations,
    generic_origin_frontier: usize,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let mut symbols = build_symbol_table(
        program,
        sources.clone(),
        source_scoped_top_level_bindings,
        const_declarations,
        !namespace_declarations.modules.is_empty(),
    );
    namespace_declarations.install(&mut symbols)?;
    let has_sources = sources.is_some() || !namespace_declarations.modules.is_empty();
    let symbols =
        symbol_table::insert_selected_machine_children(program, symbols, sources, 0, has_sources);
    let diagnostics = assign_program_symbols(program, &symbols, generic_origin_frontier);
    program.symbols = symbols;
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
    source_scoped_top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    roots: crate::resolution::lowerer::RootWatermarks,
    const_declarations: &[crate::resolution::lowerer::PendingConstDeclaration],
    namespace_declarations: &NamespaceDeclarations,
    generic_origin_frontier: usize,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    symbol_table::extend_symbol_table(
        program,
        sources.clone(),
        source_scoped_top_level_bindings,
        roots,
        const_declarations,
    );
    namespace_declarations.install(&mut program.symbols)?;
    let table = std::mem::take(&mut program.symbols);
    program.symbols = symbol_table::insert_selected_machine_children(
        program,
        table,
        Some(sources),
        roots.machines,
        true,
    );
    let symbols = program.symbols.clone();
    let diagnostics = assign_program_symbols(program, &symbols, generic_origin_frontier);
    namespace_declarations.record_import_selections(program)?;
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// Constant initializers have declaration-source lookup but no caller locals or
/// receiver. Reuse the ordinary expression resolver before copying a selected
/// initializer into any body; use-site namespaces never resolve its constructors.
pub(crate) fn assign_constant_expression_symbols(
    program: &mut SymbolResolvedTrees,
    initializers: impl IntoIterator<Item = symbol_resolved_trees::expression::ExpressionHandle>,
) {
    let attached_machines = scope::attached_machines(program);
    let declarations = &mut program.tables.declarations;
    let scope = scope::MachineScope {
        attached_machines: &attached_machines,
        resolution_indexes: &program.resolution_indexes,
        symbol: symbols::SymbolHandle::invalid(),
        lexical_requirements: scope::LexicalRequirements::none(),
        type_parameters: &[],
        attached_data: None,
        attached_data_symbol: symbols::SymbolHandle::invalid(),
        inherited_data_members: None,
        owned_data: &[],
        prior_statements: &[],
        data_definitions: &program.roots.data_definitions,
        data_members: &declarations.data_members,
        data_type_parameters: &declarations.data_type_parameters,
        data_payload_fields: &declarations.data_payload_fields,
        type_constraints: &program.tables.types.constraints,
    };
    for initializer in initializers {
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

pub(crate) use expressions::reject_unqualified_case_values;
pub(crate) use lookup::{
    MembershipSelection, constructor_type, domain_name_reaches, membership_selection,
    prefer_module_local_domain, qualified_payload_free_case_type,
};

/// Build the symbol table and give every reference its identity, then settle
/// the static module calls and outcome-specific contracts that need it. A
/// seeded lowerer extends its base's table in place.
pub(crate) fn assign(
    lowerer: &mut crate::resolution::lowerer::Lowerer,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    assign_declarations(lowerer)?;
    let machines = lowerer
        .symbol_resolved_trees
        .machines
        .iter()
        .map(|machine| machine.symbol)
        .collect::<Vec<_>>();
    assign_body_symbols(lowerer, &machines)
}

/// Allocate and resolve declaration identities before any body-local symbol
/// batch. Both eager resolution and retained-body completion use this boundary.
pub(crate) fn assign_declarations(
    lowerer: &mut crate::resolution::lowerer::Lowerer,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let seeded_roots = lowerer.seed.as_ref().map(|seed| seed.roots);
    match seeded_roots {
        None => assign_symbols(
            &mut lowerer.symbol_resolved_trees,
            lowerer.sources.take(),
            std::mem::take(&mut lowerer.source_scoped_top_level_bindings),
            &lowerer.pending_const_declarations,
            &lowerer.namespace_declarations,
            lowerer.bound_generic_application_origins,
        )?,
        Some(roots) => {
            let sources = lowerer.sources.take().ok_or_else(|| {
                vec![diagnostics::Diagnostic::error(
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
            )?;
        }
    }
    crate::resolution::operand_owners::bind(lowerer)?;
    lowerer.bound_generic_application_origins = lowerer
        .symbol_resolved_trees
        .tables
        .types
        .generic_application_origins
        .len();
    normalize_static_module_calls(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_static_module_calls,
        &lowerer.pending_static_module_statement_calls,
    );
    contracts::finalize_outcome_specific_contract_symbols(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_outcome_specific_contracts,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    Ok(())
}

/// Complete selected bodies against their existing declaration symbols. New
/// locals and synthetic state signatures append; retained bodies and headers
/// never have their symbols reallocated.
pub(crate) fn assign_body_symbols(
    lowerer: &mut crate::resolution::lowerer::Lowerer,
    selected_machines: &[symbols::SymbolHandle],
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    symbol_table::append_body_symbols(&mut lowerer.symbol_resolved_trees, selected_machines)?;
    let symbols = lowerer.symbol_resolved_trees.symbols.clone();
    type_references::assign_generic_application_origin_symbols(
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
    );
    // Synthesized states may introduce contracts after declaration indexing.
    // This resolves references, not declaration identities or child allocation.
    assign_contract_reference_symbols(&mut lowerer.symbol_resolved_trees, &symbols);
    normalize_static_module_calls(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_static_module_calls,
        &lowerer.pending_static_module_statement_calls,
    );
    contracts::finalize_outcome_specific_contract_symbols(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_outcome_specific_contracts,
    )
    .map_err(|diagnostic| vec![diagnostic])
}

pub(crate) fn assign_lexical_inputs(
    lowerer: &mut crate::resolution::lowerer::Lowerer,
    syntax: &syntax_trees::SyntaxTrees,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let mut inputs = crate::lowering::machine::lower_pending_lexical_inputs(lowerer, syntax)
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
    let symbols = lowerer.symbol_resolved_trees.symbols.clone();
    statements::assign_lexical_input_reference_symbols(
        &mut lowerer.symbol_resolved_trees,
        &symbols,
        &mut inputs,
    );
    normalize_static_module_calls(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_static_module_calls,
        &lowerer.pending_static_module_statement_calls,
    );
    Ok(())
}
