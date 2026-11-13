/*
Structural call parameter and result custody, declaration predicate rosters, and
qualification contracts (`machine_calls::structural_call_custody`).
*/
use crate::frozen_program::expression_table;
use checked_trees::CheckedMemberIdentity;
use checked_trees::{
    CheckFacts, CheckedStructuralAccess, CheckedStructuralCallCustodyPlan,
    CheckedStructuralPathQualification, CheckedStructuralResultClaimPlan,
    CheckedUnitCallCoordinate, CheckedUnitClaimTransferPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralPathSegment, FlowCallFact, FlowClaimOutcomeEntryFact,
    FlowClaimOutcomeSource,
};
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, SemanticDomainId,
};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;
use typed_trees::statement::TransitionTargetNode;
use typed_trees::types::DomainConstraintSubject;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

pub(crate) mod declaration_predicates {
    use crate::frozen_program::expression_table;
    use crate::proof_contracts::collection_predicates;
    use checked_trees::CheckedMemberIdentity;
    use checked_trees::{
        CheckedBooleanExpression, CheckedIeeeFloatComparisonKind, CheckedIntegerComparisonKind,
        CheckedOperatorFacts, CheckedScalarExpression, CheckedStructuralPredicatePathSegment,
    };
    use collection_predicates::PredicateIntegerOperand;
    use language_core::ReferenceAccess;
    use numerics::bignum::BigInt;
    use numerics::literals::FloatFormat;
    use symbols::SymbolHandle;
    use typed_trees::data::DataVariant;
    use typed_trees::data::{DataDefinition, DataField, DataMember};
    use typed_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
    };
    use typed_trees::types::{
        FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
    };

    pub(crate) mod field_carriers {
        use super::{FactSpelling, carrier_bounds};
        use crate::proof_contracts::arithmetic_domains::BigIntInterval;
        use checked_trees::{
            CheckedBooleanExpression as Boolean, CheckedIntegerComparisonKind as Comparison,
            CheckedMemberIdentity, CheckedScalarExpression as Scalar, CheckedStructuralCaseRoot,
            CheckedStructuralPredicatePathSegment as Segment,
        };
        use numerics::bignum::BigInt;
        use typed_trees::data::{DataDefinition, DataVariant};

        pub(crate) fn imply_declaration(
            program: &dyn crate::CheckingInput,
            operators: &checked_trees::CheckedOperatorFacts,
            definition: &DataDefinition,
        ) -> bool {
            let spelling = FactSpelling {
                program,
                operators,
                parameter_position: 0,
                prefix: &[],
                definition,
                selected_case: None,
            };
            let Some(predicates) = program
                .proof_facts
                .span_or_empty(definition.requires_facts)
                .iter()
                .map(|fact| spelling.boolean(*fact.as_expression()?))
                .collect::<Option<Vec<_>>>()
            else {
                return false;
            };
            let mut cases = program
                .data_members(definition)
                .iter()
                .filter_map(|member| member.as_variant())
                .peekable();
            if cases.peek().is_none() {
                let carriers = FieldCarriers {
                    program,
                    definition,
                    case: None,
                };
                return predicates
                    .iter()
                    .all(|predicate| carriers.boolean(predicate) == Some(true));
            }
            cases.all(|case| {
                let carriers = FieldCarriers {
                    program,
                    definition,
                    case: Some(case),
                };
                predicates
                    .iter()
                    .all(|predicate| carriers.boolean(predicate) == Some(true))
            })
        }

        pub(crate) struct FieldCarriers<'a> {
            program: &'a dyn crate::CheckingInput,
            definition: &'a DataDefinition,
            case: Option<&'a DataVariant>,
        }

        impl FieldCarriers<'_> {
            fn boolean(&self, predicate: &Boolean) -> Option<bool> {
                match predicate {
                    Boolean::Constant(value) => Some(*value),
                    Boolean::StructuralCaseMembership { subject, case }
                        if subject.root
                            == (CheckedStructuralCaseRoot::Parameter { position: 0 })
                            && subject.path.is_empty() =>
                    {
                        let selected = self.case?;
                        Some(
                            *case
                                == CheckedMemberIdentity::of_member(
                                    selected.identity,
                                    selected.name.clone(),
                                ),
                        )
                    }
                    Boolean::Not(operand) => Some(!self.boolean(operand)?),
                    Boolean::And { left, right } => {
                        Some(self.boolean(left)? && self.boolean(right)?)
                    }
                    Boolean::Or { left, right } => {
                        Some(self.boolean(left)? || self.boolean(right)?)
                    }
                    Boolean::Equal { left, right } => {
                        Some(self.boolean(left)? == self.boolean(right)?)
                    }
                    Boolean::IntegerComparison { kind, left, right } => {
                        let left = self.integer(left)?;
                        let right = self.integer(right)?;
                        let (left_low, left_high) = (&left.minimum, &left.maximum);
                        let (right_low, right_high) = (&right.minimum, &right.maximum);
                        match kind {
                            Comparison::LessOrEqual if left_high <= right_low => Some(true),
                            Comparison::LessOrEqual if left_low > right_high => Some(false),
                            Comparison::LessThan if left_high < right_low => Some(true),
                            Comparison::LessThan if left_low >= right_high => Some(false),
                            Comparison::Equal
                                if left_low == left_high
                                    && right_low == right_high
                                    && left_low == right_low =>
                            {
                                Some(true)
                            }
                            Comparison::Equal if left_high < right_low || right_high < left_low => {
                                Some(false)
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                }
            }

            fn integer(&self, expression: &Scalar) -> Option<facts::IntegerRange> {
                match expression {
                    Scalar::IntegerLiteral { literal } => {
                        let value = literal
                            .value_i64()
                            .map(i128::from)
                            .or_else(|| literal.value_u64().map(i128::from))?;
                        BigIntInterval::constant(BigInt::from_i128(value)).into_closed()
                    }
                    Scalar::StructuralParameterField {
                        root: checked_trees::CheckedStorageRoot::Parameter { index: 0 },
                        path,
                        scalar_type,
                        ..
                    } => {
                        let primitive_type = scalar_type.primitive()?;
                        let field = match path.as_slice() {
                            [Segment::Field(identity)] => {
                                self.program.data_members(self.definition).iter().find_map(
                                    |member| {
                                        member.as_field().filter(|field| {
                                            field.identity == identity.ordinal
                                                && field.name == identity.name
                                        })
                                    },
                                )?
                            }
                            [Segment::Case(case), Segment::Field(identity)] => {
                                let selected = self.case?;
                                if *case
                                    != CheckedMemberIdentity::of_member(
                                        selected.identity,
                                        selected.name.clone(),
                                    )
                                {
                                    return None;
                                }
                                self.program
                                    .data_payload_fields(selected)
                                    .iter()
                                    .find(|field| {
                                        field.identity == identity.ordinal
                                            && field.name == identity.name
                                    })?
                            }
                            _ => return None,
                        };
                        if field.relevance.is_erased()
                            || self.program.primitive_type_reference(field.type_reference)
                                != Some(primitive_type)
                        {
                            return None;
                        }
                        let (low, high) = carrier_bounds(primitive_type)?;
                        let mut interval = BigIntInterval {
                            low: Some(BigInt::from_i128(low)),
                            high: Some(BigInt::from_i128(high)),
                        };
                        if let Some((low, high)) =
                            crate::enforced_integer_type_bounds(self.program, field.type_reference)
                        {
                            interval = interval.intersect(&BigIntInterval {
                                low: Some(BigInt::from_i64(low)),
                                high: Some(BigInt::from_i64(high)),
                            });
                        }
                        if let Some(resident) =
                            crate::proof_contracts::default_domains::atomic_field_interval(
                                self.program,
                                self.definition,
                                field,
                            )
                        {
                            interval = interval.intersect(&BigIntInterval {
                                low: resident.low.map(BigInt::from_i64),
                                high: resident.high.map(BigInt::from_i64),
                            });
                        }
                        interval.into_closed()
                    }
                    _ => None,
                }
            }
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(crate) struct StorageDefinitionIdentity {
        symbol: SymbolHandle,
        application: Option<TypeReferenceHandle>,
    }

    impl StorageDefinitionIdentity {
        fn of(definition: &DataDefinition) -> Self {
            Self {
                symbol: definition.symbol,
                application: definition.generic_instance,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct DeclarationPredicateRow {
        pub parameter_position: u32,
        pub restored_on_return: bool,
        pub predicate: CheckedBooleanExpression,
    }

    pub fn state_declaration_predicates(
        program: &dyn crate::CheckingInput,
        operators: &CheckedOperatorFacts,
        state: &typed_trees::state::State,
    ) -> Vec<DeclarationPredicateRow> {
        let mut rows = Vec::new();
        let mut emitting = Vec::new();
        for (position, parameter) in program.state_parameters(state).iter().enumerate() {
            let Ok(parameter_position) = u32::try_from(position) else {
                break;
            };
            if parameter.is_const || parameter.relevance.is_erased() {
                continue;
            }
            let (referee, access) = match program
                .type_reference_table
                .type_reference(parameter.type_reference)
            {
                TypeReferenceNode::Reference {
                    referee, access, ..
                } => (*referee, Some(*access)),
                _ => (parameter.type_reference, None),
            };
            let restored_on_return = match access {
                Some(ReferenceAccess::Mutable) => true,
                Some(ReferenceAccess::Shared) => false,
                Some(ReferenceAccess::WriteOnly) => continue,
                None => false,
            };
            let mut emitter = RowEmitter {
                program,
                operators,
                parameter_position,
                restored_on_return,
                case_guards: Vec::new(),
                rows: &mut rows,
                emitting: &mut emitting,
            };
            match parameter_data_definition(program, referee) {
                Some(definition) => {
                    let mut visited = vec![StorageDefinitionIdentity::of(definition)];
                    emitter.definition_storage(&mut Vec::new(), definition, &mut visited);
                }
                None => emitter.descend(&mut Vec::new(), referee, &mut Vec::new()),
            }
        }
        rows
    }

    fn parameter_data_definition(
        program: &dyn crate::CheckingInput,
        referee: TypeReferenceHandle,
    ) -> Option<&DataDefinition> {
        let TypeReferenceNode::Generic {
            base_symbol: owner,
            arguments,
            ..
        } = program.type_reference_table.type_reference(
            program
                .type_reference_table
                .unconstrained_type_reference(referee),
        )
        else {
            return None;
        };
        if !arguments.is_empty() {
            return None;
        }
        program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == *owner && definition.generic_instance.is_none())
    }

    pub fn machine_declaration_predicates(
        program: &dyn crate::CheckingInput,
        operators: &CheckedOperatorFacts,
        machine: &typed_trees::machine::Machine,
    ) -> Vec<DeclarationPredicateRow> {
        program
            .machine_states(machine)
            .first()
            .map(|entry| state_declaration_predicates(program, operators, entry))
            .unwrap_or_default()
    }

    pub fn declaration_facts_carried<'program>(
        program: &'program dyn crate::CheckingInput,
        operators: &CheckedOperatorFacts,
        definition: &'program DataDefinition,
    ) -> bool {
        complete_definition_storage(
            program,
            operators,
            definition,
            &mut Vec::new(),
            &mut Vec::new(),
        )
    }

    fn complete_definition_storage<'program>(
        program: &'program dyn crate::CheckingInput,
        operators: &CheckedOperatorFacts,
        definition: &'program DataDefinition,
        visited: &mut Vec<StorageDefinitionIdentity>,
        checked_own_facts: &mut Vec<&'program DataDefinition>,
    ) -> bool {
        if visited.contains(&StorageDefinitionIdentity::of(definition)) {
            return false;
        }
        if definition_emits_rows(program, operators, definition)
            && !checked_own_facts
                .iter()
                .any(|known| std::ptr::eq(*known, definition))
        {
            if !definition_own_facts_carried(program, operators, definition) {
                return false;
            }
            checked_own_facts.push(definition);
        }
        visited.push(StorageDefinitionIdentity::of(definition));
        let complete = owned_storage_fields(program, definition).all(|(_, field)| {
            complete_owned_storage(
                program,
                operators,
                field.type_reference,
                visited,
                checked_own_facts,
            )
        });
        visited.pop();
        complete
    }

    fn complete_owned_storage<'program>(
        program: &'program dyn crate::CheckingInput,
        operators: &CheckedOperatorFacts,
        reference: TypeReferenceHandle,
        visited: &mut Vec<StorageDefinitionIdentity>,
        checked_own_facts: &mut Vec<&'program DataDefinition>,
    ) -> bool {
        let carrier = program
            .type_reference_table
            .unconstrained_type_reference(reference);
        match program.type_reference_table.type_reference(carrier) {
            TypeReferenceNode::FixedArray {
                length: FixedArrayLength::Literal(0),
                ..
            } => true,
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(_),
            } => complete_owned_storage(
                program,
                operators,
                *element_type,
                visited,
                checked_own_facts,
            ),
            TypeReferenceNode::FixedArray { element_type, .. } => {
                !storage_carries_rows(program, operators, *element_type, &mut Vec::new())
            }
            TypeReferenceNode::Reference { .. } => true,
            _ => owned_data_definition(program, carrier).is_none_or(|definition| {
                complete_definition_storage(
                    program,
                    operators,
                    definition,
                    visited,
                    checked_own_facts,
                )
            }),
        }
    }

    fn definition_fact_spans<'a>(
        program: &'a dyn crate::CheckingInput,
        definition: &'a DataDefinition,
    ) -> impl Iterator<
        Item = (
            Option<&'a DataVariant>,
            arena::HandleSpan<typed_trees::domain::ProofFact>,
        ),
    > {
        std::iter::once((None, definition.requires_facts)).chain(
            program
                .data_members(definition)
                .iter()
                .filter_map(move |member| {
                    let case = member.as_variant()?;
                    (!crate::impossible_variant(program, case))
                        .then_some((Some(case), case.requires_facts))
                }),
        )
    }

    fn definition_own_facts_carried(
        program: &dyn crate::CheckingInput,
        operators: &CheckedOperatorFacts,
        definition: &DataDefinition,
    ) -> bool {
        definition_fact_spans(program, definition).all(|(case, facts)| {
            let spelling = FactSpelling {
                program,
                operators,
                parameter_position: 0,
                prefix: &[],
                definition,
                selected_case: case,
            };
            program.conjunctive_proof_facts(facts).all(|fact| {
                let Some(expression) = fact.as_expression() else {
                    return false;
                };
                spelling.boolean(*expression).is_some()
                    || collection_predicates::data_fact_element_consequences(
                        program,
                        definition,
                        *expression,
                    )
                    .is_some_and(|consequence| consequence.exact)
            })
        })
    }

    pub(crate) struct RowEmitter<'a, 'program> {
        program: &'program dyn crate::CheckingInput,
        operators: &'program CheckedOperatorFacts,
        parameter_position: u32,
        restored_on_return: bool,
        case_guards: Vec<CheckedBooleanExpression>,
        rows: &'a mut Vec<DeclarationPredicateRow>,
        emitting: &'a mut Vec<(*const DataDefinition, bool)>,
    }

    impl RowEmitter<'_, '_> {
        fn descend(
            &mut self,
            prefix: &mut Vec<CheckedStructuralPredicatePathSegment>,
            type_reference: TypeReferenceHandle,
            visited: &mut Vec<StorageDefinitionIdentity>,
        ) {
            let carrier = self
                .program
                .type_reference_table
                .unconstrained_type_reference(type_reference);
            match self.program.type_reference_table.type_reference(carrier) {
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Literal(length),
                } => {
                    if !storage_carries_rows(self.program, self.operators, *element_type, visited) {
                        return;
                    }
                    for index in 0..*length {
                        let Ok(index) = u64::try_from(index) else {
                            return;
                        };
                        prefix.push(CheckedStructuralPredicatePathSegment::FixedIndex(index));
                        self.descend(prefix, *element_type, visited);
                        prefix.pop();
                    }
                }
                _ => {
                    let Some(definition) = owned_data_definition(self.program, carrier) else {
                        return;
                    };
                    if visited.contains(&StorageDefinitionIdentity::of(definition)) {
                        return;
                    }
                    visited.push(StorageDefinitionIdentity::of(definition));
                    self.definition_storage(prefix, definition, visited);
                    visited.pop();
                }
            }
        }

        fn definition_storage(
            &mut self,
            prefix: &mut Vec<CheckedStructuralPredicatePathSegment>,
            definition: &DataDefinition,
            visited: &mut Vec<StorageDefinitionIdentity>,
        ) {
            self.definition_rows(prefix, definition);
            for (case, field) in owned_storage_fields(self.program, definition) {
                if let Some(case) = case {
                    let identity =
                        CheckedMemberIdentity::of_member(case.identity, case.name.clone());
                    self.case_guards
                        .push(CheckedBooleanExpression::StructuralCaseMembership {
                            subject: checked_trees::CheckedStructuralCaseSubject {
                                root: checked_trees::CheckedStructuralCaseRoot::Parameter {
                                    position: self.parameter_position,
                                },
                                path: prefix.clone(),
                            },
                            case: identity.clone(),
                        });
                    prefix.push(CheckedStructuralPredicatePathSegment::Case(identity));
                }
                prefix.push(CheckedStructuralPredicatePathSegment::Field(
                    CheckedMemberIdentity::of_member(field.identity, field.name.clone()),
                ));
                self.descend(prefix, field.type_reference, visited);
                prefix.pop();
                if case.is_some() {
                    prefix.pop();
                    self.case_guards.pop();
                }
            }
        }

        fn definition_rows(
            &mut self,
            prefix: &[CheckedStructuralPredicatePathSegment],
            definition: &DataDefinition,
        ) {
            let row = std::ptr::from_ref(definition);
            let emits = match self.emitting.iter().find(|(known, _)| *known == row) {
                Some((_, emits)) => *emits,
                None => {
                    let emits = definition_emits_rows(self.program, self.operators, definition);
                    self.emitting.push((row, emits));
                    emits
                }
            };
            if !emits {
                return;
            }
            for (case, facts) in definition_fact_spans(self.program, definition) {
                if let Some(case) = case {
                    self.case_guards
                        .push(CheckedBooleanExpression::StructuralCaseMembership {
                            subject: checked_trees::CheckedStructuralCaseSubject {
                                root: checked_trees::CheckedStructuralCaseRoot::Parameter {
                                    position: self.parameter_position,
                                },
                                path: prefix.to_vec(),
                            },
                            case: CheckedMemberIdentity::of_member(
                                case.identity,
                                case.name.clone(),
                            ),
                        });
                }
                self.requirement_rows(prefix, definition, case, facts);
                if case.is_some() {
                    self.case_guards.pop();
                }
            }
        }

        fn requirement_rows(
            &mut self,
            prefix: &[CheckedStructuralPredicatePathSegment],
            definition: &DataDefinition,
            selected_case: Option<&DataVariant>,
            facts: arena::HandleSpan<typed_trees::domain::ProofFact>,
        ) {
            let spelling = FactSpelling {
                program: self.program,
                operators: self.operators,
                parameter_position: self.parameter_position,
                prefix,
                definition,
                selected_case,
            };
            for fact in self.program.conjunctive_proof_facts(facts) {
                let Some(expression) = fact.as_expression() else {
                    continue;
                };
                if let Some(predicate) = spelling.boolean(*expression) {
                    self.push(predicate);
                    continue;
                }
                let Some(consequence) = collection_predicates::data_fact_element_consequences(
                    self.program,
                    definition,
                    *expression,
                ) else {
                    continue;
                };
                for field_consequence in consequence.fields {
                    let Some(field) =
                        find_data_field(self.program, definition, field_consequence.field, "")
                    else {
                        continue;
                    };
                    let Some(primitive) = field_consequence_primitive(self.program, field) else {
                        continue;
                    };
                    let mut path = prefix.to_vec();
                    path.push(CheckedStructuralPredicatePathSegment::Field(
                        CheckedMemberIdentity::of_member(field.identity, field.name.clone()),
                    ));
                    match field_consequence.elements {
                        collection_predicates::ConsequenceShape::Array(intervals) => {
                            for (index, interval) in intervals.into_iter().enumerate() {
                                let Ok(index) = u64::try_from(index) else {
                                    break;
                                };
                                let mut element = path.clone();
                                element
                                    .push(CheckedStructuralPredicatePathSegment::FixedIndex(index));
                                if let Some(predicate) =
                                    self.interval_row(element, primitive, interval)
                                {
                                    self.push(predicate);
                                }
                            }
                        }
                        collection_predicates::ConsequenceShape::Integer(interval) => {
                            if let Some(predicate) = self.interval_row(path, primitive, interval) {
                                self.push(predicate);
                            }
                        }
                    }
                }
            }
        }

        fn interval_row(
            &self,
            path: Vec<CheckedStructuralPredicatePathSegment>,
            primitive: PrimitiveType,
            interval: collection_predicates::IntegerSet,
        ) -> Option<CheckedBooleanExpression> {
            let (minimum, maximum) = interval.bounds()?;
            let (carrier_minimum, carrier_maximum) = carrier_bounds(primitive)?;
            let leaf = || CheckedScalarExpression::StructuralParameterField {
                observation: Default::default(),
                root: checked_trees::CheckedStorageRoot::Parameter {
                    index: self.parameter_position,
                },
                path: path.clone(),
                scalar_type: primitive.into(),
            };
            let literal = |value: i128| {
                crate::land_integer_value(&BigInt::from_i128(value), primitive)
                    .map(|literal| CheckedScalarExpression::IntegerLiteral { literal })
            };
            let lower = (minimum > carrier_minimum).then(|| {
                Some(CheckedBooleanExpression::IntegerComparison {
                    kind: CheckedIntegerComparisonKind::LessOrEqual,
                    left: Box::new(literal(minimum)?),
                    right: Box::new(leaf()),
                })
            });
            let upper = (maximum < carrier_maximum).then(|| {
                Some(CheckedBooleanExpression::IntegerComparison {
                    kind: CheckedIntegerComparisonKind::LessOrEqual,
                    left: Box::new(leaf()),
                    right: Box::new(literal(maximum)?),
                })
            });
            match (lower, upper) {
                (Some(lower), Some(upper)) => Some(CheckedBooleanExpression::And {
                    left: Box::new(lower?),
                    right: Box::new(upper?),
                }),
                (Some(bound), None) | (None, Some(bound)) => bound,
                (None, None) => None,
            }
        }

        fn push(&mut self, predicate: CheckedBooleanExpression) {
            let predicate = self
                .case_guards
                .iter()
                .rev()
                .fold(predicate, |predicate, guard| CheckedBooleanExpression::Or {
                    left: Box::new(CheckedBooleanExpression::Not(Box::new(guard.clone()))),
                    right: Box::new(predicate),
                });
            self.rows.push(DeclarationPredicateRow {
                parameter_position: self.parameter_position,
                restored_on_return: self.restored_on_return,
                predicate,
            });
        }
    }

    fn field_consequence_primitive(
        program: &dyn crate::CheckingInput,
        field: &DataField,
    ) -> Option<PrimitiveType> {
        let carrier = program
            .type_reference_table
            .unconstrained_type_reference(field.type_reference);
        let leaf = match program.type_reference_table.type_reference(carrier) {
            TypeReferenceNode::FixedArray { element_type, .. } => *element_type,
            _ => carrier,
        };
        let primitive = program.primitive_type_reference(leaf)?;
        carrier_bounds(primitive).map(|_| primitive)
    }

    fn carrier_bounds(primitive: PrimitiveType) -> Option<(i128, i128)> {
        Some(match primitive {
            PrimitiveType::I8 => (i128::from(i8::MIN), i128::from(i8::MAX)),
            PrimitiveType::I16 => (i128::from(i16::MIN), i128::from(i16::MAX)),
            PrimitiveType::I32 => (i128::from(i32::MIN), i128::from(i32::MAX)),
            PrimitiveType::I64 => (i128::from(i64::MIN), i128::from(i64::MAX)),
            PrimitiveType::U8 => (0, i128::from(u8::MAX)),
            PrimitiveType::U16 => (0, i128::from(u16::MAX)),
            PrimitiveType::U32 => (0, i128::from(u32::MAX)),
            PrimitiveType::U64 => (0, i128::from(u64::MAX)),
            _ => return None,
        })
    }

    fn definition_emits_rows(
        program: &dyn crate::CheckingInput,
        operators: &CheckedOperatorFacts,
        definition: &DataDefinition,
    ) -> bool {
        definition_fact_spans(program, definition).any(|(case, facts)| {
            !facts.is_empty()
                && (case.is_some()
                    || !field_carriers::imply_declaration(program, operators, definition))
        })
    }

    fn owned_storage_fields<'program>(
        program: &'program dyn crate::CheckingInput,
        definition: &'program DataDefinition,
    ) -> impl Iterator<Item = (Option<&'program DataVariant>, &'program DataField)> {
        program
            .data_members(definition)
            .iter()
            .flat_map(move |member| {
                let (case, fields) = match member {
                    DataMember::Field(field) => (None, std::slice::from_ref(field)),
                    DataMember::Variant(case) => (Some(case), program.data_payload_fields(case)),
                };
                fields.iter().map(move |field| (case, field))
            })
            .filter(move |(_, field)| {
                field.symbol.is_valid()
                    && !field.relevance.is_erased()
                    && !matches!(
                        program.type_reference_table.type_reference(
                            program
                                .type_reference_table
                                .unconstrained_type_reference(field.type_reference)
                        ),
                        TypeReferenceNode::Reference { .. }
                    )
            })
    }

    fn storage_carries_rows(
        program: &dyn crate::CheckingInput,
        operators: &CheckedOperatorFacts,
        type_reference: TypeReferenceHandle,
        visited: &mut Vec<StorageDefinitionIdentity>,
    ) -> bool {
        let carrier = program
            .type_reference_table
            .unconstrained_type_reference(type_reference);
        match program.type_reference_table.type_reference(carrier) {
            TypeReferenceNode::FixedArray {
                length: FixedArrayLength::Literal(0),
                ..
            } => false,
            TypeReferenceNode::FixedArray { element_type, .. } => {
                storage_carries_rows(program, operators, *element_type, visited)
            }
            _ => {
                let Some(definition) = owned_data_definition(program, carrier) else {
                    return false;
                };
                if visited.contains(&StorageDefinitionIdentity::of(definition)) {
                    return false;
                }
                if definition_emits_rows(program, operators, definition) {
                    return true;
                }
                visited.push(StorageDefinitionIdentity::of(definition));
                let carries = owned_storage_fields(program, definition).any(|(_, field)| {
                    storage_carries_rows(program, operators, field.type_reference, visited)
                });
                visited.pop();
                carries
            }
        }
    }

    fn owned_data_definition(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<&DataDefinition> {
        let carrier = program
            .type_reference_table
            .unconstrained_type_reference(type_reference);
        match program.type_reference_table.type_reference(carrier) {
            TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
                program.closed_data_definition(carrier)
            }
            _ => None,
        }
    }

    fn find_data_field<'program>(
        program: &'program dyn crate::CheckingInput,
        definition: &DataDefinition,
        symbol: SymbolHandle,
        name: &str,
    ) -> Option<&'program DataField> {
        let symbol = if symbol.is_valid()
            && let Some(application) = definition.generic_instance
        {
            program
                .data_constructor_field(application, definition.symbol, symbol)?
                .symbol
        } else {
            symbol
        };
        program.data_members(definition).iter().find_map(|member| {
            let field = member.as_field()?;
            if field.relevance.is_erased() {
                return None;
            }
            if symbol.is_valid() {
                (field.symbol == symbol).then_some(field)
            } else {
                (field.name.as_str() == name).then_some(field)
            }
        })
    }

    pub(crate) struct FactSpelling<'a> {
        program: &'a dyn crate::CheckingInput,
        operators: &'a CheckedOperatorFacts,
        parameter_position: u32,
        prefix: &'a [CheckedStructuralPredicatePathSegment],
        definition: &'a DataDefinition,
        selected_case: Option<&'a DataVariant>,
    }

    impl FactSpelling<'_> {
        fn selects_declared_operator(&self, expression: ExpressionHandle) -> bool {
            let mut selected = false;
            self.operators
                .for_each_expression_use(expression, |_, operator| {
                    selected |= operator.status
                        != checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback;
                });
            selected
        }

        fn boolean(&self, expression: ExpressionHandle) -> Option<CheckedBooleanExpression> {
            if self.selects_declared_operator(expression) {
                return None;
            }
            if let Some(consequence) = collection_predicates::exact_predicate_ordering(
                self.program,
                self.definition.symbol,
                expression,
            ) {
                return self.exact_ordering(consequence);
            }
            match expression_table(self.program).expression(expression) {
                ExpressionNode::Boolean(value) => Some(CheckedBooleanExpression::Constant(*value)),
                ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                    Some(CheckedBooleanExpression::Not(Box::new(
                        self.boolean(unary.operand)?,
                    )))
                }
                ExpressionNode::Binary(binary) => match binary.operator {
                    BinaryOperator::CaseMembership | BinaryOperator::Equal
                        if crate::has_exact_data_case_membership_meaning(
                            self.program,
                            self.definition,
                            expression,
                            binary,
                        ) =>
                    {
                        let (path, reference) = self.field_path(binary.left)?;
                        let selected = expression_table(self.program)
                            .name_path_node(binary.right)?
                            .symbol;
                        let case = self
                            .program
                            .closed_data_constructor_case(reference, selected)?;
                        Some(CheckedBooleanExpression::StructuralCaseMembership {
                            subject: checked_trees::CheckedStructuralCaseSubject {
                                root: checked_trees::CheckedStructuralCaseRoot::Parameter {
                                    position: self.parameter_position,
                                },
                                path,
                            },
                            case: CheckedMemberIdentity::of_member(
                                case.identity,
                                case.name.clone(),
                            ),
                        })
                    }
                    BinaryOperator::And => Some(CheckedBooleanExpression::And {
                        left: Box::new(self.boolean(binary.left)?),
                        right: Box::new(self.boolean(binary.right)?),
                    }),
                    BinaryOperator::Or => Some(CheckedBooleanExpression::Or {
                        left: Box::new(self.boolean(binary.left)?),
                        right: Box::new(self.boolean(binary.right)?),
                    }),
                    BinaryOperator::Less => {
                        self.comparison(expression, Order::Less, binary.left, binary.right)
                    }
                    BinaryOperator::LessOrEqual => {
                        self.comparison(expression, Order::LessOrEqual, binary.left, binary.right)
                    }
                    BinaryOperator::Greater => {
                        self.comparison(expression, Order::Less, binary.right, binary.left)
                    }
                    BinaryOperator::GreaterOrEqual => {
                        self.comparison(expression, Order::LessOrEqual, binary.right, binary.left)
                    }
                    BinaryOperator::Equal | BinaryOperator::NotEqual => {
                        let negated = binary.operator == BinaryOperator::NotEqual;
                        if let Some(comparison) = self.comparison(
                            expression,
                            Order::Equal { negated },
                            binary.left,
                            binary.right,
                        ) {
                            return Some(comparison);
                        }
                        let equal = CheckedBooleanExpression::Equal {
                            left: Box::new(self.boolean(binary.left)?),
                            right: Box::new(self.boolean(binary.right)?),
                        };
                        Some(if negated {
                            CheckedBooleanExpression::Not(Box::new(equal))
                        } else {
                            equal
                        })
                    }
                    _ => None,
                },
                _ => {
                    let (path, leaf_type) = self.field_path(expression)?;
                    (self.program.primitive_type_reference(leaf_type) == Some(PrimitiveType::Bool))
                        .then_some(CheckedBooleanExpression::StructuralParameterField {
                            observation: Default::default(),
                            parameter_position: self.parameter_position,
                            path,
                        })
                }
            }
        }

        fn exact_ordering(
            &self,
            consequence: collection_predicates::PredicateOrderingConsequence,
        ) -> Option<CheckedBooleanExpression> {
            let operand = |operand| match operand {
                PredicateIntegerOperand::Argument(subject) if subject.projection.is_empty() => {
                    self.scalar(subject.expression)
                }
                PredicateIntegerOperand::Integer(value) => {
                    let literal = numerics::literals::IntegerLiteral::from_parts(
                        value < 0,
                        numerics::literals::IntegerRadix::Decimal,
                        &value.unsigned_abs().to_string(),
                    )
                    .ok()?;
                    Some((CheckedScalarExpression::IntegerLiteral { literal }, None))
                }
                _ => None,
            };
            let (left, left_type) = operand(consequence.left)?;
            let (right, right_type) = operand(consequence.right)?;
            let primitive = match (left_type, right_type) {
                (Some(left), Some(right)) if left == right => left,
                (Some(primitive), None) | (None, Some(primitive)) => primitive,
                _ => return None,
            };
            carrier_bounds(primitive)?;
            let (left, right, kind, negated) = match consequence.operator {
                BinaryOperator::Equal => (left, right, CheckedIntegerComparisonKind::Equal, false),
                BinaryOperator::NotEqual => {
                    (left, right, CheckedIntegerComparisonKind::Equal, true)
                }
                BinaryOperator::Less => {
                    (left, right, CheckedIntegerComparisonKind::LessThan, false)
                }
                BinaryOperator::LessOrEqual => (
                    left,
                    right,
                    CheckedIntegerComparisonKind::LessOrEqual,
                    false,
                ),
                BinaryOperator::Greater => {
                    (right, left, CheckedIntegerComparisonKind::LessThan, false)
                }
                BinaryOperator::GreaterOrEqual => (
                    right,
                    left,
                    CheckedIntegerComparisonKind::LessOrEqual,
                    false,
                ),
                _ => return None,
            };
            let comparison = CheckedBooleanExpression::IntegerComparison {
                kind,
                left: Box::new(land_integer_operand(left, primitive)?),
                right: Box::new(land_integer_operand(right, primitive)?),
            };
            Some(if negated {
                CheckedBooleanExpression::Not(Box::new(comparison))
            } else {
                comparison
            })
        }

        fn comparison(
            &self,
            authored: ExpressionHandle,
            order: Order,
            left: ExpressionHandle,
            right: ExpressionHandle,
        ) -> Option<CheckedBooleanExpression> {
            let (left_authored, right_authored) = (left, right);
            let (left, left_type) = self.scalar(left)?;
            let (right, right_type) = self.scalar(right)?;
            let operand_type = match (left_type, right_type) {
                (Some(left_type), Some(right_type)) if left_type == right_type => left_type,
                (Some(operand_type), None) | (None, Some(operand_type)) => operand_type,
                _ => return None,
            };
            if matches!(operand_type, PrimitiveType::F32 | PrimitiveType::F64) {
                let binary = expression_table(self.program).binary_node(authored)?;
                let operand_types = [
                    self.operand_type_reference(binary.left),
                    self.operand_type_reference(binary.right),
                ];
                if !crate::has_builtin_or_sealed_float_comparison_meaning(
                    self.program,
                    self.definition.symbol,
                    authored,
                    binary.operator,
                    &operand_types,
                ) {
                    return None;
                }
                let kind = match order {
                    Order::Less => CheckedIeeeFloatComparisonKind::LessThan,
                    Order::LessOrEqual => CheckedIeeeFloatComparisonKind::LessOrEqual,
                    Order::Equal { negated: false } => CheckedIeeeFloatComparisonKind::Equal,
                    Order::Equal { negated: true } => CheckedIeeeFloatComparisonKind::NotEqual,
                };
                return Some(CheckedBooleanExpression::ScalarIeeeFloatComparison {
                    kind,
                    left: Box::new(self.land_float_operand(left_authored, left, operand_type)?),
                    right: Box::new(self.land_float_operand(
                        right_authored,
                        right,
                        operand_type,
                    )?),
                });
            }
            carrier_bounds(operand_type)?;
            let (kind, negated) = match order {
                Order::Less => (CheckedIntegerComparisonKind::LessThan, false),
                Order::LessOrEqual => (CheckedIntegerComparisonKind::LessOrEqual, false),
                Order::Equal { negated } => (CheckedIntegerComparisonKind::Equal, negated),
            };
            let comparison = CheckedBooleanExpression::IntegerComparison {
                kind,
                left: Box::new(land_integer_operand(left, operand_type)?),
                right: Box::new(land_integer_operand(right, operand_type)?),
            };
            Some(if negated {
                CheckedBooleanExpression::Not(Box::new(comparison))
            } else {
                comparison
            })
        }

        fn land_float_operand(
            &self,
            authored: ExpressionHandle,
            operand: CheckedScalarExpression,
            primitive: PrimitiveType,
        ) -> Option<CheckedScalarExpression> {
            let format = match primitive {
                PrimitiveType::F32 => FloatFormat::F32,
                PrimitiveType::F64 => FloatFormat::F64,
                _ => return None,
            };
            match (
                &operand,
                expression_table(self.program).expression(authored),
            ) {
                (
                    CheckedScalarExpression::IeeeFloatLiteral { .. },
                    ExpressionNode::Float(literal),
                ) => {
                    if literal.landing().is_some_and(|landing| landing != format) {
                        return None;
                    }
                    Some(CheckedScalarExpression::IeeeFloatLiteral {
                        value: typed_trees::expression::ieee_float_value(literal, format),
                    })
                }
                (CheckedScalarExpression::StructuralParameterField { scalar_type, .. }, _) => {
                    (scalar_type.primitive() == Some(primitive)).then_some(operand)
                }
                _ => None,
            }
        }

        fn operand_type_reference(
            &self,
            expression: ExpressionHandle,
        ) -> Option<TypeReferenceHandle> {
            self.field_path(expression)
                .map(|(_, type_reference)| type_reference)
        }

        fn scalar(
            &self,
            expression: ExpressionHandle,
        ) -> Option<(CheckedScalarExpression, Option<PrimitiveType>)> {
            if self.selects_declared_operator(expression) {
                return None;
            }
            match expression_table(self.program).expression(expression) {
                ExpressionNode::Name(path)
                    if path.symbol.is_valid()
                        && self.program.symbols.get(path.symbol).kind
                            == symbols::SymbolKind::TypeParameter =>
                {
                    let (primitive, value) =
                        crate::proof_contracts::default_domains::applied_const_integer(
                            self.program,
                            self.definition,
                            path.symbol,
                        )?;
                    let literal = numerics::literals::IntegerLiteral::from_parts(
                        value < 0,
                        numerics::literals::IntegerRadix::Decimal,
                        &value.unsigned_abs().to_string(),
                    )
                    .ok()?;
                    let landed = crate::land_integer_value(&BigInt::from_i128(value), primitive)?;
                    Some((
                        CheckedScalarExpression::IntegerLiteral {
                            literal: literal.with_landing(landed.landing()?),
                        },
                        Some(primitive),
                    ))
                }
                ExpressionNode::Integer(literal) => {
                    let primitive = literal.landing().and_then(|landing| {
                        use numerics::literals::LandedIntegerType;
                        match landing.landed_type {
                            LandedIntegerType::I8 => Some(PrimitiveType::I8),
                            LandedIntegerType::I16 => Some(PrimitiveType::I16),
                            LandedIntegerType::I32 => Some(PrimitiveType::I32),
                            LandedIntegerType::I64 => Some(PrimitiveType::I64),
                            LandedIntegerType::U8 => Some(PrimitiveType::U8),
                            LandedIntegerType::U16 => Some(PrimitiveType::U16),
                            LandedIntegerType::U32 => Some(PrimitiveType::U32),
                            LandedIntegerType::U64 => Some(PrimitiveType::U64),
                            LandedIntegerType::Addr => None,
                        }
                    });
                    Some((
                        CheckedScalarExpression::IntegerLiteral {
                            literal: literal.clone(),
                        },
                        primitive,
                    ))
                }
                ExpressionNode::Float(literal) => {
                    let format = literal.landing().unwrap_or(FloatFormat::F64);
                    let primitive = literal.landing().map(|format| match format {
                        FloatFormat::F32 => PrimitiveType::F32,
                        FloatFormat::F64 => PrimitiveType::F64,
                    });
                    Some((
                        CheckedScalarExpression::IeeeFloatLiteral {
                            value: typed_trees::expression::ieee_float_value(literal, format),
                        },
                        primitive,
                    ))
                }
                _ => {
                    if let Some(closed) = self
                        .program
                        .closed_integer_value_in(expression, self.definition.symbol)
                    {
                        let spelling = closed.value.to_string();
                        let literal = numerics::literals::IntegerLiteral::from_parts(
                            spelling.starts_with('-'),
                            numerics::literals::IntegerRadix::Decimal,
                            spelling.trim_start_matches('-'),
                        )
                        .ok()?;
                        let literal = if let Some(primitive) = closed.primitive {
                            if !primitive.is_fixed_integer() {
                                return None;
                            }
                            let landed = crate::land_integer_value(&closed.value, primitive)?;
                            literal.with_landing(landed.landing()?)
                        } else {
                            literal
                        };
                        return Some((
                            CheckedScalarExpression::IntegerLiteral { literal },
                            closed.primitive,
                        ));
                    }
                    let (path, leaf_type) = self.field_path(expression)?;
                    let primitive = self.program.primitive_type_reference(leaf_type)?;
                    if primitive == PrimitiveType::Bool || primitive == PrimitiveType::Addr {
                        return None;
                    }
                    Some((
                        CheckedScalarExpression::StructuralParameterField {
                            observation: Default::default(),
                            root: checked_trees::CheckedStorageRoot::Parameter {
                                index: self.parameter_position,
                            },
                            path,
                            scalar_type: primitive.into(),
                        },
                        Some(primitive),
                    ))
                }
            }
        }

        fn field_path(
            &self,
            expression: ExpressionHandle,
        ) -> Option<(
            Vec<CheckedStructuralPredicatePathSegment>,
            TypeReferenceHandle,
        )> {
            match expression_table(self.program).expression(expression) {
                ExpressionNode::Name(name) => {
                    if self.definition.predicate_subject_symbol.is_valid()
                        && name.symbol == self.definition.predicate_subject_symbol
                        && name.head_symbol == name.symbol
                        && expression_table(self.program)
                            .name_path_member_symbols(name.member_symbols)
                            == [name.symbol]
                    {
                        return Some((
                            self.prefix.to_vec(),
                            self.definition.predicate_subject_type,
                        ));
                    }
                    let members = expression_table(self.program).name_path_members(name.members);
                    let member_symbols = expression_table(self.program)
                        .name_path_member_symbols(name.member_symbols);
                    let mut path = self.prefix.to_vec();
                    let mut owner = self.definition;
                    let mut leaf = None;
                    for (index, member) in members.iter().enumerate() {
                        let symbol = member_symbols
                            .get(index)
                            .copied()
                            .unwrap_or_else(SymbolHandle::invalid);
                        let field = if let Some(field) =
                            find_data_field(self.program, owner, symbol, member.as_str())
                        {
                            field
                        } else {
                            let case = self.selected_case.filter(|_| index == 0)?;
                            let symbol = if symbol.is_valid()
                                && let Some(application) = owner.generic_instance
                            {
                                self.program
                                    .data_constructor_field(application, owner.symbol, symbol)?
                                    .symbol
                            } else {
                                symbol
                            };
                            let field =
                                self.program
                                    .data_payload_fields(case)
                                    .iter()
                                    .find(|field| {
                                        !field.relevance.is_erased()
                                            && if symbol.is_valid() {
                                                field.symbol == symbol
                                            } else {
                                                field.name.as_str() == member.as_str()
                                            }
                                    })?;
                            path.push(CheckedStructuralPredicatePathSegment::Case(
                                CheckedMemberIdentity::of_member(case.identity, case.name.clone()),
                            ));
                            field
                        };
                        path.push(CheckedStructuralPredicatePathSegment::Field(
                            CheckedMemberIdentity::of_member(field.identity, field.name.clone()),
                        ));
                        if index + 1 == members.len() {
                            leaf = Some(field.type_reference);
                        } else {
                            owner = owned_data_definition(self.program, field.type_reference)?;
                        }
                    }
                    Some((path, leaf?))
                }
                ExpressionNode::Member(member) => {
                    let (mut path, receiver_type) = self.field_path(member.receiver)?;
                    let owner = owned_data_definition(self.program, receiver_type)?;
                    let field = crate::value_custody::places::applied_data_member_field(
                        self.program,
                        receiver_type,
                        member,
                    )?;
                    if !field.symbol.is_valid() {
                        return None;
                    }
                    if let Some(case) = self
                        .program
                        .data_members(owner)
                        .iter()
                        .filter_map(|row| row.as_variant())
                        .find(|case| {
                            self.program
                                .data_payload_fields(case)
                                .iter()
                                .any(|row| row.symbol == field.symbol)
                        })
                    {
                        path.push(CheckedStructuralPredicatePathSegment::Case(
                            CheckedMemberIdentity::of_member(case.identity, case.name.clone()),
                        ));
                    }
                    path.push(CheckedStructuralPredicatePathSegment::Field(
                        CheckedMemberIdentity::of_member(field.identity, field.name.clone()),
                    ));
                    Some((path, field.type_reference))
                }
                ExpressionNode::Indexed(indexed) => {
                    if self.selects_declared_operator(expression) {
                        return None;
                    }
                    let (mut path, collection) = self.field_path(indexed.collection)?;
                    let collection = self
                        .program
                        .type_reference_table
                        .unconstrained_type_reference(collection);
                    let TypeReferenceNode::FixedArray { element_type, .. } =
                        self.program.type_reference_table.type_reference(collection)
                    else {
                        return None;
                    };
                    let index = self
                        .program
                        .expression_table
                        .constant_integer_value(indexed.index)
                        .and_then(|index| u64::try_from(index).ok())?;
                    path.push(CheckedStructuralPredicatePathSegment::FixedIndex(index));
                    Some((path, *element_type))
                }
                ExpressionNode::Borrow(borrow) => self.field_path(borrow.target),
                _ => None,
            }
        }
    }

    #[derive(Clone, Copy)]
    pub(crate) enum Order {
        Less,
        LessOrEqual,
        Equal { negated: bool },
    }

    fn land_integer_operand(
        expression: CheckedScalarExpression,
        primitive: PrimitiveType,
    ) -> Option<CheckedScalarExpression> {
        match expression {
            CheckedScalarExpression::IntegerLiteral { literal } if literal.landing().is_none() => {
                let value = literal
                    .value_i64()
                    .map(BigInt::from_i64)
                    .or_else(|| literal.value_u64().map(BigInt::from_u64))?;
                let landed = crate::land_integer_value(&value, primitive)?;
                Some(CheckedScalarExpression::IntegerLiteral {
                    literal: literal.with_landing(landed.landing()?),
                })
            }
            CheckedScalarExpression::IeeeFloatLiteral { .. } => None,
            other => Some(other),
        }
    }

    #[cfg(test)]
    pub(crate) mod tests {

        use super::*;
        use arena::HandleSpan;
        use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
        use typed_trees::{
            TypedTrees,
            domain::ProofFact,
            expression::{
                Expression, NamePath, TableBinaryExpression, TableMatchArm, TableMatchExpression,
            },
            name::Identifier,
            signature::StateParameter,
            state::State,
        };

        pub(crate) mod own_fact_reuse {
            use super::*;
            use arena::HandleSpan;
            use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
            use typed_trees::{
                TypedTrees,
                domain::{ProofFact, ProofMembershipFact},
                name::Identifier,
            };

            fn graph(
                edges: &[Vec<usize>],
                own_facts: &[bool],
                bad: Option<usize>,
            ) -> (TypedTrees, Vec<SymbolHandle>, Vec<TypeReferenceHandle>) {
                let names = ["Root", "D", "B", "Alias", "Leaf", "Tail", "Other", "Last"];
                let mut builder = SymbolTableBuilder::default();
                let mut symbols = Vec::new();
                let mut fields = Vec::new();
                for name in names.iter().take(edges.len()) {
                    let symbol = builder.insert_root(SymbolKind::Data, SymbolNameRef::Static(name));
                    fields.push(builder.insert_children(
                        symbol,
                        [
                            (SymbolKind::Field, SymbolNameRef::Static("left")),
                            (SymbolKind::Field, SymbolNameRef::Static("right")),
                        ],
                    ));
                    symbols.push(symbol);
                }
                let mut program = TypedTrees {
                    symbols: builder.finish(),
                    ..Default::default()
                };
                let references = symbols
                    .iter()
                    .zip(names)
                    .map(|(&symbol, name)| {
                        program
                            .type_reference_table
                            .insert(TypeReferenceNode::Named {
                                symbol,
                                name: Identifier::from(name),
                            })
                    })
                    .collect::<Vec<_>>();
                let requirement = program
                    .expression_table
                    .insert(ExpressionNode::Boolean(false));
                let facts = program
                    .proof_facts
                    .insert_many([ProofFact::Expression(requirement)]);
                let unsupported = program
                    .proof_facts
                    .insert_many([ProofFact::Membership(ProofMembershipFact::default())]);
                for (position, children) in edges.iter().enumerate() {
                    let members =
                        program
                            .data_members
                            .insert_many(children.iter().enumerate().map(|(i, &child)| {
                                DataMember::Field(DataField {
                                    symbol: fields[position].handle_at(i as u32),
                                    name: Identifier::from(if i == 0 { "left" } else { "right" }),
                                    type_reference: references[child],
                                    ..Default::default()
                                })
                            }));
                    program.push_data_definition(DataDefinition {
                        symbol: symbols[position],
                        name: Identifier::from(names[position]),
                        members,
                        requires_facts: if bad == Some(position) {
                            unsupported
                        } else if own_facts[position] {
                            facts
                        } else {
                            HandleSpan::empty()
                        },
                        ..Default::default()
                    });
                }
                (program, symbols, references)
            }

            fn compare(
                program: &TypedTrees,
                definition: &DataDefinition,
                expected: bool,
            ) -> (usize, usize) {
                let operators = CheckedOperatorFacts::default();
                let mut own_evaluations = 0;
                assert_eq!(
                    original_definition_storage(
                        program,
                        &operators,
                        definition,
                        &mut Vec::new(),
                        &mut own_evaluations,
                    ),
                    expected
                );
                let mut visited = Vec::new();
                let mut checked_own = Vec::new();
                assert_eq!(
                    complete_definition_storage(
                        program,
                        &operators,
                        definition,
                        &mut visited,
                        &mut checked_own,
                    ),
                    expected
                );
                assert!(visited.is_empty());
                assert_eq!(
                    declaration_facts_carried(program, &operators, definition),
                    expected
                );
                (own_evaluations, checked_own.len())
            }

            #[test]
            fn own_fact_reuse_preserves_original_storage_walks_and_alias_cycles() {
                let diamond = vec![vec![1, 2], vec![3, 3], vec![3, 3], vec![4, 4], vec![]];
                let (program, symbols, _) = graph(&diamond, &[true; 5], None);
                let root = program
                    .unique_data_definition_by_symbol(symbols[0])
                    .unwrap();
                let (old, once) = compare(&program, root, true);
                assert_eq!(old, 15);
                assert_eq!(once, 5);

                let (program, symbols, _) = graph(&[vec![1], vec![0]], &[true; 2], None);
                compare(
                    &program,
                    program
                        .unique_data_definition_by_symbol(symbols[0])
                        .unwrap(),
                    false,
                );
                let (mut program, symbols, references) =
                    graph(&[vec![1, 3], vec![2], vec![], vec![1]], &[true; 4], None);
                let mut alias = program
                    .unique_data_definition_by_symbol(symbols[3])
                    .unwrap()
                    .clone();
                alias.symbol = symbols[2];
                program.retain_data_application_definition(references[3], alias, true);
                assert!(program.data_definitions().iter().all(|definition| {
                    definition_own_facts_carried(
                        &program,
                        &CheckedOperatorFacts::default(),
                        definition,
                    )
                }));
                assert!(definition_own_facts_carried(
                    &program,
                    &CheckedOperatorFacts::default(),
                    program.data_application_definition(references[3]).unwrap(),
                ));
                compare(
                    &program,
                    program
                        .unique_data_definition_by_symbol(symbols[0])
                        .unwrap(),
                    false,
                );

                let mut alias = program
                    .data_application_definition(references[3])
                    .unwrap()
                    .clone();
                alias.symbol = arena::Handle::from_parts(
                    symbols[2].arena_index(),
                    symbols[2].generation() + 1,
                );
                program.retain_data_application_definition(references[3], alias, true);
                compare(
                    &program,
                    program
                        .unique_data_definition_by_symbol(symbols[0])
                        .unwrap(),
                    true,
                );

                let mut alias = program
                    .data_application_definition(references[3])
                    .unwrap()
                    .clone();
                alias.symbol = symbols[2];
                alias.members = HandleSpan::empty();
                alias.requires_facts = program
                    .proof_facts
                    .insert_many([ProofFact::Membership(ProofMembershipFact::default())]);
                program.retain_data_application_definition(references[3], alias, true);
                compare(
                    &program,
                    program
                        .unique_data_definition_by_symbol(symbols[0])
                        .unwrap(),
                    false,
                );

                let (program, symbols, _) =
                    graph(&[vec![1, 2], vec![], vec![]], &[true; 3], Some(1));
                let root = program
                    .unique_data_definition_by_symbol(symbols[0])
                    .unwrap();
                let operators = CheckedOperatorFacts::default();
                let mut checked = Vec::new();
                assert!(!complete_definition_storage(
                    &program,
                    &operators,
                    root,
                    &mut Vec::new(),
                    &mut checked
                ));
                assert_eq!(checked.len(), 1);
                assert!(std::ptr::eq(checked[0], root));
                compare(&program, root, false);

                for bad in [false, true] {
                    let (mut program, symbols, references) =
                        graph(&[vec![1], vec![]], &[true; 2], bad.then_some(1));
                    let mut root = program
                        .unique_data_definition_by_symbol(symbols[0])
                        .unwrap()
                        .clone();
                    let original_field = program.data_members(&root)[0].as_field().unwrap().clone();
                    for length in [
                        FixedArrayLength::Literal(0),
                        FixedArrayLength::Literal(2),
                        FixedArrayLength::ConstParameter {
                            symbol: symbols[0],
                            name: Identifier::from("N"),
                        },
                    ] {
                        let is_zero = matches!(length, FixedArrayLength::Literal(0));
                        let is_literal = matches!(length, FixedArrayLength::Literal(_));
                        let reference =
                            program
                                .type_reference_table
                                .insert(TypeReferenceNode::FixedArray {
                                    element_type: references[1],
                                    length,
                                });
                        root.members =
                            program
                                .data_members
                                .insert_many([DataMember::Field(DataField {
                                    type_reference: reference,
                                    ..original_field.clone()
                                })]);
                        compare(&program, &root, is_zero || (is_literal && !bad));
                    }
                    for access in [
                        ReferenceAccess::Shared,
                        ReferenceAccess::Mutable,
                        ReferenceAccess::WriteOnly,
                    ] {
                        let reference =
                            program
                                .type_reference_table
                                .insert(TypeReferenceNode::Reference {
                                    referee: references[1],
                                    access,
                                    lifetime: None,
                                });
                        root.members =
                            program
                                .data_members
                                .insert_many([DataMember::Field(DataField {
                                    type_reference: reference,
                                    ..original_field.clone()
                                })]);
                        compare(&program, &root, true);
                    }
                }
                let (mut program, symbols, references) =
                    graph(&[vec![1], vec![]], &[true, false], None);
                let mut root = program
                    .unique_data_definition_by_symbol(symbols[0])
                    .unwrap()
                    .clone();
                let reference =
                    program
                        .type_reference_table
                        .insert(TypeReferenceNode::FixedArray {
                            element_type: references[1],
                            length: FixedArrayLength::ConstParameter {
                                symbol: symbols[0],
                                name: Identifier::from("N"),
                            },
                        });
                let mut field = program.data_members(&root)[0].as_field().unwrap().clone();
                field.type_reference = reference;
                root.members = program.data_members.insert_many([DataMember::Field(field)]);
                compare(&program, &root, true);
            }

            fn original_definition_storage(
                program: &dyn crate::CheckingInput,
                operators: &CheckedOperatorFacts,
                definition: &DataDefinition,
                visited: &mut Vec<StorageDefinitionIdentity>,
                own_evaluations: &mut usize,
            ) -> bool {
                if visited.contains(&StorageDefinitionIdentity::of(definition))
                    || !{
                        *own_evaluations += usize::from(!definition.requires_facts.is_empty());
                        definition_own_facts_carried(program, operators, definition)
                    }
                {
                    return false;
                }
                visited.push(StorageDefinitionIdentity::of(definition));
                let complete = owned_storage_fields(program, definition).all(|(_, field)| {
                    original_owned_storage(
                        program,
                        operators,
                        field.type_reference,
                        visited,
                        own_evaluations,
                    )
                });
                visited.pop();
                complete
            }

            fn original_owned_storage(
                program: &dyn crate::CheckingInput,
                operators: &CheckedOperatorFacts,
                reference: TypeReferenceHandle,
                visited: &mut Vec<StorageDefinitionIdentity>,
                own_evaluations: &mut usize,
            ) -> bool {
                let carrier = program
                    .type_reference_table
                    .unconstrained_type_reference(reference);
                match program.type_reference_table.type_reference(carrier) {
                    TypeReferenceNode::FixedArray {
                        length: FixedArrayLength::Literal(0),
                        ..
                    } => true,
                    TypeReferenceNode::FixedArray {
                        element_type,
                        length: FixedArrayLength::Literal(_),
                    } => original_owned_storage(
                        program,
                        operators,
                        *element_type,
                        visited,
                        own_evaluations,
                    ),
                    TypeReferenceNode::FixedArray { element_type, .. } => {
                        !storage_carries_rows(program, operators, *element_type, &mut Vec::new())
                    }
                    TypeReferenceNode::Reference { .. } => true,
                    _ => owned_data_definition(program, carrier).is_none_or(|definition| {
                        original_definition_storage(
                            program,
                            operators,
                            definition,
                            visited,
                            own_evaluations,
                        )
                    }),
                }
            }
        }

        fn fixture() -> (TypedTrees, State, SymbolHandle, HandleSpan<ProofFact>) {
            let mut symbols = SymbolTableBuilder::default();
            let integer =
                symbols.insert_root(SymbolKind::BuiltinType, SymbolNameRef::Static("u64"));
            let pair = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Pair"));
            let fields = symbols.insert_children(
                pair,
                [
                    (SymbolKind::Field, SymbolNameRef::Static("lo")),
                    (SymbolKind::Field, SymbolNameRef::Static("hi")),
                ],
            );
            let envelope = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Envelope"));
            let members = symbols.insert_children(
                envelope,
                [
                    (SymbolKind::Field, SymbolNameRef::Static("marker")),
                    (SymbolKind::Variant, SymbolNameRef::Static("Empty")),
                    (SymbolKind::Variant, SymbolNameRef::Static("Point")),
                ],
            );
            let payload = symbols
                .insert_children(
                    members.handle_at(2),
                    [(SymbolKind::Field, SymbolNameRef::Static("pair"))],
                )
                .handle_at(0);
            let state_symbol =
                symbols.insert_root(SymbolKind::State, SymbolNameRef::Static("entry"));
            let parameter = symbols
                .insert_children(
                    state_symbol,
                    [(SymbolKind::Parameter, SymbolNameRef::Static("value"))],
                )
                .handle_at(0);
            let mut program = TypedTrees {
                symbols: symbols.finish(),
                ..Default::default()
            };
            let mut named = |symbol, name: &str| {
                program
                    .type_reference_table
                    .insert(TypeReferenceNode::Named {
                        symbol,
                        name: Identifier::from(name),
                    })
            };
            let integer_type = named(integer, "u64");
            let pair_type = named(pair, "Pair");
            let envelope_type = named(envelope, "Envelope");
            let mut pair_members = HandleSpan::empty();
            let mut operands = Vec::new();
            for (position, name) in ["lo", "hi"].into_iter().enumerate() {
                let symbol = fields.handle_at(position as u32);
                program.data_members.append_to_span(
                    &mut pair_members,
                    DataMember::Field(DataField {
                        identity: Some(20 + position as u64),
                        symbol,
                        name: Identifier::from(name),
                        type_reference: integer_type,
                        ..Default::default()
                    }),
                );
                operands.push(program.expression_table.insert_tree(&Expression::Name(
                    NamePath::resolved(vec![Identifier::from(name)], symbol, symbol),
                )));
            }
            let equality =
                program
                    .expression_table
                    .insert(ExpressionNode::Binary(TableBinaryExpression {
                        policy: numerics::arithmetic::ArithmeticDomain::Exact,
                        left: operands[0],
                        operator: BinaryOperator::Equal,
                        right: operands[1],
                    }));
            let mut pair_facts = HandleSpan::empty();
            program
                .proof_facts
                .append_to_span(&mut pair_facts, ProofFact::Expression(equality));
            program.push_data_definition(DataDefinition {
                symbol: pair,
                name: Identifier::from("Pair"),
                members: pair_members,
                requires_facts: pair_facts,
                ..Default::default()
            });
            let mut payload_fields = HandleSpan::empty();
            program.data_payload_fields.append_to_span(
                &mut payload_fields,
                DataField {
                    identity: Some(10),
                    symbol: payload,
                    name: Identifier::from("pair"),
                    type_reference: pair_type,
                    ..Default::default()
                },
            );
            let mut envelope_members = HandleSpan::empty();
            program.data_members.append_to_span(
                &mut envelope_members,
                DataMember::Field(DataField {
                    identity: Some(0),
                    symbol: members.handle_at(0),
                    name: Identifier::from("marker"),
                    type_reference: integer_type,
                    ..Default::default()
                }),
            );
            for (position, name) in [(1, "Empty"), (2, "Point")] {
                program.data_members.append_to_span(
                    &mut envelope_members,
                    DataMember::Variant(typed_trees::data::DataVariant {
                        identity: Some(position as u64),
                        symbol: members.handle_at(position),
                        name: Identifier::from(name),
                        payload: if position == 2 {
                            payload_fields
                        } else {
                            HandleSpan::empty()
                        },
                        ..Default::default()
                    }),
                );
            }
            program.push_data_definition(DataDefinition {
                symbol: envelope,
                name: Identifier::from("Envelope"),
                members: envelope_members,
                ..Default::default()
            });
            let borrowed = program
                .type_reference_table
                .insert(TypeReferenceNode::Reference {
                    referee: envelope_type,
                    access: ReferenceAccess::Mutable,
                    lifetime: None,
                });
            let mut parameters = HandleSpan::empty();
            program.state_parameters.append_to_span(
                &mut parameters,
                StateParameter {
                    symbol: parameter,
                    name: Identifier::from("value"),
                    type_reference: borrowed,
                    ..Default::default()
                },
            );
            (
                program,
                State {
                    symbol: state_symbol,
                    parameters,
                    ..Default::default()
                },
                envelope,
                pair_facts,
            )
        }

        #[test]
        fn declaration_roster_guards_owned_payload_coupling_before_observing_fields() {
            let (program, state, envelope, _) = fixture();
            let operators = CheckedOperatorFacts::default();
            let definition = program.unique_data_definition_by_symbol(envelope).unwrap();
            assert!(declaration_facts_carried(&program, &operators, definition));
            let rows = state_declaration_predicates(&program, &operators, &state);
            assert_eq!(
                rows.len(),
                1,
                "the Point payload's Pair equality must not disappear"
            );
            assert!(rows[0].restored_on_return);
            let identity = |ordinal, name| {
                checked_trees::CheckedMemberIdentity::of_member(
                    Some(ordinal),
                    Identifier::from(name),
                )
            };
            let leaf = |ordinal, name| CheckedScalarExpression::StructuralParameterField {
                observation: Default::default(),
                root: checked_trees::CheckedStorageRoot::Parameter { index: 0 },
                path: vec![
                    CheckedStructuralPredicatePathSegment::Case(identity(2, "Point")),
                    CheckedStructuralPredicatePathSegment::Field(identity(10, "pair")),
                    CheckedStructuralPredicatePathSegment::Field(identity(ordinal, name)),
                ],
                scalar_type: PrimitiveType::U64.into(),
            };
            assert_eq!(
                rows[0].predicate,
                CheckedBooleanExpression::Or {
                    left: Box::new(CheckedBooleanExpression::Not(Box::new(
                        CheckedBooleanExpression::StructuralCaseMembership {
                            subject: checked_trees::CheckedStructuralCaseSubject {
                                root: checked_trees::CheckedStructuralCaseRoot::Parameter {
                                    position: 0
                                },
                                path: Vec::new(),
                            },
                            case: identity(2, "Point"),
                        }
                    ))),
                    right: Box::new(CheckedBooleanExpression::IntegerComparison {
                        kind: CheckedIntegerComparisonKind::Equal,
                        left: Box::new(leaf(20, "lo")),
                        right: Box::new(leaf(21, "hi")),
                    }),
                }
            );
        }

        #[test]
        fn outer_storage_cannot_hide_an_unsupported_payload_predicate() {
            let (mut program, _, envelope, facts) = fixture();
            let condition = program.proof_facts.span_or_empty(facts)[0]
                .as_expression()
                .copied()
                .unwrap();
            let yes = program
                .expression_table
                .insert(ExpressionNode::Boolean(true));
            let no = program
                .expression_table
                .insert(ExpressionNode::Boolean(false));
            let capture = program.expression_table.insert_match_subject(condition);
            let arms = program.expression_table.insert_match_arms([
                TableMatchArm {
                    pattern: typed_trees::expression::MatchPattern::Value(yes),
                    value: yes,
                    ..Default::default()
                },
                TableMatchArm {
                    pattern: typed_trees::expression::MatchPattern::Value(no),
                    value: no,
                    ..Default::default()
                },
            ]);
            let predicate =
                program
                    .expression_table
                    .insert(ExpressionNode::Match(TableMatchExpression {
                        subject: condition,
                        capture,
                        arms,
                    }));
            program
                .expression_table
                .insert(ExpressionNode::SavedSubject(
                    typed_trees::expression::TableSavedSubject {
                        component: Default::default(),
                        capture,
                        pattern_head: yes,
                    },
                ));
            program.proof_facts.span_mut_or_empty(facts)[0] = ProofFact::Expression(predicate);
            let definition = program.unique_data_definition_by_symbol(envelope).unwrap();
            assert!(definition.requires_facts.is_empty());
            assert!(
                !declaration_facts_carried(&program, &CheckedOperatorFacts::default(), definition),
                "an unrelated Envelope marker store must not bypass the nested complete-domain fence"
            );
        }

        #[test]
        fn unresolved_nested_array_extents_cannot_hide_payload_requirements() {
            let (mut program, _, envelope, _) = fixture();
            let definition = program.unique_data_definition_by_symbol(envelope).unwrap();
            let payload = program
                .data_members(definition)
                .iter()
                .filter_map(|member| member.as_variant())
                .find(|case| !case.payload.is_empty())
                .unwrap()
                .payload;
            let pair_type = program.data_payload_fields.span_or_empty(payload)[0].type_reference;
            let extent = program.expression_table.insert(ExpressionNode::Integer(
                numerics::literals::IntegerLiteral::from_value(2),
            ));
            let extent = program
                .type_reference_table
                .insert(TypeReferenceNode::ConstExpression(extent));
            let inner = program
                .type_reference_table
                .insert(TypeReferenceNode::FixedArray {
                    element_type: pair_type,
                    length: FixedArrayLength::Expression(extent),
                });
            let outer = program
                .type_reference_table
                .insert(TypeReferenceNode::FixedArray {
                    element_type: inner,
                    length: FixedArrayLength::Expression(extent),
                });
            program.data_payload_fields.span_mut_or_empty(payload)[0].type_reference = outer;
            let definition = program.unique_data_definition_by_symbol(envelope).unwrap();
            assert!(!declaration_facts_carried(
                &program,
                &CheckedOperatorFacts::default(),
                definition
            ));
        }

        #[test]
        fn carrier_implied_case_roster_does_not_expand_array_elements() {
            let (mut program, state, envelope, pair_facts) = fixture();
            let root_span = program.roots.data_definitions;
            for definition in program.tables.data_definitions.span_mut_or_empty(root_span) {
                if definition.requires_facts == pair_facts {
                    definition.requires_facts = HandleSpan::empty();
                }
            }
            let parameter = program.state_parameters(&state)[0].clone();
            let TypeReferenceNode::Reference { referee, .. } = *program
                .type_reference_table
                .type_reference(parameter.type_reference)
            else {
                panic!("fixture parameter must be a borrow")
            };
            let subject =
                program
                    .expression_table
                    .insert_tree(&Expression::Name(NamePath::resolved(
                        vec![Identifier::from("self")],
                        parameter.symbol,
                        parameter.symbol,
                    )));
            let cases: Vec<_> = program
                .data_members(program.unique_data_definition_by_symbol(envelope).unwrap())
                .iter()
                .filter_map(|member| member.as_variant())
                .map(|case| (case.symbol, case.name.clone()))
                .collect();
            let mut membership = Vec::new();
            for (symbol, name) in cases {
                let selected =
                    program
                        .expression_table
                        .insert_tree(&Expression::Name(NamePath::resolved(
                            vec![name],
                            symbol,
                            symbol,
                        )));
                membership.push(program.expression_table.insert(ExpressionNode::Binary(
                    TableBinaryExpression {
                        policy: numerics::arithmetic::ArithmeticDomain::Exact,
                        left: subject,
                        operator: BinaryOperator::CaseMembership,
                        right: selected,
                    },
                )));
            }
            let predicate =
                program
                    .expression_table
                    .insert(ExpressionNode::Binary(TableBinaryExpression {
                        policy: numerics::arithmetic::ArithmeticDomain::Exact,
                        left: membership[0],
                        operator: BinaryOperator::Or,
                        right: membership[1],
                    }));
            let mut facts = HandleSpan::empty();
            program
                .proof_facts
                .append_to_span(&mut facts, ProofFact::Expression(predicate));
            for definition in program.tables.data_definitions.span_mut_or_empty(root_span) {
                if definition.symbol == envelope {
                    definition.predicate_subject_symbol = parameter.symbol;
                    definition.predicate_subject_type = referee;
                    definition.requires_facts = facts;
                }
            }
            let array = program
                .type_reference_table
                .insert(TypeReferenceNode::FixedArray {
                    element_type: referee,
                    length: FixedArrayLength::Literal(32),
                });
            let borrowed = program
                .type_reference_table
                .insert(TypeReferenceNode::Reference {
                    referee: array,
                    access: ReferenceAccess::Mutable,
                    lifetime: None,
                });
            program.state_parameters.span_mut_or_empty(state.parameters)[0].type_reference =
                borrowed;
            let operators = CheckedOperatorFacts::default();
            assert!(
                state_declaration_predicates(&program, &operators, &state).is_empty(),
                "case validity belongs to the element carrier, not 32 copied predicate trees"
            );

            program.proof_facts.span_mut_or_empty(facts)[0] = ProofFact::Expression(membership[0]);
            assert_eq!(
                state_declaration_predicates(&program, &operators, &state).len(),
                32
            );
        }
    }
}
#[cfg(test)]
pub(crate) mod state_selection_tests {
    use super::semantic_caller_state;
    use checked_trees::{FlowControlFacts, FlowStateFact};
    use symbols::SymbolHandle;

