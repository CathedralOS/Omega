//! A late dynamic call pass must not replace an exact projected requirement.

use source::{SourceMap, SourceOrigin};
use std::path::PathBuf;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::statement::StatementNode;

fn typed_modules(modules: &[(&str, String)]) -> TypedTrees {
    let binding = "pub boundary data Binding<R>;";
    let mut sources = SourceMap::default();
    let binding_id = sources
        .add_with_metadata(
            PathBuf::from("core/binding.omg"),
            binding.to_owned(),
            PathBuf::from("core"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let mut texts = vec![(binding_id, binding)];
    for (name, text) in modules {
        let source_id = sources
            .add(PathBuf::from(format!("{name}.omg")), text.clone())
            .source_id;
        texts.push((source_id, text.as_str()));
    }
    crate::front_end::typed_program_from_source_map(sources, &texts)
}

fn seed_projected_calls(
    program: &mut TypedTrees,
    caller_path: &str,
    requirement_path: &str,
    receiver_owner_path: &str,
) -> usize {
    let requirement = program
        .traits()
        .iter()
        .find(|definition| {
            program.symbols.display_path(definition.symbol, "::") == requirement_path
        })
        .expect("exact requirement owner");
    let target = program.trait_machine_signatures(requirement)[0].symbol;
    let receiver_owner = program
        .data_definitions()
        .iter()
        .find(|definition| {
            program.symbols.display_path(definition.symbol, "::") == receiver_owner_path
        })
        .expect("exact projected receiver owner");
    let field_symbol = program
        .data_members(receiver_owner)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) if field.name.as_str() == "sink" => {
                Some(field.symbol)
            }
            _ => None,
        })
        .expect("sink field");
    let machine = program
        .machines()
        .iter()
        .find(|machine| program.symbols.display_path(machine.symbol, "::") == caller_path)
        .expect("exact caller");
    let coordinates = program
        .machine_states(machine)
        .iter()
        .map(|state| state.statement_nodes)
        .collect::<Vec<_>>();
    let mut count = 0;
    for statements in coordinates {
        for statement in program.statement_table.statements_mut(statements) {
            if let StatementNode::Call(call) = statement
                && call.target.as_str() == "emit"
            {
                // Model the exact endpoint and target stage 04's first projected
                // resolution pass supplies before this later dynamic pass.
                call.receiver_symbol = field_symbol;
                call.target_symbol = target;
                count += 1;
            }
        }
    }
    count
}

fn emit_targets(program: &TypedTrees) -> Vec<SymbolHandle> {
    program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .flat_map(|state| program.statement_table.statements(state.statement_nodes))
        .filter_map(|statement| match statement {
            StatementNode::Call(call) if call.target.as_str() == "emit" => Some(call.target_symbol),
            _ => None,
        })
        .collect()
}

#[test]
fn dynamic_resolution_preserves_same_named_requirement_owners() {
    for receiver in ["context.sink", "self.sink", "sink"] {
        for reverse in [false, true] {
            let parameters = if receiver == "context.sink" {
                "context: &mut Runner"
            } else {
                "&mut self"
            };
            let arguments = if receiver == "context.sink" {
                "context"
            } else {
                ""
            };
            let module = |name: &str| {
                format!(
                    r#"
            module {name};
            pub boundary trait Sink {{ machine emit(); }}
            pub data Runner {{ sink: Binding<Sink>; }}
            pub machine Runner::run({parameters}) reaches Sink {{
                {receiver}.emit();
                transition {{ _ -> again({arguments}) }}
                state again({parameters}) {{ {receiver}.emit(); }}
            }}
        "#
                )
            };
            let mut modules = [("left", module("left")), ("right", module("right"))];
            if reverse {
                modules.reverse();
            }
            let mut program = typed_modules(&modules);
            for owner in ["left", "right"] {
                assert_eq!(
                    seed_projected_calls(
                        &mut program,
                        &format!("{owner}::Runner::run"),
                        &format!("{owner}::Sink"),
                        &format!("{owner}::Runner")
                    ),
                    2
                );
            }
            let expected = emit_targets(&program);
            super::resolve_dynamic_call_targets(&mut program)
                .expect("resolve exact dynamic requirements");
            assert_eq!(
                emit_targets(&program),
                expected,
                "receiver {receiver}; module declaration order reversed: {reverse}"
            );
        }
    }
}

#[test]
fn dynamic_resolution_follows_the_whole_projected_receiver() {
    let mut program = typed_modules(&[
        (
            "left",
            r#"
            module left;
            use right;
            pub boundary trait Sink { machine emit(); }
            pub data Wrapper { carrier: right::Carrier; }
            pub data Runner { sink: Binding<Sink>; }
            pub machine Runner::run(external: &mut Wrapper) {
                external.carrier.sink.emit();
                transition { _ -> again(external) }
                state again(external: &mut Wrapper) { external.carrier.sink.emit(); }
            }
        "#
            .to_owned(),
        ),
        (
            "right",
            r#"
            module right;
            pub boundary trait Sink { machine emit(); }
            pub data Carrier { sink: Binding<Sink>; }
        "#
            .to_owned(),
        ),
    ]);
    assert_eq!(
        seed_projected_calls(
            &mut program,
            "left::Runner::run",
            "right::Sink",
            "right::Carrier"
        ),
        2
    );
    let expected = emit_targets(&program);
    super::resolve_dynamic_call_targets(&mut program)
        .expect("resolve projected dynamic requirements");
    assert_eq!(emit_targets(&program), expected);
}
