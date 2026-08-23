use psi_checked_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_checked_trees::name::Identifier;
use psi_checked_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use psi_checked_trees::{
    CheckedTrees, DynamicConformanceBindingFacts, DynamicConformanceRowFact,
    DynamicConformanceRowSource, DynamicConformanceSelectionFact,
};
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(crate) fn validated_dynamic_conformance_bindings(
    program: &CheckedTrees,
) -> Result<DynamicConformanceBindingFacts, Diagnostic> {
    let selections = &program.facts.dynamic_conformances.selections;
    for (index, selection) in selections.iter().enumerate() {
        if selections[..index].iter().any(|prior| {
            prior.occurrence == selection.occurrence
                || (prior.machine == selection.machine
                    && prior.state == selection.state
                    && prior.statement_index == selection.statement_index
                    && prior.binding == selection.binding)
        }) {
            return Err(Diagnostic::error(
                "state-graph dynamic selection occurrence or binding coordinate is duplicated",
            ));
        }
        validate_selection(program, selection)?;
    }
    Ok(program.facts.dynamic_conformances.binding_facts())
}

fn validate_selection(
    program: &CheckedTrees,
    selection: &DynamicConformanceSelectionFact,
) -> Result<(), Diagnostic> {
    if !selection.binding.is_valid() {
        return Err(Diagnostic::error(
            "state-graph dynamic selection binding identity is invalid",
        ));
    }
    let machine = exact_machine(program, selection.machine)?;
    let state = exact_state(program, selection.machine, selection.state)?;
    let statement = program
        .statement_table
        .statements(state.statement_nodes)
        .get(selection.statement_index)
        .ok_or_else(|| {
            Diagnostic::error("state-graph dynamic selection statement coordinate is out of range")
        })?;
    let psi_checked_trees::statement::StatementNode::LocalData(local) = statement else {
        return Err(Diagnostic::error(
            "state-graph dynamic selection coordinate is not a local-data statement",
        ));
    };
    if local.symbol != selection.binding || local.name != selection.binding_name {
        return Err(Diagnostic::error(
            "state-graph dynamic selection binding disagrees with its exact local declaration",
        ));
    }

    let occurrence = strip_mutable(program, local.initial_value)?;
    if occurrence != selection.occurrence || !exact_expression(program, occurrence) {
        return Err(Diagnostic::error(
            "state-graph dynamic selection occurrence disagrees with its exact initializer",
        ));
    }
    let ExpressionNode::Cast(cast) = program.expression_table.expression(occurrence) else {
        return Err(Diagnostic::error(
            "state-graph dynamic selection occurrence is not a retained cast",
        ));
    };

    let dynamic = exact_dynamic_type(program, cast.target_type)?;
    let TypeReferenceNode::DynamicTrait {
        symbol: target_trait,
        conformance,
        ..
    } = dynamic
    else {
        unreachable!("exact_dynamic_type returns a dynamic trait")
    };
    if *target_trait != selection.target_trait || *conformance != selection.conformance {
        return Err(Diagnostic::error(
            "state-graph dynamic selection target identity drifted from its exact cast",
        ));
    }

    let source = exact_source_place(program, cast.value)?;
    if source.name != selection.source_name || source.path != selection.source_path {
        return Err(Diagnostic::error(
            "state-graph dynamic selection source place disagrees with its exact cast",
        ));
    }
    let source_declaration = exact_source_declaration(
        program,
        machine,
        state,
        selection.statement_index,
        &source.path,
    )?;
    if source_declaration.symbol != selection.source_symbol {
        return Err(Diagnostic::error(
            "state-graph dynamic selection source declaration identity drifted",
        ));
    }
    let source_data = exact_named_data(program, source_declaration.type_reference)?;
    if source_data.symbol != selection.source_data {
        return Err(Diagnostic::error(
            "state-graph dynamic selection source-data identity drifted",
        ));
    }

    let target = exact_trait(program, selection.target_trait)?;
    let conformance_symbol = selection.conformance.ok_or_else(|| {
        Diagnostic::error("state-graph dynamic selection has no exact conformance identity")
    })?;
    let conformance = exact_conformance(program, conformance_symbol)?;
    if conformance.carrier_name() != Some(&source_data.name)
        || conformance.trait_name != target.name
        || !conformance.arguments.is_empty()
    {
        return Err(Diagnostic::error(
            "state-graph exact dynamic conformance disagrees with its retained carrier or trait",
        ));
    }
    let rows = program
        .closed_conformance_rows(conformance)
        .ok_or_else(|| {
            Diagnostic::error("state-graph dynamic selection names a bodyless conformance")
        })?;
    validate_rows(program, rows, &selection.rows)
}

