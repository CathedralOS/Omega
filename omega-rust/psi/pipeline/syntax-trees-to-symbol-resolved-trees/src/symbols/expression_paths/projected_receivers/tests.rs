use super::*;
use crate::lower_syntax_trees;
use source_files_to_tokens::Lexer;
use tokens_to_syntax_trees::parse_syntax_trees;

fn payload_program() -> symbol_resolved_trees::SymbolResolvedTrees {
    let source = r#"
        data First {} data Second {} data Result { value: u64; }
        data Choice { case Left(item: First); case Right(item: Second); }
        data ForeignChoice { case Left(item: First); }
        machine First::read(self) -> u64 { 1 }
        machine Second::read(self) -> u64 { 2 }
        machine read() -> u64 { 99 }
        machine select(value: Choice, payload: Second) -> Result {
            transition value {
                Choice::Left { item as payload } -> Result { value: payload.read() }
                Choice::Right { item as payload } -> Result { value: payload.read() }
            }
        }
        machine foreign(value: Choice) {}
    "#;
    let syntax =
        parse_syntax_trees(&Lexer::new(source).tokenize().expect("tokenize")).expect("parse");
    lower_syntax_trees(&syntax).expect("resolve")
}

fn payload_calls(program: &symbol_resolved_trees::SymbolResolvedTrees) -> Vec<TableCallExpression> {
    use symbol_resolved_trees::statement::{StatementNode, TransitionTargetNode};
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "select")
        .unwrap();
    let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
    program
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| {
            let StatementNode::Transition(transition) = statement else {
                return None;
            };
            let TransitionTargetNode::Value(value) = program
                .tables
                .bodies
                .statements
                .transition_target(transition.target)
            else {
                return None;
            };
            let ExpressionNode::StructLiteral(literal) =
                program.tables.bodies.expressions.expression(*value)
            else {
                return None;
            };
            let field = &program
                .tables
                .bodies
                .expressions
                .struct_fields(literal.fields)[0];
            let ExpressionNode::Call(call) =
                program.tables.bodies.expressions.expression(field.value)
            else {
                return None;
            };
            Some(call.clone())
        })
        .collect()
}

#[test]
fn payload_candidates_follow_exact_cases_and_rebound_fields() {
    let program = payload_program();
    let calls = payload_calls(&program);
    assert_eq!(calls.len(), 2);
    for (call, expected_owner) in calls.iter().zip(["First::read", "Second::read"]) {
        let callee = program
            .machines
            .iter()
            .find(|machine| machine.name.as_str() == expected_owner)
            .unwrap();
        let expected = program
            .machine_state(program.machine_state_handles(callee.states)[0])
            .symbol;
        assert_eq!(
            call.target_symbol, expected,
            "the selected case owns the rebound payload"
        );
    }
}

