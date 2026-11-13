/*
Builtin bound-expression meaning, collection predicate exploration, default
domains, and scalar domain membership (`proof_contracts::{bound_expression_meaning,
collection_predicates, default_domains, domains}`).
*/
pub mod bound_expression_meaning {

    use crate::frozen_program::expression_table;
    use crate::value_custody::places::declared_place_type_raw;
    use language_core::OperatorSpelling;
    use typed_trees::data::{DataDefinition, DataMember};
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
    use typed_trees::machine::Machine;
    use typed_trees::state::State;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
    use typed_trees::{AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget};

    pub(crate) mod definition_membership {
        use super::{exact_case_membership_meaning, membership_subject_matches_owner};
        use crate::frozen_program::expression_table;
        use crate::value_custody::places::{exact_data_member_field, unwrapped_type_reference};
        use arena::HandleSpan;
        use symbols::{SymbolHandle, SymbolKind};
        use typed_trees::data::{DataDefinition, DataField};
        use typed_trees::domain::{DomainDefinition, ProofFact};
        use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableBinaryExpression};
        use typed_trees::types::TypeReferenceHandle;

        pub fn has_exact_domain_case_membership_meaning(
            program: &dyn crate::CheckingInput,
            domain: &DomainDefinition,
            expression: ExpressionHandle,
            comparison: &TableBinaryExpression,
        ) -> bool {
            if !has_exact_domain_definition(program, domain)
                || !owns_expression(program, domain.facts, expression, comparison)
            {
                return false;
            }
            exact_case_membership_meaning(program, expression, comparison, |subject, owner| {
                membership_subject_matches_owner(program, subject, owner, |subject| {
                    subject_type(program, DefinitionSubject::Domain(domain), subject, 0)
                })
            })
        }

        pub fn has_exact_data_case_membership_meaning(
            program: &dyn crate::CheckingInput,
            data: &DataDefinition,
            expression: ExpressionHandle,
            comparison: &TableBinaryExpression,
        ) -> bool {
            let mut definitions = program
                .data_definitions()
                .iter()
                .filter(|row| row.symbol == data.symbol);
            if !data.symbol.is_valid()
                || program.symbols.get(data.symbol).kind != SymbolKind::Data
                || definitions.next() != Some(data)
                || definitions.next().is_some()
                || !owns_expression(program, data.requires_facts, expression, comparison)
            {
                return false;
            }
            exact_case_membership_meaning(program, expression, comparison, |subject, owner| {
                membership_subject_matches_owner(program, subject, owner, |subject| {
                    subject_type(program, DefinitionSubject::Data(data), subject, 0)
                })
            })
        }

        fn owns_expression(
            program: &dyn crate::CheckingInput,
            facts: HandleSpan<ProofFact>,
            expression: ExpressionHandle,
            comparison: &TableBinaryExpression,
        ) -> bool {
            if !expression_table(program).expression_is_valid(expression)
                || !matches!(expression_table(program).expression(expression),
            ExpressionNode::Binary(retained) if retained == comparison)
            {
                return false;
            }
            facts_contain_expression(program, facts, expression)
        }

        pub(crate) fn facts_contain_expression(
            program: &dyn crate::CheckingInput,
            facts: HandleSpan<ProofFact>,
            expression: ExpressionHandle,
        ) -> bool {
            let mut seen = Vec::new();
            program.proof_facts.span_or_empty(facts).iter().any(|fact| {
                let mut reaches = |root| reaches_expression(program, root, expression, &mut seen);
                match fact {
                    ProofFact::Expression(root) => reaches(*root),
                    ProofFact::Membership(membership) => reaches(membership.value),
                    ProofFact::Proposition(application) => expression_table(program)
                        .expression_handles(application.arguments)
                        .iter()
                        .any(|argument| reaches(*argument)),
                }
            })
        }

        fn reaches_expression(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            target: ExpressionHandle,
            seen: &mut Vec<(u32, u32)>,
        ) -> bool {
            if !expression.is_valid() {
                return false;
            }
            let key = (expression.arena_index(), expression.generation());
            match seen.binary_search(&key) {
                Ok(_) => return false,
                Err(position) => seen.insert(position, key),
            }
            if expression == target {
                return true;
            }
            let mut found = false;
            crate::queries::expression_traversal::children(
                program,
                expression_table(program).expression(expression),
                |child| found = found || reaches_expression(program, child, target, seen),
            );
            found
        }

        #[derive(Clone, Copy)]
        pub(crate) enum DefinitionSubject<'program> {
            Domain(&'program DomainDefinition),
            Data(&'program DataDefinition),
        }

        pub fn exact_domain_subject_type(
            program: &dyn crate::CheckingInput,
            domain: &DomainDefinition,
            expression: ExpressionHandle,
        ) -> Option<TypeReferenceHandle> {
            if !has_exact_domain_definition(program, domain)
                || !matches!(
                    expression_table(program).expression(expression),
                    ExpressionNode::Name(_)
                )
            {
                return None;
            }
            subject_type(program, DefinitionSubject::Domain(domain), expression, 0)
        }

        pub(crate) fn has_exact_domain_definition(
            program: &dyn crate::CheckingInput,
            domain: &DomainDefinition,
        ) -> bool {
            let mut definitions = program
                .domain_definitions()
                .iter()
                .filter(|candidate| candidate.symbol == domain.symbol);
            domain.symbol.is_valid()
                && program.symbols.get(domain.symbol).kind == SymbolKind::Domain
                && definitions.next() == Some(domain)
                && definitions.next().is_none()
        }

        fn subject_type(
            program: &dyn crate::CheckingInput,
            owner: DefinitionSubject<'_>,
            expression: ExpressionHandle,
            depth: usize,
        ) -> Option<TypeReferenceHandle> {
            let node = expression_table(program).expression_at_depth(expression, depth)?;
            let reference = match node {
                ExpressionNode::Name(path) => {
                    let [name] = expression_table(program).name_path_members(path.members) else {
                        return None;
                    };
                    match owner {
                        DefinitionSubject::Domain(domain) => {
                            if !typed_trees::domain::domain_subject_reference(
                                program, domain, expression,
                            ) {
                                return None;
                            }
                            domain.target_type
                        }
                        DefinitionSubject::Data(data) => {
                            if data.predicate_subject_symbol.is_valid()
                                && path.symbol == data.predicate_subject_symbol
                                && path.head_symbol == path.symbol
                                && expression_table(program)
                                    .name_path_member_symbols(path.member_symbols)
                                    == [path.symbol]
                            {
                                return Some(data.predicate_subject_type);
                            }
                            if !path.symbol.is_valid()
                                || path.head_symbol != path.symbol
                                || expression_table(program)
                                    .name_path_member_symbols(path.member_symbols)
                                    != [path.symbol]
                            {
                                return None;
                            }
                            checked_field(program, data, path.symbol, name.as_str())?.type_reference
                        }
                    }
                }
                ExpressionNode::Borrow(borrow) => {
                    subject_type(program, owner, borrow.target, depth + 1)?
                }
                ExpressionNode::Member(member) if member.case_variant.is_none() => {
                    let receiver = unwrapped_type_reference(
                        program,
                        subject_type(program, owner, member.receiver, depth + 1)?,
                    )?;
                    let symbol = &program
                        .type_reference_table
                        .type_reference(receiver)
                        .as_named_symbol()?;
                    let mut definitions = program
                        .data_definitions()
                        .iter()
                        .filter(|row| row.symbol == *symbol);
                    let data = definitions.next()?;
                    if definitions.next().is_some() {
                        return None;
                    }
                    checked_field(program, data, member.member_symbol, member.member.as_str())?
                        .type_reference
                }
                _ => return None,
            };
            program
                .type_reference_table
                .contains_type_reference(reference)
                .then_some(reference)
        }

        fn checked_field<'program>(
            program: &'program dyn crate::CheckingInput,
            data: &'program DataDefinition,
            symbol: SymbolHandle,
            name: &str,
        ) -> Option<&'program DataField> {
            if !data.symbol.is_valid() || program.symbols.get(data.symbol).kind != SymbolKind::Data
            {
                return None;
            }
            let field = exact_data_member_field(program, data, symbol, name, None)?;
            let declaration = program.symbols.get(field.symbol);
            (declaration.kind == SymbolKind::Field
                && declaration.parent == data.symbol
                && program.symbols.name(field.symbol) == field.name.as_str())
            .then_some(field)
        }
    }
    pub(crate) mod domain_bounds {
        use super::builtin_type_reference;
        use super::definition_membership::{
            exact_domain_subject_type, facts_contain_expression, has_exact_domain_definition,
        };
        use crate::frozen_program::expression_table;
        use language_core::OperatorSpelling;
        use symbols::BuiltinTypeAtom;
        use typed_trees::domain::DomainDefinition;
        use typed_trees::expression::{
            BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
        };
        use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

        pub fn has_exact_integer_domain_subject(
            program: &dyn crate::CheckingInput,
            domain: &DomainDefinition,
            bound_lookup: &crate::proof_contracts::immutable_integer_bounds::ImmutableBoundLookup<
                '_,
            >,
            expression: ExpressionHandle,
        ) -> bool {
            if !has_exact_domain_definition(program, domain) {
                return false;
            }
            let subject_type = match expression_table(program).expression(expression) {
                ExpressionNode::Name(path) => {
                    if crate::queries::expression_traversal::single_segment_name_symbol(
                        program, expression,
                    )
                    .is_none()
                        || expression_table(program).name_path_member_symbols(path.member_symbols)
                            != [path.symbol]
                    {
                        return false;
                    }
                    let Some(subject_type) =
                        crate::value_custody::places::bound_symbol_declared_type(
                            program,
                            path.symbol,
                        )
                    else {
                        return false;
                    };
                    subject_type
                }
                ExpressionNode::Member(_) => {
                    let Some(subject_type) =
                crate::proof_contracts::immutable_integer_bounds::projected_integer_bound_subject_type(
                    program,
                    bound_lookup,
                    expression,
                )
            else {
                return false;
            };
                    subject_type
                }
                _ => return false,
            };
            matches!(integer_carrier(program, domain.target_type), Some(carrier)
        if integer_carrier(program, subject_type) == Some(carrier))
        }

        fn integer_carrier(
            program: &dyn crate::CheckingInput,
            mut reference: TypeReferenceHandle,
        ) -> Option<PrimitiveType> {
            for _ in 0..128 {
                match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                    _ => {
                        let primitive = crate::value_custody::recasts::exact_primitive_type(
                            program, reference,
                        )?;
                        return (primitive.accepts_integer_literal()
                            && primitive != PrimitiveType::Addr)
                            .then_some(primitive);
                    }
                }
            }
            None
        }

        pub fn has_builtin_domain_decomposed_guard_meaning(
            program: &dyn crate::CheckingInput,
            domain: &DomainDefinition,
            expression: ExpressionHandle,
        ) -> bool {
            if !expression_table(program).expression_is_valid(expression)
                || !has_exact_domain_definition(program, domain)
                || !facts_contain_expression(program, domain.facts, expression)
            {
                return false;
            }
            let Some(binary) = expression_table(program).binary_node(expression) else {
                return matches!(expression_table(program).expression(expression),
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot);
            };
            let spelling = match binary.operator {
                BinaryOperator::And | BinaryOperator::Or => return true,
                BinaryOperator::Less => OperatorSpelling::Less,
                BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
                BinaryOperator::Greater => OperatorSpelling::Greater,
                BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
                BinaryOperator::Equal => OperatorSpelling::Equal,
                BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
                _ => return false,
            };
            typed_trees::operator::has_builtin_spelled_expression_meaning(
                program,
                domain.symbol,
                expression,
                spelling,
                &[
                    operand_type(program, domain, binary.left),
                    operand_type(program, domain, binary.right),
                ],
            )
        }

        fn operand_type(
            program: &dyn crate::CheckingInput,
            domain: &DomainDefinition,
            expression: ExpressionHandle,
        ) -> Option<TypeReferenceHandle> {
            match expression_table(program).expression(expression) {
        ExpressionNode::Name(_) => {
            exact_domain_subject_type(program, domain, expression).or_else(|| {
                crate::value_custody::expression_types::domain_expression_result_type_reference(
                    program, domain, expression,
                )
            })
        }
        ExpressionNode::Integer(_) => {
            crate::landed_integer_literal_type_reference(program, expression)
        }
        ExpressionNode::Boolean(_) => builtin_type_reference(program, BuiltinTypeAtom::Bool),
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            builtin_type_reference(program, BuiltinTypeAtom::Bool)
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
                    | BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::And
                    | BinaryOperator::Or
            ) =>
        {
            builtin_type_reference(program, BuiltinTypeAtom::Bool)
        }
        _ => crate::value_custody::expression_types::domain_expression_result_type_reference(
            program, domain, expression,
        ),
    }
        }
    }
    pub use definition_membership::{
        exact_domain_subject_type, has_exact_data_case_membership_meaning,
        has_exact_domain_case_membership_meaning,
    };
    pub use domain_bounds::{
        has_builtin_domain_decomposed_guard_meaning, has_exact_integer_domain_subject,
    };

    pub fn has_builtin_bound_expression_meaning(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
    ) -> bool {
        matches!(
            bound_subtree_height(program, machine, state, expression, 0),
            SubtreeMeaning::Valid(_)
        )
    }

    pub fn match_result_unique_arm(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        value: bool,
    ) -> Option<ExpressionHandle> {
        let ExpressionNode::Match(dispatch) = expression_table(program).expression(expression)
        else {
            return None;
        };
        let mut survivors = expression_table(program)
            .match_arms(dispatch.arms)
            .iter()
            .filter(|arm| {
                !matches!(
                    expression_table(program).expression(arm.value),
                    ExpressionNode::Boolean(literal) if *literal != value
                )
            });
        let arm = survivors.next()?;
        survivors.next().is_none().then_some(arm.value)
    }

    pub fn has_builtin_decomposed_guard_meaning(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
    ) -> bool {
        if let Some(binary) = expression_table(program).binary_node(expression) {
            match binary.operator {
                BinaryOperator::And | BinaryOperator::Or => return true,
                BinaryOperator::Equal | BinaryOperator::NotEqual
                    if [binary.left, binary.right].into_iter().any(|operand| {
                        matches!(
                            expression_table(program).expression(operand),
                            ExpressionNode::Boolean(_)
                        )
                    }) =>
                {
                    return builtin_boolean_equality(program, machine, state, expression, binary);
                }
                _ => {}
            }
        }
        has_builtin_bound_expression_meaning(program, machine, state, expression)
    }

    pub fn has_builtin_binary_expression_meaning(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
    ) -> bool {
        let Some(memos) = crate::frozen_program::frozen_program_memos(program) else {
            return binary_expression_meaning(program, machine, state, expression);
        };
        let Some(key) = meaning_key(program, machine, state, expression) else {
            return binary_expression_meaning(program, machine, state, expression);
        };
        if let Ok(meanings) = memos.binary_meanings.lock()
            && let Some(&meaning) = meanings.get(&key)
        {
            return meaning;
        }
        let meaning = binary_expression_meaning(program, machine, state, expression);
        if let Ok(mut meanings) = memos.binary_meanings.lock() {
            meanings.insert(key, meaning);
        }
        meaning
    }

    pub(crate) fn meaning_key(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
    ) -> Option<(u32, u32, ExpressionHandle)> {
        let position = |slice_start: *const u8, size: usize, length: usize, item: *const u8| {
            let offset = (item as usize).checked_sub(slice_start as usize)?;
            (offset % size == 0 && offset / size < length)
                .then(|| u32::try_from(offset / size).ok())
                .flatten()
        };
        let machines = program.machines();
        let machine_position = position(
            machines.as_ptr().cast(),
            std::mem::size_of::<Machine>(),
            machines.len(),
            std::ptr::from_ref(machine).cast(),
        )?;
        let state_position = match state {
            None => u32::MAX,
            Some(state) => {
                let states = program.machine_states(machine);
                position(
                    states.as_ptr().cast(),
                    std::mem::size_of::<State>(),
                    states.len(),
                    std::ptr::from_ref(state).cast(),
                )?
            }
        };
        Some((machine_position, state_position, expression))
    }

    fn binary_expression_meaning(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
    ) -> bool {
        if !expression_table(program).expression_is_valid(expression) {
            return false;
        }
        let Some(binary) = expression_table(program).binary_node(expression) else {
            return false;
        };
        if program.static_type_predicate_operands(expression).is_some() {
            return true;
        }
        match binary.operator {
            BinaryOperator::CaseMembership => {
                has_exact_case_membership_meaning(program, machine, state, expression, binary)
            }
            BinaryOperator::Equal | BinaryOperator::NotEqual => {
                builtin_boolean_equality(program, machine, state, expression, binary)
            }
            BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual => {
                builtin_ordering(program, machine, state, expression, binary)
            }
            BinaryOperator::Add
            | BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Modulo => {
                builtin_arithmetic_node(program, machine, state, expression, binary)
            }
            BinaryOperator::And
            | BinaryOperator::Or
            | BinaryOperator::BitwiseAnd
            | BinaryOperator::BitwiseOr
            | BinaryOperator::BitwiseXor
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight => true,
        }
    }

    #[derive(Clone, Copy)]
    pub(crate) enum SubtreeMeaning {
        Valid(u8),
        Invalid,
        DepthLimit,
    }

    fn bound_subtree_height(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
        depth: usize,
    ) -> SubtreeMeaning {
        if !expression.is_valid() {
            return SubtreeMeaning::Invalid;
        }
        if depth >= 128 {
            return SubtreeMeaning::DepthLimit;
        }
        let memo = crate::frozen_program::frozen_program_memos(program).and_then(|memos| {
            meaning_key(program, machine, state, expression).map(|key| (memos, key))
        });
        if let Some((memos, key)) = memo
            && let Ok(meanings) = memos.bound_subtree_meanings.lock()
            && let Some(&cached) = meanings.get(&key)
        {
            return match cached {
                Some(height) if depth + usize::from(height) <= 128 => SubtreeMeaning::Valid(height),
                Some(_) => SubtreeMeaning::DepthLimit,
                None => SubtreeMeaning::Invalid,
            };
        }
        let outcome = match expression_table(program).expression(expression) {
            ExpressionNode::Binary(binary) => {
                if !has_builtin_binary_expression_meaning(program, machine, state, expression) {
                    SubtreeMeaning::Invalid
                } else {
                    match bound_subtree_height(program, machine, state, binary.left, depth + 1) {
                        SubtreeMeaning::Invalid => SubtreeMeaning::Invalid,
                        SubtreeMeaning::DepthLimit => SubtreeMeaning::DepthLimit,
                        SubtreeMeaning::Valid(left_height) => {
                            match bound_subtree_height(
                                program,
                                machine,
                                state,
                                binary.right,
                                depth + 1,
                            ) {
                                SubtreeMeaning::Invalid => SubtreeMeaning::Invalid,
                                SubtreeMeaning::DepthLimit => SubtreeMeaning::DepthLimit,
                                SubtreeMeaning::Valid(right_height) => SubtreeMeaning::Valid(
                                    left_height.max(right_height).saturating_add(1),
                                ),
                            }
                        }
                    }
                }
            }
            ExpressionNode::Borrow(borrow) => {
                match bound_subtree_height(program, machine, state, borrow.target, depth + 1) {
                    SubtreeMeaning::Valid(height) => {
                        SubtreeMeaning::Valid(height.saturating_add(1))
                    }
                    other => other,
                }
            }
            ExpressionNode::Atomic(atomic) => {
                match bound_subtree_height(program, machine, state, atomic.value, depth + 1) {
                    SubtreeMeaning::Valid(height) => {
                        SubtreeMeaning::Valid(height.saturating_add(1))
                    }
                    other => other,
                }
            }
            ExpressionNode::Unary(unary) => {
                match bound_subtree_height(program, machine, state, unary.operand, depth + 1) {
                    SubtreeMeaning::Valid(height) => {
                        SubtreeMeaning::Valid(height.saturating_add(1))
                    }
                    other => other,
                }
            }
            ExpressionNode::Cast(cast) => {
                match bound_subtree_height(program, machine, state, cast.value, depth + 1) {
                    SubtreeMeaning::Valid(height) => {
                        SubtreeMeaning::Valid(height.saturating_add(1))
                    }
                    other => other,
                }
            }
            _ => SubtreeMeaning::Valid(1),
        };
        if let Some((memos, key)) = memo {
            match outcome {
                SubtreeMeaning::Valid(height) => {
                    if let Ok(mut meanings) = memos.bound_subtree_meanings.lock() {
                        meanings.insert(key, Some(height));
                    }
                }
                SubtreeMeaning::Invalid => {
                    if let Ok(mut meanings) = memos.bound_subtree_meanings.lock() {
                        meanings.insert(key, None);
                    }
                }
                SubtreeMeaning::DepthLimit => {}
            }
        }
        outcome
    }

    pub(crate) fn builtin_ordering(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
        comparison: &typed_trees::expression::TableBinaryExpression,
    ) -> bool {
        builtin_comparison_meaning(
            program,
            machine,
            state,
            expression,
            comparison,
            |operator| match operator {
                BinaryOperator::Less => Some(OperatorSpelling::Less),
                BinaryOperator::LessOrEqual => Some(OperatorSpelling::LessEqual),
                BinaryOperator::Greater => Some(OperatorSpelling::Greater),
                BinaryOperator::GreaterOrEqual => Some(OperatorSpelling::GreaterEqual),
                _ => None,
            },
        )
    }

    pub(crate) fn builtin_boolean_equality(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
        comparison: &typed_trees::expression::TableBinaryExpression,
    ) -> bool {
        if has_exact_case_membership_meaning(program, machine, state, expression, comparison) {
            return true;
        }
        builtin_comparison_meaning(
            program,
            machine,
            state,
            expression,
            comparison,
            |operator| match operator {
                BinaryOperator::Equal => Some(OperatorSpelling::Equal),
                BinaryOperator::NotEqual => Some(OperatorSpelling::NotEqual),
                _ => None,
            },
        )
    }

    pub fn sealed_float_comparison(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
        comparison: &typed_trees::expression::TableBinaryExpression,
    ) -> bool {
        sealed_float_comparison_declaration(program, machine, state, expression, comparison)
            .is_some()
    }

    pub fn sealed_float_comparison_declaration(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
        comparison: &typed_trees::expression::TableBinaryExpression,
    ) -> Option<symbols::SymbolHandle> {
        let operand_types = [
            operand_type(program, machine, state, comparison.left),
            operand_type(program, machine, state, comparison.right),
        ];
        sealed_float_comparison_declaration_for_operands(
            program,
            machine.symbol,
            expression,
            comparison.operator,
            &operand_types,
        )
    }

    pub fn has_builtin_or_sealed_float_comparison_meaning(
        program: &dyn crate::CheckingInput,
        owner: symbols::SymbolHandle,
        expression: ExpressionHandle,
        comparison: BinaryOperator,
        operand_types: &[Option<TypeReferenceHandle>],
    ) -> bool {
        if operand_types.len() != 2 || !expression_table(program).expression_is_valid(expression) {
            return false;
        }
        let Some(spelling) = typed_trees::operator::comparison_operator_spelling(comparison) else {
            return false;
        };
        let builtin = match expression_table(program).expression(expression) {
            ExpressionNode::Binary(binary) if binary.operator == comparison => {
                typed_trees::operator::has_builtin_spelled_expression_meaning(
                    program,
                    owner,
                    expression,
                    spelling,
                    operand_types,
                )
            }
            ExpressionNode::Call(_) => false,
            _ => return false,
        };
        builtin
            || sealed_float_comparison_for_operands(
                program,
                owner,
                expression,
                comparison,
                operand_types,
            )
    }

    fn sealed_float_comparison_for_operands(
        program: &dyn crate::CheckingInput,
        owner: symbols::SymbolHandle,
        expression: ExpressionHandle,
        comparison: BinaryOperator,
        operand_types: &[Option<TypeReferenceHandle>],
    ) -> bool {
        sealed_float_comparison_declaration_for_operands(
            program,
            owner,
            expression,
            comparison,
            operand_types,
        )
        .is_some()
    }

    pub(crate) fn sealed_float_comparison_declaration_for_operands(
        program: &dyn crate::CheckingInput,
        owner: symbols::SymbolHandle,
        expression: ExpressionHandle,
        comparison: BinaryOperator,
        operand_types: &[Option<TypeReferenceHandle>],
    ) -> Option<symbols::SymbolHandle> {
        let spelling = typed_trees::operator::comparison_operator_spelling(comparison)?;
        if !typed_trees::operator::selected_trait_operator_meanings(
            program,
            owner,
            spelling,
            operand_types,
        )
        .is_empty()
        {
            return None;
        }
        let candidates = typed_trees::operator::resolve_spelling_for_operands(
            program,
            spelling,
            operand_types,
            expression_table(program).source_span(expression),
        );
        let [candidate] = candidates.as_slice() else {
            return None;
        };
        let operator = candidate.operator;
        if let Some(call) = expression_table(program).call_node(expression)
            && !call_targets_declaration_entry(program, call.target_symbol, operator.symbol)
        {
            return None;
        }
        if !operator.is_boundary
            || !matches!(
                typed_trees::operator::primitive_float_binary_semantics(program, operator),
                Some((operation, _)) if operation == comparison
            )
        {
            return None;
        }
        (crate::proof_contracts::float_projection_bindings::semantic_operations::
        is_sealed_toolchain_float_operator(program, operator)
        && expression_table(program).authored_selection_occurrences(expression).all(|occurrence| {
                match program
                    .authored_declaration_selections()
                    .get(occurrence)
                    .map(|selection| selection.target())
                {
                    Some(AuthoredDeclarationSelectionTarget::LateBound(
                        language_semantics::declaration_selection::
                            AuthoredDeclarationSelectionLateBinding::CheckedOperator,
                    )) => true,
                    Some(AuthoredDeclarationSelectionTarget::Resolved(selection)) => {
                        selection.selected_symbol() == operator.symbol
                    }
                    _ => false,
                }
            }))
    .then_some(operator.symbol)
    }

    pub(crate) fn guard_comparison(
        program: &dyn crate::CheckingInput,
        owner: symbols::SymbolHandle,
        expression: ExpressionHandle,
        operand_type: &dyn Fn(ExpressionHandle) -> Option<TypeReferenceHandle>,
    ) -> Option<typed_trees::expression::TableBinaryExpression> {
        match expression_table(program).expression(expression) {
            ExpressionNode::Binary(binary) => Some(*binary),
            ExpressionNode::Call(call)
                if !call.receiver.is_valid()
                    && !call.static_machine_parameter.is_valid()
                    && call.static_requirement_dispatch.is_none()
                    && call.machine_arguments.is_empty()
                    && call.evidence_arguments.is_empty() =>
            {
                let operator = program.machine_token_bindings().iter().find(|operator| {
                    call_targets_declaration_entry(program, call.target_symbol, operator.symbol)
                })?;
                let (operation, _) =
                    typed_trees::operator::primitive_float_binary_semantics(program, operator)?;
                let [left, right] = expression_table(program).expression_handles(call.arguments)
                else {
                    return None;
                };
                let comparison = typed_trees::expression::TableBinaryExpression {
                    policy: numerics::arithmetic::ArithmeticDomain::Exact,
                    operator: operation,
                    left: *left,
                    right: *right,
                };
                sealed_float_comparison_for_operands(
                    program,
                    owner,
                    expression,
                    comparison.operator,
                    &[
                        operand_type(comparison.left),
                        operand_type(comparison.right),
                    ],
                )
                .then_some(comparison)
            }
            _ => None,
        }
    }

    fn call_targets_declaration_entry(
        program: &dyn crate::CheckingInput,
        target: symbols::SymbolHandle,
        declaration: symbols::SymbolHandle,
    ) -> bool {
        program
            .machine_by_symbol(declaration)
            .and_then(|machine| program.machine_states(machine).first())
            .is_some_and(|entry| entry.symbol == target)
    }

    pub fn has_exact_case_membership_meaning(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
        comparison: &typed_trees::expression::TableBinaryExpression,
    ) -> bool {
        exact_case_membership_meaning(program, expression, comparison, |subject, owner| {
            membership_subject_matches_owner(program, subject, owner, |subject| {
                operand_type(program, machine, state, subject).or_else(|| {
                    crate::reserved_result_place(program, subject)
                        .filter(|place| place.machine_symbol == machine.symbol)
                        .map(|place| place.type_reference)
                })
            })
        })
    }

    pub fn applied_case_membership_variant(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
        comparison: &typed_trees::expression::TableBinaryExpression,
    ) -> Option<symbols::SymbolHandle> {
        if !has_exact_case_membership_meaning(program, machine, state, expression, comparison) {
            return None;
        }
        selected_case_in_subject_namespace(program, comparison, |subject| {
            operand_type(program, machine, state, subject)
        })
    }

    pub fn applied_declaration_case_membership_variant(
        program: &dyn crate::CheckingInput,
        declaration_owner: symbols::SymbolHandle,
        parameters: &[typed_trees::signature::StateParameter],
        expression: ExpressionHandle,
        comparison: &typed_trees::expression::TableBinaryExpression,
    ) -> Option<symbols::SymbolHandle> {
        if !has_exact_declaration_case_membership_meaning(
            program,
            declaration_owner,
            parameters,
            expression,
            comparison,
        ) {
            return None;
        }
        selected_case_in_subject_namespace(program, comparison, |subject| {
            crate::parameter_expression_result_type_reference(
                program,
                declaration_owner,
                parameters,
                subject,
            )
        })
    }

    fn selected_case_in_subject_namespace(
        program: &dyn crate::CheckingInput,
        comparison: &typed_trees::expression::TableBinaryExpression,
        subject_type: impl FnOnce(ExpressionHandle) -> Option<TypeReferenceHandle>,
    ) -> Option<symbols::SymbolHandle> {
        let selected = expression_table(program)
            .name_path_node(comparison.right)?
            .symbol;
        let mut subject = comparison.left;
        while let ExpressionNode::Borrow(borrow) = expression_table(program).expression(subject) {
            subject = borrow.target;
        }
        let reference = subject_type(subject).map(|reference| {
            program
                .type_reference_table
                .peeled_type_reference(reference)
        });
        if let Some(reference) = reference
            && program.closed_data_definition(reference).is_some()
        {
            return program
                .closed_data_constructor_case(reference, selected)
                .map(|case| case.symbol);
        }
        Some(selected)
    }

    pub fn has_exact_declaration_case_membership_meaning(
        program: &dyn crate::CheckingInput,
        declaration_owner: symbols::SymbolHandle,
        parameters: &[typed_trees::signature::StateParameter],
        expression: ExpressionHandle,
        comparison: &typed_trees::expression::TableBinaryExpression,
    ) -> bool {
        exact_case_membership_meaning(program, expression, comparison, |subject, owner| {
            membership_subject_matches_owner(program, subject, owner, |subject| {
                crate::parameter_expression_result_type_reference(
                    program,
                    declaration_owner,
                    parameters,
                    subject,
                )
                .or_else(|| {
                    crate::reserved_result_place(program, subject)
                        .filter(|result| result.machine_symbol == declaration_owner)
                        .map(|result| result.type_reference)
                })
            })
        })
    }

    pub fn has_exact_typed_case_membership_meaning(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        comparison: &typed_trees::expression::TableBinaryExpression,
        subject_type: Option<TypeReferenceHandle>,
    ) -> bool {
        exact_case_membership_meaning(program, expression, comparison, |subject, owner| {
            membership_subject_matches_owner(program, subject, owner, |_| subject_type)
        })
    }

    fn exact_case_membership_meaning(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        comparison: &typed_trees::expression::TableBinaryExpression,
        subject_matches: impl FnOnce(ExpressionHandle, &DataDefinition) -> bool,
    ) -> bool {
        if !expression_table(program).expression_is_valid(expression)
            || !expression_table(program).expression_is_valid(comparison.left)
            || !expression_table(program).expression_is_valid(comparison.right)
            || !matches!(
                comparison.operator,
                BinaryOperator::Equal | BinaryOperator::CaseMembership
            )
        {
            return false;
        }
        let Some(owner) = exact_case_reference_owner(program, comparison.right) else {
            return false;
        };
        if !subject_matches(comparison.left, owner) {
            return false;
        }
        if comparison.operator == BinaryOperator::CaseMembership {
            return true;
        }
        let Some(case) = expression_table(program).name_path_node(comparison.right) else {
            return false;
        };
        let mut has_owner = false;
        let mut has_case = false;
        for occurrence in expression_table(program).authored_selection_occurrences(expression) {
            let Some(selection) = program.authored_declaration_selections().get(occurrence) else {
                return false;
            };
            let target = selection.target();
            let Some(selected) = target.as_resolved() else {
                return false;
            };
            match selection.kind() {
                AuthoredDeclarationSelectionKind::CaseReference
                    if selected.selected_symbol() == owner.symbol && !has_owner =>
                {
                    has_owner = true
                }
                AuthoredDeclarationSelectionKind::CaseMembership
                    if selected.selected_symbol() == case.symbol && !has_case =>
                {
                    has_case = true
                }
                _ => return false,
            }
        }
        has_owner && has_case
    }

    pub fn exact_case_reference_owner(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<&DataDefinition> {
        if !expression_table(program).expression_is_valid(expression) {
            return None;
        }
        let case = expression_table(program).name_path_node(expression)?;
        program.variant_owner(case.symbol)?;
        let position = if let Some(memos) = crate::frozen_program::frozen_program_memos(program) {
            let cached = memos
                .case_reference_owners
                .lock()
                .ok()
                .and_then(|owners| owners.get(&expression).copied());
            if let Some(position) = cached {
                position
            } else {
                let position = exact_case_reference_owner_position(program, expression);
                if let Ok(mut owners) = memos.case_reference_owners.lock() {
                    owners.insert(expression, position);
                }
                position
            }
        } else {
            exact_case_reference_owner_position(program, expression)
        }?;
        program.data_definitions().get(position)
    }

    fn exact_case_reference_owner_position(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<usize> {
        if !expression_table(program).expression_is_valid(expression) {
            return None;
        }
        let case = expression_table(program).name_path_node(expression)?;
        let table = expression_table(program);
        let members = table.name_path_members(case.members);
        let selected = table.name_path_member_symbols(case.member_symbols);
        if members.len() == 1 && selected.len() == 1 && selected.first() == Some(&case.symbol) {
            let parent = program.symbols.get(case.symbol).parent;
            return program
                .data_variant_position_for_symbol(case.symbol)
                .map(|(owner_index, _)| owner_index as usize)
                .filter(|&owner_index| program.data_definitions()[owner_index].symbol == parent);
        }
        if members.len() < 2
            || selected.len() != members.len()
            || selected.first() != Some(&case.head_symbol)
            || selected.last() != Some(&case.symbol)
            || selected.iter().any(|symbol| !symbol.is_valid())
            || selected[..selected.len() - 2]
                .iter()
                .any(|symbol| program.symbols.get(*symbol).kind != symbols::SymbolKind::Module)
            || selected.windows(2).any(|pair| {
                !program
                    .symbols
                    .child_handles(pair[0])
                    .is_some_and(|mut children| children.any(|child| child == pair[1]))
            })
        {
            return None;
        }
        let carrier = selected[selected.len() - 2];
        (program.symbols.get(case.symbol).parent == carrier)
            .then(|| program.data_variant_position_for_symbol(case.symbol))
            .flatten()
            .map(|(owner_index, _)| owner_index as usize)
            .filter(|&owner_index| program.data_definitions()[owner_index].symbol == carrier)
    }

    fn membership_subject_matches_owner(
        program: &dyn crate::CheckingInput,
        subject: ExpressionHandle,
        owner: &DataDefinition,
        subject_type: impl FnOnce(ExpressionHandle) -> Option<TypeReferenceHandle>,
    ) -> bool {
        let mut subject = subject;
        let mut borrow_depth = 0;
        while let ExpressionNode::Borrow(borrow) = expression_table(program).expression(subject) {
            if borrow_depth >= 128 {
                return false;
            }
            borrow_depth += 1;
            subject = borrow.target;
        }
        match expression_table(program).expression(subject) {
            ExpressionNode::Name(path) if program.machine(path.symbol).is_some() => {
                return false;
            }
            ExpressionNode::StructLiteral(literal) => {
                if !literal.case_symbol.is_some_and(|case| {
                    case.is_valid()
                        && program.symbols.get(case).kind == symbols::SymbolKind::Variant
                        && program.symbols.get(case).parent == literal.type_symbol
                }) {
                    return false;
                }
            }
            ExpressionNode::Name(path) if path.head_symbol != path.symbol => {
                return exact_case_reference_owner(program, subject)
                    .is_some_and(|subject_owner| subject_owner.symbol == owner.symbol)
                    && program.data_members(owner).iter().any(|member| {
                        matches!(member, DataMember::Variant(variant)
                        if variant.symbol == path.symbol && variant.payload.is_empty())
                    });
            }
            _ => {}
        }
        let Some(subject_type) = subject_type(subject).and_then(|reference| {
            crate::value_custody::places::unwrapped_type_reference(program, reference)
        }) else {
            return false;
        };
        if program
            .data_constructor_definition(subject_type, owner.symbol)
            .is_some()
        {
            return true;
        }
        match program.type_reference_table.type_reference(subject_type) {
            TypeReferenceNode::Named { symbol, .. } => *symbol == owner.symbol,
            TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol == owner.symbol,
            _ => false,
        }
    }

    pub(crate) fn operand_type(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        operand_type_at_depth(program, machine, state, expression, 0)
    }

    fn operand_type_at_depth(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Option<TypeReferenceHandle> {
        if depth >= 128 || !expression_table(program).expression_is_valid(expression) {
            return None;
        }
        if let Some(result) = crate::reserved_result_place(program, expression) {
            return (result.machine_symbol == machine.symbol).then_some(result.type_reference);
        }
        let computed_result = || match state {
            Some(state) => {
                crate::expression_result_type_reference(program, machine, state, expression)
            }
            None => crate::parameter_expression_result_type_reference(
                program,
                machine.symbol,
                program.state_parameters(program.machine_states(machine).first()?),
                expression,
            ),
        };
        let reference = match expression_table(program).expression(expression) {
            ExpressionNode::SavedSubject(saved) => {
                let source = program.saved_value_source(saved)?;
                operand_type_at_depth(program, machine, state, source, depth + 1)
            }
            ExpressionNode::Boolean(_) => {
                builtin_type_reference(program, symbols::BuiltinTypeAtom::Bool)
            }
            ExpressionNode::Integer(_) => {
                crate::declarations::operators::landed_integer_literal_type_reference(
                    program, expression,
                )
            }
            ExpressionNode::Name(path) => {
                crate::value_custody::expression_types::named_value_type_reference(program, path)
            }
            ExpressionNode::Member(_)
                if crate::value_custody::places::collection_length_receiver(
                    program, machine, state, expression,
                )
                .is_some() =>
            {
                builtin_type_reference(program, symbols::BuiltinTypeAtom::U64)
            }
            ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
                declared_place_type_raw(program, machine, state, expression).or_else(|| {
                    operand_place_projection_type(program, machine, state, expression, depth + 1)
                })
            }
            ExpressionNode::Call(_)
            | ExpressionNode::Binary(_)
            | ExpressionNode::StructLiteral(_)
            | ExpressionNode::Borrow(_) => computed_result(),
            ExpressionNode::Cast(cast) if cast.form.is_recast() => computed_result(),
            ExpressionNode::Cast(cast) => Some(cast.target_type),
            ExpressionNode::ZeroValue(type_reference) => Some(*type_reference),
            // Unknown operands cannot borrow another operand's carrier.
            _ => None,
        }?;
        program
            .type_reference_table
            .contains_type_reference(reference)
            .then_some(reference)
    }

    fn operand_place_projection_type(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Option<TypeReferenceHandle> {
        let node = expression_table(program).expression_at_depth(expression, depth)?;
        match node {
            ExpressionNode::Borrow(borrow) => {
                operand_place_projection_type(program, machine, state, borrow.target, depth + 1)
            }
            ExpressionNode::Member(member) => {
                let receiver = crate::value_custody::places::unwrapped_type_reference(
                    program,
                    operand_type_at_depth(program, machine, state, member.receiver, depth + 1)?,
                )?;
                let owner = program.type_reference_table.type_head_symbol(receiver)?;
                let data =
                    crate::declarations::data_queries::data_definition_by_symbol(program, owner)?;
                crate::value_custody::places::exact_data_member_field(
                    program,
                    data,
                    member.member_symbol,
                    member.member.as_str(),
                    member.case_variant.as_ref().map(|case| case.as_str()),
                )
                .map(|field| field.type_reference)
            }
            ExpressionNode::Indexed(indexed) => {
                if expression_table(program).indexed_on_range(indexed) {
                    return None;
                }
                let collection_type =
                    operand_type_at_depth(program, machine, state, indexed.collection, depth + 1)?;
                if !crate::value_custody::places::place_has_builtin_coordinates(
                    program, machine, state, expression,
                ) {
                    return None;
                }
                let collection = crate::value_custody::places::unwrapped_type_reference(
                    program,
                    collection_type,
                )?;
                crate::value_custody::places::collection_element_type(program, collection)
            }
            _ => None,
        }
    }

    pub(crate) fn builtin_type_reference(
        program: &dyn crate::CheckingInput,
        atom: symbols::BuiltinTypeAtom,
    ) -> Option<TypeReferenceHandle> {
        let symbol = program
            .symbols
            .child_handles(program.symbols.root())?
            .find(|symbol| program.symbols.builtin_type_atom(*symbol) == Some(atom))?;
        program
            .type_reference_table
            .find_named_type_reference(symbol)
    }

    pub(crate) fn builtin_arithmetic(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
    ) -> bool {
        let Some(binary) = expression_table(program).binary_node(expression) else {
            return false;
        };
        builtin_arithmetic_node(program, machine, state, expression, binary)
            && folded_constant_is_builtin(program, machine, state, binary.left)
            && folded_constant_is_builtin(program, machine, state, binary.right)
    }

    fn builtin_arithmetic_node(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
        binary: &typed_trees::expression::TableBinaryExpression,
    ) -> bool {
        builtin_arithmetic_for_operand_types(
            program,
            machine.symbol,
            expression,
            &[
                operand_type(program, machine, state, binary.left),
                operand_type(program, machine, state, binary.right),
            ],
        )
    }

    pub(crate) fn builtin_arithmetic_for_operand_types(
        program: &dyn crate::CheckingInput,
        owner: symbols::SymbolHandle,
        expression: ExpressionHandle,
        operand_types: &[Option<TypeReferenceHandle>],
    ) -> bool {
        let Some(binary) = expression_table(program).binary_node(expression) else {
            return false;
        };
        if typed_trees::closed_numeric::has_anonymous_operator_meaning(program, expression)
            && crate::value_custody::literals::has_anonymous_numeric_results(program, binary.left)
            && crate::value_custody::literals::has_anonymous_numeric_results(program, binary.right)
        {
            return true;
        }
        let Some(spelling) = typed_trees::operator::arithmetic_operator_spelling(binary.operator)
        else {
            return false;
        };
        typed_trees::operator::has_builtin_spelled_expression_meaning(
            program,
            owner,
            expression,
            spelling,
            operand_types,
        )
    }

    pub(crate) fn folded_constant_is_builtin(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
    ) -> bool {
        folded_constant_has_builtin_meaning(program, expression, &|expression| {
            expression_table(program)
                .binary_node(expression)
                .is_some_and(|binary| {
                    builtin_arithmetic_node(program, machine, state, expression, binary)
                })
        })
    }

    pub(crate) fn folded_constant_has_builtin_meaning(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        builtin_arithmetic: &dyn Fn(ExpressionHandle) -> bool,
    ) -> bool {
        constant_subtree_meaning(program, expression, 0, builtin_arithmetic).unwrap_or(true)
    }

    fn constant_subtree_meaning(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        depth: usize,
        builtin_arithmetic: &dyn Fn(ExpressionHandle) -> bool,
    ) -> Option<bool> {
        if depth >= 128 {
            return Some(false);
        }
        match expression_table(program).expression(expression) {
            ExpressionNode::Integer(_) => Some(true),
            ExpressionNode::Borrow(borrow) => {
                constant_subtree_meaning(program, borrow.target, depth + 1, builtin_arithmetic)
            }
            ExpressionNode::Binary(binary) => {
                let left =
                    constant_subtree_meaning(program, binary.left, depth + 1, builtin_arithmetic)?;
                let right =
                    constant_subtree_meaning(program, binary.right, depth + 1, builtin_arithmetic)?;
                Some(left && right && builtin_arithmetic(expression))
            }
            _ => None,
        }
    }

    fn builtin_comparison_meaning(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
        comparison: &typed_trees::expression::TableBinaryExpression,
        spelling: impl Fn(BinaryOperator) -> Option<OperatorSpelling>,
    ) -> bool {
        let Some(spelling) = spelling(comparison.operator) else {
            return false;
        };
        typed_trees::operator::has_builtin_spelled_expression_meaning(
            program,
            machine.symbol,
            expression,
            spelling,
            &[
                operand_type(program, machine, state, comparison.left),
                operand_type(program, machine, state, comparison.right),
            ],
        )
    }

    pub fn static_type_boolean_equalities(
        program: &dyn crate::CheckingInput,
    ) -> Vec<ExpressionHandle> {
        let mut meanings = std::collections::HashMap::<
            ExpressionHandle,
            bool,
            symbols::BuildSymbolHasher,
        >::default();
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                let mut expressions = Vec::new();
                for statement in program.statement_table.statements(state.statement_nodes) {
                    crate::collect_statement_expression_trees(program, statement, &mut expressions);
                }
                for contract in program
                    .machine_contracts(machine)
                    .iter()
                    .chain(program.state_contracts(state))
                {
                    for fact in program.proof_facts.span_or_empty(contract.facts) {
                        crate::append_fact_expression_roots(program, fact, &mut expressions);
                    }
                }
                for expression in expressions {
                    let Some(binary) = expression_table(program).binary_node(expression) else {
                        continue;
                    };
                    if !matches!(
                        binary.operator,
                        BinaryOperator::Equal | BinaryOperator::NotEqual
                    ) {
                        continue;
                    }
                    if !matches!(
                        expression_table(program).expression(binary.left),
                        ExpressionNode::Boolean(_)
                    ) && !matches!(
                        expression_table(program).expression(binary.right),
                        ExpressionNode::Boolean(_)
                    ) {
                        continue;
                    }
                    let builtin =
                        builtin_boolean_equality(program, machine, Some(state), expression, binary);
                    meanings
                        .entry(expression)
                        .and_modify(|all| *all &= builtin)
                        .or_insert(builtin);
                }
            }
        }
        let mut equalities = meanings
            .into_iter()
            .filter_map(|(expression, all)| all.then_some(expression))
            .collect::<Vec<_>>();
        equalities
            .sort_unstable_by_key(|expression| (expression.arena_index(), expression.generation()));
        equalities.dedup();
        equalities
    }

    pub fn case_membership_type_facts(
        program: &dyn crate::CheckingInput,
    ) -> Vec<typed_trees::CaseMembershipTypeFacts> {
        let mut facts = Vec::new();
        let mut seen =
            std::collections::HashSet::<ExpressionHandle, symbols::BuildSymbolHasher>::default();
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                let mut expressions = Vec::new();
                for statement in program.statement_table.statements(state.statement_nodes) {
                    crate::collect_statement_expression_trees(program, statement, &mut expressions);
                }
                for contract in program
                    .machine_contracts(machine)
                    .iter()
                    .chain(program.state_contracts(state))
                {
                    crate::collect_contract_facts(program, contract.facts, &mut expressions);
                }
                for expression in expressions {
                    let Some(binary) = expression_table(program).binary_node(expression) else {
                        continue;
                    };
                    if !matches!(
                        binary.operator,
                        BinaryOperator::CaseMembership | BinaryOperator::Equal
                    ) || !seen.insert(expression)
                    {
                        continue;
                    }
                    let membership = typed_trees::expression::TableBinaryExpression {
                        policy: binary.policy,
                        left: binary.left,
                        operator: binary.operator,
                        right: binary.right,
                    };
                    facts.extend(applied_case_type_facts(
                        program,
                        machine,
                        state,
                        expression,
                        &membership,
                    ));
                }
            }
        }
        facts
    }

    fn applied_case_type_facts(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        membership: &typed_trees::expression::TableBinaryExpression,
    ) -> Option<typed_trees::CaseMembershipTypeFacts> {
        let selected =
            applied_case_membership_variant(program, machine, Some(state), expression, membership)?;
        let mut subject = membership.left;
        while let ExpressionNode::Borrow(borrow) = expression_table(program).expression(subject) {
            subject = borrow.target;
        }
        let subject_type = program
            .type_reference_table
            .peeled_type_reference(operand_type(program, machine, Some(state), subject)?);
        let (_, variant) = program.variant_owner(selected)?;
        program.closed_data_definition(subject_type)?;
        crate::impossible_variant(program, variant).then(|| typed_trees::CaseMembershipTypeFacts {
            membership: expression,
            impossible: true,
            equations: Vec::new(),
        })
    }
}