    fn original(
        control: &FlowControlFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
    ) -> Result<&FlowStateFact, &'static str> {
        let mut rows = control
            .states
            .iter()
            .map(|(_, row)| row)
            .filter(|row| row.machine_symbol == machine && row.state_symbol == state);
        let row = rows.next().ok_or("structural call has no semantic state")?;
        if rows.next().is_some() {
            return Err("structural call has ambiguous semantic states");
        }
        Ok(row)
    }

    #[test]
    fn caller_state_selection_preserves_exact_rows_errors_and_replacement() {
        let machine = SymbolHandle::from_parts(7, 1);
        let next_machine = SymbolHandle::from_parts(7, 2);
        let state = SymbolHandle::from_parts(9, 1);
        let next_state = SymbolHandle::from_parts(9, 2);
        let check = |facts: &FlowControlFacts| {
            for (owner, symbol) in [
                (machine, state),
                (next_machine, state),
                (machine, next_state),
                (SymbolHandle::invalid(), SymbolHandle::invalid()),
            ] {
                assert_eq!(
                    semantic_caller_state(facts, owner, symbol).map(std::ptr::from_ref),
                    original(facts, owner, symbol).map(std::ptr::from_ref),
                );
            }
        };
        let row = |owner, symbol| FlowStateFact {
            machine_symbol: owner,
            state_symbol: symbol,
            ..Default::default()
        };
        let mut facts = FlowControlFacts::default();
        check(&facts);
        assert_eq!(
            semantic_caller_state(&facts, machine, state).unwrap_err(),
            "structural call has no semantic state"
        );
        let first = facts.states.insert(row(machine, state));
        facts.states.insert(row(next_machine, state));
        facts.states.insert(row(machine, next_state));
        facts
            .states
            .insert(row(SymbolHandle::invalid(), SymbolHandle::invalid()));
        check(&facts);
        assert!(std::ptr::eq(
            semantic_caller_state(&facts, machine, state).unwrap(),
            facts.states.get(first)
        ));
        facts.states.insert(row(machine, state));
        check(&facts);
        assert_eq!(
            semantic_caller_state(&facts, machine, state).unwrap_err(),
            "structural call has ambiguous semantic states"
        );
        let snapshot = facts.clone();
        facts.states.get_mut(first).machine_symbol = next_machine;
        check(&facts);
        check(&snapshot);

        let mut old = arena::PagedArenaSet::default();
        old.insert(row(machine, state));
        *facts.states = old;
        check(&facts);
        let revision = facts.states.revision();
        let mut replacement = arena::PagedArenaSet::default();
        replacement.insert(row(next_machine, next_state));
        assert_eq!(replacement.revision(), revision);
        *facts.states = replacement;
        check(&facts);
        assert_eq!(
            semantic_caller_state(&facts, machine, state).unwrap_err(),
            "structural call has no semantic state"
        );
        assert!(semantic_caller_state(&facts, next_machine, next_state).is_ok());
    }
}

