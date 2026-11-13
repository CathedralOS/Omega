//! `let { a, b as _ } = subject;` reaches typed preparation as a marker local
//! whose roster holds only what the author spelled. This pass gives each row
//! the symbol of the field it names in the subject's record and the marker
//! the record's own symbol, so the checker decides coverage by identity and
//! never reads a spelling.
//!
//! A subject this pass cannot resolve keeps an unresolved marker, and a row
//! naming no field, or a name two fields share, keeps no symbol. Neither is an
//! error here: the checker refuses both where a program reaches the pattern.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataMember};
use typed_trees::expression::{ExpressionNode, TableMatchField};
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableLocalData};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn prepare(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let mut resolutions = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for handle in state.statement_nodes.handles() {
                let Some(local) = program.statement_table.statement(handle).as_local_data() else {
                    continue;
                };
                if local.role != language_core::LocalBindingRole::RecordPattern {
                    continue;
                }
                let Some(record) = subject_record(program, state, local) else {
                    resolutions.push((handle, SymbolHandle::invalid(), Vec::new()));
                    continue;
                };
                let roster = program
                    .expression_table
                    .match_fields(local.pattern_fields)
                    .iter()
                    .map(|row| TableMatchField {
                        field_symbol: named_field(program, record, row.member.as_str()),
                        ..row.clone()
                    })
                    .collect::<Vec<_>>();
                resolutions.push((handle, record.symbol, roster));
            }
        }
    }
    for (handle, subject, roster) in resolutions {
        // Each resolved marker receives rows of its own: two markers may
        // share authored rows while their subjects are different records.
        let pattern_fields = subject
            .is_valid()
            .then(|| program.expression_table.insert_match_fields(roster));
        if let StatementNode::LocalData(local) = program.statement_table.statement_mut(handle) {
            local.pattern_subject = subject;
            if let Some(pattern_fields) = pattern_fields {
                local.pattern_fields = pattern_fields;
            }
        }
    }
    Ok(())
}

/// The field of `record` spelled `name`, when exactly one is.
fn named_field(program: &TypedTrees, record: &DataDefinition, name: &str) -> SymbolHandle {
    let mut named = program
        .data_members(record)
        .iter()
        .filter_map(DataMember::as_field)
        .filter(|field| field.name.as_str() == name);
    match (named.next(), named.next()) {
        (Some(field), None) => field.symbol,
        _ => SymbolHandle::invalid(),
    }
}

/// The plain record the marker destructures: a declared data type with
/// fields only, reached through references and domain constraints.
///
/// The subject's type is read from the declaration of the binding it names,
/// a parameter or an earlier typed local of the same state, and otherwise
/// from the marker's own type. A field path, a call result, an applied
/// generic record, a quotient and a sum all stay unresolved.
fn subject_record<'program>(
    program: &'program TypedTrees,
    state: &State,
    local: &TableLocalData,
) -> Option<&'program DataDefinition> {
    let declared = declared_binding_type(program, state, local.initial_value).or_else(|| {
        local
            .type_reference
            .is_valid()
            .then_some(local.type_reference)
    })?;
    let carrier = program.type_reference_table.peeled_type_reference(declared);
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(carrier)
    else {
        return None;
    };
    let record = program.unique_data_definition_by_symbol(*symbol)?;
    let plain = program.data_type_parameters(record).is_empty()
        && record.lifetime_parameters.is_empty()
        && record.quotient.is_none()
        && program
            .data_members(record)
            .iter()
            .all(|member| matches!(member, DataMember::Field(_)));
    plain.then_some(record)
}

fn declared_binding_type(
    program: &TypedTrees,
    state: &State,
    subject: typed_trees::expression::ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    if !subject.is_valid() {
        return None;
    }
    let ExpressionNode::Name(path) = program.expression_table.expression(subject) else {
        return None;
    };
    let symbol = path.resolved_symbol()?;
    if program
        .expression_table
        .name_path_members(path.members)
        .len()
        != 1
    {
        return None;
    }
    let declared = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == symbol)
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .filter_map(StatementNode::as_local_data)
                .find(|binding| binding.symbol == symbol)
                .map(|binding| binding.type_reference)
        })?;
    declared.is_valid().then_some(declared)
}

#[cfg(test)]
mod tests {
    use super::prepare;
    use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use typed_trees::TypedTrees;
    use typed_trees::data::{DataDefinition, DataField, DataMember, DataVariant};
    use typed_trees::expression::{
        ExpressionHandle, ExpressionNode, MatchFieldAction, TableMatchField, TableNamePath,
    };
    use typed_trees::machine::Machine;
    use typed_trees::name::Identifier;
    use typed_trees::signature::StateParameter;
    use typed_trees::state::State;
    use typed_trees::statement::{StatementHandle, StatementNode, TableLocalData};
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    struct Source {
        program: TypedTrees,
        pair: SymbolHandle,
        fields: [SymbolHandle; 2],
        markers: Vec<StatementHandle>,
    }

