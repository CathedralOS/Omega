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

        if scalar_field_type(program, &field.type_reference).is_some() {
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
            resolved::types::TypeReference::Named { symbol, .. }
                if program.symbols.primitive_type(*symbol) == Some(PrimitiveType::U8)
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
            resolved::types::TypeReference::Named { symbol, .. }
                if program.symbols.primitive_type(*symbol) == Some(PrimitiveType::U8)
        )
    }

    fn scalar_field_type<'a>(
        program: &'a SymbolResolvedTrees,
        mut reference: &'a resolved::types::TypeReference,
    ) -> Option<symbols::PrimitiveType> {
        loop {
            reference = match reference {
                resolved::types::TypeReference::Reference(node) => {
                    program.child_type_reference(node.storage.referee)
                }
                resolved::types::TypeReference::Constrained(node) => {
                    program.child_type_reference(node.storage.base_type)
                }
                resolved::types::TypeReference::AliasUse { expansion, .. } => expansion,
                _ => return reference.primitive_type(&program.symbols),
            };
        }
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

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::lowerer::TypingCohort;
        use resolved::expression::TableBinaryExpression;
        use resolved::statement::{TableTransition, TransitionTargetNode};

        fn resolve(text: &str) -> SymbolResolvedTrees {
            let mut sources = source::SourceMap::default();
            let source = sources
                .add("coverage/main.omg".into(), text.to_owned())
                .source_id;
            let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
            let syntax =
                tokens_to_syntax_trees::parse_syntax_trees_with_id(source, &tokens).unwrap();
            let mut request = syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax);
            request.sources = Some(std::sync::Arc::new(sources));
            syntax_trees_to_symbol_resolved_trees::resolve(request).unwrap()
        }

        #[test]
        fn separately_evaluated_calls_do_not_prove_dispatch_coverage() {
            for text in [
                "machine flag<const B: bool>() -> bool { B } machine uncovered() { transition { flag<false>() == true -> {} flag<true>() == false -> {} } }",
                "machine flag() -> bool { false } machine uncovered() { transition { flag() == true -> {} flag() == false -> {} } }",
                "machine number() -> u32 { 0u32 } machine uncovered() { transition { number() == 0u32 -> {} number() != 0u32 -> {} } }",
            ] {
                let program = resolve(text);
                let error =
                    validate_case_dispatch_exhaustiveness_in(&program, &TypingCohort::default())
                        .expect_err("separate invocations do not name one unchanged subject");
                assert!(error.message.contains("can fall through"), "{error:?}");
            }
        }

        #[test]
        fn capturing_a_call_result_or_adding_a_wildcard_closes_coverage() {
            for text in [
                "machine flag() -> bool { false } machine covered() { let value: bool = flag(); transition { value == true -> {} value == false -> {} } }",
                "machine flag() -> bool { false } machine covered() { transition flag() { true -> {} false -> {} } }",
                "machine flag() -> bool { false } machine covered() { transition { flag() == true -> {} _ -> {} } }",
                "machine covered(value: u32) { transition { value == 0u32 -> {} value != 0u32 -> {} } }",
            ] {
                let program = resolve(text);
                crate::lower_symbol_resolved_trees(&program)
                    .expect("one captured value or wildcard has exhaustive coverage");
            }
        }

        #[test]
        fn coverage_distinguishes_conversion_targets_and_builtin_policies() {
            for text in [
                "machine uncovered(value: u16) { transition { 0 == (value as u8) -> {} 0 != (value as u16) -> {} } }",
                "machine uncovered(value: u8) { transition { 255 == (value +% 1u8) -> {} 255 != (value +| 1u8) -> {} } }",
            ] {
                let program = resolve(text);
                validate_case_dispatch_exhaustiveness_in(&program, &TypingCohort::default())
                    .expect_err("different conversions or unselected computations need a capture");
            }
            for text in [
                "machine covered(value: u8) { transition { 0 == (value as u16) -> {} 0 != (value as u16) -> {} } }",
                "machine covered(value: u8) { transition { 0 == (value +% 1u8) -> {} 0 != (value +% 1u8) -> {} } }",
                "machine covered(value: u8) { transition { 0 == (value * 0u8) -> {} 0 != (value * 0u8) -> {} } }",
            ] {
                let program = resolve(text);
                crate::lower_symbol_resolved_trees(&program)
                    .expect("the same builtin computation covers its complement");
            }
        }

        #[test]
        fn coverage_uses_copied_values_and_versions_memory_reads_across_calls() {
            let prefix = "machine flip(value: &mut bool) -> bool { value = false; false } ";
            let program = resolve(&format!(
                "{prefix} machine uncovered(value: &mut bool) {{ transition {{ value == true -> {{}} flip(value) == true -> {{}} value == false -> {{}} }} }}"
            ));
            validate_case_dispatch_exhaustiveness_in(&program, &TypingCohort::default())
                .expect_err("the call can change the pointee between alternatives");
            for body in [
                "machine covered(value: &mut bool, captured: bool) { transition { captured == true -> {} flip(value) == true -> {} captured == false -> {} } }",
                "machine covered(value: &mut bool) { transition { value == true -> {} value == false -> {} flip(value) == true -> {} } }",
                "machine covered(value: &mut bool) { transition { flip(value) == true -> {} value == true -> {} value == false -> {} } }",
            ] {
                let program = resolve(&format!("{prefix}{body}"));
                crate::lower_symbol_resolved_trees(&program).expect(
                    "snapshots, complete prefixes and fresh complete suffixes cover arrivals",
                );
            }
        }

        #[test]
        fn unchanged_fields_and_captured_fields_keep_boolean_coverage() {
            for body in [
                "machine covered(flags: &Flags) { transition { flags.flag == true -> {} flags.flag == false -> {} } }",
                "machine covered(flags: &Flags) { transition flags.flag { true -> {} false -> {} } }",
                "machine covered(flags: &Flags) { let captured: bool = flags.flag; transition { captured == true -> {} captured == false -> {} } }",
            ] {
                let program = resolve(&format!("data Flags {{ flag: bool; }} {body}"));
                crate::lower_symbol_resolved_trees(&program)
                    .unwrap_or_else(|error| panic!("{body}: {error:?}"));
            }
        }

        #[test]
        fn borrowed_case_captures_keep_their_storage_version() {
            let mut program = resolve(
                "data Mode { case First; case Second; } machine change(value: &mut Mode) -> bool { value = Mode::First; false } machine uncovered(value: &mut Mode) { transition { value in Mode::First -> {} change(value) == true -> {} value in Mode::Second -> {} } }",
            );
            validate_case_dispatch_exhaustiveness_in(&program, &TypingCohort::default())
                .expect_err("the call changes the case between borrowed reads");

            // A saved structural subject retains the borrowed place; its capture
            // identity does not snapshot the referent's case. Use the ordinary
            // capture producer with the same resolved borrowed subject.
            let expressions = &mut program.tables.bodies.expressions;
            let memberships = expressions
                .iter_expressions()
                .filter_map(|(handle, node)| match node {
                    ExpressionNode::Membership(membership) => Some((handle, membership.value)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(memberships.len(), 2);
            let capture = expressions.insert_match_subject(memberships[0].1);
            for (membership, _) in memberships {
                let saved = expressions.insert(ExpressionNode::SavedSubject(
                    resolved::expression::TableSavedSubject {
                        capture,
                        ..Default::default()
                    },
                ));
                let ExpressionNode::Membership(membership) = expressions.expression_mut(membership)
                else {
                    unreachable!();
                };
                membership.value = saved;
            }
            validate_case_dispatch_exhaustiveness_in(&program, &TypingCohort::default())
                .expect_err("a borrowed capture cannot freeze the referent's case");
        }

        #[test]
        fn constants_boolean_captures_and_builtin_indices_keep_coverage() {
            for text in [
                "const ENABLED: bool = true; machine covered() { transition { ENABLED == true -> {} } }",
                "const COUNT: u32 = 1u32; machine covered() { transition { COUNT == 0u32 -> {} COUNT != 0u32 -> {} } }",
                "machine covered<const COUNT: u32>() { transition { COUNT == 0u32 -> {} COUNT != 0u32 -> {} } }",
                "machine covered() { transition true { true -> {} false -> {} } }",
                "machine covered(left: u32, right: u32) { transition (left == right) { true -> {} false -> {} } }",
                "machine covered(items: &[bool; 2]) { transition { items[0u64] == true -> {} items[0u64] == false -> {} } }",
                "machine covered(items: &[bool; 2]) { transition items[0u64] { true -> {} false -> {} } }",
            ] {
                let program = resolve(text);
                crate::lower_symbol_resolved_trees(&program)
                    .unwrap_or_else(|error| panic!("{text}: {error:?}"));
            }
        }

        #[test]
        fn an_authored_indexer_is_an_effectful_guard_evaluation() {
            let program = resolve(
                "data Buffer { flag: bool; } machine [] select(buffer: &mut Buffer, index: u64) -> bool { buffer.flag = true; false } machine uncovered(buffer: &mut Buffer) { transition { buffer.flag == true -> {} buffer[0u64] -> {} buffer.flag == false -> {} } }",
            );
            validate_case_dispatch_exhaustiveness_in(&program, &TypingCohort::default())
                .expect_err("index syntax can invoke the authored machine and change the field");
        }

        #[test]
        fn a_complete_prefix_does_not_enumerate_later_storage_versions() {
            let suffix = "flip(value) == true -> {} value == true -> {} ".repeat(20);
            let program = resolve(&format!(
                "machine flip(value: &mut bool) -> bool {{ value = false; false }} machine covered(value: &mut bool) {{ transition {{ value == true -> {{}} value == false -> {{}} {suffix} }} }}"
            ));
            validate_case_dispatch_exhaustiveness_in(&program, &TypingCohort::default())
                .expect("the first pair covers every arrival before the later guards");
        }

        #[test]
        fn partial_matrix_search_retains_exact_witnesses_and_bounds_work() {
            let mut program = resolve("machine matrix(left: bool, right: bool) {}");
            let machine = &program.machines[0];
            let state_handle = program.machine_state_handles(machine.states)[0];
            let parameters =
                program.state_parameters(program.machine_state(state_handle).parameters);
            let symbols = [parameters[0].symbol, parameters[1].symbol];
            let subjects = symbols.map(|symbol| {
                program
                    .tables
                    .bodies
                    .expressions
                    .insert(ExpressionNode::Name(TableNamePath {
                        symbol,
                        head_symbol: symbol,
                        ..Default::default()
                    }))
            });
            let mut scope = DispatchScope {
                program: &program,
                machine: &program.machines[0],
                state: program.machine_state(state_handle),
                bindings: Vec::new(),
            };
            scope.bindings = coverage_bindings(&scope);
            let boolean = |position: usize, value| FiniteConstraint::Bool {
                subject: subjects[position],
                epoch: 0,
                value,
            };
            let mut arms = vec![
                vec![boolean(0, false)],
                vec![boolean(0, true), boolean(1, false)],
            ];
            let axes = finite_axes(&scope, &arms, &[]);
            assert!(
                matches!(first_uncovered_assignment(&scope, &axes, &arms), MatrixCoverage::Uncovered(values) if values == [1, 1])
            );
            arms.push(vec![boolean(0, true), boolean(1, true)]);
            assert!(matches!(
                first_uncovered_assignment(&scope, &axes, &arms),
                MatrixCoverage::Complete
            ));
            let expensive = vec![(0..65_537).map(|_| boolean(0, true)).collect()];
            assert!(matches!(
                first_uncovered_assignment(&scope, &axes, &expensive),
                MatrixCoverage::TooLarge
            ));
        }

        fn dispatches(
            groups: &[&[Option<bool>]],
            retained_continuation: bool,
        ) -> SymbolResolvedTrees {
            let mut program = resolve("machine dispatch_test(selected: bool) {}");
            let machine = &program.machines[0];
            let state_handle = program.machine_state_handles(machine.states)[0];
            let state = program.machine_state(state_handle);
            let subject_symbol = program.state_parameters(state.parameters)[0].symbol;
            let expressions = &mut program.tables.bodies.expressions;
            let mut members = HandleSpan::empty();
            expressions.push_name_path_member(&mut members, DiagnosticName::generated("selected"));
            let subject = expressions.insert(ExpressionNode::Name(TableNamePath {
                members,
                symbol: subject_symbol,
                head_symbol: subject_symbol,
                ..Default::default()
            }));
            let terminal = program
                .tables
                .bodies
                .statements
                .insert_transition_target(TransitionTargetNode::Terminal);
            let mut statement_nodes = HandleSpan::empty();
            for group in groups {
                for (arm_position, value) in group.iter().enumerate() {
                    let guard = match value {
                        None => TransitionGuardNode::Always,
                        Some(value) => {
                            let expressions = &mut program.tables.bodies.expressions;
                            let expected = expressions.insert(ExpressionNode::Boolean(*value));
                            TransitionGuardNode::When(expressions.insert(ExpressionNode::Binary(
                                TableBinaryExpression {
                                    left: subject,
                                    operator: BinaryOperator::Equal,
                                    right: expected,
                                    policy: numerics::arithmetic::ArithmeticDomain::Exact,
                                },
                            )))
                        }
                    };
                    let statement =
                        program
                            .tables
                            .bodies
                            .statements
                            .insert(StatementNode::Transition(TableTransition {
                                guard,
                                target: terminal,
                                continuation: if retained_continuation {
                                    terminal
                                } else {
                                    Default::default()
                                },
                                ends_dispatch: arm_position + 1 == group.len(),
                                ..Default::default()
                            }));
                    statement_nodes.push_contiguous(statement);
                }
            }
            let state = program
                .tables
                .declarations
                .machine_states
                .get_mut(state_handle);
            state.name = DiagnosticName::generated("dispatch_test");
            state.statement_nodes = statement_nodes;
            program
        }

        #[test]
        fn dispatch_continuation_does_not_supply_a_missing_boolean_arm() {
            for retained_continuation in [false, true] {
                let missing = dispatches(&[&[Some(true)]], retained_continuation);
                let error =
                    validate_case_dispatch_exhaustiveness_in(&missing, &TypingCohort::default())
                        .expect_err("the false input has no selected arm");
                assert!(
                    error
                        .message
                        .contains("transition dispatch in `dispatch_test` can fall through"),
                    "{error:?}"
                );
                for arms in [&[Some(true), Some(false)][..], &[Some(true), None][..]] {
                    validate_case_dispatch_exhaustiveness_in(
                        &dispatches(&[arms], retained_continuation),
                        &TypingCohort::default(),
                    )
                    .expect("a complete pair or wildcard covers the same subject");
                }
            }
        }

        #[test]
        fn dispatch_continuation_cannot_borrow_the_next_dispatchs_wildcard() {
            let program = dispatches(&[&[Some(true)], &[None]], true);
            let error =
                validate_case_dispatch_exhaustiveness_in(&program, &TypingCohort::default())
                    .expect_err("each authored dispatch owes its own coverage");
            assert!(
                error
                    .message
                    .contains("no arm matches when every guard is false"),
                "{error:?}"
            );
            validate_case_dispatch_exhaustiveness_in(
                &dispatches(&[&[Some(true), Some(false)], &[None]], true),
                &TypingCohort::default(),
            )
            .expect("two independently complete dispatches remain valid");
        }
    }

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
                let mut scope = DispatchScope {
                    program,
                    machine,
                    state,
                    bindings: Vec::new(),
                };
                scope.bindings = coverage_bindings(&scope);
                let statements = resolved_statements(program).statements(state.statement_nodes);
                let location =
                    if machine.name.as_str().rsplit("::").next() == Some(state.name.as_str()) {
                        machine.name.as_str().to_owned()
                    } else {
                        format!("{}::{}", machine.name, state.name)
                    };

                let mut run = Vec::new();
                for statement in statements {
                    match statement {
                        StatementNode::Transition(transition) => {
                            run.push(&transition.guard);
                            if transition.ends_dispatch {
                                check_dispatch_run(&scope, &run, &location)?;
                                run.clear();
                            }
                        }
                        _ => {
                            check_dispatch_run(&scope, &run, &location)?;
                            run.clear();
                        }
                    }
                }
                check_dispatch_run(&scope, &run, &location)?;
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
            epoch: usize,
            value: bool,
        },
    }

    struct CaseClaim {
        data_index: usize,
        subject: ExpressionHandle,
        epoch: usize,
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
        bindings: Vec<CoverageBinding>,
    }

    fn check_dispatch_run(
        scope: &DispatchScope<'_>,
        run: &[&TransitionGuardNode],
        location: &str,
    ) -> Result<(), Diagnostic> {
        let program = scope.program;
        if run.is_empty() {
            return Ok(());
        }
        // Target or continuation metadata cannot prove that a guard matches.
        // Only the arms inside this authored dispatch contribute coverage.
        let mut finite_arms: Vec<Vec<FiniteConstraint>> = Vec::new();
        let mut uncounted_claims: Vec<CaseClaim> = Vec::new();
        let mut compare_arms: Vec<(ExpressionHandle, ExpressionHandle, usize, usize, bool)> =
            Vec::new();
        let mut has_opaque_arm = false;
        let mut epoch = 1;
        let subject_epoch = |subject, epoch| {
            if snapshot_subject(scope, subject) {
                0
            } else {
                epoch
            }
        };

        for guard in run {
            if let TransitionGuardNode::When(expression) = guard
                && guard_may_change_storage(scope, *expression)
            {
                // Keep old constraints: an earlier complete prefix still covers
                // every arrival. Later memory reads receive a new storage epoch;
                // immutable scalar snapshots retain their captured identity.
                epoch += 1;
                has_opaque_arm = true;
                continue;
            }
            match classify_arm(scope, guard) {
                ArmShape::Default => return Ok(()),
                ArmShape::Finite(mut constraints) => {
                    for constraint in &mut constraints {
                        match constraint {
                            FiniteConstraint::Case(claim) => {
                                claim.epoch = subject_epoch(claim.subject, epoch)
                            }
                            FiniteConstraint::Bool {
                                subject,
                                epoch: subject_version,
                                ..
                            } => *subject_version = subject_epoch(*subject, epoch),
                        }
                    }
                    finite_arms.push(constraints);
                }
                ArmShape::UncountedCase(mut claim) => {
                    claim.epoch = subject_epoch(claim.subject, epoch);
                    uncounted_claims.push(claim);
                }
                ArmShape::Compare {
                    left,
                    right,
                    negated,
                } => compare_arms.push((
                    left,
                    right,
                    subject_epoch(left, epoch),
                    subject_epoch(right, epoch),
                    negated,
                )),
                ArmShape::Opaque => has_opaque_arm = true,
            }
        }

        let compare_pair_closes =
            compare_arms
                .iter()
                .any(|(left, right, left_epoch, right_epoch, negated)| {
                    !*negated
                        && compare_arms.iter().any(
                            |(
                                other_left,
                                other_right,
                                other_left_epoch,
                                other_right_epoch,
                                other_negated,
                            )| {
                                *other_negated
                                    && left_epoch == other_left_epoch
                                    && right_epoch == other_right_epoch
                                    && same_coverage_subject(scope, *left, *other_left)
                                    && same_coverage_subject(scope, *right, *other_right)
                            },
                        )
                });
        if compare_pair_closes {
            return Ok(());
        }

        let axes = finite_axes(scope, &finite_arms, &uncounted_claims);
        if !axes.is_empty() {
            match first_uncovered_assignment(scope, &axes, &finite_arms) {
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
                                    && claim.epoch == axes[0].epoch
                                    && same_coverage_subject(scope, axes[0].subject, claim.subject)
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
        epoch: usize,
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

    fn guard_may_change_storage(scope: &DispatchScope<'_>, root: ExpressionHandle) -> bool {
        let program = scope.program;
        let expressions = &program.tables.bodies.expressions;
        let mut pending = vec![root];
        while let Some(expression) = pending.pop() {
            match expressions.expression(expression) {
                ExpressionNode::Call(_) | ExpressionNode::Atomic(_) | ExpressionNode::Match(_) => {
                    return true;
                }
                ExpressionNode::ArrayLiteral(elements) => {
                    pending.extend(expressions.expression_handles(*elements));
                }
                ExpressionNode::StructLiteral(literal) => {
                    pending.extend(
                        expressions
                            .struct_fields(literal.fields)
                            .iter()
                            .map(|field| field.value),
                    );
                }
                ExpressionNode::Binary(binary) => {
                    if binary.operator.is_comparison() {
                        if !builtin_operand(scope, binary.left)
                            || !builtin_operand(scope, binary.right)
                        {
                            return true;
                        }
                    } else if !builtin_binary_subject(scope, binary) {
                        return true;
                    }
                    pending.extend([binary.left, binary.right]);
                }
                ExpressionNode::Unary(unary) => pending.push(unary.operand),
                ExpressionNode::Cast(cast) => {
                    if builtin_conversion_type(program, cast).is_none() {
                        return true;
                    }
                    pending.push(cast.value);
                }
                ExpressionNode::Member(member) => {
                    if !live_symbol(program, coverage_member_symbol(scope, member)) {
                        return true;
                    }
                    pending.push(member.receiver);
                }
                ExpressionNode::Indexed(indexed) => {
                    if builtin_index_element_type(scope, indexed).is_none() {
                        return true;
                    }
                    pending.extend([indexed.collection, indexed.index]);
                }
                ExpressionNode::Membership(membership) => pending.push(membership.value),
                ExpressionNode::Borrow(borrow) => pending.push(borrow.target),
                ExpressionNode::Range(range) => pending.extend(
                    [range.start, range.end]
                        .into_iter()
                        .filter(|child| child.is_valid()),
                ),
                // Saved subjects were evaluated by their capture, not by this guard.
                ExpressionNode::SavedSubject(_)
                | ExpressionNode::Boolean(_)
                | ExpressionNode::Float(_)
                | ExpressionNode::UndeterminedNaN(_)
                | ExpressionNode::Integer(_)
                | ExpressionNode::Name(_)
                | ExpressionNode::String(_)
                | ExpressionNode::TypeExpression(_)
                | ExpressionNode::ZeroValue(_) => {}
            }
        }
        false
    }

    /// Prepare binding evidence once for this scope. Storage versions can then
    /// retain copied scalars without rescanning declarations per guard.
    struct CoverageBinding {
        symbol: (u32, u32),
        primitive: bool,
        snapshot: bool,
    }

    fn coverage_bindings(scope: &DispatchScope<'_>) -> Vec<CoverageBinding> {
        let program = scope.program;
        let parameters = program.state_parameters(scope.state.parameters);
        let statements = program.state_statements(scope.state.statements);
        let binding = |symbol, reference: &TypeReference, mutable: bool| CoverageBinding {
            symbol: symbol_key(symbol),
            primitive: comparison_primitive_type(program, reference).is_some(),
            snapshot: live_symbol(program, symbol)
                && !mutable
                && scalar_value_type(program, reference).is_some(),
        };
        let mut bindings = parameters
            .iter()
            .map(|parameter| {
                binding(
                    parameter.symbol,
                    &parameter.type_reference,
                    parameter.is_mutable,
                )
            })
            .chain(statements.iter().filter_map(|statement| {
                let resolved::statement::Statement::LocalData(local) = statement else {
                    return None;
                };
                let inferred = (local.is_operand_capture || local.type_is_inferred)
                    .then(|| subject_declared_type(scope, local.initial_value))
                    .flatten();
                Some(binding(
                    local.symbol,
                    inferred.as_ref().unwrap_or(&local.type_reference),
                    local.is_mutable,
                ))
            }))
            .chain(
                program
                    .machine_type_parameters(scope.machine)
                    .iter()
                    .filter_map(|parameter| match &parameter.kind {
                        resolved::data::TypeParameterKind::Const { type_reference }
                        | resolved::data::TypeParameterKind::Value { type_reference } => {
                            Some(binding(parameter.symbol, type_reference, false))
                        }
                        _ => None,
                    }),
            )
            .collect::<Vec<_>>();
        bindings.sort_unstable_by_key(|binding| binding.symbol);
        let mut unique = Vec::with_capacity(bindings.len());
        for group in bindings.chunk_by(|left, right| left.symbol == right.symbol) {
            if let [binding] = group {
                unique.push(CoverageBinding {
                    symbol: binding.symbol,
                    primitive: binding.primitive,
                    snapshot: binding.snapshot,
                });
            }
        }
        unique
    }

    fn comparison_primitive_type<'program>(
        program: &'program SymbolResolvedTrees,
        mut reference: &'program TypeReference,
    ) -> Option<symbols::PrimitiveType> {
        loop {
            match reference {
                TypeReference::Reference(reference_) => {
                    reference = program.child_type_reference(reference_.referee)
                }
                _ => return reference.primitive_type(&program.symbols),
            }
        }
    }

    fn builtin_operand(scope: &DispatchScope<'_>, expression: ExpressionHandle) -> bool {
        let program = scope.program;
        let bindings = &scope.bindings;
        let expressions = &program.tables.bodies.expressions;
        match expressions.expression(expression) {
            ExpressionNode::Boolean(_) | ExpressionNode::Integer(_) | ExpressionNode::Float(_) => {
                true
            }
            ExpressionNode::Name(path) => {
                !path.static_application.is_valid()
                    && if let Ok(position) = bindings
                        .binary_search_by_key(&symbol_key(path.symbol), |binding| binding.symbol)
                    {
                        (!path.head_symbol.is_valid() || path.head_symbol == path.symbol)
                            && bindings[position].primitive
                    } else {
                        live_symbol(program, path.symbol)
                            && program.symbols.get(path.symbol).kind == symbols::SymbolKind::Const
                            && program
                                .const_declaration(path.symbol)
                                .is_some_and(|declaration| {
                                    declaration
                                        .declared_type
                                        .primitive_type(&program.symbols)
                                        .is_some()
                                })
                    }
            }
            ExpressionNode::SavedSubject(saved) => {
                builtin_operand(scope, expressions.match_subject(saved.capture).subject)
            }
            // Only a saved subject uses this signature query: the call has
            // already run once before its capture, never while testing a guard.
            ExpressionNode::Call(call) => declared_call_return(program, call.target_symbol)
                .is_some_and(|reference| reference.primitive_type(&program.symbols).is_some()),
            ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
                subject_declared_type(scope, expression).is_some_and(|reference| {
                    comparison_primitive_type(program, &reference).is_some()
                })
            }
            ExpressionNode::Cast(cast) => builtin_conversion_type(program, cast).is_some(),
            ExpressionNode::Unary(unary) => builtin_operand(scope, unary.operand),
            ExpressionNode::Binary(binary) => {
                builtin_binary_subject(scope, binary)
                    && builtin_operand(scope, binary.left)
                    && builtin_operand(scope, binary.right)
            }
            _ => false,
        }
    }

    fn symbol_key(symbol: SymbolHandle) -> (u32, u32) {
        (symbol.arena_index(), symbol.generation())
    }

    fn scalar_value_type<'program>(
        program: &'program SymbolResolvedTrees,
        mut reference: &'program TypeReference,
    ) -> Option<symbols::PrimitiveType> {
        loop {
            match reference {
                TypeReference::Constrained(constrained) => {
                    reference = program.child_type_reference(constrained.base_type)
                }
                TypeReference::AliasUse { expansion, .. } => reference = expansion,
                _ => return reference.primitive_type(&program.symbols),
            }
        }
    }

    /// Memory projections and borrowed captures retain a storage version;
    /// copied scalar values retain their identity across an unrelated call.
    fn snapshot_subject(scope: &DispatchScope<'_>, expression: ExpressionHandle) -> bool {
        let program = scope.program;
        let bindings = &scope.bindings;
        match program.tables.bodies.expressions.expression(expression) {
            ExpressionNode::Boolean(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::String(_) => true,
            ExpressionNode::SavedSubject(saved) => {
                saved.capture.is_valid()
                    && subject_declared_type(
                        scope,
                        program
                            .tables
                            .bodies
                            .expressions
                            .match_subject(saved.capture)
                            .subject,
                    )
                    .is_some_and(|reference| scalar_value_type(program, &reference).is_some())
            }
            ExpressionNode::Name(path) => {
                !path.static_application.is_valid()
                    && if let Ok(position) = bindings
                        .binary_search_by_key(&symbol_key(path.symbol), |binding| binding.symbol)
                    {
                        (!path.head_symbol.is_valid() || path.head_symbol == path.symbol)
                            && bindings[position].snapshot
                    } else {
                        live_symbol(program, path.symbol)
                            && program.symbols.get(path.symbol).kind == symbols::SymbolKind::Const
                            && program
                                .const_declaration(path.symbol)
                                .is_some_and(|declaration| {
                                    scalar_value_type(program, &declaration.declared_type).is_some()
                                })
                    }
            }
            ExpressionNode::Cast(cast) => {
                builtin_conversion_type(program, cast).is_some()
                    && snapshot_subject(scope, cast.value)
            }
            ExpressionNode::Unary(unary) => snapshot_subject(scope, unary.operand),
            ExpressionNode::Binary(binary) => {
                builtin_binary_subject(scope, binary)
                    && snapshot_subject(scope, binary.left)
                    && snapshot_subject(scope, binary.right)
            }
            _ => false,
        }
    }

    fn builtin_binary_subject(
        scope: &DispatchScope<'_>,
        binary: &resolved::expression::TableBinaryExpression,
    ) -> bool {
        // A qualified operand may select an authored operation during typing;
        // an unqualified builtin operand cannot acquire a foreign operator.
        // Explicit policy/logical/bitwise operations also have builtin meaning.
        binary.policy != numerics::arithmetic::ArithmeticDomain::Exact
            || matches!(
                binary.operator,
                BinaryOperator::And
                    | BinaryOperator::Or
                    | BinaryOperator::BitwiseAnd
                    | BinaryOperator::BitwiseOr
                    | BinaryOperator::BitwiseXor
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight
            )
            || builtin_operand(scope, binary.left) && builtin_operand(scope, binary.right)
    }

    fn builtin_conversion_type(
        program: &SymbolResolvedTrees,
        cast: &resolved::expression::TableCastExpression,
    ) -> Option<symbols::PrimitiveType> {
        if cast.form != language_core::cast_form::CastForm::Value
            || !cast.semantic_domain.is_empty()
            || cast.semantic_domain_symbol.is_valid()
            || !cast.semantic_domain_arguments.is_empty()
        {
            return None;
        }
        let types = &program.tables.declarations.child_type_references;
        types
            .is_valid(cast.target_type)
            .then(|| types.get(cast.target_type).primitive_type(&program.symbols))
            .flatten()
    }

    fn finite_axes(
        scope: &DispatchScope<'_>,
        arms: &[Vec<FiniteConstraint>],
        uncounted_claims: &[CaseClaim],
    ) -> Vec<FiniteAxis> {
        let mut axes = Vec::new();
        for constraint in arms.iter().flatten() {
            match constraint {
                FiniteConstraint::Case(claim) => push_axis(
                    scope,
                    &mut axes,
                    claim.subject,
                    claim.epoch,
                    FiniteAxisKind::Case {
                        data_index: claim.data_index,
                    },
                ),
                FiniteConstraint::Bool { subject, epoch, .. } => {
                    push_axis(scope, &mut axes, *subject, *epoch, FiniteAxisKind::Bool)
                }
            }
        }
        for claim in uncounted_claims {
            push_axis(
                scope,
                &mut axes,
                claim.subject,
                claim.epoch,
                FiniteAxisKind::Case {
                    data_index: claim.data_index,
                },
            );
        }
        axes
    }

    fn push_axis(
        scope: &DispatchScope<'_>,
        axes: &mut Vec<FiniteAxis>,
        subject: ExpressionHandle,
        epoch: usize,
        kind: FiniteAxisKind,
    ) {
        let program = scope.program;
        if axes.iter().any(|axis| {
            axis.epoch == epoch
                && axis_kinds_equal(axis.kind, kind)
                && same_coverage_subject(scope, axis.subject, subject)
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
            epoch,
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
        scope: &DispatchScope<'_>,
        axes: &[FiniteAxis],
        arms: &[Vec<FiniteConstraint>],
    ) -> MatrixCoverage {
        const MAX_MATRIX_WORK: usize = 65_536;
        let arities = axes
            .iter()
            .map(|axis| match axis.kind {
                FiniteAxisKind::Case { .. } => axis.possible_cases.len(),
                FiniteAxisKind::Bool => 2,
            })
            .collect::<Vec<_>>();
        if arities.contains(&0) {
            return MatrixCoverage::Complete;
        }
        // Bind each constraint to its exact subject/version once, before search.
        // A malformed/unbound arm contributes no coverage.
        let prepared = arms
            .iter()
            .filter_map(|arm| {
                arm.iter()
                    .map(|constraint| {
                        constraint_axis(scope, axes, constraint).map(|axis| (axis, constraint))
                    })
                    .collect::<Option<Vec<_>>>()
            })
            .collect::<Vec<_>>();
        let mut assignment = vec![usize::MAX; axes.len()];
        let mut depth = 0;
        let mut work = 0;
        loop {
            work += 1;
            if work > MAX_MATRIX_WORK {
                return MatrixCoverage::TooLarge;
            }
            let mut covered = false;
            let mut pending = false;
            for arm in &prepared {
                let mut possible = true;
                let mut decided = true;
                for (axis, constraint) in arm {
                    work += 1;
                    if work > MAX_MATRIX_WORK {
                        return MatrixCoverage::TooLarge;
                    }
                    let value = assignment[*axis];
                    if value == usize::MAX {
                        decided = false;
                    } else if !constraint_matches(&axes[*axis], value, constraint) {
                        possible = false;
                        break;
                    }
                }
                if possible && decided {
                    covered = true;
                    break;
                }
                pending |= possible;
            }
            if !covered && !pending {
                // All arms are already impossible under this prefix, so any
                // completion is an actual uncovered assignment.
                for value in &mut assignment[depth..] {
                    *value = 0;
                }
                return MatrixCoverage::Uncovered(assignment);
            }
            if !covered {
                assignment[depth] = 0;
                depth += 1;
                continue;
            }
            // A satisfied arm covers every completion of this prefix. Explicit
            // work frames avoid enumerating later storage epochs (or using the
            // host stack) once an earlier dispatch prefix is already exhaustive.
            loop {
                if depth == 0 {
                    return MatrixCoverage::Complete;
                }
                depth -= 1;
                assignment[depth] += 1;
                if assignment[depth] < arities[depth] {
                    depth += 1;
                    break;
                }
                assignment[depth] = usize::MAX;
            }
        }
    }

    fn constraint_axis(
        scope: &DispatchScope<'_>,
        axes: &[FiniteAxis],
        constraint: &FiniteConstraint,
    ) -> Option<usize> {
        let (subject, epoch, kind) = match constraint {
            FiniteConstraint::Case(claim) => (
                claim.subject,
                claim.epoch,
                FiniteAxisKind::Case {
                    data_index: claim.data_index,
                },
            ),
            FiniteConstraint::Bool { subject, epoch, .. } => {
                (*subject, *epoch, FiniteAxisKind::Bool)
            }
        };
        axes.iter().position(|axis| {
            axis.epoch == epoch
                && axis_kinds_equal(axis.kind, kind)
                && same_coverage_subject(scope, axis.subject, subject)
        })
    }

    fn constraint_matches(axis: &FiniteAxis, value: usize, constraint: &FiniteConstraint) -> bool {
        match constraint {
            FiniteConstraint::Case(claim) => claim
                .covered
                .as_ref()
                .is_some_and(|variants| variants.contains(&axis.possible_cases[value])),
            FiniteConstraint::Bool {
                value: expected, ..
            } => value == usize::from(*expected),
        }
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
                            epoch: 0,
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
                            epoch: 0,
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
                        epoch: 0,
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
                            epoch: 0,
                            value: *value,
                        });
                        continue;
                    }
                    if let ExpressionNode::Boolean(value) =
                        program.tables.bodies.expressions.expression(binary.left)
                    {
                        constraints.push(FiniteConstraint::Bool {
                            subject: binary.right,
                            epoch: 0,
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
                if !same_coverage_subject(scope, subject, membership.value) {
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
            epoch: 0,
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

    fn same_coverage_subject(
        scope: &DispatchScope<'_>,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> bool {
        let program = scope.program;
        // An expression handle names syntax, not one runtime evaluation. In
        // particular, even the same call node can run again in another guard.
        if !program.tables.bodies.expressions.expression_is_valid(left)
            || !program.tables.bodies.expressions.expression_is_valid(right)
        {
            return false;
        }

        let expressions = &program.tables.bodies.expressions;
        match (expressions.expression(left), expressions.expression(right)) {
            (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) => left == right,
            (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => left == right,
            (ExpressionNode::Float(left), ExpressionNode::Float(right)) => left == right,
            (ExpressionNode::String(left), ExpressionNode::String(right)) => left == right,
            (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
                live_symbol(program, left.symbol)
                    && left.symbol == right.symbol
                    && left.head_symbol == right.head_symbol
                    && (left.head_symbol == left.symbol
                        || left.members.count() == left.member_symbols.count()
                            && right.members.count() == right.member_symbols.count()
                            && expressions
                                .name_path_member_symbols(left.member_symbols)
                                .iter()
                                .all(|symbol| live_symbol(program, *symbol)))
                    && expressions.name_path_member_symbols(left.member_symbols)
                        == expressions.name_path_member_symbols(right.member_symbols)
                    && !left.static_application.is_valid()
                    && !right.static_application.is_valid()
                    && left.reserved_result_of == right.reserved_result_of
            }
            (ExpressionNode::Member(left), ExpressionNode::Member(right)) => {
                let left_symbol = coverage_member_symbol(scope, left);
                live_symbol(program, left_symbol)
                    && left_symbol == coverage_member_symbol(scope, right)
                    && left.case_variant == right.case_variant
                    && same_coverage_subject(scope, left.receiver, right.receiver)
            }
            (ExpressionNode::Indexed(left), ExpressionNode::Indexed(right)) => {
                builtin_index_element_type(scope, left).is_some()
                    && same_coverage_subject(scope, left.collection, right.collection)
                    && same_coverage_subject(scope, left.index, right.index)
            }
            (ExpressionNode::Unary(left), ExpressionNode::Unary(right)) => {
                left.operator == right.operator
                    && same_coverage_subject(scope, left.operand, right.operand)
            }
            (ExpressionNode::Binary(left), ExpressionNode::Binary(right)) => {
                builtin_binary_subject(scope, left)
                    && left.operator == right.operator
                    && left.policy == right.policy
                    && same_coverage_subject(scope, left.left, right.left)
                    && same_coverage_subject(scope, left.right, right.right)
            }
            (ExpressionNode::Membership(left), ExpressionNode::Membership(right)) => {
                left.domain_symbol == right.domain_symbol
                    && left.case_type_symbol == right.case_type_symbol
                    && left.case_symbol == right.case_symbol
                    && (live_symbol(program, left.domain_symbol)
                        || live_symbol(program, left.case_type_symbol)
                            && live_symbol(program, left.case_symbol))
                    && same_coverage_subject(scope, left.value, right.value)
            }
            (ExpressionNode::SavedSubject(left), ExpressionNode::SavedSubject(right)) => {
                left.capture.is_valid()
                    && left.capture == right.capture
                    && left.component == right.component
                    && left.pattern_head == right.pattern_head
            }
            (ExpressionNode::Cast(left), ExpressionNode::Cast(right)) => {
                builtin_conversion_type(program, left).is_some_and(|left_type| {
                    Some(left_type) == builtin_conversion_type(program, right)
                }) && left.domain == right.domain
                    && same_coverage_subject(scope, left.value, right.value)
            }
            // A call's declaration and complete application identify what runs,
            // never the value of two separate invocations. Bind its result or
            // use a dispatch capture before asking coverage to share it.
            (ExpressionNode::Call(_), ExpressionNode::Call(_)) => false,
            _ => false,
        }
    }

    fn live_symbol(program: &SymbolResolvedTrees, symbol: SymbolHandle) -> bool {
        program.symbols.symbols().nodes().is_valid(symbol)
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
                    declared_member_field(scope, member).map(|field| field.type_reference.clone())
                }),
            ExpressionNode::Boolean(_)
            | ExpressionNode::Membership(_)
            | ExpressionNode::Unary(resolved::expression::TableUnaryExpression {
                operator: resolved::expression::UnaryOperator::LogicalNot,
                ..
            })
            | ExpressionNode::Binary(resolved::expression::TableBinaryExpression {
                operator:
                    BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
                    | BinaryOperator::And
                    | BinaryOperator::Or,
                ..
            }) => Some(TypeReference::Named {
                symbol: scope
                    .program
                    .symbols
                    .builtin_atom_symbol(symbols::BuiltinTypeAtom::Bool)?,
                name: DiagnosticName::generated("bool"),
                supplied_operand_owner: Default::default(),
            }),
            ExpressionNode::Indexed(indexed) => builtin_index_element_type(scope, indexed),
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

    fn builtin_index_element_type(
        scope: &DispatchScope<'_>,
        indexed: &resolved::expression::TableIndexedExpression,
    ) -> Option<TypeReference> {
        let collection = subject_declared_type(scope, indexed.collection)?;
        let mut reference = &collection;
        loop {
            match reference {
                TypeReference::Reference(borrow) => {
                    reference = scope.program.child_type_reference(borrow.referee)
                }
                TypeReference::AliasUse { expansion, .. } => reference = expansion,
                TypeReference::FixedArray(array) => {
                    let element = scope.program.child_type_reference(array.element_type);
                    return (element.primitive_type(&scope.program.symbols).is_some()
                        && builtin_operand(scope, indexed.index))
                    .then(|| element.clone());
                }
                TypeReference::Slice(slice) => {
                    let element = scope.program.child_type_reference(slice.element_type);
                    return (element.primitive_type(&scope.program.symbols).is_some()
                        && builtin_operand(scope, indexed.index))
                    .then(|| element.clone());
                }
                _ => return None,
            }
        }
    }

    fn coverage_member_symbol(
        scope: &DispatchScope<'_>,
        member: &resolved::expression::TableMemberExpression,
    ) -> SymbolHandle {
        if live_symbol(scope.program, member.member_symbol) {
            member.member_symbol
        } else {
            declared_member_field(scope, member)
                .map_or_else(SymbolHandle::invalid, |field| field.symbol)
        }
    }

    /// Typing supplies a receiver's selected carrier when resolution could not
    /// name its member yet. Coverage and type discovery use that same unique
    /// field declaration, never the member's leaf spelling as an identity.
    fn declared_member_field<'program>(
        scope: &DispatchScope<'program>,
        member: &resolved::expression::TableMemberExpression,
    ) -> Option<&'program resolved::data::DataField> {
        let program = scope.program;
        let receiver = subject_declared_type(scope, member.receiver)?;
        let definition = &program.data_definitions[data_index_for_type(program, &receiver)?];
        let mut fields = program
            .data_members(definition.members)
            .iter()
            .filter_map(|member_| match member_ {
                DataMember::Field(field) if field.name == member.member => Some(field),
                _ => None,
            });
        match (fields.next(), fields.next()) {
            (Some(field), None)
                if !member.member_symbol.is_valid() || member.member_symbol == field.symbol =>
            {
                Some(field)
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
            let parameters = scope.program.state_parameters(scope.state.parameters);
            if let Some(position) = scope
                .program
                .resolution_indexes
                .parameter_position(parameters, symbol)
                && let Some(parameter) = parameters.get(position as usize)
                && parameter.symbol == symbol
            {
                return Some(parameter.type_reference.clone());
            }
            let statements = scope.program.state_statements(scope.state.statements);
            if let Some(position) = scope
                .program
                .resolution_indexes
                .local_statement_position(statements, symbol)
                && let Some(resolved::statement::Statement::LocalData(local)) =
                    statements.get(position as usize)
                && local.symbol == symbol
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
        if !live_symbol(scope.program, symbol) {
            return None;
        }
        let parent = scope.program.symbols.get(symbol).parent;
        if !live_symbol(scope.program, parent) {
            return None;
        }
        let owner = if scope.program.symbols.get(parent).kind == symbols::SymbolKind::Variant {
            scope.program.symbols.get(parent).parent
        } else {
            parent
        };
        let position = scope
            .program
            .resolution_indexes
            .data_definition_position(&scope.program.data_definitions, owner)?;
        let definition = &scope.program.data_definitions[position as usize];
        if definition.symbol != owner {
            return None;
        }
        for member in scope.program.data_members(definition.members) {
            match member {
                DataMember::Field(field) if field.symbol == symbol && parent == owner => {
                    return Some(field.type_reference.clone());
                }
                DataMember::Variant(variant) if variant.symbol == parent => {
                    return scope
                        .program
                        .data_payload_fields(variant.payload)
                        .iter()
                        .find(|field| field.symbol == symbol)
                        .map(|field| field.type_reference.clone());
                }
                _ => {}
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
                .resolution_indexes
                .data_definition_position(&program.data_definitions, *symbol)
                .map(|position| position as usize),
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
                            &lowerer.source_trees.symbols,
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
                    } else if !proposition_const_symbol_type(lowerer, symbol).is_some_and(
                        |actual| {
                            proposition_const_types_match(
                                &lowerer.source_trees.symbols,
                                type_reference,
                                actual,
                            )
                        },
                    ) {
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
        symbols: &symbols::SymbolTable,
        proposition: &str,
        binder: &str,
        type_reference: &TypeReference,
        literal: &numerics::literals::IntegerLiteral,
    ) -> Result<(), Diagnostic> {
        let Some(primitive) = type_reference.primitive_type(symbols) else {
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

    fn proposition_const_types_match(
        symbols: &symbols::SymbolTable,
        expected: &TypeReference,
        actual: &TypeReference,
    ) -> bool {
        match (
            expected.primitive_type(symbols),
            actual.primitive_type(symbols),
        ) {
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
        /* The address holds the unsigned 64-bit range. */
        let carrier = match primitive {
            PrimitiveType::Integer(carrier) => carrier,
            PrimitiveType::Addr => numerics::integer_carrier::FixedInteger::U64,
            PrimitiveType::Bool
            | PrimitiveType::F32
            | PrimitiveType::F64
            | PrimitiveType::Selector => return false,
        };
        literal
            .value_bignum()
            .is_some_and(|value| carrier.admits(&value))
    }
}