fn exact_machine<'program>(
    program: &'program CheckedTrees,
    symbol: SymbolHandle,
) -> Result<&'program psi_checked_trees::machine::Machine, Diagnostic> {
    if !symbol.is_valid() {
        return Err(Diagnostic::error(
            "state-graph dynamic selection machine identity is invalid",
        ));
    }
    let mut matches = program
        .machines()
        .iter()
        .filter(|machine| machine.symbol == symbol);
    let machine = matches
        .next()
        .ok_or_else(|| Diagnostic::error("state-graph dynamic selection machine is missing"))?;
    if matches.next().is_some() {
        return Err(Diagnostic::error(
            "state-graph dynamic selection machine is duplicated",
        ));
    }
    Ok(machine)
}

fn exact_state<'program>(
    program: &'program CheckedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Result<&'program psi_checked_trees::state::State, Diagnostic> {
    if !state_symbol.is_valid() {
        return Err(Diagnostic::error(
            "state-graph dynamic selection state identity is invalid",
        ));
    }
    let machine = exact_machine(program, machine_symbol)?;
    let owner_matches = program
        .machine_states(machine)
        .iter()
        .filter(|state| state.symbol == state_symbol)
        .collect::<Vec<_>>();
    let all_matches = program
        .machines()
        .iter()
        .flat_map(|candidate| program.machine_states(candidate))
        .filter(|state| state.symbol == state_symbol)
        .count();
    if let [state] = owner_matches.as_slice()
        && all_matches == 1
    {
        return Ok(*state);
    }
    Err(Diagnostic::error(
        "state-graph dynamic selection state is missing, duplicated, or cross-owned",
    ))
}

fn exact_expression(program: &CheckedTrees, handle: ExpressionHandle) -> bool {
    handle.is_valid()
        && program
            .expression_table
            .iter_expressions()
            .any(|(candidate, _)| candidate == handle)
}

fn strip_mutable(
    program: &CheckedTrees,
    expression: ExpressionHandle,
) -> Result<ExpressionHandle, Diagnostic> {
    if !exact_expression(program, expression) {
        return Err(Diagnostic::error(
            "state-graph dynamic selection initializer expression is invalid",
        ));
    }
    Ok(match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => inner.target,
        _ => expression,
    })
}

fn exact_dynamic_type(
    program: &CheckedTrees,
    mut reference: TypeReferenceHandle,
) -> Result<&TypeReferenceNode, Diagnostic> {
    let mut visited = Vec::new();
    loop {
        if !reference.is_valid() || visited.contains(&reference) {
            return Err(Diagnostic::error(
                "state-graph dynamic selection target type is invalid or cyclic",
            ));
        }
        visited.push(reference);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. } => reference = *referee,
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            dynamic @ TypeReferenceNode::DynamicTrait { .. } => return Ok(dynamic),
            _ => {
                return Err(Diagnostic::error(
                    "state-graph dynamic selection cast target is not a dynamic trait",
                ));
            }
        }
    }
}

#[derive(Debug)]
struct SourcePlace {
    name: Identifier,
    path: Vec<Identifier>,
}