#[test]
fn payload_candidates_reject_foreign_fields_cases_and_roots() {
    let program = payload_program();
    let calls = payload_calls(&program);
    let call = &calls[0];
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "select")
        .unwrap();
    let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
    let foreign = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "foreign")
        .unwrap();
    let foreign_state = program.machine_state(program.machine_state_handles(foreign.states)[0]);
    let foreign_root = program.state_parameters(foreign_state.parameters)[0].symbol;
    let choice = program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Choice")
        .unwrap();
    let fields = program
        .data_members(choice.members)
        .iter()
        .filter_map(|member| {
            let DataMember::Variant(variant) = member else {
                return None;
            };
            Some(program.data_payload_fields(variant.payload)[0].symbol)
        })
        .collect::<Vec<_>>();
    let foreign_choice = program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "ForeignChoice")
        .unwrap();
    let DataMember::Variant(foreign_variant) = &program.data_members(foreign_choice.members)[0]
    else {
        panic!("foreign case");
    };
    let foreign_field = program.data_payload_fields(foreign_variant.payload)[0].symbol;
    let scope = MachineScope {
        symbol: machine.symbol,
        type_parameters: &[],
        attached_data: machine.attached_data.as_ref(),
        attached_data_symbol: machine.attached_data_symbol,
        inherited_data_members: None,
        owned_data: &[],
        prior_statements: &[],
        data_definitions: &program.data_definitions,
        data_members: &program.tables.declarations.data_members,
        data_payload_fields: &program.tables.declarations.data_payload_fields,
        type_constraints: &program.tables.types.constraints,
    };
    for mutation in [
        "exact",
        "foreign_field",
        "other_case_field",
        "stale_field",
        "foreign_case",
        "missing_case",
        "foreign_root",
        "stale_root",
    ] {
        let mut table = program.tables.bodies.expressions.clone();
        let ExpressionNode::Member(member) = table.expression_mut(call.receiver) else {
            panic!("payload receiver");
        };
        member.member_symbol = match mutation {
            "foreign_field" => foreign_field,
            "other_case_field" => fields[1],
            "stale_field" => {
                SymbolHandle::from_parts(fields[0].arena_index(), fields[0].generation() + 1)
            }
            _ => fields[0],
        };
        if matches!(mutation, "foreign_case" | "missing_case") {
            member.case_variant = Some(symbol_resolved_trees::name::DiagnosticName::generated(
                if mutation == "foreign_case" {
                    "Right"
                } else {
                    "Absent"
                },
            ));
        }
        let root = member.receiver;
        let ExpressionNode::Name(name) = table.expression_mut(root) else {
            panic!("root");
        };
        if matches!(mutation, "foreign_root" | "stale_root") {
            let symbol = if mutation == "foreign_root" {
                foreign_root
            } else {
                SymbolHandle::from_parts(name.symbol.arena_index(), name.symbol.generation() + 1)
            };
            name.symbol = symbol;
            name.head_symbol = symbol;
        }
        let actual = call_target(
            &scope,
            program.state_parameters(state.parameters),
            state.symbol,
            call,
            &table,
            &program.tables.declarations.child_type_references,
            &program.symbols,
        );
        if mutation == "exact" {
            assert!(actual.is_valid());
            assert_eq!(actual, call.target_symbol);
        } else {
            assert!(
                !actual.is_valid(),
                "{mutation} cannot supply a payload method candidate"
            );
        }
    }
}