pub use declaration_predicates::{
    DeclarationPredicateRow, declaration_facts_carried, machine_declaration_predicates,
    state_declaration_predicates,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateContractRow {
    Expression(typed_trees::expression::ExpressionHandle),
    SourceFact(arena::Handle<typed_trees::domain::ProofFact>),
    Domain {
        parameter: SymbolHandle,
        constraint: typed_trees::types::DomainConstraint,
    },
    Finite {
        parameter: SymbolHandle,
    },
}

pub fn restates_structural_parameter_qualification(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    fact: &typed_trees::domain::ProofFact,
) -> bool {
    let Some(membership) = fact.as_membership() else {
        return false;
    };
    let Some(symbol) =
        crate::queries::expression_traversal::single_segment_name_symbol(program, membership.value)
    else {
        return false;
    };
    let Some(parameter) = program.state_parameter_by_symbol(state, symbol) else {
        return false;
    };
    if program
        .primitive_type_reference(parameter.type_reference)
        .is_some()
    {
        return false;
    }
    let reference = program
        .type_reference_table
        .dereferenced_type_reference(parameter.type_reference);
    let Some(domain) = program.domain_definition(membership.domain_symbol) else {
        return false;
    };
    structural_result_qualifications(program, reference)
        .is_ok_and(|domains| domains.contains(&domain.semantic_id))
}

pub fn structural_state_contract_predicates(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
) -> Option<Vec<StateContractRow>> {
    let mut expected = Vec::new();
    let mut predicates = Vec::new();
    for parameter in program.state_parameters(state) {
        if finite_arrival_parameter(program, state, parameter) {
            predicates.push(StateContractRow::Finite {
                parameter: parameter.symbol,
            });
        }
        if program
            .primitive_type_reference(parameter.type_reference)
            .is_some()
        {
            expected.extend(
                crate::scalar_predicate_domains(program, parameter.type_reference)
                    .into_iter()
                    .chain(crate::scalar_type_tags(program, parameter.type_reference))
                    .chain(crate::scalar_type_routed_domains(
                        program,
                        parameter.type_reference,
                    ))
                    .map(|(_, domain)| (parameter.symbol, domain.0)),
            );
            continue;
        }
        let reference = program
            .type_reference_table
            .dereferenced_type_reference(parameter.type_reference);
        let Ok(domains) = structural_result_qualifications(program, reference) else {
            return None;
        };
        expected.extend(
            domains
                .into_iter()
                .map(|domain| (parameter.symbol, domain.0)),
        );
    }
    let mut actual = Vec::new();
    for contract in program.state_contracts(state) {
        if contract.kind != typed_trees::signature::SignatureContractKind::Requires {
            return None;
        }
        let facts = program.proof_facts.span(contract.facts)?;
        if facts.is_empty() {
            return None;
        }
        for (fact_handle, fact) in contract.facts.handles().zip(facts) {
            match fact {
                typed_trees::domain::ProofFact::Membership(membership) => {
                    let path = expression_table(program).name_path_node(membership.value)?;
                    let symbol = crate::queries::expression_traversal::single_segment_name_symbol(
                        program,
                        membership.value,
                    )?;
                    program.domain_definition(membership.domain_symbol)?;
                    let parameter = program.state_parameter_by_symbol(state, path.symbol);
                    let tag = parameter.is_some_and(|parameter| {
                        program
                            .primitive_type_reference(parameter.type_reference)
                            .is_some()
                            && crate::scalar_type_tags(program, parameter.type_reference)
                                .contains(&(membership.domain_symbol, membership.semantic_domain))
                    });
                    actual.push((symbol, membership.semantic_domain.0));
                    if let Some(parameter) = parameter
                        && program
                            .primitive_type_reference(parameter.type_reference)
                            .is_some()
                        && !tag
                    {
                        let constraint =
                            scalar_declared_domain(program, parameter.type_reference, membership)?;
                        predicates.push(StateContractRow::Domain {
                            parameter: parameter.symbol,
                            constraint,
                        });
                    }
                }
                typed_trees::domain::ProofFact::Expression(expression) => {
                    predicates.push(StateContractRow::Expression(*expression));
                }
                typed_trees::domain::ProofFact::Proposition(_) => {
                    predicates.push(StateContractRow::SourceFact(fact_handle));
                }
            }
        }
    }
    expected.sort_by_key(|(symbol, domain)| (symbol.arena_index(), symbol.generation(), *domain));
    actual.sort_by_key(|(symbol, domain)| (symbol.arena_index(), symbol.generation(), *domain));
    (actual == expected).then_some(predicates)
}

pub fn finite_arrival_parameter(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    parameter: &typed_trees::signature::StateParameter,
) -> bool {
    use typed_trees::types::PrimitiveType;
    if parameter.is_const
        || parameter.relevance.is_erased()
        || !matches!(
            program.primitive_type_reference(parameter.type_reference),
            Some(PrimitiveType::F32 | PrimitiveType::F64)
        )
        || !program.machines().iter().any(|machine| {
            program
                .machine_states(machine)
                .iter()
                .skip(1)
                .any(|internal| internal.symbol == state.symbol)
        })
    {
        return false;
    }
    let mut reference = parameter.type_reference;
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(reference)
    {
        if program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .any(|constraint| {
                matches!(
                    constraint,
                    TypeConstraintNode::Domain(domain)
                        if domain.subject
                            == DomainConstraintSubject::Value(
                                language_semantics::value_domain::ValueDomain::Finite,
                            )
                )
            })
        {
            return true;
        }
        reference = *base_type;
    }
    false
}

pub fn state_finite_predicate(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    parameter: SymbolHandle,
) -> Option<checked_trees::CheckedBooleanExpression> {
    use checked_trees::CheckedScalarExpression;
    let parameters = program.state_parameters(state);
    let index = parameters
        .iter()
        .position(|candidate| candidate.symbol == parameter)?;
    if !finite_arrival_parameter(program, state, &parameters[index]) {
        return None;
    }
    let primitive_type = program.primitive_type_reference(parameters[index].type_reference)?;
    let position = parameters[..index]
        .iter()
        .filter(|parameter| {
            !parameter.relevance.is_erased()
                && typed_trees::callable_values::ScalarValueType::from_type_reference(
                    program.typed_program(),
                    parameter.type_reference,
                )
                .is_some()
        })
        .count();
    crate::scalar_value_domain_predicate(
        language_semantics::value_domain::ValueDomain::Finite,
        CheckedScalarExpression::Parameter {
            position,
            scalar_type: primitive_type.into(),
        },
    )
}

fn scalar_declared_domain(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
    membership: &typed_trees::domain::ProofMembershipFact,
) -> Option<typed_trees::types::DomainConstraint> {
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(reference)
    {
        for constraint in program.type_reference_table.constraints(*constraints) {
            if let typed_trees::types::TypeConstraintNode::Domain(domain) = constraint
                && domain.symbol == membership.domain_symbol
                && domain.semantic_id == membership.semantic_domain
            {
                return Some(domain.clone());
            }
        }
        reference = *base_type;
    }
    None
}

pub struct StructuralResultContract {
    pub carrier: TypeReferenceHandle,
    pub qualifications: Vec<SemanticDomainId>,
}

fn declared_carrier_qualifications(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
    carrier_is_referent: bool,
) -> Result<(TypeReferenceHandle, Vec<SemanticDomainId>), &'static str> {
    let mut qualifications = Vec::new();
    loop {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let retained = program.type_reference_table.constraints(*constraints);
                if retained.len() != constraints.len() {
                    return Err("declared qualification span is stale");
                }
                for constraint in retained {
                    if matches!(constraint, TypeConstraintNode::ArithmeticDomain(_))
                        && program.primitive_type_reference(reference).is_some()
                    {
                        continue;
                    }
                    let TypeConstraintNode::Domain(domain) = constraint else {
                        return Err("declared qualification constraint is not a domain");
                    };
                    if matches!(domain.subject, DomainConstraintSubject::Carry(_)) {
                        continue;
                    }
                    if !domain.semantic_id.is_valid() {
                        return Err("declared qualification has no exact identity");
                    }
                    qualifications.push(domain.semantic_id);
                }
                reference = *base_type;
            }
            TypeReferenceNode::Reference { referee, .. } if carrier_is_referent => {
                reference = *referee;
            }
            _ => break,
        }
    }
    qualifications.sort_by_key(|domain| domain.0);
    qualifications.dedup();
    Ok((reference, qualifications))
}

