//! Named member operands select normalized implementation rows, independently
//! of the private spelling used to store an inline realization machine.

use diagnostics::Diagnostic;
use source::SourceSpan;
use std::collections::HashMap;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::expression::{ExpressionNode, ExpressionTable, StaticMachineArgument};
use symbol_resolved_trees::statement::Statement;
use symbol_resolved_trees::types::TypeReference;
use symbols::{JoinedPath, SymbolHandle, SymbolKind, SymbolLookup, SymbolTable};

#[derive(Clone, Copy)]
struct Member {
    machine: SymbolHandle,
    entry: SymbolHandle,
    complete: bool,
}

struct Members {
    rows: HashMap<SymbolHandle, HashMap<String, Vec<Member>>, symbols::BuildSymbolHasher>,
}

impl Members {
    fn collect(program: &SymbolResolvedTrees) -> Self {
        let mut rows: HashMap<_, HashMap<String, Vec<Member>>, symbols::BuildSymbolHasher> =
            HashMap::default();
        for implementation in program.conformances.iter() {
            let names = rows.entry(implementation.symbol).or_default();
            for row in &implementation.rows {
                let complete = row.declaring_trait.is_valid()
                    && row.requirement.is_valid()
                    && row.realization_state.is_valid()
                    && program
                        .machine(row.realization_machine)
                        .is_some_and(|machine| {
                            program
                                .machine_state_handles(machine.states)
                                .iter()
                                .any(|state| {
                                    program.machine_state(*state).symbol == row.realization_state
                                })
                        });
                names
                    .entry(row.requirement_name.as_str().to_owned())
                    .or_default()
                    .push(Member {
                        machine: row.realization_machine,
                        entry: row.realization_state,
                        complete,
                    });
            }
        }
        Self { rows }
    }

    fn in_owner(
        &self,
        owner: SymbolHandle,
        name: &str,
        at: SourceSpan,
    ) -> Result<Member, Diagnostic> {
        match self
            .rows
            .get(&owner)
            .and_then(|names| names.get(name))
            .map(Vec::as_slice)
        {
            Some([member]) if member.complete => Ok(*member),
            Some([_]) => Err(error(
                at,
                "named implementation member lost its exact normalized realization",
            )),
            Some(_) => Err(error(
                at,
                "named implementation member is ambiguous between requirement overloads",
            )),
            None => Err(error(
                at,
                "named implementation has no such requirement member",
            )),
        }
    }

    fn path(
        &self,
        symbols: &SymbolTable,
        owner: &str,
        name: &str,
        at: SourceSpan,
    ) -> Result<Option<Member>, Diagnostic> {
        match symbols.lookup_top_level_by_name_and_kinds_from_source_matching(
            owner,
            &[SymbolKind::Conformance],
            at,
            |_| true,
        ) {
            SymbolLookup::Unique(owner) => self.in_owner(owner, name, at).map(Some),
            SymbolLookup::Ambiguous { .. } => {
                Err(error(at, "named implementation owner is ambiguous"))
            }
            SymbolLookup::NotFound => Ok(None),
        }
    }
}

pub(super) struct ResolvedStaticOperand {
    pub(super) source: SourceSpan,
    pub(super) entry: SymbolHandle,
}