pub mod collection_predicates {
    pub(crate) mod exploration {
        use super::IntegerSet;
        use crate::CheckingInput;
        use crate::frozen_program::expression_table;
        use crate::proof_contracts::bound_expression_meaning::has_builtin_binary_expression_meaning;
        use language_core::ReferenceAccess;
        use std::collections::VecDeque;
        use typed_trees::expression::{
            BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
        };
        use typed_trees::machine::Machine;
        use typed_trees::signature::SignatureContractKind;
        use typed_trees::signature::StateParameter;
        use typed_trees::state::State;
        use typed_trees::statement::{StatementNode, TransitionExit, TransitionGuardNode};
        use typed_trees::statement::{TransitionTargetHandle, TransitionTargetNode};
        use typed_trees::types::{
            FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
        };

        const WORK_BUDGET: usize = 1 << 14;
        const MAXIMUM_EXTENT: usize = 4096;
        const MAXIMUM_RANGES: usize = 64;

        pub(crate) struct Outcomes {
            pub(crate) when_true: Option<Vec<IntegerSet>>,
            pub(crate) when_false: Option<Vec<IntegerSet>>,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) enum Binding {
            Array {
                first: usize,
                length: usize,
                element_type: TypeReferenceHandle,
            },
            CollectionExtent {
                atom: usize,
            },
            Atom {
                atom: usize,
                carrier: TypeReferenceHandle,
            },
            Constant {
                value: i128,
                carrier: TypeReferenceHandle,
            },
            Boolean(bool),
        }

        pub(crate) fn integer_carrier(
            program: &dyn CheckingInput,
            type_reference: TypeReferenceHandle,
        ) -> Option<(i128, i128)> {
            program
                .primitive_type_reference(type_reference)?
                .integer_bounds()
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum ValueShape {
            Collection {
                length: Option<usize>,
                element_type: TypeReferenceHandle,
            },
            Integer(TypeReferenceHandle),
            Record(TypeReferenceHandle),
            Boolean,
        }

        pub(crate) fn value_shape(
            program: &dyn CheckingInput,
            type_reference: TypeReferenceHandle,
        ) -> Option<ValueShape> {
            let mut reference = type_reference;
            for _ in 0..8 {
                if !program
                    .type_reference_table
                    .contains_type_reference(reference)
                {
                    return None;
                }
                match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Reference {
                        referee, access, ..
                    } if *access == ReferenceAccess::Shared => {
                        reference = *referee;
                    }
                    TypeReferenceNode::Reference { .. } => return None,
                    TypeReferenceNode::Constrained { base_type, .. } => {
                        reference = *base_type;
                    }
                    TypeReferenceNode::FixedArray {
                        element_type,
                        length: FixedArrayLength::Literal(length),
                    } => {
                        return (*length <= MAXIMUM_EXTENT
                            && integer_carrier(program, *element_type).is_some())
                        .then_some(ValueShape::Collection {
                            length: Some(*length),
                            element_type: *element_type,
                        });
                    }
                    TypeReferenceNode::Slice { element_type } => {
                        integer_carrier(program, *element_type)?;
                        return Some(ValueShape::Collection {
                            length: None,
                            element_type: *element_type,
                        });
                    }
                    _ if crate::value_custody::places::data_definition_for_type(
                        program, reference,
                    )
                    .is_some() =>
                    {
                        return Some(ValueShape::Record(reference));
                    }
                    _ => {
                        return match program.primitive_type_reference(reference)? {
                            PrimitiveType::Bool => Some(ValueShape::Boolean),
                            _ => integer_carrier(program, reference)
                                .map(|_| ValueShape::Integer(reference)),
                        };
                    }
                }
            }
            None
        }

        fn has_only_carrier_parameters(program: &dyn CheckingInput, entry: &State) -> bool {
            program
                .state_parameters(entry)
                .iter()
                .all(|parameter| is_carrier_type(program, parameter.type_reference, 0))
        }

        fn is_carrier_type(
            program: &dyn CheckingInput,
            mut reference: TypeReferenceHandle,
            depth: usize,
        ) -> bool {
            if depth >= 8 {
                return false;
            }
            for _ in 0..8 {
                match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Reference {
                        referee,
                        access: ReferenceAccess::Shared,
                        ..
                    } => reference = *referee,
                    TypeReferenceNode::FixedArray { element_type, .. }
                    | TypeReferenceNode::Slice { element_type } => {
                        return super::bare_integer_parameter_type(program, *element_type)
                            .is_some();
                    }
                    TypeReferenceNode::Named { .. } => {
                        if super::bare_integer_parameter_type(program, reference).is_some()
                            || program.primitive_type_reference(reference)
                                == Some(PrimitiveType::Bool)
                        {
                            return true;
                        }
                        let Some(definition) =
                            crate::value_custody::places::data_definition_for_type(
                                program, reference,
                            )
                        else {
                            return false;
                        };
                        return definition.quotient.is_none()
                            && program
                                .proof_facts
                                .span_or_empty(definition.requires_facts)
                                .is_empty()
                            && program.data_members(definition).iter().all(|member| {
                                member.as_field().is_some_and(|field| {
                                    is_carrier_type(program, field.type_reference, depth + 1)
                                })
                            });
                    }
                    _ => return false,
                }
            }
            false
        }

        pub(crate) fn admitted_predicate_machine(
            program: &dyn CheckingInput,
            symbol: symbols::SymbolHandle,
            entered_by_call: bool,
        ) -> Option<usize> {
            if !symbol.is_valid() {
                return None;
            }
            let (index, machine) = program.machines().iter().enumerate().find(|(_, machine)| {
                machine.symbol == symbol
                    || program
                        .machine_states(machine)
                        .first()
                        .is_some_and(|entry| entry.symbol == symbol)
            })?;
            let entry = program.machine_states(machine).first()?;
            let pure = machine.attached_data.is_none()
                && machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
                && machine.body_is_present
                && !machine.structural_type_equations_pending
                && !machine.suspends
                && !machine.blocks
                && machine.type_parameters.is_empty()
                && program.machine_owned_data(machine).is_empty()
                && program
                    .service_reach_rows
                    .services(machine.service_reach_row)
                    .is_empty();
            let returns_boolean =
                program.primitive_type_reference(entry.return_type) == Some(PrimitiveType::Bool);
            let unconditional = entered_by_call
                || (has_only_carrier_parameters(program, entry)
                    && program
                        .machine_contracts(machine)
                        .iter()
                        .chain(program.state_contracts(entry))
                        .all(|contract| !matches!(contract.kind, SignatureContractKind::Requires)));
            let parameters_readable = program.machine_states(machine).iter().all(|state| {
                program
                    .state_parameters(state)
                    .iter()
                    .all(|parameter| !parameter.is_mutable)
            });
            (pure && returns_boolean && unconditional && parameters_readable).then_some(index)
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct FrameKey {
            machine: usize,
            state: usize,
            bindings: Vec<Binding>,
        }

        pub(crate) enum Exit {
            Result(bool, Vec<IntegerSet>),
            Enter(FrameKey, Vec<IntegerSet>),
        }

        pub(crate) enum Stop {
            Split {
                atom: usize,
                parts: [IntegerSet; 2],
            },
            Call {
                expression: ExpressionHandle,
                machine: usize,
                state: usize,
                bindings: Vec<Binding>,
            },
            Unsupported,
        }

        #[derive(Debug, Clone)]
        pub(crate) enum Value {
            Boolean(bool),
            CollectionExtent {
                atom: usize,
            },
            Integer {
                set: IntegerSet,
                atom: Option<usize>,
                carrier: Option<TypeReferenceHandle>,
            },
            Array {
                first: usize,
                length: usize,
                element_type: TypeReferenceHandle,
            },
        }

        #[derive(Clone, Copy)]
        pub(crate) enum Scope<'scope> {
            State {
                machine: &'scope Machine,
                state: &'scope State,
            },
            Declaration {
                owner: symbols::SymbolHandle,
                parameters: &'scope [StateParameter],
            },
        }

        impl Scope<'_> {
            fn owner(self) -> symbols::SymbolHandle {
                match self {
                    Self::State { machine, .. } => machine.symbol,
                    Self::Declaration { owner, .. } => owner,
                }
            }

            fn parameters<'a>(&'a self, program: &'a dyn CheckingInput) -> &'a [StateParameter] {
                match self {
                    Self::State { state, .. } => program.state_parameters(state),
                    Self::Declaration { parameters, .. } => parameters,
                }
            }

            fn expression_type(
                self,
                program: &dyn CheckingInput,
                expression: ExpressionHandle,
            ) -> Option<TypeReferenceHandle> {
                match self {
                    Self::State { machine, state } => {
                        crate::expression_result_type_reference(program, machine, state, expression)
                    }
                    Self::Declaration { owner, parameters } => {
                        crate::parameter_expression_result_type_reference(
                            program, owner, parameters, expression,
                        )
                    }
                }
            }