pub fn structural_result_contract(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Result<StructuralResultContract, &'static str> {
    let (carrier, qualifications) = declared_carrier_qualifications(program, reference, false)?;
    if matches!(
        program.type_reference_table.type_reference(carrier),
        TypeReferenceNode::Reference { .. }
    ) {
        return Err("structural result custody cannot manufacture an owned reference");
    }
    Ok(StructuralResultContract {
        carrier,
        qualifications,
    })
}

pub fn structural_result_qualifications(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Result<Vec<SemanticDomainId>, &'static str> {
    Ok(structural_result_contract(program, reference)?.qualifications)
}

pub fn view_result_carrier_qualifications(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Result<Vec<SemanticDomainId>, &'static str> {
    let carrier_is_referent = matches!(
        crate::value_custody::reference_shells::byte_sequence_carrier(program, reference, &[]),
        Some(
            checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. }
                | checked_trees::CheckedByteSequenceCarrier::BoundedView { .. }
        )
    );
    Ok(declared_carrier_qualifications(program, reference, carrier_is_referent)?.1)
}

pub fn result_membership_carrier(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
    domain: SymbolHandle,
) -> Option<(Vec<CheckedUnitStructuralPathSegment>, TypeReferenceHandle)> {
    if !crate::stores_reference_shape(program, reference, &[]) {
        return Some((Vec::new(), reference));
    }
    let target = program.domain_definition(domain)?.target_type;
    let identity = crate::stored_structural_type_identity(program, target, &[], &[]);
    let mut path = Vec::new();
    for _ in 0..=program.type_reference_table.type_reference_count() {
        if crate::stored_structural_type_identity(program, reference, &[], &[]) == identity {
            return Some((path, reference));
        }
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Reference { referee, .. } => {
                if crate::stores_reference_shape(program, reference, &[]) {
                    path.push(CheckedUnitStructuralPathSegment::Referent);
                }
                reference = *referee;
            }
            _ => return None,
        }
    }
    None
}