/// Rejoin all declaration-owned static operand graphs after row normalization.
/// The receiving telescope continues to check operand categories and contracts.
pub(super) fn bind(program: &mut SymbolResolvedTrees) -> Result<(), Diagnostic> {
    let members = Members::collect(program);
    let expressions = program
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .filter_map(|(handle, node)| matches!(node, ExpressionNode::Call(_)).then_some(handle))
        .collect::<Vec<_>>();
    for expression in expressions {
        let ExpressionNode::Call(call) =
            program.tables.bodies.expressions.expression_mut(expression)
        else {
            continue;
        };
        let mut arguments = std::mem::take(&mut call.machine_arguments);
        let mut resolved = Vec::new();
        let result = bind_arguments(
            &mut arguments,
            &members,
            &program.symbols,
            &mut program.tables.bodies.expressions,
            &mut resolved,
        );
        if let ExpressionNode::Call(call) =
            program.tables.bodies.expressions.expression_mut(expression)
        {
            call.machine_arguments = arguments;
        }
        result?;
        let table = &program.tables.bodies.expressions;
        let occurrences = table
            .authored_selection_occurrences(expression)
            .collect::<Vec<_>>();
        let exposure = table.authored_expression_exposure(expression);
        let partition = table.compiler_selection_partition(expression);
        super::authored_selections::finalize_normalized_static_operands(
            program,
            &occurrences,
            exposure,
            partition,
            &resolved,
        )?;
    }
    // TableCall rows are produced from these declaration statements by the
    // existing table normalization; they do not need another resolver pass.
    let statements = program
        .tables
        .declarations
        .state_statements
        .iter()
        .filter_map(|(handle, statement)| matches!(statement, Statement::Call(_)).then_some(handle))
        .collect::<Vec<_>>();
    for statement in statements {
        let Statement::Call(call) = program
            .tables
            .declarations
            .state_statements
            .get_mut(statement)
        else {
            continue;
        };
        let mut arguments = std::mem::take(&mut call.machine_arguments);
        let occurrences = call.authored_static_argument_selections.to_vec();
        let mut resolved = Vec::new();
        let result = bind_arguments(
            &mut arguments,
            &members,
            &program.symbols,
            &mut program.tables.bodies.expressions,
            &mut resolved,
        );
        if let Statement::Call(call) = program
            .tables
            .declarations
            .state_statements
            .get_mut(statement)
        {
            call.machine_arguments = arguments;
        }
        result?;
        // The attachments, rather than a source-span search, retain the exact
        // compiler-copy partition and original private statement exposure.
        super::authored_selections::finalize_normalized_static_operands(
            program,
            &occurrences,
            None,
            None,
            &resolved,
        )?;
    }
    let mut result = Ok(());
    let symbols = &program.symbols;
    let table = &mut program.tables.bodies.expressions;
    program.roots.data_definitions.for_each_mut(|data| {
        if result.is_ok()
            && let Some(argument) = &mut data.cleanup_selection
        {
            result = bind_argument(
                argument,
                None,
                &members,
                symbols,
                table,
                &mut Vec::new(),
                &mut Vec::new(),
            );
        }
    });
    result?;
    let mut result = Ok(());
    program.roots.domain_definitions.for_each_mut(|domain| {
        if result.is_ok()
            && let Some(argument) = &mut domain.cleanup_selection
        {
            result = bind_argument(
                argument,
                None,
                &members,
                symbols,
                table,
                &mut Vec::new(),
                &mut Vec::new(),
            );
        }
    });
    result?;
    // Type-shaped code operands keep their category and source owner. P03
    // retains their type-reference occurrence, and the receiving declaration
    // still rejects a machine identity in a data/type slot.
    let references = program
        .tables
        .declarations
        .child_type_references
        .iter()
        .filter_map(|(handle, reference)| match reference {
            TypeReference::Named { symbol, name, .. } if !symbol.is_valid() => {
                Some((handle, name.clone()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    for (handle, name) in references {
        let Some((owner, member)) = name.as_str().rsplit_once("::") else {
            continue;
        };
        let Some(selected) = members.path(&program.symbols, owner, member, name.source_span())?
        else {
            continue;
        };
        if !program
            .symbols
            .source_reference_can_see_symbol(name.source_span(), selected.machine)
        {
            return Err(error(
                name.source_span(),
                "named implementation realization is not visible from this source",
            ));
        }
        if let TypeReference::Named { symbol, .. } = program
            .tables
            .declarations
            .child_type_references
            .get_mut(handle)
        {
            *symbol = selected.entry;
        }
    }
    Ok(())
}

fn bind_arguments(
    arguments: &mut [StaticMachineArgument],
    members: &Members,
    symbols: &SymbolTable,
    table: &mut ExpressionTable,
    resolved: &mut Vec<ResolvedStaticOperand>,
) -> Result<(), Diagnostic> {
    let mut active = Vec::new();
    for argument in arguments {
        bind_argument(
            argument,
            None,
            members,
            symbols,
            table,
            resolved,
            &mut active,
        )?;
    }
    Ok(())
}

fn bind_argument(
    argument: &mut StaticMachineArgument,
    owner: Option<SymbolHandle>,
    members: &Members,
    symbols: &SymbolTable,
    table: &mut ExpressionTable,
    resolved: &mut Vec<ResolvedStaticOperand>,
    active: &mut Vec<arena::Handle<StaticMachineArgument>>,
) -> Result<(), Diagnostic> {
    if argument.type_reference.is_valid()
        || argument.const_literal.is_some()
        || argument.evidence_projection.is_some()
    {
        return Ok(());
    }
    let source = argument
        .path
        .first()
        .zip(argument.path.last())
        .map(|(first, last)| {
            SourceSpan::new(
                first.source_span().source_id,
                source::Span::new(first.source_span().span.start, last.source_span().span.end),
            )
        });
    let selected = if !argument.symbol.is_valid()
        && let Some(source) = source
    {
        match (owner, argument.path.split_last()) {
            (Some(owner), Some((name, []))) => {
                Some(members.in_owner(owner, name.as_str(), source)?)
            }
            (_, Some((name, prefix))) if !prefix.is_empty() => {
                members.path(symbols, &prefix.joined_path(), name.as_str(), source)?
            }
            _ => None,
        }
    } else {
        None
    };
    if let Some(member) = selected {
        let at = source.unwrap_or_default();
        if !symbols.source_reference_can_see_symbol(at, member.machine) {
            return Err(error(
                at,
                "named implementation realization is not visible from this source",
            ));
        }
        argument.symbol = member.entry;
        if argument.path.iter().any(|name| name.is_source_backed()) {
            resolved.push(ResolvedStaticOperand {
                source: at,
                entry: member.entry,
            });
        }
    }
    let selected_owner = (argument.symbol.is_valid()
        && symbols.get(argument.symbol).kind == SymbolKind::Conformance)
        .then_some(argument.symbol);
    if let Some(application) = &mut argument.application {
        for nested in &mut application.arguments {
            bind_argument(nested, None, members, symbols, table, resolved, active)?;
        }
        if application.member.is_valid() {
            if active.contains(&application.member) {
                return Err(Diagnostic::error("cyclic static member operand graph"));
            }
            let mut member = table
                .static_member(application.member)
                .ok_or_else(|| {
                    Diagnostic::error("static member lost its receiving expression table")
                })?
                .clone();
            active.push(application.member);
            let result = bind_argument(
                &mut member,
                selected_owner,
                members,
                symbols,
                table,
                resolved,
                active,
            );
            active.pop();
            *table.static_member_mut(application.member).ok_or_else(|| {
                Diagnostic::error("static member lost its receiving expression table")
            })? = member;
            result?;
        }
    }
    Ok(())
}

fn error(at: SourceSpan, message: &str) -> Diagnostic {
    Diagnostic::error(message).with_source_span(at)
}