            fn builtin_binary(
                self,
                program: &dyn CheckingInput,
                expression: ExpressionHandle,
            ) -> bool {
                match self {
                    Self::State { machine, state } => has_builtin_binary_expression_meaning(
                        program,
                        machine,
                        Some(state),
                        expression,
                    ),
                    Self::Declaration { owner, .. } => {
                        let Some(binary) = expression_table(program).binary_node(expression) else {
                            return false;
                        };
                        let Some(spelling) =
                            typed_trees::operator::binary_operator_spelling(binary.operator)
                        else {
                            return false;
                        };
                        let references = [binary.left, binary.right]
                            .map(|operand| self.expression_type(program, operand));
                        typed_trees::operator::has_builtin_spelled_expression_meaning(
                            program,
                            owner,
                            expression,
                            spelling,
                            &references,
                        )
                    }
                }
            }
        }

        pub(crate) struct Frame<'scope> {
            pub(crate) scope: Scope<'scope>,
            pub(crate) bindings: &'scope [Binding],
            pub(crate) locals: &'scope [(symbols::SymbolHandle, Binding)],
            pub(crate) completed_calls: &'scope [(ExpressionHandle, bool)],
        }

        pub(crate) fn explore(
            program: &dyn CheckingInput,
            machine: usize,
            bindings: Vec<Binding>,
            atoms: Vec<IntegerSet>,
        ) -> Option<Outcomes> {
            let mut remaining = WORK_BUDGET;
            explore_at(program, machine, 0, bindings, atoms, &mut remaining, 0)
        }

        pub(crate) fn explore_expression(
            program: &dyn CheckingInput,
            owner: symbols::SymbolHandle,
            parameters: &[StateParameter],
            expression: ExpressionHandle,
            bindings: Vec<Binding>,
            atoms: Vec<IntegerSet>,
        ) -> Option<Outcomes> {
            let mut remaining = WORK_BUDGET;
            let mut pending = vec![(atoms, Vec::new())];
            let mut outcomes = Outcomes {
                when_true: None,
                when_false: None,
            };
            while let Some((atoms, completed_calls)) = pending.pop() {
                remaining = remaining.checked_sub(1)?;
                let frame = Frame {
                    scope: Scope::Declaration { owner, parameters },
                    bindings: &bindings,
                    locals: &[],
                    completed_calls: &completed_calls,
                };
                match evaluate_boolean(program, &frame, &atoms, expression) {
                    Ok(value) => {
                        let result = if value {
                            &mut outcomes.when_true
                        } else {
                            &mut outcomes.when_false
                        };
                        *result = Some(match result.take() {
                            Some(previous) => join(&previous, &atoms)?,
                            None => atoms,
                        });
                    }
                    Err(stop) => pending.extend(resume_after_stop(
                        program,
                        stop,
                        atoms,
                        &completed_calls,
                        &mut remaining,
                        0,
                    )?),
                }
            }
            Some(outcomes)
        }

        pub(crate) type ResumedObservation = (Vec<IntegerSet>, Vec<(ExpressionHandle, bool)>);

        fn resume_after_stop(
            program: &dyn CheckingInput,
            stop: Stop,
            atoms: Vec<IntegerSet>,
            completed_calls: &[(ExpressionHandle, bool)],
            remaining: &mut usize,
            nesting: usize,
        ) -> Option<Vec<ResumedObservation>> {
            match stop {
                Stop::Split { atom, parts } => parts
                    .into_iter()
                    .map(|part| {
                        let mut refined = atoms.clone();
                        *refined.get_mut(atom)? = part;
                        Some((refined, completed_calls.to_vec()))
                    })
                    .collect(),
                Stop::Call {
                    expression,
                    machine,
                    state,
                    bindings,
                } => {
                    let called = explore_at(
                        program,
                        machine,
                        state,
                        bindings,
                        atoms,
                        remaining,
                        nesting + 1,
                    )?;
                    Some(
                        [(true, called.when_true), (false, called.when_false)]
                            .into_iter()
                            .filter_map(|(value, atoms)| {
                                let atoms = atoms?;
                                let mut completed = completed_calls.to_vec();
                                completed.push((expression, value));
                                Some((atoms, completed))
                            })
                            .collect(),
                    )
                }
                Stop::Unsupported => None,
            }
        }

        fn explore_at(
            program: &dyn CheckingInput,
            machine: usize,
            state: usize,
            bindings: Vec<Binding>,
            atoms: Vec<IntegerSet>,
            remaining: &mut usize,
            nesting: usize,
        ) -> Option<Outcomes> {
            if nesting >= 64 {
                return None;
            }
            *remaining = remaining.checked_sub(1)?;
            let mut frames: Vec<(FrameKey, Vec<IntegerSet>)> = Vec::new();
            let mut pending: VecDeque<usize> = VecDeque::new();
            let mut outcomes = Outcomes {
                when_true: None,
                when_false: None,
            };
            frames.push((
                FrameKey {
                    machine,
                    state,
                    bindings,
                },
                atoms,
            ));
            pending.push_back(0);
            while let Some(frame_index) = pending.pop_front() {
                let (key, atoms) = frames[frame_index].clone();
                for exit in visit(program, &key, atoms, remaining, nesting)? {
                    match exit {
                        Exit::Result(result, atoms) => {
                            let joined = if result {
                                &mut outcomes.when_true
                            } else {
                                &mut outcomes.when_false
                            };
                            *joined = Some(match joined.take() {
                                None => atoms,
                                Some(existing) => join(&existing, &atoms)?,
                            });
                        }
                        Exit::Enter(target, atoms) => {
                            *remaining = remaining.checked_sub(1)?;
                            match frames.iter().position(|(existing, _)| *existing == target) {
                                None => {
                                    frames.push((target, atoms));
                                    pending.push_back(frames.len() - 1);
                                }
                                Some(position) => {
                                    let joined = join(&frames[position].1, &atoms)?;
                                    if joined != frames[position].1 {
                                        frames[position].1 = joined;
                                        if !pending.contains(&position) {
                                            pending.push_back(position);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Some(outcomes)
        }

        fn join(left: &[IntegerSet], right: &[IntegerSet]) -> Option<Vec<IntegerSet>> {
            left.iter()
                .zip(right)
                .map(|(left, right)| {
                    let joined = left.union(right);
                    (joined.range_count() <= MAXIMUM_RANGES).then_some(joined)
                })
                .collect()
        }

        fn visit(
            program: &dyn CheckingInput,
            key: &FrameKey,
            atoms: Vec<IntegerSet>,
            remaining: &mut usize,
            nesting: usize,
        ) -> Option<Vec<Exit>> {
            let machine = program.machines().get(key.machine)?;
            let state = program.machine_states(machine).get(key.state)?;
            if program.state_parameters(state).len() != key.bindings.len() {
                return None;
            }
            let statements = program.statement_table.statements(state.statement_nodes);
            let mut exits = Vec::new();
            let mut stack = vec![(0usize, atoms, Vec::new(), Vec::new())];
            while let Some((index, atoms, locals, completed_calls)) = stack.pop() {
                *remaining = remaining.checked_sub(1)?;
                let frame = Frame {
                    scope: Scope::State { machine, state },
                    bindings: &key.bindings,
                    locals: &locals,
                    completed_calls: &completed_calls,
                };
                let step = match statements.get(index)? {
                    StatementNode::LocalData(local)
                        if !local.is_mutable
                            && local.ordinary_value_initializer().is_some()
                            && local.relevance == language_core::BindingRelevance::Relevant =>
                    {
                        match argument_binding(
                            program,
                            &frame,
                            &atoms,
                            local.initial_value,
                            local.type_reference,
                        ) {
                            Ok(binding) => {
                                let mut declared = locals.clone();
                                declared.push((local.symbol, binding));
                                stack.push((index + 1, atoms, declared, completed_calls));
                                continue;
                            }
                            Err(stop) => Err(stop),
                        }
                    }
                    StatementNode::Transition(transition)
                        if transition.exit == TransitionExit::Ordinary
                            && !transition.continuation.is_valid() =>
                    {
                        let taken = match transition.guard {
                            TransitionGuardNode::Always => Ok(true),
                            TransitionGuardNode::When(guard) => {
                                evaluate_boolean(program, &frame, &atoms, guard)
                            }
                        };
                        match taken {
                            Ok(false) => {
                                stack.push((index + 1, atoms, locals, completed_calls));
                                continue;
                            }
                            Ok(true) => {
                                target_exit(program, &frame, key, &atoms, transition.target)
                            }
                            Err(stop) => Err(stop),
                        }
                    }
                    StatementNode::Expression(value) if index + 1 == statements.len() => {
                        evaluate_boolean(program, &frame, &atoms, *value)
                            .map(|value| Exit::Result(value, atoms.clone()))
                    }
                    _ => return None,
                };
                match step {
                    Ok(exit) => exits.push(exit),
                    Err(stop) => {
                        for (atoms, completed) in resume_after_stop(
                            program,
                            stop,
                            atoms,
                            &completed_calls,
                            remaining,
                            nesting,
                        )? {
                            stack.push((index, atoms, locals.clone(), completed));
                        }
                    }
                }
            }
            Some(exits)
        }

        fn target_exit(
            program: &dyn CheckingInput,
            frame: &Frame<'_>,
            key: &FrameKey,
            atoms: &[IntegerSet],
            target: TransitionTargetHandle,
        ) -> Result<Exit, Stop> {
            if !target.is_valid() {
                return Err(Stop::Unsupported);
            }
            match program.statement_table.transition_target(target) {
                TransitionTargetNode::Value(result) => {
                    if let Some(call) = expression_table(program).call_node(*result) {
                        return tail_call_exit(program, frame, atoms, call);
                    }
                    let result = evaluate_boolean(program, frame, atoms, *result)?;
                    Ok(Exit::Result(result, atoms.to_vec()))
                }
                TransitionTargetNode::Named {
                    path,
                    arguments,
                    static_machine_parameter,
                    evidence_arguments,
                    ..
                } => {
                    if static_machine_parameter.is_valid() || !evidence_arguments.is_empty() {
                        return Err(Stop::Unsupported);
                    }
                    let (machine, state) =
                        named_target(program, key.machine, path.symbol).ok_or(Stop::Unsupported)?;
                    let arguments = program.statement_table.expression_handles(*arguments);
                    enter(program, frame, atoms, machine, state, arguments)
                }
                TransitionTargetNode::Terminal => Err(Stop::Unsupported),
            }
        }

        fn tail_call_exit(
            program: &dyn CheckingInput,
            frame: &Frame<'_>,
            atoms: &[IntegerSet],
            call: &typed_trees::expression::TableCallExpression,
        ) -> Result<Exit, Stop> {
            if !call.is_ordinary_positional_call() {
                return Err(Stop::Unsupported);
            }
            let checked_caller = matches!(frame.scope, Scope::State { .. });
            let machine = admitted_predicate_machine(program, call.target_symbol, checked_caller)
                .ok_or(Stop::Unsupported)?;
            let arguments = expression_table(program).expression_handles(call.arguments);
            enter(program, frame, atoms, machine, 0, arguments)
        }

        fn enter(
            program: &dyn CheckingInput,
            frame: &Frame<'_>,
            atoms: &[IntegerSet],
            machine: usize,
            state: usize,
            arguments: &[ExpressionHandle],
        ) -> Result<Exit, Stop> {
            let target_machine = program.machines().get(machine).ok_or(Stop::Unsupported)?;
            let target_state = program
                .machine_states(target_machine)
                .get(state)
                .ok_or(Stop::Unsupported)?;
            let parameters = program.state_parameters(target_state);
            if parameters.len() != arguments.len() {
                return Err(Stop::Unsupported);
            }
            let mut bindings = Vec::with_capacity(arguments.len());
            for (parameter, argument) in parameters.iter().zip(arguments) {
                bindings.push(argument_binding(
                    program,
                    frame,
                    atoms,
                    *argument,
                    parameter.type_reference,
                )?);
            }
            Ok(Exit::Enter(
                FrameKey {
                    machine,
                    state,
                    bindings,
                },
                atoms.to_vec(),
            ))
        }

        fn named_target(
            program: &dyn CheckingInput,
            current_machine: usize,
            symbol: symbols::SymbolHandle,
        ) -> Option<(usize, usize)> {
            if let Some(state) = typed_trees::state::state_position(
                program.machine_states(program.machines().get(current_machine)?),
                symbol,
            )
            .filter(|position| *position != 0)
            {
                return Some((current_machine, state));
            }
            admitted_predicate_machine(program, symbol, true).map(|machine| (machine, 0))
        }

        fn argument_binding(
            program: &dyn CheckingInput,
            frame: &Frame<'_>,
            atoms: &[IntegerSet],
            argument: ExpressionHandle,
            parameter_type: TypeReferenceHandle,
        ) -> Result<Binding, Stop> {
            let shape = value_shape(program, parameter_type).ok_or(Stop::Unsupported)?;
            match (evaluate(program, frame, atoms, argument)?, shape) {
                (
                    Value::Array {
                        first,
                        length,
                        element_type,
                    },
                    ValueShape::Collection {
                        length: expected, ..
                    },
                ) if expected.is_none_or(|expected| length == expected) => Ok(Binding::Array {
                    first,
                    length,
                    element_type,
                }),
                (Value::CollectionExtent { atom }, ValueShape::Collection { length: None, .. }) => {
                    Ok(Binding::CollectionExtent { atom })
                }
                (Value::Integer { set, atom, .. }, ValueShape::Integer(carrier)) => {
                    let (low, high) = integer_carrier(program, carrier).ok_or(Stop::Unsupported)?;
                    if !set.is_subset_of(&IntegerSet::range(low, high)) {
                        return Err(Stop::Unsupported);
                    }
                    match (set.single_value(), atom) {
                        (Some(value), _) => Ok(Binding::Constant { value, carrier }),
                        (None, Some(atom)) => Ok(Binding::Atom { atom, carrier }),
                        (None, None) => Err(Stop::Unsupported),
                    }
                }
                (Value::Boolean(value), ValueShape::Boolean) => Ok(Binding::Boolean(value)),
                _ => Err(Stop::Unsupported),
            }
        }

        pub(crate) fn evaluate_boolean(
            program: &dyn CheckingInput,
            frame: &Frame<'_>,
            atoms: &[IntegerSet],
            expression: ExpressionHandle,
        ) -> Result<bool, Stop> {
            match evaluate(program, frame, atoms, expression)? {
                Value::Boolean(value) => Ok(value),
                _ => Err(Stop::Unsupported),
            }
        }

        pub(crate) fn evaluate(
            program: &dyn CheckingInput,
            frame: &Frame<'_>,
            atoms: &[IntegerSet],
            expression: ExpressionHandle,
        ) -> Result<Value, Stop> {
            if !expression_table(program).expression_is_valid(expression) {
                return Err(Stop::Unsupported);
            }
            match expression_table(program).expression(expression) {
                ExpressionNode::Call(call) => {
                    if let Some((_, value)) = frame
                        .completed_calls
                        .iter()
                        .find(|(captured, _)| *captured == expression)
                    {
                        return Ok(Value::Boolean(*value));
                    }
                    let Exit::Enter(target, _) = tail_call_exit(program, frame, atoms, call)?
                    else {
                        return Err(Stop::Unsupported);
                    };
                    Err(Stop::Call {
                        expression,
                        machine: target.machine,
                        state: target.state,
                        bindings: target.bindings,
                    })
                }
                ExpressionNode::Member(member)
                    if super::length_member(program, expression).is_some() =>
                {
                    let (set, atom) = match evaluate(program, frame, atoms, member.receiver)? {
                        Value::Array { length, .. } => {
                            (IntegerSet::singleton(length as i128), None)
                        }
                        Value::CollectionExtent { atom } => (
                            atoms.get(atom).ok_or(Stop::Unsupported)?.clone(),
                            Some(atom),
                        ),
                        _ => return Err(Stop::Unsupported),
                    };
                    Ok(Value::Integer {
                set,
                atom,
                carrier: Some(
                    crate::proof_contracts::bound_expression_meaning::builtin_type_reference(
                        program,
                        symbols::BuiltinTypeAtom::U64,
                    )
                    .ok_or(Stop::Unsupported)?,
                ),
            })
                }
                ExpressionNode::Boolean(value) => Ok(Value::Boolean(*value)),
                ExpressionNode::Integer(literal) => Ok(Value::Integer {
                    set: IntegerSet::singleton(i128::from(
                        literal.value_i64().ok_or(Stop::Unsupported)?,
                    )),
                    atom: None,
                    carrier: None,
                }),
                ExpressionNode::Name(path) => {
                    if expression_table(program)
                        .name_path_members(path.members)
                        .len()
                        != 1
                        || !path.symbol.is_valid()
                    {
                        return Err(Stop::Unsupported);
                    }
                    let binding = match frame
                        .locals
                        .iter()
                        .rev()
                        .find(|(symbol, _)| *symbol == path.symbol)
                    {
                        Some((_, binding)) => binding,
                        None => {
                            let position = typed_trees::signature::state_parameter_position(
                                frame.scope.parameters(program),
                                path.symbol,
                            )
                            .ok_or(Stop::Unsupported)?;
                            frame.bindings.get(position).ok_or(Stop::Unsupported)?
                        }
                    };
                    Ok(match binding {
                        Binding::Array {
                            first,
                            length,
                            element_type,
                        } => Value::Array {
                            first: *first,
                            length: *length,
                            element_type: *element_type,
                        },
                        Binding::CollectionExtent { atom } => {
                            Value::CollectionExtent { atom: *atom }
                        }
                        Binding::Atom { atom, carrier } => Value::Integer {
                            set: atoms.get(*atom).ok_or(Stop::Unsupported)?.clone(),
                            atom: Some(*atom),
                            carrier: Some(*carrier),
                        },
                        Binding::Constant { value, carrier } => Value::Integer {
                            set: IntegerSet::singleton(*value),
                            atom: None,
                            carrier: Some(*carrier),
                        },
                        Binding::Boolean(value) => Value::Boolean(*value),
                    })
                }
                ExpressionNode::Borrow(borrow) if borrow.access == ReferenceAccess::Shared => {
                    match evaluate(program, frame, atoms, borrow.target)? {
                        array @ (Value::Array { .. } | Value::CollectionExtent { .. }) => Ok(array),
                        _ => Err(Stop::Unsupported),
                    }
                }
                ExpressionNode::Indexed(indexed) => {
                    let references = [indexed.collection, indexed.index]
                        .map(|operand| frame.scope.expression_type(program, operand));
                    if !typed_trees::operator::builtin_index_read_is_spelled(
                        program,
                        frame.scope.owner(),
                        expression,
                        &references,
                    ) {
                        return Err(Stop::Unsupported);
                    }
                    let Value::Array {
                        first,
                        length,
                        element_type,
                    } = evaluate(program, frame, atoms, indexed.collection)?
                    else {
                        return Err(Stop::Unsupported);
                    };
                    let Value::Integer { set, atom, .. } =
                        evaluate(program, frame, atoms, indexed.index)?
                    else {
                        return Err(Stop::Unsupported);
                    };
                    let Some(position) = set.single_value() else {
                        let (Some(atom), Some((low, _))) = (atom, set.bounds()) else {
                            return Err(Stop::Unsupported);
                        };
                        let low_part = IntegerSet::singleton(low);
                        return Err(Stop::Split {
                            atom,
                            parts: [set.difference(&low_part), low_part],
                        });
                    };
                    let element = usize::try_from(position)
                        .ok()
                        .filter(|element| *element < length)
                        .ok_or(Stop::Unsupported)?;
                    Ok(Value::Integer {
                        set: atoms.get(first + element).ok_or(Stop::Unsupported)?.clone(),
                        atom: Some(first + element),
                        carrier: Some(element_type),
                    })
                }
                ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => Ok(
                    Value::Boolean(!evaluate_boolean(program, frame, atoms, unary.operand)?),
                ),
                ExpressionNode::Binary(binary) => {
                    evaluate_binary(program, frame, atoms, expression, binary)
                }
                _ => Err(Stop::Unsupported),
            }
        }

        fn evaluate_binary(
            program: &dyn CheckingInput,
            frame: &Frame<'_>,
            atoms: &[IntegerSet],
            expression: ExpressionHandle,
            binary: &typed_trees::expression::TableBinaryExpression,
        ) -> Result<Value, Stop> {
            match binary.operator {
                BinaryOperator::And => Ok(Value::Boolean(
                    evaluate_boolean(program, frame, atoms, binary.left)?
                        && evaluate_boolean(program, frame, atoms, binary.right)?,
                )),
                BinaryOperator::Or => Ok(Value::Boolean(
                    evaluate_boolean(program, frame, atoms, binary.left)?
                        || evaluate_boolean(program, frame, atoms, binary.right)?,
                )),
                _ => {
                    if !frame.scope.builtin_binary(program, expression) {
                        return Err(Stop::Unsupported);
                    }
                    let left = evaluate(program, frame, atoms, binary.left)?;
                    let right = evaluate(program, frame, atoms, binary.right)?;
                    match binary.operator {
                        BinaryOperator::Equal
                        | BinaryOperator::NotEqual
                        | BinaryOperator::Less
                        | BinaryOperator::LessOrEqual
                        | BinaryOperator::Greater
                        | BinaryOperator::GreaterOrEqual => compare(binary.operator, left, right),
                        BinaryOperator::Add | BinaryOperator::Subtract => {
                            exact_index_arithmetic(program, binary.operator, left, right)
                        }
                        _ => Err(Stop::Unsupported),
                    }
                }
            }
        }

        fn compare(operator: BinaryOperator, left: Value, right: Value) -> Result<Value, Stop> {
            match (left, right) {
                (Value::Boolean(left), Value::Boolean(right)) => match operator {
                    BinaryOperator::Equal => Ok(Value::Boolean(left == right)),
                    BinaryOperator::NotEqual => Ok(Value::Boolean(left != right)),
                    _ => Err(Stop::Unsupported),
                },
                (
                    Value::Integer {
                        set: left,
                        atom: left_atom,
                        ..
                    },
                    Value::Integer {
                        set: right,
                        atom: right_atom,
                        ..
                    },
                ) => {
                    if let Some(bound) = right.single_value() {
                        return refine(operator, left, left_atom, bound);
                    }
                    if let Some(bound) = left.single_value() {
                        return refine(operator.mirrored(), right, right_atom, bound);
                    }
                    decided_by_bounds(operator, &left, &right)
                        .map(Value::Boolean)
                        .ok_or(Stop::Unsupported)
                }
                _ => Err(Stop::Unsupported),
            }
        }

        fn refine(
            operator: BinaryOperator,
            subject: IntegerSet,
            atom: Option<usize>,
            bound: i128,
        ) -> Result<Value, Stop> {
            let satisfying = IntegerSet::satisfying(operator, bound).ok_or(Stop::Unsupported)?;
            let holds = subject.intersect(&satisfying);
            let fails = subject.difference(&satisfying);
            if fails.is_empty() {
                return Ok(Value::Boolean(true));
            }
            if holds.is_empty() {
                return Ok(Value::Boolean(false));
            }
            let atom = atom.ok_or(Stop::Unsupported)?;
            if holds.range_count() > MAXIMUM_RANGES || fails.range_count() > MAXIMUM_RANGES {
                return Err(Stop::Unsupported);
            }
            Err(Stop::Split {
                atom,
                parts: [fails, holds],
            })
        }

        fn decided_by_bounds(
            operator: BinaryOperator,
            left: &IntegerSet,
            right: &IntegerSet,
        ) -> Option<bool> {
            let (left_low, left_high) = left.bounds()?;
            let (right_low, right_high) = right.bounds()?;
            match operator {
                BinaryOperator::Less if left_high < right_low => Some(true),
                BinaryOperator::Less if left_low >= right_high => Some(false),
                BinaryOperator::LessOrEqual if left_high <= right_low => Some(true),
                BinaryOperator::LessOrEqual if left_low > right_high => Some(false),
                BinaryOperator::Greater => decided_by_bounds(BinaryOperator::Less, right, left),
                BinaryOperator::GreaterOrEqual => {
                    decided_by_bounds(BinaryOperator::LessOrEqual, right, left)
                }
                BinaryOperator::Equal | BinaryOperator::NotEqual
                    if left_high < right_low || right_high < left_low =>
                {
                    Some(operator == BinaryOperator::NotEqual)
                }
                _ => None,
            }
        }

        fn exact_index_arithmetic(
            program: &dyn CheckingInput,
            operator: BinaryOperator,
            left: Value,
            right: Value,
        ) -> Result<Value, Stop> {
            let (
                Value::Integer {
                    set: left,
                    carrier: left_carrier,
                    ..
                },
                Value::Integer {
                    set: right,
                    carrier: right_carrier,
                    ..
                },
            ) = (left, right)
            else {
                return Err(Stop::Unsupported);
            };
            let carrier = left_carrier.or(right_carrier).ok_or(Stop::Unsupported)?;
            if [left_carrier, right_carrier]
                .into_iter()
                .flatten()
                .any(|typed| {
                    program.arithmetic_domain_for_type_reference(typed)
                        != numerics::arithmetic::ArithmeticDomain::Exact
                        || program.primitive_type_reference(typed)
                            != program.primitive_type_reference(carrier)
                })
            {
                return Err(Stop::Unsupported);
            }
            let (Some(left), Some(right)) = (left.single_value(), right.single_value()) else {
                return Err(Stop::Unsupported);
            };
            let value = match operator {
                BinaryOperator::Add => left.checked_add(right),
                BinaryOperator::Subtract => left.checked_sub(right),
                _ => None,
            }
            .ok_or(Stop::Unsupported)?;
            let (low, high) = integer_carrier(program, carrier).ok_or(Stop::Unsupported)?;
            if value < low || value > high {
                return Err(Stop::Unsupported);
            }
            Ok(Value::Integer {
                set: IntegerSet::singleton(value),
                atom: None,
                carrier: Some(carrier),
            })
        }
    }
    pub(crate) use crate::proof_contracts::arithmetic_domains::IntegerSet;
    use exploration::{Binding, ValueShape, integer_carrier, value_shape};
    use typed_trees::data::{DataDefinition, DataField, DataMember};
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
    use typed_trees::types::TypeReferenceHandle;
    pub fn length_member(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<&typed_trees::expression::TableMemberExpression> {
        let member = program.expression_table.member_node(expression)?;
        (language_semantics::declaration_selection::CollectionMeasure::from_authored_spelling(
            member.member.as_str(),
        ) == Some(language_semantics::declaration_selection::CollectionMeasure::Length))
        .then_some(member)
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum ArgumentKnowledge {
        Array(Vec<IntegerSet>),
        Integer(IntegerSet),
        CollectionExtent(IntegerSet),
        Boolean(bool),
    }

    pub(crate) struct PredicateApplication<'program> {
        pub(crate) machine: usize,
        pub(crate) arguments: &'program [ExpressionHandle],
    }

    pub(crate) fn predicate_application(
        program: &dyn crate::CheckingInput,
        owner: symbols::SymbolHandle,
        fact: ExpressionHandle,
    ) -> Option<PredicateApplication<'_>> {
        if !program.expression_table.expression_is_valid(fact) {
            return None;
        }
        let call_expression = match program.expression_table.expression(fact) {
            ExpressionNode::Call(_) => fact,
            ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Equal => {
                let call = match (
                    program.expression_table.expression(binary.left),
                    program.expression_table.expression(binary.right),
                ) {
                    (ExpressionNode::Call(_), ExpressionNode::Boolean(true)) => binary.left,
                    (ExpressionNode::Boolean(true), ExpressionNode::Call(_)) => binary.right,
                    _ => return None,
                };
                let boolean =
                    crate::proof_contracts::bound_expression_meaning::builtin_type_reference(
                        program,
                        symbols::BuiltinTypeAtom::Bool,
                    )?;
                if !typed_trees::operator::has_builtin_spelled_expression_meaning(
                    program,
                    owner,
                    fact,
                    language_core::OperatorSpelling::Equal,
                    &[Some(boolean), Some(boolean)],
                ) {
                    return None;
                }
                call
            }
            _ => return None,
        };
        predicate_call(program, call_expression)
    }

    fn predicate_call(
        program: &dyn crate::CheckingInput,
        call_expression: ExpressionHandle,
    ) -> Option<PredicateApplication<'_>> {
        let call = program.expression_table.call_node(call_expression)?;
        if !call.is_ordinary_positional_call() {
            return None;
        }
        let machine = exploration::admitted_predicate_machine(program, call.target_symbol, false)?;
        let arguments = program.expression_table.expression_handles(call.arguments);
        if arguments.len() != call.arguments.len() {
            return None;
        }
        Some(PredicateApplication { machine, arguments })
    }

    pub(crate) fn bare_integer_parameter_type(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Option<typed_trees::types::PrimitiveType> {
        if !program
            .type_reference_table
            .contains_type_reference(reference)
        {
            return None;
        }
        if !matches!(
            program.type_reference_table.type_reference(reference),
            typed_trees::types::TypeReferenceNode::Named { .. }
        ) {
            return None;
        }
        program
            .primitive_type_reference(reference)
            .filter(|primitive| primitive.is_fixed_integer())
    }

    pub(crate) fn closed_integer_predicate_value(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        values: &[(i128, Option<TypeReferenceHandle>)],
    ) -> Option<bool> {
        let application = predicate_call(program, expression)?;
        let machine = program.machines().get(application.machine)?;
        let states = program.machine_states(machine);
        if states.len() != machine.states.len() {
            return None;
        }
        let entry = states.first()?;
        let parameters = program.state_parameters(entry);
        if parameters.len() != entry.parameters.len()
            || parameters.len() != values.len()
            || parameters.len() != application.arguments.len()
        {
            return None;
        }
        let knowledge = parameters
            .iter()
            .zip(application.arguments)
            .zip(values)
            .map(|((parameter, actual), (value, actual_type))| {
                let primitive = bare_integer_parameter_type(program, parameter.type_reference)?;
                if let Some(actual_type) = actual_type {
                    if program.primitive_type_reference(*actual_type) != Some(primitive) {
                        return None;
                    }
                } else {
                    let literal = program.expression_table.integer_node(*actual)?;
                    if literal.landing().is_some_and(|landing| {
                        typed_trees::types::PrimitiveType::from_name(landing.landed_type.name())
                            != Some(primitive)
                            || landing.domain != numerics::arithmetic::ArithmeticDomain::Exact
                    }) {
                        return None;
                    }
                }
                let (minimum, maximum) = primitive.integer_bounds()?;
                if *value < minimum || *value > maximum {
                    return None;
                }
                Some(ArgumentKnowledge::Integer(IntegerSet::singleton(*value)))
            })
            .collect::<Option<Vec<_>>>()?;
        unfold(program, application.machine, &knowledge)?.decided()
    }

    #[derive(Clone)]
    pub struct PredicateIntegerSubject {
        pub expression: ExpressionHandle,
        pub projection: Vec<facts::PlaceSegment>,
    }

    #[derive(Clone)]
    pub struct PredicateIntegerReadSubject {
        pub predicate: symbols::SymbolHandle,
        pub expression: ExpressionHandle,
        pub arguments: Vec<ExpressionHandle>,
    }

    #[derive(Clone)]
    pub enum PredicateIntegerOperand {
        Argument(PredicateIntegerSubject),
        Read(PredicateIntegerReadSubject),
        Integer(i128),
    }

    pub fn predicate_integer_operand(
        program: &dyn crate::CheckingInput,
        predicate: symbols::SymbolHandle,
        arguments: &[ExpressionHandle],
        expression: ExpressionHandle,
    ) -> Option<PredicateIntegerOperand> {
        let position = exploration::admitted_predicate_machine(program, predicate, false)?;
        let machine = &program.machines()[position];
        let entry = program.machine_states(machine).first()?;
        if program.state_parameters(entry).len() != arguments.len() {
            return None;
        }
        if let ExpressionNode::Integer(literal) = program.expression_table.expression(expression) {
            return Some(PredicateIntegerOperand::Integer(
                literal.decimal_value_i128()?,
            ));
        }
        if program.expression_table.indexed_node(expression).is_some() {
            if !crate::immutable_entry_indexed_read_identity(program, expression)?
                .has_scope(machine.symbol, entry.symbol)
            {
                return None;
            }
            return Some(PredicateIntegerOperand::Read(PredicateIntegerReadSubject {
                predicate,
                expression,
                arguments: arguments.to_vec(),
            }));
        }
        let (position, projection) =
            crate::immutable_entry_integer_projection(program, predicate, expression)?;
        Some(PredicateIntegerOperand::Argument(PredicateIntegerSubject {
            expression: *arguments.get(position)?,
            projection,
        }))
    }

    pub use crate::proof_contracts::arithmetic_domains::PredicateInterval;

    pub fn exact_predicate_intervals(
        program: &dyn crate::CheckingInput,
        owner: symbols::SymbolHandle,
        expression: ExpressionHandle,
    ) -> Option<Vec<PredicateInterval>> {
        let application = predicate_application(program, owner, expression)?;
        let rows = interval_consequences(program, &application)?;
        if rows.len() != application.arguments.len() {
            return None;
        }
        let narrowed = rows
            .iter()
            .map(|row| {
                let hull = IntegerSet::range(row.low, row.high);
                if row.extent {
                    ArgumentKnowledge::CollectionExtent(hull)
                } else {
                    ArgumentKnowledge::Integer(hull)
                }
            })
            .collect::<Vec<_>>();
        (unfold(program, application.machine, &narrowed)?.decided() == Some(true)).then_some(rows)
    }

    pub(crate) fn interval_consequences(
        program: &dyn crate::CheckingInput,
        application: &PredicateApplication<'_>,
    ) -> Option<Vec<PredicateInterval>> {
        let entry = program
            .machine_states(&program.machines()[application.machine])
            .first()?;
        let knowledge = program
            .state_parameters(entry)
            .iter()
            .map(|parameter| unknown_knowledge(program, parameter.type_reference))
            .collect::<Option<Vec<_>>>()?;
        if knowledge.len() != application.arguments.len() {
            return None;
        }
        let unfolding = unfold(program, application.machine, &knowledge)?;
        let mut rows = Vec::new();
        for (position, (argument, knowledge)) in
            application.arguments.iter().zip(&knowledge).enumerate()
        {
            let extent = match knowledge {
                ArgumentKnowledge::Integer(_) => false,
                ArgumentKnowledge::CollectionExtent(_) => true,
                _ => continue,
            };
            let Some(values) = unfolding.when_true(position) else {
                continue;
            };
            let [values] = values.as_slice() else {
                continue;
            };
            let Some((low, high)) = values.bounds() else {
                continue;
            };
            rows.push(PredicateInterval {
                argument: *argument,
                extent,
                low,
                high,
            });
        }
        Some(rows)
    }

    pub(crate) struct Unfolding {
        outcomes: exploration::Outcomes,
        layout: Vec<std::ops::Range<usize>>,
    }

    impl Unfolding {
        pub(crate) fn decided(&self) -> Option<bool> {
            match (&self.outcomes.when_true, &self.outcomes.when_false) {
                (Some(_), None) => Some(true),
                (None, Some(_)) => Some(false),
                _ => None,
            }
        }

        pub(crate) fn when_true(&self, position: usize) -> Option<Vec<IntegerSet>> {
            let atoms = self.outcomes.when_true.as_ref()?;
            Some(atoms.get(self.layout.get(position)?.clone())?.to_vec())
        }
    }

    pub(crate) fn unfold(
        program: &dyn crate::CheckingInput,
        machine: usize,
        arguments: &[ArgumentKnowledge],
    ) -> Option<Unfolding> {
        let entry = program
            .machine_states(program.machines().get(machine)?)
            .first()?;
        let parameters = program.state_parameters(entry);
        let (bindings, atoms, layout) = argument_bindings(program, parameters, arguments)?;
        let outcomes = exploration::explore(program, machine, bindings, atoms)?;
        Some(Unfolding { outcomes, layout })
    }

    fn argument_bindings(
        program: &dyn crate::CheckingInput,
        parameters: &[typed_trees::signature::StateParameter],
        arguments: &[ArgumentKnowledge],
    ) -> Option<(Vec<Binding>, Vec<IntegerSet>, Vec<std::ops::Range<usize>>)> {
        if parameters.len() != arguments.len() {
            return None;
        }
        let mut atoms = Vec::new();
        let mut bindings = Vec::with_capacity(arguments.len());
        let mut layout = Vec::with_capacity(arguments.len());
        for (parameter, knowledge) in parameters.iter().zip(arguments) {
            let first = atoms.len();
            match (value_shape(program, parameter.type_reference)?, knowledge) {
                (
                    ValueShape::Collection {
                        length,
                        element_type,
                    },
                    ArgumentKnowledge::Array(elements),
                ) if length.is_none_or(|length| elements.len() == length) => {
                    let (low, high) = integer_carrier(program, element_type)?;
                    let carrier = IntegerSet::range(low, high);
                    atoms.extend(elements.iter().map(|element| element.intersect(&carrier)));
                    bindings.push(Binding::Array {
                        first,
                        length: elements.len(),
                        element_type,
                    });
                }
                (
                    ValueShape::Collection { length: None, .. },
                    ArgumentKnowledge::CollectionExtent(set),
                ) => {
                    atoms.push(set.intersect(&IntegerSet::range(0, i128::from(u64::MAX))));
                    bindings.push(Binding::CollectionExtent { atom: first });
                }
                (ValueShape::Integer(carrier), ArgumentKnowledge::Integer(set)) => {
                    let (low, high) = integer_carrier(program, carrier)?;
                    atoms.push(set.intersect(&IntegerSet::range(low, high)));
                    bindings.push(Binding::Atom {
                        atom: first,
                        carrier,
                    });
                }
                (ValueShape::Boolean, ArgumentKnowledge::Boolean(value)) => {
                    bindings.push(Binding::Boolean(*value));
                }
                _ => return None,
            }
            layout.push(first..atoms.len());
        }
        if atoms.iter().any(IntegerSet::is_empty) {
            return None;
        }
        Some((bindings, atoms, layout))
    }

    pub(crate) fn unknown_knowledge(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<ArgumentKnowledge> {
        Some(match value_shape(program, type_reference)? {
            ValueShape::Collection {
                length: Some(length),
                element_type,
            } => {
                let (low, high) = integer_carrier(program, element_type)?;
                ArgumentKnowledge::Array(vec![IntegerSet::range(low, high); length])
            }
            ValueShape::Integer(carrier) => {
                let (low, high) = integer_carrier(program, carrier)?;
                ArgumentKnowledge::Integer(IntegerSet::range(low, high))
            }
            ValueShape::Collection { length: None, .. } => {
                ArgumentKnowledge::CollectionExtent(IntegerSet::range(0, i128::from(u64::MAX)))
            }
            ValueShape::Boolean | ValueShape::Record(_) => return None,
        })
    }

    fn zero_knowledge(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<ArgumentKnowledge> {
        Some(match value_shape(program, type_reference)? {
            ValueShape::Collection {
                length: Some(length),
                ..
            } => ArgumentKnowledge::Array(vec![IntegerSet::singleton(0); length]),
            ValueShape::Integer(_) => ArgumentKnowledge::Integer(IntegerSet::singleton(0)),
            ValueShape::Boolean => ArgumentKnowledge::Boolean(false),
            ValueShape::Collection { length: None, .. } | ValueShape::Record(_) => return None,
        })
    }

    pub(crate) fn fact_argument_field<'program>(
        program: &'program dyn crate::CheckingInput,
        definition: &DataDefinition,
        argument: ExpressionHandle,
    ) -> Option<&'program DataField> {
        let symbol =
            crate::queries::expression_traversal::single_member_argument_symbol(program, argument)?;
        program
            .data_members(definition)
            .iter()
            .find_map(|member| match member {
                DataMember::Field(field) if field.symbol == symbol => Some(field),
                _ => None,
            })
    }

    fn data_fact_unfolding(
        program: &dyn crate::CheckingInput,
        definition: &DataDefinition,
        fact: ExpressionHandle,
        field_knowledge: &mut dyn FnMut(&DataField) -> Option<ArgumentKnowledge>,
    ) -> Option<(Unfolding, Vec<symbols::SymbolHandle>)> {
        let demanded = crate::proof_contracts::default_domains::data_predicate_fields(
            program,
            definition,
            &typed_trees::domain::ProofFact::Expression(fact),
        );
        let fields = program
            .data_members(definition)
            .iter()
            .filter_map(|member| member.as_field())
            .filter(|field| demanded.contains(&field.symbol))
            .collect::<Vec<_>>();
        let parameters = fields
            .iter()
            .map(|field| typed_trees::signature::StateParameter {
                symbol: field.symbol,
                name: field.name.clone(),
                type_reference: field.type_reference,
                ..Default::default()
            })
            .collect::<Vec<_>>();
        let knowledge = fields
            .iter()
            .map(|field| field_knowledge(field))
            .collect::<Option<Vec<_>>>()?;
        let (bindings, atoms, layout) = argument_bindings(program, &parameters, &knowledge)?;
        let outcomes = exploration::explore_expression(
            program,
            definition.symbol,
            &parameters,
            fact,
            bindings,
            atoms,
        )?;
        Some((
            Unfolding { outcomes, layout },
            fields.iter().map(|field| field.symbol).collect(),
        ))
    }

    pub(crate) fn data_fact_holds_at_zero(
        program: &dyn crate::CheckingInput,
        definition: &DataDefinition,
        fact: ExpressionHandle,
    ) -> bool {
        data_fact_unfolding(program, definition, fact, &mut |field| {
            zero_knowledge(program, field.type_reference)
        })
        .and_then(|(unfolding, _)| unfolding.decided())
            == Some(true)
    }

    pub(crate) fn data_fact_applies_to_field(
        program: &dyn crate::CheckingInput,
        definition: &DataDefinition,
        fact: ExpressionHandle,
        field: symbols::SymbolHandle,
    ) -> Option<bool> {
        Some(
            crate::proof_contracts::default_domains::data_predicate_fields(
                program,
                definition,
                &typed_trees::domain::ProofFact::Expression(fact),
            )
            .contains(&field),
        )
    }

    pub(crate) fn data_fact_decided(
        program: &dyn crate::CheckingInput,
        definition: &DataDefinition,
        fact: ExpressionHandle,
        field_knowledge: &mut dyn FnMut(&DataField) -> Option<ArgumentKnowledge>,
    ) -> Option<bool> {
        data_fact_unfolding(program, definition, fact, field_knowledge)?
            .0
            .decided()
    }

    pub(crate) fn standing_field_elements(
        program: &dyn crate::CheckingInput,
        definition: &DataDefinition,
        field: &DataField,
    ) -> Option<Vec<IntegerSet>> {
        let ArgumentKnowledge::Array(mut elements) =
            unknown_knowledge(program, field.type_reference)?
        else {
            return None;
        };
        let mut constrained = false;
        for fact in program.conjunctive_proof_facts(definition.requires_facts) {
            let Some(fact) = fact.as_expression() else {
                continue;
            };
            let Some((unfolding, fields)) =
                data_fact_unfolding(program, definition, *fact, &mut |argument| {
                    unknown_knowledge(program, argument.type_reference)
                })
            else {
                continue;
            };
            for (position, argument_field) in fields.iter().enumerate() {
                if *argument_field != field.symbol {
                    continue;
                }
                let Some(implied) = unfolding.when_true(position) else {
                    continue;
                };
                if implied.len() != elements.len() {
                    continue;
                }
                for (element, implied) in elements.iter_mut().zip(implied) {
                    *element = element.intersect(&implied);
                }
                constrained = true;
            }
        }
        constrained.then_some(elements)
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum ConsequenceShape {
        Array(Vec<IntegerSet>),
        Integer(IntegerSet),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct FieldConsequence {
        pub(crate) field: symbols::SymbolHandle,
        pub(crate) elements: ConsequenceShape,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct FactConsequence {
        pub(crate) fields: Vec<FieldConsequence>,
        pub(crate) exact: bool,
    }

    pub(crate) fn data_fact_element_consequences(
        program: &dyn crate::CheckingInput,
        definition: &DataDefinition,
        fact: ExpressionHandle,
    ) -> Option<FactConsequence> {
        let (unfolding, fields) = data_fact_unfolding(program, definition, fact, &mut |field| {
            unknown_knowledge(program, field.type_reference)
        })?;
        let mut consequences: Vec<(symbols::SymbolHandle, Vec<IntegerSet>, bool)> = Vec::new();
        for (position, field) in fields.iter().enumerate() {
            let implied = unfolding.when_true(position)?;
            let is_array =
                match unknown_knowledge(program, field_type(program, definition, *field)?)? {
                    ArgumentKnowledge::Array(_) => true,
                    ArgumentKnowledge::Integer(_) => false,
                    ArgumentKnowledge::CollectionExtent(_) | ArgumentKnowledge::Boolean(_) => {
                        return None;
                    }
                };
            match consequences
                .iter_mut()
                .find(|(symbol, _, _)| symbol == field)
            {
                Some((_, existing, _)) if existing.len() == implied.len() => {
                    for (element, implied) in existing.iter_mut().zip(&implied) {
                        *element = element.intersect(implied);
                    }
                }
                Some(_) => return None,
                None => consequences.push((*field, implied, is_array)),
            }
        }
        let mut rows = Vec::with_capacity(consequences.len());
        for (field, elements, is_array) in &consequences {
            let intervals = elements
                .iter()
                .map(IntegerSet::interval_hull)
                .collect::<Option<Vec<_>>>()?;
            rows.push(FieldConsequence {
                field: *field,
                elements: if *is_array {
                    ConsequenceShape::Array(intervals)
                } else {
                    ConsequenceShape::Integer(intervals.first()?.clone())
                },
            });
        }
        let exact = data_fact_decided(program, definition, fact, &mut |field| {
            let row = rows.iter().find(|row| row.field == field.symbol)?;
            Some(match &row.elements {
                ConsequenceShape::Array(intervals) => ArgumentKnowledge::Array(intervals.clone()),
                ConsequenceShape::Integer(interval) => ArgumentKnowledge::Integer(interval.clone()),
            })
        }) == Some(true);
        Some(FactConsequence {
            fields: rows,
            exact,
        })
    }

    fn field_type(
        program: &dyn crate::CheckingInput,
        definition: &DataDefinition,
        field: symbols::SymbolHandle,
    ) -> Option<TypeReferenceHandle> {
        program
            .data_members(definition)
            .iter()
            .find_map(|member| match member {
                DataMember::Field(candidate) if candidate.symbol == field => {
                    Some(candidate.type_reference)
                }
                _ => None,
            })
    }
}

pub mod default_domains {
    pub(crate) mod call_summaries {
        use symbols::SymbolHandle;
        use symbols::SymbolKeyMap;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};

        pub(crate) type StateToMachine = SymbolKeyMap<SymbolHandle, SymbolHandle>;

        pub(crate) fn state_to_machine_index(program: &dyn crate::CheckingInput) -> StateToMachine {
            let mut index = StateToMachine::default();
            for machine in program.machines() {
                for state in program.machine_states(machine) {
                    index.entry(state.symbol).or_insert(machine.symbol);
                }
            }
            index
        }
    }
    pub(crate) mod case_requirements {
        use crate::frozen_program::expression_table;
        use typed_trees::data::DataDefinition;
        use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

        pub fn selected_case_requirements(
            program: &dyn crate::CheckingInput,
            definition: &DataDefinition,
            case: symbols::SymbolHandle,
        ) -> Vec<ExpressionHandle> {
            let Some(selected) = case_position(program, case) else {
                return Vec::new();
            };
            let mut requirements = Vec::new();
            for fact in program.proof_facts.span_or_empty(definition.requires_facts) {
                let Some(expression) = fact.as_expression() else {
                    continue;
                };
                let mut clauses = Vec::new();
                flatten(program, *expression, BinaryOperator::And, &mut clauses);
                for clause in clauses {
                    project_clause(program, clause, selected, &mut requirements);
                }
            }
            requirements
        }

        pub fn case_predicate_expressions(
            program: &dyn crate::CheckingInput,
            definition: &DataDefinition,
            case: symbols::SymbolHandle,
        ) -> Vec<ExpressionHandle> {
            let mut requirements = selected_case_requirements(program, definition, case);
            for fact in program.proof_facts.span_or_empty(definition.requires_facts) {
                let Some(expression) = fact.as_expression() else {
                    continue;
                };
                let mut clauses = Vec::new();
                flatten(program, *expression, BinaryOperator::And, &mut clauses);
                for clause in clauses {
                    if !clause_mentions_membership(program, clause) {
                        requirements.push(clause);
                    }
                }
            }
            requirements
        }

        fn clause_mentions_membership(
            program: &dyn crate::CheckingInput,
            clause: ExpressionHandle,
        ) -> bool {
            let mut disjuncts = Vec::new();
            flatten(program, clause, BinaryOperator::Or, &mut disjuncts);
            disjuncts.into_iter().any(|disjunct| {
                let mut atoms = Vec::new();
                flatten(program, disjunct, BinaryOperator::And, &mut atoms);
                atoms
                    .into_iter()
                    .any(|atom| membership_case(program, atom).is_some())
            })
        }

        fn project_clause(
            program: &dyn crate::CheckingInput,
            clause: ExpressionHandle,
            selected: u32,
            requirements: &mut Vec<ExpressionHandle>,
        ) {
            let mut disjuncts = Vec::new();
            flatten(program, clause, BinaryOperator::Or, &mut disjuncts);
            let mut mentions_case = false;
            let mut live = None;
            for disjunct in disjuncts {
                let mut atoms = Vec::new();
                flatten(program, disjunct, BinaryOperator::And, &mut atoms);
                let mut guards = Vec::new();
                let mut residual = Vec::new();
                for atom in atoms {
                    match membership_case(program, atom) {
                        Some(position) => guards.push(position),
                        None => residual.push(atom),
                    }
                }
                mentions_case |= !guards.is_empty();
                if guards.iter().any(|position| *position != selected) {
                    continue;
                }
                if live.is_some() {
                    return;
                }
                live = Some(residual);
            }
            if mentions_case && let Some(residual) = live {
                requirements.extend(residual);
            }
        }

        fn membership_case(
            program: &dyn crate::CheckingInput,
            atom: ExpressionHandle,
        ) -> Option<u32> {
            let ExpressionNode::Binary(binary) = expression_table(program).expression(atom) else {
                return None;
            };
            let membership = binary.operator == BinaryOperator::CaseMembership
                || expression_table(program)
                    .authored_selection_occurrences(atom)
                    .any(|occurrence| {
                        program
                            .authored_declaration_selections()
                            .get(occurrence)
                            .is_some_and(|selection| {
                                selection.kind()
                                    == typed_trees::AuthoredDeclarationSelectionKind::CaseMembership
                            })
                    });
            if !membership {
                return None;
            }
            let path = expression_table(program).name_path_node(binary.right)?;
            [path.symbol, path.head_symbol]
                .into_iter()
                .chain(
                    expression_table(program)
                        .name_path_member_symbols(path.member_symbols)
                        .last()
                        .copied(),
                )
                .find_map(|symbol| case_position(program, symbol))
        }

        fn case_position(
            program: &dyn crate::CheckingInput,
            case: symbols::SymbolHandle,
        ) -> Option<u32> {
            if !case.is_valid() {
                return None;
            }
            program
                .data_variant_position_for_symbol(case)
                .map(|(_, position)| position)
        }

        fn flatten(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            operator: BinaryOperator,
            out: &mut Vec<ExpressionHandle>,
        ) {
            if let ExpressionNode::Binary(binary) = expression_table(program).expression(expression)
                && binary.operator == operator
            {
                flatten(program, binary.left, operator, out);
                flatten(program, binary.right, operator, out);
                return;
            }
            out.push(expression);
        }
    }
    pub(crate) mod float_predicates {
        use crate::frozen_program::expression_table;
        use numerics::float_semantics::{FloatFormat, FloatMeaning, FloatSemantics};
        use symbols::{BuiltinTypeAtom, SymbolHandle};
        use typed_trees::data::{DataDefinition, DataField};
        use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
        use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

        pub fn data_float_predicate(
            program: &dyn crate::CheckingInput,
            definition: &DataDefinition,
            expression: ExpressionHandle,
            field_value: &impl Fn(SymbolHandle, PrimitiveType) -> Option<FloatMeaning>,
        ) -> Option<bool> {
            predicate(program, definition, expression, field_value, 0)
        }

        pub(crate) fn float_field_literal_bound_meaning(
            program: &dyn crate::CheckingInput,
            definition: &DataDefinition,
            conjunct: ExpressionHandle,
        ) -> Option<FloatMeaning> {
            if !expression_table(program).expression_is_valid(conjunct) {
                return None;
            }
            let binary = expression_table(program).binary_node(conjunct)?;
            let field_format = |expression| {
                let field = declared_field(program, definition, expression)?;
                match program.primitive_type_reference(field.type_reference)? {
                    PrimitiveType::F32 => Some(numerics::literals::FloatFormat::F32),
                    PrimitiveType::F64 => Some(numerics::literals::FloatFormat::F64),
                    _ => None,
                }
            };
            let unread = |_, _| Some(FloatMeaning::Zero { negative: false });
            let (left, left_primitive, left_type) = operand(
                program,
                definition,
                binary.left,
                &unread,
                field_format(binary.right),
            )?;
            let (right, right_primitive, right_type) = operand(
                program,
                definition,
                binary.right,
                &unread,
                field_format(binary.left),
            )?;
            if left_primitive != right_primitive
                || !crate::has_builtin_or_sealed_float_comparison_meaning(
                    program,
                    definition.symbol,
                    conjunct,
                    binary.operator,
                    &[Some(left_type), Some(right_type)],
                )
            {
                return None;
            }
            match (
                expression_table(program).expression(binary.left),
                expression_table(program).expression(binary.right),
            ) {
                (ExpressionNode::Float(_), ExpressionNode::Name(_)) => Some(left),
                (ExpressionNode::Name(_), ExpressionNode::Float(_)) => Some(right),
                _ => None,
            }
        }

        fn predicate(
            program: &dyn crate::CheckingInput,
            definition: &DataDefinition,
            expression: ExpressionHandle,
            field_value: &impl Fn(SymbolHandle, PrimitiveType) -> Option<FloatMeaning>,
            depth: usize,
        ) -> Option<bool> {
            let node = expression_table(program).expression_at_depth(expression, depth)?;
            let ExpressionNode::Binary(binary) = node else {
                return match expression_table(program).expression(expression) {
                    ExpressionNode::Boolean(value) => Some(*value),
                    _ => None,
                };
            };
            if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
                let left = predicate(program, definition, binary.left, field_value, depth + 1)?;
                let right = predicate(program, definition, binary.right, field_value, depth + 1)?;
                return Some(if binary.operator == BinaryOperator::And {
                    left && right
                } else {
                    left || right
                });
            }
            let (left, left_primitive, left_type) =
                operand(program, definition, binary.left, field_value, None)?;
            let (right, right_primitive, right_type) =
                operand(program, definition, binary.right, field_value, None)?;
            if left_primitive != right_primitive
                || !crate::has_builtin_or_sealed_float_comparison_meaning(
                    program,
                    definition.symbol,
                    expression,
                    binary.operator,
                    &[Some(left_type), Some(right_type)],
                )
            {
                return None;
            }
            Some(match binary.operator {
                BinaryOperator::Equal => FloatSemantics::equal(&left, &right),
                BinaryOperator::NotEqual => FloatSemantics::not_equal(&left, &right),
                BinaryOperator::Less => FloatSemantics::less(&left, &right),
                BinaryOperator::LessOrEqual => FloatSemantics::less_or_equal(&left, &right),
                BinaryOperator::Greater => FloatSemantics::greater(&left, &right),
                BinaryOperator::GreaterOrEqual => FloatSemantics::greater_or_equal(&left, &right),
                _ => return None,
            })
        }

        fn operand(
            program: &dyn crate::CheckingInput,
            definition: &DataDefinition,
            expression: ExpressionHandle,
            field_value: &impl Fn(SymbolHandle, PrimitiveType) -> Option<FloatMeaning>,
            destination: Option<numerics::literals::FloatFormat>,
        ) -> Option<(FloatMeaning, PrimitiveType, TypeReferenceHandle)> {
            match expression_table(program).expression(expression) {
                ExpressionNode::Name(path) if crate::is_single_segment_name_path(program, path) => {
                    let field = declared_field(program, definition, expression)?;
                    let primitive = program.primitive_type_reference(field.type_reference)?;
                    if !matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64) {
                        return None;
                    }
                    Some((
                        field_value(field.symbol, primitive)?,
                        primitive,
                        field.type_reference,
                    ))
                }
                ExpressionNode::Float(literal) => {
                    let (format, primitive, atom) = match literal.landing().or(destination)? {
                        numerics::literals::FloatFormat::F32 => (
                            FloatFormat::BINARY32,
                            PrimitiveType::F32,
                            BuiltinTypeAtom::F32,
                        ),
                        numerics::literals::FloatFormat::F64 => (
                            FloatFormat::BINARY64,
                            PrimitiveType::F64,
                            BuiltinTypeAtom::F64,
                        ),
                    };
                    let symbol = program
                        .symbols
                        .child_handles(program.symbols.root())?
                        .find(|symbol| program.symbols.builtin_type_atom(*symbol) == Some(atom))?;
                    let type_reference = program
                        .type_reference_table
                        .find_named_type_reference(symbol)?;
                    Some((
                        FloatSemantics::from_decimal(format, literal.text())?,
                        primitive,
                        type_reference,
                    ))
                }
                _ => None,
            }
        }

        fn declared_field<'program>(
            program: &'program dyn crate::CheckingInput,
            definition: &DataDefinition,
            expression: ExpressionHandle,
        ) -> Option<&'program DataField> {
            let symbol = crate::queries::expression_traversal::single_segment_name_symbol(
                program, expression,
            )?;
            program.data_field(definition, symbol)
        }
    }
    pub(crate) mod place_queries {
        use crate::frozen_program::expression_table;
        use crate::value_custody::places::collection_operations::MeasureReceiver;
        use language_core::{is_rooted_at, is_self_receiver};
        use symbols::JoinedPath;
        use typed_trees::data::DataDefinition;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        pub(crate) fn declared_root_established(
            program: &dyn crate::CheckingInput,
            state: &State,
            root: symbols::SymbolHandle,
        ) -> bool {
            if !root.is_valid() {
                return false;
            }
            if program.state_parameter_by_symbol(state, root).is_some() {
                return true;
            }
            if program
                .data_type_parameter_by_symbol(root)
                .is_some_and(|parameter| {
                    matches!(
                        parameter.kind,
                        typed_trees::data::TypeParameterKind::Const { .. }
                            | typed_trees::data::TypeParameterKind::Value { .. }
                    )
                })
                || program
                    .const_declaration_by_symbol(root)
                    .is_some_and(|declaration| declaration.evaluated_value.is_some())
            {
                return true;
            }
            program
                .state_local_declaration(state, root)
                .is_some_and(|(_, local)| {
                    local.initial_value.is_valid()
                        || !crate::value_custody::data::type_requires_establishment(
                            program,
                            local.type_reference,
                        )
                })
        }

        pub(crate) fn expression_root_symbol(
            program: &dyn crate::CheckingInput,
            mut expression: ExpressionHandle,
        ) -> symbols::SymbolHandle {
            while expression.is_valid() {
                expression = match expression_table(program).expression(expression) {
                    ExpressionNode::Name(path) => return path.head_symbol,
                    ExpressionNode::Member(member) => member.receiver,
                    ExpressionNode::Indexed(indexed) => indexed.collection,
                    ExpressionNode::Borrow(borrow) => borrow.target,
                    _ => break,
                };
            }
            symbols::SymbolHandle::invalid()
        }

        pub(crate) fn self_place_spelling(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
        ) -> Option<String> {
            match expression_table(program).expression(expression) {
                ExpressionNode::Name(path) => {
                    let members = expression_table(program).name_path_members(path.members);
                    members.first()?;
                    Some(members.joined_path_with("."))
                }
                ExpressionNode::Member(member) => {
                    let receiver = self_place_spelling(program, member.receiver)?;
                    Some(format!("{receiver}.{}", member.member.as_str()))
                }
                ExpressionNode::Indexed(indexed) => {
                    let collection = self_place_spelling(program, indexed.collection)?;
                    let index = match expression_table(program).expression(indexed.index) {
                        ExpressionNode::Integer(value) => value.to_string(),
                        _ => return None,
                    };
                    Some(format!("{collection}[{index}]"))
                }
                ExpressionNode::Borrow(inner) => self_place_spelling(program, inner.target),
                _ => None,
            }
        }

        pub(crate) fn is_data_rooted(data_binding: Option<&str>, spelling: &str) -> bool {
            data_binding.is_some_and(|root| is_rooted_at(root, spelling))
        }

        pub(crate) fn write_place_spelling(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
        ) -> Option<String> {
            match expression_table(program).expression(expression) {
                ExpressionNode::Indexed(indexed) => {
                    let collection = write_place_spelling(program, indexed.collection)?;
                    let index = match expression_table(program).expression(indexed.index) {
                        ExpressionNode::Integer(value) => value.to_string(),
                        _ => "*".to_owned(),
                    };
                    Some(format!("{collection}[{index}]"))
                }
                ExpressionNode::Member(member) => {
                    let receiver = write_place_spelling(program, member.receiver)?;
                    Some(format!("{receiver}.{}", member.member.as_str()))
                }
                ExpressionNode::Borrow(inner) => write_place_spelling(program, inner.target),
                ExpressionNode::Name(_) => self_place_spelling(program, expression),
                _ => None,
            }
        }

        pub(crate) fn place_spelling_covers(place: &str, nested: &str) -> bool {
            let mut place_parts = place.split('.');
            let mut nested_parts = nested.split('.');
            loop {
                match (place_parts.next(), nested_parts.next()) {
                    (Some(place_part), Some(nested_part)) => {
                        if !segment_covers(place_part, nested_part) {
                            return false;
                        }
                    }
                    (None, _) => return true,
                    (Some(_), None) => return false,
                }
            }
        }

        pub(crate) fn place_spellings_may_overlap(left: &str, right: &str) -> bool {
            fn coordinates(place: &str) -> impl Iterator<Item = &str> {
                place.split(['.', '[', ']']).filter(|part| !part.is_empty())
            }
            coordinates(left)
                .zip(coordinates(right))
                .all(|(left, right)| left == right || left == "*" || right == "*")
        }

        fn segment_covers(place: &str, nested: &str) -> bool {
            let mut place = place;
            let mut nested = nested;
            loop {
                let Some(at) = place.find("[*]") else {
                    return place == nested || (place.is_empty() && nested.starts_with('['));
                };
                if !nested.starts_with(&place[..at]) {
                    return false;
                }
                nested = &nested[at..];
                let Some(close) = nested.find(']') else {
                    return false;
                };
                nested = &nested[close + 1..];
                place = &place[at + "[*]".len()..];
            }
        }

        pub(crate) fn is_subplace(child: &str, parent: &str) -> bool {
            child.starts_with(&format!("{parent}.")) || child.starts_with(&format!("{parent}["))
        }

        pub(crate) fn write_target_index_expressions(
            program: &dyn crate::CheckingInput,
            target: ExpressionHandle,
        ) -> Vec<ExpressionHandle> {
            let mut indexes = Vec::new();
            let mut cursor = target;
            loop {
                match expression_table(program).expression(cursor) {
                    ExpressionNode::Indexed(indexed) => {
                        indexes.push(indexed.index);
                        cursor = indexed.collection;
                    }
                    ExpressionNode::Member(member) => cursor = member.receiver,
                    ExpressionNode::Borrow(inner) => cursor = inner.target,
                    _ => break,
                }
            }
            indexes
        }

        pub(crate) fn transition_evaluated_expressions(
            program: &dyn crate::CheckingInput,
            transition: &typed_trees::statement::TableTransition,
        ) -> Vec<ExpressionHandle> {
            let mut evaluated = Vec::new();
            if let typed_trees::statement::TransitionGuardNode::When(guard) = &transition.guard {
                evaluated.push(*guard);
            }
            for handle in transition.targets() {
                match program.statement_table.transition_target(handle) {
                    typed_trees::statement::TransitionTargetNode::Named { arguments, .. } => {
                        evaluated.extend_from_slice(
                            program.statement_table.expression_handles(*arguments),
                        )
                    }
                    typed_trees::statement::TransitionTargetNode::Value(expression) => {
                        evaluated.push(*expression);
                    }
                    _ => {}
                }
            }
            evaluated
        }

        pub(crate) fn data_has_case_requires_facts(
            program: &dyn crate::CheckingInput,
            definition: &DataDefinition,
        ) -> bool {
            program.data_members(definition).iter().any(|member| {
                matches!(member, typed_trees::data::DataMember::Variant(variant)
            if !variant.requires_facts.is_empty())
            })
        }

        pub(crate) fn fact_names_field(
            program: &dyn crate::CheckingInput,
            definition: &DataDefinition,
            fact: &typed_trees::domain::ProofFact,
            field: &str,
        ) -> bool {
            let reads = predicate_read_set(program, definition, fact, false);
            declared_predicate_fields(program, definition)
                .into_iter()
                .any(|declared| {
                    declared.name.as_str() == field && reads.fields.contains(&declared.symbol)
                })
        }

        pub(crate) fn data_predicate_fields(
            program: &dyn crate::CheckingInput,
            definition: &DataDefinition,
            fact: &typed_trees::domain::ProofFact,
        ) -> Vec<symbols::SymbolHandle> {
            predicate_read_set(program, definition, fact, false).fields
        }

        pub(crate) fn fact_reads_field_contents(
            program: &dyn crate::CheckingInput,
            definition: &DataDefinition,
            fact: &typed_trees::domain::ProofFact,
            field: symbols::SymbolHandle,
        ) -> bool {
            predicate_read_set(program, definition, fact, true)
                .fields
                .contains(&field)
        }

        pub(crate) struct PredicateReadSet {
            fields: Vec<symbols::SymbolHandle>,
            paths: Vec<Vec<facts::PlaceSegment>>,
            complete_paths: bool,
        }
        impl Default for PredicateReadSet {
            fn default() -> Self {
                Self {
                    fields: Vec::new(),
                    paths: Vec::new(),
                    complete_paths: true,
                }
            }
        }

        pub(crate) fn fact_is_disjoint_from_store(
            program: &dyn crate::CheckingInput,
            definition: &DataDefinition,
            fact: &typed_trees::domain::ProofFact,
            destination: &[facts::PlaceSegment],
        ) -> bool {
            let reads = predicate_read_set(program, definition, fact, false);
            reads.complete_paths && !reads.paths.is_empty() && reads.paths.iter().all(|read| {
        !crate::value_custody::places::storage_path_relations::canonical_place_segments_may_overlap(
            program,
            read,
            destination,
        )
    })
        }

        pub(crate) fn predicate_projection(
            program: &dyn crate::CheckingInput,
            definition: &DataDefinition,
            application: TypeReferenceHandle,
            expression: ExpressionHandle,
        ) -> Option<Vec<facts::PlaceSegment>> {
            let query = (|| -> Result<Vec<facts::PlaceSegment>, &'static str> {
                let application =
                    crate::value_custody::places::unwrapped_type_reference(program, application)
                        .ok_or("receiving application is unavailable")?;
                program
                    .data_constructor_definition(application, definition.symbol)
                    .ok_or("receiving application does not own the predicate declaration")?;
                let mut parameters = declared_predicate_fields(program, definition)
                    .iter()
                    .map(|field| typed_trees::signature::StateParameter {
                        symbol: field.symbol,
                        name: field.name.clone(),
                        type_reference: field.type_reference,
                        ..Default::default()
                    })
                    .collect::<Vec<_>>();
                parameters.push(typed_trees::signature::StateParameter {
                    symbol: definition.predicate_subject_symbol,
                    name: "\0predicate_subject".into(),
                    type_reference: application,
                    ..Default::default()
                });
                let result = crate::parameter_expression_result_type_reference(
                    program,
                    definition.symbol,
                    &parameters,
                    expression,
                )
                .ok_or("projection has no result in the exact declaration telescope")?;
                if crate::reference_shell(program, result).is_some() {
                    return Err("projection result is a reference descriptor, not inline contents");
                }
                crate::value_custody::places::parameter_source_type_application(
                    program,
                    definition.symbol,
                    &parameters,
                    expression,
                )
                .ok_or(
                    "projection has no authenticated ordinary coordinate/selected index meaning",
                )?;
                let mut place = crate::canonical_place_from_expression(program, expression)
                    .ok_or("projection has no canonical place")?;
                if place.root.as_symbol() != Some(definition.predicate_subject_symbol) {
                    let symbol = place
                        .root
                        .as_symbol()
                        .ok_or("projection is not rooted in a declaration subject")?;
                    parameters
                        .iter()
                        .find(|p| {
                            p.symbol == symbol && p.symbol != definition.predicate_subject_symbol
                        })
                        .ok_or(
                            "projection root is not the receiving subject or one of its fields",
                        )?;
                    let mut path = Vec::new();
                    crate::value_custody::places::push_field_place_segments(
                        program, &mut path, symbol,
                    );
                    path.extend(place.segments);
                    place.segments = path;
                }
                let mut reference = application;
                for segment in &place.segments {
                    if crate::reference_shell(program, reference).is_some() {
                        return Err(
                            "projection crosses a reference descriptor; field storage is not referent contents",
                        );
                    }
                    reference = crate::project_type_reference_from_segments(
                        program,
                        reference,
                        std::slice::from_ref(segment),
                    )
                    .ok_or("projection segment is outside its declared receiving carrier")?;
                }
                if crate::reference_shell(program, reference).is_some()
                    || !crate::declared_carriers_match(program, reference, result)
                {
                    return Err(
                        "projection carrier is not the same inline value as its declared coordinate",
                    );
                }
                Ok(place.segments)
            })();
            query.ok()
        }

        fn declared_predicate_fields<'program>(
            program: &'program dyn crate::CheckingInput,
            definition: &'program DataDefinition,
        ) -> Vec<&'program typed_trees::data::DataField> {
            program
                .data_members(definition)
                .iter()
                .filter_map(|member| member.as_field())
                .chain(
                    program
                        .data_members(definition)
                        .iter()
                        .filter_map(|member| member.as_variant())
                        .flat_map(|variant| program.data_payload_fields(variant)),
                )
                .collect()
        }

        #[derive(Default)]
        pub(crate) struct PredicateArgumentReferences {
            symbols: Vec<symbols::SymbolHandle>,
            expressions: Vec<ExpressionHandle>,
            types: Vec<TypeReferenceHandle>,
        }

        impl Extend<symbols::SymbolHandle> for PredicateArgumentReferences {
            fn extend<T: IntoIterator<Item = symbols::SymbolHandle>>(&mut self, symbols: T) {
                for symbol in symbols {
                    if !self.symbols.contains(&symbol) {
                        self.symbols.push(symbol);
                    }
                }
            }
        }

        impl typed_trees::references::TypedReferences for PredicateArgumentReferences {
            fn expression(&mut self, expression: ExpressionHandle) -> bool {
                self.expressions.push(expression);
                false
            }
            fn type_expression(&mut self, expression: ExpressionHandle) -> bool {
                self.expressions.push(expression);
                false
            }
            fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
                if self.types.contains(&reference) {
                    false
                } else {
                    self.types.push(reference);
                    true
                }
            }
        }

        fn predicate_read_set(
            program: &dyn crate::CheckingInput,
            definition: &DataDefinition,
            fact: &typed_trees::domain::ProofFact,
            contents_only: bool,
        ) -> PredicateReadSet {
            let declared = declared_predicate_fields(program, definition);
            let receiving_field = |symbol| {
                if let Some(field) = declared.iter().find(|field| field.symbol == symbol) {
                    return Some(field.symbol);
                }
                let reference = definition
                    .generic_instance
                    .unwrap_or(definition.predicate_subject_type);
                if !reference.is_valid() {
                    return None;
                }
                let (source, _) = program.any_field_owner(symbol)?;
                let selected = program.data_constructor_field(reference, source.symbol, symbol)?;
                declared
                    .iter()
                    .any(|field| field.symbol == selected.symbol)
                    .then_some(selected.symbol)
            };
            let mut reads = PredicateReadSet::default();
            let mut pending = match fact {
                typed_trees::domain::ProofFact::Expression(expression) => vec![(*expression, true)],
                typed_trees::domain::ProofFact::Membership(membership) => {
                    vec![(membership.value, true)]
                }
                typed_trees::domain::ProofFact::Proposition(application) => {
                    expression_table(program)
                        .expression_handles(application.arguments)
                        .iter()
                        .map(|value| (*value, true))
                        .collect()
                }
            };
            let mut static_references = PredicateArgumentReferences::default();
            if let typed_trees::domain::ProofFact::Membership(membership) = fact {
                for argument in program
                    .type_reference_table
                    .type_reference_handles(membership.domain_arguments)
                {
                    typed_trees::references::visit_type_reference(
                        program,
                        &mut static_references,
                        *argument,
                    );
                }
            }
            pending.extend(
                static_references
                    .expressions
                    .drain(..)
                    .map(|expression| (expression, true)),
            );
            for symbol in static_references.symbols.drain(..) {
                if let Some(field) = receiving_field(symbol) {
                    reads.complete_paths = false;
                    if !reads.fields.contains(&field) {
                        reads.fields.push(field);
                    }
                }
            }
            let mut visited = Vec::new();
            while let Some((expression, whole_subject)) = pending.pop() {
                if !expression.is_valid() || visited.contains(&(expression, whole_subject)) {
                    continue;
                }
                visited.push((expression, whole_subject));
                if !contents_only {
                    let application = definition
                        .generic_instance
                        .unwrap_or(definition.predicate_subject_type);
                    if let Some(path) =
                        predicate_projection(program, definition, application, expression)
                    {
                        if !path.is_empty() || whole_subject {
                            if let Some(facts::PlaceSegment::Field { symbol }) = path.first() {
                                if !reads.fields.contains(symbol) {
                                    reads.fields.push(*symbol);
                                }
                            } else {
                                for field in &declared {
                                    if !reads.fields.contains(&field.symbol) {
                                        reads.fields.push(field.symbol);
                                    }
                                }
                            }
                            for segment in &path {
                                if let facts::PlaceSegment::Index { expression } = segment {
                                    pending.push((*expression, true));
                                }
                            }
                            if !reads.paths.contains(&path) {
                                reads.paths.push(path);
                            }
                        }
                        continue;
                    }
                    if crate::canonical_place_from_expression(program, expression).is_some_and(
                        |place| {
                            place.root.as_symbol() == Some(definition.predicate_subject_symbol)
                                || place
                                    .root
                                    .as_symbol()
                                    .is_some_and(|symbol| receiving_field(symbol).is_some())
                        },
                    ) {
                        reads.complete_paths = false;
                    }
                }
                if contents_only
            && let Some(member) = expression_table(program).member_node(expression)
            && let Some(field) = membership_field(program, definition, member.receiver)
            && crate::value_custody::places::collection_operations::collection_measure_member(
                program,
                member,
                MeasureReceiver::Declared(field.type_reference),
            )
            .is_some()
        {
            continue;
        }
                match expression_table(program).expression(expression) {
            ExpressionNode::Name(path) => {
                if let Some(symbol) = receiving_field(path.symbol) {
                    if !reads.fields.contains(&symbol) { reads.fields.push(symbol); }
                } else if definition.predicate_subject_symbol.is_valid()
                    && path.symbol == definition.predicate_subject_symbol && whole_subject
                {
                    for field in &declared { if !reads.fields.contains(&field.symbol) { reads.fields.push(field.symbol); } }
                }
            }
            ExpressionNode::Member(member) => {
                let logical_receiver = expression_table(program).name_path_node(member.receiver)
                    .is_some_and(|path| definition.predicate_subject_symbol.is_valid()
                        && path.symbol == definition.predicate_subject_symbol);
                if logical_receiver {
                    if let Some(field) = crate::value_custody::places::exact_data_member_field(
                        program, definition, receiving_field(member.member_symbol).unwrap_or(member.member_symbol), member.member.as_str(),
                        member.case_variant.as_ref().map(|case| case.as_str()),
                    ) {
                        if !reads.fields.contains(&field.symbol) { reads.fields.push(field.symbol); }
                    } else {
                        for field in &declared { if !reads.fields.contains(&field.symbol) { reads.fields.push(field.symbol); } }
                    }
                } else { pending.push((member.receiver, false)); }
            }
            ExpressionNode::Call(call) => {
                crate::queries::expression_traversal::children(program, expression_table(program).expression(expression), |child| pending.push((child, true)));
                let call = typed_trees::references::StaticArgumentCall::Expression(expression, call);
                for argument in call.arguments(program) {
                    typed_trees::references::visit_static_argument(program, &mut static_references, argument);
                }
                typed_trees::references::visit_static_machine_bindings(program, &mut static_references, call.application());
                typed_trees::references::visit_static_machine_bindings(program, &mut static_references, call.selection_application());
                pending.extend(static_references.expressions.drain(..).map(|expression| (expression, true)));
                for symbol in static_references.symbols.drain(..) {
                    if let Some(field) = receiving_field(symbol) { reads.complete_paths = false; if !reads.fields.contains(&field) { reads.fields.push(field); } }
                    else if definition.predicate_subject_symbol.is_valid() && symbol == definition.predicate_subject_symbol {
                            reads.complete_paths = false;
                            for field in &declared { if !reads.fields.contains(&field.symbol) { reads.fields.push(field.symbol); } }
                    }
                }
            }
            ExpressionNode::Binary(binary) if binary.operator == typed_trees::expression::BinaryOperator::CaseMembership
                || expression_table(program).authored_selection_occurrences(expression).any(|occurrence| {
                    program.authored_declaration_selections().get(occurrence).is_some_and(|selection|
                        selection.kind() == typed_trees::AuthoredDeclarationSelectionKind::CaseMembership)
                }) =>
            {
                pending.push((binary.left, false));
            }
            node => crate::queries::expression_traversal::children(program, node, |child| pending.push((child, true))),
        }
            }
            reads
        }

        pub(crate) fn membership_field<'program>(
            program: &'program dyn crate::CheckingInput,
            definition: &'program DataDefinition,
            value: ExpressionHandle,
        ) -> Option<&'program typed_trees::data::DataField> {
            let symbol = match expression_table(program).expression(value) {
                ExpressionNode::Name(path) => path.symbol,
                ExpressionNode::Member(member) => {
                    let path = expression_table(program).name_path_node(member.receiver)?;
                    let (source, _) = program.any_field_owner(member.member_symbol)?;
                    if !source.predicate_subject_symbol.is_valid()
                        || path.symbol != source.predicate_subject_symbol
                    {
                        return None;
                    }
                    member.member_symbol
                }
                _ => return None,
            };
            let declared = declared_predicate_fields(program, definition);
            if let Some(field) = declared.iter().find(|field| field.symbol == symbol) {
                return Some(*field);
            }
            let reference = definition
                .generic_instance
                .unwrap_or(definition.predicate_subject_type);
            if !reference.is_valid() {
                return None;
            }
            let (source, _) = program.any_field_owner(symbol)?;
            let selected = program.data_constructor_field(reference, source.symbol, symbol)?;
            declared
                .into_iter()
                .find(|field| field.symbol == selected.symbol)
        }
    }
    pub(crate) mod symbolic_values {
        use crate::frozen_program::expression_table;
        use language_semantics::const_value::CanonicalConstValue;
        use language_semantics::const_value::DecodedCanonicalConstValue;
        use language_semantics::declaration_selection::CollectionMeasure;
        use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
        use typed_trees::types::PrimitiveType;

        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(crate) enum LiteralValue {
            Integer(i128),
            Float32(u32),
            Float64(u64),
        }

        impl LiteralValue {
            pub(crate) fn integer(self) -> Option<i128> {
                match self {
                    Self::Integer(value) => Some(value),
                    _ => None,
                }
            }

            fn float(
                self,
                primitive: PrimitiveType,
            ) -> Option<numerics::float_semantics::FloatMeaning> {
                use numerics::float_semantics::FloatMeaning;
                match (self, primitive) {
                    (Self::Float32(bits), PrimitiveType::F32) => {
                        Some(FloatMeaning::from_f32(f32::from_bits(bits)))
                    }
                    (Self::Float64(bits), PrimitiveType::F64) => {
                        Some(FloatMeaning::from_f64(f64::from_bits(bits)))
                    }
                    _ => None,
                }
            }
        }

        pub(crate) fn literal_value(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
        ) -> Option<LiteralValue> {
            match expression_table(program).expression(expression) {
                ExpressionNode::Integer(value) => {
                    value.decimal_value_i128().map(LiteralValue::Integer)
                }
                ExpressionNode::Boolean(value) => {
                    Some(LiteralValue::Integer(if *value { 1 } else { 0 }))
                }
                ExpressionNode::Float(value) => {
                    use numerics::float_semantics::{FloatFormat, FloatSemantics};
                    Some(match value.landing()? {
                        numerics::literals::FloatFormat::F32 => LiteralValue::Float32(
                            FloatSemantics::from_decimal(FloatFormat::BINARY32, value.text())?
                                .to_f32()
                                .to_bits(),
                        ),
                        numerics::literals::FloatFormat::F64 => LiteralValue::Float64(
                            FloatSemantics::from_decimal(FloatFormat::BINARY64, value.text())?
                                .to_f64()
                                .to_bits(),
                        ),
                    })
                }
                ExpressionNode::Borrow(inner) => literal_value(program, inner.target),
                _ => None,
            }
        }

        pub(crate) fn predicate_holds(
            program: &dyn crate::CheckingInput,
            definition: &typed_trees::data::DataDefinition,
            fields: &[(String, Option<LiteralValue>)],
            active_case: Option<symbols::SymbolHandle>,
        ) -> bool {
            let valuation = fields
                .iter()
                .map(|(name, value)| (name.as_str(), *value))
                .collect::<Vec<_>>();
            program
                .conjunctive_proof_facts(definition.requires_facts)
                .all(|fact| {
                    let Some(expression) = fact.as_expression() else {
                        return false;
                    };
                    fold(
                        program,
                        definition,
                        &valuation,
                        &[],
                        &[],
                        false,
                        active_case,
                        *expression,
                    )
                    .is_some_and(|value| value != 0)
                })
        }

        pub(crate) fn struct_literal_snapshot(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
        ) -> (
            Vec<(String, Option<LiteralValue>)>,
            Vec<(String, SymbolicValue)>,
        ) {
            let ExpressionNode::StructLiteral(literal) =
                expression_table(program).expression(expression)
            else {
                return (Vec::new(), Vec::new());
            };
            let fields = expression_table(program).struct_fields(literal.fields);
            let values = fields
                .iter()
                .map(|field| {
                    (
                        field.name.as_str().to_string(),
                        literal_value(program, field.value),
                    )
                })
                .collect();
            let symbols = fields
                .iter()
                .filter_map(|field| {
                    expression_symbolic_value(program, field.value)
                        .map(|symbol| (field.name.as_str().to_string(), symbol))
                })
                .collect();
            (values, symbols)
        }

        pub(crate) fn mentions_float(
            program: &dyn crate::CheckingInput,
            definition: &typed_trees::data::DataDefinition,
            expression: ExpressionHandle,
        ) -> bool {
            match expression_table(program).expression(expression) {
                ExpressionNode::Float(_) | ExpressionNode::UndeterminedNaN(_) => true,
                ExpressionNode::Name(path) => {
                    let is_float_field = |field: &typed_trees::data::DataField| {
                        field.symbol.is_valid()
                            && field.symbol == path.symbol
                            && matches!(
                                program.primitive_type_reference(field.type_reference),
                                Some(PrimitiveType::F32 | PrimitiveType::F64)
                            )
                    };
                    program
                        .data_members(definition)
                        .iter()
                        .any(|member| match member {
                            typed_trees::data::DataMember::Field(field) => is_float_field(field),
                            typed_trees::data::DataMember::Variant(variant) => program
                                .data_payload_fields(variant)
                                .iter()
                                .any(is_float_field),
                        })
                }
                ExpressionNode::Binary(binary) => {
                    mentions_float(program, definition, binary.left)
                        || mentions_float(program, definition, binary.right)
                }
                ExpressionNode::Unary(unary) => mentions_float(program, definition, unary.operand),
                _ => false,
            }
        }

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub(crate) enum SymbolicValue {
            Atom(symbols::SymbolHandle),
            Integer(i128),
            Add(Vec<SymbolicValue>),
            Multiply(Vec<SymbolicValue>),
            Subtract(Box<SymbolicValue>, Box<SymbolicValue>),
        }

        pub(crate) fn fold(
            program: &dyn crate::CheckingInput,
            definition: &typed_trees::data::DataDefinition,
            valuation: &[(&str, Option<LiteralValue>)],
            symbols: &[(String, SymbolicValue)],
            measures: &[(String, Option<i128>, Option<i128>)],
            born_zero: bool,
            active_case: Option<symbols::SymbolHandle>,
            expression: ExpressionHandle,
        ) -> Option<i128> {
            match expression_table(program).expression(expression) {
                ExpressionNode::Name(path) => {
                    if path.symbol.is_valid()
                        && program.symbols.get(path.symbol).kind
                            == symbols::SymbolKind::TypeParameter
                    {
                        return applied_const_integer(program, definition, path.symbol)
                            .map(|(_, value)| value);
                    }
                    let last = expression_table(program)
                        .name_path_members(path.members)
                        .last()?
                        .as_str();
                    match valuation.iter().find(|(name, _)| *name == last) {
                        Some((_, value)) => value.and_then(LiteralValue::integer),
                        None if born_zero => Some(0),
                        None => None,
                    }
                }
                ExpressionNode::Integer(value) => value.decimal_value_i128(),
                ExpressionNode::Member(member)
                    if CollectionMeasure::from_authored_spelling(member.member.as_str())
                        .is_some() =>
                {
                    let path = expression_table(program).name_path_node(member.receiver)?;
                    let field = expression_table(program)
                        .name_path_members(path.members)
                        .last()?
                        .as_str();
                    match measures.iter().find(|(name, _, _)| name == field) {
                        Some((_, length, capacity)) => {
                            match CollectionMeasure::from_authored_spelling(member.member.as_str())
                            {
                                Some(CollectionMeasure::Length) => *length,
                                Some(CollectionMeasure::Capacity) => *capacity,
                                None => None,
                            }
                        }
                        None if born_zero => Some(0),
                        None => None,
                    }
                }
                ExpressionNode::Member(member)
                    if subject_field(program, definition, member.receiver) =>
                {
                    match valuation
                        .iter()
                        .find(|(name, _)| *name == member.member.as_str())
                    {
                        Some((_, value)) => value.and_then(LiteralValue::integer),
                        None if born_zero => Some(0),
                        None => None,
                    }
                }
                ExpressionNode::Binary(binary) => {
                    if binary.operator == BinaryOperator::CaseMembership
                        || expression_table(program)
                            .authored_selection_occurrences(expression)
                            .any(|occurrence| {
                                program
                                    .authored_declaration_selections()
                                    .get(occurrence)
                                    .is_some_and(|selection| {
                                        selection.kind()
                                    == typed_trees::AuthoredDeclarationSelectionKind::CaseMembership
                                    })
                            })
                    {
                        return case_membership_value(program, binary.right, active_case);
                    }
                    if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
                        let left = fold(
                            program,
                            definition,
                            valuation,
                            symbols,
                            measures,
                            born_zero,
                            active_case,
                            binary.left,
                        );
                        let right = fold(
                            program,
                            definition,
                            valuation,
                            symbols,
                            measures,
                            born_zero,
                            active_case,
                            binary.right,
                        );
                        return match binary.operator {
                            BinaryOperator::Or => match (left, right) {
                                (Some(value), _) | (_, Some(value)) if value != 0 => Some(1),
                                (Some(left), Some(right)) => {
                                    Some(i128::from(left != 0 || right != 0))
                                }
                                _ => None,
                            },
                            BinaryOperator::And => match (left, right) {
                                (Some(0), _) | (_, Some(0)) => Some(0),
                                (Some(left), Some(right)) => {
                                    Some(i128::from(left != 0 && right != 0))
                                }
                                _ => None,
                            },
                            _ => None,
                        };
                    }
                    if !matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
                        && mentions_float(program, definition, expression)
                    {
                        return super::float_predicates::data_float_predicate(
                            program,
                            definition,
                            expression,
                            &|symbol, primitive| {
                                let field = program.data_field(definition, symbol)?;
                                match valuation
                                    .iter()
                                    .find(|(name, _)| *name == field.name.as_str())
                                {
                                    Some((_, value)) => {
                                        value.and_then(|value| value.float(primitive))
                                    }
                                    None if born_zero => {
                                        Some(numerics::float_semantics::FloatMeaning::Zero {
                                            negative: false,
                                        })
                                    }
                                    None => None,
                                }
                            },
                        )
                        .map(i128::from);
                    }
                    if matches!(
                        binary.operator,
                        BinaryOperator::Equal | BinaryOperator::NotEqual
                    ) && let (Some(left), Some(right)) = (
                        symbolic_operand(program, symbols, binary.left),
                        symbolic_operand(program, symbols, binary.right),
                    ) && left == right
                    {
                        return Some(i128::from(matches!(binary.operator, BinaryOperator::Equal)));
                    }
                    let left = fold(
                        program,
                        definition,
                        valuation,
                        symbols,
                        measures,
                        born_zero,
                        active_case,
                        binary.left,
                    )?;
                    let right = fold(
                        program,
                        definition,
                        valuation,
                        symbols,
                        measures,
                        born_zero,
                        active_case,
                        binary.right,
                    )?;
                    match binary.operator {
                        BinaryOperator::Add => left.checked_add(right),
                        BinaryOperator::Subtract => left.checked_sub(right),
                        BinaryOperator::Multiply => left.checked_mul(right),
                        BinaryOperator::LessOrEqual => Some(i128::from(left <= right)),
                        BinaryOperator::Less => Some(i128::from(left < right)),
                        BinaryOperator::GreaterOrEqual => Some(i128::from(left >= right)),
                        BinaryOperator::Greater => Some(i128::from(left > right)),
                        BinaryOperator::Equal => Some(i128::from(left == right)),
                        BinaryOperator::NotEqual => Some(i128::from(left != right)),
                        BinaryOperator::And => Some(i128::from(left != 0 && right != 0)),
                        BinaryOperator::Or => Some(i128::from(left != 0 || right != 0)),
                        _ => None,
                    }
                }
                _ => None,
            }
        }

        fn subject_field(
            program: &dyn crate::CheckingInput,
            definition: &typed_trees::data::DataDefinition,
            receiver: ExpressionHandle,
        ) -> bool {
            let ExpressionNode::Name(path) = expression_table(program).expression(receiver) else {
                return false;
            };
            let members = expression_table(program).name_path_members(path.members);
            members.len() == 1
                && (path.symbol == definition.predicate_subject_symbol
                    || members[0].as_str() == "self")
        }

        fn case_membership_value(
            program: &dyn crate::CheckingInput,
            case: ExpressionHandle,
            active_case: Option<symbols::SymbolHandle>,
        ) -> Option<i128> {
            let active = active_case.filter(|symbol| symbol.is_valid())?;
            let path = expression_table(program).name_path_node(case)?;
            if path.resolved_symbol() == Some(active) {
                return Some(1);
            }
            let spelled = expression_table(program)
                .name_path_members(path.members)
                .last()?
                .as_str();
            let active_name = program.symbols.name(active);
            Some(i128::from(spelled == active_name))
        }

        pub(crate) fn applied_const_integer(
            program: &dyn crate::CheckingInput,
            definition: &typed_trees::data::DataDefinition,
            binder: symbols::SymbolHandle,
        ) -> Option<(PrimitiveType, i128)> {
            let (primitive, actual) = applied_const_argument(program, definition, binder)?;
            let typed_trees::types::TypeReferenceNode::Named { symbol, name } =
                program.type_reference_table.type_reference(actual)
            else {
                return None;
            };
            if symbol.is_valid() {
                return None;
            }
            if let Some(value) = CanonicalConstValue::from_atom(name.as_str()) {
                return match value.decode_encoding()? {
                    DecodedCanonicalConstValue::Integer { value, .. } => Some((primitive, value)),
                    _ => None,
                };
            }
            name.as_str().parse().ok().map(|value| (primitive, value))
        }

        pub(crate) fn applied_const_argument(
            program: &dyn crate::CheckingInput,
            definition: &typed_trees::data::DataDefinition,
            binder: symbols::SymbolHandle,
        ) -> Option<(PrimitiveType, typed_trees::types::TypeReferenceHandle)> {
            let typed_trees::types::TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } = program
                .type_reference_table
                .type_reference(definition.generic_instance?)
            else {
                return None;
            };
            let template = program.unique_data_definition_by_symbol(*base_symbol)?;
            let parameters = program.data_type_parameters(template);
            let position = parameters.iter().position(|parameter| {
                parameter.symbol == binder && parameter.kind.is_const_or_value()
            })?;
            let (typed_trees::data::TypeParameterKind::Const { type_reference }
            | typed_trees::data::TypeParameterKind::Value { type_reference }) =
                parameters[position].kind
            else {
                return None;
            };
            let primitive = program.primitive_type_reference(type_reference)?;
            if !primitive.is_fixed_integer() {
                return None;
            }
            let actual = *program
                .type_reference_table
                .type_reference_handles(*arguments)
                .get(position)?;
            Some((primitive, actual))
        }

        pub(crate) fn expression_symbolic_value(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
        ) -> Option<SymbolicValue> {
            match expression_table(program).expression(expression) {
                ExpressionNode::Name(path) if path.symbol.is_valid() => {
                    Some(SymbolicValue::Atom(path.symbol))
                }
                ExpressionNode::Integer(value) => {
                    value.decimal_value_i128().map(SymbolicValue::Integer)
                }
                ExpressionNode::Binary(binary) => symbolic_binary_shape(binary, |expression| {
                    expression_symbolic_value(program, expression)
                }),
                _ => None,
            }
        }

        fn commutative_symbolic_value(
            add: bool,
            left: SymbolicValue,
            right: SymbolicValue,
        ) -> SymbolicValue {
            let mut operands = vec![left, right];
            operands.sort_by_key(symbolic_sort_key);
            if add {
                SymbolicValue::Add(operands)
            } else {
                SymbolicValue::Multiply(operands)
            }
        }

        fn symbolic_sort_key(value: &SymbolicValue) -> String {
            match value {
                SymbolicValue::Atom(symbol) => {
                    format!("a:{}:{}", symbol.arena_index(), symbol.generation())
                }
                SymbolicValue::Integer(value) => format!("i:{value}"),
                SymbolicValue::Add(values) => format!(
                    "+({})",
                    values
                        .iter()
                        .map(symbolic_sort_key)
                        .collect::<Vec<_>>()
                        .join(",")
                ),
                SymbolicValue::Multiply(values) => format!(
                    "*({})",
                    values
                        .iter()
                        .map(symbolic_sort_key)
                        .collect::<Vec<_>>()
                        .join(",")
                ),
                SymbolicValue::Subtract(left, right) => {
                    format!(
                        "-({},{})",
                        symbolic_sort_key(left),
                        symbolic_sort_key(right)
                    )
                }
            }
        }

        fn symbolic_operand(
            program: &dyn crate::CheckingInput,
            symbols: &[(String, SymbolicValue)],
            expression: ExpressionHandle,
        ) -> Option<SymbolicValue> {
            match expression_table(program).expression(expression) {
                ExpressionNode::Name(path) => {
                    let field = expression_table(program)
                        .name_path_members(path.members)
                        .last()?
                        .as_str();
                    symbols
                        .iter()
                        .find(|(name, _)| name == field)
                        .map(|(_, value)| value.clone())
                        .or_else(|| {
                            path.symbol
                                .is_valid()
                                .then_some(SymbolicValue::Atom(path.symbol))
                        })
                }
                ExpressionNode::Integer(value) => {
                    value.decimal_value_i128().map(SymbolicValue::Integer)
                }
                ExpressionNode::Binary(binary) => symbolic_binary_shape(binary, |expression| {
                    symbolic_operand(program, symbols, expression)
                }),
                _ => None,
            }
        }

        pub(crate) fn expression_sequence_measures(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
        ) -> Option<(Option<i128>, Option<i128>)> {
            match expression_table(program).expression(expression) {
                ExpressionNode::String(literal) => {
                    let measure = i128::try_from(literal.len()).ok();
                    Some((measure, measure))
                }
                ExpressionNode::ArrayLiteral(elements) => {
                    let measure = i128::try_from(
                        program.expression_table.expression_handles(*elements).len(),
                    )
                    .ok();
                    Some((measure, measure))
                }
                _ => None,
            }
        }

        pub(crate) fn expression_contains_call(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
        ) -> bool {
            crate::queries::expression_traversal::visit_pre_order(
                program,
                vec![expression],
                |_, node| {
                    matches!(node, ExpressionNode::Call(_))
                        || matches!(
                            node,
                            ExpressionNode::Atomic(atomic)
                                if crate::value_custody::atomic_operations::atomic_once_operands(
                                    program, atomic
                                )
                                .is_some()
                        )
                },
            )
        }

        fn symbolic_binary_shape(
            binary: &typed_trees::expression::TableBinaryExpression,
            operand: impl Fn(ExpressionHandle) -> Option<SymbolicValue>,
        ) -> Option<SymbolicValue> {
            let left = operand(binary.left)?;
            let right = operand(binary.right)?;
            match binary.operator {
                BinaryOperator::Add => Some(commutative_symbolic_value(true, left, right)),
                BinaryOperator::Multiply => Some(commutative_symbolic_value(false, left, right)),
                BinaryOperator::Subtract => {
                    Some(SymbolicValue::Subtract(Box::new(left), Box::new(right)))
                }
                _ => None,
            }
        }
    }
    pub(crate) mod requires_fact_intervals {
        use crate::frozen_program::expression_table;
        use typed_trees::data::DataMember;
        use typed_trees::data::{DataDefinition, DataField, DataVariant};
        use typed_trees::domain::ProofFact;
        use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
        use typed_trees::machine::Machine;
        use typed_trees::state::State;

        use super::place_queries::data_definition_for_expression;
        use crate::proof_contracts::arithmetic_domains::{BigIntInterval, Interval};

        pub fn data_requires_field_intervals(
            program: &dyn crate::CheckingInput,
            definition: &DataDefinition,
        ) -> Option<Vec<(symbols::SymbolHandle, Option<i64>, Option<i64>)>> {
            let fields = declared_data_fields(program, definition);
            field_intervals(
                program,
                &fields,
                definition.symbol,
                program.proof_facts.span_or_empty(definition.requires_facts),
            )
        }

        pub fn case_requires_field_intervals(
            program: &dyn crate::CheckingInput,
            case: &DataVariant,
        ) -> Option<Vec<(symbols::SymbolHandle, Option<i64>, Option<i64>)>> {
            let fields = program.data_payload_fields(case).iter().collect::<Vec<_>>();
            field_intervals(
                program,
                &fields,
                case.symbol,
                program.proof_facts.span_or_empty(case.requires_facts),
            )
        }

        pub fn impossible_variant(
            program: &dyn crate::CheckingInput,
            variant: &DataVariant,
        ) -> bool {
            use typed_trees::expression::ExpressionNode;
            program
                .proof_facts
                .span_or_empty(variant.requires_facts)
                .iter()
                .any(|fact| match fact {
                    typed_trees::domain::ProofFact::Expression(expression) => {
                        match expression_table(program).expression(*expression) {
                            ExpressionNode::Integer(literal) => {
                                literal.decimal_value_i128() == Some(0)
                            }
                            ExpressionNode::Boolean(value) => !*value,
                            _ => false,
                        }
                    }
                    typed_trees::domain::ProofFact::Membership(_)
                    | typed_trees::domain::ProofFact::Proposition(_) => false,
                })
        }

        fn declared_data_fields<'a>(
            program: &'a dyn crate::CheckingInput,
            definition: &'a DataDefinition,
        ) -> Vec<&'a DataField> {
            program
                .data_members(definition)
                .iter()
                .filter_map(|member| match member {
                    DataMember::Field(field) => Some(field),
                    DataMember::Variant(_) => None,
                })
                .collect()
        }

        fn field_intervals(
            program: &dyn crate::CheckingInput,
            fields: &[&DataField],
            owner: symbols::SymbolHandle,
            facts: &[ProofFact],
        ) -> Option<Vec<(symbols::SymbolHandle, Option<i64>, Option<i64>)>> {
            let mut intervals: Vec<(symbols::SymbolHandle, Interval)> = Vec::new();
            let mut conjuncts = facts
                .iter()
                .map(|fact| fact.as_expression().copied())
                .collect::<Option<Vec<_>>>()?;
            while let Some(conjunct) = conjuncts.pop() {
                if let Some(binary) = expression_table(program).binary_node(conjunct)
                    && binary.operator == BinaryOperator::And
                {
                    conjuncts.extend([binary.left, binary.right]);
                    continue;
                }
                let rows = conjunct_field_intervals(program, fields, owner, conjunct)?;
                for (field, stated) in rows {
                    match intervals.iter_mut().find(|(symbol, _)| *symbol == field) {
                        Some((_, interval)) => *interval = interval.intersect(stated),
                        None => intervals.push((field, stated)),
                    }
                }
            }
            Some(
                intervals
                    .into_iter()
                    .map(|(symbol, interval)| (symbol, interval.low, interval.high))
                    .collect(),
            )
        }

        fn conjunct_field_intervals(
            program: &dyn crate::CheckingInput,
            fields: &[&DataField],
            owner: symbols::SymbolHandle,
            conjunct: ExpressionHandle,
        ) -> Option<Vec<(symbols::SymbolHandle, Interval)>> {
            if let Some(bounds) =
                field_literal_bounds(program, fields, &[ProofFact::Expression(conjunct)])
            {
                bounds
                    .into_iter()
                    .map(|bound| {
                        if !literal_comparison_has_builtin_meaning(program, fields, owner, bound) {
                            return None;
                        }
                        let value = expression_table(program).integer_node(bound.literal)?;
                        let bound_value = i64::try_from(value.decimal_value_i128()?).ok()?;
                        let interval = BigIntInterval::from_comparison(
                            bound.operator,
                            true,
                            numerics::bignum::BigInt::from_i64(bound_value),
                        )?;
                        Some((bound.field, interval.to_interval()?))
                    })
                    .collect()
            } else {
                exact_predicate_field_intervals(program, fields, owner, conjunct)
            }
        }

        fn literal_comparison_has_builtin_meaning(
            program: &dyn crate::CheckingInput,
            fields: &[&DataField],
            owner: symbols::SymbolHandle,
            bound: FieldLiteralBound,
        ) -> bool {
            use language_core::OperatorSpelling;

            let Some(field) = fields.iter().find(|field| field.symbol == bound.field) else {
                return false;
            };
            let Some(binary) = expression_table(program).binary_node(bound.conjunct) else {
                return false;
            };
            let spelling = match binary.operator {
                BinaryOperator::Less => OperatorSpelling::Less,
                BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
                BinaryOperator::Greater => OperatorSpelling::Greater,
                BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
                BinaryOperator::Equal => OperatorSpelling::Equal,
                _ => return false,
            };
            let field_type = Some(field.type_reference);
            let literal_type = crate::landed_integer_literal_type_reference(program, bound.literal);
            let operands = if side_selects_field(program, binary.left, field.symbol) {
                [field_type, literal_type]
            } else {
                [literal_type, field_type]
            };
            typed_trees::operator::has_builtin_spelled_expression_meaning(
                program,
                owner,
                bound.conjunct,
                spelling,
                &operands,
            )
        }

        fn exact_predicate_field_intervals(
            program: &dyn crate::CheckingInput,
            fields: &[&DataField],
            owner: symbols::SymbolHandle,
            expression: ExpressionHandle,
        ) -> Option<Vec<(symbols::SymbolHandle, Interval)>> {
            let application = crate::proof_contracts::collection_predicates::predicate_application(
                program, owner, expression,
            )?;
            let rows = crate::proof_contracts::collection_predicates::exact_predicate_intervals(
                program, owner, expression,
            )?;
            let entry = program
                .machine_states(&program.machines()[application.machine])
                .first()?;
            let parameters = program.state_parameters(entry);
            if rows.len() != parameters.len() || rows.len() != application.arguments.len() {
                return None;
            }
            rows.into_iter()
                .zip(parameters)
                .zip(application.arguments)
                .map(|((row, parameter), actual)| {
                    if row.extent || row.argument != *actual {
                        return None;
                    }
                    let path = expression_table(program).name_path_node(*actual)?;
                    let field = fields
                        .iter()
                        .find(|field| field.symbol.is_valid() && field.symbol == path.symbol)?;
                    let primitive = program.primitive_type_reference(field.type_reference)?;
                    if crate::proof_contracts::collection_predicates::bare_integer_parameter_type(
                        program,
                        parameter.type_reference,
                    ) != Some(primitive)
                    {
                        return None;
                    }
                    Some((
                        field.symbol,
                        Interval {
                            low: Some(i64::try_from(row.low).ok()?),
                            high: Some(i64::try_from(row.high).ok()?),
                        },
                    ))
                })
                .collect()
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct FieldLiteralBound {
            pub(crate) field: symbols::SymbolHandle,
            pub(crate) operator: BinaryOperator,
            pub(crate) literal: ExpressionHandle,
            pub(crate) conjunct: ExpressionHandle,
        }

        #[derive(Debug, Clone, Copy)]
        pub(crate) enum FieldRepresentationBound {
            Integer {
                field: symbols::SymbolHandle,
                interval: Interval,
            },
            FloatLiteral(FieldLiteralBound),
        }

        impl FieldRepresentationBound {
            pub(crate) fn field(self) -> symbols::SymbolHandle {
                match self {
                    Self::Integer { field, .. } => field,
                    Self::FloatLiteral(bound) => bound.field,
                }
            }
        }

        pub(crate) fn data_requires_field_representation_bounds(
            program: &dyn crate::CheckingInput,
            definition: &DataDefinition,
        ) -> Option<Vec<FieldRepresentationBound>> {
            use typed_trees::types::PrimitiveType;

            let fields = declared_data_fields(program, definition);
            let mut conjuncts = program
                .proof_facts
                .span_or_empty(definition.requires_facts)
                .iter()
                .map(|fact| fact.as_expression().copied())
                .collect::<Option<Vec<_>>>()?;
            let mut normalized = Vec::new();
            while let Some(conjunct) = conjuncts.pop() {
                if let Some(binary) = expression_table(program).binary_node(conjunct)
                    && binary.operator == BinaryOperator::And
                {
                    conjuncts.extend([binary.left, binary.right]);
                    continue;
                }
                if let Some(bounds) =
                    field_literal_bounds(program, &fields, &[ProofFact::Expression(conjunct)])
                    && bounds.iter().all(|bound| {
                        fields
                            .iter()
                            .find(|field| field.symbol == bound.field)
                            .and_then(|field| {
                                program.primitive_type_reference(field.type_reference)
                            })
                            .is_some_and(|primitive| {
                                matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)
                            })
                    })
                {
                    normalized.extend(
                        bounds
                            .into_iter()
                            .map(FieldRepresentationBound::FloatLiteral),
                    );
                    continue;
                }
                for (field, interval) in
                    conjunct_field_intervals(program, &fields, definition.symbol, conjunct)?
                {
                    let declaration = fields.iter().find(|candidate| candidate.symbol == field)?;
                    if !program
                        .primitive_type_reference(declaration.type_reference)?
                        .accepts_integer_literal()
                    {
                        return None;
                    }
                    normalized.push(FieldRepresentationBound::Integer { field, interval });
                }
            }
            Some(normalized)
        }

        fn field_literal_bounds(
            program: &dyn crate::CheckingInput,
            fields: &[&DataField],
            facts: &[ProofFact],
        ) -> Option<Vec<FieldLiteralBound>> {
            let mut bounds = Vec::new();
            let mut conjuncts = Vec::new();
            for fact in facts {
                let expression = fact.as_expression()?;
                conjuncts.push(*expression);
            }
            while let Some(conjunct) = conjuncts.pop() {
                let binary = expression_table(program).binary_node(conjunct)?;
                if binary.operator == BinaryOperator::And {
                    conjuncts.extend([binary.left, binary.right]);
                    continue;
                }
                let is_literal = |expression| {
                    matches!(
                        expression_table(program).expression(expression),
                        ExpressionNode::Integer(_) | ExpressionNode::Float(_)
                    )
                };
                let field_named = |expression| {
                    fields
                        .iter()
                        .find(|field| side_selects_field(program, expression, field.symbol))
                };
                let (field, literal, field_on_left) = match (
                    field_named(binary.left),
                    is_literal(binary.right),
                    field_named(binary.right),
                    is_literal(binary.left),
                ) {
                    (Some(field), true, _, _) => (field, binary.right, true),
                    (_, _, Some(field), true) => (field, binary.left, false),
                    _ => return None,
                };
                let operator = if field_on_left {
                    binary.operator
                } else {
                    binary.operator.mirrored()
                };
                if !matches!(
                    operator,
                    BinaryOperator::LessOrEqual
                        | BinaryOperator::Less
                        | BinaryOperator::GreaterOrEqual
                        | BinaryOperator::Greater
                        | BinaryOperator::Equal
                ) {
                    return None;
                }
                bounds.push(FieldLiteralBound {
                    field: field.symbol,
                    operator,
                    literal,
                    conjunct,
                });
            }
            Some(bounds)
        }

        pub(crate) fn atomic_field_interval(
            program: &dyn crate::CheckingInput,
            definition: &DataDefinition,
            field: &DataField,
        ) -> Option<Interval> {
            if crate::reference_shell(program, field.type_reference).is_some()
                || !crate::is_atomic_type(program, field.type_reference)
            {
                return None;
            }
            data_requires_field_intervals(program, definition)?
                .into_iter()
                .find(|(symbol, _, _)| *symbol == field.symbol)
                .map(|(_, low, high)| Interval { low, high })
        }

        /*
            The interval that the `requires` facts reading `field` place on it,
            when each such fact is a conjunction of literal bounds on `field`
            alone. Unlike `data_requires_field_intervals`, a sibling's float
            bound does not hide this field's interval; a fact relating `field`
            to anything else leaves no interval.
        */
        pub(crate) fn single_field_requires_interval(
            program: &dyn crate::CheckingInput,
            definition: &DataDefinition,
            field: &DataField,
        ) -> Option<Interval> {
            let fields = declared_data_fields(program, definition);
            let mut interval = None;
            for fact in program.proof_facts.span_or_empty(definition.requires_facts) {
                let read = super::place_queries::data_predicate_fields(program, definition, fact);
                if !read.contains(&field.symbol) {
                    continue;
                }
                if read != [field.symbol] {
                    return None;
                }
                let rows = field_intervals(
                    program,
                    &fields,
                    definition.symbol,
                    std::slice::from_ref(fact),
                )?;
                for (symbol, low, high) in rows {
                    if symbol != field.symbol {
                        return None;
                    }
                    interval = Some(
                        interval
                            .unwrap_or(Interval::UNBOUNDED)
                            .intersect(Interval { low, high }),
                    );
                }
            }
            interval
        }

        fn side_selects_field(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            field: symbols::SymbolHandle,
        ) -> bool {
            field.is_valid()
                && expression_table(program)
                    .name_path_node(expression)
                    .is_some_and(|path| path.symbol == field)
        }
    }

    pub use crate::proof_contracts::contract_entailment::data_predicates_admit_zero;
    pub use case_requirements::case_predicate_expressions;
    pub(crate) use case_requirements::selected_case_requirements;
    pub use float_predicates::data_float_predicate;
    pub(crate) use float_predicates::float_field_literal_bound_meaning;
    pub(crate) use place_queries::data_predicate_fields;
    pub(crate) use place_queries::membership_field;
    pub(crate) use place_queries::predicate_projection;
    pub(crate) use requires_fact_intervals::{
        FieldRepresentationBound, atomic_field_interval, data_requires_field_representation_bounds,
    };
    pub use requires_fact_intervals::{
        case_requires_field_intervals, data_requires_field_intervals, impossible_variant,
    };
    pub(crate) use symbolic_values::applied_const_integer;
}