pub fn structural_result_projected_qualifications(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Result<Vec<CheckedStructuralPathQualification>, &'static str> {
    structural_result_qualification_contract(program, type_reference)
        .map(|contract| contract.projected_qualifications)
}

pub use checked_trees::CheckedStructuralQualificationContract as StructuralQualificationContract;

pub fn structural_result_qualification_contract(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Result<StructuralQualificationContract, &'static str> {
    structural_qualification_inventory(program, type_reference, false, true)
}

pub fn structural_qualification_contract(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Result<StructuralQualificationContract, &'static str> {
    structural_qualification_inventory(program, type_reference, false, false)
}

pub fn zeroed_byte_projected_qualifications(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Result<Vec<CheckedStructuralPathQualification>, &'static str> {
    structural_qualification_inventory(program, type_reference, true, false)
        .map(|contract| contract.projected_qualifications)
}

fn structural_qualification_inventory(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
    zero_only: bool,
    published_root: bool,
) -> Result<StructuralQualificationContract, &'static str> {
    fn collect(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
        zero_only: bool,
        is_root: bool,
        published_root: bool,
        path: &mut Vec<CheckedUnitStructuralPathSegment>,
        whole: &mut Vec<SemanticDomainId>,
        output: &mut Vec<CheckedStructuralPathQualification>,
        carriers: &mut Vec<(SemanticDomainId, TypeReferenceHandle)>,
        projection_types: &mut Vec<TypeReferenceHandle>,
        active: &mut Vec<TypeReferenceHandle>,
    ) -> Result<(), &'static str> {
        if zero_only && crate::reference_shell(program, reference).is_some() {
            return Ok(());
        }
        let initial_qualifications = output.len();
        let result = match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. } => {
                if zero_only {
                    Ok(())
                } else {
                    let pointer = (!is_root || published_root)
                        && crate::stores_reference_shape(program, reference, &[]);
                    if pointer {
                        path.push(CheckedUnitStructuralPathSegment::Referent);
                    }
                    let result = collect(
                        program,
                        *referee,
                        zero_only,
                        is_root && !pointer,
                        true,
                        path,
                        whole,
                        output,
                        carriers,
                        projection_types,
                        active,
                    );
                    if pointer {
                        path.pop();
                    }
                    result
                }
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let retained = program.type_reference_table.constraints(*constraints);
                if retained.len() != constraints.len() {
                    return Err("projected qualification span is stale");
                }
                for constraint in retained {
                    let TypeConstraintNode::Domain(domain) = constraint else {
                        continue;
                    };
                    if matches!(domain.subject, DomainConstraintSubject::Carry(_)) {
                        continue;
                    }
                    if !domain.semantic_id.is_valid() {
                        return Err("projected qualification has no exact identity");
                    }
                    if program
                        .primitive_type_reference(reference)
                        .is_some_and(|primitive| {
                            crate::exact_declared_domain_carrier_interval(
                                program, primitive, domain,
                            )
                            .is_some()
                        })
                    {
                        continue;
                    }
                    if zero_only
                        && !crate::field_domains::domain_admits_zero_byte_carrier(
                            program,
                            domain.symbol,
                            reference,
                        )
                    {
                        continue;
                    }
                    if !carriers.contains(&(domain.semantic_id, reference)) {
                        carriers.push((domain.semantic_id, reference));
                    }
                    if !is_root || !path.is_empty() {
                        output.push(CheckedStructuralPathQualification {
                            path: path.clone(),
                            domain: domain.semantic_id,
                        });
                    } else {
                        whole.push(domain.semantic_id);
                    }
                }
                collect(
                    program,
                    *base_type,
                    zero_only,
                    is_root,
                    published_root,
                    path,
                    whole,
                    output,
                    carriers,
                    projection_types,
                    active,
                )
            }
            TypeReferenceNode::Slice { element_type } => {
                if zero_only {
                    return Ok(());
                }
                path.push(CheckedUnitStructuralPathSegment::AllElements);
                let result = collect(
                    program,
                    *element_type,
                    false,
                    false,
                    false,
                    path,
                    whole,
                    output,
                    carriers,
                    projection_types,
                    active,
                );
                path.pop();
                result
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                let length = match length {
                    typed_trees::types::FixedArrayLength::Literal(length) => Some(*length),
                    typed_trees::types::FixedArrayLength::ConstCall { .. }
                        if program
                            .constant_recipe_for_array_length(reference)
                            .is_valid() =>
                    {
                        None
                    }
                    _ => return Err("projected qualification array length is not literal"),
                };
                let mut element_rows = Vec::new();
                let mut element_projection_types = Vec::new();
                collect(
                    program,
                    *element_type,
                    zero_only,
                    false,
                    false,
                    &mut Vec::new(),
                    whole,
                    &mut element_rows,
                    carriers,
                    &mut element_projection_types,
                    active,
                )?;
                if element_rows.is_empty() {
                    return Ok(());
                }
                let Some(length) = length else {
                    return Err(
                        "projected qualification array length is a constant recipe fixed at target closure",
                    );
                };
                output
                    .try_reserve(
                        element_rows
                            .len()
                            .checked_mul(length)
                            .ok_or("projected qualification count overflow")?,
                    )
                    .map_err(|_| "projected qualification storage exhausted")?;
                for index in 0..length {
                    let index = u64::try_from(index)
                        .map_err(|_| "projected qualification index overflow")?;
                    for row in &element_rows {
                        let mut qualified = row.clone();
                        qualified
                            .path
                            .insert(0, CheckedUnitStructuralPathSegment::FixedIndex(index));
                        qualified.path.splice(0..0, path.iter().cloned());
                        output.push(qualified);
                    }
                }
                if length != 0 {
                    for reference in element_projection_types {
                        if !projection_types.contains(&reference) {
                            projection_types.push(reference);
                        }
                    }
                }
                Ok(())
            }
            TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
                let Some(record) = program.closed_data_definition(reference) else {
                    return Ok(());
                };
                if active.contains(&reference) {
                    return Ok(());
                }
                active.push(reference);
                let mut fields = program
                    .data_members(record)
                    .iter()
                    .filter_map(|member| member.as_field().map(|field| (None, field)))
                    .collect::<Vec<_>>();
                for variant in program
                    .data_members(record)
                    .iter()
                    .filter_map(|member| member.as_variant())
                    .take(if zero_only { 1 } else { usize::MAX })
                {
                    fields.extend(
                        program
                            .data_payload_fields(variant)
                            .iter()
                            .map(|field| (Some(variant), field)),
                    );
                }
                for (case, field) in fields {
                    if field.relevance.is_erased()
                        || program
                            .primitive_type_reference(field.type_reference)
                            .is_some()
                        || crate::byte_sequence_carrier(program, field.type_reference, &[])
                            .is_some()
                    {
                        continue;
                    }
                    if let Some(case) = case {
                        path.push(CheckedUnitStructuralPathSegment::Case(
                            CheckedMemberIdentity::of_member(case.identity, case.name.clone()),
                        ));
                    }
                    path.push(CheckedUnitStructuralPathSegment::Field(
                        CheckedMemberIdentity::of_member(field.identity, field.name.clone()),
                    ));
                    collect(
                        program,
                        field.type_reference,
                        zero_only,
                        false,
                        false,
                        path,
                        whole,
                        output,
                        carriers,
                        projection_types,
                        active,
                    )?;
                    path.pop();
                    if case.is_some() {
                        path.pop();
                    }
                }
                active.pop();
                Ok(())
            }
            _ => Ok(()),
        };
        if output.len() > initial_qualifications && !projection_types.contains(&reference) {
            projection_types.push(reference);
        }
        result
    }
    let mut output = Vec::new();
    let mut whole = Vec::new();
    let mut carriers = Vec::new();
    let mut projection_types = Vec::new();
    collect(
        program,
        type_reference,
        zero_only,
        true,
        published_root,
        &mut Vec::new(),
        &mut whole,
        &mut output,
        &mut carriers,
        &mut projection_types,
        &mut Vec::new(),
    )?;
    output.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.domain.0.cmp(&right.domain.0))
    });
    output.dedup();
    whole.sort_by_key(|domain| domain.0);
    whole.dedup();
    Ok(StructuralQualificationContract {
        qualifications: whole,
        projected_qualifications: output,
        domains: carriers,
        projection_types,
    })
}

