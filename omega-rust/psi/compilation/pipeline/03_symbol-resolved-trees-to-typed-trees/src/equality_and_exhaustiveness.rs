/*
Pre-erasure resolved-tree validation and proposition typing: bare case equality
rejection (`equality`), synthesized `Equatable` conformance prerequisites
(`equatable`), case-dispatch exhaustiveness (`exhaustiveness`), and proposition
formulas (`proposition`).
*/

pub(crate) mod equality {
    /*
    Equality vs membership (frozen decision 11): `==`/`!=` is always VALUE
    equality. A bare PAYLOAD-BEARING case name (`Command::Move`) denotes no
    value -- only its domain -- so comparing against it is an error that
    suggests `in`. The brace form (`Command::Move { dx: 1 }`) IS a constructed
    value: when an `Equatable<Type>` implementation exists the compare is
    legal and lowers through it (`crate::expressions::equatable`) in every
    position. Payload-less cases are values too; their selected comparison
    still owns the meaning. Mathematical identity uses explicit core::Identity.

    This check runs on the RESOLVED trees, BEFORE membership lowering: `in`
    is still a distinct `Membership` node here, and transition case arms
    desugar to membership at parse time, so every equality this pass sees is
    user-written. That is what lets the check cover ALL positions --
    statements, transition guard subjects/conditions, transition target
    arguments, domain proof facts, and machine
    contracts -- without re-flagging the tag compare that membership lowering
    synthesizes afterwards (the guard tag clamp survives only as that
    internal lowering of `in`).
    */

    use crate::tables::{resolved_expressions, resolved_statements};
    use diagnostics::Diagnostic;
    use resolved::SymbolResolvedTrees;
    use resolved::data::{DataDefinition, DataMember};
    use resolved::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
    use resolved::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
    use symbol_resolved_trees as resolved;

    pub(crate) fn validate_equality_operands_in(
        program: &SymbolResolvedTrees,
        cohort: &crate::lowerer::TypingCohort<'_>,
    ) -> Result<(), Diagnostic> {
        for (ordinal, machine) in program.machines.iter().enumerate() {
            if ordinal < cohort.machines && !cohort.completes(machine.symbol) {
                continue;
            }
            for contract in program.signature_contracts(machine.contracts) {
                scan_proof_facts(program, contract.facts)?;
            }

            for state_handle in program.machine_state_handles(machine.states) {
                let state = program.machine_state(*state_handle);
                for statement in resolved_statements(program).statements(state.statement_nodes) {
                    scan_statement(program, statement)?;
                }
            }
        }

        for domain in program
            .domain_definitions
            .iter()
            .skip(cohort.domain_definitions)
        {
            scan_proof_facts(program, domain.facts)?;
        }

        Ok(())
    }

    fn scan_proof_facts(
        program: &SymbolResolvedTrees,
        facts: arena::HandleSpan<resolved::domain::ProofFact>,
    ) -> Result<(), Diagnostic> {
        for fact in program.proof_facts(facts) {
            match fact {
                resolved::domain::ProofFact::Expression(expression) => {
                    scan_expression(program, *expression)?;
                }
                resolved::domain::ProofFact::Membership(membership) => {
                    scan_expression(program, membership.value)?;
                }
            }
        }
        Ok(())
    }

    fn scan_statement(
        program: &SymbolResolvedTrees,
        statement: &StatementNode,
    ) -> Result<(), Diagnostic> {
        match statement {
            StatementNode::Assembly(handle) => {
                let declarations = &program.tables.declarations;
                let graph = declarations.assembly_graphs.get(*handle);
                for block in declarations.assembly_blocks.span_or_empty(graph.blocks) {
                    for expression in declarations
                        .assembly_expressions
                        .span_or_empty(block.requires)
                        .iter()
                        .chain(
                            declarations
                                .assembly_expressions
                                .span_or_empty(block.ensures),
                        )
                    {
                        scan_expression(program, *expression)?;
                    }
                    for instruction in
                        resolved_statements(program).statements(block.statement_nodes)
                    {
                        scan_statement(program, instruction)?;
                    }
                }
                Ok(())
            }
            StatementNode::RootBinding(binding) => {
                scan_expression(program, binding.receiver)?;
                if binding.implementation_operand.is_valid() {
                    scan_expression(program, binding.implementation_operand)?;
                }
                Ok(())
            }
            StatementNode::AssemblyFact(fact) => scan_expression(program, fact.expression),
            StatementNode::Assignment(assignment) => {
                scan_expression(program, assignment.target)?;
                scan_expression(program, assignment.value)
            }
            StatementNode::Call(call) => {
                for argument in resolved_statements(program).expression_handles(call.arguments) {
                    scan_expression(program, *argument)?;
                }
                Ok(())
            }
            StatementNode::ProofOutputBindingStatement(package) => {
                scan_expression(program, package.call)
            }
            StatementNode::Expression(expression) => scan_expression(program, *expression),
            StatementNode::LocalData(local_data) => {
                scan_expression(program, local_data.initial_value)
            }
            StatementNode::Transition(transition) => {
                if let TransitionGuardNode::When(guard) = transition.guard {
                    scan_expression(program, guard)?;
                }
                scan_transition_target(program, transition.target)?;
                scan_transition_target(program, transition.continuation)
            }
        }
    }

    fn scan_transition_target(
        program: &SymbolResolvedTrees,
        target: resolved::statement::TransitionTargetHandle,
    ) -> Result<(), Diagnostic> {
        if !target.is_valid() {
            return Ok(());
        }
        match program.tables.bodies.statements.transition_target(target) {
            TransitionTargetNode::Named { arguments, .. } => {
                for argument in resolved_statements(program).expression_handles(*arguments) {
                    scan_expression(program, *argument)?;
                }
                Ok(())
            }
            TransitionTargetNode::Value(expression) => scan_expression(program, *expression),
            TransitionTargetNode::Terminal => Ok(()),
        }
    }

    fn scan_expression(
        program: &SymbolResolvedTrees,
        expression: ExpressionHandle,
    ) -> Result<(), Diagnostic> {
        if !expression.is_valid() {
            return Ok(());
        }

        let expressions = &program.tables.bodies.expressions;
        match expressions.expression(expression) {
            ExpressionNode::SavedSubject(_) => Ok(()),
            ExpressionNode::Match(dispatch) => {
                scan_expression(program, dispatch.subject)?;
                for arm in expressions.match_arms(dispatch.arms) {
                    if let resolved::expression::MatchPattern::Value(pattern) = arm.pattern {
                        check_equality_operand(program, dispatch.subject)?;
                        check_equality_operand(program, pattern)?;
                        scan_expression(program, pattern)?;
                    }
                    scan_expression(program, arm.value)?;
                }
                Ok(())
            }
            ExpressionNode::Atomic(atomic) => scan_expression(program, atomic.value),
            ExpressionNode::Binary(binary) => {
                if matches!(
                    binary.operator,
                    BinaryOperator::Equal | BinaryOperator::NotEqual
                ) {
                    for operand in [binary.left, binary.right] {
                        check_equality_operand(program, operand)?;
                    }
                }
                scan_expression(program, binary.left)?;
                scan_expression(program, binary.right)
            }
            ExpressionNode::ArrayLiteral(elements) => {
                for element in expressions.expression_handles(*elements) {
                    scan_expression(program, *element)?;
                }
                Ok(())
            }
            ExpressionNode::Cast(cast) => scan_expression(program, cast.value),
            ExpressionNode::Call(call) => {
                scan_expression(program, call.receiver)?;
                for argument in expressions.expression_handles(call.arguments) {
                    scan_expression(program, *argument)?;
                }
                Ok(())
            }
            ExpressionNode::Indexed(indexed) => {
                scan_expression(program, indexed.collection)?;
                scan_expression(program, indexed.index)
            }
            ExpressionNode::Member(member) => scan_expression(program, member.receiver),
            // The membership DOMAIN is a name path, not a value: a bare case
            // name is exactly what `in` is for, so only the subject is scanned.
            ExpressionNode::Membership(membership) => scan_expression(program, membership.value),
            ExpressionNode::Borrow(inner) => scan_expression(program, inner.target),
            ExpressionNode::Range(range) => {
                scan_expression(program, range.start)?;
                scan_expression(program, range.end)
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in expressions.struct_fields(literal.fields) {
                    scan_expression(program, field.value)?;
                }
                Ok(())
            }
            ExpressionNode::Unary(unary) => scan_expression(program, unary.operand),
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::String(_)
            | ExpressionNode::TypeExpression(_)
            | ExpressionNode::ZeroValue(_) => Ok(()),
        }
    }

    fn check_equality_operand(
        program: &SymbolResolvedTrees,
        operand: ExpressionHandle,
    ) -> Result<(), Diagnostic> {
        match program.tables.bodies.expressions.expression(operand) {
            ExpressionNode::Name(path) => {
                let members = resolved_expressions(program).name_path_members(path.members);
                let [type_name, case_name] = members else {
                    return Ok(());
                };
                let Some((data, case_name)) = payload_bearing_case(
                    program,
                    type_name.as_str(),
                    case_name.as_str(),
                    program.tables.bodies.expressions.source_span(operand),
                ) else {
                    return Ok(());
                };
                let data_name = data.name.as_str();
                Err(Diagnostic::error(format!(
                    "`{data_name}::{case_name}` carries a payload, so the bare case name is not a value -- use `in` to test the case (`value in {data_name}::{case_name}`), or compare against a constructed value"
                )))
            }
            ExpressionNode::StructLiteral(literal) => {
                let Some(case_name) = literal.case_name.as_ref() else {
                    return Ok(());
                };
                let Some((data, case_name)) = payload_bearing_case(
                    program,
                    literal.type_name.as_str(),
                    case_name.as_str(),
                    program.tables.bodies.expressions.source_span(operand),
                ) else {
                    return Ok(());
                };
                let data_name = data.name.as_str();
                if !matches!(
                    crate::expressions::equatable::equality_selection(program, data),
                    crate::expressions::equatable::EqualitySelection::Missing
                ) {
                    return Ok(());
                }
                Err(Diagnostic::error(format!(
                    "cannot compare against payload-bearing case `{data_name}::{case_name}` with `==`: structural equality is synthesized only through a named implementation -- match on the case in a transition to bind its payload, or declare `{}` to synthesize structural equality, or use `core::Identity` with complete values and their carrier for mathematical identity",
                    crate::expressions::equatable::suggested_equality_declaration(data_name)
                )))
            }
            _ => Ok(()),
        }
    }