pub mod domains {
    pub(crate) mod closed_instances {
        use language_semantics::SemanticDomainId;
        use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
        use numerics::bignum::BigInt;
        use symbols::SymbolHandle;
        use typed_trees::data::TypeParameterKind;
        use typed_trees::domain::DomainDefinition;
        use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

        pub(crate) fn closed_integer_index_bindings(
            program: &dyn crate::CheckingInput,
            definition: &DomainDefinition,
            arguments: &[TypeReferenceHandle],
            semantic_domain: SemanticDomainId,
        ) -> Option<Vec<(SymbolHandle, BigInt)>> {
            let parameters = typed_trees::domain::index_parameters(program, definition);
            if parameters.len() != arguments.len()
                || definition.index_arguments.len() != parameters.len()
                || typed_trees::domain::has_generic_carrier(program, definition)
            {
                return None;
            }
            if !parameters
                .iter()
                .zip(&definition.index_arguments)
                .all(|(parameter, argument)| {
                    matches!(program.type_reference_table.type_reference(*argument),
                TypeReferenceNode::Named { symbol, .. } if *symbol == parameter.symbol)
                })
            {
                return None;
            }
            let identity = typed_trees::domain::indexed_domain_instance_name(
                program, definition, parameters, arguments,
            )
            .ok()?;
            if !semantic_domain.is_valid()
                || program.semantic_domains.name(semantic_domain) != Some(identity.as_str())
            {
                return None;
            }
            let mut integer_bindings = Vec::new();
            for (parameter, argument) in parameters.iter().zip(arguments) {
                if !parameter.symbol.is_valid() {
                    return None;
                }
                if matches!(parameter.kind, TypeParameterKind::Type) {
                    program
                        .require_closed_application_operands(std::slice::from_ref(argument))
                        .ok()?;
                    continue;
                }
                let TypeParameterKind::Const { type_reference } = parameter.kind else {
                    return None;
                };
                let carrier = program.primitive_type_reference(type_reference)?;
                let (minimum, maximum) =
                    crate::proof_contracts::proof_embeddings::primitive_range(carrier)?;
                let value = match program.type_reference_table.type_reference(*argument) {
                    TypeReferenceNode::Named { symbol, name } if !symbol.is_valid() => {
                        if let Some(canonical) = CanonicalConstValue::from_atom(name.as_str()) {
                            if PrimitiveType::from_name(&canonical.type_name) != Some(carrier) {
                                return None;
                            }
                            let DecodedCanonicalConstValue::Integer { value, .. } =
                                canonical.decode_encoding()?
                            else {
                                return None;
                            };
                            BigInt::from_i128(value)
                        } else {
                            BigInt::from_decimal_str(name.as_str())?
                        }
                    }
                    TypeReferenceNode::ConstExpression(expression) => {
                        crate::closed_integer_range_bound(program, *expression)?
                    }
                    _ => return None,
                };
                if value < minimum || value > maximum {
                    return None;
                }
                integer_bindings.push((parameter.symbol, value));
            }
            Some(integer_bindings)
        }
    }
    pub(crate) use closed_instances::closed_integer_index_bindings;
    use language_semantics::CarryPermission;