pub fn reconstruct_structural_parameter_return_claims(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    parameter: SymbolHandle,
) -> Result<
    Vec<(
        PermissionClaimIdentity,
        Vec<CheckedUnitStructuralPathSegment>,
    )>,
    &'static str,
> {
    let owner = program
        .machine(machine)
        .ok_or("structural returned claim has no exact machine")?;
    let destination = program
        .machine_states(owner)
        .iter()
        .find(|candidate| candidate.symbol == state)
        .ok_or("structural returned claim has no exact state")?;
    let mut parameters = program
        .state_parameters(destination)
        .iter()
        .filter(|candidate| candidate.symbol == parameter);
    let source = parameters
        .next()
        .ok_or("structural claim outcome names an absent parameter")?;
    if parameters.next().is_some()
        || owner.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || source.is_mutable
        || source.is_const
        || program.type_multiplicity(destination.return_type) != Multiplicity::Linear
        || program.normalized_type_identity(source.type_reference)
            != program.normalized_type_identity(destination.return_type)
    {
        return Err("structural returned claim differs from its exact whole input contract");
    }
    structural_result_qualifications(program, source.type_reference)?;
    structural_result_qualifications(program, destination.return_type)?;
    let outcomes = returned_claim_outcomes(facts, machine, state)?
        .ok_or("structural callee has no exact claim outcome")?;
    if outcomes.is_empty() {
        return Err("structural callee has no returned claims");
    }
    let mut paths = Vec::with_capacity(outcomes.len());
    for outcome in outcomes {
        let FlowClaimOutcomeSource::Input {
            parameter_symbol,
            segments,
        } = outcome.source
        else {
            return Err("structural returned claim does not continue an input");
        };
        if parameter_symbol != parameter {
            return Err("structural returned claims do not continue the same whole input");
        }
        let input_segments = retained_claim_segments(facts, segments)?;
        let output_segments = retained_claim_segments(facts, outcome.output_segments)?;
        let input_path = structural_claim_path(program, source.type_reference, input_segments)?;
        let output_path = structural_claim_path(program, destination.return_type, output_segments)?;
        if input_path != output_path {
            return Err("structural returned claim changes its typed input path");
        }
        paths.push(output_path);
    }
    paths.sort();
    for pair in paths.windows(2) {
        if pair[1].starts_with(&pair[0]) {
            return Err("structural returned claim paths duplicate or overlap");
        }
    }
    let mut entries = Vec::with_capacity(paths.len());
    let state_permissions = facts
        .flow
        .ownership
        .with_state_permissions(state, |handles| handles.to_vec());
    for handle in state_permissions {
        let event = facts.flow.ownership.permissions.get(handle);
        if event.machine_symbol != machine
            || event.state_symbol != state
            || event.source != PermissionEventSource::StateEntry
            || event.kind != PermissionEventKind::Establish
            || event.root != facts::PlaceRoot::Symbol(parameter)
            || event.multiplicity != Multiplicity::Linear
            || !event.obligation_live
        {
            continue;
        }
        if event.access != PermissionAccess::Owned
            || event.claim_identity == PermissionClaimIdentity::Unknown
            || entries
                .iter()
                .any(|(identity, _)| *identity == event.claim_identity)
        {
            return Err("structural returned claim has ambiguous entry custody");
        }
        let segments = retained_claim_segments(facts, event.segments)?;
        entries.push((
            event.claim_identity,
            structural_claim_path(program, source.type_reference, segments)?,
        ));
    }
    entries.sort_by(|left, right| left.1.cmp(&right.1));
    if entries.len() != paths.len()
        || entries
            .iter()
            .zip(&paths)
            .any(|((_, entry_path), output_path)| entry_path != output_path)
    {
        return Err("structural returned claims differ from the complete input frontier");
    }
    Ok(entries)
}