#[test]
fn payload_candidates_keep_same_spelled_nominal_owners_in_their_source() {
    use source::{SourceMap, SourceOrigin, SourceResolutionStratum};
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokens_to_syntax_trees::parse_syntax_trees_with_id;

    let source = r#"
        data Cell {} data Result { value: u64; }
        data Choice { case Item(cell: Cell); }
        machine Cell::read(self) -> u64 { 1 }
        machine select(value: Choice) -> Result {
            transition value {
                Choice::Item { cell } -> Result { value: cell.read() }
            }
        }
    "#;
    let mut sources = SourceMap::default();
    let base_id = sources
        .add(PathBuf::from("base.omg"), source.to_owned())
        .source_id;
    let extension_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from("extension.omg"),
            source.to_owned(),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let mut syntax = parse_syntax_trees_with_id(extension_id, &tokens).expect("extension syntax");
    syntax.extend_from(&parse_syntax_trees_with_id(base_id, &tokens).expect("base syntax"));
    let program = crate::lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
        .expect("resolve sources");
    let call_from = |source_id| {
        program
            .tables
            .bodies
            .expressions
            .iter_expressions()
            .find_map(|(_, expression)| {
                let ExpressionNode::Call(call) = expression else {
                    return None;
                };
                (call.target.source_span().source_id == source_id
                    && call.target_symbol.is_valid()
                    && matches!(
                        program.tables.bodies.expressions.expression(call.receiver),
                        ExpressionNode::Member(_)
                    ))
                .then_some(call.clone())
            })
            .expect("selected payload method")
    };
    let base_call = call_from(base_id);
    let extension_call = call_from(extension_id);
    for (call, source_id) in [(&base_call, base_id), (&extension_call, extension_id)] {
        assert_eq!(
            program
                .symbols
                .symbol_provenance_source_span(call.target_symbol)
                .unwrap()
                .source_id,
            source_id
        );
    }
    assert_ne!(base_call.target_symbol, extension_call.target_symbol);

    let machine = program
        .machines
        .iter()
        .find(|machine| {
            machine.name.as_str() == "select" && machine.name.source_span().source_id == base_id
        })
        .unwrap();
    let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
    let scope = MachineScope {
        symbol: machine.symbol,
        type_parameters: &[],
        attached_data: machine.attached_data.as_ref(),
        attached_data_symbol: machine.attached_data_symbol,
        inherited_data_members: None,
        owned_data: &[],
        prior_statements: &[],
        data_definitions: &program.data_definitions,
        data_members: &program.tables.declarations.data_members,
        data_payload_fields: &program.tables.declarations.data_payload_fields,
        type_constraints: &program.tables.types.constraints,
    };
    for foreign in [false, true] {
        let mut call = base_call.clone();
        if foreign {
            // Keep the exact base receiver and field; only make method lookup
            // see the same-spelled declaration from the extension's source.
            call.target = extension_call.target.clone();
        }
        let actual = call_target(
            &scope,
            program.state_parameters(state.parameters),
            state.symbol,
            &call,
            &program.tables.bodies.expressions,
            &program.tables.declarations.child_type_references,
            &program.symbols,
        );
        if foreign {
            assert!(
                !actual.is_valid(),
                "a foreign method cannot borrow the base payload type"
            );
        } else {
            assert_eq!(actual, base_call.target_symbol);
        }
    }
}

#[test]
fn indexed_candidates_follow_declared_elements_not_index_values() {
    let cases = [
        ("literal", "", "self.cells[0]", true),
        ("variable", "", "self.cells[index]", true),
        ("call", "", "self.cells[compute()]", true),
        ("parameter", "", "cells[index]", true),
        ("slice", "", "slice[index]", true),
        (
            "local_reference",
            "let local: &mut [Cell; 2] = &mut self.cells;",
            "local[index]",
            true,
        ),
        (
            "local_owned",
            "let local: [Cell; 2] = [Cell {}, Cell {}];",
            "local[index]",
            true,
        ),
        ("member", "", "self.buckets[index].cell", true),
        ("nested", "", "self.matrix[index][index]", true),
        ("whole_inner_array", "", "self.matrix[index]", false),
        ("scalar_index", "", "self.cell[index]", false),
        ("missing_root", "", "missing[index]", false),
        ("missing_field", "", "self.buckets[index].missing", false),
        ("later_local", "", "later[index]", false),
    ];
    for (name, prefix, receiver, admitted) in cases {
        let source = format!(
            r#"
            data Cell {{}} data Bucket {{ cell: Cell; }}
            data Owner {{ cells: [Cell; 2]; buckets: [Bucket; 2]; matrix: [[Cell; 2]; 2]; cell: Cell; }}
            machine Cell::read(&self) -> u64 {{ 1 }}
            machine read() -> u64 {{ 2 }}
            machine compute() -> u64 {{ 0 }}
            machine Owner::run(&mut self, cells: &mut [Cell; 2], slice: &mut [Cell], index: u64) {{
                {prefix}
                let result: u64 = {receiver}.read();
                let later: [Cell; 2] = [Cell {{}}, Cell {{}}];
            }}
        "#
        );
        let syntax =
            parse_syntax_trees(&Lexer::new(&source).tokenize().expect("tokenize")).expect("parse");
        let program = lower_syntax_trees(&syntax).expect("resolve");
        let machine = program
            .machines
            .iter()
            .find(|machine| machine.name.as_str() == "Owner::run")
            .expect("caller");
        let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
        let result = program
            .tables
            .bodies
            .statements
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| match statement {
                symbol_resolved_trees::statement::StatementNode::LocalData(local)
                    if local.name.as_str() == "result" =>
                {
                    Some(local)
                }
                _ => None,
            })
            .expect("result");
        let ExpressionNode::Call(call) = program
            .tables
            .bodies
            .expressions
            .expression(result.initial_value)
        else {
            panic!("method");
        };
        let callee = program
            .machines
            .iter()
            .find(|machine| machine.name.as_str() == "Cell::read")
            .expect("callee");
        let expected = if admitted {
            program
                .machine_state(program.machine_state_handles(callee.states)[0])
                .symbol
        } else {
            SymbolHandle::invalid()
        };
        assert_eq!(
            call.target_symbol, expected,
            "{name}: exact method candidate without free-name fallback"
        );
    }
}

