use std::sync::Arc;

use source::SourceMap;
use symbol_resolved_trees::SymbolResolvedTrees;

mod symbol_table;

use symbol_table::build_symbol_table;
use type_references::assign_type_reference_symbols;

mod contracts;
mod domain_facts;
mod expression_paths;
mod expressions;
mod lookup;
mod measures;
mod propositions;
mod scope;
mod scoped_paths;
mod statements;
mod targets;
mod top_level;
mod type_references;

use contracts::assign_contract_reference_symbols;
use domain_facts::assign_domain_fact_symbols;
use statements::assign_statement_reference_symbols;
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
    use symbols::{SymbolHandle, SymbolKind};
    for (expression, path) in calls {
        let Some((target, receiver)) = path.split_last() else {
            continue;
        };
        let receiver_name = receiver
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::");
        let reference = lookup::diagnostic_path_source_span(path);
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
        let name = format!("{receiver_name}::{}", target.as_str());
        let selected = program
            .symbols
            .find_top_level_by_name_and_kinds_from_source(
                &name,
                &[SymbolKind::Machine, SymbolKind::Proposition],
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
        if let ExpressionNode::Name(original_path) = program
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
            let receiver_name = receiver
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
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
                    &[SymbolKind::Machine],
                    reference,
                )
                .and_then(|machine| {
                    symbols.find_child_by_name_and_kind(machine, "entry", SymbolKind::State)
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
            let name = path
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
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
            let name = path
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
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
            let name = path
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
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

pub(crate) fn assign_symbols(
    program: &mut SymbolResolvedTrees,
    sources: Option<Arc<SourceMap>>,
    source_scoped_top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    const_declarations: &[crate::lowerer::PendingConstDeclaration],
    namespace_declarations: &NamespaceDeclarations,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let mut symbols = build_symbol_table(
        program,
        sources,
        source_scoped_top_level_bindings,
        const_declarations,
        !namespace_declarations.modules.is_empty(),
    );
    namespace_declarations.install(&mut symbols)?;
    let diagnostics = assign_top_level_symbols(program, &symbols);
    assign_type_reference_symbols(program, &symbols);
    propositions::assign_proposition_expression_symbols(program, &symbols);
    measures::assign_measure_expression_symbols(program, &symbols);
    assign_contract_reference_symbols(program, &symbols);
    assign_domain_fact_symbols(program, &symbols);
    assign_statement_reference_symbols(program, &symbols);
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
    roots: crate::lowerer::RootWatermarks,
    const_declarations: &[crate::lowerer::PendingConstDeclaration],
    namespace_declarations: &NamespaceDeclarations,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    symbol_table::extend_symbol_table(
        program,
        sources,
        source_scoped_top_level_bindings,
        roots,
        const_declarations,
    );
    namespace_declarations.install(&mut program.symbols)?;
    let symbols = program.symbols.clone();
    let diagnostics = assign_top_level_symbols(program, &symbols);
    assign_type_reference_symbols(program, &symbols);
    propositions::assign_proposition_expression_symbols(program, &symbols);
    measures::assign_measure_expression_symbols(program, &symbols);
    assign_contract_reference_symbols(program, &symbols);
    assign_domain_fact_symbols(program, &symbols);
    assign_statement_reference_symbols(program, &symbols);
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
    let declarations = &mut program.tables.declarations;
    let scope = scope::MachineScope {
        symbol: symbols::SymbolHandle::invalid(),
        type_parameters: &[],
        attached_data: None,
        attached_data_symbol: symbols::SymbolHandle::invalid(),
        inherited_data_members: None,
        owned_data: &[],
        prior_statements: &[],
        data_definitions: &program.roots.data_definitions,
        data_members: &declarations.data_members,
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

pub(crate) use lookup::{
    MembershipSelection, bare_case_type, constructor_type, membership_selection,
};