fn returned_claim_outcomes(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
) -> Result<Option<&[FlowClaimOutcomeEntryFact]>, &'static str> {
    let mut outcomes = facts
        .flow
        .ownership
        .claim_outcome_maps
        .iter()
        .map(|(_, map)| map)
        .filter(|map| map.machine_symbol == machine && map.state_symbol == state);
    let Some(outcome) = outcomes.next() else {
        return Ok(None);
    };
    if outcomes.next().is_some() {
        return Err("structural callee has ambiguous claim outcomes");
    }
    let entries = facts
        .flow
        .ownership
        .claim_outcome_entries
        .span_or_empty(outcome.entries);
    if entries.len() != outcome.entries.len() {
        return Err("structural claim outcome span is stale");
    }
    Ok(Some(entries))
}

fn retained_claim_segments(
    facts: &CheckFacts,
    span: arena::HandleSpan<facts::PlaceSegment>,
) -> Result<&[facts::PlaceSegment], &'static str> {
    let segments = facts.flow.ownership.segments.span_or_empty(span);
    if segments.len() != span.len() {
        return Err("structural claim path span is stale");
    }
    Ok(segments)
}

pub fn structural_claim_path(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    segments: &[facts::PlaceSegment],
) -> Result<Vec<CheckedUnitStructuralPathSegment>, &'static str> {
    let (path, reference) = structural_owned_path(program, reference, segments)?;
    if program.primitive_type_reference(reference).is_some()
        || program.type_multiplicity(reference) != Multiplicity::Linear
    {
        return Err("structural claim path does not select linear structural custody");
    }
    Ok(path)
}

pub fn structural_owned_path(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    segments: &[facts::PlaceSegment],
) -> Result<(Vec<CheckedUnitStructuralPathSegment>, TypeReferenceHandle), &'static str> {
    let (path, reference) = structural_storage_path(program, reference, segments)?;
    structural_result_qualifications(program, reference)?;
    Ok((path, reference))
}

pub fn structural_transfer_storage_path(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    segments: &[facts::PlaceSegment],
) -> Result<(Vec<CheckedUnitStructuralPathSegment>, TypeReferenceHandle), &'static str> {
    let (path, reference) = structural_storage_path(program, reference, segments)?;
    let carrier = program
        .type_reference_table
        .unconstrained_type_reference(reference);
    if matches!(
        program.type_reference_table.type_reference(carrier),
        TypeReferenceNode::Reference { .. }
    ) {
        view_result_carrier_qualifications(program, reference)?;
    } else {
        structural_result_qualifications(program, reference)?;
    }
    Ok((path, reference))
}

pub fn structural_storage_path(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
    segments: &[facts::PlaceSegment],
) -> Result<(Vec<CheckedUnitStructuralPathSegment>, TypeReferenceHandle), &'static str> {
    let mut path = Vec::with_capacity(segments.len());
    let mut payload_fields: Option<&[typed_trees::data::DataField]> = None;
    for segment in segments {
        structural_result_qualifications(program, reference)?;
        reference = program
            .type_reference_table
            .unconstrained_type_reference(reference);
        match (
            segment,
            program.type_reference_table.type_reference(reference),
        ) {
            (
                facts::PlaceSegment::Field { symbol },
                TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. },
            ) => {
                let definition = program
                    .closed_data_definition(reference)
                    .ok_or("structural claim field has no exact data owner")?;
                let field = if let Some(fields) = payload_fields.take() {
                    fields.iter().find(|field| field.symbol == *symbol)
                } else {
                    program.data_members(definition).iter().find_map(|member| {
                        let field = member.as_field()?;
                        (field.symbol == *symbol).then_some(field)
                    })
                }
                .ok_or("structural claim field differs from its typed owner")?;
                if field.relevance != language_core::BindingRelevance::Relevant {
                    return Err("structural claim traverses an erased field");
                }
                path.push(CheckedUnitStructuralPathSegment::Field(
                    CheckedMemberIdentity::of_member(field.identity, field.name.clone()),
                ));
                reference = field.type_reference;
            }
            (
                facts::PlaceSegment::Case { variant },
                TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. },
            ) if payload_fields.is_none() => {
                let definition = program
                    .closed_data_definition(reference)
                    .ok_or("structural claim case has no exact data owner")?;
                let selected = program
                    .data_members(definition)
                    .iter()
                    .filter_map(|member| member.as_variant())
                    .find(|candidate| candidate.symbol == *variant)
                    .ok_or("structural claim case differs from its typed owner")?;
                path.push(CheckedUnitStructuralPathSegment::Case(
                    CheckedMemberIdentity::of_member(selected.identity, selected.name.clone()),
                ));
                payload_fields = Some(program.data_payload_fields(selected));
            }
            (
                facts::PlaceSegment::FixedIndex { index },
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: typed_trees::types::FixedArrayLength::Literal(length),
                },
            ) if payload_fields.is_none() && *index < *length => {
                path.push(CheckedUnitStructuralPathSegment::FixedIndex(
                    u64::try_from(*index).map_err(|_| "structural claim index exceeds u64")?,
                ));
                reference = *element_type;
            }
            _ => return Err("structural claim has an unsupported or mistyped path"),
        }
    }
    if payload_fields.is_some() {
        return Err("structural claim ends at a case namespace rather than a value");
    }
    view_result_carrier_qualifications(program, reference)?;
    Ok((path, reference))
}

pub fn borrowed_slice_view_result(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
    crate::slice_view_shell(program, reference, &[]).is_some_and(|view| {
        matches!(
            view.access,
            language_semantics::ReferenceAccess::Shared
                | language_semantics::ReferenceAccess::Mutable
        )
    })
}

fn structural_authored_arguments<'program>(
    program: &'program dyn crate::CheckingInput,
    call: &FlowCallFact,
    source_state: &typed_trees::state::State,
    coordinate: &CheckedUnitCallCoordinate,
    target: checked_trees::CheckedUnitCallTargetSource,
    parameter_count: usize,
) -> Result<&'program [typed_trees::expression::ExpressionHandle], &'static str> {
    let authored_arguments = if call.authored_expression.is_valid() {
        let ExpressionNode::Call(authored) =
            expression_table(program).expression(call.authored_expression)
        else {
            return Err("structural call occurrence has no authored expression");
        };
        let matches = match target {
            checked_trees::CheckedUnitCallTargetSource::Declaration { state, .. } => {
                call.runtime_call.is_none()
                    && crate::call_target_entry_symbol(
                        program,
                        crate::call_target_symbol(program, authored),
                    ) == state
            }
            checked_trees::CheckedUnitCallTargetSource::Runtime { contract } => {
                call.runtime_call
                    .is_some_and(|binding| binding.contract == contract)
                    && authored.runtime_call == call.runtime_call
            }
        };
        if !matches {
            return Err("structural call expression target changed");
        }
        expression_table(program).expression_handles(authored.arguments)
    } else {
        match program
            .statement_table
            .statements(source_state.statement_nodes)
            .get(coordinate.statement_index as usize)
        {
            Some(StatementNode::Call(authored)) => {
                let matches = match target {
                    checked_trees::CheckedUnitCallTargetSource::Declaration { state, .. } => {
                        call.runtime_call.is_none()
                            && crate::call_target_entry_symbol(program, authored.target_symbol)
                                == state
                    }
                    checked_trees::CheckedUnitCallTargetSource::Runtime { contract } => {
                        call.runtime_call
                            .is_some_and(|binding| binding.contract == contract)
                            && authored.runtime_call == call.runtime_call
                    }
                };
                if !matches {
                    return Err("structural call statement target changed");
                }
                program
                    .statement_table
                    .expression_handles(authored.arguments)
            }
            Some(StatementNode::Transition(transition)) => {
                let checked_trees::CheckedUnitCallTargetSource::Declaration {
                    state: target_state,
                    ..
                } = target
                else {
                    return Err("runtime callable invocation cannot be a named transition");
                };
                let names_target = |symbol: SymbolHandle| {
                    symbol == target_state
                        || program.machine_by_symbol(symbol).is_some_and(|machine| {
                            program
                                .machine_states(machine)
                                .first()
                                .is_some_and(|entry| entry.symbol == target_state)
                        })
                };
                let mut named = transition.targets().filter_map(|target| {
                    match program.statement_table.transition_target(target) {
                        TransitionTargetNode::Named {
                            path, arguments, ..
                        } if names_target(path.symbol) => Some(*arguments),
                        _ => None,
                    }
                });
                let arguments = named
                    .next()
                    .ok_or("structural call occurrence has no authored statement")?;
                if named.next().is_some() {
                    return Err("structural transition call names its target twice");
                }
                program.statement_table.expression_handles(arguments)
            }
            _ => return Err("structural call occurrence has no authored statement"),
        }
    };
    if authored_arguments.len() != parameter_count {
        return Err("structural call has no exact explicit argument roster");
    }
    Ok(authored_arguments)
}

fn semantic_caller_state(
    control: &checked_trees::FlowControlFacts,
    caller_machine: SymbolHandle,
    caller_state: SymbolHandle,
) -> Result<&checked_trees::FlowStateFact, &'static str> {
    let mut states = control
        .machine_states(caller_machine)
        .into_iter()
        .filter(|state| {
            state.machine_symbol == caller_machine && state.state_symbol == caller_state
        });
    let state = states
        .next()
        .ok_or("structural call has no semantic state")?;
    if states.next().is_some() {
        return Err("structural call has ambiguous semantic states");
    }
    Ok(state)
}