fn exact_source_place(
    program: &CheckedTrees,
    expression: ExpressionHandle,
) -> Result<SourcePlace, Diagnostic> {
    if !exact_expression(program, expression) {
        return Err(Diagnostic::error(
            "state-graph dynamic selection source expression is invalid",
        ));
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => exact_source_place(program, atomic.value),
        ExpressionNode::Borrow(inner) => exact_source_place(program, inner.target),
        ExpressionNode::Name(name) => {
            let members = program.expression_table.name_path_members(name.members);
            let leaf = members.last().cloned().ok_or_else(|| {
                Diagnostic::error("state-graph dynamic selection source path is empty")
            })?;
            if !name.symbol.is_valid() {
                return Err(Diagnostic::error(
                    "state-graph dynamic selection source symbol is invalid",
                ));
            }
            Ok(SourcePlace {
                name: leaf.clone(),
                path: vec![leaf],
            })
        }
        ExpressionNode::Member(member) => {
            if !member.member_symbol.is_valid() {
                return Err(Diagnostic::error(
                    "state-graph dynamic selection member source symbol is invalid",
                ));
            }
            let mut source = exact_source_place(program, member.receiver)?;
            source.name = member.member.clone();
            source.path.push(member.member.clone());
            Ok(source)
        }
        _ => Err(Diagnostic::error(
            "state-graph dynamic selection source is not a closed retained place",
        )),
    }
}

#[derive(Debug, Clone, Copy)]
struct ExactSourceDeclaration {
    symbol: SymbolHandle,
    type_reference: TypeReferenceHandle,
}

fn exact_source_declaration(
    program: &CheckedTrees,
    machine: &psi_checked_trees::machine::Machine,
    state: &psi_checked_trees::state::State,
    statement_index: usize,
    path: &[Identifier],
) -> Result<ExactSourceDeclaration, Diagnostic> {
    let (root, rest) = path
        .split_first()
        .ok_or_else(|| Diagnostic::error("state-graph dynamic selection source path is empty"))?;
    if rest.is_empty() {
        let mut matches = Vec::new();
        for parameter in program.state_parameters(state) {
            if parameter.name == *root {
                matches.push(ExactSourceDeclaration {
                    symbol: parameter.symbol,
                    type_reference: parameter.type_reference,
                });
            }
        }
        for statement in program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .take(statement_index)
        {
            if let psi_checked_trees::statement::StatementNode::LocalData(local) = statement
                && local.name == *root
            {
                matches.push(ExactSourceDeclaration {
                    symbol: local.symbol,
                    type_reference: local.type_reference,
                });
            }
        }
        return exact_one_source_declaration(matches);
    }

    let mut current_data = if root.as_str() == "self" {
        if let Some(owned) = program
            .machine_owned_data(machine)
            .iter()
            .find(|owned| owned.name == rest[0])
        {
            if rest.len() == 1 {
                return exact_one_source_declaration(vec![ExactSourceDeclaration {
                    symbol: owned.symbol,
                    type_reference: owned.type_reference,
                }]);
            }
            exact_named_data(program, owned.type_reference)?
        } else {
            let attached = machine.attached_data.as_ref().ok_or_else(|| {
                Diagnostic::error("state-graph dynamic selection self source has no attached data")
            })?;
            let matches = program
                .data_definitions()
                .iter()
                .filter(|definition| definition.name == *attached)
                .collect::<Vec<_>>();
            let [definition] = matches.as_slice() else {
                return Err(Diagnostic::error(
                    "state-graph dynamic selection attached source data is missing or ambiguous",
                ));
            };
            *definition
        }
    } else {
        let root =
            exact_lexical_source_declaration(program, machine, state, statement_index, root)?;
        exact_named_data(program, root.type_reference)?
    };

    for (index, member_name) in rest.iter().enumerate() {
        let member = exact_data_field(program, current_data, member_name)?;
        let declaration = ExactSourceDeclaration {
            symbol: member.symbol,
            type_reference: member.type_reference,
        };
        if index + 1 == rest.len() {
            return exact_one_source_declaration(vec![declaration]);
        }
        current_data = exact_named_data(program, declaration.type_reference)?;
    }
    Err(Diagnostic::error(
        "state-graph dynamic selection source declaration is missing or ambiguous",
    ))
}