    pub(crate) mod membership_carriers {
        use diagnostics::Diagnostic;
        use symbols::SymbolHandle;
        use typed_trees::domain::DomainDefinition;
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::types::TypeReferenceHandle;

        pub fn validate_domain_membership_carriers(
            program: &dyn crate::CheckingInput,
            contracts: &arena::Arena<checked_trees::ContractProofFact>,
            owners: &crate::expression_owners::OwnerEnvironmentIndex,
        ) -> Result<(), Diagnostic> {
            for (expression, _) in program.expression_table.iter_expressions() {
                for test in program.expression_table.executable_domain_tests(expression) {
                    let Some(domain) = program.domain_definition(test.domain) else {
                        continue;
                    };
                    validate_membership_carrier(
                        program,
                        contracts,
                        owners,
                        expression,
                        test.subject,
                        domain,
                        MembershipSite::ExecutableTest,
                    )?;
                }
            }
            for (_, fact) in program.proof_facts.iter() {
                let typed_trees::domain::ProofFact::Membership(membership) = fact else {
                    continue;
                };
                if !membership.value.is_valid() {
                    continue;
                }
                let Some(domain) = program
                    .domain_definition_for_instance(
                        membership.domain_symbol,
                        membership.semantic_domain,
                    )
                    .or_else(|| program.domain_definition(membership.domain_symbol))
                else {
                    continue;
                };
                validate_membership_carrier(
                    program,
                    contracts,
                    owners,
                    membership.value,
                    membership.value,
                    domain,
                    MembershipSite::ProofFact,
                )?;
            }
            Ok(())
        }