pub fn structural_call_custody(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    caller_machine: SymbolHandle,
    caller_state: SymbolHandle,
    operation: &CheckedUnitEffectOperationPlan,
) -> Result<CheckedStructuralCallCustodyPlan, &'static str> {
    let Some((call, result)) = operation
        .call()
        .filter(|call| !call.is_boundary())
        .and_then(|call| Some((call, call.structural_result()?)))
    else {
        return Err("structural custody requires an ordinary structural call");
    };
    let coordinate = &call.coordinate;
    let target_source = call.target.source;
    let planned_call = call;
    let structural_arguments = &call.structural_arguments;
    let caller = program
        .machine(caller_machine)
        .ok_or("structural call has no exact caller")?;
    let source_state = program
        .state_in_machine(caller, caller_state)
        .ok_or("structural call has no exact caller state")?;
    let (return_type, parameters, entry_state) =
        if let checked_trees::CheckedUnitCallTargetSource::Runtime { contract } = target_source {
            let checked_trees::CheckedUnitCallRoute::Runtime {
                contract: retained, ..
            } = call.route()
            else {
                return Err("runtime structural call lost its exact invocation route");
            };
            if *retained != contract || !program.runtime_callable_contracts.is_valid(contract) {
                return Err("runtime structural call has a stale or redirected contract");
            }
            let capsule = facts
                .contract_plans
                .runtime_callable_crash_capsule(contract)
                .ok_or("runtime structural call has no checked contract capsule")?;
            if capsule.target_contract_report_fingerprint()
                != call.target.contract_report_fingerprint
            {
                return Err("runtime structural call changed its checked contract identity");
            }
            let row = program.runtime_callable_contracts.get(contract);
            let signature = &row.signature;
            let parameters = program.state_signature_parameters(signature);
            crate::machine_calls::reference_result_custody::runtime_declared_return_sources(
                program,
                contract,
                row.lexical_owner,
            )?;
            (signature.return_type, parameters, None)
        } else {
            let (target_machine, target_state) = call
                .target
                .declaration()
                .ok_or("structural declaration call lost its target")?;
            let target_machine = &target_machine;
            let target_state = &target_state;
            let dynamic_slot = match call.route() {
                checked_trees::CheckedUnitCallRoute::Dynamic {
                    interface_identity,
                    requirement,
                    ..
                } => {
                    let slot = program
                        .dynamic_slot_by_symbol(*target_state)
                        .ok_or("structural selector call has no exact slot")?;
                    if slot.symbol != *target_machine
                        || slot.requirement != *requirement
                        || slot.interface_identity != *interface_identity
                    {
                        return Err("structural selector call differs from its public slot");
                    }
                    Some(slot)
                }
                _ => None,
            };
            if let Some(slot) = dynamic_slot {
                crate::machine_calls::reference_result_custody::declared_return_sources(
                    program,
                    &slot.parameters,
                    slot.return_type,
                )?;
                (
                    slot.return_type,
                    slot.parameters.as_slice(),
                    Some(*target_state),
                )
            } else {
                let target = program
                    .machine(*target_machine)
                    .ok_or("structural call has no exact callee")?;
                let destination = typed_trees::state::state_by_symbol(
                    program.machine_states(target),
                    *target_state,
                )
                .ok_or("structural call has no exact callee state")?;
                let activation_operation = match call.route() {
                    checked_trees::CheckedUnitCallRoute::ActivationStorage { operation, bound } => {
                        let contract =
                            crate::exact_toolchain_activation_storage_contract(program, target)
                                .ok_or(
                                    "activation operation lost its sealed declaration contract",
                                )?;
                        if contract.operation != *operation {
                            return Err(
                                "activation operation differs from its selected declaration",
                            );
                        }
                        let expected_bound = if matches!(
                            *operation,
                            crate::ActivationStorageOperation::Establish
                                | crate::ActivationStorageOperation::Split
                        ) {
                            Some(crate::activation_storage_bound(program, target)?)
                        } else {
                            None
                        };
                        if *bound != expected_bound {
                            return Err(
                                "activation operation differs from its selected static bound",
                            );
                        }
                        true
                    }
                    _ => false,
                };
                if !target.supplies_checked_body() && !activation_operation {
                    return Err("structural call result differs from the callee contract");
                }
                (
                    destination.return_type,
                    program.state_parameters(destination),
                    program
                        .machine_states(target)
                        .first()
                        .map(|entry| entry.symbol),
                )
            }
        };
    let expected_multiplicity =
        crate::machine_calls::reference_result_custody::result_multiplicity(program, return_type);
    if expected_multiplicity != result.multiplicity {
        return Err("structural call result differs from the callee contract");
    }
    let result_qualifications = if borrowed_slice_view_result(program, return_type) {
        view_result_carrier_qualifications(program, return_type)?
    } else if crate::machine_calls::reference_result_custody::parts(program, return_type).is_some()
        || crate::machine_calls::reference_result_custody::has_borrowed_parts(program, return_type)
    {
        Vec::new()
    } else {
        structural_result_qualifications(program, return_type)?
    };
    let state = semantic_caller_state(&facts.flow.control, caller_machine, caller_state)?;
    let retained_calls = facts.flow.control.calls.span_or_empty(state.calls);
    if retained_calls.len() != state.calls.len() {
        return Err("structural call semantic occurrence span is stale");
    }
    let mut calls = retained_calls.iter().filter(|call| {
        call.statement_index == coordinate.statement_index as usize
            && call.call_ordinal == coordinate.call_ordinal as usize
    });
    let call = calls
        .next()
        .ok_or("structural call has no exact semantic occurrence")?;
    let targets_destination = match target_source {
        checked_trees::CheckedUnitCallTargetSource::Declaration { machine, state } => {
            call.runtime_call.is_none()
                && (call.target_symbol == state
                    || (call.target_symbol == machine && entry_state == Some(state)))
        }
        checked_trees::CheckedUnitCallTargetSource::Runtime { contract } => {
            let site = crate::find_call_site(
                program,
                caller_machine,
                caller_state,
                call.statement_index,
                call.call_ordinal,
            )
            .ok_or("runtime structural call lost its authored call site")?;
            let Some(crate::CallContractSource::Runtime { binding, .. }) =
                crate::call_contract_source(program, caller_machine, &site)?
            else {
                return Err("runtime structural call source changed its invocation kind");
            };
            let checked_trees::CheckedUnitCallRoute::Runtime { callee, .. } = planned_call.route()
            else {
                return Err("runtime structural call lost its evaluated callee");
            };
            let callee_matches = match callee {
                checked_trees::CheckedCallScalarArgument::Computation(value) => {
                    facts.values.scalar_computations.nodes.is_valid(*value)
                        && facts
                            .values
                            .scalar_computations
                            .nodes
                            .get(*value)
                            .authored_root
                            == binding.callee
                }
                checked_trees::CheckedCallScalarArgument::Pure(value) => facts
                    .values
                    .scalar_expressions
                    .bound_expression_at(
                        caller_state,
                        coordinate.statement_index,
                        checked_trees::CheckedScalarExpressionRole::RuntimeCallCallee {
                            call_ordinal: coordinate.call_ordinal,
                        },
                    )
                    .is_some_and(|(source, retained)| {
                        source.expression == binding.callee && retained == value
                    }),
            };
            binding.contract == contract && call.runtime_call == Some(binding) && callee_matches
        }
    };
    if calls.next().is_some() || !targets_destination {
        return Err("structural call target or occurrence differs from source");
    }
    let transfers = facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| {
            event.machine_symbol == caller_machine
                && event.state_symbol == caller_state
                && event.source
                    == PermissionEventSource::Call {
                        statement_index: call.statement_index,
                        call_ordinal: call.call_ordinal,
                        target_symbol: call.target_symbol,
                    }
                && event.kind == PermissionEventKind::Transfer
                && event.access == PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Linear
                && event.obligation_live
        })
        .collect::<Vec<_>>();
    let claim_transfers = owned_argument_claim_transfers(
        program,
        call,
        source_state,
        coordinate,
        parameters,
        target_source,
        structural_arguments,
        &transfers,
        &facts.flow.ownership.segments,
    )?;
    if crate::machine_calls::reference_result_custody::local_owes_record_loans(program, return_type)
    {
        use crate::machine_calls::reference_result_custody::record_call_reception;
        let reception = record_call_reception(program, facts, caller_machine, source_state, call)?;
        if !reception.residual_paths.is_empty()
            && !crate::machine_calls::reference_result_custody::transient_call_record_loans(
                program,
                facts,
                caller_machine,
                source_state,
                call,
                structural_arguments,
                &reception.residual_paths,
            )
        {
            return Err("reference record result has no exact captured leaf loans");
        }
    }
    let forwarded_reference_sources =
        crate::machine_calls::reference_result_custody::terminal_call_reference_sources(
            program,
            facts,
            source_state,
            call,
            result,
        )
        .unwrap_or_default();
    let reference_loans = if !forwarded_reference_sources.is_empty() {
        arena::HandleSpan::empty()
    } else if crate::machine_calls::reference_result_custody::has_borrowed_parts(
        program,
        return_type,
    ) {
        crate::machine_calls::reference_result_custody::result_loans(
            program,
            facts,
            caller_machine,
            source_state,
            call,
            result,
        )
        .ok_or("reference result has no exact returned ingress and loan lifetime")?
    } else {
        arena::HandleSpan::empty()
    };
    let result_claims = structural_call_result_claims(
        program,
        facts,
        caller_machine,
        caller_state,
        call,
        return_type,
    )?;
    let mut retained_reference_inputs = Vec::new();
    if crate::reference_result_custody::stored_loan_parts(program, return_type).is_some() {
        let sources = match target_source {
            checked_trees::CheckedUnitCallTargetSource::Runtime { contract } => {
                let row = program.runtime_callable_contracts.get(contract);
                crate::machine_calls::reference_result_custody::runtime_declared_return_sources(
                    program,
                    contract,
                    row.lexical_owner,
                )?
            }
            checked_trees::CheckedUnitCallTargetSource::Declaration { state, .. } => {
                crate::machine_calls::reference_result_custody::call_return_sources(
                    program, facts, state,
                )
                .ok_or("reference result has no complete input origins")?
            }
        };
        for source in sources {
            let parameter_index = match source.source.root {
                checked_trees::CheckedReferenceOriginRoot::Parameter { index } => index,
                checked_trees::CheckedReferenceOriginRoot::Invocation { .. } => continue,
            };
            let actual = structural_arguments
                .get(parameter_index as usize)
                .ok_or("reference result input is absent")?;
            if actual.access.borrow_access_kind().is_some()
                && !retained_reference_inputs.contains(&parameter_index)
            {
                retained_reference_inputs.push(parameter_index);
            }
        }
    }
    Ok(CheckedStructuralCallCustodyPlan {
        reference_loans,
        retained_reference_inputs,
        forwarded_reference_sources,
        result_qualifications,
        claim_transfers,
        result_claims,
    })
}

pub fn structural_call_result_claims(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    caller_machine: SymbolHandle,
    caller_state: SymbolHandle,
    call: &FlowCallFact,
    result_type: typed_trees::types::TypeReferenceHandle,
) -> Result<Vec<CheckedStructuralResultClaimPlan>, &'static str> {
    let result_frontier = crate::linear_claim_frontier(program, result_type);
    let possible_paths = possible_returned_claim_paths(program, facts, call)?;
    if let Some(paths) = &possible_paths
        && paths.iter().any(|path| {
            !result_frontier
                .iter()
                .any(|claim| claim.path.as_slice() == *path)
        })
    {
        return Err("structural callee outcome exceeds its declared result frontier");
    }
    let received = if result_frontier.is_empty() {
        Vec::new()
    } else {
        received_result_claims(program, facts, caller_machine, caller_state, call)?
    };
    if received.iter().any(|received| {
        !result_frontier
            .iter()
            .any(|claim| claim.path.as_slice() == received.path)
    }) {
        return Err("structural result receives claims outside its result frontier");
    }
    let mut result_claims = Vec::with_capacity(result_frontier.len());
    for claim in &result_frontier {
        let absent = possible_paths
            .as_ref()
            .is_some_and(|paths| !paths.contains(&claim.path.as_slice()));
        let identity = if received.is_empty() {
            PermissionClaimIdentity::Unknown
        } else {
            let mut matching = received
                .iter()
                .filter(|received| received.path == claim.path.as_slice());
            let received = matching
                .next()
                .ok_or("structural result claim has no receiving establishment")?;
            if absent {
                if received.obligation_live
                    || received.identity != PermissionClaimIdentity::Unknown
                    || matching.any(|other| {
                        other.obligation_live
                            || other.identity != PermissionClaimIdentity::Unknown
                            || other.evaluation != received.evaluation
                    })
                {
                    return Err("structural callee omits a live or ambiguous result claim");
                }
            } else if matching.next().is_some() {
                return Err("structural result claim is received twice");
            }
            received.identity
        };
        if absent {
            if !claim.conditional || identity != PermissionClaimIdentity::Unknown {
                return Err("structural callee omits a live or unconditional result claim");
            }
            continue;
        }
        result_claims.push(CheckedStructuralResultClaimPlan {
            claim: identity,
            path: structural_claim_path(program, result_type, &claim.path)?,
        });
    }
    result_claims.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(result_claims)
}

fn possible_returned_claim_paths<'facts>(
    program: &dyn crate::CheckingInput,
    facts: &'facts CheckFacts,
    call: &FlowCallFact,
) -> Result<Option<Vec<&'facts [facts::PlaceSegment]>>, &'static str> {
    let Some(state) = crate::call_target_state(program, call.target_symbol) else {
        return Ok(None);
    };
    let machine = program
        .machine_holding_state(state.symbol)
        .ok_or("structural callee state has no exact owner")?;
    if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody {
        return Ok(None);
    }
    let Some(outcomes) = returned_claim_outcomes(facts, machine.symbol, state.symbol)? else {
        return Ok(None);
    };
    let mut paths = Vec::with_capacity(outcomes.len());
    for outcome in outcomes {
        let path = retained_claim_segments(facts, outcome.output_segments)?;
        if outcome.source == FlowClaimOutcomeSource::Unknown || paths.contains(&path) {
            return Err("structural callee outcome has an unknown or duplicate result claim");
        }
        paths.push(path);
    }
    Ok(Some(paths))
}

#[allow(clippy::too_many_arguments)]
fn owned_argument_claim_transfers(
    program: &dyn crate::CheckingInput,
    call: &FlowCallFact,
    source_state: &typed_trees::state::State,
    coordinate: &CheckedUnitCallCoordinate,
    parameters: &[typed_trees::signature::StateParameter],
    target: checked_trees::CheckedUnitCallTargetSource,
    structural_arguments: &[checked_trees::CheckedUnitStructuralArgumentPlan],
    transfers: &[&checked_trees::FlowPermissionEventFact],
    segments: &arena::PagedArenaSet<facts::PlaceSegment>,
) -> Result<Vec<CheckedUnitClaimTransferPlan>, &'static str> {
    let mut claim_transfers = Vec::with_capacity(transfers.len());
    if transfers.is_empty() {
        return Ok(claim_transfers);
    }
    let authored_arguments = structural_authored_arguments(
        program,
        call,
        source_state,
        coordinate,
        target,
        parameters.len(),
    )?;
    let mut accounted = vec![false; transfers.len()];
    for (parameter_position, authored_argument) in authored_arguments.iter().enumerate() {
        if program
            .primitive_type_reference(parameters[parameter_position].type_reference)
            .is_some()
        {
            continue;
        }
        let argument_position = parameters[..parameter_position]
            .iter()
            .filter(|parameter| {
                program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
            })
            .count();
        let Some(argument) = structural_arguments.get(argument_position) else {
            return Err("structural claim transfer names an absent argument");
        };
        if argument.access != CheckedStructuralAccess::Owned {
            continue;
        }
        let Some(mut place) = crate::canonical_place_from_expression_in_state(
            program,
            source_state.symbol,
            call.statement_index,
            *authored_argument,
        ) else {
            continue;
        };
        if argument
            .source_structural_result_binding_ordinal()
            .is_some()
            && place.root.as_symbol().is_some()
            && transfers
                .iter()
                .any(|event| event.root == facts::PlaceRoot::Expression(*authored_argument))
        {
            if transfers.iter().any(|event| {
                event.root == place.root
                    && segments
                        .span_or_empty(event.segments)
                        .starts_with(&place.segments)
            }) {
                return Err("owned actual has ambiguous storage and completed-value custody");
            }
            place = crate::CanonicalPlace {
                root: facts::PlaceRoot::Expression(*authored_argument),
                segments: Vec::new(),
            };
        }
        for (event_index, event) in transfers.iter().enumerate() {
            if event.root != place.root
                || !segments
                    .span_or_empty(event.segments)
                    .starts_with(place.segments.as_slice())
                || accounted[event_index]
            {
                continue;
            }
            let root_type = crate::canonical_place_type_reference(
                program,
                source_state.symbol,
                call.statement_index,
                &crate::CanonicalPlace {
                    root: place.root,
                    segments: Vec::new(),
                },
            )
            .ok_or("owned argument has no canonical root type")?;
            let root_type = crate::assignment_value_type(program, root_type);
            let (event_path, _) =
                structural_owned_path(program, root_type, segments.span_or_empty(event.segments))?;
            let path = event_path
                .strip_prefix(argument.path.as_slice())
                .ok_or("claim is outside the selected owned argument")?
                .to_vec();
            accounted[event_index] = true;
            claim_transfers.push(CheckedUnitClaimTransferPlan {
                claim_identity: event.claim_identity,
                argument_index: u32::try_from(argument_position)
                    .map_err(|_| "structural argument position exceeds u32")?,
                path,
            });
        }
    }
    if accounted.iter().any(|accounted| !accounted) {
        return Err("consumed claim does not cross at an owned whole argument");
    }
    Ok(claim_transfers)
}

#[derive(PartialEq, Eq)]
pub(crate) struct ReceivedResultClaim<'facts> {
    path: &'facts [facts::PlaceSegment],
    identity: PermissionClaimIdentity,
    provenance: language_semantics::PermissionProvenance,
    obligation_live: bool,
    evaluation: checked_trees::FlowPermissionEvaluation,
}

fn received_result_claims<'facts>(
    program: &dyn crate::CheckingInput,
    facts: &'facts CheckFacts,
    caller_machine: SymbolHandle,
    caller_state: SymbolHandle,
    call: &FlowCallFact,
) -> Result<Vec<ReceivedResultClaim<'facts>>, &'static str> {
    let mut received = Vec::new();
    let state_permissions = facts
        .flow
        .ownership
        .with_state_permissions(caller_state, |handles| handles.to_vec());
    for handle in state_permissions {
        let event = facts.flow.ownership.permissions.get(handle);
        if event.machine_symbol != caller_machine
            || event.state_symbol != caller_state
            || event.kind != PermissionEventKind::Establish
            || event.access != PermissionAccess::Owned
            || event.multiplicity != Multiplicity::Linear
            || !matches!(event.source,
                PermissionEventSource::Statement { statement_index }
                | PermissionEventSource::Call { statement_index, .. }
                if statement_index == call.statement_index)
        {
            continue;
        }
        let Some((_, producer, relative)) =
            crate::receiving_value(program, &facts.flow.ownership.segments, event)
        else {
            continue;
        };
        if producer != call.authored_expression {
            continue;
        }
        if event.evaluation == checked_trees::FlowPermissionEvaluation::Unknown {
            return Err("structural result establishment has no exact evaluation");
        }
        if event.obligation_live && event.claim_identity == PermissionClaimIdentity::Unknown {
            return Err("structural result establishment has no claim identity");
        }
        if !event.obligation_live
            && (event.claim_identity != PermissionClaimIdentity::Unknown
                || !relative
                    .iter()
                    .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. })))
        {
            return Err("inactive structural result claim has no exact conditional absence");
        }
        let receiving = ReceivedResultClaim {
            path: relative,
            identity: event.claim_identity,
            provenance: event.provenance,
            obligation_live: event.obligation_live,
            evaluation: event.evaluation,
        };
        if !received.contains(&receiving) {
            received.push(receiving);
        }
    }
    Ok(received)
}