#[test]
fn indexed_candidate_rejects_foreign_and_stale_parameter_roots() {
    let source = r#"
        data Cell {} data Owner {}
        machine Cell::read(&self) -> u64 { 1 }
        machine read() -> u64 { 2 }
        machine Owner::run(&mut self, cells: &mut [Cell; 2]) {
            let result: u64 = cells[0].read();
        }
        machine Owner::foreign(&mut self, cells: &mut [Cell; 2]) {}
    "#;
    let syntax =
        parse_syntax_trees(&Lexer::new(source).tokenize().expect("tokenize")).expect("parse");
    let program = lower_syntax_trees(&syntax).expect("resolve");
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "Owner::run")
        .expect("caller");
    let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
    let foreign = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "Owner::foreign")
        .expect("foreign");
    let foreign_state = program.machine_state(program.machine_state_handles(foreign.states)[0]);
    let foreign_symbol = program
        .state_parameters(foreign_state.parameters)
        .iter()
        .find(|parameter| parameter.name.as_str() == "cells")
        .expect("foreign cells")
        .symbol;
    let symbol_resolved_trees::statement::StatementNode::LocalData(result) = &program
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes)[0]
    else {
        panic!("result");
    };
    let ExpressionNode::Call(call) = program
        .tables
        .bodies
        .expressions
        .expression(result.initial_value)
    else {
        panic!("call");
    };
    let scope = MachineScope {
        symbol: machine.symbol,
        type_parameters: &[],
        attached_data: machine.attached_data.as_ref(),
        attached_data_symbol: machine.attached_data_symbol,
        inherited_data_members: None,
        owned_data: &[],
        prior_statements: &[],
        data_definitions: &program.data_definitions,
        data_members: &program.tables.declarations.data_members,
        data_payload_fields: &program.tables.declarations.data_payload_fields,
        type_constraints: &program.tables.types.constraints,
    };
    for variant in ["exact", "foreign", "stale"] {
        let mut table = program.tables.bodies.expressions.clone();
        let ExpressionNode::Indexed(indexed) = table.expression(call.receiver) else {
            panic!("indexed receiver");
        };
        let root = indexed.collection;
        let ExpressionNode::Name(name) = table.expression_mut(root) else {
            panic!("root");
        };
        let symbol = match variant {
            "foreign" => foreign_symbol,
            "stale" => {
                SymbolHandle::from_parts(name.symbol.arena_index(), name.symbol.generation() + 1)
            }
            _ => name.symbol,
        };
        name.symbol = symbol;
        name.head_symbol = symbol;
        let target = call_target(
            &scope,
            program.state_parameters(state.parameters),
            state.symbol,
            call,
            &table,
            &program.tables.declarations.child_type_references,
            &program.symbols,
        );
        if variant == "exact" {
            assert_eq!(target, call.target_symbol);
            assert!(target.is_valid());
        } else {
            assert!(
                !target.is_valid(),
                "{variant} root cannot select by spelling"
            );
        }
    }
}