        /*
            Where a membership whose carriers do not match on their face was
            written. A domain over a generic carrier binds that carrier at each
            use and is matched where the arguments are known, at either site.

            A proof fact (`requires value in Small` with `value: T`) is a
            hypothesis: each caller must establish it for its own carrier, so
            the generic declaration is judged on each specialized copy.

            An executable test in a body (`cell in Cell::Ready` with
            `cell: &Value`) is code the body runs for every admitted `Value`.
            generics.md: "An unspecialized body checks every admitted
            alternative; one consumer cannot authorize an otherwise invalid
            generic body." So it is refused even when the only caller passes a
            `Cell`, unless a declared equation such as `requires Value == Cell`
            made the carriers match.
        */
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum MembershipSite {
            ExecutableTest,
            ProofFact,
        }

        impl MembershipSite {
            fn label(self) -> &'static str {
                match self {
                    Self::ExecutableTest => "executable domain test",
                    Self::ProofFact => "domain membership",
                }
            }
        }

        fn validate_membership_carrier(
            program: &dyn crate::CheckingInput,
            contracts: &arena::Arena<checked_trees::ContractProofFact>,
            owners: &crate::expression_owners::OwnerEnvironmentIndex,
            owner_expression: ExpressionHandle,
            subject: ExpressionHandle,
            domain: &DomainDefinition,
            site: MembershipSite,
        ) -> Result<(), Diagnostic> {
            let Err(diagnostic) = validate_declaration_carrier(
                program,
                contracts,
                owners,
                owner_expression,
                subject,
                domain,
                site,
            ) else {
                return Ok(());
            };
            let same_identity = program.domain_definitions().iter().filter(|candidate| {
                candidate.symbol != domain.symbol
                    && domain.semantic_id != language_semantics::SemanticDomainId::NULL
                    && candidate.semantic_id == domain.semantic_id
            });
            for candidate in same_identity {
                if validate_declaration_carrier(
                    program,
                    contracts,
                    owners,
                    owner_expression,
                    subject,
                    candidate,
                    site,
                )
                .is_ok()
                {
                    return Ok(());
                }
            }
            Err(diagnostic)
        }