    fn payload_bearing_case<'program>(
        program: &'program SymbolResolvedTrees,
        type_name: &str,
        case_name: &str,
        reference: source::SourceSpan,
    ) -> Option<(&'program DataDefinition, String)> {
        let data_definition = crate::expressions::equatable::data_definition_by_name_from(
            program, type_name, reference,
        )?;

        program
            .data_members(data_definition.members)
            .iter()
            .find_map(|member| match member {
                DataMember::Variant(variant)
                    if variant.name.as_str() == case_name && variant.payload.count() > 0 =>
                {
                    Some((data_definition, case_name.to_owned()))
                }
                _ => None,
            })
    }
}

pub(crate) mod equatable {
    use crate::tables::{resolved_statements, type_constraints, type_references};
    use diagnostics::Diagnostic;
    use resolved::SymbolResolvedTrees;
    use resolved::data::{DataDefinition, DataField, DataMember, DataShapeKind};
    use resolved::trait_definition::{Conformance, ConformanceRow, ConformanceRowSource};
    use symbol_resolved_trees as resolved;
    use typed_trees::types::PrimitiveType;

    const EQUATABLE_TRAIT_PATH: &str = "Equatable";
    const EQUALS_REQUIREMENT: &str = "equals";

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum DataEqualityShape {
        Implicit,
        Structural,
    }

    pub(crate) fn data_definition_by_name_from<'program>(
        program: &'program SymbolResolvedTrees,
        type_name: &str,
        reference: source::SourceSpan,
    ) -> Option<&'program DataDefinition> {
        if reference.span.start != reference.span.end
            && let Some(symbol) = program
                .symbols
                .find_top_level_by_name_and_kinds_from_source(
                    type_name,
                    &[symbols::SymbolKind::Data],
                    reference,
                )
        {
            return data_definition_by_symbol(program, symbol);
        }
        let mut matches = program
            .data_definitions
            .iter()
            .filter(|definition| definition.name.as_str() == type_name);
        let first = matches.next()?;
        matches.next().is_none().then_some(first)
    }

    pub(crate) fn data_definition_by_symbol(
        program: &SymbolResolvedTrees,
        symbol: symbols::SymbolHandle,
    ) -> Option<&DataDefinition> {
        program.data_definition_by_symbol(symbol)
    }

    fn canonical_data_name(
        program: &SymbolResolvedTrees,
        symbol: symbols::SymbolHandle,
        authored: &resolved::name::DiagnosticName,
    ) -> String {
        data_definition_by_symbol(program, symbol)
            .map(|definition| definition.name.as_str().to_owned())
            .unwrap_or_else(|| authored.as_str().to_owned())
    }

    pub(crate) fn data_equality_shape(
        program: &SymbolResolvedTrees,
        data: &DataDefinition,
    ) -> DataEqualityShape {
        let members = program.data_members(data.members);
        match DataDefinition::shape_kind_from_members(members) {
            DataShapeKind::Empty => DataEqualityShape::Implicit,
            DataShapeKind::Record | DataShapeKind::Mixed => DataEqualityShape::Structural,
            DataShapeKind::Enum => {
                let payload_bearing = members.iter().any(|member| {
                matches!(member, DataMember::Variant(variant) if variant.payload.count() > 0)
            });
                if payload_bearing {
                    DataEqualityShape::Structural
                } else {
                    DataEqualityShape::Implicit
                }
            }
        }
    }

    pub(crate) enum EqualitySelection<'program> {
        Missing,
        Selected(EqualityImplementation<'program>),
        Ambiguous(Vec<String>),
    }

    pub(crate) struct EqualityImplementation<'program> {
        pub(crate) conformance: &'program Conformance,
        pub(crate) equals_state: symbols::SymbolHandle,
        pub(crate) synthesized: bool,
    }

    fn is_equatable_implementation(
        program: &SymbolResolvedTrees,
        conformance: &Conformance,
    ) -> bool {
        program.symbols.display_path(conformance.trait_symbol, "::") == EQUATABLE_TRAIT_PATH
    }

    pub(crate) fn equatable_operand_data<'program>(
        program: &'program SymbolResolvedTrees,
        conformance: &Conformance,
    ) -> Option<&'program DataDefinition> {
        let [resolved::types::TypeReference::Named { symbol, name, .. }] =
            program.child_type_references(conformance.arguments)
        else {
            return None;
        };
        if symbol.is_valid() {
            data_definition_by_symbol(program, *symbol)
        } else {
            data_definition_by_name_from(program, name.as_str(), name.source_span())
        }
    }

    pub(crate) fn implementation_name(
        program: &SymbolResolvedTrees,
        conformance: &Conformance,
    ) -> String {
        conformance.alias.as_ref().map_or_else(
            || program.symbols.display_path(conformance.symbol, "::"),
            |alias| alias.as_str().to_owned(),
        )
    }

    fn equals_row(conformance: &Conformance) -> Option<&ConformanceRow> {
        let rows = &conformance.rows;
        rows.iter()
            .find(|row| row.requirement_name.as_str() == EQUALS_REQUIREMENT)
    }

    pub(crate) fn equality_selection<'program>(
        program: &'program SymbolResolvedTrees,
        data: &DataDefinition,
    ) -> EqualitySelection<'program> {
        let candidates = program
            .conformances
            .iter()
            .filter(|conformance| {
                is_equatable_implementation(program, conformance)
                    && equatable_operand_data(program, conformance)
                        .is_some_and(|operand| operand.symbol == data.symbol)
            })
            .collect::<Vec<_>>();
        match candidates.as_slice() {
            [] => EqualitySelection::Missing,
            [conformance] => {
                let row = equals_row(conformance);
                EqualitySelection::Selected(EqualityImplementation {
                    conformance,
                    equals_state: row
                        .map_or_else(symbols::SymbolHandle::invalid, |row| row.realization_state),
                    synthesized: row
                        .is_some_and(|row| row.source == ConformanceRowSource::Synthesized),
                })
            }
            _ => EqualitySelection::Ambiguous(
                candidates
                    .iter()
                    .map(|conformance| implementation_name(program, conformance))
                    .collect(),
            ),
        }
    }

    pub(crate) fn synthesized_equality_row(
        program: &SymbolResolvedTrees,
        state: symbols::SymbolHandle,
    ) -> Option<&DataDefinition> {
        program.conformances.iter().find_map(|conformance| {
            (is_equatable_implementation(program, conformance)
                && equals_row(conformance).is_some_and(|row| {
                    row.source == ConformanceRowSource::Synthesized
                        && row.realization_state == state
                }))
            .then(|| equatable_operand_data(program, conformance))
            .flatten()
        })
    }

    pub(crate) fn suggested_equality_declaration(type_name: &str) -> String {
        format!("{type_name}Equality satisfies Equatable<{type_name}> {{ }}")
    }

    pub(crate) enum FieldEquality<'program> {
        Direct,
        Text,
        Structural(&'program DataDefinition),
    }

    pub(crate) fn field_equality<'program>(
        program: &'program SymbolResolvedTrees,
        context: impl Fn() -> String,
        owner_data: &DataDefinition,
        field: &DataField,
    ) -> Result<FieldEquality<'program>, Diagnostic> {
        let owner = owner_data.name.as_str();
        if byte_sequence_text_carrier(program, &field.type_reference) {
            return Ok(FieldEquality::Text);
        }

        let Some(base_name) = value_type_base_name(program, &field.type_reference) else {
            return Err(Diagnostic::error(format!(
                "{}: field `{}` of `{owner}` is not Equatable: only scalar primitives, payload-less sums, and data types with an Equatable implementation participate in synthesized structural equality",
                context(),
                field.name
            )));
        };

        if PrimitiveType::from_name(&base_name).is_some() {
            return Ok(FieldEquality::Direct);
        }

        let Some(field_data) = data_definition_by_name_from(
            program,
            &base_name,
            program
                .symbols
                .symbol_source_span(owner_data.symbol)
                .unwrap_or_default(),
        ) else {
            return Err(Diagnostic::error(format!(
                "{}: field `{}` of `{owner}` is not Equatable: `{base_name}` is not a comparable data type",
                context(),
                field.name
            )));
        };

        if field_data.quotient.is_some() {
            return Err(Diagnostic::error(format!(
                "{}: field `{}` of `{owner}` has quotient type `{base_name}`; quotient representatives have no synthesized structural equality, so use a named lifted equality operation instead",
                context(),
                field.name
            )));
        }

        match data_equality_shape(program, field_data) {
            DataEqualityShape::Implicit => Ok(FieldEquality::Direct),
            DataEqualityShape::Structural => match equality_selection(program, field_data) {
                EqualitySelection::Selected(_) => Ok(FieldEquality::Structural(field_data)),
                EqualitySelection::Missing => Err(Diagnostic::error(format!(
                    "{}: field `{}` of `{owner}` is not Equatable: add `{}`",
                    context(),
                    field.name,
                    suggested_equality_declaration(&base_name)
                ))),
                EqualitySelection::Ambiguous(implementations) => Err(Diagnostic::error(format!(
                    "{}: field `{}` of `{owner}` has {} Equatable implementations for `{base_name}` (`{}`); structural equality selects at most one",
                    context(),
                    field.name,
                    implementations.len(),
                    implementations.join("`, `")
                ))),
            },
        }
    }

    pub(crate) fn validate_equatable_conformances_in(
        program: &SymbolResolvedTrees,
        cohort: &crate::lowerer::TypingCohort<'_>,
    ) -> Result<(), Diagnostic> {
        let skip = if cohort.data_definitions == program.data_definitions.len() {
            cohort.conformances
        } else {
            0
        };
        for conformance in program.conformances.iter().skip(skip) {
            if !is_equatable_implementation(program, conformance)
                || !equals_row(conformance)
                    .is_some_and(|row| row.source == ConformanceRowSource::Synthesized)
            {
                continue;
            }
            let Some(data) = equatable_operand_data(program, conformance) else {
                continue;
            };
            let type_name = data.name.as_str();
            let context = format!(
                "conformance `{} satisfies Equatable<{type_name}>`",
                implementation_name(program, conformance)
            );
            if data.quotient.is_some() {
                return Err(Diagnostic::error(format!(
                    "{context} cannot synthesize equality for a quotient type; quotient representatives are opaque, so equality requires a named lifted operation"
                )));
            }

            let mut visiting = vec![type_name.to_owned()];
            validate_equatable_data(program, &context, data, &mut visiting)?;
        }
        Ok(())
    }

    fn validate_equatable_data(
        program: &SymbolResolvedTrees,
        context: &str,
        data: &DataDefinition,
        visiting: &mut Vec<String>,
    ) -> Result<(), Diagnostic> {
        let owner = data.name.as_str();
        for member in program.data_members(data.members) {
            let fields: &[DataField] = match member {
                DataMember::Field(field) => std::slice::from_ref(field),
                DataMember::Variant(variant) => program.data_payload_fields(variant.payload),
            };
            for field in fields {
                let FieldEquality::Structural(nested) =
                    field_equality(program, || context.to_owned(), data, field)?
                else {
                    continue;
                };
                let nested_name = nested.name.as_str();
                if visiting.iter().any(|name| name == nested_name) {
                    return Err(Diagnostic::error(format!(
                        "{context}: type `{nested_name}` is recursive through field `{}` of `{owner}`: synthesized structural equality would not terminate, so recursive Equatable implementations are not supported yet",
                        field.name
                    )));
                }
                visiting.push(nested_name.to_owned());
                validate_equatable_data(program, context, nested, visiting)?;
                visiting.pop();
            }
        }
        Ok(())
    }

    pub(crate) fn byte_slice_text_view(
        program: &SymbolResolvedTrees,
        type_reference: &resolved::types::TypeReference,
    ) -> bool {
        let resolved::types::TypeReference::Reference(reference) = type_reference else {
            return false;
        };
        let mut referee = program.child_type_reference(reference.storage.referee);
        while let resolved::types::TypeReference::Constrained(constrained) = referee {
            referee = program.child_type_reference(constrained.storage.base_type);
        }
        let resolved::types::TypeReference::Slice(slice) = referee else {
            return false;
        };
        matches!(
            program.child_type_reference(slice.storage.element_type),
            resolved::types::TypeReference::Named { name, .. }
                if PrimitiveType::from_name(name.as_str()) == Some(PrimitiveType::U8)
        )
    }

    pub(crate) fn byte_sequence_text_carrier(
        program: &SymbolResolvedTrees,
        type_reference: &resolved::types::TypeReference,
    ) -> bool {
        if byte_slice_text_view(program, type_reference) {
            return true;
        }
        let resolved::types::TypeReference::Constrained(constrained) = type_reference else {
            return false;
        };
        if !type_constraints(program)
            .span_or_empty(constrained.storage.constraints)
            .iter()
            .any(|constraint| matches!(constraint, resolved::types::TypeConstraint::Domain(_)))
        {
            return false;
        }
        let resolved::types::TypeReference::FixedArray(array) =
            program.child_type_reference(constrained.storage.base_type)
        else {
            return false;
        };
        matches!(
            array.storage.length,
            resolved::types::FixedArrayLength::Literal(_)
        ) && matches!(
            program.child_type_reference(array.storage.element_type),
            resolved::types::TypeReference::Named { name, .. }
                if PrimitiveType::from_name(name.as_str()) == Some(PrimitiveType::U8)
        )
    }

    pub(crate) fn value_type_base_name(
        program: &SymbolResolvedTrees,
        type_reference: &resolved::types::TypeReference,
    ) -> Option<String> {
        match type_reference {
            resolved::types::TypeReference::Reference(reference) => value_type_base_name(
                program,
                program.child_type_reference(reference.storage.referee),
            ),
            resolved::types::TypeReference::Constrained(constrained) => value_type_base_name(
                program,
                program.child_type_reference(constrained.storage.base_type),
            ),
            resolved::types::TypeReference::Named { symbol, name, .. } => {
                Some(canonical_data_name(program, *symbol, name))
            }
            _ => None,
        }
    }

    pub(crate) fn table_type_base_name(
        program: &SymbolResolvedTrees,
        type_reference: resolved::types::TypeReferenceHandle,
    ) -> Option<String> {
        if !type_reference.is_valid() {
            return None;
        }
        match type_references(program).type_reference(type_reference) {
            resolved::types::TypeReferenceNode::Reference { referee, .. } => {
                table_type_base_name(program, *referee)
            }
            resolved::types::TypeReferenceNode::Constrained { base_type, .. } => {
                table_type_base_name(program, *base_type)
            }
            resolved::types::TypeReferenceNode::Named { symbol, name } => {
                Some(canonical_data_name(program, *symbol, name))
            }
            _ => None,
        }
    }

    #[derive(Debug, Clone)]
    pub(crate) struct EqualityScope {
        pub(crate) value_types: Vec<(String, String)>,
        pub(crate) synthesized_row_data: symbols::SymbolHandle,
    }

    impl EqualityScope {
        pub(crate) fn for_state_interface(
            program: &SymbolResolvedTrees,
            machine: &resolved::machine::Machine,
            state: &resolved::state::State,
        ) -> Self {
            let mut value_types = Vec::new();
            for owned in program.machine_owned_data(machine.owned_data) {
                if let Some(base) = value_type_base_name(program, &owned.type_reference) {
                    value_types.push((owned.name.as_str().to_owned(), base));
                }
            }
            for parameter in program.state_parameters(state.parameters) {
                if let Some(base) = value_type_base_name(program, &parameter.type_reference) {
                    value_types.push((parameter.name.as_str().to_owned(), base));
                }
            }
            let synthesized_row_data = program
                .conformances
                .iter()
                .find(|conformance| {
                    is_equatable_implementation(program, conformance)
                        && equals_row(conformance).is_some_and(|row| {
                            row.source == ConformanceRowSource::Synthesized
                                && row.realization_machine == machine.symbol
                        })
                })
                .and_then(|conformance| equatable_operand_data(program, conformance))
                .map_or_else(symbols::SymbolHandle::invalid, |data| data.symbol);
            Self {
                value_types,
                synthesized_row_data,
            }
        }

        pub(crate) fn for_state(
            program: &SymbolResolvedTrees,
            machine: &resolved::machine::Machine,
            state: &resolved::state::State,
        ) -> Self {
            let mut scope = Self::for_state_interface(program, machine, state);
            for statement in resolved_statements(program).statements(state.statement_nodes) {
                if let resolved::statement::StatementNode::LocalData(local) = statement
                    && let Some(base) = table_type_base_name(program, local.type_reference)
                {
                    scope
                        .value_types
                        .push((local.name.as_str().to_owned(), base));
                }
            }

            scope
        }

        pub(crate) fn value_type(&self, name: &str) -> Option<&str> {
            self.value_types
                .iter()
                .rev()
                .find(|(value_name, _)| value_name == name)
                .map(|(_, type_name)| type_name.as_str())
        }
    }
}