fn exact_lexical_source_declaration(
    program: &CheckedTrees,
    machine: &psi_checked_trees::machine::Machine,
    state: &psi_checked_trees::state::State,
    statement_index: usize,
    name: &Identifier,
) -> Result<ExactSourceDeclaration, Diagnostic> {
    let mut matches = Vec::new();
    for parameter in program.state_parameters(state) {
        if parameter.name == *name {
            matches.push(ExactSourceDeclaration {
                symbol: parameter.symbol,
                type_reference: parameter.type_reference,
            });
        }
    }
    for statement in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
    {
        if let psi_checked_trees::statement::StatementNode::LocalData(local) = statement
            && local.name == *name
        {
            matches.push(ExactSourceDeclaration {
                symbol: local.symbol,
                type_reference: local.type_reference,
            });
        }
    }
    for owned in program.machine_owned_data(machine) {
        if owned.name == *name {
            matches.push(ExactSourceDeclaration {
                symbol: owned.symbol,
                type_reference: owned.type_reference,
            });
        }
    }
    exact_one_source_declaration(matches)
}

fn exact_one_source_declaration(
    matches: Vec<ExactSourceDeclaration>,
) -> Result<ExactSourceDeclaration, Diagnostic> {
    match matches.as_slice() {
        [declaration] if declaration.symbol.is_valid() && declaration.type_reference.is_valid() => {
            Ok(*declaration)
        }
        _ => Err(Diagnostic::error(
            "state-graph dynamic selection source declaration is missing or ambiguous",
        )),
    }
}

fn exact_data_field<'program>(
    program: &'program CheckedTrees,
    data: &'program psi_checked_trees::data::DataDefinition,
    name: &Identifier,
) -> Result<&'program psi_checked_trees::data::DataField, Diagnostic> {
    let mut matches = Vec::new();
    for member in program.data_members(data) {
        match member {
            psi_checked_trees::data::DataMember::Field(field) if field.name == *name => {
                matches.push(field);
            }
            psi_checked_trees::data::DataMember::Variant(variant) => {
                matches.extend(
                    program
                        .data_payload_fields(variant)
                        .iter()
                        .filter(|field| field.name == *name),
                );
            }
            _ => {}
        }
    }
    match matches.as_slice() {
        [field] if field.symbol.is_valid() && field.type_reference.is_valid() => Ok(*field),
        _ => Err(Diagnostic::error(
            "state-graph dynamic selection source member is missing or ambiguous",
        )),
    }
}

fn exact_named_data<'program>(
    program: &'program CheckedTrees,
    mut reference: TypeReferenceHandle,
) -> Result<&'program psi_checked_trees::data::DataDefinition, Diagnostic> {
    let mut visited = Vec::new();
    loop {
        if !reference.is_valid() || visited.contains(&reference) {
            return Err(Diagnostic::error(
                "state-graph dynamic selection source type is invalid or cyclic",
            ));
        }
        visited.push(reference);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. } => reference = *referee,
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Named { symbol, name } => {
                if !symbol.is_valid() {
                    return Err(Diagnostic::error(
                        "state-graph dynamic selection named source type is invalid",
                    ));
                }
                let matches = program
                    .data_definitions()
                    .iter()
                    .filter(|definition| definition.symbol == *symbol)
                    .collect::<Vec<_>>();
                if let [definition] = matches.as_slice()
                    && definition.name == *name
                {
                    return Ok(*definition);
                }
                return Err(Diagnostic::error(
                    "state-graph dynamic selection named source type is missing, duplicated, or name-incoherent",
                ));
            }
            _ => {
                return Err(Diagnostic::error(
                    "state-graph dynamic selection source is not a nominal data type",
                ));
            }
        }
    }
}

fn exact_trait<'program>(
    program: &'program CheckedTrees,
    symbol: SymbolHandle,
) -> Result<&'program psi_checked_trees::trait_definition::TraitDefinition, Diagnostic> {
    if !symbol.is_valid() {
        return Err(Diagnostic::error(
            "state-graph dynamic selection trait identity is invalid",
        ));
    }
    let matches = program
        .traits()
        .iter()
        .filter(|definition| definition.symbol == symbol)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [definition] => Ok(*definition),
        _ => Err(Diagnostic::error(
            "state-graph dynamic selection trait is missing or duplicated",
        )),
    }
}

