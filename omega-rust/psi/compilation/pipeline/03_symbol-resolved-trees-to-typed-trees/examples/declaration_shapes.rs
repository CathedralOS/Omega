//! Inspect actual typed payload sizes before selecting lookup benchmark shapes.
//! Takes standalone source paths; imports still need their ordinary resolver input.

use std::sync::Arc;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

fn summarize(path: &str) {
    let text = std::fs::read_to_string(path).unwrap();
    let mut bodies = std::collections::BTreeMap::<&str, (usize, usize)>::new();
    let (mut field_queries, mut field_preparations) = (0, 0);
    for line in text.lines() {
        if let Some(query) = line.strip_prefix("binding-query ") {
            let (scope, _) = query.rsplit_once(" Handle {").unwrap();
            bodies.entry(scope).or_default().0 += 1;
        } else if let Some(scope) = line.strip_prefix("binding-prepare ") {
            bodies.entry(scope).or_default().1 += 1;
        } else if line.starts_with("field-query ") {
            field_queries += 1;
        } else if line == "field-prepare" {
            field_preparations += 1;
        }
    }
    let mut baseline_visits = 0;
    let mut preparation_visits = 0;
    let mut queries = 0;
    let mut preparations = 0;
    for (scope, (calls, builds)) in bodies {
        let rows: usize = scope
            .split("count: ")
            .skip(1)
            .map(|count| count.split(' ').next().unwrap().parse::<usize>().unwrap())
            .sum();
        baseline_visits += calls * rows;
        preparation_visits += builds * rows;
        queries += calls;
        preparations += builds;
        println!(
            "body queries={calls} preparations={builds} inspected_rows_per_preparation={rows}"
        );
    }
    println!(
        "total binding_queries={queries} binding_preparations={preparations} scan_row_visits={baseline_visits} preparation_row_visits={preparation_visits} field_queries={field_queries} field_preparations={field_preparations}"
    );
}

fn main() {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments
        .first()
        .is_some_and(|arg| arg == "--profile-summary")
    {
        summarize(&arguments[1]);
        return;
    }
    for path in arguments {
        let text = std::fs::read_to_string(&path).unwrap();
        let mut sources = source::SourceMap::default();
        let id = sources.add(path.clone().into(), text.clone()).source_id;
        let tokens = source_files_to_tokens::Lexer::new(&text)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(id, &tokens).unwrap();
        let mut request = syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax);
        request.sources = Some(Arc::new(sources));
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request).unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        println!("source={path}");
        for machine in typed.machines() {
            for state in typed.machine_states(machine) {
                let statements = typed.statement_table.statements(state.statement_nodes);
                let locals = statements
                    .iter()
                    .filter_map(StatementNode::as_local_data)
                    .count();
                let binders: Vec<_> = typed
                    .state_parameters(state)
                    .iter()
                    .map(|p| p.symbol)
                    .chain(
                        statements
                            .iter()
                            .filter_map(StatementNode::as_local_data)
                            .map(|p| p.symbol),
                    )
                    .collect();
                let uses = typed.expression_table.expression_entries().filter(|(_, expression)| {
                    matches!(expression, ExpressionNode::Name(path) if path.resolved_symbol().is_some_and(|symbol| binders.contains(&symbol)))
                }).count();
                println!(
                    "body={} parameters={} locals={locals} statements={} name_uses={uses}",
                    machine.name.as_str(),
                    state.parameters.count(),
                    statements.len()
                );
            }
        }
        for data in typed.data_definitions() {
            println!(
                "data={} fields={}",
                data.name.as_str(),
                typed.data_member_fields(typed.data_members(data)).count()
            );
        }
    }
}