        fn validate_declaration_carrier(
            program: &dyn crate::CheckingInput,
            contracts: &arena::Arena<checked_trees::ContractProofFact>,
            owners: &crate::expression_owners::OwnerEnvironmentIndex,
            owner_expression: ExpressionHandle,
            subject: ExpressionHandle,
            domain: &DomainDefinition,
            site: MembershipSite,
        ) -> Result<(), Diagnostic> {
            let Some(subject_type) =
                crate::expression_owners::checked_expression_type_reference_from_exact_owner(
                    program,
                    contracts,
                    owner_expression,
                    subject,
                    Some(owners),
                )
                .and_then(|reference| crate::unwrapped_type_reference(program, reference))
            else {
                return Ok(());
            };
            let Some(target) = crate::unwrapped_type_reference(program, domain.target_type) else {
                return Ok(());
            };
            if crate::declared_carriers_match(program, subject_type, target) {
                return Ok(());
            }
            let substitutions = declared_type_equations(program, owners, owner_expression);
            let normalized_subject =
                executable_domain_carrier(program, subject_type, &substitutions);
            let normalized_target = executable_domain_carrier(program, target, &substitutions);
            if normalized_subject
                .zip(normalized_target)
                .is_some_and(|(subject, target)| {
                    crate::value_custody::places::declared_carriers_match_with_substitutions(
                        program,
                        subject,
                        target,
                        &substitutions,
                    )
                })
            {
                return Ok(());
            }
            let subject_type = normalized_subject.unwrap_or(subject_type);
            let target = normalized_target.unwrap_or(target);
            if mentions_type_parameter(program, target)
                || (site == MembershipSite::ProofFact
                    && mentions_type_parameter(program, subject_type))
            {
                return Ok(());
            }
            Err(Diagnostic::error(format!(
                "{} `{}` expects subject carrier `{}`, got `{}`",
                site.label(),
                domain.name.as_str(),
                program.display_type_reference_with_constraints(target),
                program.display_type_reference_with_constraints(subject_type),
            ))
            .with_source_span(program.expression_table.source_span(owner_expression)))
        }

        fn mentions_type_parameter(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> bool {
            let inputs =
                typed_trees::references::type_reference_inputs(program.typed_program(), reference);
            let parameters = inputs
                .symbols
                .into_iter()
                .filter(|symbol| {
                    program.symbols.get(*symbol).kind == symbols::SymbolKind::TypeParameter
                })
                .collect::<Vec<_>>();
            program.type_reference_mentions_any(reference, &parameters)
        }

        fn executable_domain_carrier(
            program: &dyn crate::CheckingInput,
            mut reference: TypeReferenceHandle,
            substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        ) -> Option<TypeReferenceHandle> {
            let mut visited = Vec::new();
            loop {
                if !reference.is_valid() || visited.contains(&reference) {
                    return None;
                }
                visited.push(reference);
                let substituted = crate::value_custody::places::substituted_declared_carrier(
                    program,
                    reference,
                    substitutions,
                )?;
                let unwrapped = crate::unwrapped_type_reference(program, substituted)?;
                if unwrapped == substituted {
                    return Some(unwrapped);
                }
                reference = unwrapped;
            }
        }

        fn declared_type_equations(
            program: &dyn crate::CheckingInput,
            owners: &crate::expression_owners::OwnerEnvironmentIndex,
            expression: ExpressionHandle,
        ) -> Vec<(SymbolHandle, TypeReferenceHandle)> {
            let Some(owner) =
                crate::expression_owners::source_declaration_owner(program, expression, owners)
            else {
                return Vec::new();
            };
            crate::value_custody::places::declaration_type_equations(program, owner)
        }
    }
    pub(crate) mod runtime_predicates {
        use crate::frozen_program::expression_table;
        use checked_trees::CheckedIeeeFloatComparisonKind;
        use checked_trees::{
            CheckedBooleanExpression as Boolean, CheckedIntegerBinaryKind,
            CheckedIntegerComparisonKind as Comparison, CheckedScalarExpression as Scalar,
        };
        use semantic_vocabulary::IeeeFloatValue;
        use typed_trees::domain::{DomainDefinition, ProofFact, ProofMembershipFact};
        use typed_trees::expression::TableCallExpression;
        use typed_trees::expression::{
            BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
        };
        use typed_trees::statement::TransitionGuardNode;
        use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

        pub fn scalar_membership_carrier(
            program: &dyn crate::CheckingInput,
            membership: &ProofMembershipFact,
        ) -> Option<PrimitiveType> {
            if crate::field_domains::domain_requires_provenance(program, membership.domain_symbol) {
                return None;
            }
            scalar_membership_value_carrier(program, membership)
        }

        pub fn scalar_membership_value_carrier(
            program: &dyn crate::CheckingInput,
            membership: &ProofMembershipFact,
        ) -> Option<PrimitiveType> {
            let arguments = program
                .type_reference_table
                .type_reference_handles(membership.domain_arguments);
            if arguments.len() != membership.domain_arguments.len() {
                return None;
            }
            Some(
                scalar_application(
                    program,
                    membership.domain_symbol,
                    membership.semantic_domain,
                    arguments,
                )?
                .primitive,
            )
        }

        pub(crate) struct ScalarApplication<'program> {
            declaration: &'program DomainDefinition,
            primitive: PrimitiveType,
            indices: Vec<(symbols::SymbolHandle, numerics::bignum::BigInt)>,
        }

        pub fn scalar_domain_local_predicate(
            program: &dyn crate::CheckingInput,
            domain: &DomainDefinition,
        ) -> Option<Boolean> {
            typed_trees::domain::required_subject_memberships(program.typed_program(), domain)?;
            let facts = program.proof_facts(domain);
            if facts
                .iter()
                .all(|fact| matches!(fact, ProofFact::Membership(_)))
            {
                return Some(Boolean::Constant(true));
            }
            if !domain.type_parameters.is_empty() || !domain.index_arguments.is_empty() {
                return None;
            }
            let primitive = program.primitive_type_reference(domain.target_type)?;
            let subject = Scalar::Parameter {
                position: 0,
                primitive_type: primitive,
            };
            let spelling = Predicate {
                program,
                domain,
                subject: &subject,
                primitive,
                indices: Vec::new(),
                substituted_formal: symbols::SymbolHandle::invalid(),
            };
            let mut predicates = Vec::new();
            for fact in facts {
                match fact {
                    ProofFact::Expression(expression) => {
                        predicates.push(spelling.boolean(*expression, 0)?);
                    }
                    ProofFact::Membership(_) => {}
                    ProofFact::Proposition(_) => return None,
                }
            }
            Some(
                predicates
                    .into_iter()
                    .reduce(conjunction)
                    .unwrap_or(Boolean::Constant(true)),
            )
        }

        pub fn scalar_domain_definition_predicate(
            program: &dyn crate::CheckingInput,
            symbol: symbols::SymbolHandle,
            identity: language_semantics::SemanticDomainId,
        ) -> Option<Boolean> {
            let definition = program
                .typed_program()
                .domain_definition_for_instance(symbol, identity)?;
            let arguments = program
                .typed_program()
                .domain_arguments_for_instance(symbol, identity)?;
            let primitive_type = program.primitive_type_reference(definition.target_type)?;
            let subject = Scalar::Parameter {
                position: 0,
                primitive_type,
            };
            let predicates = project(
                program,
                symbol,
                identity,
                arguments,
                &subject,
                &mut Vec::new(),
            )?;
            Some(
                predicates
                    .into_iter()
                    .reduce(conjunction)
                    .unwrap_or(Boolean::Constant(true)),
            )
        }