pub(crate) mod exhaustiveness {
    use crate::expressions::call_results::declared_call_return;
    use crate::tables::{resolved_expressions, resolved_statements};
    use arena::HandleSpan;
    use diagnostics::Diagnostic;
    use resolved::SymbolResolvedTrees;
    use resolved::data::{DataDefinition, DataMember, DataVariant};
    use resolved::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, TableNamePath};
    use resolved::name::DiagnosticName;
    use resolved::statement::{StatementNode, TransitionGuardNode};
    use resolved::types::TypeReference;
    use symbol_resolved_trees as resolved;
    use symbols::SymbolHandle;

    pub(crate) fn validate_case_dispatch_exhaustiveness_in(
        program: &SymbolResolvedTrees,
        cohort: &crate::lowerer::TypingCohort<'_>,
    ) -> Result<(), Diagnostic> {
        for (ordinal, machine) in program.machines.iter().enumerate() {
            if ordinal < cohort.machines && !cohort.completes(machine.symbol) {
                continue;
            }
            for state_handle in program.machine_state_handles(machine.states) {
                let state = program.machine_state(*state_handle);
                let scope = DispatchScope {
                    program,
                    machine,
                    state,
                };
                let statements = resolved_statements(program).statements(state.statement_nodes);
                let location =
                    if machine.name.as_str().rsplit("::").next() == Some(state.name.as_str()) {
                        machine.name.as_str().to_owned()
                    } else {
                        format!("{}::{}", machine.name, state.name)
                    };

                let mut run = Vec::new();
                let mut run_has_continuation = false;
                for statement in statements {
                    match statement {
                        StatementNode::Transition(transition) => {
                            run.push(&transition.guard);
                            run_has_continuation |= transition.continuation.is_valid();
                            if transition.ends_dispatch {
                                check_dispatch_run(&scope, &run, run_has_continuation, &location)?;
                                run.clear();
                                run_has_continuation = false;
                            }
                        }
                        _ => {
                            check_dispatch_run(&scope, &run, run_has_continuation, &location)?;
                            run.clear();
                            run_has_continuation = false;
                        }
                    }
                }
                check_dispatch_run(&scope, &run, run_has_continuation, &location)?;
            }
        }

        Ok(())
    }

    enum ArmShape {
        Default,
        Finite(Vec<FiniteConstraint>),
        UncountedCase(CaseClaim),
        Compare {
            left: ExpressionHandle,
            right: ExpressionHandle,
            negated: bool,
        },
        Opaque,
    }

    enum FiniteConstraint {
        Case(CaseClaim),
        Bool {
            subject: ExpressionHandle,
            value: bool,
        },
    }

    struct CaseClaim {
        data_index: usize,
        subject: ExpressionHandle,
        covered: Option<Vec<usize>>,
    }

    enum LeafContribution {
        Tags {
            data_index: usize,
            variants: Vec<usize>,
        },
        Predicate {
            data_index: usize,
        },
        NotCase,
    }

    struct DispatchScope<'a> {
        program: &'a SymbolResolvedTrees,
        machine: &'a resolved::machine::Machine,
        state: &'a resolved::state::State,
    }

    fn check_dispatch_run(
        scope: &DispatchScope<'_>,
        run: &[&TransitionGuardNode],
        has_continuation: bool,
        location: &str,
    ) -> Result<(), Diagnostic> {
        let program = scope.program;
        if run.is_empty() {
            return Ok(());
        }
        if has_continuation {
            return Ok(());
        }

        let mut finite_arms: Vec<Vec<FiniteConstraint>> = Vec::new();
        let mut uncounted_claims: Vec<CaseClaim> = Vec::new();
        let mut compare_arms: Vec<(ExpressionHandle, ExpressionHandle, bool)> = Vec::new();
        let mut has_opaque_arm = false;

        for guard in run {
            match classify_arm(scope, guard) {
                ArmShape::Default => return Ok(()),
                ArmShape::Finite(constraints) => finite_arms.push(constraints),
                ArmShape::UncountedCase(claim) => uncounted_claims.push(claim),
                ArmShape::Compare {
                    left,
                    right,
                    negated,
                } => compare_arms.push((left, right, negated)),
                ArmShape::Opaque => has_opaque_arm = true,
            }
        }

        let compare_pair_closes = compare_arms.iter().any(|(left, right, negated)| {
            !*negated
                && compare_arms
                    .iter()
                    .any(|(other_left, other_right, other_negated)| {
                        *other_negated
                            && expressions_structurally_equal(program, *left, *other_left)
                            && expressions_structurally_equal(program, *right, *other_right)
                    })
        });
        if compare_pair_closes {
            return Ok(());
        }

        let axes = finite_axes(program, &finite_arms, &uncounted_claims);
        if !axes.is_empty() {
            match first_uncovered_assignment(program, &axes, &finite_arms) {
                MatrixCoverage::Complete => return Ok(()),
                MatrixCoverage::TooLarge => {
                    return Err(Diagnostic::error(format!(
                        "transition pattern matrix in `{location}` is too large for the exhaustiveness proof; add a `_` arm"
                    )));
                }
                MatrixCoverage::Uncovered(assignment) => {
                    if axes
                        .iter()
                        .all(|axis| matches!(axis.kind, FiniteAxisKind::Bool))
                    {
                        return Err(Diagnostic::error(format!(
                            "transition dispatch in `{location}` can fall through: no arm matches when every guard is false, and no `_` arm exists; add a `_ ->` arm (or complete the `true`/`false` pair)"
                        )));
                    }
                    if axes.len() == 1
                        && let FiniteAxisKind::Case { data_index } = axes[0].kind
                    {
                        let data_definition = &program.data_definitions[data_index];
                        let mut covered = vec![true; case_count(program, data_definition)];
                        for variant in &axes[0].possible_cases {
                            covered[*variant] = false;
                        }
                        for constraints in &finite_arms {
                            for constraint in constraints {
                                if let FiniteConstraint::Case(claim) = constraint
                                    && claim.data_index == data_index
                                    && expressions_structurally_equal(
                                        program,
                                        axes[0].subject,
                                        claim.subject,
                                    )
                                    && let Some(variants) = &claim.covered
                                {
                                    for variant in variants {
                                        covered[*variant] = true;
                                    }
                                }
                            }
                        }
                        let type_name = data_definition.name.as_str();
                        if !uncounted_claims.is_empty() || has_opaque_arm {
                            return Err(Diagnostic::error(format!(
                                "match over `{type_name}` is not exhaustive: it relies on arms the compiler cannot count (predicate domains, guarded patterns, or value compares); add a `_` arm"
                            )));
                        }
                        let uncovered = uncovered_case_names(program, data_definition, &covered);
                        return Err(Diagnostic::error(format!(
                            "match over `{type_name}` does not cover {}; add an arm or `_`",
                            uncovered
                                .iter()
                                .map(|case| format!("`{type_name}::{case}`"))
                                .collect::<Vec<_>>()
                                .join(", ")
                        )));
                    }

                    if !uncounted_claims.is_empty() || has_opaque_arm {
                        return Err(Diagnostic::error(format!(
                            "transition pattern matrix in `{location}` is not exhaustive: it relies on arms the compiler cannot count (predicate domains, guarded patterns, or value compares); add a `_` arm"
                        )));
                    }
                    return Err(Diagnostic::error(format!(
                        "transition pattern matrix in `{location}` does not cover {}; add an arm or `_`",
                        format_assignment(program, &axes, &assignment)
                    )));
                }
            }
        }

        Err(Diagnostic::error(format!(
            "transition dispatch in `{location}` can fall through: no arm matches when every \
         guard is false, and no `_` arm exists; add a `_ ->` arm (or complete the \
         `true`/`false` pair)"
        )))
    }

    struct FiniteAxis {
        subject: ExpressionHandle,
        kind: FiniteAxisKind,
        possible_cases: Vec<usize>,
    }

    #[derive(Clone, Copy)]
    enum FiniteAxisKind {
        Case { data_index: usize },
        Bool,
    }

    enum MatrixCoverage {
        Complete,
        Uncovered(Vec<usize>),
        TooLarge,
    }

    fn finite_axes(
        program: &SymbolResolvedTrees,
        arms: &[Vec<FiniteConstraint>],
        uncounted_claims: &[CaseClaim],
    ) -> Vec<FiniteAxis> {
        let mut axes = Vec::new();
        for constraint in arms.iter().flatten() {
            match constraint {
                FiniteConstraint::Case(claim) => push_axis(
                    program,
                    &mut axes,
                    claim.subject,
                    FiniteAxisKind::Case {
                        data_index: claim.data_index,
                    },
                ),
                FiniteConstraint::Bool { subject, .. } => {
                    push_axis(program, &mut axes, *subject, FiniteAxisKind::Bool)
                }
            }
        }
        for claim in uncounted_claims {
            push_axis(
                program,
                &mut axes,
                claim.subject,
                FiniteAxisKind::Case {
                    data_index: claim.data_index,
                },
            );
        }
        axes
    }

    fn push_axis(
        program: &SymbolResolvedTrees,
        axes: &mut Vec<FiniteAxis>,
        subject: ExpressionHandle,
        kind: FiniteAxisKind,
    ) {
        if axes.iter().any(|axis| {
            axis_kinds_equal(axis.kind, kind)
                && expressions_structurally_equal(program, axis.subject, subject)
        }) {
            return;
        }
        let possible_cases = match kind {
            FiniteAxisKind::Case { data_index } => {
                possible_case_indexes(program, &program.data_definitions[data_index])
            }
            FiniteAxisKind::Bool => Vec::new(),
        };
        axes.push(FiniteAxis {
            subject,
            kind,
            possible_cases,
        });
    }

    fn possible_case_indexes(
        program: &SymbolResolvedTrees,
        definition: &DataDefinition,
    ) -> Vec<usize> {
        program
            .data_members(definition.members)
            .iter()
            .filter_map(|member| match member {
                DataMember::Variant(variant) => Some(variant),
                DataMember::Field(_) => None,
            })
            .enumerate()
            .filter_map(|(variant_index, variant)| {
                (!variant_is_impossible(program, variant)).then_some(variant_index)
            })
            .collect()
    }

    fn variant_is_impossible(program: &SymbolResolvedTrees, variant: &DataVariant) -> bool {
        program
            .proof_facts(variant.requires_facts)
            .iter()
            .any(|fact| match fact {
                resolved::domain::ProofFact::Expression(expression) => {
                    match program.tables.bodies.expressions.expression(*expression) {
                        ExpressionNode::Integer(literal) => literal.decimal_value_i128() == Some(0),
                        ExpressionNode::Boolean(value) => !*value,
                        _ => false,
                    }
                }
                resolved::domain::ProofFact::Membership(_) => false,
            })
    }

    fn axis_kinds_equal(left: FiniteAxisKind, right: FiniteAxisKind) -> bool {
        match (left, right) {
            (FiniteAxisKind::Bool, FiniteAxisKind::Bool) => true,
            (
                FiniteAxisKind::Case { data_index: left },
                FiniteAxisKind::Case { data_index: right },
            ) => left == right,
            _ => false,
        }
    }

    fn first_uncovered_assignment(
        program: &SymbolResolvedTrees,
        axes: &[FiniteAxis],
        arms: &[Vec<FiniteConstraint>],
    ) -> MatrixCoverage {
        const MAX_MATRIX_CELLS: usize = 65_536;

        let arities: Vec<usize> = axes
            .iter()
            .map(|axis| match axis.kind {
                FiniteAxisKind::Case { .. } => axis.possible_cases.len(),
                FiniteAxisKind::Bool => 2,
            })
            .collect();
        let Some(cell_count) = arities
            .iter()
            .try_fold(1usize, |product, arity| product.checked_mul(*arity))
        else {
            return MatrixCoverage::TooLarge;
        };
        if cell_count > MAX_MATRIX_CELLS {
            return MatrixCoverage::TooLarge;
        }

        for ordinal in 0..cell_count {
            let mut remainder = ordinal;
            let mut assignment = Vec::with_capacity(axes.len());
            for arity in &arities {
                assignment.push(remainder % arity);
                remainder /= arity;
            }
            if !arms
                .iter()
                .any(|arm| arm_matches(program, axes, &assignment, arm))
            {
                return MatrixCoverage::Uncovered(assignment);
            }
        }
        MatrixCoverage::Complete
    }

    fn arm_matches(
        program: &SymbolResolvedTrees,
        axes: &[FiniteAxis],
        assignment: &[usize],
        constraints: &[FiniteConstraint],
    ) -> bool {
        constraints.iter().all(|constraint| {
            let (subject, kind) = match constraint {
                FiniteConstraint::Case(claim) => (
                    claim.subject,
                    FiniteAxisKind::Case {
                        data_index: claim.data_index,
                    },
                ),
                FiniteConstraint::Bool { subject, .. } => (*subject, FiniteAxisKind::Bool),
            };
            let Some(axis_index) = axes.iter().position(|axis| {
                axis_kinds_equal(axis.kind, kind)
                    && expressions_structurally_equal(program, axis.subject, subject)
            }) else {
                return false;
            };
            match constraint {
                FiniteConstraint::Case(claim) => {
                    let variant = axes[axis_index].possible_cases[assignment[axis_index]];
                    claim
                        .covered
                        .as_ref()
                        .is_some_and(|variants| variants.contains(&variant))
                }
                FiniteConstraint::Bool { value, .. } => {
                    assignment[axis_index] == usize::from(*value)
                }
            }
        })
    }

    fn format_assignment(
        program: &SymbolResolvedTrees,
        axes: &[FiniteAxis],
        assignment: &[usize],
    ) -> String {
        let cells = axes
            .iter()
            .zip(assignment)
            .map(|(axis, value)| match axis.kind {
                FiniteAxisKind::Bool => format!("`{}`", value != &0),
                FiniteAxisKind::Case { data_index } => {
                    let definition = &program.data_definitions[data_index];
                    let variant = axis.possible_cases.get(*value).copied().unwrap_or(*value);
                    let case = program
                        .data_members(definition.members)
                        .iter()
                        .filter_map(|member| match member {
                            DataMember::Variant(variant) => Some(variant.name.as_str()),
                            DataMember::Field(_) => None,
                        })
                        .nth(variant)
                        .unwrap_or("<unknown>");
                    format!("`{}::{case}`", definition.name.as_str())
                }
            })
            .collect::<Vec<_>>();
        if cells.len() == 1 {
            cells[0].clone()
        } else {
            format!("({})", cells.join(", "))
        }
    }

    pub(crate) fn closed_boolean_guard(
        program: &SymbolResolvedTrees,
        expression: ExpressionHandle,
    ) -> Option<bool> {
        let expressions = &program.tables.bodies.expressions;
        match expressions.expression(expression) {
            ExpressionNode::Boolean(value) => Some(*value),
            ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Equal => {
                match (
                    expressions.expression(binary.left),
                    expressions.expression(binary.right),
                ) {
                    (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) => {
                        Some(left == right)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn closed_boolean_guard_through_names(
        program: &SymbolResolvedTrees,
        expression: ExpressionHandle,
    ) -> Option<bool> {
        if let Some(value) = closed_boolean_guard(program, expression) {
            return Some(value);
        }
        let ExpressionNode::Name(path) = program.tables.bodies.expressions.expression(expression)
        else {
            return None;
        };
        let declaration = program.const_declaration(path.symbol)?;
        closed_boolean_guard(program, declaration.initializer)
    }

    fn classify_arm(scope: &DispatchScope<'_>, guard: &TransitionGuardNode) -> ArmShape {
        let program = scope.program;
        let TransitionGuardNode::When(expression) = guard else {
            return ArmShape::Default;
        };

        let mut conjuncts = Vec::new();
        flatten_binary(program, *expression, BinaryOperator::And, &mut conjuncts);

        if let [conjunct] = conjuncts[..] {
            if let Some(claim) = classify_membership_union(scope, conjunct) {
                return if claim.covered.is_some() {
                    ArmShape::Finite(vec![FiniteConstraint::Case(claim)])
                } else {
                    ArmShape::UncountedCase(claim)
                };
            }
            if let Some(value) = closed_boolean_guard(program, conjunct) {
                return if value {
                    ArmShape::Default
                } else {
                    ArmShape::Opaque
                };
            }
            if let ExpressionNode::Binary(binary) =
                program.tables.bodies.expressions.expression(conjunct)
            {
                if binary.operator == BinaryOperator::Equal {
                    if let ExpressionNode::Boolean(value) =
                        program.tables.bodies.expressions.expression(binary.right)
                    {
                        let value = *value;
                        if let Some(subject) =
                            closed_boolean_guard_through_names(program, binary.left)
                        {
                            return if subject == value {
                                ArmShape::Default
                            } else {
                                ArmShape::Opaque
                            };
                        }
                        return ArmShape::Finite(vec![FiniteConstraint::Bool {
                            subject: binary.left,
                            value,
                        }]);
                    }
                    if let ExpressionNode::Boolean(value) =
                        program.tables.bodies.expressions.expression(binary.left)
                    {
                        let value = *value;
                        if let Some(subject) =
                            closed_boolean_guard_through_names(program, binary.right)
                        {
                            return if subject == value {
                                ArmShape::Default
                            } else {
                                ArmShape::Opaque
                            };
                        }
                        return ArmShape::Finite(vec![FiniteConstraint::Bool {
                            subject: binary.right,
                            value,
                        }]);
                    }
                }
                if matches!(
                    binary.operator,
                    BinaryOperator::Equal | BinaryOperator::NotEqual
                ) {
                    return ArmShape::Compare {
                        left: binary.left,
                        right: binary.right,
                        negated: binary.operator == BinaryOperator::NotEqual,
                    };
                }
            }
            return ArmShape::Opaque;
        }

        let mut constraints = Vec::new();
        let mut first_case_claim = None;
        let mut countable = true;
        for conjunct in conjuncts {
            if let Some(claim) = classify_membership_union(scope, conjunct) {
                if first_case_claim.is_none() {
                    first_case_claim = Some(CaseClaim {
                        data_index: claim.data_index,
                        subject: claim.subject,
                        covered: None,
                    });
                }
                if claim.covered.is_none() {
                    countable = false;
                }
                constraints.push(FiniteConstraint::Case(claim));
                continue;
            }
            match program.tables.bodies.expressions.expression(conjunct) {
                ExpressionNode::Boolean(true) => continue,
                ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Equal => {
                    if let ExpressionNode::Boolean(value) =
                        program.tables.bodies.expressions.expression(binary.right)
                    {
                        constraints.push(FiniteConstraint::Bool {
                            subject: binary.left,
                            value: *value,
                        });
                        continue;
                    }
                    if let ExpressionNode::Boolean(value) =
                        program.tables.bodies.expressions.expression(binary.left)
                    {
                        constraints.push(FiniteConstraint::Bool {
                            subject: binary.right,
                            value: *value,
                        });
                        continue;
                    }
                    countable = false;
                }
                _ => countable = false,
            }
        }
        if countable && !constraints.is_empty() {
            ArmShape::Finite(constraints)
        } else if let Some(claim) = first_case_claim {
            ArmShape::UncountedCase(claim)
        } else {
            ArmShape::Opaque
        }
    }

    fn classify_membership_union(
        scope: &DispatchScope<'_>,
        expression: ExpressionHandle,
    ) -> Option<CaseClaim> {
        let program = scope.program;
        let mut leaves = Vec::new();
        flatten_binary(program, expression, BinaryOperator::Or, &mut leaves);

        let expressions = &program.tables.bodies.expressions;
        let mut subject = ExpressionHandle::invalid();
        let mut data_index: Option<usize> = None;
        let mut variants: Vec<usize> = Vec::new();
        let mut countable = true;

        for leaf in leaves {
            let ExpressionNode::Membership(membership) = expressions.expression(leaf) else {
                return None;
            };

            if subject.is_valid() {
                if !expressions_structurally_equal(program, subject, membership.value) {
                    countable = false;
                }
            } else {
                subject = membership.value;
            }

            match classify_membership_leaf(scope, membership) {
                LeafContribution::Tags {
                    data_index: leaf_data,
                    variants: leaf_variants,
                } => match data_index {
                    None => {
                        data_index = Some(leaf_data);
                        variants.extend(leaf_variants);
                    }
                    Some(existing) if existing == leaf_data => variants.extend(leaf_variants),
                    Some(_) => countable = false,
                },
                LeafContribution::Predicate {
                    data_index: leaf_data,
                } => {
                    data_index.get_or_insert(leaf_data);
                    countable = false;
                }
                LeafContribution::NotCase => countable = false,
            }
        }

        let data_index = data_index?;
        Some(CaseClaim {
            data_index,
            subject,
            covered: countable.then_some(variants),
        })
    }

    fn classify_membership_leaf(
        scope: &DispatchScope<'_>,
        membership: &resolved::expression::TableMembershipExpression,
    ) -> LeafContribution {
        let program = scope.program;
        if membership.domain_symbol.is_valid() {
            return classify_declared_domain(scope, membership.domain_symbol);
        }

        let subject_data = subject_data_index(scope, membership.value);
        match find_case(program, subject_data, membership) {
            Some((data_index, variant)) => LeafContribution::Tags {
                data_index,
                variants: vec![variant],
            },
            None => LeafContribution::NotCase,
        }
    }

    fn classify_declared_domain(
        scope: &DispatchScope<'_>,
        domain_symbol: SymbolHandle,
    ) -> LeafContribution {
        let program = scope.program;
        let Some(domain) = program.domain_definition_by_symbol(domain_symbol) else {
            return LeafContribution::NotCase;
        };

        let resolved::types::TypeReference::Named {
            symbol: target,
            name,
            ..
        } = &domain.target_type
        else {
            return LeafContribution::NotCase;
        };
        let data_index = if target.is_valid() {
            program
                .data_definitions
                .iter()
                .position(|definition| definition.symbol == *target)
        } else {
            data_definition_index(program, name)
        };
        let Some(data_index) = data_index else {
            return LeafContribution::NotCase;
        };
        if case_count(program, &program.data_definitions[data_index]) == 0 {
            return LeafContribution::NotCase;
        }

        let [resolved::domain::ProofFact::Expression(case_union)] =
            program.proof_facts(domain.facts)
        else {
            return LeafContribution::Predicate { data_index };
        };

        let mut leaves = Vec::new();
        flatten_binary(program, *case_union, BinaryOperator::Or, &mut leaves);

        let expressions = &program.tables.bodies.expressions;
        let mut variants = Vec::new();
        for leaf in leaves {
            let ExpressionNode::Membership(leaf_membership) = expressions.expression(leaf) else {
                return LeafContribution::Predicate { data_index };
            };
            if !is_domain_subject(program, domain, leaf_membership.value) {
                return LeafContribution::Predicate { data_index };
            }
            if leaf_membership.domain_symbol.is_valid() {
                return LeafContribution::Predicate { data_index };
            }
            match find_case(program, Some(data_index), leaf_membership) {
                Some((leaf_data, variant)) if leaf_data == data_index => variants.push(variant),
                _ => return LeafContribution::Predicate { data_index },
            }
        }

        LeafContribution::Tags {
            data_index,
            variants,
        }
    }

    fn is_domain_subject(
        program: &SymbolResolvedTrees,
        domain: &resolved::domain::DomainDefinition,
        expression: ExpressionHandle,
    ) -> bool {
        matches!(
            program.tables.bodies.expressions.expression(expression),
            ExpressionNode::Name(path)
                if domain.subject_symbol.is_valid()
                    && path.symbol == domain.subject_symbol
                    && path.head_symbol == domain.subject_symbol
                    && path.members.count() == 1
                    && program.tables.bodies.expressions.name_path_member_symbols(path.member_symbols)
                        == [domain.subject_symbol]
        )
    }

    fn find_case(
        program: &SymbolResolvedTrees,
        subject_data: Option<usize>,
        membership: &resolved::expression::TableMembershipExpression,
    ) -> Option<(usize, usize)> {
        let subject_index = |data_index| match subject_data {
            Some(subject_index)
                if generic_base_symbol(&program.data_definitions[subject_index])
                    == Some(program.data_definitions[data_index].symbol) =>
            {
                subject_index
            }
            _ => data_index,
        };
        if membership.case_type_symbol.is_valid() {
            let data_index = program
                .data_definitions
                .iter()
                .position(|definition| definition.symbol == membership.case_type_symbol)?;
            let variant =
                variant_index(program, &program.data_definitions[data_index], |variant| {
                    variant.symbol == membership.case_symbol
                })?;
            return Some((subject_index(data_index), variant));
        }
        let members = resolved_expressions(program).name_path_members(membership.domain);
        let [type_name, case_name] = members else {
            return None;
        };
        let data_index = data_definition_index(program, type_name)?;
        let variant = variant_index(program, &program.data_definitions[data_index], |variant| {
            variant.name.as_str() == case_name.as_str()
        })?;
        Some((subject_index(data_index), variant))
    }

    pub(crate) fn data_definition_index(
        program: &SymbolResolvedTrees,
        type_name: &DiagnosticName,
    ) -> Option<usize> {
        if type_name.is_source_backed() {
            let symbol = program
                .symbols
                .find_top_level_by_name_and_kinds_from_source(
                    type_name.as_str(),
                    &[symbols::SymbolKind::Data],
                    type_name.source_span(),
                )?;
            return program
                .data_definitions
                .iter()
                .position(|definition| definition.symbol == symbol);
        }
        program
            .data_definitions
            .iter()
            .position(|definition| definition.name.as_str() == type_name.as_str())
    }

    fn variant_index(
        program: &SymbolResolvedTrees,
        definition: &DataDefinition,
        matches: impl Fn(&DataVariant) -> bool,
    ) -> Option<usize> {
        program
            .data_members(definition.members)
            .iter()
            .filter_map(|member| match member {
                DataMember::Variant(variant) => Some(variant),
                DataMember::Field(_) => None,
            })
            .position(matches)
    }

    fn case_count(program: &SymbolResolvedTrees, definition: &DataDefinition) -> usize {
        program
            .data_members(definition.members)
            .iter()
            .filter(|member| matches!(member, DataMember::Variant(_)))
            .count()
    }

    fn uncovered_case_names(
        program: &SymbolResolvedTrees,
        definition: &DataDefinition,
        covered: &[bool],
    ) -> Vec<String> {
        program
            .data_members(definition.members)
            .iter()
            .filter_map(|member| match member {
                DataMember::Variant(variant) => Some(variant.name.as_str().to_owned()),
                DataMember::Field(_) => None,
            })
            .zip(covered.iter())
            .filter_map(|(name, covered)| (!covered).then_some(name))
            .collect()
    }

    fn flatten_binary(
        program: &SymbolResolvedTrees,
        expression: ExpressionHandle,
        operator: BinaryOperator,
        leaves: &mut Vec<ExpressionHandle>,
    ) {
        if let ExpressionNode::Binary(binary) =
            program.tables.bodies.expressions.expression(expression)
            && binary.operator == operator
        {
            let (left, right) = (binary.left, binary.right);
            flatten_binary(program, left, operator, leaves);
            flatten_binary(program, right, operator, leaves);
            return;
        }
        leaves.push(expression);
    }

    fn expressions_structurally_equal(
        program: &SymbolResolvedTrees,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> bool {
        if left == right {
            return true;
        }
        if !left.is_valid() || !right.is_valid() {
            return false;
        }

        let expressions = &program.tables.bodies.expressions;
        match (expressions.expression(left), expressions.expression(right)) {
            (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) => left == right,
            (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => left == right,
            (ExpressionNode::Float(left), ExpressionNode::Float(right)) => left == right,
            (ExpressionNode::String(left), ExpressionNode::String(right)) => left == right,
            (ExpressionNode::ArrayLiteral(left), ExpressionNode::ArrayLiteral(right)) => {
                let left = expressions.expression_handles(*left);
                let right = expressions.expression_handles(*right);
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right)
                        .all(|(left, right)| expressions_structurally_equal(program, *left, *right))
            }
            (ExpressionNode::StructLiteral(left), ExpressionNode::StructLiteral(right)) => {
                let left_fields = expressions.struct_fields(left.fields);
                let right_fields = expressions.struct_fields(right.fields);
                left.type_symbol == right.type_symbol
                    && left.case_symbol == right.case_symbol
                    && left_fields.len() == right_fields.len()
                    && left_fields.iter().zip(right_fields).all(|(left, right)| {
                        left.field_symbol == right.field_symbol
                            && left.name == right.name
                            && expressions_structurally_equal(program, left.value, right.value)
                    })
            }
            (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
                left.symbol == right.symbol
                    && name_paths_equal(
                        expressions.name_path_members(left.members),
                        expressions.name_path_members(right.members),
                    )
            }
            (ExpressionNode::Member(left), ExpressionNode::Member(right)) => {
                left.member.as_str() == right.member.as_str()
                    && expressions_structurally_equal(program, left.receiver, right.receiver)
            }
            (ExpressionNode::Indexed(left), ExpressionNode::Indexed(right)) => {
                expressions_structurally_equal(program, left.collection, right.collection)
                    && expressions_structurally_equal(program, left.index, right.index)
            }
            (ExpressionNode::Unary(left), ExpressionNode::Unary(right)) => {
                left.operator == right.operator
                    && expressions_structurally_equal(program, left.operand, right.operand)
            }
            (ExpressionNode::Binary(left), ExpressionNode::Binary(right)) => {
                left.operator == right.operator
                    && expressions_structurally_equal(program, left.left, right.left)
                    && expressions_structurally_equal(program, left.right, right.right)
            }
            (ExpressionNode::Membership(left), ExpressionNode::Membership(right)) => {
                left.domain_symbol == right.domain_symbol
                    && name_paths_equal(
                        expressions.name_path_members(left.domain),
                        expressions.name_path_members(right.domain),
                    )
                    && expressions_structurally_equal(program, left.value, right.value)
            }
            (ExpressionNode::Borrow(left), ExpressionNode::Borrow(right)) => {
                left.access == right.access
                    && expressions_structurally_equal(program, left.target, right.target)
            }
            (ExpressionNode::Cast(left), ExpressionNode::Cast(right)) => {
                expressions_structurally_equal(program, left.value, right.value)
            }
            (ExpressionNode::Call(left), ExpressionNode::Call(right)) => {
                left.target.as_str() == right.target.as_str()
                    && expressions_structurally_equal(program, left.receiver, right.receiver)
                    && {
                        let left_arguments = expressions.expression_handles(left.arguments);
                        let right_arguments = expressions.expression_handles(right.arguments);
                        left_arguments.len() == right_arguments.len()
                            && left_arguments.iter().zip(right_arguments.iter()).all(
                                |(left, right)| {
                                    expressions_structurally_equal(program, *left, *right)
                                },
                            )
                    }
            }
            _ => false,
        }
    }

    fn name_paths_equal(left: &[DiagnosticName], right: &[DiagnosticName]) -> bool {
        left.len() == right.len()
            && left
                .iter()
                .zip(right.iter())
                .all(|(left, right)| left.as_str() == right.as_str())
    }

    fn subject_data_index(scope: &DispatchScope<'_>, subject: ExpressionHandle) -> Option<usize> {
        let type_reference = subject_declared_type(scope, subject);
        type_reference
            .as_ref()
            .and_then(|t| data_index_for_type(scope.program, t))
    }

    fn subject_declared_type(
        scope: &DispatchScope<'_>,
        subject: ExpressionHandle,
    ) -> Option<TypeReference> {
        let expressions = &scope.program.tables.bodies.expressions;

        match expressions.expression(subject) {
            ExpressionNode::Member(member) => field_declared_type(scope, member.member_symbol)
                .or_else(|| {
                    let receiver = subject_declared_type(scope, member.receiver)?;
                    let definition = &scope.program.data_definitions
                        [data_index_for_type(scope.program, &receiver)?];
                    scope
                        .program
                        .data_members(definition.members)
                        .iter()
                        .find_map(|member_| match member_ {
                            DataMember::Field(field)
                                if field.name.as_str() == member.member.as_str() =>
                            {
                                Some(field.type_reference.clone())
                            }
                            _ => None,
                        })
                }),
            ExpressionNode::Name(path) => name_declared_type(scope, path),
            ExpressionNode::Call(call) if call.target_symbol.is_valid() => {
                declared_call_return(scope.program, call.target_symbol).cloned()
            }
            ExpressionNode::SavedSubject(saved) => {
                subject_declared_type(scope, expressions.match_subject(saved.capture).subject)
            }
            _ => None,
        }
    }

    fn name_declared_type(
        scope: &DispatchScope<'_>,
        path: &TableNamePath,
    ) -> Option<TypeReference> {
        for symbol in [path.symbol, path.head_symbol] {
            if !symbol.is_valid() {
                continue;
            }
            if let Some(parameter) = scope
                .program
                .state_parameters(scope.state.parameters)
                .iter()
                .find(|parameter| parameter.symbol == symbol)
            {
                return Some(parameter.type_reference.clone());
            }
            if let Some(local) = scope
                .program
                .state_statements(scope.state.statements)
                .iter()
                .find_map(|statement| {
                    let resolved::statement::Statement::LocalData(local) = statement else {
                        return None;
                    };
                    (local.symbol == symbol).then_some(local)
                })
            {
                if local.is_operand_capture || matches!(local.type_reference, TypeReference::Unit) {
                    return subject_declared_type(scope, local.initial_value)
                        .or_else(|| Some(local.type_reference.clone()));
                }
                return Some(local.type_reference.clone());
            }
            if let Some(declaration) = scope.program.const_declaration(symbol) {
                return Some(declaration.declared_type.clone());
            }
            if let Some(type_reference) = field_declared_type(scope, symbol) {
                return Some(type_reference);
            }
        }
        None
    }

    fn field_declared_type(
        scope: &DispatchScope<'_>,
        symbol: SymbolHandle,
    ) -> Option<TypeReference> {
        if !symbol.is_valid() {
            return None;
        }
        for owned in scope.program.machine_owned_data(scope.machine.owned_data) {
            if owned.symbol == symbol {
                return Some(owned.type_reference.clone());
            }
        }
        for definition in &scope.program.data_definitions {
            for member in scope.program.data_members(definition.members) {
                match member {
                    DataMember::Field(field) if field.symbol == symbol => {
                        return Some(field.type_reference.clone());
                    }
                    DataMember::Variant(variant) => {
                        for field in scope.program.data_payload_fields(variant.payload) {
                            if field.symbol == symbol {
                                return Some(field.type_reference.clone());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        None
    }

    fn data_index_for_type(
        program: &SymbolResolvedTrees,
        type_reference: &TypeReference,
    ) -> Option<usize> {
        match type_reference {
            TypeReference::Named { symbol, .. } => program
                .data_definitions
                .iter()
                .position(|definition| definition.symbol == *symbol),
            TypeReference::Generic(generic) => {
                program.data_definitions.iter().position(|definition| {
                    matches!(
                        &definition.generic_instance,
                        Some(TypeReference::Generic(instance))
                            if instance.base_symbol == generic.base_symbol
                                && type_arguments_equal(
                                    program,
                                    instance.arguments,
                                    generic.arguments,
                                )
                    )
                })
            }
            TypeReference::Reference(reference) => {
                data_index_for_type(program, program.child_type_reference(reference.referee))
            }
            TypeReference::Constrained(constrained) => {
                data_index_for_type(program, program.child_type_reference(constrained.base_type))
            }
            _ => None,
        }
    }

    fn generic_base_symbol(definition: &DataDefinition) -> Option<SymbolHandle> {
        match &definition.generic_instance {
            Some(TypeReference::Generic(application)) => Some(application.base_symbol),
            _ => None,
        }
    }

    fn type_arguments_equal(
        program: &SymbolResolvedTrees,
        left: HandleSpan<TypeReference>,
        right: HandleSpan<TypeReference>,
    ) -> bool {
        let left = program.child_type_references(left);
        let right = program.child_type_references(right);
        left.len() == right.len()
            && left
                .iter()
                .zip(right.iter())
                .all(|(left, right)| type_references_equal(program, left, right))
    }

    fn type_references_equal(
        program: &SymbolResolvedTrees,
        left: &TypeReference,
        right: &TypeReference,
    ) -> bool {
        match (left, right) {
            (
                TypeReference::Named { symbol: left, .. },
                TypeReference::Named { symbol: right, .. },
            ) => left == right,
            (TypeReference::Generic(left), TypeReference::Generic(right)) => {
                left.base_symbol == right.base_symbol
                    && type_arguments_equal(program, left.arguments, right.arguments)
            }
            (TypeReference::Reference(left), TypeReference::Reference(right)) => {
                type_references_equal(
                    program,
                    program.child_type_reference(left.referee),
                    program.child_type_reference(right.referee),
                )
            }
            (TypeReference::Constrained(left), TypeReference::Constrained(right)) => {
                type_references_equal(
                    program,
                    program.child_type_reference(left.base_type),
                    program.child_type_reference(right.base_type),
                )
            }
            _ => left == right,
        }
    }
}

pub(crate) mod proposition {
    use crate::expressions::expression::lower_expression_handle_from_table_in_fact_position;
    use crate::lowerer::Lowerer;
    use crate::lowerer::name::lower_name;
    use crate::tables::{expression_table_mut, resolved_expressions};
    use diagnostics::Diagnostic;
    use resolved::types::TypeReference;
    use symbol_resolved_trees as resolved;
    use typed::proposition::PropositionBinderArgumentKind as ArgumentKind;
    use typed_trees as typed;

    pub(crate) fn is_proposition_call_target(
        symbols: &symbols::SymbolTable,
        target_symbol: symbols::SymbolHandle,
    ) -> bool {
        target_symbol.is_valid()
            && matches!(
                symbols.get(target_symbol).kind,
                symbols::SymbolKind::Proposition | symbols::SymbolKind::PropositionParameter
            )
    }

    pub(crate) fn lower_proposition_application(
        lowerer: &mut Lowerer,
        call: &resolved::expression::TableCallExpression,
    ) -> Result<typed::proposition::PropositionApplication, Diagnostic> {
        let declaration = lowerer
            .source_trees
            .propositions
            .iter()
            .find(|proposition| proposition.symbol == call.target_symbol);
        let proposition_parameter = lowerer
            .source_trees
            .tables
            .declarations
            .data_type_parameters
            .iter()
            .map(|(_, parameter)| parameter)
            .find(|parameter| {
                parameter.symbol == call.target_symbol
                    && matches!(
                        parameter.kind,
                        resolved::data::TypeParameterKind::Proposition { .. }
                    )
            });
        if declaration.is_none() && proposition_parameter.is_none() {
            return Err(Diagnostic::error(format!(
                "proposition application `{}` has no resolved declaration or generic proposition parameter",
                call.target.as_str()
            )));
        }
        let binders = declaration
            .map(|declaration| {
                lowerer
                    .source_trees
                    .tables
                    .declarations
                    .proposition_binders
                    .span_or_empty(declaration.binders)
            })
            .unwrap_or(&[]);
        if binders.len() != call.machine_arguments.len() {
            return Err(Diagnostic::error(format!(
                "proposition `{}` expects {} proof-static binder argument(s), received {}",
                call.target.as_str(),
                binders.len(),
                call.machine_arguments.len()
            )));
        }
        let mut typed_binder_arguments = Vec::with_capacity(binders.len());
        for (binder, argument) in binders.iter().zip(&call.machine_arguments) {
            let (kind, symbol) = match &binder.kind {
                resolved::proposition::PropositionBinderKind::Type => {
                    if argument.const_literal.is_some() {
                        return Err(Diagnostic::error(format!(
                            "proposition `{}` type binder `{}` received a const literal",
                            call.target.as_str(),
                            binder.name.as_str()
                        )));
                    }
                    let symbol = resolve_proposition_static_path(lowerer, argument);
                    if classify_proposition_static_symbol(lowerer, symbol)
                        != Some(ArgumentKind::Type)
                    {
                        return Err(Diagnostic::error(format!(
                            "proposition `{}` type binder `{}` received a non-type argument",
                            call.target.as_str(),
                            binder.name.as_str()
                        )));
                    }
                    (ArgumentKind::Type, symbol)
                }
                resolved::proposition::PropositionBinderKind::Const { type_reference } => {
                    let symbol = if argument.const_literal.is_some() {
                        symbols::SymbolHandle::invalid()
                    } else {
                        resolve_proposition_static_path(lowerer, argument)
                    };
                    if let Some(literal) = &argument.const_literal {
                        validate_const_literal_argument(
                            call.target.as_str(),
                            binder.name.as_str(),
                            type_reference,
                            literal,
                        )?;
                    } else if classify_proposition_static_symbol(lowerer, symbol)
                        != Some(ArgumentKind::Const)
                    {
                        return Err(Diagnostic::error(format!(
                            "proposition `{}` const binder `{}` received a non-const argument",
                            call.target.as_str(),
                            binder.name.as_str()
                        )));
                    } else if !proposition_const_symbol_type(lowerer, symbol)
                        .is_some_and(|actual| proposition_const_types_match(type_reference, actual))
                    {
                        return Err(Diagnostic::error(format!(
                            "proposition `{}` const binder `{}` received a const argument with a different declared type",
                            call.target.as_str(),
                            binder.name.as_str()
                        )));
                    }
                    (ArgumentKind::Const, symbol)
                }
                resolved::proposition::PropositionBinderKind::Machine => {
                    if argument.evidence_projection.is_some() {
                        (ArgumentKind::Machine, symbols::SymbolHandle::invalid())
                    } else {
                        let symbol = resolve_proposition_static_path(lowerer, argument);
                        if argument.const_literal.is_some()
                            || classify_proposition_static_symbol(lowerer, symbol)
                                != Some(ArgumentKind::Machine)
                        {
                            return Err(Diagnostic::error(format!(
                                "proposition `{}` machine binder `{}` received a non-machine argument",
                                call.target.as_str(),
                                binder.name.as_str()
                            )));
                        }
                        (ArgumentKind::Machine, symbol)
                    }
                }
            };
            if symbol.is_valid()
                && lowerer
                    .typed_trees
                    .type_reference_table
                    .find_named_type_reference(symbol)
                    .is_none()
            {
                let name = argument.path.last().ok_or_else(|| {
                    Diagnostic::error("resolved proposition argument has no source path")
                })?;
                lowerer.typed_trees.type_reference_table.insert(
                    typed::types::TypeReferenceNode::Named {
                        symbol,
                        name: lower_name(name),
                    },
                );
            }
            if let Some(literal) = &argument.const_literal {
                let expression = expression_table_mut(&mut lowerer.typed_trees)
                    .insert(typed::expression::ExpressionNode::Integer(literal.clone()));
                lowerer
                    .typed_trees
                    .type_reference_table
                    .insert(typed::types::TypeReferenceNode::ConstExpression(expression));
            }
            typed_binder_arguments.push(typed::proposition::PropositionBinderArgument {
                kind,
                path: argument
                    .path
                    .iter()
                    .map(lower_name)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                const_literal: argument.const_literal.clone(),
                evidence_projection: argument.evidence_projection.as_ref().map(|projection| {
                    typed::expression::EvidenceProjection {
                        term: lower_name(&projection.term),
                        member: lower_name(&projection.member),
                    }
                }),
                symbol,
            });
        }
        let parameters = if let Some(declaration) = declaration {
            lowerer
                .source_trees
                .state_parameters(declaration.parameters)
        } else if let Some(resolved::data::TypeParameterKind::Proposition { contract }) =
            proposition_parameter.map(|parameter| &parameter.kind)
        {
            lowerer.source_trees.state_parameters(contract.parameters)
        } else {
            &[]
        };
        if parameters.len() != call.arguments.len() {
            return Err(Diagnostic::error(format!(
                "proposition `{}` expects {} value argument(s), received {}",
                call.target.as_str(),
                parameters.len(),
                call.arguments.len()
            )));
        }

        let arguments = resolved_expressions(lowerer.source_trees)
            .expression_handles(call.arguments)
            .iter()
            .map(|argument| {
                lower_expression_handle_from_table_in_fact_position(
                    lowerer.source_trees,
                    &lowerer.source_trees.tables.bodies.expressions,
                    &mut lowerer.typed_trees,
                    *argument,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let arguments =
            expression_table_mut(&mut lowerer.typed_trees).insert_expression_handles(arguments);

        Ok(typed::proposition::PropositionApplication {
            proposition: call.target_symbol,
            name: lower_name(&call.target),
            binder_arguments: typed_binder_arguments.into_boxed_slice(),
            arguments,
        })
    }

    fn resolve_proposition_static_path(
        lowerer: &Lowerer,
        argument: &resolved::expression::StaticMachineArgument,
    ) -> symbols::SymbolHandle {
        if argument.symbol.is_valid() {
            return argument.symbol;
        }
        lowerer
            .source_trees
            .symbols
            .find_descendant_by_path(
                lowerer.source_trees.symbols.root(),
                argument.path.iter().map(|member| member.as_str()),
            )
            .unwrap_or_else(symbols::SymbolHandle::invalid)
    }

    fn classify_proposition_static_symbol(
        lowerer: &Lowerer,
        symbol: symbols::SymbolHandle,
    ) -> Option<ArgumentKind> {
        if !symbol.is_valid() {
            return None;
        }
        match lowerer.source_trees.symbols.get(symbol).kind {
            symbols::SymbolKind::BuiltinType | symbols::SymbolKind::Data => {
                Some(ArgumentKind::Type)
            }
            symbols::SymbolKind::State
            | symbols::SymbolKind::MachineParameter
            | symbols::SymbolKind::PropositionMachineParameter => Some(ArgumentKind::Machine),
            symbols::SymbolKind::TypeParameter => lowerer
                .source_trees
                .tables
                .declarations
                .proposition_binders
                .iter()
                .map(|(_, binder)| binder)
                .find(|binder| binder.symbol == symbol)
                .map(|binder| match binder.kind {
                    resolved::proposition::PropositionBinderKind::Type => ArgumentKind::Type,
                    resolved::proposition::PropositionBinderKind::Const { .. } => {
                        ArgumentKind::Const
                    }
                    resolved::proposition::PropositionBinderKind::Machine => ArgumentKind::Machine,
                })
                .or_else(|| {
                    find_data_type_parameter(lowerer, symbol).and_then(|parameter| match parameter
                        .kind
                    {
                        resolved::data::TypeParameterKind::Type => Some(ArgumentKind::Type),
                        resolved::data::TypeParameterKind::Const { .. }
                        | resolved::data::TypeParameterKind::Value { .. } => {
                            Some(ArgumentKind::Const)
                        }
                        resolved::data::TypeParameterKind::Machine { .. } => {
                            Some(ArgumentKind::Machine)
                        }
                        resolved::data::TypeParameterKind::Implementation
                        | resolved::data::TypeParameterKind::Proposition { .. } => None,
                    })
                }),
            _ => None,
        }
    }

    fn validate_const_literal_argument(
        proposition: &str,
        binder: &str,
        type_reference: &TypeReference,
        literal: &numerics::literals::IntegerLiteral,
    ) -> Result<(), Diagnostic> {
        let Some(primitive) = type_reference.primitive_type() else {
            return Err(Diagnostic::error(format!(
                "proposition `{proposition}` const binder `{binder}` has a non-primitive type and cannot receive integer literal `{}`",
                literal
            )));
        };
        if !primitive.accepts_integer_literal() || !integer_literal_fits(literal, primitive) {
            return Err(Diagnostic::error(format!(
                "proposition `{proposition}` const binder `{binder}` cannot receive integer literal `{}` as `{}`",
                literal,
                primitive.name()
            )));
        }
        Ok(())
    }

    fn proposition_const_symbol_type<'a>(
        lowerer: &'a Lowerer,
        symbol: symbols::SymbolHandle,
    ) -> Option<&'a TypeReference> {
        lowerer
            .source_trees
            .tables
            .declarations
            .proposition_binders
            .iter()
            .map(|(_, binder)| binder)
            .find(|binder| binder.symbol == symbol)
            .and_then(|binder| match &binder.kind {
                resolved::proposition::PropositionBinderKind::Const { type_reference } => {
                    Some(type_reference)
                }
                resolved::proposition::PropositionBinderKind::Type
                | resolved::proposition::PropositionBinderKind::Machine => None,
            })
            .or_else(|| {
                find_data_type_parameter(lowerer, symbol).and_then(|parameter| {
                    match &parameter.kind {
                        resolved::data::TypeParameterKind::Const { type_reference }
                        | resolved::data::TypeParameterKind::Value { type_reference } => {
                            Some(type_reference)
                        }
                        resolved::data::TypeParameterKind::Type
                        | resolved::data::TypeParameterKind::Implementation
                        | resolved::data::TypeParameterKind::Machine { .. }
                        | resolved::data::TypeParameterKind::Proposition { .. } => None,
                    }
                })
            })
    }

    fn find_data_type_parameter<'l>(
        lowerer: &'l Lowerer,
        symbol: symbols::SymbolHandle,
    ) -> Option<&'l resolved::data::TypeParameter> {
        lowerer
            .source_trees
            .tables
            .declarations
            .data_type_parameters
            .iter()
            .map(|(_, parameter)| parameter)
            .find(|parameter| parameter.symbol == symbol)
    }

    fn proposition_const_types_match(expected: &TypeReference, actual: &TypeReference) -> bool {
        match (expected.primitive_type(), actual.primitive_type()) {
            (Some(expected), Some(actual)) => expected == actual,
            _ => match (expected, actual) {
                (
                    TypeReference::Named {
                        symbol: expected, ..
                    },
                    TypeReference::Named { symbol: actual, .. },
                ) => expected == actual,
                _ => expected == actual,
            },
        }
    }

    fn integer_literal_fits(
        literal: &numerics::literals::IntegerLiteral,
        primitive: resolved::types::PrimitiveType,
    ) -> bool {
        use resolved::types::PrimitiveType;
        let signed_width = match primitive {
            PrimitiveType::I8 => Some(8),
            PrimitiveType::I16 => Some(16),
            PrimitiveType::I32 => Some(32),
            PrimitiveType::I64 => Some(64),
            _ => None,
        };
        if let Some(width) = signed_width {
            return literal.value_i64().is_some_and(|value| {
                width == 64 || (-(1i64 << (width - 1))..=(1i64 << (width - 1)) - 1).contains(&value)
            });
        }
        let unsigned_width = match primitive {
            PrimitiveType::U8 => Some(8),
            PrimitiveType::U16 => Some(16),
            PrimitiveType::U32 => Some(32),
            PrimitiveType::U64 | PrimitiveType::Addr => Some(64),
            _ => None,
        };
        unsigned_width.is_some_and(|width| {
            literal
                .value_u64()
                .is_some_and(|value| width == 64 || value < (1u64 << width))
        })
    }
}