    /// `machine read(pair: Pair, shape: Shape)` whose body holds one marker
    /// per subject: `pair`, `shape`, and a call-like subject that names no
    /// binding. `Pair { a, b }` is a plain record; `Shape` has a case.
    fn source() -> Source {
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let top = builder.insert_children(
            root,
            [
                (SymbolKind::Data, SymbolNameRef::Static("Pair")),
                (SymbolKind::Data, SymbolNameRef::Static("Shape")),
                (SymbolKind::Machine, SymbolNameRef::Static("read")),
            ],
        );
        let (pair, shape, machine) = (top.handle_at(0), top.handle_at(1), top.handle_at(2));
        let pair_fields = builder.insert_children(
            pair,
            [
                (SymbolKind::Field, SymbolNameRef::Static("a")),
                (SymbolKind::Field, SymbolNameRef::Static("b")),
            ],
        );
        let fields = [pair_fields.handle_at(0), pair_fields.handle_at(1)];
        let state = builder
            .insert_children(
                machine,
                [(SymbolKind::State, SymbolNameRef::Static("entry"))],
            )
            .handle_at(0);
        let parameters = builder.insert_children(
            state,
            [
                (SymbolKind::Parameter, SymbolNameRef::Static("pair")),
                (SymbolKind::Parameter, SymbolNameRef::Static("shape")),
            ],
        );
        let mut program = TypedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        let named = |program: &mut TypedTrees, symbol, name: &'static str| {
            program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol,
                    name: Identifier::generated(name),
                })
        };
        let pair_type = named(&mut program, pair, "Pair");
        let shape_type = named(&mut program, shape, "Shape");
        let members =
            program
                .tables
                .data_members
                .insert_many(fields.into_iter().zip(["a", "b"]).map(|(symbol, name)| {
                    DataMember::Field(DataField {
                        symbol,
                        name: Identifier::generated(name),
                        ..Default::default()
                    })
                }));
        program.push_data_definition(DataDefinition {
            symbol: pair,
            name: Identifier::generated("Pair"),
            members,
            ..Default::default()
        });
        let cases = program
            .tables
            .data_members
            .insert_many([DataMember::Variant(DataVariant {
                name: Identifier::generated("Round"),
                ..Default::default()
            })]);
        program.push_data_definition(DataDefinition {
            symbol: shape,
            name: Identifier::generated("Shape"),
            members: cases,
            ..Default::default()
        });
        let subject = |program: &mut TypedTrees, symbol, name: &'static str| {
            let mut members = arena::HandleSpan::empty();
            program
                .expression_table
                .push_name_path_member(&mut members, Identifier::generated(name));
            program
                .expression_table
                .insert(ExpressionNode::Name(TableNamePath {
                    members,
                    symbol,
                    ..TableNamePath::default()
                }))
        };
        let subjects = [
            subject(&mut program, parameters.handle_at(0), "pair"),
            subject(&mut program, parameters.handle_at(1), "shape"),
            program
                .expression_table
                .insert(ExpressionNode::Boolean(true)),
        ];
        let marker = |program: &mut TypedTrees, initial_value: ExpressionHandle| {
            let row = |member: &'static str| TableMatchField {
                component: language_core::PatternComponent::Fields,
                member: Identifier::generated(member),
                field_symbol: SymbolHandle::invalid(),
                action: MatchFieldAction::Waive,
            };
            let pattern_fields = program.expression_table.insert_match_fields(vec![
                row("b"),
                row("a"),
                row("missing"),
            ]);
            StatementNode::LocalData(TableLocalData {
                role: language_core::LocalBindingRole::RecordPattern,
                initial_value,
                pattern_fields,
                ..Default::default()
            })
        };
        let statements = subjects.map(|subject| marker(&mut program, subject));
        let statement_nodes = program.statement_table.insert_statements(statements);
        let parameters = program.state_parameters.insert_many(
            [(0, pair_type, "pair"), (1, shape_type, "shape")].map(|(position, declared, name)| {
                StateParameter {
                    symbol: parameters.handle_at(position),
                    name: Identifier::generated(name),
                    type_reference: declared,
                    ..Default::default()
                }
            }),
        );
        let states = program.machine_states.insert_many([State {
            symbol: state,
            parameters,
            statement_nodes,
            ..Default::default()
        }]);
        program.push_machine(Machine {
            symbol: machine,
            name: Identifier::generated("read"),
            states,
            ..Default::default()
        });
        let _: TypeReferenceHandle = pair_type;
        Source {
            program,
            pair,
            fields,
            markers: statement_nodes.handles().collect(),
        }
    }

    fn marker(program: &TypedTrees, handle: StatementHandle) -> &TableLocalData {
        program
            .statement_table
            .statement(handle)
            .as_local_data()
            .expect("a record pattern marker")
    }

    #[test]
    fn a_marker_over_a_declared_plain_record_names_that_record_and_its_fields() {
        let Source {
            mut program,
            pair,
            fields: [a, b],
            markers,
        } = source();
        let authored = marker(&program, markers[0]).pattern_fields;
        prepare(&mut program).expect("the pass refuses nothing");
        let resolved = marker(&program, markers[0]);
        assert_eq!(resolved.pattern_subject, pair);
        assert_ne!(resolved.pattern_fields, authored);
        let symbols = program
            .expression_table
            .match_fields(resolved.pattern_fields)
            .iter()
            .map(|row| row.field_symbol)
            .collect::<Vec<_>>();
        // Authored order is kept; a spelling that names no field has no symbol.
        assert_eq!(symbols, [b, a, SymbolHandle::invalid()]);
    }

    #[test]
    fn a_marker_over_a_sum_or_an_undeclared_subject_stays_unresolved() {
        let Source {
            mut program,
            markers,
            ..
        } = source();
        let before = [markers[1], markers[2]].map(|handle| marker(&program, handle).clone());
        prepare(&mut program).expect("the pass refuses nothing");
        for (handle, before) in [markers[1], markers[2]].into_iter().zip(before) {
            let after = marker(&program, handle);
            assert!(!after.pattern_subject.is_valid());
            assert_eq!(after, &before);
        }
    }
}