        fn scalar_application<'program>(
            program: &'program dyn crate::CheckingInput,
            symbol: symbols::SymbolHandle,
            semantic_domain: language_semantics::SemanticDomainId,
            arguments: &[TypeReferenceHandle],
        ) -> Option<ScalarApplication<'program>> {
            let domain = typed_trees::domain::domain_by_symbol(program, symbol)?;
            if program
                .domain_definitions()
                .iter()
                .filter(|row| row.symbol == domain.symbol)
                .count()
                != 1
                || !domain.symbol.is_valid()
                || program.symbols.get(domain.symbol).kind != symbols::SymbolKind::Domain
                || domain.alias.is_some()
                || !super::scalar_tags::exact_instance(program, domain, arguments, semantic_domain)
            {
                return None;
            }
            let indices = if arguments.is_empty() {
                Vec::new()
            } else {
                super::closed_integer_index_bindings(program, domain, arguments, semantic_domain)?
            };
            Some(ScalarApplication {
                declaration: domain,
                primitive: crate::value_custody::recasts::exact_primitive_type(
                    program,
                    domain.target_type,
                )?,
                indices,
            })
        }

        pub fn scalar_membership_predicates(
            program: &dyn crate::CheckingInput,
            membership: &ProofMembershipFact,
            subject: &Scalar,
        ) -> Option<Vec<Boolean>> {
            scalar_membership_carrier(program, membership)?;
            scalar_membership_value_predicates(program, membership, subject)
        }

        pub fn scalar_membership_value_predicates(
            program: &dyn crate::CheckingInput,
            membership: &ProofMembershipFact,
            subject: &Scalar,
        ) -> Option<Vec<Boolean>> {
            scalar_membership_value_carrier(program, membership)?;
            let arguments = program
                .type_reference_table
                .type_reference_handles(membership.domain_arguments);
            if arguments.len() != membership.domain_arguments.len() {
                return None;
            }
            project(
                program,
                membership.domain_symbol,
                membership.semantic_domain,
                arguments,
                subject,
                &mut Vec::new(),
            )
        }

        fn project(
            program: &dyn crate::CheckingInput,
            symbol: symbols::SymbolHandle,
            semantic_domain: language_semantics::SemanticDomainId,
            arguments: &[TypeReferenceHandle],
            subject: &Scalar,
            active: &mut Vec<symbols::SymbolHandle>,
        ) -> Option<Vec<Boolean>> {
            let application = scalar_application(program, symbol, semantic_domain, arguments)?;
            if subject.primitive_type() != Some(application.primitive) || active.contains(&symbol) {
                return None;
            }
            active.push(symbol);
            let domain = application.declaration;
            let spelling = Predicate {
                program,
                domain,
                subject,
                primitive: application.primitive,
                indices: application.indices,
                substituted_formal: symbols::SymbolHandle::invalid(),
            };
            let mut predicates = Vec::new();
            for fact in program.proof_facts.span(domain.facts)? {
                match fact {
                    ProofFact::Expression(expression) => {
                        predicates.push(spelling.boolean(*expression, 0)?)
                    }
                    ProofFact::Membership(nested) => {
                        scalar_membership_carrier(program, nested)?;
                        crate::exact_domain_subject_type(program, domain, nested.value)?;
                        let arguments = program
                            .type_reference_table
                            .type_reference_handles(nested.domain_arguments);
                        if arguments.len() != nested.domain_arguments.len() {
                            return None;
                        }
                        predicates.extend(project(
                            program,
                            nested.domain_symbol,
                            nested.semantic_domain,
                            arguments,
                            subject,
                            active,
                        )?);
                    }
                    ProofFact::Proposition(_) => return None,
                }
            }
            active.pop();
            Some(predicates)
        }

        pub(crate) struct Predicate<'program> {
            program: &'program dyn crate::CheckingInput,
            domain: &'program DomainDefinition,
            subject: &'program Scalar,
            primitive: PrimitiveType,
            indices: Vec<(symbols::SymbolHandle, numerics::bignum::BigInt)>,
            substituted_formal: symbols::SymbolHandle,
        }

        impl Predicate<'_> {
            fn boolean(&self, expression: ExpressionHandle, depth: usize) -> Option<Boolean> {
                if depth >= 128 || !expression_table(self.program).expression_is_valid(expression) {
                    return None;
                }
                if self
                    .program
                    .prepared_mathematics()
                    .is_some_and(|view| view.application(expression).is_some())
                {
                    return crate::mathematical_applications::scalar_predicate(
                        self.program,
                        expression,
                        |capture, _| self.scalar(capture, depth + 1),
                    );
                }
                if matches!(self.primitive, PrimitiveType::F32 | PrimitiveType::F64) {
                    return self.float_boolean(expression, depth);
                }
                match expression_table(self.program).expression(expression) {
                    ExpressionNode::Boolean(value) => Some(Boolean::Constant(*value)),
                    ExpressionNode::Name(path)
                        if self.primitive == PrimitiveType::Bool
                            && (crate::exact_domain_subject_type(
                                self.program,
                                self.domain,
                                expression,
                            )
                            .is_some()
                                || path.symbol == self.substituted_formal) =>
                    {
                        self.subject_boolean()
                    }
                    ExpressionNode::Call(call) => self.declaration_call(call, depth),
                    ExpressionNode::Member(member) => self.declaration_projection(member, depth),
                    ExpressionNode::Unary(unary)
                        if unary.operator == UnaryOperator::LogicalNot
                            && crate::has_builtin_domain_decomposed_guard_meaning(
                                self.program,
                                self.domain,
                                expression,
                            ) =>
                    {
                        Some(Boolean::Not(Box::new(
                            self.boolean(unary.operand, depth + 1)?,
                        )))
                    }
                    ExpressionNode::Binary(binary)
                        if crate::has_builtin_domain_decomposed_guard_meaning(
                            self.program,
                            self.domain,
                            expression,
                        ) =>
                    {
                        if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
                            let left = Box::new(self.boolean(binary.left, depth + 1)?);
                            let right = Box::new(self.boolean(binary.right, depth + 1)?);
                            return Some(if binary.operator == BinaryOperator::And {
                                Boolean::And { left, right }
                            } else {
                                Boolean::Or { left, right }
                            });
                        }
                        if self.primitive == PrimitiveType::Bool
                            && matches!(
                                binary.operator,
                                BinaryOperator::Equal | BinaryOperator::NotEqual
                            )
                        {
                            let value = Boolean::Equal {
                                left: Box::new(self.boolean(binary.left, depth + 1)?),
                                right: Box::new(self.boolean(binary.right, depth + 1)?),
                            };
                            return Some(if binary.operator == BinaryOperator::NotEqual {
                                Boolean::Not(Box::new(value))
                            } else {
                                value
                            });
                        }
                        let (kind, left, right, negated) = match binary.operator {
                            BinaryOperator::Equal => {
                                (Comparison::Equal, binary.left, binary.right, false)
                            }
                            BinaryOperator::NotEqual => {
                                (Comparison::Equal, binary.left, binary.right, true)
                            }
                            BinaryOperator::Less => {
                                (Comparison::LessThan, binary.left, binary.right, false)
                            }
                            BinaryOperator::LessOrEqual => {
                                (Comparison::LessOrEqual, binary.left, binary.right, false)
                            }
                            BinaryOperator::Greater => {
                                (Comparison::LessThan, binary.right, binary.left, false)
                            }
                            BinaryOperator::GreaterOrEqual => {
                                (Comparison::LessOrEqual, binary.right, binary.left, false)
                            }
                            _ => return None,
                        };
                        let comparison = Boolean::IntegerComparison {
                            kind,
                            left: Box::new(self.scalar(left, depth + 1)?),
                            right: Box::new(self.scalar(right, depth + 1)?),
                        };
                        Some(if negated {
                            Boolean::Not(Box::new(comparison))
                        } else {
                            comparison
                        })
                    }
                    _ => None,
                }
            }

            fn subject_boolean(&self) -> Option<Boolean> {
                match self.subject {
                    Scalar::Boolean(value) => Some((**value).clone()),
                    Scalar::Parameter { position, .. } => Some(Boolean::Parameter {
                        position: *position,
                    }),
                    Scalar::ErasedParameter { position, .. } => Some(Boolean::ErasedParameter {
                        position: *position,
                    }),
                    _ => None,
                }
            }

            fn declaration_call(
                &self,
                call: &TableCallExpression,
                depth: usize,
            ) -> Option<Boolean> {
                let (body, formal) = self.declaration_body(call)?;
                self.clone_with_formal(formal).boolean(body, depth + 1)
            }

            fn declaration_projection(
                &self,
                member: &typed_trees::expression::TableMemberExpression,
                depth: usize,
            ) -> Option<Boolean> {
                if member.case_variant.is_some() {
                    return None;
                }
                let mut fields = Vec::new();
                let mut receiver = member.receiver;
                fields.push((member.member_symbol, member.member.as_str().to_owned()));
                let call = loop {
                    match expression_table(self.program).expression(receiver) {
                        ExpressionNode::Member(inner) if inner.case_variant.is_none() => {
                            fields.push((inner.member_symbol, inner.member.as_str().to_owned()));
                            receiver = inner.receiver;
                        }
                        ExpressionNode::Call(call) => break call.clone(),
                        _ => return None,
                    }
                };
                let (mut body, formal) = self.declaration_body(&call)?;
                for (symbol, name) in fields.iter().rev() {
                    body = self.literal_field(body, *symbol, name)?;
                }
                self.clone_with_formal(formal).boolean(body, depth + 1)
            }

            fn literal_field(
                &self,
                expression: ExpressionHandle,
                symbol: symbols::SymbolHandle,
                name: &str,
            ) -> Option<ExpressionHandle> {
                let literal = expression_table(self.program).struct_literal_node(expression)?;
                if literal.case_name.is_some() {
                    return None;
                }
                let mut selected = None;
                for field in expression_table(self.program).struct_fields(literal.fields) {
                    let matches = if symbol.is_valid() && field.field_symbol.is_valid() {
                        field.field_symbol == symbol
                    } else {
                        field.name.as_str() == name
                    };
                    if !matches {
                        continue;
                    }
                    if selected.is_some() {
                        return None;
                    }
                    selected = Some(field.value);
                }
                selected
            }

            fn declaration_body(
                &self,
                call: &TableCallExpression,
            ) -> Option<(ExpressionHandle, symbols::SymbolHandle)> {
                if !self.call_receiver_is_declaration(call)
                    || !call.machine_arguments.is_empty()
                    || !call.evidence_arguments.is_empty()
                    || call.quotient_operation.is_some()
                    || call.private_layout_operation.is_some()
                    || call.installed_interrupt_operation.is_some()
                    || call.layout_query.is_some()
                    || call.native_code_layout.is_some()
                {
                    return None;
                }
                if let Some(dispatch) = &call.static_requirement_dispatch
                    && (dispatch.evidence_parameter.is_valid()
                        || !dispatch.realization_state.is_valid()
                        || dispatch.realization_state != call.target_symbol)
                {
                    return None;
                }
                let (machine, state) = crate::machine_calls::calls::machine_state_by_symbol(
                    self.program,
                    call.target_symbol,
                )?;
                if self.program.machine_states(machine).len() != 1 || !machine.body_is_present {
                    return None;
                }
                let parameters = self.program.state_parameters(state);
                let arguments = expression_table(self.program).expression_handles(call.arguments);
                let [parameter] = parameters else {
                    return None;
                };
                let [argument] = arguments else {
                    return None;
                };
                if parameter.is_mutable
                    || parameter.is_const
                    || self
                        .program
                        .primitive_type_reference(parameter.type_reference)
                        != Some(self.primitive)
                    || crate::exact_domain_subject_type(self.program, self.domain, *argument)
                        .is_none()
                {
                    return None;
                }
                let statements = self
                    .program
                    .statement_table
                    .statements(state.statement_nodes);
                let [statement] = statements else {
                    return None;
                };
                let body = if let Some(expression) = statement.as_expression() {
                    *expression
                } else {
                    let transition = statement.as_transition()?;
                    if transition.continuation.is_valid()
                        || !matches!(transition.guard, TransitionGuardNode::Always)
                    {
                        return None;
                    }
                    let typed_trees::statement::TransitionTargetNode::Value(value) = self
                        .program
                        .statement_table
                        .transition_target(transition.target)
                    else {
                        return None;
                    };
                    *value
                };
                Some((body, parameter.symbol))
            }

            fn call_receiver_is_declaration(&self, call: &TableCallExpression) -> bool {
                if !call.receiver.is_valid() {
                    return true;
                }
                let Some(path) = expression_table(self.program).name_path_node(call.receiver)
                else {
                    return false;
                };
                matches!(
                    self.program.symbols.get(path.symbol).kind,
                    symbols::SymbolKind::Module
                        | symbols::SymbolKind::Trait
                        | symbols::SymbolKind::Conformance
                        | symbols::SymbolKind::ConformanceParameter
                )
            }

            fn clone_with_formal(&self, formal: symbols::SymbolHandle) -> Self {
                Self {
                    program: self.program,
                    domain: self.domain,
                    subject: self.subject,
                    primitive: self.primitive,
                    indices: self.indices.clone(),
                    substituted_formal: formal,
                }
            }

            fn float_boolean(&self, expression: ExpressionHandle, depth: usize) -> Option<Boolean> {
                if depth >= 128 {
                    return None;
                }
                if let Some(binary) = expression_table(self.program).binary_node(expression)
                    && matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
                {
                    let left = Box::new(self.float_boolean(binary.left, depth + 1)?);
                    let right = Box::new(self.float_boolean(binary.right, depth + 1)?);
                    return Some(if binary.operator == BinaryOperator::And {
                        Boolean::And { left, right }
                    } else {
                        Boolean::Or { left, right }
                    });
                }
                let (operator, left, right) = self.float_comparison(expression)?;
                let (kind, left, right) = match operator {
                    BinaryOperator::Equal => (CheckedIeeeFloatComparisonKind::Equal, left, right),
                    BinaryOperator::NotEqual => {
                        (CheckedIeeeFloatComparisonKind::NotEqual, left, right)
                    }
                    BinaryOperator::Less => (CheckedIeeeFloatComparisonKind::LessThan, left, right),
                    BinaryOperator::LessOrEqual => {
                        (CheckedIeeeFloatComparisonKind::LessOrEqual, left, right)
                    }
                    BinaryOperator::Greater => {
                        (CheckedIeeeFloatComparisonKind::LessThan, right, left)
                    }
                    BinaryOperator::GreaterOrEqual => {
                        (CheckedIeeeFloatComparisonKind::LessOrEqual, right, left)
                    }
                    _ => return None,
                };
                Some(Boolean::ScalarIeeeFloatComparison {
                    kind,
                    left: Box::new(self.float_scalar(left)?),
                    right: Box::new(self.float_scalar(right)?),
                })
            }

            fn float_comparison(
                &self,
                expression: ExpressionHandle,
            ) -> Option<(BinaryOperator, ExpressionHandle, ExpressionHandle)> {
                if let Some(binary) = expression_table(self.program).binary_node(expression)
                    && matches!(
                        binary.operator,
                        BinaryOperator::Equal
                            | BinaryOperator::NotEqual
                            | BinaryOperator::Less
                            | BinaryOperator::LessOrEqual
                            | BinaryOperator::Greater
                            | BinaryOperator::GreaterOrEqual
                    )
                {
                    return Some((binary.operator, binary.left, binary.right));
                }
                let call = expression_table(self.program).call_node(expression)?;
                let (machine, _) =
                    crate::find_machine_by_entry_state(self.program, call.target_symbol)?;
                let binding = self
                    .program
                    .machine_token_binding_by_symbol(machine.symbol)?;
                let (operator, format) =
                    typed_trees::operator::primitive_float_binary_semantics(self.program, binding)?;
                let expected = match self.primitive {
                    PrimitiveType::F32 => numerics::literals::FloatFormat::F32,
                    PrimitiveType::F64 => numerics::literals::FloatFormat::F64,
                    _ => return None,
                };
                if format != expected {
                    return None;
                }
                let [left, right] =
                    expression_table(self.program).expression_handles(call.arguments)
                else {
                    return None;
                };
                Some((operator, *left, *right))
            }

            fn float_scalar(&self, expression: ExpressionHandle) -> Option<Scalar> {
                if crate::exact_domain_subject_type(self.program, self.domain, expression).is_some()
                {
                    return Some(self.subject.clone());
                }
                let literal = expression_table(self.program).expression(expression);
                let bits = match literal {
                    ExpressionNode::Float(literal) => match self.primitive {
                        PrimitiveType::F32 => {
                            IeeeFloatValue::Binary32(literal.value_f32().to_bits())
                        }
                        PrimitiveType::F64 => {
                            IeeeFloatValue::Binary64(literal.value_f64().to_bits())
                        }
                        _ => return None,
                    },
                    _ => return None,
                };
                Some(Scalar::IeeeFloatLiteral { value: bits })
            }

            fn scalar(&self, expression: ExpressionHandle, depth: usize) -> Option<Scalar> {
                if depth >= 128
                    || !self.primitive.accepts_integer_literal()
                    || self.primitive == PrimitiveType::Addr
                {
                    return None;
                }
                if crate::exact_domain_subject_type(self.program, self.domain, expression).is_some()
                {
                    return Some(self.subject.clone());
                }
                if let Some(symbol) = crate::single_segment_name_symbol(self.program, expression)
                    && let Some((_, value)) =
                        self.indices.iter().find(|(index, _)| *index == symbol)
                {
                    let reference =
                crate::value_custody::expression_types::domain_expression_result_type_reference(
                    self.program,
                    self.domain,
                    expression,
                )?;
                    if self.program.primitive_type_reference(reference) != Some(self.primitive) {
                        return None;
                    }
                    return Some(Scalar::IntegerLiteral {
                        literal: crate::land_integer_value(value, self.primitive)?,
                    });
                }
                if let ExpressionNode::Binary(binary) =
                    expression_table(self.program).expression(expression)
                {
                    let kind = match binary.operator {
                        BinaryOperator::BitwiseAnd => Some(CheckedIntegerBinaryKind::BitwiseAnd),
                        BinaryOperator::BitwiseOr => Some(CheckedIntegerBinaryKind::BitwiseOr),
                        BinaryOperator::BitwiseXor => Some(CheckedIntegerBinaryKind::BitwiseXor),
                        _ => None,
                    };
                    if let Some(kind) = kind {
                        let reference = crate::value_custody::expression_types::domain_expression_result_type_reference(
                    self.program, self.domain, expression,
                )?;
                        if self.program.primitive_type_reference(reference) != Some(self.primitive)
                        {
                            return None;
                        }
                        return Some(Scalar::IntegerBinary {
                            kind,
                            primitive_type: self.primitive,
                            left: Box::new(self.scalar(binary.left, depth + 1)?),
                            right: Box::new(self.scalar(binary.right, depth + 1)?),
                            formation_decided: false,
                            trap_free: false,
                        });
                    }
                }
                let value = self
                    .program
                    .closed_integer_value_in(expression, self.domain.symbol)?;
                if value
                    .primitive
                    .is_some_and(|primitive| primitive != self.primitive)
                {
                    return None;
                }
                Some(Scalar::IntegerLiteral {
                    literal: crate::land_integer_value(&value.value, self.primitive)?,
                })
            }
        }

        pub fn scalar_field_domain_predicate(
            program: &dyn crate::CheckingInput,
            constraint: &typed_trees::types::DomainConstraint,
            primitive: PrimitiveType,
        ) -> Option<Boolean> {
            scalar_domain_predicate(
                program,
                constraint,
                Scalar::Parameter {
                    position: 0,
                    primitive_type: primitive,
                },
            )
        }

        pub fn scalar_domain_predicate(
            program: &dyn crate::CheckingInput,
            constraint: &typed_trees::types::DomainConstraint,
            subject: Scalar,
        ) -> Option<Boolean> {
            if !constraint.establishment_routes.is_empty() {
                return None;
            }
            scalar_domain_value_predicate(program, constraint, subject)
        }

        pub fn scalar_domain_value_predicate(
            program: &dyn crate::CheckingInput,
            constraint: &typed_trees::types::DomainConstraint,
            subject: Scalar,
        ) -> Option<Boolean> {
            let primitive = subject.primitive_type()?;
            if let typed_trees::types::DomainConstraintSubject::Value(domain) = constraint.subject {
                return crate::scalar_value_domain_predicate(domain, subject);
            }
            if let Some((minimum, maximum)) =
                crate::exact_declared_domain_carrier_interval(program, primitive, constraint)
            {
                if minimum > maximum {
                    return None;
                }
                let endpoint = |value: &numerics::bignum::BigInt| {
                    Some(Scalar::IntegerLiteral {
                        literal: crate::land_integer_value(value, primitive)?,
                    })
                };
                return Some(conjunction(
                    Boolean::IntegerComparison {
                        kind: Comparison::LessOrEqual,
                        left: Box::new(endpoint(&minimum)?),
                        right: Box::new(subject.clone()),
                    },
                    Boolean::IntegerComparison {
                        kind: Comparison::LessOrEqual,
                        left: Box::new(subject),
                        right: Box::new(endpoint(&maximum)?),
                    },
                ));
            }
            if super::scalar_tags::scalar_tag_constraint(program, constraint, primitive) {
                return Some(Boolean::Constant(true));
            }
            if constraint.subject != typed_trees::types::DomainConstraintSubject::Declared {
                return None;
            }
            let predicates = project(
                program,
                constraint.symbol,
                constraint.semantic_id,
                &constraint.arguments,
                &subject,
                &mut Vec::new(),
            )?;
            Some(
                predicates
                    .into_iter()
                    .reduce(conjunction)
                    .unwrap_or(Boolean::Constant(true)),
            )
        }

        fn conjunction(left: Boolean, right: Boolean) -> Boolean {
            Boolean::And {
                left: Box::new(left),
                right: Box::new(right),
            }
        }

        pub fn scalar_field_restriction(
            primitive: PrimitiveType,
            bounds: Option<semantic_vocabulary::BoundedIntegerType>,
            predicate: Option<Boolean>,
        ) -> Option<checked_trees::CheckedUnitStructuralFieldType> {
            use checked_trees::CheckedUnitStructuralFieldType as Field;
            let Some(mut predicate) = predicate else {
                return Some(bounds.map_or(Field::Scalar(primitive), Field::BoundedInteger));
            };
            if let Some(bounds) = bounds {
                let subject = Scalar::Parameter {
                    position: 0,
                    primitive_type: primitive,
                };
                let endpoint = |value| {
                    use numerics::bignum::BigInt;
                    let value = match value {
                        semantic_vocabulary::IntegerValue::Signed(value) => {
                            BigInt::from_i128(value)
                        }
                        semantic_vocabulary::IntegerValue::Unsigned(value) => {
                            BigInt::from_u128(value)
                        }
                    };
                    Some(Scalar::IntegerLiteral {
                        literal: crate::land_integer_value(&value, primitive)?,
                    })
                };
                predicate = conjunction(
                    predicate,
                    Boolean::And {
                        left: Box::new(Boolean::IntegerComparison {
                            kind: Comparison::LessOrEqual,
                            left: Box::new(endpoint(bounds.minimum())?),
                            right: Box::new(subject.clone()),
                        }),
                        right: Box::new(Boolean::IntegerComparison {
                            kind: Comparison::LessOrEqual,
                            left: Box::new(subject),
                            right: Box::new(endpoint(bounds.maximum())?),
                        }),
                    },
                );
            }
            Some(Field::RefinedScalar {
                primitive,
                predicate,
            })
        }
    }
    pub use membership_carriers::validate_domain_membership_carriers;
    pub use runtime_predicates::{
        scalar_domain_definition_predicate, scalar_domain_local_predicate, scalar_domain_predicate,
        scalar_domain_value_predicate, scalar_field_domain_predicate, scalar_field_restriction,
        scalar_membership_carrier, scalar_membership_predicates, scalar_membership_value_carrier,
        scalar_membership_value_predicates,
    };
    pub(crate) mod scalar_tags {
        use symbols::SymbolHandle;
        use typed_trees::domain::DomainDefinition;
        use typed_trees::types::{
            DomainConstraintSubject, PrimitiveType, TypeConstraintNode, TypeReferenceHandle,
            TypeReferenceNode,
        };

        fn constrained_scalar_domains(
            program: &dyn crate::CheckingInput,
            mut reference: TypeReferenceHandle,
            admits: impl Fn(
                &dyn crate::CheckingInput,
                &typed_trees::types::DomainConstraint,
                PrimitiveType,
            ) -> bool,
        ) -> Vec<(SymbolHandle, language_semantics::SemanticDomainId)> {
            let Some(primitive) = program.primitive_type_reference(reference) else {
                return Vec::new();
            };
            let mut domains = Vec::new();
            let mut seen = Vec::new();
            while !seen.contains(&reference) {
                seen.push(reference);
                let TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } = program.type_reference_table.type_reference(reference)
                else {
                    break;
                };
                for constraint in program.type_reference_table.constraints(*constraints) {
                    if let TypeConstraintNode::Domain(domain) = constraint
                        && admits(program, domain, primitive)
                        && !domains.contains(&(domain.symbol, domain.semantic_id))
                    {
                        domains.push((domain.symbol, domain.semantic_id));
                    }
                }
                reference = *base_type;
            }
            domains
        }

        pub fn scalar_type_tags(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> Vec<(SymbolHandle, language_semantics::SemanticDomainId)> {
            constrained_scalar_domains(program, reference, scalar_tag_constraint)
        }

        pub fn scalar_type_routed_domains(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> Vec<(SymbolHandle, language_semantics::SemanticDomainId)> {
            constrained_scalar_domains(program, reference, |program, domain, primitive| {
                domain.subject == DomainConstraintSubject::Declared
                    && !domain.establishment_routes.is_empty()
                    && super::scalar_domain_value_predicate(
                        program,
                        domain,
                        checked_trees::CheckedScalarExpression::Parameter {
                            position: 0,
                            primitive_type: primitive,
                        },
                    )
                    .is_some()
            })
        }

        pub(crate) fn scalar_tag_constraint(
            program: &dyn crate::CheckingInput,
            constraint: &typed_trees::types::DomainConstraint,
            primitive: PrimitiveType,
        ) -> bool {
            constraint.subject == DomainConstraintSubject::Declared
                && constraint.establishment_routes.is_empty()
                && program.domain_definitions().iter().any(|declaration| {
                    declaration.symbol == constraint.symbol
                        && exact_instance(
                            program,
                            declaration,
                            &constraint.arguments,
                            constraint.semantic_id,
                        )
                        && vacuous_scalar_tag(program, declaration, primitive, &mut Vec::new())
                })
        }

        pub fn scalar_membership_is_tag(
            program: &dyn crate::CheckingInput,
            membership: &typed_trees::domain::ProofMembershipFact,
            primitive: PrimitiveType,
        ) -> bool {
            let arguments = program
                .type_reference_table
                .type_reference_handles(membership.domain_arguments);
            arguments.len() == membership.domain_arguments.len()
                && program
                    .domain_definition(membership.domain_symbol)
                    .is_some_and(|declaration| {
                        exact_instance(program, declaration, arguments, membership.semantic_domain)
                            && vacuous_scalar_tag(program, declaration, primitive, &mut Vec::new())
                    })
        }

        pub fn scalar_predicate_domains(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> Vec<(SymbolHandle, language_semantics::SemanticDomainId)> {
            constrained_scalar_domains(program, reference, |program, domain, primitive| {
                domain.subject == DomainConstraintSubject::Declared
                    && !scalar_tag_constraint(program, domain, primitive)
                    && super::scalar_field_domain_predicate(program, domain, primitive).is_some()
            })
        }

        fn vacuous_scalar_tag(
            program: &dyn crate::CheckingInput,
            declaration: &DomainDefinition,
            primitive: PrimitiveType,
            active: &mut Vec<SymbolHandle>,
        ) -> bool {
            if !declaration.symbol.is_valid()
                || !declaration.semantic_id.is_valid()
                || active.contains(&declaration.symbol)
                || declaration.predicate_presence.is_present()
                || !declaration.facts.is_empty()
                || !declaration.establishment_routes.is_empty()
                || declaration.semantic_roles.arithmetic_policy.is_some()
                || (!typed_trees::domain::has_generic_carrier(program, declaration)
                    && program.primitive_type_reference(declaration.target_type) != Some(primitive))
            {
                return false;
            }
            let Some(alias) = &declaration.alias else {
                return true;
            };
            active.push(declaration.symbol);
            let valid = !alias.constituents.is_empty()
                && alias.constituents.iter().all(|constituent| {
                    program
                        .domain_definition(constituent.domain_symbol)
                        .is_some_and(|candidate| {
                            typed_trees::domain::index_parameters(program, candidate).is_empty()
                                && vacuous_scalar_tag(program, candidate, primitive, active)
                        })
                });
            active.pop();
            valid
        }

        pub(crate) fn exact_instance(
            program: &dyn crate::CheckingInput,
            domain: &DomainDefinition,
            arguments: &[TypeReferenceHandle],
            semantic_domain: language_semantics::SemanticDomainId,
        ) -> bool {
            let parameters = typed_trees::domain::index_parameters(program, domain);
            semantic_domain.is_valid()
                && parameters.len() == arguments.len()
                && arguments.iter().all(|argument| {
                    program
                        .type_reference_table
                        .contains_type_reference(*argument)
                })
                && typed_trees::domain::indexed_domain_instance_name(
                    program, domain, parameters, arguments,
                )
                .is_ok_and(|identity| {
                    program.semantic_domains.name(semantic_domain) == Some(identity.as_str())
                })
        }
    }
    pub(crate) use scalar_tags::scalar_tag_constraint;
    pub use scalar_tags::{
        scalar_membership_is_tag, scalar_predicate_domains, scalar_type_routed_domains,
        scalar_type_tags,
    };

    use crate::declarations::symbols::TopLevelSymbols;
    use crate::proof_contracts::proof_facts::{ProofFactOwner, validate_domain_fact_payloads};
    use crate::proof_contracts::properties::{
        declared_property_requirements, type_satisfies_declared_property,
    };
    use crate::value_custody::type_references::{
        TypeReferenceOwner, type_reference_label, type_references_match,
        validate_type_reference_handle,
    };
    use diagnostics::Diagnostic;
    use facts::FactPlan;
    use symbols::JoinedPath;
    use symbols::SymbolHandle;

    pub(crate) fn validate_domain_definitions(
        program: &dyn crate::CheckingInput,
        symbols: &TopLevelSymbols<'_>,
        fact_plan: &FactPlan,
        diagnostics: &mut Vec<Diagnostic>,
        formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
    ) {
        validate_domain_aliases(program, diagnostics);
        validate_progress_profile_domains(program, diagnostics);
        validate_repeated_normalized_domain_identities(program, fact_plan, diagnostics);
        validate_capacity_normalized_domain_agreement(program, fact_plan, diagnostics);

        for domain in program.domain_definitions() {
            validate_type_reference_handle(
                program,
                domain.target_type,
                symbols,
                diagnostics,
                TypeReferenceOwner::DomainTarget {
                    domain: domain.name.as_str(),
                    generic_depth: 0,
                },
                crate::value_custody::type_references::TypeParameterScope {
                    operand_lifetime_owner: symbols::SymbolHandle::invalid(),
                    lexical_owner: domain.symbol,
                    type_parameters: program.domain_type_parameters(domain),
                    lifetime_parameters: &[],
                },
                formation_obligations,
            );
            validate_domain_fact_payloads(
                program,
                fact_plan,
                domain,
                diagnostics,
                ProofFactOwner::Domain(domain.name.as_str()),
            );
            validate_domain_membership_targets(program, fact_plan, domain, diagnostics);
        }

        validate_domain_membership_cycles(program, fact_plan, diagnostics);
    }

    fn validate_progress_profile_domains(
        program: &dyn crate::CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for domain in program.domain_definitions().iter().filter(|domain| {
            domain.classification == Some(language_semantics::DomainClassification::ProgressProfile)
        }) {
            if domain.alias.is_some() {
                diagnostics.push(Diagnostic::error(format!(
                    "progress profile `{}` must be an atomic domain, not a transparent alias",
                    domain.name
                )));
            }
            if domain.predicate_presence.is_present() || !program.proof_facts(domain).is_empty() {
                diagnostics.push(Diagnostic::error(format!(
                    "progress profile `{}` must be predicate-free; remove its `requires` facts",
                    domain.name
                )));
            }
            if !program.domain_operators(domain).is_empty() || !domain.semantic_roles.is_empty() {
                diagnostics.push(Diagnostic::error(format!(
                "progress profile `{}` is opaque and cannot contribute operators or other domain semantic roles",
                domain.name
            )));
            }
            if domain.establishment_routes.is_empty() {
                diagnostics.push(Diagnostic::error(format!(
                "progress profile `{}` requires at least one exact `established by` boundary requirement",
                domain.name
            )));
            } else if domain.establishment_routes.iter().any(|route| {
                !matches!(
                    route,
                    language_semantics::DomainEstablishmentRoute::BoundaryRequirement { .. }
                )
            }) {
                diagnostics.push(Diagnostic::error(format!(
                "progress profile `{}` may be established only by exact boundary trait requirements",
                domain.name
            )));
            }
        }
    }

    fn validate_domain_aliases(
        program: &dyn crate::CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for domain in program.domain_definitions() {
            let Some(alias) = domain.alias.as_ref() else {
                continue;
            };
            if generic_alias_carrier_parameter(program, domain).is_some_and(|parameter| {
                parameter.bounds != typed_trees::data::DataProperties::default()
            }) {
                diagnostics.push(Diagnostic::error(format!(
                "domain alias `{}` requires typed carrier-bound application evidence before expansion",
                domain.name
            )));
            }
            if !typed_trees::domain::index_parameters(program, domain).is_empty() {
                diagnostics.push(Diagnostic::error(format!(
                    "domain alias `{}` requires typed index-application evidence before expansion",
                    domain.name
                )));
            }
            if alias.constituents.is_empty() {
                diagnostics.push(Diagnostic::error(format!(
                    "domain alias `{}` must name at least one constituent",
                    domain.name
                )));
            }
            if domain.predicate_presence.is_present()
                || !program.proof_facts(domain).is_empty()
                || !program.domain_operators(domain).is_empty()
            {
                diagnostics.push(Diagnostic::error(format!(
                "domain alias `{}` must be transparent; it cannot also declare predicate facts or operators",
                domain.name
            )));
            }

            for constituent in &alias.constituents {
                let label = program
                    .domain_path_members(constituent.domain)
                    .joined_path();
                if label == "Carry::Portable" || CarryPermission::from_name(&label).is_some() {
                    continue;
                }
                let Some(referenced) =
                    domain_definition_by_symbol(program, constituent.domain_symbol)
                else {
                    if label.starts_with("Carry::") {
                        diagnostics.push(Diagnostic::error(format!(
                        "domain alias `{}` references unknown compiler carry permission `{label}`; expected `Carry::AcrossSuspend`, `Carry::AnyCpu`, `Carry::AnyThread`, `Carry::MovableAddress`, or `Carry::Portable`",
                        domain.name
                    )));
                    } else {
                        diagnostics.push(Diagnostic::error(format!(
                            "domain alias `{}` references unknown domain `{label}`",
                            domain.name
                        )));
                    }
                    continue;
                };
                if let Some(parameter) = generic_alias_carrier_parameter(program, referenced) {
                    for requirement in declared_property_requirements(&parameter.bounds) {
                        if !type_satisfies_declared_property(
                            program,
                            program.domain_type_parameters(domain),
                            domain.target_type,
                            requirement,
                        ) {
                            diagnostics.push(Diagnostic::error(format!(
                            "domain alias `{}` cannot apply constituent `{}`: its carrier requires `{requirement}`",
                            domain.name, referenced.name
                        )));
                        }
                    }
                } else if !type_references_match(
                    program,
                    domain.target_type,
                    referenced.target_type,
                ) {
                    diagnostics.push(Diagnostic::error(format!(
                    "domain alias `{}` includes `{}` but they classify different types: `{}` vs `{}`",
                    domain.name,
                    referenced.name,
                    type_reference_label(program, domain.target_type),
                    type_reference_label(program, referenced.target_type)
                )));
                }
                let index_count = typed_trees::domain::index_parameters(program, referenced).len();
                if index_count != 0 {
                    diagnostics.push(Diagnostic::error(format!(
                    "domain alias `{}` supplies no index arguments to constituent `{}`, which requires {index_count}",
                    domain.name, referenced.name
                )));
                }
                if domain.is_public && !referenced.is_public {
                    diagnostics.push(Diagnostic::error(format!(
                        "public domain alias `{}` cannot publish private constituent `{}`",
                        domain.name, referenced.name
                    )));
                }
            }
        }

        let mut reported = Vec::new();
        for domain in program.domain_definitions() {
            validate_domain_alias_cycle_from(
                program,
                domain.symbol,
                &mut Vec::new(),
                &mut reported,
                diagnostics,
            );
        }
    }

    fn generic_alias_carrier_parameter<'program>(
        program: &'program dyn crate::CheckingInput,
        domain: &typed_trees::domain::DomainDefinition,
    ) -> Option<&'program typed_trees::data::TypeParameter> {
        let parameter = program.domain_type_parameters(domain).first()?;
        let typed_trees::types::TypeReferenceNode::Named { symbol, .. } = program
            .type_reference_table
            .type_reference(domain.target_type)
        else {
            return None;
        };
        (matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
            && parameter.symbol.is_valid()
            && *symbol == parameter.symbol)
            .then_some(parameter)
    }

    fn validate_domain_alias_cycle_from(
        program: &dyn crate::CheckingInput,
        domain_symbol: SymbolHandle,
        path: &mut Vec<SymbolHandle>,
        reported: &mut Vec<SymbolHandle>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if !domain_symbol.is_valid() || reported.contains(&domain_symbol) {
            return;
        }
        if let Some(cycle_start) = path.iter().position(|symbol| *symbol == domain_symbol) {
            let cycle_symbols = path[cycle_start..]
                .iter()
                .copied()
                .chain(std::iter::once(domain_symbol))
                .collect::<Vec<_>>();
            let cycle = cycle_symbols
                .iter()
                .filter_map(|symbol| domain_definition_by_symbol(program, *symbol))
                .map(|domain| domain.name.to_string())
                .collect::<Vec<_>>()
                .join(" -> ");
            diagnostics.push(Diagnostic::error(format!("domain alias cycle: {cycle}")));
            reported.extend(cycle_symbols);
            return;
        }

        let Some(domain) = domain_definition_by_symbol(program, domain_symbol) else {
            return;
        };
        let Some(alias) = domain.alias.as_ref() else {
            return;
        };
        path.push(domain_symbol);
        for constituent in &alias.constituents {
            validate_domain_alias_cycle_from(
                program,
                constituent.domain_symbol,
                path,
                reported,
                diagnostics,
            );
        }
        path.pop();
    }

    fn validate_repeated_normalized_domain_identities(
        program: &dyn crate::CheckingInput,
        fact_plan: &FactPlan,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let domains = program.domain_definitions();
        for (domain_index, domain) in domains.iter().enumerate() {
            if domain.semantic_id.is_valid()
                && domains[..domain_index].iter().any(|prior| {
                    prior.semantic_id == domain.semantic_id
                        && !same_normalized_domain_owner(program, prior, domain)
                })
            {
                diagnostics.push(Diagnostic::error(format!(
                "domain `{}` shares a normalized semantic identity with a distinct declaration owner; domain identity must retain package and module ownership",
                domain.name,
            )));
            }
            if domains[..domain_index]
                .iter()
                .any(|prior| same_normalized_domain_owner(program, prior, domain))
            {
                continue;
            }
            let normalized_facts = match normalized_domain_facts(program, fact_plan, domain.symbol)
            {
                Ok(facts) => facts,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            let mut peers = domains
                .iter()
                .skip(domain_index + 1)
                .filter(|peer| same_normalized_domain_owner(program, peer, domain));
            if peers.any(|peer| {
                peer.semantic_id != domain.semantic_id
                    || peer.predicate_presence != domain.predicate_presence
                    || peer.classification != domain.classification
                    || peer.semantic_roles != domain.semantic_roles
                    || peer.establishment_routes != domain.establishment_routes
                    || match normalized_domain_facts(program, fact_plan, peer.symbol) {
                        Ok(facts) => facts != normalized_facts,
                        Err(diagnostic) => {
                            diagnostics.push(diagnostic);
                            false
                        }
                    }
            }) {
                diagnostics.push(Diagnostic::error(format!(
                "domain `{}` is declared more than once with different normalized semantics; repeated capacity specializations may share a name only when their semantic identities are equal",
                domain.name
            )));
            }
        }
    }

    fn validate_capacity_normalized_domain_agreement(
        program: &dyn crate::CheckingInput,
        fact_plan: &FactPlan,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let domains = program.domain_definitions();
        for (domain_index, domain) in domains.iter().enumerate() {
            if !domain_target_normalizes_its_capacity(program, domain)
                || domains[..domain_index]
                    .iter()
                    .any(|prior| prior.name == domain.name)
            {
                continue;
            }
            let normalized_facts = match normalized_domain_facts(program, fact_plan, domain.symbol)
            {
                Ok(facts) => facts,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            if domains
                .iter()
                .skip(domain_index + 1)
                .filter(|peer| {
                    peer.name == domain.name
                        && domain_target_normalizes_its_capacity(program, peer)
                        && !same_normalized_domain_owner(program, peer, domain)
                })
                .any(
                    |peer| match normalized_domain_facts(program, fact_plan, peer.symbol) {
                        Ok(facts) => facts != normalized_facts,
                        Err(diagnostic) => {
                            diagnostics.push(diagnostic);
                            false
                        }
                    },
                )
            {
                diagnostics.push(Diagnostic::error(format!(
                "domain `{}` is declared over different bounded carriers with different normalized facts; capacity specializations of one name must agree",
                domain.name
            )));
            }
        }
    }

    fn domain_target_normalizes_its_capacity(
        program: &dyn crate::CheckingInput,
        domain: &typed_trees::domain::DomainDefinition,
    ) -> bool {
        matches!(
            program
                .type_reference_table
                .type_reference(domain.target_type),
            typed_trees::types::TypeReferenceNode::FixedArray { .. }
        )
    }

    fn same_normalized_domain_owner(
        program: &dyn crate::CheckingInput,
        left: &typed_trees::domain::DomainDefinition,
        right: &typed_trees::domain::DomainDefinition,
    ) -> bool {
        left.name == right.name
            && program.symbols.symbol_module(left.symbol)
                == program.symbols.symbol_module(right.symbol)
            && program
                .symbols
                .same_symbol_source_package(left.symbol, right.symbol)
    }

    fn normalized_domain_facts(
        program: &dyn crate::CheckingInput,
        fact_plan: &FactPlan,
        domain_symbol: SymbolHandle,
    ) -> Result<Vec<String>, Diagnostic> {
        let binders = domain_definition_by_symbol(program, domain_symbol)
            .map(|domain| {
                program
                    .domain_type_parameters(domain)
                    .iter()
                    .enumerate()
                    .map(|(position, parameter)| (parameter.symbol, format!("$index{position}")))
                    .chain(std::iter::once((
                        domain.subject_symbol,
                        "$subject".to_owned(),
                    )))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let expression_identity = |expression| {
            typed_trees::type_identity::normalized_expression_identity(program, expression, &binders)
            .ok_or_else(|| Diagnostic::error(format!("domain `{}` has a fact with no complete normalized expression identity; an unbound captured subject cannot establish domain agreement", program.symbols.name(domain_symbol))))
        };
        let mut normalized = Vec::new();
        for fact in fact_plan.facts_for_symbol(domain_symbol) {
            let identity = match fact.payload {
                facts::FactPayload::BooleanExpression(expression) => {
                    Some(format!("bool:{}", expression_identity(expression)?))
                }
                facts::FactPayload::DomainMembership {
                    value,
                    domain_symbol,
                    semantic_domain,
                    ..
                } => {
                    let semantic_id = if semantic_domain.is_valid() {
                        semantic_domain
                    } else {
                        domain_definition_by_symbol(program, domain_symbol)
                            .map(|domain| domain.semantic_id)
                            .unwrap_or_default()
                    };
                    Some(format!(
                        "membership:{}:{semantic_id:?}",
                        expression_identity(value)?
                    ))
                }
                _ => None,
            };
            if let Some(identity) = identity {
                normalized.push(identity);
            }
        }
        normalized.sort_unstable();
        normalized.dedup();
        Ok(normalized)
    }

    fn validate_domain_membership_targets(
        program: &dyn crate::CheckingInput,
        fact_plan: &FactPlan,
        domain: &typed_trees::domain::DomainDefinition,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for membership in domain_membership_facts(fact_plan, domain.symbol) {
            let Some(referenced_domain) =
                domain_definition_by_symbol(program, membership.domain_symbol)
            else {
                continue;
            };

            let subject_type =
                crate::value_custody::expression_types::domain_expression_result_type_reference(
                    program,
                    domain,
                    membership.value,
                );
            let Some(subject_type) = subject_type else {
                diagnostics.push(Diagnostic::error(format!(
                    "domain `{}` membership in `{}` has no resolved subject type",
                    domain.name, referenced_domain.name,
                )));
                continue;
            };
            if type_references_match(program, subject_type, referenced_domain.target_type) {
                continue;
            }

            diagnostics.push(Diagnostic::error(format!(
                "domain `{}` imports `{}` but they classify different types: `{}` vs `{}`",
                domain.name,
                referenced_domain.name,
                type_reference_label(program, subject_type),
                type_reference_label(program, referenced_domain.target_type)
            )));
        }
    }

    fn validate_domain_membership_cycles(
        program: &dyn crate::CheckingInput,
        fact_plan: &FactPlan,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let mut reported = Vec::new();
        for domain in program.domain_definitions() {
            let mut path = Vec::new();
            validate_domain_membership_cycle_from(
                program,
                fact_plan,
                domain.symbol,
                &mut path,
                &mut reported,
                diagnostics,
            );
        }
    }

    fn validate_domain_membership_cycle_from(
        program: &dyn crate::CheckingInput,
        fact_plan: &FactPlan,
        domain_symbol: SymbolHandle,
        path: &mut Vec<SymbolHandle>,
        reported: &mut Vec<SymbolHandle>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if !domain_symbol.is_valid() || reported.contains(&domain_symbol) {
            return;
        }

        if let Some(cycle_start) = path.iter().position(|symbol| *symbol == domain_symbol) {
            reported.push(domain_symbol);
            let cycle = path[cycle_start..]
                .iter()
                .copied()
                .chain(std::iter::once(domain_symbol))
                .filter_map(|symbol| domain_definition_by_symbol(program, symbol))
                .map(|domain| domain.name.to_string())
                .collect::<Vec<_>>()
                .join(" -> ");
            diagnostics.push(Diagnostic::error(format!(
                "domain membership cycle: {cycle}"
            )));
            return;
        }

        let Some(domain) = domain_definition_by_symbol(program, domain_symbol) else {
            return;
        };

        path.push(domain_symbol);
        for membership in domain_membership_facts(fact_plan, domain.symbol) {
            validate_domain_membership_cycle_from(
                program,
                fact_plan,
                membership.domain_symbol,
                path,
                reported,
                diagnostics,
            );
        }
        path.pop();
    }

    fn domain_membership_facts(
        fact_plan: &FactPlan,
        symbol: SymbolHandle,
    ) -> impl Iterator<Item = facts::DomainMembershipFact> + '_ {
        fact_plan.domain_memberships_for_symbol(symbol)
    }

    fn domain_definition_by_symbol(
        program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<&typed_trees::domain::DomainDefinition> {
        if !symbol.is_valid() {
            return None;
        }

        program.domain_definition(symbol)
    }
}