fn exact_conformance<'program>(
    program: &'program CheckedTrees,
    symbol: SymbolHandle,
) -> Result<&'program psi_checked_trees::trait_definition::Conformance, Diagnostic> {
    if !symbol.is_valid() {
        return Err(Diagnostic::error(
            "state-graph dynamic selection conformance identity is invalid",
        ));
    }
    let matches = program
        .conformances()
        .iter()
        .filter(|conformance| conformance.symbol == symbol)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [conformance] => Ok(*conformance),
        _ => Err(Diagnostic::error(
            "state-graph dynamic selection conformance is missing or duplicated",
        )),
    }
}

fn validate_rows(
    program: &CheckedTrees,
    rows: &[psi_checked_trees::trait_definition::ConformanceRow],
    selected: &[DynamicConformanceRowFact],
) -> Result<(), Diagnostic> {
    if rows.len() != selected.len() {
        return Err(Diagnostic::error(
            "state-graph dynamic selection row map is partial or expanded",
        ));
    }
    for (index, (row, selected)) in rows.iter().zip(selected).enumerate() {
        if rows[..index].iter().any(|prior| {
            prior.declaring_trait == row.declaring_trait && prior.requirement == row.requirement
        }) {
            return Err(Diagnostic::error(
                "state-graph dynamic selection requirement row is duplicated",
            ));
        }
        let declaring_trait = exact_trait(program, row.declaring_trait)?;
        if declaring_trait.name != row.declaring_trait_name {
            return Err(Diagnostic::error(
                "state-graph dynamic selection declaring-trait name drifted",
            ));
        }
        let requirements = program
            .trait_machine_signatures(declaring_trait)
            .iter()
            .filter(|requirement| requirement.symbol == row.requirement)
            .collect::<Vec<_>>();
        let [requirement] = requirements.as_slice() else {
            return Err(Diagnostic::error(
                "state-graph dynamic selection requirement is missing or duplicated in its exact trait",
            ));
        };
        if requirement.name != row.requirement_name {
            return Err(Diagnostic::error(
                "state-graph dynamic selection requirement name drifted",
            ));
        }
        exact_state(program, row.realization_machine, row.realization_state)?;
        let source = match row.source {
            psi_checked_trees::trait_definition::ConformanceRowSource::Inline => {
                DynamicConformanceRowSource::Inline
            }
            psi_checked_trees::trait_definition::ConformanceRowSource::Reference => {
                DynamicConformanceRowSource::Reference
            }
            psi_checked_trees::trait_definition::ConformanceRowSource::TraitDefault => {
                DynamicConformanceRowSource::TraitDefault
            }
        };
        let expected = DynamicConformanceRowFact {
            declaring_trait: row.declaring_trait,
            requirement: row.requirement,
            realization_machine: row.realization_machine,
            realization_state: row.realization_state,
            source,
        };
        if *selected != expected {
            return Err(Diagnostic::error(
                "state-graph dynamic selection row order or exact identity drifted",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_arena::HandleSpan;
    use psi_checked_trees::data::{DataDefinition, DataField, DataMember};
    use psi_checked_trees::expression::{
        ExpressionNode, TableBorrowExpression, TableCastExpression, TableMemberExpression,
        TableNamePath,
    };
    use psi_checked_trees::machine::Machine;
    use psi_checked_trees::signature::{StateParameter, StateSignature};
    use psi_checked_trees::state::State;
    use psi_checked_trees::statement::{StatementNode, TableLocalData};
    use psi_checked_trees::trait_definition::{
        Conformance, ConformanceImplementation, ConformanceRow, ConformanceRowSource,
        ConformanceSubject, TraitDefinition,
    };

    const ITEM: u32 = 1;
    const SHAPE: u32 = 2;
    const REQUIREMENT_A: u32 = 3;
    const CONFORMANCE: u32 = 4;
    const REALIZATION_MACHINE: u32 = 5;
    const REALIZATION_STATE_A: u32 = 6;
    const SOURCE_MACHINE: u32 = 7;
    const SOURCE_STATE: u32 = 8;
    const SOURCE_PARAMETER: u32 = 9;
    const BINDING: u32 = 10;
    const REQUIREMENT_B: u32 = 11;
    const REALIZATION_STATE_B: u32 = 12;
    const HOLDER: u32 = 13;
    const ITEM_FIELD: u32 = 14;

    #[derive(Clone, Copy)]
    enum RowShape {
        Honest,
        CrossOwned,
        DuplicateSlot,
    }

    fn symbol(index: u32) -> SymbolHandle {
        SymbolHandle::from_arena_index(index)
    }

    fn name(value: &str) -> Identifier {
        Identifier::generated(value)
    }

    fn source_name_expression(
        program: &mut CheckedTrees,
        symbol: SymbolHandle,
        source_name: &str,
    ) -> ExpressionHandle {
        let mut members = HandleSpan::empty();
        let mut member_symbols = HandleSpan::empty();
        program
            .typed
            .expression_table
            .push_name_path_member(&mut members, name(source_name));
        program
            .typed
            .expression_table
            .push_name_path_member_symbol(&mut member_symbols, symbol);
        program
            .typed
            .expression_table
            .insert(ExpressionNode::Name(TableNamePath {
                members,
                member_symbols,
                head_symbol: symbol,
                symbol,
            }))
    }

    fn fixture(member_source: bool, row_shape: RowShape) -> CheckedTrees {
        let mut program = CheckedTrees::default();
        let item_type = program
            .typed
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: symbol(ITEM),
                name: name("Item"),
            });
        program.typed.push_data_definition(DataDefinition {
            symbol: symbol(ITEM),
            name: name("Item"),
            ..Default::default()
        });

        let source_type = if member_source {
            let holder_type = program
                .typed
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: symbol(HOLDER),
                    name: name("Holder"),
                });
            let mut holder = DataDefinition {
                symbol: symbol(HOLDER),
                name: name("Holder"),
                ..Default::default()
            };
            program.typed.push_data_member(
                &mut holder,
                DataMember::Field(DataField {
                    symbol: symbol(ITEM_FIELD),
                    name: name("item"),
                    type_reference: item_type,
                    ..Default::default()
                }),
            );
            program.typed.push_data_definition(holder);
            holder_type
        } else {
            item_type
        };

        let mut shape = TraitDefinition {
            symbol: symbol(SHAPE),
            name: name("Shape"),
            ..Default::default()
        };
        for (symbol_index, requirement_name) in [(REQUIREMENT_A, "code"), (REQUIREMENT_B, "size")] {
            program.typed.push_trait_machine_signature(
                &mut shape,
                StateSignature {
                    symbol: symbol(symbol_index),
                    name: name(requirement_name),
                    ..Default::default()
                },
            );
        }
        program.typed.push_trait_definition(shape);

        let mut realization_machine = Machine {
            symbol: symbol(REALIZATION_MACHINE),
            name: name("ItemShape"),
            ..Default::default()
        };
        program.typed.push_machine_state(
            &mut realization_machine,
            State {
                symbol: symbol(REALIZATION_STATE_A),
                name: name("code"),
                ..Default::default()
            },
        );
        program.typed.push_machine_state(
            &mut realization_machine,
            State {
                symbol: symbol(REALIZATION_STATE_B),
                name: name("size"),
                ..Default::default()
            },
        );
        program.typed.push_machine(realization_machine);

        let dynamic_type =
            program
                .typed
                .type_reference_table
                .insert(TypeReferenceNode::DynamicTrait {
                    symbol: symbol(SHAPE),
                    name: name("Shape"),
                    conformance: Some(symbol(CONFORMANCE)),
                    conformance_carrier: Some(name("Item")),
                    conformance_name: Some(name("Primary")),
                });
        let dynamic_reference =
            program
                .typed
                .type_reference_table
                .insert(TypeReferenceNode::Reference {
                    referee: dynamic_type,
                    access: psi_language_semantics::ReferenceAccess::Shared,
                    lifetime: None,
                });

        let mut source_machine = Machine {
            symbol: symbol(SOURCE_MACHINE),
            name: name("Root"),
            ..Default::default()
        };
        let mut source_state = State {
            symbol: symbol(SOURCE_STATE),
            name: name("run"),
            ..Default::default()
        };
        program.typed.push_state_parameter(
            &mut source_state,
            StateParameter {
                symbol: symbol(SOURCE_PARAMETER),
                name: name(if member_source { "holder" } else { "item" }),
                type_reference: source_type,
                ..Default::default()
            },
        );
        let root = source_name_expression(
            &mut program,
            symbol(SOURCE_PARAMETER),
            if member_source { "holder" } else { "item" },
        );
        let source = if member_source {
            program
                .typed
                .expression_table
                .insert(ExpressionNode::Member(TableMemberExpression {
                    receiver: root,
                    member_symbol: symbol(ITEM_FIELD),
                    member: name("item"),
                    case_variant: None,
                }))
        } else {
            root
        };
        let borrowed = program
            .typed
            .expression_table
            .insert(ExpressionNode::Borrow(TableBorrowExpression {
                target: source,
                access: psi_language_semantics::ReferenceAccess::Mutable,
            }));
        let occurrence =
            program
                .typed
                .expression_table
                .insert(ExpressionNode::Cast(TableCastExpression {
                    value: borrowed,
                    target_type: dynamic_reference,
                    target_label: HandleSpan::empty(),
                    domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
                    semantic_domain: HandleSpan::empty(),
                    semantic_domain_arguments: HandleSpan::empty(),
                    semantic_domain_symbol: SymbolHandle::invalid(),
                    semantic_domain_id: Default::default(),
                    form: Default::default(),
                }));
        program.typed.statement_table.push_statement(
            &mut source_state.statement_nodes,
            StatementNode::LocalData(TableLocalData {
                symbol: symbol(BINDING),
                name: name("erased"),
                type_reference: dynamic_reference,
                initial_value: occurrence,
                is_mutable: false,
            }),
        );
        program
            .typed
            .push_machine_state(&mut source_machine, source_state);
        program.typed.push_machine(source_machine);

        let mut rows = vec![
            ConformanceRow {
                declaring_trait: symbol(SHAPE),
                declaring_trait_name: name("Shape"),
                requirement: symbol(REQUIREMENT_A),
                requirement_name: name("code"),
                realization_machine: symbol(REALIZATION_MACHINE),
                realization_state: symbol(REALIZATION_STATE_A),
                realization_name: name("code"),
                source: ConformanceRowSource::Inline,
            },
            ConformanceRow {
                declaring_trait: symbol(SHAPE),
                declaring_trait_name: name("Shape"),
                requirement: symbol(REQUIREMENT_B),
                requirement_name: name("size"),
                realization_machine: symbol(REALIZATION_MACHINE),
                realization_state: symbol(REALIZATION_STATE_B),
                realization_name: name("size"),
                source: ConformanceRowSource::Reference,
            },
        ];
        match row_shape {
            RowShape::Honest => {}
            RowShape::CrossOwned => rows[0].realization_machine = symbol(SOURCE_MACHINE),
            RowShape::DuplicateSlot => rows[1] = rows[0].clone(),
        }
        let selected_rows = rows
            .iter()
            .map(|row| DynamicConformanceRowFact {
                declaring_trait: row.declaring_trait,
                requirement: row.requirement,
                realization_machine: row.realization_machine,
                realization_state: row.realization_state,
                source: match row.source {
                    ConformanceRowSource::Inline => DynamicConformanceRowSource::Inline,
                    ConformanceRowSource::Reference => DynamicConformanceRowSource::Reference,
                    ConformanceRowSource::TraitDefault => DynamicConformanceRowSource::TraitDefault,
                },
            })
            .collect();
        program.typed.push_conformance(Conformance {
            symbol: symbol(CONFORMANCE),
            subject: ConformanceSubject::Carrier(name("Item")),
            trait_name: name("Shape"),
            alias: Some(name("Primary")),
            implementation: ConformanceImplementation::Closed { rows },
            ..Default::default()
        });
        program
            .facts
            .dynamic_conformances
            .selections
            .push(DynamicConformanceSelectionFact {
                occurrence,
                binding: symbol(BINDING),
                binding_name: name("erased"),
                machine: symbol(SOURCE_MACHINE),
                state: symbol(SOURCE_STATE),
                statement_index: 0,
                source_symbol: symbol(if member_source {
                    ITEM_FIELD
                } else {
                    SOURCE_PARAMETER
                }),
                source_name: name("item"),
                source_path: if member_source {
                    vec![name("holder"), name("item")]
                } else {
                    vec![name("item")]
                },
                source_data: symbol(ITEM),
                target_trait: symbol(SHAPE),
                conformance: Some(symbol(CONFORMANCE)),
                rows: selected_rows,
            });
        program
    }

    fn error(program: &CheckedTrees) -> String {
        validated_dynamic_conformance_bindings(program)
            .expect_err("invalid dynamic carrier must fail closed")
            .message
    }

    #[test]
    fn exact_direct_and_member_selections_copy_verbatim() {
        for member_source in [false, true] {
            let program = fixture(member_source, RowShape::Honest);
            let bindings = validated_dynamic_conformance_bindings(&program)
                .expect("exact dynamic selection carrier");
            assert_eq!(bindings, program.facts.dynamic_conformances.binding_facts());
            assert_eq!(bindings.selections[0].rows.len(), 2);
        }
    }

    #[test]
    fn binding_statement_occurrence_and_source_coordinates_fail_independently() {
        let mut binding = fixture(false, RowShape::Honest);
        binding.facts.dynamic_conformances.selections[0].binding = symbol(90);
        assert!(error(&binding).contains("binding disagrees"));

        let mut statement = fixture(false, RowShape::Honest);
        statement.facts.dynamic_conformances.selections[0].statement_index = 1;
        assert!(error(&statement).contains("out of range"));

        let mut occurrence = fixture(false, RowShape::Honest);
        occurrence.facts.dynamic_conformances.selections[0].occurrence =
            ExpressionHandle::invalid();
        assert!(error(&occurrence).contains("occurrence disagrees"));

        let mut source = fixture(true, RowShape::Honest);
        source.facts.dynamic_conformances.selections[0]
            .source_path
            .swap(0, 1);
        assert!(error(&source).contains("source place disagrees"));
    }

    #[test]
    fn target_source_data_and_conformance_identity_fail_closed() {
        let mut target = fixture(false, RowShape::Honest);
        target.facts.dynamic_conformances.selections[0].target_trait = symbol(90);
        assert!(error(&target).contains("target identity drifted"));

        let mut source_declaration = fixture(true, RowShape::Honest);
        source_declaration.facts.dynamic_conformances.selections[0].source_symbol =
            symbol(SOURCE_PARAMETER);
        assert!(error(&source_declaration).contains("source declaration identity drifted"));

        let mut source_data = fixture(false, RowShape::Honest);
        source_data.facts.dynamic_conformances.selections[0].source_data = symbol(HOLDER);
        assert!(error(&source_data).contains("source-data identity drifted"));

        let mut conformance = fixture(false, RowShape::Honest);
        conformance.facts.dynamic_conformances.selections[0].conformance = None;
        assert!(error(&conformance).contains("target identity drifted"));
    }

    #[test]
    fn row_map_rejects_partial_reordered_duplicate_and_cross_owned_realizations() {
        let mut partial = fixture(false, RowShape::Honest);
        partial.facts.dynamic_conformances.selections[0].rows.pop();
        assert!(error(&partial).contains("partial or expanded"));

        let mut reordered = fixture(false, RowShape::Honest);
        reordered.facts.dynamic_conformances.selections[0]
            .rows
            .swap(0, 1);
        assert!(error(&reordered).contains("row order"));

        assert!(error(&fixture(false, RowShape::DuplicateSlot)).contains("row is duplicated"));
        assert!(error(&fixture(false, RowShape::CrossOwned)).contains("cross-owned"));
    }

    #[test]
    fn duplicate_selection_occurrence_or_binding_coordinate_is_rejected() {
        let mut program = fixture(false, RowShape::Honest);
        let duplicate = program.facts.dynamic_conformances.selections[0].clone();
        program
            .facts
            .dynamic_conformances
            .selections
            .push(duplicate);
        assert!(error(&program).contains("duplicated"));
    }
}
