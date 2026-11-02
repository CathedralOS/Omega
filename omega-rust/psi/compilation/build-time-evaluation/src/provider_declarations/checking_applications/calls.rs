//! Demanded boundary applications from the ordinary live reference inventory.
//! The operational graph determines invocation; these exact typed call nodes
//! retain the qualifier and complete application that a method symbol cannot.
use super::schemas::Schema;
use symbols::SymbolHandle;
use typed_trees::{
    TypedTrees, expression::ExpressionNode, references::MachineSidecarIndex,
    trait_definition::AppliedTraitScope,
};

pub(super) struct Demand {
    pub called: SymbolHandle,
    pub requirement: SymbolHandle,
    pub schema: Schema,
}

pub(super) fn collect(
    program: &TypedTrees,
    machines: &[SymbolHandle],
    calls: &[SymbolHandle],
) -> Vec<Demand> {
    let sidecars = MachineSidecarIndex::build(program);
    let mut demands = Vec::new();
    let mut add = |owner, called, qualifier, application| {
        if !calls.contains(&called) {
            return;
        }
        if let Some(demand) = describe(program, owner, called, qualifier, application)
            && !demands.iter().any(|known: &Demand| {
                known.called == demand.called && known.schema.key == demand.schema.key
            })
        {
            demands.push(demand);
        }
    };
    for &symbol in machines {
        let Some(machine) = program.machine_by_symbol(symbol) else {
            continue;
        };
        let references =
            typed_trees::references::collect_machine_references(program, machine, &sidecars);
        let mut expressions = references.expressions.into_iter().collect::<Vec<_>>();
        expressions.sort_unstable_by_key(|handle| (handle.arena_index(), handle.generation()));
        for expression in expressions {
            if let ExpressionNode::Call(call) = program.expression_table.expression(expression) {
                add(
                    machine.symbol,
                    call.target_symbol,
                    validation::call_receiver_parts(program, call.receiver).0,
                    call.requirement_application,
                );
            }
        }
        let mut statements = references.statements.into_iter().collect::<Vec<_>>();
        statements.sort_unstable_by_key(|handle| (handle.arena_index(), handle.generation()));
        for statement in statements {
            if let Some(call) = program.statement_table.statement(statement).as_call() {
                add(
                    machine.symbol,
                    call.target_symbol,
                    call.receiver_symbol,
                    Default::default(),
                );
            }
        }
    }
    // Cleanup, named operators and generated calls also retain exact graph
    // coordinates even when no authored TableCall remains at that position.
    for &called in calls {
        if demands.iter().any(|demand| demand.called == called) {
            continue;
        }
        if let Some(demand) = describe(
            program,
            SymbolHandle::invalid(),
            called,
            SymbolHandle::invalid(),
            Default::default(),
        ) {
            demands.push(demand);
        }
    }
    demands
}

fn describe(
    program: &TypedTrees,
    owner: SymbolHandle,
    called: SymbolHandle,
    qualifier: SymbolHandle,
    application: typed_trees::types::TypeReferenceHandle,
) -> Option<Demand> {
    let closed = program.closed_requirement_application(called);
    let requirement = closed
        .as_ref()
        .map_or(called, |application| application.template);
    let scope = application
        .is_valid()
        .then(|| AppliedTraitScope::from_type_reference(program, application, owner))
        .flatten()
        .or_else(|| {
            closed
                .as_ref()
                .and_then(|application| application.trait_scope.scope(program))
        })
        .or_else(|| validation::qualified_trait_scope(program, owner, qualifier, None))
        .or_else(|| {
            program
                .trait_machine_signature(requirement)
                .map(|(declaration, _)| AppliedTraitScope::abstract_scope(declaration))
        });
    let schema = match scope {
        Some(scope) if scope.declaration.is_boundary => Schema::trait_scope(program, scope)?,
        Some(_) => return None,
        None => Schema::exact(program, requirement)?,
    };
    Some(Demand {
        called,
        requirement,
        schema,
    })
}
