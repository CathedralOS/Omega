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
    pub(crate) mod implication {
        use super::exploration::{ValueShape, admitted_predicate_machine, value_shape};
        use crate::frozen_program::expression_table;
        use crate::proof_contracts::bound_expression_meaning::has_builtin_binary_expression_meaning;
        use crate::{
            ScopedArithmeticComparison as Comparison, ScopedArithmeticValue as Integer,
            scoped_arithmetic_comparisons_unsatisfiable,
        };
        use numerics::bignum::BigInt;
        use std::fs::OpenOptions;
        use symbols::SymbolHandle;
        use typed_trees::expression::{
            BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
        };
        use typed_trees::machine::Machine;
        use typed_trees::signature::SignatureContractKind;
        use typed_trees::state::State;
        use typed_trees::statement::{
            StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode,
        };
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        pub(crate) mod exact_conditions {
            use super::super::{
                PredicateApplication, PredicateBooleanSubject, PredicateCondition,
                PredicateIntegerOperand, PredicateOrderingConsequence,
            };
            use super::*;
            use crate::frozen_program::expression_table;

            pub(crate) fn exact_conjunction(
                program: &dyn crate::CheckingInput,
                application: &PredicateApplication<'_>,
            ) -> Option<Vec<PredicateCondition>> {
                let machine = program.machines().get(application.machine)?;
                let entry = program.machine_states(machine).first()?;
                let parameters = program.state_parameters(entry);
                if !admitted_activation_parameters(program, parameters, application.arguments.len())
                {
                    return None;
                }
                let mut proof = Implication {
                    program,
                    context: machine,
                    remaining: WORK_BUDGET,
                    next_identity: 0,
                    lemmas: Vec::new(),
                    updates: Vec::new(),
                    records: Vec::new(),
                    record_updates: Vec::new(),
                    antecedents: Vec::new(),
                };
                let mut initial = Path::default();
                let arguments = parameters
                    .iter()
                    .map(|parameter| proof.parameter(parameter.type_reference, &mut initial))
                    .collect::<Option<Vec<_>>>()?;
                let frame = Frame {
                    machine: application.machine,
                    state: 0,
                    storage: Vec::new(),
                    bindings: parameters
                        .iter()
                        .zip(&arguments)
                        .map(|(parameter, value)| (parameter.symbol, value.clone()))
                        .collect(),
                };
                let mut expressions = Vec::new();
                for statement in program.statement_table.statements(entry.statement_nodes) {
                    proof.spend()?;
                    crate::queries::expression_traversal::collect_statement_expression_trees(
                        program,
                        statement,
                        &mut expressions,
                    );
                }
                let mut candidates = Vec::new();
                let mut seen = Vec::new();
                let mut seen_booleans = Vec::new();
                for expression in &expressions {
                    proof.spend()?;
                    if let Some(subject) = super::super::predicate_boolean_subject(
                        program,
                        machine.symbol,
                        application.arguments,
                        *expression,
                    ) {
                        let identity = crate::immutable_entry_boolean_projection(
                            program,
                            machine.symbol,
                            *expression,
                        )?;
                        if !seen_booleans.contains(&identity) {
                            seen_booleans.push(identity);
                            for holds in [true, false] {
                                candidates.push(Candidate::Boolean {
                                    expression: *expression,
                                    subject: subject.clone(),
                                    holds,
                                });
                            }
                        }
                    }
                    let Some(binary) = expression_table(program).binary_node(*expression) else {
                        continue;
                    };
                    if !matches!(
                        binary.operator,
                        BinaryOperator::Equal
                            | BinaryOperator::NotEqual
                            | BinaryOperator::Less
                            | BinaryOperator::LessOrEqual
                            | BinaryOperator::Greater
                            | BinaryOperator::GreaterOrEqual
                    ) || !has_builtin_binary_expression_meaning(
                        program,
                        machine,
                        Some(entry),
                        *expression,
                    ) || seen.contains(expression)
                    {
                        continue;
                    }
                    let Some(left) = super::super::predicate_integer_operand(
                        program,
                        machine.symbol,
                        application.arguments,
                        binary.left,
                    ) else {
                        continue;
                    };
                    let Some(right) = super::super::predicate_integer_operand(
                        program,
                        machine.symbol,
                        application.arguments,
                        binary.right,
                    ) else {
                        continue;
                    };
                    seen.push(*expression);
                    candidates.push(Candidate::Ordering {
                        left: binary.left,
                        right: binary.right,
                        condition: PredicateOrderingConsequence {
                            left,
                            right,
                            operator: binary.operator,
                        },
                    });
                }
                candidates.sort_by_key(Candidate::has_reads);
                let mut selected = Vec::new();
                let mut paths = vec![initial.clone()];
                for candidate in candidates {
                    if necessary(
                        &mut proof,
                        application,
                        &frame,
                        &arguments,
                        &initial,
                        &candidate,
                    )? {
                        paths = proof.form_condition(&frame, &paths, &candidate)?;
                        selected.push(candidate.into_condition());
                    }
                }
                if sufficient(&mut proof, application, &arguments, &paths)? {
                    return (!selected.is_empty()).then_some(selected);
                }

                let mut subjects = Vec::new();
                for expression in expressions {
                    proof.spend()?;
                    let Some(reference) = crate::expression_result_type_reference(
                        program, machine, entry, expression,
                    ) else {
                        continue;
                    };
                    let Some(carrier) = program
                        .primitive_type_reference(reference)
                        .filter(|carrier| carrier.is_fixed_integer())
                    else {
                        continue;
                    };
                    if program.arithmetic_domain_for_type_reference(reference)
                        != numerics::arithmetic::ArithmeticDomain::Exact
                    {
                        continue;
                    }
                    let Some(operand) = super::super::predicate_integer_operand(
                        program,
                        machine.symbol,
                        application.arguments,
                        expression,
                    ) else {
                        continue;
                    };
                    let identity = match &operand {
                        PredicateIntegerOperand::Argument(_) => {
                            let (position, projection) = crate::immutable_entry_integer_projection(
                                program,
                                machine.symbol,
                                expression,
                            )?;
                            SubjectIdentity::Parameter(position, projection)
                        }
                        PredicateIntegerOperand::Read(_) => SubjectIdentity::Read(
                            crate::immutable_entry_indexed_read_identity(program, expression)?,
                        ),
                        PredicateIntegerOperand::Integer(_) => continue,
                    };
                    if subjects
                        .iter()
                        .any(|previous: &Subject| previous.identity == identity)
                    {
                        continue;
                    }
                    subjects.push(Subject {
                        identity,
                        expression,
                        operand,
                        carrier,
                    });
                }
                subjects
                    .sort_by_key(|subject| !matches!(subject.identity, SubjectIdentity::Read(_)));
                for left in 0..subjects.len() {
                    for right in (left + 1)..subjects.len() {
                        if subjects[left].carrier != subjects[right].carrier {
                            continue;
                        }
                        for operator in [
                            BinaryOperator::NotEqual,
                            BinaryOperator::Equal,
                            BinaryOperator::Less,
                            BinaryOperator::LessOrEqual,
                            BinaryOperator::Greater,
                            BinaryOperator::GreaterOrEqual,
                        ] {
                            proof.spend()?;
                            let candidate = Candidate::Ordering {
                                left: subjects[left].expression,
                                right: subjects[right].expression,
                                condition: PredicateOrderingConsequence {
                                    left: subjects[left].operand.clone(),
                                    right: subjects[right].operand.clone(),
                                    operator,
                                },
                            };
                            if !necessary(
                                &mut proof,
                                application,
                                &frame,
                                &arguments,
                                &initial,
                                &candidate,
                            )? {
                                continue;
                            }
                            paths = proof.form_condition(&frame, &paths, &candidate)?;
                            selected.push(candidate.into_condition());
                            if sufficient(&mut proof, application, &arguments, &paths)? {
                                return Some(selected);
                            }
                        }
                    }
                }
                None
            }

            pub(crate) enum Candidate {
                Ordering {
                    left: ExpressionHandle,
                    right: ExpressionHandle,
                    condition: PredicateOrderingConsequence,
                },
                Boolean {
                    expression: ExpressionHandle,
                    subject: PredicateBooleanSubject,
                    holds: bool,
                },
            }
            impl Candidate {
                fn has_reads(&self) -> bool {
                    match self {
                        Self::Ordering { condition, .. } => {
                            matches!(condition.left, PredicateIntegerOperand::Read(_))
                                || matches!(condition.right, PredicateIntegerOperand::Read(_))
                        }
                        Self::Boolean { .. } => false,
                    }
                }
                fn into_condition(self) -> PredicateCondition {
                    match self {
                        Self::Ordering { condition, .. } => PredicateCondition::Ordering(condition),
                        Self::Boolean { subject, holds, .. } => {
                            PredicateCondition::Boolean { subject, holds }
                        }
                    }
                }
            }
            #[derive(PartialEq, Eq)]
            pub(crate) enum SubjectIdentity {
                Parameter(usize, Vec<facts::PlaceSegment>),
                Read(crate::ImmutableEntryReadIdentity),
            }
            pub(crate) struct Subject {
                identity: SubjectIdentity,
                expression: ExpressionHandle,
                operand: PredicateIntegerOperand,
                carrier: typed_trees::types::PrimitiveType,
            }

            fn necessary(
                proof: &mut Implication<'_>,
                application: &PredicateApplication<'_>,
                frame: &Frame,
                arguments: &[Value],
                initial: &Path,
                candidate: &Candidate,
            ) -> Option<bool> {
                for (path, goal) in proof.condition(frame, initial.clone(), candidate, true)? {
                    if proof.generalized_activation(
                        application.machine,
                        0,
                        arguments.to_vec(),
                        path,
                        &goal,
                        Judgment::Consequence,
                        0,
                    ) != Some(true)
                    {
                        return Some(false);
                    }
                }
                Some(true)
            }

            fn sufficient(
                proof: &mut Implication<'_>,
                application: &PredicateApplication<'_>,
                arguments: &[Value],
                paths: &[Path],
            ) -> Option<bool> {
                for path in paths {
                    proof.spend()?;
                    if proof.generalized_activation(
                        application.machine,
                        0,
                        arguments.to_vec(),
                        path.clone(),
                        &Boolean::Constant(true),
                        Judgment::RequiredResult,
                        0,
                    ) != Some(true)
                    {
                        return Some(false);
                    }
                }
                Some(true)
            }

            impl Implication<'_> {
                fn inert_integer(
                    &mut self,
                    frame: &Frame,
                    path: Path,
                    expression: ExpressionHandle,
                ) -> Option<Vec<(Path, Integer)>> {
                    if expression_table(self.program)
                        .indexed_node(expression)
                        .is_some()
                    {
                        return self.inert_read(frame, path, expression);
                    }
                    self.expression(frame, path, expression, 0)?
                        .into_iter()
                        .map(|(path, value)| {
                            let Value::Integer { value, .. } = value else {
                                return None;
                            };
                            Some((path, value))
                        })
                        .collect()
                }
                fn condition(
                    &mut self,
                    frame: &Frame,
                    path: Path,
                    candidate: &Candidate,
                    inert: bool,
                ) -> Option<Vec<(Path, Boolean)>> {
                    if let Candidate::Boolean {
                        expression, holds, ..
                    } = candidate
                    {
                        return self
                            .expression(frame, path, *expression, 0)?
                            .into_iter()
                            .map(|(path, value)| {
                                let Value::Boolean(value) = value else {
                                    return None;
                                };
                                Some((
                                    path,
                                    if *holds {
                                        value
                                    } else {
                                        Boolean::Not(Box::new(value))
                                    },
                                ))
                            })
                            .collect();
                    }
                    let Candidate::Ordering {
                        left,
                        right,
                        condition,
                    } = candidate
                    else {
                        return None;
                    };
                    let mut rows = Vec::new();
                    for (path, left) in self.condition_integer(frame, path, *left, inert)? {
                        for (path, right) in self.condition_integer(frame, path, *right, inert)? {
                            rows.push((
                                path,
                                Boolean::Comparison(compare(
                                    condition.operator,
                                    left.clone(),
                                    right,
                                )),
                            ));
                        }
                    }
                    (!rows.is_empty()).then_some(rows)
                }

                fn condition_integer(
                    &mut self,
                    frame: &Frame,
                    path: Path,
                    expression: ExpressionHandle,
                    inert: bool,
                ) -> Option<Vec<(Path, Integer)>> {
                    if inert {
                        return self.inert_integer(frame, path, expression);
                    }
                    self.expression(frame, path, expression, 0)?
                        .into_iter()
                        .map(|(path, value)| {
                            let Value::Integer { value, .. } = value else {
                                return None;
                            };
                            Some((path, value))
                        })
                        .collect()
                }

                fn form_condition(
                    &mut self,
                    frame: &Frame,
                    paths: &[Path],
                    candidate: &Candidate,
                ) -> Option<Vec<Path>> {
                    let mut next = Vec::new();
                    for path in paths {
                        for (path, value) in
                            self.condition(frame, path.clone(), candidate, false)?
                        {
                            if let Some(path) = self.assume(path, &value, true)? {
                                next.push(path);
                            }
                        }
                    }
                    Some(next)
                }
            }
        }
        pub(crate) mod read_subjects {
            use super::{Frame, Implication, Integer, Path, Value};
            use crate::frozen_program::expression_table;
            use crate::proof_contracts::collection_predicates::{
                PredicateApplication, PredicateIntegerOperand, PredicateIntegerReadSubject,
            };
            use typed_trees::expression::{ExpressionHandle, ExpressionNode};

            pub(crate) type Subjects = Vec<(PredicateIntegerOperand, Integer)>;

            impl Implication<'_> {
                pub(crate) fn runtime_read_subjects(
                    &mut self,
                    application: &PredicateApplication<'_>,
                    frame: &Frame,
                    path: Path,
                    subjects: Subjects,
                ) -> Option<Vec<(Path, Subjects)>> {
                    let program = self.program;
                    let machine = &program.machines()[application.machine];
                    let entry = self.program.machine_states(machine).first()?;
                    let mut expressions = Vec::new();
                    for statement in self
                        .program
                        .statement_table
                        .statements(entry.statement_nodes)
                    {
                        self.spend()?;
                        crate::queries::expression_traversal::collect_statement_expression_trees(
                            self.program,
                            statement,
                            &mut expressions,
                        );
                    }
                    let mut reads = Vec::new();
                    for expression in expressions {
                        self.spend()?;
                        let Some(indexed) = expression_table(self.program).indexed_node(expression)
                        else {
                            continue;
                        };
                        if expression_table(self.program)
                            .constant_integer_value(indexed.index)
                            .is_some()
                        {
                            continue;
                        }
                        let Some(identity) =
                            crate::immutable_entry_indexed_read_identity(self.program, expression)
                        else {
                            continue;
                        };
                        if !reads.iter().any(|(_, previous)| *previous == identity) {
                            reads.push((expression, identity));
                        }
                    }
                    let mut candidates = vec![(path, subjects)];
                    for (position, (expression, _)) in reads.into_iter().enumerate() {
                        let mut next = Vec::new();
                        for (path, subjects) in candidates {
                            for (path, value) in self.inert_read(frame, path, expression)? {
                                self.spend()?;
                                let mut subjects = subjects.clone();
                                subjects.insert(
                                    position,
                                    (
                                        PredicateIntegerOperand::Read(
                                            PredicateIntegerReadSubject {
                                                predicate: machine.symbol,
                                                expression,
                                                arguments: application.arguments.to_vec(),
                                            },
                                        ),
                                        value,
                                    ),
                                );
                                next.push((path, subjects));
                            }
                        }
                        candidates = next;
                    }
                    Some(candidates)
                }

                pub(crate) fn inert_read(
                    &mut self,
                    frame: &Frame,
                    path: Path,
                    expression: ExpressionHandle,
                ) -> Option<Vec<(Path, Integer)>> {
                    let ExpressionNode::Indexed(indexed) =
                        expression_table(self.program).expression(expression)
                    else {
                        return None;
                    };
                    let mut values = Vec::new();
                    for (path, collection) in self.expression(frame, path, indexed.collection, 0)? {
                        let Value::Collection {
                            identity, element, ..
                        } = collection
                        else {
                            return None;
                        };
                        for (path, selector) in self.expression(frame, path, indexed.index, 0)? {
                            let Value::Integer {
                                value: selector, ..
                            } = selector
                            else {
                                return None;
                            };
                            for (path, value) in self.read(path, identity, selector, element, 0)? {
                                let Value::Integer { value, .. } = value else {
                                    return None;
                                };
                                values.push((path, value));
                            }
                        }
                    }
                    Some(values)
                }
            }
        }
        pub(crate) use exact_conditions::exact_conjunction;
        pub(crate) mod record_observations {
            use super::*;

            pub(crate) struct Observation {
                identity: usize,
                projection: Vec<facts::PlaceSegment>,
                value: Value,
            }

            pub(crate) struct Update {
                identity: usize,
                previous: Value,
                field: SymbolHandle,
                value: Value,
            }

            impl Implication<'_> {
                pub(crate) fn record_store(
                    &mut self,
                    previous: Value,
                    field: SymbolHandle,
                    value: Value,
                ) -> Option<Value> {
                    self.spend()?;
                    let Value::Record { reference, .. } = previous else {
                        return None;
                    };
                    let identity = self.next_identity;
                    self.next_identity += 1;
                    self.record_updates.push(Update {
                        identity,
                        previous,
                        field,
                        value,
                    });
                    Some(Value::Record {
                        identity,
                        reference,
                        projection: Vec::new(),
                    })
                }

                pub(crate) fn record_observes_integer(
                    &mut self,
                    identity: usize,
                    projection: &[facts::PlaceSegment],
                    value: &Integer,
                    path: &Path,
                ) -> Option<bool> {
                    for position in 0..self.records.len() {
                        self.spend()?;
                        let observation = &self.records[position];
                        if observation.identity != identity
                            || !observation.projection.starts_with(projection)
                        {
                            continue;
                        }
                        match observation.value.clone() {
                            Value::Integer { value: formal, .. } if contains(value, &formal) => {
                                return Some(true);
                            }
                            Value::Collection { identity, .. }
                                if self.collection_observes_integer(identity, value, path)? =>
                            {
                                return Some(true);
                            }
                            _ => {}
                        }
                    }
                    Some(false)
                }

                pub(crate) fn record_member(
                    &mut self,
                    mut path: Path,
                    receiver: Value,
                    symbol: SymbolHandle,
                ) -> Option<(Path, Value)> {
                    self.spend()?;
                    let Value::Record {
                        identity,
                        reference,
                        mut projection,
                    } = receiver
                    else {
                        return None;
                    };
                    let definition = crate::value_custody::places::data_definition_for_type(
                        self.program,
                        reference,
                    )?;
                    if !self.program.data_members(definition).iter().any(|member| {
            matches!(member,
            typed_trees::data::DataMember::Field(field) if field.symbol == symbol && field.relevance == language_core::BindingRelevance::Relevant)
        }) {
            return None;
        }
                    let segment = facts::PlaceSegment::Field { symbol };
                    let field_type = crate::project_type_reference_from_segments(
                        self.program,
                        reference,
                        std::slice::from_ref(&segment),
                    )?;
                    if crate::reference_shell(self.program, field_type).is_some() {
                        return None;
                    }
                    if let Some(update) = self
                        .record_updates
                        .iter()
                        .find(|update| update.identity == identity)
                    {
                        if !projection.is_empty() {
                            return None;
                        }
                        if update.field == symbol {
                            return Some((path, update.value.clone()));
                        }
                        let previous = update.previous.clone();
                        return self.record_member(path, previous, symbol);
                    }
                    projection.push(segment);
                    if let Some(value) = self
                        .records
                        .iter()
                        .find(|observation| {
                            observation.identity == identity && observation.projection == projection
                        })
                        .map(|observation| observation.value.clone())
                    {
                        if let Value::Integer {
                            value,
                            carrier: Some(carrier),
                        } = &value
                        {
                            let (low, high) = self
                                .program
                                .primitive_type_reference(*carrier)?
                                .integer_bounds()?;
                            path.comparisons.push(compare(
                                BinaryOperator::GreaterOrEqual,
                                value.clone(),
                                number(low),
                            ));
                            path.comparisons.push(compare(
                                BinaryOperator::LessOrEqual,
                                value.clone(),
                                number(high),
                            ));
                        }
                        if let Value::Boolean(Boolean::Variable(value)) = &value {
                            boolean_bounds(&mut path, value);
                        }
                        return Some((path, value));
                    }
                    let value = match value_shape(self.program, field_type)? {
                        ValueShape::Record(reference) => Value::Record {
                            identity,
                            reference,
                            projection: projection.clone(),
                        },
                        ValueShape::Integer(carrier) => self.integer(carrier, &mut path)?,
                        ValueShape::Collection { .. } => self.parameter(field_type, &mut path)?,
                        ValueShape::Boolean => self.boolean(&mut path)?,
                    };
                    self.records.push(Observation {
                        identity,
                        projection,
                        value: value.clone(),
                    });
                    Some((path, value))
                }

                pub(crate) fn integer_subjects(
                    &mut self,
                    expression: ExpressionHandle,
                    reference: TypeReferenceHandle,
                    value: Value,
                    path: &mut Path,
                    subjects: &mut Vec<(super::super::PredicateIntegerSubject, Integer)>,
                    depth: usize,
                ) -> Option<()> {
                    self.spend()?;
                    if depth >= 32 {
                        return None;
                    }
                    match &value {
                        Value::Integer { value, .. } => subjects.push((
                            super::super::PredicateIntegerSubject {
                                expression,
                                projection: Vec::new(),
                            },
                            value.clone(),
                        )),
                        Value::Record { reference, .. } => {
                            let definition =
                                crate::value_custody::places::data_definition_for_type(
                                    self.program,
                                    *reference,
                                )?;
                            let fields = self
                                .program
                                .data_members(definition)
                                .iter()
                                .filter_map(|member| match member {
                                    typed_trees::data::DataMember::Field(field)
                                        if field.relevance
                                            == language_core::BindingRelevance::Relevant =>
                                    {
                                        Some(field.symbol)
                                    }
                                    _ => None,
                                })
                                .collect::<Vec<_>>();
                            for symbol in fields {
                                let field_type = crate::project_type_reference_from_segments(
                                    self.program,
                                    *reference,
                                    &[facts::PlaceSegment::Field { symbol }],
                                )?;
                                if crate::reference_shell(self.program, field_type).is_some()
                                    || !matches!(
                                        value_shape(self.program, field_type),
                                        Some(
                                            ValueShape::Integer(_)
                                                | ValueShape::Record(_)
                                                | ValueShape::Collection { .. }
                                        )
                                    )
                                {
                                    continue;
                                }
                                let (next, field) =
                                    self.record_member(path.clone(), value.clone(), symbol)?;
                                *path = next;
                                let before = subjects.len();
                                self.integer_subjects(
                                    expression,
                                    field_type,
                                    field,
                                    path,
                                    subjects,
                                    depth + 1,
                                )?;
                                for (subject, _) in &mut subjects[before..] {
                                    subject
                                        .projection
                                        .insert(0, facts::PlaceSegment::Field { symbol });
                                }
                            }
                        }
                        Value::Collection {
                            identity,
                            length,
                            element,
                        } => {
                            let Some(ValueShape::Collection {
                                length: Some(extent),
                                element_type,
                            }) = value_shape(self.program, reference)
                            else {
                                return Some(());
                            };
                            if element_type != *element {
                                return None;
                            }
                            for index in 0..extent {
                                self.spend()?;
                                let selector = number(i128::try_from(index).ok()?);
                                if !self.implies(
                                    path,
                                    &compare(
                                        BinaryOperator::GreaterOrEqual,
                                        selector.clone(),
                                        number(0),
                                    ),
                                ) || !self.implies(
                                    path,
                                    &compare(
                                        BinaryOperator::Less,
                                        selector.clone(),
                                        length.clone(),
                                    ),
                                ) {
                                    return None;
                                }
                                let mut outcomes =
                                    self.read(path.clone(), *identity, selector, *element, 0)?;
                                if outcomes.len() != 1 {
                                    return None;
                                }
                                let (next, value) = outcomes.pop()?;
                                *path = next;
                                let before = subjects.len();
                                self.integer_subjects(
                                    expression,
                                    element_type,
                                    value,
                                    path,
                                    subjects,
                                    depth + 1,
                                )?;
                                for (subject, _) in &mut subjects[before..] {
                                    subject
                                        .projection
                                        .insert(0, facts::PlaceSegment::FixedIndex { index });
                                }
                            }
                        }
                        _ => {}
                    }
                    Some(())
                }
            }
        }
        pub(crate) mod storage {
            use super::*;
            use crate::frozen_program::expression_table;

            #[derive(Clone)]
            pub(crate) struct Home {
                identity: usize,
                value: Value,
            }

            #[derive(Clone)]
            pub(crate) struct Binding {
                pub(crate) symbol: SymbolHandle,
                pub(crate) place: Place,
            }

            #[derive(Clone)]
            pub(crate) struct Place {
                home: usize,
                projection: Vec<Projection>,
                reference: TypeReferenceHandle,
            }

            #[derive(Clone)]
            pub(crate) enum Projection {
                Field(SymbolHandle),
                Element(Integer),
            }

            #[derive(Clone)]
            pub(crate) struct Argument {
                value: Value,
                reference: TypeReferenceHandle,
                backing: Option<Place>,
            }

            impl Implication<'_> {
                pub(crate) fn seed_body_storage(
                    &mut self,
                    frame: &mut Frame,
                    path: &mut Path,
                    parameters: &[typed_trees::signature::StateParameter],
                ) -> Option<()> {
                    for parameter in parameters {
                        self.spend()?;
                        let value = frame
                            .bindings
                            .iter()
                            .find(|(symbol, _)| *symbol == parameter.symbol)?
                            .1
                            .clone();
                        let place = self.storage_home(path, value, parameter.type_reference);
                        frame.storage.push(Binding {
                            symbol: parameter.symbol,
                            place,
                        });
                    }
                    Some(())
                }

                fn storage_home(
                    &mut self,
                    path: &mut Path,
                    value: Value,
                    reference: TypeReferenceHandle,
                ) -> Place {
                    let identity = self.next_identity;
                    self.next_identity += 1;
                    path.storage.push(Home { identity, value });
                    Place {
                        home: identity,
                        projection: Vec::new(),
                        reference,
                    }
                }

                pub(crate) fn bind_body_storage(
                    &mut self,
                    frame: &mut Frame,
                    path: &mut Path,
                    symbol: SymbolHandle,
                    argument: Argument,
                ) -> Option<()> {
                    self.spend()?;
                    let place = match argument.backing {
                        Some(mut place) => {
                            place.reference = argument.reference;
                            place
                        }
                        None => self.storage_home(path, argument.value, argument.reference),
                    };
                    frame.storage.push(Binding { symbol, place });
                    Some(())
                }

                pub(crate) fn body_local(
                    &mut self,
                    frame: &Frame,
                    path: Path,
                    local: &typed_trees::statement::TableLocalData,
                ) -> Option<Vec<(Frame, Path)>> {
                    self.body_argument(frame, path, local.initial_value, local.type_reference)?
                        .into_iter()
                        .map(|(mut path, argument)| {
                            let mut frame = frame.clone();
                            self.bind_body_storage(&mut frame, &mut path, local.symbol, argument)?;
                            Some((frame, path))
                        })
                        .collect()
                }

                pub(crate) fn body_arguments(
                    &mut self,
                    frame: &Frame,
                    path: Path,
                    expressions: &[ExpressionHandle],
                    parameters: &[typed_trees::signature::StateParameter],
                ) -> Option<Vec<(Path, Vec<Argument>)>> {
                    let mut rows = vec![(path, Vec::new())];
                    for (expression, parameter) in expressions.iter().zip(parameters) {
                        let mut next = Vec::new();
                        for (path, arguments) in rows {
                            for (path, argument) in self.body_argument(
                                frame,
                                path,
                                *expression,
                                parameter.type_reference,
                            )? {
                                let mut arguments = arguments.clone();
                                arguments.push(argument);
                                next.push((path, arguments));
                            }
                        }
                        rows = next;
                    }
                    Some(rows)
                }

                fn body_argument(
                    &mut self,
                    frame: &Frame,
                    path: Path,
                    expression: ExpressionHandle,
                    reference: TypeReferenceHandle,
                ) -> Option<Vec<(Path, Argument)>> {
                    if crate::reference_shell(self.program, reference).is_some() {
                        let mut result = Vec::new();
                        for (path, place) in self.storage_place(frame, path, expression, 0)? {
                            for (path, value) in self.storage_value(path, &place)? {
                                result.push((
                                    path,
                                    Argument {
                                        value: declared_value(self.program, value, reference)?,
                                        reference,
                                        backing: Some(place.clone()),
                                    },
                                ));
                            }
                        }
                        Some(result)
                    } else {
                        self.expression(frame, path, expression, 0)?
                            .into_iter()
                            .map(|(path, value)| {
                                Some((
                                    path,
                                    Argument {
                                        value: declared_value(self.program, value, reference)?,
                                        reference,
                                        backing: None,
                                    },
                                ))
                            })
                            .collect()
                    }
                }

                fn storage_place(
                    &mut self,
                    frame: &Frame,
                    path: Path,
                    expression: ExpressionHandle,
                    depth: usize,
                ) -> Option<Vec<(Path, Place)>> {
                    self.spend()?;
                    if depth >= 64 {
                        return None;
                    }
                    let machine = &self.program.machines()[frame.machine];
                    let state = &self.program.machine_states(machine)[frame.state];
                    if !crate::value_custody::places::place_has_builtin_coordinates(
                        self.program,
                        machine,
                        Some(state),
                        expression,
                    ) {
                        return None;
                    }
                    match expression_table(self.program).expression(expression) {
                        ExpressionNode::Name(name)
                            if name.symbol == name.head_symbol
                                && expression_table(self.program)
                                    .name_path_members(name.members)
                                    .len()
                                    == 1 =>
                        {
                            let binding = frame
                                .storage
                                .iter()
                                .rev()
                                .find(|binding| binding.symbol == name.symbol)?;
                            Some(vec![(path, binding.place.clone())])
                        }
                        ExpressionNode::Borrow(borrow) if borrow.access.is_readable() => {
                            self.storage_place(frame, path, borrow.target, depth + 1)
                        }
                        ExpressionNode::Member(member) => {
                            let symbol = crate::value_custody::places::effective_member_symbol(
                                self.program,
                                member.receiver,
                                member,
                            );
                            let mut result = Vec::new();
                            for (path, place) in
                                self.storage_place(frame, path, member.receiver, depth + 1)?
                            {
                                for (path, receiver) in self.storage_value(path, &place)? {
                                    let Value::Record { reference, .. } = receiver else {
                                        return None;
                                    };
                                    let (path, _) = self.record_member(path, receiver, symbol)?;
                                    let reference = crate::project_type_reference_from_segments(
                                        self.program,
                                        reference,
                                        &[facts::PlaceSegment::Field { symbol }],
                                    )?;
                                    let mut place = place.clone();
                                    place.projection.push(Projection::Field(symbol));
                                    place.reference = reference;
                                    result.push((path, place));
                                }
                            }
                            Some(result)
                        }
                        ExpressionNode::Indexed(indexed) => {
                            let mut result = Vec::new();
                            for (path, place) in
                                self.storage_place(frame, path, indexed.collection, depth + 1)?
                            {
                                for (path, collection) in self.storage_value(path, &place)? {
                                    let Value::Collection {
                                        length, element, ..
                                    } = collection
                                    else {
                                        return None;
                                    };
                                    for (path, selector) in
                                        self.expression(frame, path, indexed.index, depth + 1)?
                                    {
                                        let Value::Integer {
                                            value: selector, ..
                                        } = selector
                                        else {
                                            return None;
                                        };
                                        self.storage_index(&path, &selector, &length)?;
                                        let mut place = place.clone();
                                        place.projection.push(Projection::Element(selector));
                                        place.reference = element;
                                        result.push((path, place));
                                    }
                                }
                            }
                            Some(result)
                        }
                        _ => None,
                    }
                }

                fn storage_index(
                    &self,
                    path: &Path,
                    selector: &Integer,
                    length: &Integer,
                ) -> Option<()> {
                    (self.implies(
                        path,
                        &compare(BinaryOperator::GreaterOrEqual, selector.clone(), number(0)),
                    ) && self.implies(
                        path,
                        &compare(BinaryOperator::Less, selector.clone(), length.clone()),
                    ))
                    .then_some(())
                }

                pub(crate) fn storage_value(
                    &mut self,
                    path: Path,
                    place: &Place,
                ) -> Option<Vec<(Path, Value)>> {
                    let value = path
                        .storage
                        .iter()
                        .find(|home| home.identity == place.home)?
                        .value
                        .clone();
                    let mut rows = vec![(path, value)];
                    for projection in &place.projection {
                        let mut next = Vec::new();
                        for (path, value) in rows {
                            match projection {
                                Projection::Field(symbol) => {
                                    next.push(self.record_member(path, value, *symbol)?)
                                }
                                Projection::Element(selector) => {
                                    let Value::Collection {
                                        identity,
                                        length,
                                        element,
                                    } = value
                                    else {
                                        return None;
                                    };
                                    self.storage_index(&path, selector, &length)?;
                                    next.extend(self.read(
                                        path,
                                        identity,
                                        selector.clone(),
                                        element,
                                        0,
                                    )?);
                                }
                            }
                        }
                        rows = next;
                    }
                    rows.into_iter()
                        .map(|(path, value)| {
                            Some((path, declared_value(self.program, value, place.reference)?))
                        })
                        .collect()
                }

                pub(crate) fn assignment(
                    &mut self,
                    frame: &Frame,
                    path: Path,
                    target: ExpressionHandle,
                    source: ExpressionHandle,
                ) -> Option<Vec<(Frame, Path)>> {
                    let mut result = Vec::new();
                    for (path, place) in self.storage_place(frame, path, target, 0)? {
                        if !crate::has_plain_contents_without_nominal_cleanup(
                            self.program,
                            place.reference,
                        ) || !crate::has_contents(
                            self.program,
                            place.reference,
                            &[],
                            crate::ContentsRequirement::StableObservation,
                        ) {
                            return None;
                        }
                        for (path, value) in self.expression(frame, path, source, 0)? {
                            let replacement = declared_value(self.program, value, place.reference)?;
                            let current = path
                                .storage
                                .iter()
                                .find(|home| home.identity == place.home)?
                                .value
                                .clone();
                            for (mut path, value) in self.replace_storage(
                                path,
                                current,
                                &place.projection,
                                &replacement,
                            )? {
                                path.storage
                                    .iter_mut()
                                    .find(|home| home.identity == place.home)?
                                    .value = value;
                                result.push((frame.clone(), path));
                            }
                        }
                    }
                    Some(result)
                }

                fn replace_storage(
                    &mut self,
                    path: Path,
                    current: Value,
                    projection: &[Projection],
                    replacement: &Value,
                ) -> Option<Vec<(Path, Value)>> {
                    self.spend()?;
                    let Some((first, rest)) = projection.split_first() else {
                        return Some(vec![(path, replacement.clone())]);
                    };
                    let mut result = Vec::new();
                    match first {
                        Projection::Field(symbol) => {
                            let (path, child) =
                                self.record_member(path, current.clone(), *symbol)?;
                            for (path, value) in
                                self.replace_storage(path, child, rest, replacement)?
                            {
                                result.push((
                                    path,
                                    self.record_store(current.clone(), *symbol, value)?,
                                ));
                            }
                        }
                        Projection::Element(selector) => {
                            let Value::Collection {
                                identity: previous,
                                length,
                                element,
                            } = current
                            else {
                                return None;
                            };
                            self.storage_index(&path, selector, &length)?;
                            let rows = if rest.is_empty() {
                                vec![(path, replacement.clone())]
                            } else {
                                let mut rows = Vec::new();
                                for (path, child) in
                                    self.read(path, previous, selector.clone(), element, 0)?
                                {
                                    rows.extend(self.replace_storage(
                                        path,
                                        child,
                                        rest,
                                        replacement,
                                    )?);
                                }
                                rows
                            };
                            for (path, value) in rows {
                                let identity = self.next_identity;
                                self.next_identity += 1;
                                self.updates.push(CollectionUpdate {
                                    identity,
                                    previous,
                                    selector: selector.clone(),
                                    value,
                                });
                                result.push((
                                    path,
                                    Value::Collection {
                                        identity,
                                        length: length.clone(),
                                        element,
                                    },
                                ));
                            }
                        }
                    }
                    Some(result)
                }
            }
        }

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub(crate) enum Value {
            Record {
                identity: usize,
                reference: TypeReferenceHandle,
                projection: Vec<facts::PlaceSegment>,
            },
            Integer {
                value: Integer,
                carrier: Option<TypeReferenceHandle>,
            },
            Collection {
                identity: usize,
                length: Integer,
                element: TypeReferenceHandle,
            },
            Boolean(Boolean),
        }

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub(crate) enum Boolean {
            Constant(bool),
            Variable(Integer),
            Comparison(Comparison),
            And(Box<Self>, Box<Self>),
            Or(Box<Self>, Box<Self>),
            Not(Box<Self>),
            Call {
                machine: usize,
                arguments: Vec<Value>,
            },
        }

        #[derive(Clone)]
        pub(crate) struct Observation {
            collection: usize,
            selector: Integer,
            value: Value,
        }

        pub(crate) type Clause = Vec<Comparison>;

        #[derive(Clone, Default)]
        pub(crate) struct Path {
            comparisons: Vec<Comparison>,
            clauses: Vec<Clause>,
            observations: Vec<Observation>,
            storage: Vec<storage::Home>,
        }

        #[derive(Clone)]
        pub(crate) struct Frame {
            machine: usize,
            state: usize,
            bindings: Vec<(SymbolHandle, Value)>,
            storage: Vec<storage::Binding>,
        }

        pub(crate) struct Lemma {
            machine: usize,
            state: usize,
            arguments: Vec<Value>,
            premises: Vec<Comparison>,
            premise_clauses: Vec<Clause>,
            measures: Vec<(usize, BinaryOperator)>,
            judgment: Judgment,
            goal: Boolean,
        }

        #[derive(Clone, Copy, PartialEq, Eq)]
        pub(crate) enum Judgment {
            Consequence,
            RequiredResult,
        }

        pub(crate) struct CollectionUpdate {
            identity: usize,
            previous: usize,
            selector: Integer,
            value: Value,
        }

        pub(crate) struct Implication<'program> {
            program: &'program dyn crate::CheckingInput,
            context: &'program Machine,
            remaining: usize,
            next_identity: usize,
            lemmas: Vec<Lemma>,
            updates: Vec<CollectionUpdate>,
            records: Vec<record_observations::Observation>,
            record_updates: Vec<record_observations::Update>,
            antecedents: Vec<Boolean>,
        }

        pub(crate) fn proves(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            goal: ExpressionHandle,
        ) -> bool {
            let Some(entry) = program.machine_states(machine).first() else {
                return false;
            };
            let Some(machine_position) = program
                .machines()
                .iter()
                .position(|candidate| candidate.symbol == machine.symbol)
            else {
                return false;
            };
            let mut proof = Implication {
                program,
                context: machine,
                remaining: WORK_BUDGET,
                next_identity: 0,
                lemmas: Vec::new(),
                updates: Vec::new(),
                records: Vec::new(),
                record_updates: Vec::new(),
                antecedents: Vec::new(),
            };
            let started = std::time::Instant::now();
            let proved = proof.theorem(entry, machine_position, goal);
            if let Some(trace) = std::env::var_os("OMEGA_COLLECTION_PROOF_TRACE") {
                use std::io::Write as _;
                if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(trace) {
                    let _ = writeln!(
                        file,
                        "theorem={} goal={} elapsed_micros={} spent={} proved={proved}",
                        machine.name,
                        expression_table(program).display_name(goal),
                        started.elapsed().as_micros(),
                        WORK_BUDGET - proof.remaining,
                    );
                }
            }
            proved
        }

        pub(crate) fn ordering_consequences(
            program: &dyn crate::CheckingInput,
            application: &super::PredicateApplication<'_>,
        ) -> Vec<super::PredicateOrderingConsequence> {
            ordering_relations(program, application, RelationMeaning::Necessary)
        }

        pub(crate) fn exact_ordering(
            program: &dyn crate::CheckingInput,
            application: &super::PredicateApplication<'_>,
        ) -> Option<super::PredicateOrderingConsequence> {
            ordering_relations(program, application, RelationMeaning::Equivalent)
                .into_iter()
                .next()
        }

        #[derive(Clone, Copy, PartialEq, Eq)]
        pub(crate) enum RelationMeaning {
            Necessary,
            Equivalent,
        }

        fn ordering_relations(
            program: &dyn crate::CheckingInput,
            application: &super::PredicateApplication<'_>,
            meaning: RelationMeaning,
        ) -> Vec<super::PredicateOrderingConsequence> {
            let machine = &program.machines()[application.machine];
            let Some(entry) = program.machine_states(machine).first() else {
                return Vec::new();
            };
            let parameters = program.state_parameters(entry);
            if parameters.len() != application.arguments.len() {
                return Vec::new();
            }
            let mut proof = Implication {
                program,
                context: machine,
                remaining: WORK_BUDGET,
                next_identity: 0,
                lemmas: Vec::new(),
                updates: Vec::new(),
                records: Vec::new(),
                record_updates: Vec::new(),
                antecedents: Vec::new(),
            };
            let mut path = Path::default();
            let Some(arguments) = parameters
                .iter()
                .map(|parameter| proof.parameter(parameter.type_reference, &mut path))
                .collect::<Option<Vec<_>>>()
            else {
                return Vec::new();
            };
            let mut subjects = Vec::new();
            for ((parameter, argument), value) in parameters
                .iter()
                .zip(application.arguments.iter())
                .zip(&arguments)
            {
                if proof
                    .integer_subjects(
                        *argument,
                        parameter.type_reference,
                        value.clone(),
                        &mut path,
                        &mut subjects,
                        0,
                    )
                    .is_none()
                {
                    return Vec::new();
                }
            }
            let frame = Frame {
                machine: application.machine,
                state: 0,
                storage: Vec::new(),
                bindings: parameters
                    .iter()
                    .zip(&arguments)
                    .map(|(parameter, value)| (parameter.symbol, value.clone()))
                    .collect(),
            };
            let subjects = subjects
                .into_iter()
                .map(|(subject, value)| (super::PredicateIntegerOperand::Argument(subject), value))
                .collect();
            let Some(candidates) = proof.runtime_read_subjects(application, &frame, path, subjects)
            else {
                return Vec::new();
            };
            let Some((_, roster)) = candidates.first() else {
                return Vec::new();
            };
            let mut consequences = Vec::new();
            for left_position in 0..roster.len() {
                for right_position in (left_position + 1)..roster.len() {
                    for operator in [
                        BinaryOperator::NotEqual,
                        BinaryOperator::Equal,
                        BinaryOperator::Less,
                        BinaryOperator::LessOrEqual,
                        BinaryOperator::Greater,
                        BinaryOperator::GreaterOrEqual,
                    ] {
                        let mut proved = true;
                        for (path, subjects) in &candidates {
                            let goal = Boolean::Comparison(compare(
                                operator,
                                subjects[left_position].1.clone(),
                                subjects[right_position].1.clone(),
                            ));
                            if proof.spend().is_none() {
                                return consequences;
                            }
                            if proof.generalized_activation(
                                application.machine,
                                0,
                                arguments.clone(),
                                path.clone(),
                                &goal,
                                Judgment::Consequence,
                                0,
                            ) != Some(true)
                            {
                                proved = false;
                                break;
                            }
                            if meaning == RelationMeaning::Equivalent {
                                let Some(Some(converse_path)) =
                                    proof.assume(path.clone(), &goal, true)
                                else {
                                    proved = false;
                                    break;
                                };
                                if proof.generalized_activation(
                                    application.machine,
                                    0,
                                    arguments.clone(),
                                    converse_path,
                                    &Boolean::Constant(true),
                                    Judgment::RequiredResult,
                                    0,
                                ) != Some(true)
                                {
                                    proved = false;
                                    break;
                                }
                            }
                        }
                        if proved {
                            consequences.push(super::PredicateOrderingConsequence {
                                left: roster[left_position].0.clone(),
                                right: roster[right_position].0.clone(),
                                operator,
                            });
                            if meaning == RelationMeaning::Equivalent {
                                return consequences;
                            }
                        }
                        if proof.remaining == 0 {
                            return consequences;
                        }
                    }
                }
            }
            consequences
        }

        const WORK_BUDGET: usize = 8192;

        impl Implication<'_> {
            fn theorem(
                &mut self,
                entry: &State,
                machine_position: usize,
                goal: ExpressionHandle,
            ) -> bool {
                let program = self.program;
                let machine = self.context;
                let mut path = Path::default();
                let mut bindings = Vec::new();
                for parameter in program.state_parameters(entry) {
                    let Some(value) = self.parameter(parameter.type_reference, &mut path) else {
                        return false;
                    };
                    bindings.push((parameter.symbol, value));
                }
                let mut frame = Frame {
                    machine: machine_position,
                    state: 0,
                    bindings,
                    storage: Vec::new(),
                };
                if self
                    .seed_body_storage(&mut frame, &mut path, program.state_parameters(entry))
                    .is_none()
                {
                    return false;
                }
                let mut candidates = vec![(path, Vec::<Boolean>::new())];
                for premise in program.contract_expressions(
                    program.machine_contracts(machine).iter(),
                    SignatureContractKind::Requires,
                ) {
                    let mut next = Vec::new();
                    for (path, predicates) in candidates {
                        let Some(values) = self.expression(&frame, path, premise, 0) else {
                            continue;
                        };
                        for (path, value) in values {
                            let Value::Boolean(value) = value else {
                                continue;
                            };
                            let Some(branches) = self.premise(path, &value, true) else {
                                return false;
                            };
                            for (path, additional) in branches {
                                let mut combined = predicates.clone();
                                combined.extend(additional);
                                next.push((path, combined));
                            }
                        }
                    }
                    if next.is_empty() {
                        return false;
                    }
                    candidates = next;
                }
                for (path, predicates) in candidates {
                    self.antecedents = predicates;
                    let Some(exits) = self.body_exits(frame.clone(), path) else {
                        return false;
                    };
                    for (mut frame, path) in exits {
                        frame.machine = machine_position;
                        frame.state = 0;
                        let Some(goals) = self.expression(&frame, path, goal, 0) else {
                            return false;
                        };
                        for (path, goal) in goals {
                            let Value::Boolean(goal) = goal else {
                                return false;
                            };
                            if self.establish(&path, &goal, 0) != Some(true) {
                                return false;
                            }
                        }
                    }
                }
                true
            }

            fn establish(&mut self, path: &Path, goal: &Boolean, depth: usize) -> Option<bool> {
                self.spend()?;
                if depth >= 64 {
                    return None;
                }
                if self.entails(path, goal) {
                    return Some(true);
                }
                match goal {
                    Boolean::Call { machine, arguments } => {
                        if self.antecedents.iter().any(|antecedent| antecedent == goal) {
                            return Some(true);
                        }
                        return self.activation(
                            *machine,
                            0,
                            arguments.clone(),
                            path.clone(),
                            &Boolean::Constant(true),
                            Judgment::RequiredResult,
                            depth + 1,
                        );
                    }
                    Boolean::And(left, right) => {
                        return Some(
                            self.establish(path, left, depth + 1)?
                                && self.establish(path, right, depth + 1)?,
                        );
                    }
                    Boolean::Or(left, right)
                        if self.establish(path, left, depth + 1) == Some(true)
                            || self.establish(path, right, depth + 1) == Some(true) =>
                    {
                        return Some(true);
                    }
                    _ => {}
                }
                for antecedent in self.antecedents.clone() {
                    let Boolean::Call { machine, arguments } = antecedent else {
                        continue;
                    };
                    if self.activation(
                        machine,
                        0,
                        arguments,
                        path.clone(),
                        goal,
                        Judgment::Consequence,
                        depth + 1,
                    ) == Some(true)
                    {
                        return Some(true);
                    }
                }
                Some(false)
            }

            fn body_exits(&mut self, frame: Frame, path: Path) -> Option<Vec<(Frame, Path)>> {
                let entry = (frame.machine, frame.state);
                let mut pending = vec![(0usize, frame, path, vec![entry])];
                let mut exits = Vec::new();
                while let Some((position, frame, path, visited)) = pending.pop() {
                    self.spend()?;
                    let machine = &self.program.machines()[frame.machine];
                    let state = &self.program.machine_states(machine)[frame.state];
                    let statements = self
                        .program
                        .statement_table
                        .statements(state.statement_nodes);
                    let Some(statement) = statements.get(position) else {
                        exits.push((frame, path));
                        continue;
                    };
                    match statement {
                        StatementNode::LocalData(local)
                            if local.ordinary_value_initializer().is_some()
                                && local.relevance == language_core::BindingRelevance::Relevant =>
                        {
                            for (frame, path) in self.body_local(&frame, path, local)? {
                                pending.push((position + 1, frame, path, visited.clone()));
                            }
                        }
                        StatementNode::Assignment(assignment) => {
                            for (frame, path) in
                                self.assignment(&frame, path, assignment.target, assignment.value)?
                            {
                                pending.push((position + 1, frame, path, visited.clone()));
                            }
                        }
                        StatementNode::Transition(transition)
                            if transition.exit == TransitionExit::Ordinary
                                && !transition.continuation.is_valid() =>
                        {
                            let guards = match transition.guard {
                                TransitionGuardNode::Always => {
                                    vec![(path, Value::Boolean(Boolean::Constant(true)))]
                                }
                                TransitionGuardNode::When(guard) => {
                                    self.expression(&frame, path, guard, 0)?
                                }
                            };
                            for (path, guard) in guards {
                                let Value::Boolean(guard) = guard else {
                                    return None;
                                };
                                if let Some(path) = self.assume(path.clone(), &guard, false)? {
                                    pending.push((
                                        position + 1,
                                        frame.clone(),
                                        path,
                                        visited.clone(),
                                    ));
                                }
                                if let Some(path) = self.assume(path, &guard, true)? {
                                    match self
                                        .program
                                        .statement_table
                                        .transition_target(transition.target)
                                    {
                                        TransitionTargetNode::Terminal => {
                                            exits.push((frame.clone(), path))
                                        }
                                        TransitionTargetNode::Value(value) => {
                                            if let ExpressionNode::Call(call) =
                                                expression_table(self.program).expression(*value)
                                            {
                                                if !call.is_ordinary_positional_call() {
                                                    return None;
                                                }
                                                for (frame, path) in self.body_invocation(
                                                    &frame,
                                                    path,
                                                    call.target_symbol,
                                                    expression_table(self.program)
                                                        .expression_handles(call.arguments),
                                                )? {
                                                    let destination = (frame.machine, frame.state);
                                                    if visited.contains(&destination) {
                                                        return None;
                                                    }
                                                    let mut visited = visited.clone();
                                                    visited.push(destination);
                                                    pending.push((0, frame, path, visited));
                                                }
                                            } else {
                                                for (path, _) in
                                                    self.expression(&frame, path, *value, 0)?
                                                {
                                                    exits.push((frame.clone(), path));
                                                }
                                            }
                                        }
                                        TransitionTargetNode::Named {
                                            path: target,
                                            arguments,
                                            static_machine_parameter,
                                            evidence_arguments,
                                            ..
                                        } if !static_machine_parameter.is_valid()
                                            && evidence_arguments.is_empty() =>
                                        {
                                            for (frame, path) in self.body_invocation(
                                                &frame,
                                                path,
                                                target.symbol,
                                                self.program
                                                    .statement_table
                                                    .expression_handles(*arguments),
                                            )? {
                                                let destination = (frame.machine, frame.state);
                                                if visited.contains(&destination) {
                                                    return None;
                                                }
                                                let mut visited = visited.clone();
                                                visited.push(destination);
                                                pending.push((0, frame, path, visited));
                                            }
                                        }
                                        _ => return None,
                                    }
                                }
                            }
                        }
                        StatementNode::Expression(expression)
                            if position + 1 == statements.len() =>
                        {
                            if let ExpressionNode::Call(call) =
                                expression_table(self.program).expression(*expression)
                            {
                                if !call.is_ordinary_positional_call() {
                                    return None;
                                }
                                for (frame, path) in self.body_invocation(
                                    &frame,
                                    path,
                                    call.target_symbol,
                                    expression_table(self.program)
                                        .expression_handles(call.arguments),
                                )? {
                                    let destination = (frame.machine, frame.state);
                                    if visited.contains(&destination) {
                                        return None;
                                    }
                                    let mut visited = visited.clone();
                                    visited.push(destination);
                                    pending.push((0, frame, path, visited));
                                }
                            } else {
                                for (path, _) in self.expression(&frame, path, *expression, 0)? {
                                    exits.push((frame.clone(), path));
                                }
                            }
                        }
                        _ => return None,
                    }
                }
                Some(exits)
            }

            fn body_invocation(
                &mut self,
                frame: &Frame,
                path: Path,
                target: SymbolHandle,
                arguments: &[ExpressionHandle],
            ) -> Option<Vec<(Frame, Path)>> {
                let caller = &self.program.machines()[frame.machine];
                let destination = if let Some(state) =
                    typed_trees::state::state_position(self.program.machine_states(caller), target)
                        .filter(|position| *position != 0)
                {
                    (frame.machine, state)
                } else {
                    let (position, callee) =
                        self.program
                            .machines()
                            .iter()
                            .enumerate()
                            .find(|(_, callee)| {
                                callee.symbol == target
                                    || self
                                        .program
                                        .machine_states(callee)
                                        .first()
                                        .is_some_and(|entry| entry.symbol == target)
                            })?;
                    if callee.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
                        || !callee.body_is_present
                        || callee.structural_type_equations_pending
                        || callee.suspends
                        || callee.blocks
                        || !callee.type_parameters.is_empty()
                        || !self.program.machine_owned_data(callee).is_empty()
                        || !self
                            .program
                            .service_reach_rows
                            .services(callee.service_reach_row)
                            .is_empty()
                    {
                        return None;
                    }
                    (position, 0)
                };
                let state = &self
                    .program
                    .machine_states(&self.program.machines()[destination.0])[destination.1];
                let parameters = self.program.state_parameters(state);
                if parameters.len() != arguments.len()
                    || parameters
                        .iter()
                        .any(|parameter| parameter.is_const || parameter.relevance.is_erased())
                {
                    return None;
                }
                self.body_arguments(frame, path, arguments, parameters)?
                    .into_iter()
                    .map(|(mut path, arguments)| {
                        let mut frame = frame.clone();
                        frame.machine = destination.0;
                        frame.state = destination.1;
                        for (parameter, argument) in parameters.iter().zip(arguments) {
                            self.bind_body_storage(
                                &mut frame,
                                &mut path,
                                parameter.symbol,
                                argument,
                            )?;
                        }
                        Some((frame, path))
                    })
                    .collect()
            }

            fn spend(&mut self) -> Option<()> {
                self.remaining = self.remaining.checked_sub(1)?;
                Some(())
            }

            fn atom(&mut self, unsigned: bool) -> Integer {
                let identity = self.next_identity;
                self.next_identity += 1;
                Integer::Atom {
                    identity: format!("collection-proof:{identity}"),
                    unsigned,
                }
            }

            fn boolean(&mut self, path: &mut Path) -> Option<Value> {
                self.spend()?;
                let value = self.atom(true);
                boolean_bounds(path, &value);
                Some(Value::Boolean(Boolean::Variable(value)))
            }

            fn integer(&mut self, carrier: TypeReferenceHandle, path: &mut Path) -> Option<Value> {
                let (minimum, maximum) = self
                    .program
                    .primitive_type_reference(carrier)?
                    .integer_bounds()?;
                let value = self.atom(minimum >= 0);
                path.comparisons.push(compare(
                    BinaryOperator::GreaterOrEqual,
                    value.clone(),
                    number(minimum),
                ));
                path.comparisons.push(compare(
                    BinaryOperator::LessOrEqual,
                    value.clone(),
                    number(maximum),
                ));
                Some(Value::Integer {
                    value,
                    carrier: Some(carrier),
                })
            }

            fn parameter(
                &mut self,
                reference: TypeReferenceHandle,
                path: &mut Path,
            ) -> Option<Value> {
                match readable_shape(self.program, reference)? {
                    ValueShape::Integer(carrier) => self.integer(carrier, path),
                    ValueShape::Collection {
                        length,
                        element_type,
                    } => {
                        let identity = self.next_identity;
                        self.next_identity += 1;
                        let length = length
                            .map(|length| number(length as i128))
                            .unwrap_or_else(|| self.atom(true));
                        path.comparisons.push(compare(
                            BinaryOperator::GreaterOrEqual,
                            length.clone(),
                            number(0),
                        ));
                        path.comparisons.push(compare(
                            BinaryOperator::LessOrEqual,
                            length.clone(),
                            number(i128::from(u64::MAX)),
                        ));
                        Some(Value::Collection {
                            identity,
                            length,
                            element: element_type,
                        })
                    }
                    ValueShape::Record(reference) => {
                        let identity = self.next_identity;
                        self.next_identity += 1;
                        Some(Value::Record {
                            identity,
                            reference,
                            projection: Vec::new(),
                        })
                    }
                    ValueShape::Boolean => self.boolean(path),
                }
            }

            fn unsatisfiable(&self, comparisons: &[Comparison]) -> bool {
                scoped_arithmetic_comparisons_unsatisfiable(self.program, self.context, comparisons)
            }

            fn contradictory(
                &self,
                comparisons: &[Comparison],
                clauses: &[Clause],
                focus: Option<&[Comparison]>,
            ) -> bool {
                if comparisons
                    .iter()
                    .any(|comparison| constant_truth(comparison) == Some(false))
                {
                    return true;
                }
                let focus_atoms = focus.map(|focus| {
                    let mut atoms = Vec::new();
                    for comparison in focus {
                        comparison_atoms(comparison, &mut atoms);
                    }
                    atoms
                });
                let mut consultations = SPLIT_CONSULTATIONS;
                components(comparisons, clauses)
                    .iter()
                    .filter(|component| {
                        focus_atoms.as_ref().is_none_or(|atoms| {
                            atoms.iter().any(|atom| component.atoms.contains(atom))
                        })
                    })
                    .any(|component| {
                        self.split(
                            component.comparisons.clone(),
                            &component.clauses,
                            &mut consultations,
                        )
                    })
            }

            fn split(
                &self,
                comparisons: Vec<Comparison>,
                clauses: &[Clause],
                consultations: &mut usize,
            ) -> bool {
                let Some(remaining) = consultations.checked_sub(1) else {
                    return false;
                };
                *consultations = remaining;
                if self.unsatisfiable(&comparisons) {
                    return true;
                }
                let Some((clause, rest)) = clauses.split_first() else {
                    return false;
                };
                clause.iter().all(|literal| {
                    let mut comparisons = comparisons.clone();
                    comparisons.push(literal.clone());
                    self.split(comparisons, rest, consultations)
                })
            }

            fn impossible(&self, path: &Path) -> bool {
                self.contradictory(&path.comparisons, &path.clauses, None)
            }

            fn implies_clause(&self, path: &Path, clause: &[Comparison]) -> bool {
                let mut negated = Vec::new();
                for literal in clause {
                    match constant_truth(literal) {
                        Some(true) => return true,
                        Some(false) => continue,
                        None => {}
                    }
                    if path.comparisons.contains(literal) {
                        return true;
                    }
                    let negation = compare(
                        literal.operator.complement().unwrap_or(literal.operator),
                        literal.left.clone(),
                        literal.right.clone(),
                    );
                    if !path.comparisons.contains(&negation) {
                        negated.push(negation);
                    }
                }
                if negated.is_empty() {
                    return false;
                }
                let mut comparisons = path.comparisons.clone();
                comparisons.extend(negated.iter().cloned());
                self.contradictory(&comparisons, &path.clauses, Some(&negated))
            }

            fn implies(&self, path: &Path, comparison: &Comparison) -> bool {
                self.implies_clause(path, std::slice::from_ref(comparison))
            }

            fn entails(&self, path: &Path, goal: &Boolean) -> bool {
                if self.impossible(path) {
                    return true;
                }
                match goal {
                    Boolean::Constant(value) => *value,
                    Boolean::Variable(value) => self.implies(
                        path,
                        &compare(BinaryOperator::Equal, value.clone(), number(1)),
                    ),
                    Boolean::Comparison(comparison) => self.implies(path, comparison),
                    Boolean::And(left, right) => {
                        self.entails(path, left) && self.entails(path, right)
                    }
                    Boolean::Or(left, right) => {
                        self.entails(path, left)
                            || self.entails(path, right)
                            || self
                                .assume_readonly(path, goal, false)
                                .is_some_and(|assumed| {
                                    assumed.is_none_or(|path| self.impossible(&path))
                                })
                    }
                    Boolean::Not(value) => self
                        .assume_readonly(path, value, true)
                        .is_some_and(|assumed| assumed.is_none_or(|path| self.impossible(&path))),
                    Boolean::Call { .. } => false,
                }
            }

            fn assume_readonly(
                &self,
                path: &Path,
                value: &Boolean,
                holds: bool,
            ) -> Option<Option<Path>> {
                let mut path = path.clone();
                for clause in clauses(value, holds)? {
                    match clause.as_slice() {
                        [] => return Some(None),
                        [comparison] => path.comparisons.push(comparison.clone()),
                        _ => path.clauses.push(clause),
                    }
                }
                Some(Some(path))
            }

            fn assume(&mut self, path: Path, value: &Boolean, holds: bool) -> Option<Option<Path>> {
                self.spend()?;
                Some(
                    self.assume_readonly(&path, value, holds)?
                        .filter(|path| !self.impossible(path)),
                )
            }

            fn premise(
                &mut self,
                path: Path,
                value: &Boolean,
                holds: bool,
            ) -> Option<Vec<(Path, Vec<Boolean>)>> {
                self.spend()?;
                match value {
                    Boolean::Call { .. } => Some(vec![(
                        path,
                        if holds {
                            vec![value.clone()]
                        } else {
                            Vec::new()
                        },
                    )]),
                    Boolean::Not(value) => self.premise(path, value, !holds),
                    Boolean::And(left, right) | Boolean::Or(left, right) => {
                        if matches!(value, Boolean::And(..)) == holds {
                            let mut result = Vec::new();
                            for (path, first) in self.premise(path, left, holds)? {
                                for (path, second) in self.premise(path, right, holds)? {
                                    let mut combined = first.clone();
                                    combined.extend(second);
                                    result.push((path, combined));
                                }
                            }
                            Some(result)
                        } else {
                            let mut result = self.premise(path.clone(), left, holds)?;
                            result.extend(self.premise(path, right, holds)?);
                            Some(result)
                        }
                    }
                    _ => Some(
                        self.assume(path, value, holds)?
                            .into_iter()
                            .map(|path| (path, Vec::new()))
                            .collect(),
                    ),
                }
            }

            fn expression(
                &mut self,
                frame: &Frame,
                path: Path,
                expression: ExpressionHandle,
                depth: usize,
            ) -> Option<Vec<(Path, Value)>> {
                self.spend()?;
                if depth >= 64 || !expression_table(self.program).expression_is_valid(expression) {
                    return None;
                }
                let machine = &self.program.machines()[frame.machine];
                let state = &self.program.machine_states(machine)[frame.state];
                match expression_table(self.program).expression(expression) {
                    ExpressionNode::Integer(literal) => Some(vec![(
                        path,
                        Value::Integer {
                            value: Integer::Integer(literal.value_bignum()?),
                            carrier: None,
                        },
                    )]),
                    ExpressionNode::Boolean(value) => {
                        Some(vec![(path, Value::Boolean(Boolean::Constant(*value)))])
                    }
                    ExpressionNode::Name(name) => {
                        if expression_table(self.program)
                            .name_path_members(name.members)
                            .len()
                            != 1
                            || name.symbol != name.head_symbol
                        {
                            return None;
                        }
                        if let Some(binding) = frame
                            .storage
                            .iter()
                            .rev()
                            .find(|binding| binding.symbol == name.symbol)
                        {
                            return self.storage_value(path, &binding.place);
                        }
                        let value = frame
                            .bindings
                            .iter()
                            .rev()
                            .find(|(symbol, _)| *symbol == name.symbol)?
                            .1
                            .clone();
                        Some(vec![(path, value)])
                    }
                    ExpressionNode::Borrow(borrow) if borrow.access.is_readable() => {
                        self.expression(frame, path, borrow.target, depth + 1)
                    }
                    ExpressionNode::Member(member)
                        if super::length_member(self.program, expression).is_some() =>
                    {
                        let values = self.expression(frame, path, member.receiver, depth + 1)?;
                        values
                            .into_iter()
                            .map(|(path, value)| {
                                let Value::Collection { length, .. } = value else {
                                    return None;
                                };
                                Some((
                                    path,
                                    Value::Integer {
                                        value: length,
                                        carrier: Some(self.u64_type()?),
                                    },
                                ))
                            })
                            .collect()
                    }
                    ExpressionNode::Member(member) => {
                        let symbol = crate::value_custody::places::effective_member_symbol(
                            self.program,
                            member.receiver,
                            member,
                        );
                        let mut result = Vec::new();
                        for (path, receiver) in
                            self.expression(frame, path, member.receiver, depth + 1)?
                        {
                            result.push(self.record_member(path, receiver, symbol)?);
                        }
                        Some(result)
                    }
                    ExpressionNode::Indexed(indexed) => {
                        if !crate::value_custody::places::place_has_builtin_coordinates(
                            self.program,
                            machine,
                            Some(state),
                            expression,
                        ) {
                            return None;
                        }
                        let mut result = Vec::new();
                        for (path, collection) in
                            self.expression(frame, path, indexed.collection, depth + 1)?
                        {
                            let Value::Collection {
                                identity,
                                length,
                                element,
                            } = collection
                            else {
                                return None;
                            };
                            for (path, selector) in
                                self.expression(frame, path, indexed.index, depth + 1)?
                            {
                                let Value::Integer {
                                    value: selector, ..
                                } = selector
                                else {
                                    return None;
                                };
                                if !self.implies(
                                    &path,
                                    &compare(
                                        BinaryOperator::GreaterOrEqual,
                                        selector.clone(),
                                        number(0),
                                    ),
                                ) || !self.implies(
                                    &path,
                                    &compare(
                                        BinaryOperator::Less,
                                        selector.clone(),
                                        length.clone(),
                                    ),
                                ) {
                                    return None;
                                }
                                result.extend(self.read(path, identity, selector, element, 0)?);
                            }
                        }
                        Some(result)
                    }
                    ExpressionNode::Unary(unary) => {
                        let mut result = Vec::new();
                        for (path, value) in
                            self.expression(frame, path, unary.operand, depth + 1)?
                        {
                            let value = match (unary.operator, value) {
                                (UnaryOperator::LogicalNot, Value::Boolean(value)) => {
                                    Value::Boolean(Boolean::Not(Box::new(value)))
                                }
                                _ => return None,
                            };
                            result.push((path, value));
                        }
                        Some(result)
                    }
                    ExpressionNode::Binary(binary) => {
                        if !has_builtin_binary_expression_meaning(
                            self.program,
                            machine,
                            Some(state),
                            expression,
                        ) {
                            return None;
                        }
                        let mut result = Vec::new();
                        for (path, left) in self.expression(frame, path, binary.left, depth + 1)? {
                            for (path, right) in
                                self.expression(frame, path, binary.right, depth + 1)?
                            {
                                let value = match (left.clone(), right) {
                                    (
                                        Value::Integer {
                                            value: left,
                                            carrier: left_carrier,
                                        },
                                        Value::Integer {
                                            value: right,
                                            carrier: right_carrier,
                                        },
                                    ) => match binary.operator {
                                        BinaryOperator::Add
                                        | BinaryOperator::Subtract
                                        | BinaryOperator::Multiply => {
                                            let carrier = left_carrier.or(right_carrier)?;
                                            if self
                                                .program
                                                .arithmetic_domain_for_type_reference(carrier)
                                                != numerics::arithmetic::ArithmeticDomain::Exact
                                            {
                                                return None;
                                            }
                                            Value::Integer {
                                                value: Integer::IntegerOperation {
                                                    operator: binary.operator,
                                                    left: Box::new(left),
                                                    right: Box::new(right),
                                                },
                                                carrier: Some(carrier),
                                            }
                                        }
                                        BinaryOperator::Equal
                                        | BinaryOperator::NotEqual
                                        | BinaryOperator::Less
                                        | BinaryOperator::LessOrEqual
                                        | BinaryOperator::Greater
                                        | BinaryOperator::GreaterOrEqual => {
                                            Value::Boolean(Boolean::Comparison(compare(
                                                binary.operator,
                                                left,
                                                right,
                                            )))
                                        }
                                        _ => return None,
                                    },
                                    (Value::Boolean(left), Value::Boolean(right)) => {
                                        match binary.operator {
                                            BinaryOperator::And => Value::Boolean(Boolean::And(
                                                Box::new(left),
                                                Box::new(right),
                                            )),
                                            BinaryOperator::Or => Value::Boolean(Boolean::Or(
                                                Box::new(left),
                                                Box::new(right),
                                            )),
                                            BinaryOperator::Equal | BinaryOperator::NotEqual => {
                                                let equal = Boolean::Or(
                                                    Box::new(Boolean::And(
                                                        Box::new(left.clone()),
                                                        Box::new(right.clone()),
                                                    )),
                                                    Box::new(Boolean::And(
                                                        Box::new(Boolean::Not(Box::new(left))),
                                                        Box::new(Boolean::Not(Box::new(right))),
                                                    )),
                                                );
                                                Value::Boolean(
                                                    if binary.operator == BinaryOperator::Equal {
                                                        equal
                                                    } else {
                                                        Boolean::Not(Box::new(equal))
                                                    },
                                                )
                                            }
                                            _ => return None,
                                        }
                                    }
                                    _ => return None,
                                };
                                result.push((path, value));
                            }
                        }
                        Some(result)
                    }
                    ExpressionNode::Call(call) if call.is_ordinary_positional_call() => {
                        let machine =
                            admitted_predicate_machine(self.program, call.target_symbol, true)?;
                        let arguments =
                            expression_table(self.program).expression_handles(call.arguments);
                        Some(
                            self.arguments(frame, path, arguments, depth + 1)?
                                .into_iter()
                                .map(|(path, arguments)| {
                                    (path, Value::Boolean(Boolean::Call { machine, arguments }))
                                })
                                .collect(),
                        )
                    }
                    _ => None,
                }
            }

            fn u64_type(&self) -> Option<TypeReferenceHandle> {
                self.program
                    .type_reference_table
                    .named_references()
                    .find_map(|(reference, symbol, _)| {
                        (self.program.symbols.builtin_type_atom(symbol)
                            == Some(symbols::BuiltinTypeAtom::U64))
                        .then_some(reference)
                    })
            }

            fn arguments(
                &mut self,
                frame: &Frame,
                path: Path,
                arguments: &[ExpressionHandle],
                depth: usize,
            ) -> Option<Vec<(Path, Vec<Value>)>> {
                let mut rows = vec![(path, Vec::new())];
                for argument in arguments {
                    let mut next = Vec::new();
                    for (path, values) in rows {
                        for (path, value) in self.expression(frame, path, *argument, depth + 1)? {
                            let mut values = values.clone();
                            values.push(value);
                            next.push((path, values));
                        }
                    }
                    rows = next;
                }
                Some(rows)
            }

            fn read(
                &mut self,
                path: Path,
                collection: usize,
                selector: Integer,
                element: TypeReferenceHandle,
                observation_position: usize,
            ) -> Option<Vec<(Path, Value)>> {
                self.spend()?;
                if let Some(update) = self
                    .updates
                    .iter()
                    .find(|update| update.identity == collection)
                {
                    let previous = update.previous;
                    let stored_selector = update.selector.clone();
                    let stored_value = update.value.clone();
                    let equality = Boolean::Comparison(compare(
                        BinaryOperator::Equal,
                        selector.clone(),
                        stored_selector,
                    ));
                    let mut result = self
                        .assume(path.clone(), &equality, true)?
                        .into_iter()
                        .map(|path| Some((path, observed_carrier(stored_value.clone(), element)?)))
                        .collect::<Option<Vec<_>>>()?;
                    if let Some(path) = self.assume(path, &equality, false)? {
                        result.extend(self.read(path, previous, selector.clone(), element, 0)?);
                    }
                    return Some(result);
                }
                let Some(observation) = path.observations.get(observation_position).cloned() else {
                    let mut path = path;
                    let value = self.integer(element, &mut path)?;
                    path.observations.push(Observation {
                        collection,
                        selector,
                        value: value.clone(),
                    });
                    return Some(vec![(path, value)]);
                };
                if observation.collection != collection {
                    return self.read(
                        path,
                        collection,
                        selector,
                        element,
                        observation_position + 1,
                    );
                }
                let equality = compare(
                    BinaryOperator::Equal,
                    selector.clone(),
                    observation.selector.clone(),
                );
                if self.implies(&path, &equality) {
                    return Some(vec![(path, observed_carrier(observation.value, element)?)]);
                }
                if constant_truth(&equality) == Some(false)
                    || self.implies(
                        &path,
                        &compare(
                            BinaryOperator::NotEqual,
                            selector.clone(),
                            observation.selector.clone(),
                        ),
                    )
                {
                    return self.read(
                        path,
                        collection,
                        selector,
                        element,
                        observation_position + 1,
                    );
                }
                let mut equal_path = path.clone();
                equal_path.comparisons.push(equality);
                let mut different_path = path;
                different_path.comparisons.push(compare(
                    BinaryOperator::NotEqual,
                    selector.clone(),
                    observation.selector,
                ));
                let mut result = vec![(equal_path, observed_carrier(observation.value, element)?)];
                result.extend(self.read(
                    different_path,
                    collection,
                    selector,
                    element,
                    observation_position + 1,
                )?);
                Some(result)
            }

            fn activation(
                &mut self,
                machine: usize,
                state: usize,
                arguments: Vec<Value>,
                mut path: Path,
                goal: &Boolean,
                judgment: Judgment,
                depth: usize,
            ) -> Option<bool> {
                self.spend()?;
                if depth >= 64 {
                    return None;
                }
                if judgment == Judgment::Consequence && self.entails(&path, goal) {
                    return Some(true);
                }
                let owner = self.program.machines().get(machine)?;
                let target = self.program.machine_states(owner).get(state)?;
                let parameters = self.program.state_parameters(target);
                if !admitted_activation_parameters(self.program, parameters, arguments.len()) {
                    return None;
                }
                let mut recurrent = false;
                for lemma_position in (0..self.lemmas.len()).rev() {
                    let lemma = &self.lemmas[lemma_position];
                    if lemma.machine != machine
                        || lemma.state != state
                        || lemma.judgment != judgment
                        || lemma.goal != *goal
                    {
                        continue;
                    }
                    recurrent = true;
                    let replacements = substitutions(self.program, &lemma.arguments, &arguments)?;
                    if substitute_boolean(&lemma.goal, &replacements) != *goal {
                        continue;
                    }
                    if !lemma.premises.iter().all(|premise| {
                        self.implies(&path, &substitute_comparison(premise, &replacements))
                    }) || !lemma.premise_clauses.iter().all(|premise| {
                        self.implies_clause(&path, &substitute_clause(premise, &replacements))
                    }) {
                        continue;
                    }
                    let measures = lemma
                        .measures
                        .iter()
                        .copied()
                        .filter(|(position, direction)| {
                            let (
                                Value::Integer { value: before, .. },
                                Value::Integer { value: after, .. },
                            ) = (&lemma.arguments[*position], &arguments[*position])
                            else {
                                return false;
                            };
                            self.implies(&path, &compare(*direction, after.clone(), before.clone()))
                        })
                        .collect::<Vec<_>>();
                    if measures.is_empty() {
                        continue;
                    }
                    self.lemmas[lemma_position].measures = measures;
                    return Some(true);
                }
                if recurrent {
                    return None;
                }
                let dependent = self.depends_on_enclosing_activation(&arguments, &path)?;
                let mut generalized = parameters
                    .iter()
                    .zip(&arguments)
                    .map(|(parameter, argument)| {
                        Some(
                            match (
                                value_shape(self.program, parameter.type_reference)?,
                                argument,
                            ) {
                                (ValueShape::Integer(carrier), Value::Integer { value, .. }) => {
                                    Value::Integer {
                                        value: value.clone(),
                                        carrier: Some(carrier),
                                    }
                                }
                                (
                                    ValueShape::Collection { element_type, .. },
                                    Value::Collection {
                                        identity, length, ..
                                    },
                                ) => Value::Collection {
                                    identity: *identity,
                                    length: length.clone(),
                                    element: element_type,
                                },
                                (
                                    ValueShape::Record(reference),
                                    Value::Record {
                                        identity,
                                        projection,
                                        ..
                                    },
                                ) => Value::Record {
                                    identity: *identity,
                                    reference,
                                    projection: projection.clone(),
                                },
                                (ValueShape::Boolean, Value::Boolean(value)) => {
                                    Value::Boolean(value.clone())
                                }
                                _ => return None,
                            },
                        )
                    })
                    .collect::<Option<Vec<_>>>()?;
                let mut replacements = Vec::new();
                let mut formal_bounds = Vec::new();
                if !dependent {
                    for (position, (parameter, argument)) in
                        parameters.iter().zip(&arguments).enumerate()
                    {
                        let Value::Integer { value, .. } = argument else {
                            continue;
                        };
                        let mut bounds = Path::default();
                        let fresh = self.integer(parameter.type_reference, &mut bounds)?;
                        let Value::Integer {
                            value: replacement, ..
                        } = &fresh
                        else {
                            return None;
                        };
                        replacements.push((value.clone(), replacement.clone()));
                        generalized[position] = fresh;
                        formal_bounds.extend(bounds.comparisons);
                    }
                    if judgment == Judgment::Consequence {
                        let mut support = Vec::new();
                        boolean_atoms(goal, &mut support);
                        for observation in &path.observations {
                            if let Value::Integer { value, .. } = &observation.value
                                && (matches!(goal, Boolean::Constant(false))
                                    || support.iter().any(|atom| contains(value, atom)))
                            {
                                integer_atoms(&observation.selector, &mut support);
                            }
                        }
                        let mentions_support = |comparison: &Comparison| {
                            support.iter().any(|atom| {
                                contains(&comparison.left, atom)
                                    || contains(&comparison.right, atom)
                            })
                        };
                        let generalized_premises = path
                            .comparisons
                            .iter()
                            .filter(|comparison| mentions_support(comparison))
                            .map(|comparison| substitute_comparison(comparison, &replacements))
                            .filter(|comparison| !path.comparisons.contains(comparison))
                            .collect::<Vec<_>>();
                        let generalized_clauses = path
                            .clauses
                            .iter()
                            .filter(|clause| clause.iter().any(mentions_support))
                            .map(|clause| substitute_clause(clause, &replacements))
                            .filter(|clause| !path.clauses.contains(clause))
                            .collect::<Vec<_>>();
                        path.comparisons.extend(generalized_premises);
                        path.clauses.extend(generalized_clauses);
                    }
                    path.comparisons.extend(formal_bounds);
                }
                self.generalized_activation(
                    machine,
                    state,
                    generalized,
                    path,
                    goal,
                    judgment,
                    depth,
                )
            }

            fn depends_on_enclosing_activation(
                &mut self,
                arguments: &[Value],
                path: &Path,
            ) -> Option<bool> {
                let formals = self
                    .lemmas
                    .iter()
                    .flat_map(|lemma| lemma.arguments.iter().cloned())
                    .collect::<Vec<_>>();
                for argument in arguments {
                    let Value::Integer { value, .. } = argument else {
                        continue;
                    };
                    for formal in &formals {
                        self.spend()?;
                        match formal {
                            Value::Integer { value: formal, .. } if contains(value, formal) => {
                                return Some(true);
                            }
                            Value::Collection { identity, .. }
                                if self.collection_observes_integer(*identity, value, path)? =>
                            {
                                return Some(true);
                            }
                            Value::Record {
                                identity,
                                projection,
                                ..
                            } if self
                                .record_observes_integer(*identity, projection, value, path)? =>
                            {
                                return Some(true);
                            }
                            _ => {}
                        }
                    }
                }
                Some(false)
            }

            fn collection_observes_integer(
                &mut self,
                identity: usize,
                value: &Integer,
                path: &Path,
            ) -> Option<bool> {
                for observation in &path.observations {
                    self.spend()?;
                    if observation.collection == identity
                        && let Value::Integer {
                            value: observed, ..
                        } = &observation.value
                        && contains(value, observed)
                    {
                        return Some(true);
                    }
                }
                Some(false)
            }

            fn generalized_activation(
                &mut self,
                machine: usize,
                state: usize,
                arguments: Vec<Value>,
                path: Path,
                goal: &Boolean,
                judgment: Judgment,
                depth: usize,
            ) -> Option<bool> {
                let owner = self.program.machines().get(machine)?;
                let target = self.program.machine_states(owner).get(state)?;
                let parameters = self.program.state_parameters(target);
                if !admitted_activation_parameters(self.program, parameters, arguments.len()) {
                    return None;
                }
                let dependent = self.depends_on_enclosing_activation(&arguments, &path)?;
                let frame = Frame {
                    machine,
                    state,
                    storage: Vec::new(),
                    bindings: parameters
                        .iter()
                        .zip(&arguments)
                        .map(|(parameter, value)| (parameter.symbol, value.clone()))
                        .collect(),
                };
                let mut paths = vec![path];
                let machine_contracts = if state == 0 {
                    self.program.machine_contracts(owner)
                } else {
                    &[]
                };
                for requirement in self.program.contract_expressions(
                    machine_contracts
                        .iter()
                        .chain(self.program.state_contracts(target).iter()),
                    SignatureContractKind::Requires,
                ) {
                    let mut next = Vec::new();
                    for path in paths {
                        for (path, value) in self.expression(&frame, path, requirement, 0)? {
                            let Value::Boolean(value) = value else {
                                return None;
                            };
                            next.extend(self.assume(path, &value, true)?);
                        }
                    }
                    paths = next;
                }
                let mut result = true;
                for path in paths {
                    if !dependent {
                        let measures = arguments
                            .iter()
                            .enumerate()
                            .filter(|(_, value)| matches!(value, Value::Integer { .. }))
                            .flat_map(|(position, _)| {
                                [
                                    (position, BinaryOperator::Less),
                                    (position, BinaryOperator::Greater),
                                ]
                            })
                            .collect();
                        self.lemmas.push(Lemma {
                            machine,
                            state,
                            arguments: arguments.clone(),
                            premises: path.comparisons.clone(),
                            premise_clauses: path.clauses.clone(),
                            measures,
                            judgment,
                            goal: goal.clone(),
                        });
                    }
                    let proved = self.statements(&frame, path, goal, judgment, depth + 1);
                    if !dependent {
                        self.lemmas.pop();
                    }
                    result &= proved?;
                    if !result {
                        break;
                    }
                }
                Some(result)
            }

            fn statements(
                &mut self,
                frame: &Frame,
                path: Path,
                goal: &Boolean,
                judgment: Judgment,
                depth: usize,
            ) -> Option<bool> {
                let machine = &self.program.machines()[frame.machine];
                let state = &self.program.machine_states(machine)[frame.state];
                let statements = self
                    .program
                    .statement_table
                    .statements(state.statement_nodes);
                let mut pending = vec![(0usize, frame.clone(), path)];
                while let Some((statement_position, frame, path)) = pending.pop() {
                    self.spend()?;
                    if judgment == Judgment::Consequence && self.entails(&path, goal) {
                        continue;
                    }
                    match statements.get(statement_position)? {
                        StatementNode::LocalData(local)
                            if !local.is_mutable
                                && local.ordinary_value_initializer().is_some()
                                && local.relevance == language_core::BindingRelevance::Relevant =>
                        {
                            for (path, value) in
                                self.expression(&frame, path, local.initial_value, 0)?
                            {
                                let mut frame = frame.clone();
                                frame.bindings.push((
                                    local.symbol,
                                    declared_value(self.program, value, local.type_reference)?,
                                ));
                                pending.push((statement_position + 1, frame, path));
                            }
                        }
                        StatementNode::Transition(transition)
                            if transition.exit == TransitionExit::Ordinary
                                && !transition.continuation.is_valid() =>
                        {
                            let guards = match transition.guard {
                                TransitionGuardNode::Always => {
                                    vec![(path, Value::Boolean(Boolean::Constant(true)))]
                                }
                                TransitionGuardNode::When(guard) => {
                                    self.expression(&frame, path, guard, 0)?
                                }
                            };
                            for (path, guard) in guards {
                                let Value::Boolean(guard) = guard else {
                                    return None;
                                };
                                if let Some(path) = self.assume(path.clone(), &guard, false)? {
                                    pending.push((statement_position + 1, frame.clone(), path));
                                }
                                if let Some(path) = self.assume(path, &guard, true)? {
                                    match self
                                        .program
                                        .statement_table
                                        .transition_target(transition.target)
                                    {
                                        TransitionTargetNode::Value(value) => {
                                            if !self.result(
                                                &frame,
                                                path,
                                                *value,
                                                goal,
                                                judgment,
                                                depth + 1,
                                            )? {
                                                return Some(false);
                                            }
                                        }
                                        TransitionTargetNode::Named {
                                            path: target,
                                            arguments,
                                            static_machine_parameter,
                                            evidence_arguments,
                                            ..
                                        } if !static_machine_parameter.is_valid()
                                            && evidence_arguments.is_empty() =>
                                        {
                                            let (machine, state) = if let Some(state) =
                                                typed_trees::state::state_position(
                                                    self.program.machine_states(machine),
                                                    target.symbol,
                                                )
                                                .filter(|position| *position != 0)
                                            {
                                                (frame.machine, state)
                                            } else {
                                                (
                                                    admitted_predicate_machine(
                                                        self.program,
                                                        target.symbol,
                                                        true,
                                                    )?,
                                                    0,
                                                )
                                            };
                                            for (path, arguments) in self.arguments(
                                                &frame,
                                                path,
                                                self.program
                                                    .statement_table
                                                    .expression_handles(*arguments),
                                                0,
                                            )? {
                                                if !self.activation(
                                                    machine,
                                                    state,
                                                    arguments,
                                                    path,
                                                    goal,
                                                    judgment,
                                                    depth + 1,
                                                )? {
                                                    return Some(false);
                                                }
                                            }
                                        }
                                        _ => return None,
                                    }
                                }
                            }
                        }
                        StatementNode::Expression(value)
                            if statement_position + 1 == statements.len() =>
                        {
                            if !self.result(&frame, path, *value, goal, judgment, depth + 1)? {
                                return Some(false);
                            }
                        }
                        _ => return None,
                    }
                }
                Some(true)
            }

            fn result(
                &mut self,
                frame: &Frame,
                path: Path,
                expression: ExpressionHandle,
                goal: &Boolean,
                judgment: Judgment,
                depth: usize,
            ) -> Option<bool> {
                for (path, value) in self.expression(frame, path, expression, 0)? {
                    let Value::Boolean(value) = value else {
                        return None;
                    };
                    if judgment == Judgment::RequiredResult {
                        if self.establish(&path, &value, depth + 1) != Some(true) {
                            return Some(false);
                        }
                        continue;
                    }
                    if let Boolean::Call { machine, arguments } = value {
                        if !self.activation(
                            machine,
                            0,
                            arguments,
                            path,
                            goal,
                            judgment,
                            depth + 1,
                        )? {
                            return Some(false);
                        }
                    } else if let Some(path) = self.assume(path, &value, true)?
                        && !self.entails(&path, goal)
                    {
                        return Some(false);
                    }
                }
                Some(true)
            }
        }

        pub(crate) fn admitted_activation_parameters(
            program: &dyn crate::CheckingInput,
            parameters: &[typed_trees::signature::StateParameter],
            argument_count: usize,
        ) -> bool {
            parameters.len() == argument_count
                && parameters.iter().all(|parameter| {
                    !parameter.is_mutable
                        && value_shape(program, parameter.type_reference).is_some()
                })
        }

        fn number(value: i128) -> Integer {
            Integer::Integer(BigInt::from_i128(value))
        }

        fn compare(operator: BinaryOperator, left: Integer, right: Integer) -> Comparison {
            Comparison {
                operator,
                left,
                right,
            }
        }

        fn contains(value: &Integer, sought: &Integer) -> bool {
            value == sought
                || matches!(value, Integer::IntegerOperation { left, right, .. } if contains(left, sought) || contains(right, sought))
        }

        fn integer_atoms(value: &Integer, atoms: &mut Vec<Integer>) {
            match value {
                Integer::Atom { .. } => {
                    if !atoms.contains(value) {
                        atoms.push(value.clone());
                    }
                }
                Integer::IntegerOperation { left, right, .. } => {
                    integer_atoms(left, atoms);
                    integer_atoms(right, atoms);
                }
                _ => {}
            }
        }

        fn boolean_atoms(value: &Boolean, atoms: &mut Vec<Integer>) {
            match value {
                Boolean::Variable(value) => integer_atoms(value, atoms),
                Boolean::Comparison(comparison) => {
                    integer_atoms(&comparison.left, atoms);
                    integer_atoms(&comparison.right, atoms);
                }
                Boolean::And(left, right) | Boolean::Or(left, right) => {
                    boolean_atoms(left, atoms);
                    boolean_atoms(right, atoms);
                }
                Boolean::Not(value) => boolean_atoms(value, atoms),
                _ => {}
            }
        }

        fn substitute(value: &Integer, replacements: &[(Integer, Integer)]) -> Integer {
            if let Some((_, replacement)) =
                replacements.iter().find(|(original, _)| original == value)
            {
                return replacement.clone();
            }
            match value {
                Integer::IntegerOperation {
                    operator,
                    left,
                    right,
                } => Integer::IntegerOperation {
                    operator: *operator,
                    left: Box::new(substitute(left, replacements)),
                    right: Box::new(substitute(right, replacements)),
                },
                _ => value.clone(),
            }
        }

        fn substitute_value(value: &Value, replacements: &[(Integer, Integer)]) -> Value {
            match value {
                Value::Record { .. } => value.clone(),
                Value::Integer { value, carrier } => Value::Integer {
                    value: substitute(value, replacements),
                    carrier: *carrier,
                },
                Value::Collection {
                    identity,
                    length,
                    element,
                } => Value::Collection {
                    identity: *identity,
                    length: substitute(length, replacements),
                    element: *element,
                },
                Value::Boolean(value) => Value::Boolean(substitute_boolean(value, replacements)),
            }
        }

        fn substitute_boolean(value: &Boolean, replacements: &[(Integer, Integer)]) -> Boolean {
            match value {
                Boolean::Constant(value) => Boolean::Constant(*value),
                Boolean::Variable(value) => Boolean::Variable(substitute(value, replacements)),
                Boolean::Comparison(comparison) => {
                    Boolean::Comparison(substitute_comparison(comparison, replacements))
                }
                Boolean::And(left, right) => Boolean::And(
                    Box::new(substitute_boolean(left, replacements)),
                    Box::new(substitute_boolean(right, replacements)),
                ),
                Boolean::Or(left, right) => Boolean::Or(
                    Box::new(substitute_boolean(left, replacements)),
                    Box::new(substitute_boolean(right, replacements)),
                ),
                Boolean::Not(value) => {
                    Boolean::Not(Box::new(substitute_boolean(value, replacements)))
                }
                Boolean::Call { machine, arguments } => Boolean::Call {
                    machine: *machine,
                    arguments: arguments
                        .iter()
                        .map(|argument| substitute_value(argument, replacements))
                        .collect(),
                },
            }
        }

        fn substitute_comparison(
            comparison: &Comparison,
            replacements: &[(Integer, Integer)],
        ) -> Comparison {
            compare(
                comparison.operator,
                substitute(&comparison.left, replacements),
                substitute(&comparison.right, replacements),
            )
        }

        fn substitute_clause(clause: &[Comparison], replacements: &[(Integer, Integer)]) -> Clause {
            clause
                .iter()
                .map(|comparison| substitute_comparison(comparison, replacements))
                .collect()
        }

        const SPLIT_CONSULTATIONS: usize = 256;
        const MAXIMUM_CLAUSES: usize = 64;

        fn boolean_bounds(path: &mut Path, value: &Integer) {
            path.comparisons.push(compare(
                BinaryOperator::GreaterOrEqual,
                value.clone(),
                number(0),
            ));
            path.comparisons.push(compare(
                BinaryOperator::LessOrEqual,
                value.clone(),
                number(1),
            ));
        }

        fn clauses(value: &Boolean, holds: bool) -> Option<Vec<Clause>> {
            match value {
                Boolean::Variable(value) => Some(vec![vec![compare(
                    BinaryOperator::Equal,
                    value.clone(),
                    number(i128::from(holds)),
                )]]),
                Boolean::Constant(value) => Some(if *value == holds {
                    Vec::new()
                } else {
                    vec![Vec::new()]
                }),
                Boolean::Comparison(comparison) => Some(vec![vec![compare(
                    if holds {
                        comparison.operator
                    } else {
                        comparison
                            .operator
                            .complement()
                            .unwrap_or(comparison.operator)
                    },
                    comparison.left.clone(),
                    comparison.right.clone(),
                )]]),
                Boolean::Not(value) => clauses(value, !holds),
                Boolean::And(left, right) | Boolean::Or(left, right) => {
                    let mut left = clauses(left, holds)?;
                    let right = clauses(right, holds)?;
                    if matches!(value, Boolean::And(..)) == holds {
                        left.extend(right);
                        return Some(left);
                    }
                    if left.len().saturating_mul(right.len()) > MAXIMUM_CLAUSES {
                        return None;
                    }
                    Some(
                        left.iter()
                            .flat_map(|first| {
                                right.iter().map(|second| {
                                    let mut clause = first.clone();
                                    clause.extend(second.iter().cloned());
                                    clause
                                })
                            })
                            .collect(),
                    )
                }
                Boolean::Call { .. } => None,
            }
        }

        fn constant_truth(comparison: &Comparison) -> Option<bool> {
            let (Integer::Integer(left), Integer::Integer(right)) =
                (&comparison.left, &comparison.right)
            else {
                return None;
            };
            Some(match comparison.operator {
                BinaryOperator::Equal => left == right,
                BinaryOperator::NotEqual => left != right,
                BinaryOperator::Less => left < right,
                BinaryOperator::LessOrEqual => left <= right,
                BinaryOperator::Greater => left > right,
                BinaryOperator::GreaterOrEqual => left >= right,
                _ => return None,
            })
        }

        fn comparison_atoms(comparison: &Comparison, atoms: &mut Vec<Integer>) {
            integer_atoms(&comparison.left, atoms);
            integer_atoms(&comparison.right, atoms);
        }

        pub(crate) struct Component {
            atoms: Vec<Integer>,
            comparisons: Vec<Comparison>,
            clauses: Vec<Clause>,
        }

        fn components(comparisons: &[Comparison], clauses: &[Clause]) -> Vec<Component> {
            let mut components: Vec<Component> = Vec::new();
            let mut place =
                |atoms: Vec<Integer>, comparison: Option<&Comparison>, clause: Option<&Clause>| {
                    let mut joined = Component {
                        atoms,
                        comparisons: comparison.cloned().into_iter().collect(),
                        clauses: clause.cloned().into_iter().collect(),
                    };
                    let mut position = 0;
                    while position < components.len() {
                        if components[position]
                            .atoms
                            .iter()
                            .any(|atom| joined.atoms.contains(atom))
                        {
                            let absorbed = components.swap_remove(position);
                            for atom in absorbed.atoms {
                                if !joined.atoms.contains(&atom) {
                                    joined.atoms.push(atom);
                                }
                            }
                            joined.comparisons.extend(absorbed.comparisons);
                            joined.clauses.extend(absorbed.clauses);
                        } else {
                            position += 1;
                        }
                    }
                    components.push(joined);
                };
            for comparison in comparisons {
                let mut atoms = Vec::new();
                comparison_atoms(comparison, &mut atoms);
                if !atoms.is_empty() {
                    place(atoms, Some(comparison), None);
                }
            }
            for clause in clauses {
                let mut atoms = Vec::new();
                for comparison in clause {
                    comparison_atoms(comparison, &mut atoms);
                }
                place(atoms, None, Some(clause));
            }
            components
        }

        fn declared_value(
            program: &dyn crate::CheckingInput,
            value: Value,
            reference: TypeReferenceHandle,
        ) -> Option<Value> {
            Some(match (readable_shape(program, reference)?, value) {
                (ValueShape::Integer(carrier), Value::Integer { value, .. }) => Value::Integer {
                    value,
                    carrier: Some(carrier),
                },
                (
                    ValueShape::Collection { element_type, .. },
                    Value::Collection {
                        identity, length, ..
                    },
                ) => Value::Collection {
                    identity,
                    length,
                    element: element_type,
                },
                (
                    ValueShape::Record(reference),
                    Value::Record {
                        identity,
                        projection,
                        ..
                    },
                ) => Value::Record {
                    identity,
                    reference,
                    projection,
                },
                (ValueShape::Boolean, Value::Boolean(value)) => Value::Boolean(value),
                _ => return None,
            })
        }

        fn readable_shape(
            program: &dyn crate::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> Option<ValueShape> {
            let mut reference = reference;
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
                    } if access.is_readable() => {
                        reference = *referee;
                    }
                    _ => return value_shape(program, reference),
                }
            }
            None
        }

        fn observed_carrier(value: Value, carrier: TypeReferenceHandle) -> Option<Value> {
            let Value::Integer { value, .. } = value else {
                return None;
            };
            Some(Value::Integer {
                value,
                carrier: Some(carrier),
            })
        }

        fn substitutions(
            program: &dyn crate::CheckingInput,
            formals: &[Value],
            arguments: &[Value],
        ) -> Option<Vec<(Integer, Integer)>> {
            let mut replacements = Vec::new();
            for (formal, argument) in formals.iter().zip(arguments) {
                match (formal, argument) {
                    (
                        Value::Integer { value: formal, .. },
                        Value::Integer {
                            value: argument, ..
                        },
                    ) => replacements.push((formal.clone(), argument.clone())),
                    (
                        Value::Collection {
                            identity: first,
                            length: first_length,
                            element: first_element,
                        },
                        Value::Collection {
                            identity: second,
                            length: second_length,
                            element: second_element,
                        },
                    ) if first == second
                        && first_length == second_length
                        && program.primitive_type_reference(*first_element)
                            == program.primitive_type_reference(*second_element) => {}
                    (Value::Record { .. }, Value::Record { .. }) if formal == argument => {}
                    _ => return None,
                }
            }
            Some(replacements)
        }
    }

    pub(crate) use crate::proof_contracts::arithmetic_domains::IntegerSet;
    use exploration::{Binding, ValueShape, integer_carrier, value_shape};
    use typed_trees::data::{DataDefinition, DataField, DataMember};
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
    use typed_trees::machine::Machine;
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

    #[derive(Clone)]
    pub struct PredicateBooleanSubject {
        pub expression: ExpressionHandle,
        pub projection: Vec<facts::PlaceSegment>,
    }

    pub enum PredicateCondition {
        Ordering(PredicateOrderingConsequence),
        Boolean {
            subject: PredicateBooleanSubject,
            holds: bool,
        },
    }

    pub struct PredicateOrderingConsequence {
        pub left: PredicateIntegerOperand,
        pub right: PredicateIntegerOperand,
        pub operator: BinaryOperator,
    }

    pub fn exact_predicate_ordering(
        program: &dyn crate::CheckingInput,
        owner: symbols::SymbolHandle,
        expression: ExpressionHandle,
    ) -> Option<PredicateOrderingConsequence> {
        let application = predicate_application(program, owner, expression)?;
        implication::exact_ordering(program, &application)
    }

    pub fn exact_predicate_conjunction(
        program: &dyn crate::CheckingInput,
        owner: symbols::SymbolHandle,
        expression: ExpressionHandle,
    ) -> Option<Vec<PredicateCondition>> {
        let application = predicate_application(program, owner, expression)?;
        implication::exact_conjunction(program, &application)
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

    fn predicate_boolean_subject(
        program: &dyn crate::CheckingInput,
        predicate: symbols::SymbolHandle,
        arguments: &[ExpressionHandle],
        expression: ExpressionHandle,
    ) -> Option<PredicateBooleanSubject> {
        let position = exploration::admitted_predicate_machine(program, predicate, false)?;
        let machine = &program.machines()[position];
        let entry = program.machine_states(machine).first()?;
        if program.state_parameters(entry).len() != arguments.len() {
            return None;
        }
        let (position, projection) =
            crate::immutable_entry_boolean_projection(program, predicate, expression)?;
        Some(PredicateBooleanSubject {
            expression: *arguments.get(position)?,
            projection,
        })
    }

    pub fn predicate_ordering_consequences(
        program: &dyn crate::CheckingInput,
        call_expression: ExpressionHandle,
    ) -> Vec<PredicateOrderingConsequence> {
        let Some(application) = predicate_call(program, call_expression) else {
            return Vec::new();
        };
        application_ordering_consequences(program, &application)
    }

    pub(crate) fn application_ordering_consequences(
        program: &dyn crate::CheckingInput,
        application: &PredicateApplication<'_>,
    ) -> Vec<PredicateOrderingConsequence> {
        let mut consequences = implication::ordering_consequences(program, application);
        if let Some(intervals) = interval_consequences(program, application) {
            for interval in intervals.into_iter().filter(|row| !row.extent) {
                for (bound, operator) in [
                    (interval.low, BinaryOperator::GreaterOrEqual),
                    (interval.high, BinaryOperator::LessOrEqual),
                ] {
                    consequences.push(PredicateOrderingConsequence {
                        left: PredicateIntegerOperand::Argument(PredicateIntegerSubject {
                            expression: interval.argument,
                            projection: Vec::new(),
                        }),
                        right: PredicateIntegerOperand::Integer(bound),
                        operator,
                    });
                }
            }
        }
        consequences
    }

    pub(crate) fn argument_element_consequences(
        program: &dyn crate::CheckingInput,
        application: &PredicateApplication<'_>,
    ) -> Vec<PredicateOrderingConsequence> {
        let Some(entry) = program
            .machine_states(&program.machines()[application.machine])
            .first()
        else {
            return Vec::new();
        };
        let Some(knowledge) = program
            .state_parameters(entry)
            .iter()
            .map(|parameter| unknown_knowledge(program, parameter.type_reference))
            .collect::<Option<Vec<_>>>()
        else {
            return Vec::new();
        };
        if knowledge.len() != application.arguments.len()
            || !knowledge
                .iter()
                .any(|value| matches!(value, ArgumentKnowledge::Array(_)))
        {
            return Vec::new();
        }
        let Some(unfolding) = unfold(program, application.machine, &knowledge) else {
            return Vec::new();
        };
        let mut consequences = Vec::new();
        for (position, (argument, knowledge)) in
            application.arguments.iter().zip(&knowledge).enumerate()
        {
            let ArgumentKnowledge::Array(_) = knowledge else {
                continue;
            };
            let Some(elements) = unfolding.when_true(position) else {
                continue;
            };
            for (index, element) in elements.iter().enumerate() {
                let Some((low, high)) = element.bounds() else {
                    continue;
                };
                for (bound, operator) in [
                    (low, BinaryOperator::GreaterOrEqual),
                    (high, BinaryOperator::LessOrEqual),
                ] {
                    consequences.push(PredicateOrderingConsequence {
                        left: PredicateIntegerOperand::Argument(PredicateIntegerSubject {
                            expression: *argument,
                            projection: vec![facts::PlaceSegment::FixedIndex { index }],
                        }),
                        right: PredicateIntegerOperand::Integer(bound),
                        operator,
                    });
                }
            }
        }
        consequences
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

    pub fn premises_prove_expression(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        goal: ExpressionHandle,
    ) -> bool {
        implication::proves(program, machine, goal)
    }
}

pub mod default_domains {
    pub(crate) mod assignment_windows {
        use crate::proof_contracts::default_domains::data_reads::range_gates_hold;
        use crate::proof_contracts::default_domains::place_queries::{
            data_definition_for_expression, data_has_case_requires_facts,
            declared_root_established, is_data_rooted, is_subplace, membership_field,
            place_spelling_covers, place_spellings_may_overlap, write_place_spelling,
        };
        use crate::proof_contracts::default_domains::state_flow::PlaceValuation;
        use crate::proof_contracts::default_domains::symbolic_values::{
            LiteralValue, expression_sequence_measures, expression_symbolic_value, fold,
            literal_value,
        };
        use crate::proof_contracts::default_domains::{
            DomainRequirement, InvariantWindow, TrackedPlace,
        };
        use diagnostics::Diagnostic;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::machine::Machine;
        use typed_trees::state::State;

        pub(crate) fn preserve_proven_establishment(
            tracked: &[TrackedPlace<'_>],
            established: &mut Vec<String>,
        ) {
            established.extend(
                tracked
                    .iter()
                    .filter(|place| place.established)
                    .map(|place| place.spelling.clone()),
            );
        }

        pub(crate) fn refuse_open_windows(
            tracked: &[TrackedPlace<'_>],
            inherited_windows: &[InvariantWindow],
            machine: &Machine,
            state: &State,
            statement_index: Option<usize>,
            consumption_point: &str,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            if !tracked.iter().any(TrackedPlace::any_window_open) && inherited_windows.is_empty() {
                return;
            }
            let point = match statement_index {
                Some(index) => format!(
                    "{consumption_point} at statement {} in machine `{}` state `{}`",
                    index + 1,
                    machine.name,
                    state.name
                ),
                None => format!(
                    "{consumption_point} from machine `{}` state `{}`",
                    machine.name, state.name
                ),
            };
            for place in tracked.iter().filter(|place| place.any_window_open()) {
                let mut diagnostic = Diagnostic::error(format!(
                    "cannot complete {point}: this write left the `requires` facts of `{}` (data `{}`) unproved; restore them first",
                    place.spelling, place.definition.name,
                ));
                if let Some(opening) = place.opening_write {
                    diagnostic = diagnostic.with_source_span(opening);
                }
                diagnostics.push(diagnostic);
            }
            for (spelling, data_name, _) in inherited_windows {
                if tracked.iter().any(|place| place.spelling == *spelling) {
                    continue;
                }
                diagnostics.push(Diagnostic::error(format!(
            "cannot complete {point}: an earlier state left the `requires` facts of `{spelling}` (data `{data_name}`) unproved; restore them first"
        )));
            }
        }

        pub(crate) fn handle_assignment<'program>(
            program: &'program dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
            statement: &typed_trees::statement::StatementNode,
            call_frames: Option<&crate::CallFrameResolver<'_>>,
            tracked: &mut Vec<TrackedPlace<'program>>,
            entry_valuations: &[PlaceValuation],
            inherited_windows: &[InvariantWindow],
            poisoned_all: bool,
            poisoned_paths: &mut Vec<String>,
            born_zero: bool,
            statement_index: usize,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let Some(assignment) = statement.as_assignment() else {
                return;
            };
            let resident_requirement = match crate::atomic_assignment_carrier(program, assignment) {
                Some(_) => super::requires_fact_intervals::stored_field_requires_interval(
                    program,
                    machine,
                    state,
                    assignment.target,
                )
                .map(|source_interval| {
                    super::requires_fact_intervals::ResidentRequirement {
                        source_interval,
                        reinterpreted: false,
                    }
                }),
                None => call_frames.and_then(|frames| {
                    super::requires_fact_intervals::reinterpreted_view_store(
                        program,
                        machine,
                        state,
                        frames,
                        statement,
                        statement_index,
                    )
                }),
            };
            let target = assignment.target;
            let value = assignment.value;
            let data_binding = program
                .state_data_binding(machine, state)
                .map(|name| name.as_str().to_owned());
            let data_binding = data_binding.as_deref();
            if let Some(spelling) = write_place_spelling(program, target)
                && crate::value_custody::places::declared_place_type_raw(
                    program,
                    machine,
                    Some(state),
                    target,
                )
                .is_none_or(|reference| crate::reference_shell(program, reference).is_none())
                && let Some(definition) =
                    data_definition_for_expression(program, machine, Some(state), target)
            {
                if spelling.contains("[*]") {
                    if !poisoned_paths.contains(&spelling) {
                        poisoned_paths.push(spelling.clone());
                    }
                    for place in tracked.iter_mut() {
                        if place_spellings_may_overlap(&spelling, &place.spelling) {
                            place.fields.clear();
                            place.established_fields.clear();
                            place.symbols.clear();
                            place.measures.clear();
                            place.elements.clear();
                            place.born_zero = false;
                            place.active_case = None;
                        }
                    }
                } else {
                    let literal = match program.expression_table.expression(value) {
                        ExpressionNode::StructLiteral(literal) => Some(literal),
                        _ => None,
                    };
                    let literal_fields = literal
                        .map(|literal| program.expression_table.struct_fields(literal.fields))
                        .unwrap_or(&[]);
                    let fields = literal_fields
                        .iter()
                        .map(|field| {
                            (
                                field.name.as_str().to_string(),
                                literal_value(program, field.value),
                            )
                        })
                        .collect();
                    let symbols = literal_fields
                        .iter()
                        .filter_map(|field| {
                            expression_symbolic_value(program, field.value)
                                .map(|symbol| (field.name.as_str().to_string(), symbol))
                        })
                        .collect();
                    let measures = literal_fields
                        .iter()
                        .filter_map(|field| {
                            expression_sequence_measures(program, field.value).map(
                                |(length, capacity)| {
                                    (field.name.as_str().to_string(), length, capacity)
                                },
                            )
                        })
                        .collect();
                    tracked.retain(|place| {
                        place.spelling != spelling && !is_subplace(&place.spelling, &spelling)
                    });
                    let place_born_zero = born_zero && is_data_rooted(data_binding, &spelling);
                    tracked.push(TrackedPlace {
                spelling,
                definition,
                fields,
                established_fields: super::established_common_fields(program, definition),
                pending_requirements: Vec::new(),
                symbols,
                measures,
                established: true,
                born_zero: place_born_zero,
                window_open: false,
                opening_write: None,
                elements: Vec::new(),
                element_windows: Vec::new(),
                alternatives: Vec::new(),
                active_case: crate::value_custody::places::declared_place_type_raw(
                    program,
                    machine,
                    Some(state),
                    target,
                )
                .and_then(|reference| {
                    crate::proof_contracts::contract_entailment::completed_value_case_at_occurrence(
                        program,
                        machine,
                        state,
                        statement_index,
                        value,
                        reference,
                    )
                }),
            });
                }
            }

            let origins =
                call_frames.map(|frames| frames.assignment_storage_origins(machine, statement));
            // The written element is named by the storage it reaches, not by how the
            // assignment spells it: `v[0] = x` with `v = &mut b.cells` updates `b`'s
            // tracked elements exactly as `b.cells[0] = x` does.
            let element_destination = match &origins {
                Some(Ok(origins)) => match origins.destinations.as_slice() {
                    [origin] => origin
                        .write_storage_coordinate(program, state)
                        .ok()
                        .map(|coordinate| coordinate.place),
                    _ => None,
                },
                _ => None,
            }
            .or_else(|| crate::canonical_place_from_expression(program, target));
            let element_store = element_destination.and_then(|destination| {
                super::element_stores::handle_element_store(
                    program,
                    machine,
                    state,
                    &destination,
                    target,
                    value,
                    tracked,
                    entry_valuations,
                    inherited_windows,
                    poisoned_all,
                    poisoned_paths,
                    born_zero,
                    statement_index,
                )
            });

            let Some(origins) = origins else {
                return;
            };
            let origins = match origins {
                Ok(origins) => origins,
                Err(reason) => {
                    diagnostics.push(
                Diagnostic::error(format!(
                    "cannot establish assignment invariants: performed write has unknown destination origins in machine `{}` state `{}` at statement {statement_index}: {reason}",
                    machine.name.as_str(), state.name.as_str(),
                ))
                .with_source_span(program.expression_table.source_span(target)),
            );
                    return;
                }
            };
            if crate::proof_contracts::contract_entailment::struct_trace() {
                eprintln!(
                    "DEFAULT assignment-origins machine={} destinations={:?}",
                    machine.name, origins.destinations,
                );
            }
            let definite = origins.destinations.len() == 1;
            for origin in origins.destinations {
                let coordinate = match origin.write_storage_coordinate(program, state) {
                    Ok(coordinate) => coordinate,
                    Err(reason) => {
                        diagnostics.push(Diagnostic::error(format!(
                    "cannot establish assignment invariants: source storage interpretation is unavailable: {reason} (machine `{}` state `{}` statement {statement_index}, source `{}`, path {:?}, view {:?})",
                    machine.name, state.name, program.symbols.name(origin.root), origin.segments,
                    origin.representation.as_ref().map(|(reference, path)|
                        (program.display_type_reference(*reference), path)),
                )));
                        continue;
                    }
                };
                let destination = coordinate.place;
                let preserves_scalar_value = definite && coordinate.preserves_scalar_value;
                let stores = match field_stores(
                    program,
                    state,
                    statement_index,
                    destination,
                    preserves_scalar_value,
                    definite && coordinate.replaces_place,
                ) {
                    Ok(stores) => stores,
                    Err(reason) => {
                        diagnostics.push(Diagnostic::error(format!(
                            "cannot establish assignment invariants: {reason}"
                        )));
                        continue;
                    }
                };
                for store in stores {
                    if element_store.as_ref() == Some(&store.field_place) {
                        continue;
                    }
                    handle_field_store(
                        program,
                        machine,
                        state,
                        store,
                        value,
                        tracked,
                        entry_valuations,
                        inherited_windows,
                        poisoned_all,
                        poisoned_paths,
                        born_zero,
                        statement_index,
                        data_binding,
                        resident_requirement,
                    );
                }
            }
        }

        pub(crate) struct FieldStore<'program> {
            destination: crate::CanonicalPlace,
            receiver: crate::CanonicalPlace,
            field_place: crate::CanonicalPlace,
            receiver_type: typed_trees::types::TypeReferenceHandle,
            definition: &'program typed_trees::data::DataDefinition,
            field: &'program typed_trees::data::DataField,
            preserves_scalar_value: bool,
            replaces_place: bool,
        }

        fn field_stores<'program>(
            program: &'program dyn crate::CheckingInput,
            state: &State,
            statement_index: usize,
            destination: crate::CanonicalPlace,
            preserves_scalar_value: bool,
            replaces_place: bool,
        ) -> Result<Vec<FieldStore<'program>>, &'static str> {
            let mut stores = Vec::new();
            if !replaces_place {
                append_enclosed_field_stores(
                    program,
                    state,
                    statement_index,
                    &destination,
                    &mut stores,
                )?;
            }
            for index in (0..destination.segments.len()).rev() {
                let facts::PlaceSegment::Field { symbol } = destination.segments[index] else {
                    continue;
                };
                if !symbol.is_valid() {
                    return Err("source field identity is unavailable");
                }
                let mut receiver = destination.clone();
                receiver.segments.truncate(index);
                if matches!(
                    receiver.segments.last(),
                    Some(facts::PlaceSegment::Case { .. })
                ) {
                    receiver.segments.pop();
                }
                let receiver_type = crate::value_custody::places::canonical_place_type_reference(
                    program,
                    state.symbol,
                    statement_index,
                    &receiver,
                )
                .ok_or("source parent type is unavailable")?;
                let definition =
                    crate::value_custody::places::data_definition_for_type(program, receiver_type)
                        .ok_or("source parent declaration is unavailable")?;
                let parent = program.symbols.get(symbol).parent;
                let case_name = (program.symbols.get(parent).kind == symbols::SymbolKind::Variant)
                    .then(|| program.symbols.name(parent));
                let field = crate::value_custody::places::exact_data_member_field(
                    program,
                    definition,
                    symbol,
                    program.symbols.name(symbol),
                    case_name,
                )
                .ok_or("source field does not belong to its parent declaration")?;
                let mut field_place = destination.clone();
                field_place.segments.truncate(index + 1);
                stores.push(FieldStore {
                    destination: destination.clone(),
                    receiver,
                    receiver_type,
                    field_place,
                    definition,
                    field,
                    preserves_scalar_value,
                    replaces_place,
                });
            }
            Ok(stores)
        }

        fn append_enclosed_field_stores<'program>(
            program: &'program dyn crate::CheckingInput,
            state: &State,
            statement_index: usize,
            destination: &crate::CanonicalPlace,
            stores: &mut Vec<FieldStore<'program>>,
        ) -> Result<(), &'static str> {
            const MAX_OWNER_VISITS: usize = 16_384;
            const MAX_OWNER_DEPTH: usize = 256;
            use typed_trees::types::TypeReferenceNode;
            let root_type = crate::value_custody::places::canonical_place_type_reference(
                program,
                state.symbol,
                statement_index,
                destination,
            )
            .ok_or("partial write enclosure has no declared type")?;
            let root_type = crate::value_custody::places::assignment_value_type(program, root_type);
            let mut pending = vec![(destination.clone(), root_type, 0usize)];
            let mut visits = 0usize;
            while let Some((receiver, receiver_type, depth)) = pending.pop() {
                visits += 1;
                if visits > MAX_OWNER_VISITS || depth > MAX_OWNER_DEPTH {
                    return Err(
                        "partial write owner frontier exceeds the checked traversal budget",
                    );
                }
                if crate::reference_shell(program, receiver_type).is_some()
                    || program.primitive_type_reference(receiver_type).is_some()
                {
                    continue;
                }
                let reference = program
                    .type_reference_table
                    .unconstrained_type_reference(receiver_type);
                let element = match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::FixedArray { element_type, .. } => {
                        let extent = program
                            .closed_fixed_array_length(reference)
                            .ok_or("partial write array has no closed extent")?;
                        if extent == 0 {
                            continue;
                        }
                        Some(*element_type)
                    }
                    TypeReferenceNode::Slice { element_type } => Some(*element_type),
                    _ => None,
                };
                if let Some(element) = element {
                    let mut child = receiver;
                    child.segments.push(facts::PlaceSegment::Index {
                        expression: ExpressionHandle::invalid(),
                    });
                    pending.push((child, element, depth + 1));
                    continue;
                }
                let definition =
                    crate::value_custody::places::data_definition_for_type(program, receiver_type)
                        .ok_or("partial write owner has no complete inline declaration")?;
                if program
                    .data_members(definition)
                    .iter()
                    .any(|member| member.as_variant().is_some())
                {
                    return Err("partial write enclosure has no exact active-case frontier");
                }
                for field in program.live_data_fields(program.data_members(definition)) {
                    let mut field_place = receiver.clone();
                    crate::push_field_place_segments(
                        program,
                        &mut field_place.segments,
                        field.symbol,
                    );
                    pending.push((field_place.clone(), field.type_reference, depth + 1));
                    stores.push(FieldStore {
                        destination: destination.clone(),
                        receiver: receiver.clone(),
                        receiver_type,
                        field_place,
                        definition,
                        field,
                        preserves_scalar_value: false,
                        replaces_place: false,
                    });
                }
            }
            stores.reverse();
            Ok(())
        }

        pub(crate) fn invariant_place_spelling(
            program: &dyn crate::CheckingInput,
            place: &crate::CanonicalPlace,
        ) -> Option<String> {
            use std::fmt::Write;
            let root = place.root.as_symbol()?;
            let mut spelling = program.symbols.name(root).to_owned();
            for segment in &place.segments {
                match segment {
                    facts::PlaceSegment::Field { symbol } => {
                        spelling.push('.');
                        spelling.push_str(program.symbols.name(*symbol));
                    }
                    facts::PlaceSegment::Case { .. } => {}
                    facts::PlaceSegment::FixedIndex { index } => {
                        write!(&mut spelling, "[{index}]").ok()?;
                    }
                    facts::PlaceSegment::Index { .. } => spelling.push_str("[*]"),
                    _ => return None,
                }
            }
            Some(spelling)
        }

        #[allow(clippy::too_many_arguments)]
        fn handle_field_store<'program>(
            program: &'program dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
            store: FieldStore<'program>,
            value: ExpressionHandle,
            tracked: &mut Vec<TrackedPlace<'program>>,
            entry_valuations: &[PlaceValuation],
            inherited_windows: &[InvariantWindow],
            poisoned_all: bool,
            poisoned_paths: &[String],
            born_zero: bool,
            statement_index: usize,
            data_binding: Option<&str>,
            resident_requirement: Option<super::requires_fact_intervals::ResidentRequirement>,
        ) {
            if store.replaces_place
                && let Some(spelling) = invariant_place_spelling(program, &store.destination)
                && !spelling.contains("[*]")
            {
                tracked.retain(|place| !is_subplace(&place.spelling, &spelling));
            }
            let definition = store.definition;
            let Some(receiver_spelling) = invariant_place_spelling(program, &store.receiver) else {
                return;
            };
            if definition.requires_facts.is_empty()
                && !data_has_case_requires_facts(program, definition)
                && !crate::value_custody::data::data_requires_establishment(program, definition)
            {
                return;
            }
            let whole_field = store.replaces_place && store.destination == store.field_place;
            let field_name = store.field.name.to_string();
            let bool_byte = whole_field
                && matches!(
                    program.primitive_type_reference(store.field.type_reference),
                    Some(
                        typed_trees::types::PrimitiveType::U8
                            | typed_trees::types::PrimitiveType::I8
                    )
                )
                && matches!(
                    program.expression_table.expression(value),
                    ExpressionNode::Boolean(_)
                );
            let written = ((store.preserves_scalar_value && whole_field) || bool_byte)
                .then(|| literal_value(program, value))
                .flatten();

            if receiver_spelling.contains("[*]") {
                for sibling in tracked.iter_mut() {
                    if place_spelling_covers(&receiver_spelling, &sibling.spelling) {
                        sibling.established_fields.retain(|symbol| {
                            !program
                                .data_members(sibling.definition)
                                .iter()
                                .any(|member| {
                                    member.as_field().is_some_and(|field| {
                                        field.symbol == *symbol && field.name.as_str() == field_name
                                    })
                                })
                        });
                        sibling.fields.retain(|(name, _)| *name != field_name);
                        sibling.fields.push((field_name.clone(), None));
                        sibling.symbols.retain(|(name, _)| *name != field_name);
                        sibling.measures.retain(|(name, _, _)| *name != field_name);
                        if !store.replaces_place {
                            sibling.measures.push((field_name.clone(), None, None));
                            sibling
                                .elements
                                .retain(|(symbol, _)| *symbol != store.field.symbol);
                        }
                    }
                }
            }

            let position = tracked_receiver(
                program,
                state,
                store
                    .receiver
                    .root
                    .as_symbol()
                    .unwrap_or_else(symbols::SymbolHandle::invalid),
                tracked,
                receiver_spelling,
                definition,
                entry_valuations,
                inherited_windows,
                poisoned_all,
                poisoned_paths,
                born_zero,
                data_binding,
            );
            let region_spelling = tracked[position].spelling.as_str();
            let region_was_established = tracked[position].established
                && !tracked.iter().any(|place| {
                    place.any_window_open()
                        && (place_spelling_covers(region_spelling, &place.spelling)
                            || place_spelling_covers(&place.spelling, region_spelling))
                })
                && !inherited_windows.iter().any(|(spelling, _, _)| {
                    place_spelling_covers(region_spelling, spelling)
                        || place_spelling_covers(spelling, region_spelling)
                });
            let place = &mut tracked[position];
            place.fields.retain(|(name, _)| *name != field_name);
            place.fields.push((field_name.clone(), written));
            place.symbols.retain(|(name, _)| *name != field_name);
            if let Some(symbol) = (store.preserves_scalar_value && whole_field)
                .then(|| expression_symbolic_value(program, value))
                .flatten()
            {
                place.symbols.push((field_name.clone(), symbol));
            }
            place.measures.retain(|(name, _, _)| *name != field_name);
            if !store.replaces_place {
                place.measures.push((field_name.clone(), None, None));
                place
                    .elements
                    .retain(|(symbol, _)| *symbol != store.field.symbol);
            }
            if let Some((length, capacity)) = (store.preserves_scalar_value && whole_field)
                .then(|| expression_sequence_measures(program, value))
                .flatten()
            {
                place.measures.push((field_name.clone(), length, capacity));
            }
            if !place.alternatives.is_empty() {
                let written = (store.preserves_scalar_value && whole_field)
                    .then(|| literal_value(program, value))
                    .flatten();
                let definition = place.definition;
                let case = place.active_case;
                let holds = place.alternatives.iter().all(|fields| {
                    let mut fields = fields.clone();
                    if let Some(slot) = fields.iter_mut().find(|(name, _)| name == &field_name) {
                        slot.1 = written;
                    } else {
                        fields.push((field_name.clone(), written));
                    }
                    super::symbolic_values::predicate_holds(program, definition, &fields, case)
                });
                if holds {
                    for fields in &mut place.alternatives {
                        if let Some(slot) = fields.iter_mut().find(|(name, _)| name == &field_name)
                        {
                            slot.1 = written;
                        } else {
                            fields.push((field_name.clone(), written));
                        }
                    }
                    place.established = true;
                    place.window_open = false;
                    place.pending_requirements.clear();
                    if !place.any_window_open() {
                        place.opening_write = None;
                    }
                    return;
                }
                place.window_open = place.established || place.window_open;
            }

            let selected_field = Some(store.field).filter(|field| {
                let parent = program.symbols.get(field.symbol).parent;
                program.symbols.get(parent).kind != symbols::SymbolKind::Variant
                    || place.active_case.is_none_or(|active| active == parent)
            });
            let field_type = selected_field.map(|field| field.type_reference);
            if let Some(field) = selected_field
                && whole_field
                && !place.spelling.contains("[*]")
                && !place.established_fields.contains(&field.symbol)
            {
                place.established_fields.push(field.symbol);
            }
            let resident_field = resident_requirement
                .filter(|checked| {
                    whole_field
                        && if checked.reinterpreted {
                            super::requires_fact_intervals::single_field_requires_interval(
                                program,
                                place.definition,
                                store.field,
                            )
                            .is_some_and(|lender| lender.contains(checked.source_interval))
                        } else {
                            store.preserves_scalar_value
                                && super::requires_fact_intervals::atomic_field_interval(
                                    program,
                                    place.definition,
                                    store.field,
                                ) == Some(checked.source_interval)
                        }
                })
                .map(|_| store.field.symbol);
            let requirements = domain_requirements(program, place.definition, place.active_case);
            if crate::proof_contracts::contract_entailment::struct_trace() {
                eprintln!(
                    "DEFAULT owner-store machine={} data={} receiver={:?} destination={:?} field={:?} requirements={}",
                    machine.name,
                    place.definition.name,
                    store.receiver,
                    store.destination,
                    store.field.symbol,
                    requirements.len(),
                );
            }
            if !requirements.iter().any(|requirement| {
                super::place_queries::fact_names_field(
                    program,
                    place.definition,
                    &requirement.fact,
                    &field_name,
                )
            }) && !field_type.is_some_and(|field_type| {
                crate::value_custody::data::type_requires_establishment(program, field_type)
            }) {
                return;
            }
            let valuation: Vec<(&str, Option<LiteralValue>)> = place
                .fields
                .iter()
                .map(|(name, value)| (name.as_str(), *value))
                .collect();
            let mut all_hold = region_was_established || range_gates_hold(program, place);
            let definition = place.definition;
            let written_field = selected_field.map(|field| field.symbol);
            place.pending_requirements.retain(|requirement| {
                !requirement.case.is_valid()
                    || place
                        .active_case
                        .is_none_or(|active| active == requirement.case)
            });
            let requirements = requirements
        .into_iter()
        .filter(|requirement| {
            if !place.established || place.pending_requirements.contains(requirement) {
                return true;
            }
            if !super::place_queries::fact_names_field(
                program,
                definition,
                &requirement.fact,
                &field_name,
            ) {
                return false;
            }
            let disjoint = store
                .destination
                .segments
                .strip_prefix(store.receiver.segments.as_slice())
                .is_some_and(|path| {
                    super::place_queries::fact_is_disjoint_from_store(
                        program,
                        definition,
                        &requirement.fact,
                        path,
                    )
                });
            if crate::proof_contracts::contract_entailment::struct_trace() {
                eprintln!(
                    "DEFAULT owner-requirement machine={} data={} fact={:?} disjoint={disjoint}",
                    machine.name, definition.name, requirement.fact,
                );
            }
            if disjoint {
                return false;
            }
            let Some(expression) = requirement.fact.as_expression() else {
                return true;
            };
            written_field.is_none_or(|written| {
                crate::proof_contracts::collection_predicates::data_fact_applies_to_field(
                    program,
                    definition,
                    *expression,
                    written,
                ) != Some(false)
            })
        })
        .collect::<Vec<_>>();
            let view_primitive =
                crate::expression_result_type_reference(program, machine, state, value)
                    .and_then(|reference| program.primitive_type_reference(reference));
            let source_primitive = program.primitive_type_reference(store.field.type_reference);
            let encoding_preserves = view_primitive.zip(source_primitive).is_some_and(|(view, source)| {
        crate::proof_contracts::contract_entailment::structural_judgment::stored_encoding_preserves_field(
            program,
            &store.destination,
            view,
            source,
            definition,
            store.field.symbol,
        )
    });
            for requirement in requirements {
                let fact = &requirement.fact;
                let resident_proven = !requirement.case.is_valid()
                    && resident_field.is_some_and(|field| {
                        super::place_queries::data_predicate_fields(program, place.definition, fact)
                            == [field]
                    });
                let encoding_holds = encoding_preserves
                    && !requirement.case.is_valid()
                    && written_field.is_some_and(|written| {
                        super::place_queries::data_predicate_fields(program, place.definition, fact)
                            == [written]
                    });
                let proven = resident_proven
                    || encoding_holds
                    || match fact {
                        typed_trees::domain::ProofFact::Expression(expression) => {
                            match fold(
                                program,
                                place.definition,
                                &valuation,
                                &place.symbols,
                                &place.measures,
                                place.born_zero,
                                place.active_case,
                                *expression,
                            ) {
                                Some(value) => value != 0,
                                None => written_field.is_some_and(|written| {
                                    super::post_store_predicates::post_store_predicate_holds(
                                        program,
                                        machine,
                                        state,
                                        statement_index,
                                        &store.receiver,
                                        store.receiver_type,
                                        store.preserves_scalar_value,
                                        &store.destination,
                                        place,
                                        written,
                                        value,
                                        *expression,
                                    )
                                }),
                            }
                        }
                        typed_trees::domain::ProofFact::Membership(membership) => {
                            let mentioned =
                                membership_field(program, place.definition, membership.value);
                            if mentioned.is_some_and(|field| Some(field.symbol) == written_field) {
                                whole_field
                            && store.preserves_scalar_value
                            && crate::proof_contracts::proof_facts::string_literal_grants_domain(
                                program,
                                value,
                                membership.domain_symbol,
                            )
                            } else if mentioned.is_none() {
                                crate::proof_contracts::proof_facts::string_literal_grants_domain(
                                    program,
                                    membership.value,
                                    membership.domain_symbol,
                                )
                            } else {
                                place.established && !place.any_window_open()
                            }
                        }
                        typed_trees::domain::ProofFact::Proposition(_) => false,
                    };
                if crate::proof_contracts::contract_entailment::struct_trace() {
                    eprintln!(
                        "DEFAULT owner-proof machine={} data={} fact={fact:?} proven={proven}",
                        machine.name, definition.name,
                    );
                }
                if proven {
                    place
                        .pending_requirements
                        .retain(|pending| *pending != requirement);
                } else if !place.pending_requirements.contains(&requirement) {
                    place.pending_requirements.push(requirement);
                }
            }
            all_hold &= place.pending_requirements.is_empty();
            if all_hold && (!place.spelling.contains("[*]") || region_was_established) {
                place.established = true;
                place.window_open = false;
            } else {
                place.window_open = place.established || place.window_open;
            }
            if place.any_window_open() {
                place
                    .opening_write
                    .get_or_insert(program.expression_table.source_span(value));
            } else {
                place.opening_write = None;
            }
        }

        pub(crate) fn tracked_receiver<'program>(
            program: &'program dyn crate::CheckingInput,
            state: &State,
            receiver_root: symbols::SymbolHandle,
            tracked: &mut Vec<TrackedPlace<'program>>,
            receiver_spelling: String,
            definition: &'program typed_trees::data::DataDefinition,
            entry_valuations: &[PlaceValuation],
            inherited_windows: &[InvariantWindow],
            poisoned_all: bool,
            poisoned_paths: &[String],
            born_zero: bool,
            data_binding: Option<&str>,
        ) -> usize {
            if let Some(position) = tracked
                .iter()
                .position(|place| place.spelling == receiver_spelling)
            {
                return position;
            }
            let poisoned = poisoned_all
                || poisoned_paths
                    .iter()
                    .any(|written| place_spellings_may_overlap(&receiver_spelling, written));
            let (seeded_fields, seeded_case) = if poisoned {
                (Vec::new(), None)
            } else {
                entry_valuations
                    .iter()
                    .find(|(name, _, _)| *name == receiver_spelling)
                    .map(|(_, fields, active_case)| (fields.clone(), *active_case))
                    .unwrap_or_default()
            };
            let data_rooted = is_data_rooted(data_binding, &receiver_spelling);
            let established = tracked.iter().any(|place| {
                place.established && place_spelling_covers(&place.spelling, &receiver_spelling)
            }) || declared_root_established(program, state, receiver_root)
                || (data_rooted
                    && !crate::value_custody::data::data_requires_establishment(
                        program, definition,
                    ));
            let pending_requirements = if !established
                || inherited_windows.iter().any(|(spelling, _, _)| {
                    place_spelling_covers(&receiver_spelling, spelling)
                        || place_spelling_covers(spelling, &receiver_spelling)
                }) {
                domain_requirements(program, definition, seeded_case)
            } else {
                Vec::new()
            };
            tracked.push(TrackedPlace {
                spelling: receiver_spelling,
                definition,
                fields: seeded_fields,
                established_fields: if established {
                    super::established_common_fields(program, definition)
                } else {
                    Vec::new()
                },
                pending_requirements,
                symbols: Vec::new(),
                measures: Vec::new(),
                established,
                born_zero: !poisoned
                    && ((born_zero && data_rooted)
                        || (!established
                            && program
                                .state_local_declaration(state, receiver_root)
                                .is_some_and(|(_, local)| !local.initial_value.is_valid()))),
                window_open: false,
                opening_write: None,
                elements: Vec::new(),
                element_windows: Vec::new(),
                alternatives: Vec::new(),
                active_case: seeded_case,
            });
            tracked.len() - 1
        }

        fn domain_requirements(
            program: &dyn crate::CheckingInput,
            definition: &typed_trees::data::DataDefinition,
            active_case: Option<symbols::SymbolHandle>,
        ) -> Vec<DomainRequirement> {
            let mut requirements = program
                .conjunctive_proof_facts(definition.requires_facts)
                .map(|fact| DomainRequirement {
                    case: symbols::SymbolHandle::invalid(),
                    fact,
                })
                .collect::<Vec<_>>();
            for member in program.data_members(definition) {
                let Some(case) = member.as_variant() else {
                    continue;
                };
                if active_case.is_some_and(|active| active != case.symbol) {
                    continue;
                }
                requirements.extend(program.conjunctive_proof_facts(case.requires_facts).map(
                    |fact| DomainRequirement {
                        case: case.symbol,
                        fact,
                    },
                ));
            }
            requirements
        }
    }
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

        pub(crate) fn collect_call_summaries(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            state_to_machine: &StateToMachine,
            summaries: &SymbolKeyMap<SymbolHandle, Vec<String>>,
            call_established: &mut Vec<String>,
        ) {
            if !expression.is_valid() {
                return;
            }
            macro_rules! visit_child {
                ($child:expr) => {
                    collect_call_summaries(
                        program,
                        $child,
                        state_to_machine,
                        summaries,
                        call_established,
                    );
                };
            }
            match program.expression_table.expression(expression) {
                ExpressionNode::Call(call) => {
                    let target_machine = state_to_machine
                        .get(&call.target_symbol)
                        .copied()
                        .unwrap_or_else(SymbolHandle::invalid);
                    if let Some(established) = summaries.get(&target_machine) {
                        call_established.extend(established.iter().cloned());
                    }
                    visit_child!(call.receiver);
                    for argument in program.expression_table.expression_handles(call.arguments) {
                        visit_child!(*argument);
                    }
                }
                ExpressionNode::Binary(binary) => {
                    visit_child!(binary.left);
                    visit_child!(binary.right);
                }
                ExpressionNode::Member(member) => {
                    visit_child!(member.receiver);
                }
                ExpressionNode::Borrow(inner) => {
                    visit_child!(inner.target);
                }
                ExpressionNode::Indexed(indexed) => {
                    visit_child!(indexed.collection);
                    visit_child!(indexed.index);
                }
                ExpressionNode::Cast(cast) => {
                    visit_child!(cast.value);
                }
                ExpressionNode::Unary(unary) => {
                    visit_child!(unary.operand);
                }
                ExpressionNode::Atomic(atomic) => {
                    visit_child!(atomic.value);
                    visit_child!(atomic.result);
                }
                ExpressionNode::Range(range) => {
                    visit_child!(range.start);
                    visit_child!(range.end);
                }
                ExpressionNode::ArrayLiteral(elements) => {
                    for element in program.expression_table.expression_handles(*elements) {
                        visit_child!(*element);
                    }
                }
                ExpressionNode::StructLiteral(literal) => {
                    for field in program.expression_table.struct_fields(literal.fields) {
                        visit_child!(field.value);
                    }
                }
                ExpressionNode::Match(matched) => {
                    visit_child!(matched.subject);
                    for arm in program.expression_table.match_arms(matched.arms) {
                        visit_child!(arm.value);
                        for pattern in program.expression_table.match_pattern_operands(arm.pattern)
                        {
                            visit_child!(pattern);
                        }
                    }
                }
                _ => {}
            }
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
    pub(crate) mod data_reads {
        use crate::CheckingInput;
        use crate::frozen_program::expression_table;
        use crate::proof_contracts::default_domains::place_queries::{
            data_definition_for_expression, declared_root_established, expression_root_symbol,
            place_spelling_covers, self_place_spelling, transition_evaluated_expressions,
            write_place_spelling, write_target_index_expressions,
        };
        use crate::proof_contracts::default_domains::{InvariantWindow, TrackedPlace};
        use diagnostics::Diagnostic;
        use language_core::place_label;
        use symbols::JoinedPath;
        use typed_trees::data::DataDefinition;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use typed_trees::statement::StatementNode;
        use typed_trees::types::TypeReferenceNode;

        pub(crate) fn scan_statement_reads(
            program: &dyn CheckingInput,
            machine: &Machine,
            state: &State,
            statement: &StatementNode,
            statement_ordinal: usize,
            tracked: &[TrackedPlace<'_>],
            entry_established: &[String],
            call_established: &[String],
            inherited_windows: &[InvariantWindow],
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let diagnostic_start = diagnostics.len();
            let mut reads: Vec<ExpressionHandle> = Vec::new();
            match statement {
                StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => {}
                StatementNode::Assignment(assignment) => reads.push(assignment.value),
                StatementNode::Expression(expression) => reads.push(*expression),
                StatementNode::LocalData(local) => {
                    if local.initial_value.is_valid() {
                        reads.push(local.initial_value);
                    }
                }
                StatementNode::Call(call) => {
                    let receiver = program.statement_table.name_path_members(call.receiver);
                    let data_binding = program
                        .state_data_binding(machine, state)
                        .map(|name| name.as_str());
                    if receiver.len() > 1
                        && data_binding.is_some_and(|root| receiver[0].as_str() == root)
                        && let Some(definition) = machine
                            .attached_data
                            .as_ref()
                            .and_then(|attached| program.data_definition_by_name(attached))
                        && crate::value_custody::data::data_requires_establishment(
                            program, definition,
                        )
                    {
                        validate_data_read(
                            program,
                            state,
                            definition,
                            receiver[0].as_str(),
                            receiver[1].as_str(),
                            call.receiver_root_symbol,
                            tracked,
                            entry_established,
                            call_established,
                            inherited_windows,
                            diagnostics,
                        );
                    }
                    reads.extend(
                        program
                            .statement_table
                            .expression_handles(call.arguments)
                            .iter()
                            .copied(),
                    );
                }
                StatementNode::Transition(transition) => {
                    reads.extend(transition_evaluated_expressions(program, transition));
                }
            }
            if let Some(assignment) = statement.as_assignment() {
                reads.extend(write_target_index_expressions(program, assignment.target));
            }
            for read in reads {
                scan_expression_reads(
                    program,
                    machine,
                    state,
                    read,
                    tracked,
                    entry_established,
                    call_established,
                    inherited_windows,
                    diagnostics,
                );
            }
            for diagnostic in &mut diagnostics[diagnostic_start..] {
                diagnostic.message = format!(
                    "machine `{}` state `{}` statement {statement_ordinal}: {}",
                    machine.name, state.name, diagnostic.message,
                );
            }
        }

        fn scan_expression_reads(
            program: &dyn CheckingInput,
            machine: &Machine,
            state: &State,
            expression: ExpressionHandle,
            tracked: &[TrackedPlace<'_>],
            entry_established: &[String],
            call_established: &[String],
            inherited_windows: &[InvariantWindow],
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            scan_expression_use(
                program,
                machine,
                state,
                expression,
                tracked,
                entry_established,
                call_established,
                inherited_windows,
                true,
                diagnostics,
            );
        }

        fn scan_expression_use(
            program: &dyn CheckingInput,
            machine: &Machine,
            state: &State,
            expression: ExpressionHandle,
            tracked: &[TrackedPlace<'_>],
            entry_established: &[String],
            call_established: &[String],
            inherited_windows: &[InvariantWindow],
            consumes_value: bool,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            macro_rules! read_child {
                ($expression:expr $(,)?) => {
                    scan_expression_reads(
                        program,
                        machine,
                        state,
                        $expression,
                        tracked,
                        entry_established,
                        call_established,
                        inherited_windows,
                        diagnostics,
                    )
                };
            }
            macro_rules! validate_read {
                ($definition:expr, $receiver:expr, $member:expr, $root:expr $(,)?) => {
                    validate_data_read(
                        program,
                        state,
                        $definition,
                        $receiver,
                        $member,
                        $root,
                        tracked,
                        entry_established,
                        call_established,
                        inherited_windows,
                        diagnostics,
                    )
                };
            }
            if !expression.is_valid() {
                return;
            }
            validate_qualified_place_read(
                program,
                machine,
                state,
                expression,
                tracked,
                entry_established,
                call_established,
                inherited_windows,
                consumes_value,
                diagnostics,
            );
            if consumes_value {
                let is_name = matches!(
                    expression_table(program).expression(expression),
                    ExpressionNode::Name(_)
                );
                let has_open_window = !inherited_windows.is_empty()
                    || tracked.iter().any(TrackedPlace::any_window_open);
                if (is_name || has_open_window)
                    && let Some(spelling) = write_place_spelling(program, expression)
                {
                    let mut cached_definition = None;
                    let mut lookup_definition = || {
                        *cached_definition.get_or_insert_with(|| {
                            data_definition_for_expression(
                                program,
                                machine,
                                Some(state),
                                expression,
                            )
                        })
                    };
                    if is_name
                        && let Some(definition) = lookup_definition()
                        && crate::value_custody::data::data_requires_establishment(
                            program, definition,
                        )
                    {
                        validate_read!(
                            definition,
                            &spelling,
                            "<whole value>",
                            expression_root_symbol(program, expression),
                        );
                    }
                    if has_open_window {
                        let consumes_record = lookup_definition().is_some();
                        let overlaps_consumed_value = |place: &str| {
                            place_spelling_covers(&spelling, place)
                                || (consumes_record && place_spelling_covers(place, &spelling))
                        };
                        if let Some(place) = tracked.iter().find(|place| {
                            place.any_window_open() && overlaps_consumed_value(&place.spelling)
                        }) {
                            diagnostics.push(Diagnostic::error(format!(
                        "the next whole-value read of `{spelling}` occurs inside an OPEN invariant \
                         window: a prior write left data `{}`'s default domain FALSE -- \
                         restore the facts before copying or exposing the value (ch11)",
                        place.definition.name.as_str()
                    )));
                        } else if let Some((_, data_name, _)) = inherited_windows
                            .iter()
                            .find(|(place, _, _)| overlaps_consumed_value(place))
                        {
                            diagnostics.push(Diagnostic::error(format!(
                        "the next whole-value read of `{spelling}` occurs inside an OPEN invariant \
                         window carried from a predecessor state for data `{data_name}`"
                    )));
                        }
                    }
                }
            }
            match expression_table(program).expression(expression) {
                ExpressionNode::Name(path) => {
                    let members = expression_table(program).name_path_members(path.members);
                    if consumes_value && members.len() > 1 {
                        let receiver_spelling = members[..members.len() - 1].joined_path_with(".");
                        if let Some(place) = tracked.iter().find(|place| {
                            place_spelling_covers(&place.spelling, &receiver_spelling)
                        }) {
                            validate_read!(
                                place.definition,
                                &receiver_spelling,
                                members.last().expect("non-empty path").as_str(),
                                path.head_symbol,
                            );
                        }
                    }
                    if consumes_value
                        && members.len() > 1
                        && members[0].is_self_receiver()
                        && let Some(definition) = machine
                            .attached_data
                            .as_ref()
                            .and_then(|attached| program.data_definition_by_name(attached))
                    {
                        validate_read!(definition, "self", members[1].as_str(), path.head_symbol);
                    }
                }
                ExpressionNode::Member(member) => {
                    if let Some(receiver_spelling) = self_place_spelling(program, member.receiver) {
                        let definition = data_definition_for_expression(
                            program,
                            machine,
                            Some(state),
                            member.receiver,
                        )
                        .or_else(|| {
                            tracked
                                .iter()
                                .find(|place| place.spelling == receiver_spelling)
                                .map(|place| place.definition)
                        });
                        let established_child = self_place_spelling(program, expression)
                            .is_some_and(|child| {
                                direct_place_established(
                                    &child,
                                    tracked,
                                    entry_established,
                                    call_established,
                                )
                            });
                        let raw_current_field = definition.is_some_and(|definition| {
                            independently_present_scalar_field(
                                program,
                                definition,
                                member,
                                &receiver_spelling,
                                tracked,
                            )
                        });
                        if consumes_value
                            && !established_child
                            && !raw_current_field
                            && let Some(definition) = definition
                            && crate::value_custody::data::data_requires_establishment(
                                program, definition,
                            )
                        {
                            validate_read!(
                                definition,
                                &receiver_spelling,
                                member.member.as_str(),
                                expression_root_symbol(program, member.receiver),
                            );
                        }
                    }
                    scan_expression_use(
                        program,
                        machine,
                        state,
                        member.receiver,
                        tracked,
                        entry_established,
                        call_established,
                        inherited_windows,
                        false,
                        diagnostics,
                    );
                }

                ExpressionNode::Indexed(indexed) => {
                    scan_expression_use(
                        program,
                        machine,
                        state,
                        indexed.collection,
                        tracked,
                        entry_established,
                        call_established,
                        inherited_windows,
                        false,
                        diagnostics,
                    );
                    read_child!(indexed.index);
                }
                ExpressionNode::Binary(binary) => {
                    read_child!(binary.left);
                    read_child!(binary.right);
                }
                ExpressionNode::Borrow(inner) => {
                    if inner.access == language_core::ReferenceAccess::WriteOnly {
                        for index in write_target_index_expressions(program, inner.target) {
                            read_child!(index);
                        }
                        return;
                    }
                    if let ExpressionNode::Member(member) =
                        expression_table(program).expression(inner.target)
                        && let Some(receiver_spelling) =
                            self_place_spelling(program, member.receiver)
                        && let Some(place) = tracked.iter().find(|place| {
                            place_spelling_covers(&place.spelling, &receiver_spelling)
                                && place.any_window_open()
                        })
                    {
                        validate_read!(
                            place.definition,
                            &receiver_spelling,
                            member.member.as_str(),
                            expression_root_symbol(program, member.receiver),
                        );
                    }
                    read_child!(inner.target);
                }
                ExpressionNode::Call(call) => {
                    read_child!(call.receiver);
                    for argument in expression_table(program).expression_handles(call.arguments) {
                        read_child!(*argument);
                    }
                }
                ExpressionNode::Cast(cast) => {
                    read_child!(cast.value);
                }
                ExpressionNode::Unary(unary) => {
                    read_child!(unary.operand);
                }
                ExpressionNode::Atomic(atomic) => {
                    for operand in [atomic.value, atomic.result] {
                        read_child!(operand);
                    }
                }
                ExpressionNode::Range(range) => {
                    for bound in [range.start, range.end] {
                        read_child!(bound);
                    }
                }
                ExpressionNode::ArrayLiteral(elements) => {
                    for element in expression_table(program)
                        .expression_handles(*elements)
                        .iter()
                        .copied()
                    {
                        read_child!(element);
                    }
                }
                ExpressionNode::StructLiteral(literal) => {
                    for field in expression_table(program).struct_fields(literal.fields) {
                        read_child!(field.value);
                    }
                }
                ExpressionNode::Match(matched) => {
                    read_child!(matched.subject);
                    for arm in expression_table(program).match_arms(matched.arms) {
                        read_child!(arm.value);
                        for pattern in expression_table(program).match_pattern_operands(arm.pattern)
                        {
                            read_child!(pattern);
                        }
                    }
                }
                _ => {}
            }
        }

        fn validate_qualified_place_read(
            program: &dyn CheckingInput,
            machine: &Machine,
            state: &State,
            expression: ExpressionHandle,
            tracked: &[TrackedPlace<'_>],
            entry_established: &[String],
            call_established: &[String],
            inherited_windows: &[InvariantWindow],
            consumes_value: bool,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            if !matches!(
                expression_table(program).expression(expression),
                ExpressionNode::Name(_)
                    | ExpressionNode::Member(_)
                    | ExpressionNode::Indexed(_)
                    | ExpressionNode::Borrow(_)
            ) {
                return;
            }
            let Some(reference) = crate::value_custody::places::declared_place_type_raw(
                program,
                machine,
                Some(state),
                expression,
            ) else {
                return;
            };
            if !crate::value_custody::data::type_requires_establishment(program, reference) {
                return;
            }
            if !consumes_value
                && !matches!(
                    program.type_reference_table.type_reference(reference),
                    TypeReferenceNode::Constrained { .. }
                )
            {
                return;
            }
            if matches!(
                program.type_reference_table.type_reference(reference),
                TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. }
            ) && super::place_queries::data_definition_for_type(program, reference).is_some()
            {
                return;
            }
            let Some(spelling) = self_place_spelling(program, expression) else {
                return;
            };
            let covering = tracked
                .iter()
                .filter(|place| place_spelling_covers(&place.spelling, &spelling))
                .max_by_key(|place| place.spelling.len());
            let established = if let Some(place) = covering {
                !place.any_window_open()
                    && (place.established
                        || program
                            .data_members(place.definition)
                            .iter()
                            .filter_map(|member| member.as_field())
                            .any(|field| {
                                place.established_fields.contains(&field.symbol)
                                    && place_label(&place.spelling, field.name.as_str()) == spelling
                            }))
            } else {
                entry_established
                    .iter()
                    .any(|place| place_spelling_covers(place, &spelling))
                    || declared_root_established(
                        program,
                        state,
                        expression_root_symbol(program, expression),
                    )
            };
            let established = established
                || call_established
                    .iter()
                    .any(|place| place_spelling_covers(place, &spelling))
                || composed_carrier_established(
                    program,
                    reference,
                    &spelling,
                    tracked,
                    entry_established,
                    call_established,
                    &mut Vec::new(),
                );
            let open = tracked.iter().any(|place| {
                place.any_window_open()
                    && (place_spelling_covers(&place.spelling, &spelling)
                        || place_spelling_covers(&spelling, &place.spelling))
            }) || inherited_windows.iter().any(|(place, _, _)| {
                place_spelling_covers(place, &spelling) || place_spelling_covers(&spelling, place)
            });
            if !established || open {
                diagnostics.push(Diagnostic::error(format!(
            "reading `{spelling}` crosses an open default-domain invariant window before its declared qualification is established: the zeroed representation is not yet a value of `{}`",
            program.display_type_reference_with_constraints(reference),
        )));
            }
        }

        fn independently_present_scalar_field(
            program: &dyn CheckingInput,
            definition: &DataDefinition,
            member: &typed_trees::expression::TableMemberExpression,
            receiver_spelling: &str,
            tracked: &[TrackedPlace<'_>],
        ) -> bool {
            let symbol = crate::value_custody::places::effective_member_symbol(
                program,
                member.receiver,
                member,
            );
            if !symbol.is_valid() {
                return false;
            }
            let Some(field) = crate::value_custody::places::exact_data_member_field(
                program,
                definition,
                symbol,
                member.member.as_str(),
                member.case_variant.as_ref().map(|case| case.as_str()),
            ) else {
                return false;
            };
            if !matches!(
                program
                    .type_reference_table
                    .type_reference(field.type_reference),
                TypeReferenceNode::Named { .. }
            ) || program
                .primitive_type_reference(field.type_reference)
                .is_none()
                || crate::value_custody::data::type_requires_establishment(
                    program,
                    field.type_reference,
                )
            {
                return false;
            }
            let parent = program.symbols.get(field.symbol).parent;
            tracked.iter().any(|place| {
                place.spelling == receiver_spelling
                    && place.definition.symbol == definition.symbol
                    && place.established
                    && (program.symbols.get(parent).kind != symbols::SymbolKind::Variant
                        || place.active_case == Some(parent))
            })
        }

        fn validate_data_read(
            program: &dyn CheckingInput,
            state: &State,
            definition: &DataDefinition,
            receiver_spelling: &str,
            member_name: &str,
            receiver_root: symbols::SymbolHandle,
            tracked: &[TrackedPlace<'_>],
            entry_established: &[String],
            call_established: &[String],
            inherited_windows: &[InvariantWindow],
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let place = tracked
                .iter()
                .find(|place| place_spelling_covers(&place.spelling, receiver_spelling));
            let established = place.map(|place| place.established).unwrap_or_else(|| {
                entry_established
                    .iter()
                    .any(|established| place_spelling_covers(established, receiver_spelling))
                    || call_established
                        .iter()
                        .any(|established| place_spelling_covers(established, receiver_spelling))
                    || declared_root_established(program, state, receiver_root)
            });
            let established = established
                || composed_value_established(
                    program,
                    definition,
                    receiver_spelling,
                    tracked,
                    entry_established,
                    call_established,
                    &mut Vec::new(),
                );
            if !established {
                diagnostics.push(Diagnostic::error(format!(
                    "reading `{receiver_spelling}.{member_name}` crosses an open default-domain \
             invariant window before data `{}` is established: the zeroed representation \
             is not yet a `{}` (ch12's access gate) -- construct it on every path first \
             (the cross-state must-analysis carries establishment)",
                    definition.name.as_str(),
                    definition.name.as_str()
                )));
            }
            if tracked.iter().any(|place| {
                place_spelling_covers(&place.spelling, receiver_spelling) && place.any_window_open()
            }) {
                diagnostics.push(Diagnostic::error(format!(
                    "reading `{receiver_spelling}.{member_name}` inside an OPEN invariant window: \
             a prior write left data `{}`'s default domain FALSE -- restore the facts \
             before this consumption point (ch11)",
                    definition.name.as_str()
                )));
            }
            if place.is_none()
                && inherited_windows
                    .iter()
                    .any(|(spelling, _, _)| place_spelling_covers(spelling, receiver_spelling))
            {
                diagnostics.push(Diagnostic::error(format!(
                    "reading `{receiver_spelling}.{member_name}` inside an OPEN invariant window \
             carried from a predecessor state: data `{}`'s default domain is FALSE -- \
             restore the facts before this consumption point (ch11 window transport)",
                    definition.name.as_str()
                )));
            }
        }

        pub(crate) fn attached_value_established(
            program: &dyn CheckingInput,
            machine: &Machine,
            state: &State,
            tracked: &[TrackedPlace<'_>],
            entry_established: &[String],
            call_established: &[String],
        ) -> bool {
            let Some(data_binding) = program
                .state_data_binding(machine, state)
                .map(|name| name.as_str())
            else {
                return false;
            };
            if direct_place_established(data_binding, tracked, entry_established, call_established)
            {
                return true;
            }
            let Some(attached) = machine.attached_data.as_ref() else {
                return false;
            };
            let Some(definition) = program.data_definition_by_name(attached) else {
                return false;
            };
            composed_value_established(
                program,
                definition,
                data_binding,
                tracked,
                entry_established,
                call_established,
                &mut Vec::new(),
            )
        }

        fn composed_value_established(
            program: &dyn CheckingInput,
            definition: &DataDefinition,
            spelling: &str,
            tracked: &[TrackedPlace<'_>],
            entry_established: &[String],
            call_established: &[String],
            active: &mut Vec<symbols::SymbolHandle>,
        ) -> bool {
            if tracked.iter().any(|place| {
                place.any_window_open()
                    && (place_spelling_covers(&place.spelling, spelling)
                        || place_spelling_covers(spelling, &place.spelling))
            }) {
                return false;
            }
            if direct_place_established(spelling, tracked, entry_established, call_established) {
                return true;
            }
            if active.contains(&definition.symbol) {
                return false;
            }
            let root = tracked.iter().find(|place| place.spelling == spelling);
            let fields = root.map(|place| place.fields.as_slice()).unwrap_or(&[]);
            let valuation = fields
                .iter()
                .map(|(name, value)| (name.as_str(), *value))
                .collect::<Vec<_>>();
            for fact in program.proof_facts.span_or_empty(definition.requires_facts) {
                let Some(expression) = fact.as_expression() else {
                    return false;
                };
                if !super::symbolic_values::fold(
                    program,
                    definition,
                    &valuation,
                    root.map(|place| place.symbols.as_slice()).unwrap_or(&[]),
                    root.map(|place| place.measures.as_slice()).unwrap_or(&[]),
                    root.is_some_and(|place| place.born_zero),
                    root.and_then(|place| place.active_case),
                    *expression,
                )
                .is_some_and(|value| value != 0)
                {
                    return false;
                }
            }
            if program
                .data_members(definition)
                .iter()
                .any(|member| member.as_variant().is_some())
            {
                return false;
            }
            active.push(definition.symbol);
            let established = program
                .data_members(definition)
                .iter()
                .filter_map(|member| member.as_field())
                .all(|field| {
                    if !crate::value_custody::data::type_requires_establishment(
                        program,
                        field.type_reference,
                    ) {
                        return true;
                    }
                    if root.is_some_and(|place| place.established_fields.contains(&field.symbol)) {
                        return true;
                    }
                    let child = place_label(spelling, field.name.as_str());
                    if direct_place_established(
                        &child,
                        tracked,
                        entry_established,
                        call_established,
                    ) {
                        return true;
                    }
                    if matches!(
                        program
                            .type_reference_table
                            .type_reference(field.type_reference),
                        TypeReferenceNode::FixedArray { .. }
                    ) {
                        return composed_carrier_established(
                            program,
                            field.type_reference,
                            &child,
                            tracked,
                            entry_established,
                            call_established,
                            active,
                        );
                    }
                    if let Some(definition) = super::place_queries::data_definition_for_type(
                        program,
                        field.type_reference,
                    ) {
                        if matches!(
                            program
                                .type_reference_table
                                .type_reference(field.type_reference),
                            TypeReferenceNode::Constrained { .. }
                        ) {
                            return false;
                        }
                        return composed_value_established(
                            program,
                            definition,
                            &child,
                            tracked,
                            entry_established,
                            call_established,
                            active,
                        );
                    }
                    let Some(interval) =
                        crate::proof_contracts::arithmetic_domains::range_constraint_interval(
                            program,
                            field.type_reference,
                        )
                    else {
                        return false;
                    };
                    let Some(value) = fields
                        .iter()
                        .find(|(name, _)| name == field.name.as_str())
                        .and_then(|(_, value)| *value)
                        .and_then(super::symbolic_values::LiteralValue::integer)
                    else {
                        return false;
                    };
                    interval.low().is_none_or(|low| value >= i128::from(low))
                        && interval.high().is_none_or(|high| value <= i128::from(high))
                });
            active.pop();
            established
        }

        fn composed_carrier_established(
            program: &dyn CheckingInput,
            reference: typed_trees::types::TypeReferenceHandle,
            spelling: &str,
            tracked: &[TrackedPlace<'_>],
            entry_established: &[String],
            call_established: &[String],
            active: &mut Vec<symbols::SymbolHandle>,
        ) -> bool {
            if direct_place_established(spelling, tracked, entry_established, call_established)
                || !crate::value_custody::data::type_requires_establishment(program, reference)
            {
                return true;
            }
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: typed_trees::types::FixedArrayLength::Literal(extent),
                } => (0..*extent).all(|index| {
                    composed_carrier_established(
                        program,
                        *element_type,
                        &format!("{spelling}[{index}]"),
                        tracked,
                        entry_established,
                        call_established,
                        active,
                    )
                }),
                TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
                    super::place_queries::data_definition_for_type(program, reference).is_some_and(
                        |definition| {
                            composed_value_established(
                                program,
                                definition,
                                spelling,
                                tracked,
                                entry_established,
                                call_established,
                                active,
                            )
                        },
                    )
                }
                _ => false,
            }
        }

        fn direct_place_established(
            spelling: &str,
            tracked: &[TrackedPlace<'_>],
            entry_established: &[String],
            call_established: &[String],
        ) -> bool {
            if let Some(place) = tracked.iter().find(|place| place.spelling == spelling) {
                return place.established && !place.any_window_open();
            }
            entry_established.iter().any(|place| place == spelling)
                || call_established.iter().any(|place| place == spelling)
        }

        pub(crate) fn range_gates_hold(
            program: &dyn CheckingInput,
            place: &TrackedPlace<'_>,
        ) -> bool {
            for member in program.data_members(place.definition) {
                let Some(field) = member.as_field() else {
                    continue;
                };
                if !crate::value_custody::data::type_requires_establishment(
                    program,
                    field.type_reference,
                ) {
                    continue;
                }
                if place.established_fields.contains(&field.symbol) {
                    continue;
                }
                let Some(interval) =
                    crate::proof_contracts::arithmetic_domains::range_constraint_interval(
                        program,
                        field.type_reference,
                    )
                else {
                    return false;
                };
                let Some(value) = place
                    .fields
                    .iter()
                    .find(|(name, _)| name == field.name.as_str())
                    .and_then(|(_, value)| *value)
                    .and_then(super::symbolic_values::LiteralValue::integer)
                else {
                    return false;
                };
                if interval.low().is_some_and(|low| value < i128::from(low))
                    || interval.high().is_some_and(|high| value > i128::from(high))
                {
                    return false;
                }
            }
            true
        }
    }
    pub(crate) mod element_stores {
        use super::assignment_windows::{invariant_place_spelling, tracked_receiver};
        use super::state_flow::PlaceValuation;
        use super::symbolic_values::{LiteralValue, literal_value};
        use super::{InvariantWindow, TrackedPlace};
        use crate::CheckingInput;
        use crate::proof_contracts::collection_predicates::{
            self, ArgumentKnowledge, IntegerSet, unknown_knowledge,
        };
        use typed_trees::data::{DataDefinition, DataField, DataMember};
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::machine::Machine;
        use typed_trees::state::State;

        pub(crate) fn handle_element_store<'program>(
            program: &'program dyn CheckingInput,
            machine: &Machine,
            state: &State,
            destination: &crate::CanonicalPlace,
            target: ExpressionHandle,
            value: ExpressionHandle,
            tracked: &mut Vec<TrackedPlace<'program>>,
            entry_valuations: &[PlaceValuation],
            inherited_windows: &[InvariantWindow],
            poisoned_all: bool,
            poisoned_paths: &[String],
            born_zero: bool,
            statement_index: usize,
        ) -> Option<crate::CanonicalPlace> {
            // An element store writes one or more index positions below a data field:
            // `receiver.field[index]...`. A single index is a known or runtime element;
            // a deeper one leaves the field's elements unknown.
            let field_position = destination
                .segments
                .iter()
                .rposition(|segment| matches!(segment, facts::PlaceSegment::Field { .. }))?;
            let facts::PlaceSegment::Field {
                symbol: field_symbol,
            } = destination.segments[field_position]
            else {
                return None;
            };
            let indices = &destination.segments[field_position + 1..];
            if indices.is_empty()
                || !indices.iter().all(|segment| {
                    matches!(
                        segment,
                        facts::PlaceSegment::Index { .. } | facts::PlaceSegment::FixedIndex { .. }
                    )
                })
            {
                return None;
            }
            let element_index = match indices {
                [facts::PlaceSegment::FixedIndex { index }] => Some(Some(*index)),
                [facts::PlaceSegment::Index { expression }] => Some(
                    program
                        .expression_table
                        .constant_integer_value(*expression)
                        .and_then(|index| usize::try_from(index).ok()),
                ),
                _ => None,
            };
            let mut receiver = destination.clone();
            receiver.segments.truncate(field_position);
            if matches!(
                receiver.segments.last(),
                Some(facts::PlaceSegment::Case { .. })
            ) {
                receiver.segments.pop();
            }
            let receiver_spelling = invariant_place_spelling(program, &receiver)?;
            let receiver_type = crate::value_custody::places::canonical_place_type_reference(
                program,
                state.symbol,
                statement_index,
                &receiver,
            )?;
            let definition =
                crate::value_custody::places::data_definition_for_type(program, receiver_type)?;
            let field = data_field(program, definition, program.symbols.name(field_symbol))
                .filter(|field| field.symbol == field_symbol)?;
            let mut handled = destination.clone();
            handled.segments.truncate(field_position + 1);
            let handled = Some(handled);
            let affected = program
                .conjunctive_proof_facts(definition.requires_facts)
                .filter(|fact| {
                    super::place_queries::fact_reads_field_contents(
                        program,
                        definition,
                        fact,
                        field.symbol,
                    )
                })
                .collect::<Vec<_>>();
            if affected.is_empty() {
                return handled;
            }
            let data_binding = program
                .state_data_binding(machine, state)
                .map(|name| name.as_str().to_owned());
            let data_binding = data_binding.as_deref();
            let position = tracked_receiver(
                program,
                state,
                receiver.root.as_symbol()?,
                tracked,
                receiver_spelling,
                definition,
                entry_valuations,
                inherited_windows,
                poisoned_all,
                poisoned_paths,
                born_zero,
                data_binding,
            );
            let place = &mut tracked[position];
            let precise_receiver = !place.spelling.contains("[*]");
            let standing = precise_receiver
                && place.established
                && !place.window_open
                && place.element_windows.is_empty()
                && !inherited_windows
                    .iter()
                    .any(|(spelling, _, _)| *spelling == place.spelling);
            let mut elements = current_elements(program, place, field, standing);
            if let Some(elements) = elements.as_mut() {
                let element_type = element_carrier(program, field.type_reference);
                let stored = match (literal_value(program, value), element_type) {
                    (Some(LiteralValue::Integer(stored)), Some(_)) => IntegerSet::singleton(stored),
                    (_, Some(carrier)) => {
                        proved_stored_set(program, machine, state, statement_index, value)
                            .map(|proved| proved.intersect(&carrier))
                            .unwrap_or(carrier)
                    }
                    (_, None) => IntegerSet::empty(),
                };
                let literal_index = element_index
                    .flatten()
                    .filter(|index| *index < elements.len());
                match (literal_index, element_index) {
                    (Some(index), _) => elements[index] = stored,
                    (None, Some(_)) => {
                        for element in elements.iter_mut() {
                            *element = element.union(&stored);
                        }
                    }
                    (None, None) => {
                        *elements = unknown_elements(program, field).unwrap_or_default()
                    }
                }
            }
            let all_hold = precise_receiver
                && elements.as_ref().is_some_and(|elements| {
                    affected.iter().all(|fact| {
                        let Some(expression) = fact.as_expression() else {
                            return false;
                        };
                        collection_predicates::data_fact_decided(
                            program,
                            definition,
                            *expression,
                            &mut |argument| {
                                if argument.symbol == field.symbol {
                                    return Some(ArgumentKnowledge::Array(elements.clone()));
                                }
                                other_field_knowledge(program, place, argument, standing)
                            },
                        ) == Some(true)
                    })
                });
            place.elements.retain(|(symbol, _)| *symbol != field.symbol);
            if let Some(elements) = elements {
                place.elements.push((field.symbol, elements));
            }
            place
                .element_windows
                .retain(|symbol| *symbol != field.symbol);
            if !all_hold {
                place.element_windows.push(field.symbol);
                place
                    .opening_write
                    .get_or_insert(program.expression_table.source_span(target));
            } else if !place.any_window_open() {
                place.opening_write = None;
            }
            handled
        }

        fn data_field<'program>(
            program: &'program dyn CheckingInput,
            definition: &DataDefinition,
            name: &str,
        ) -> Option<&'program DataField> {
            program
                .data_members(definition)
                .iter()
                .find_map(|member| match member {
                    DataMember::Field(field) if field.name.as_str() == name => Some(field),
                    _ => None,
                })
        }

        fn proved_stored_set(
            program: &dyn CheckingInput,
            machine: &Machine,
            state: &State,
            statement_index: usize,
            value: ExpressionHandle,
        ) -> Option<IntegerSet> {
            let (low, high) =
                crate::proof_contracts::arithmetic_domains::occurrence_integer_expression_bounds(
                    program,
                    machine.symbol,
                    state.symbol,
                    statement_index,
                    value,
                )?;
            Some(IntegerSet::range(i128::from(low), i128::from(high)))
        }

        fn element_carrier(
            program: &dyn CheckingInput,
            field_type: typed_trees::types::TypeReferenceHandle,
        ) -> Option<IntegerSet> {
            match unknown_knowledge(program, field_type)? {
                ArgumentKnowledge::Array(elements) => elements.into_iter().next(),
                _ => None,
            }
        }

        fn unknown_elements(
            program: &dyn CheckingInput,
            field: &DataField,
        ) -> Option<Vec<IntegerSet>> {
            match unknown_knowledge(program, field.type_reference)? {
                ArgumentKnowledge::Array(elements) => Some(elements),
                _ => None,
            }
        }

        fn current_elements(
            program: &dyn CheckingInput,
            place: &TrackedPlace<'_>,
            field: &DataField,
            standing: bool,
        ) -> Option<Vec<IntegerSet>> {
            if let Some((_, elements)) = place
                .elements
                .iter()
                .find(|(symbol, _)| *symbol == field.symbol)
            {
                return Some(elements.clone());
            }
            if standing
                && let Some(elements) =
                    collection_predicates::standing_field_elements(program, place.definition, field)
            {
                return Some(elements);
            }
            unknown_elements(program, field)
        }

        fn other_field_knowledge(
            program: &dyn CheckingInput,
            place: &TrackedPlace<'_>,
            field: &DataField,
            standing: bool,
        ) -> Option<ArgumentKnowledge> {
            if let Some(elements) = current_elements(program, place, field, standing)
                && !elements.is_empty()
            {
                return Some(ArgumentKnowledge::Array(elements));
            }
            let ArgumentKnowledge::Integer(carrier) =
                unknown_knowledge(program, field.type_reference)?
            else {
                return None;
            };
            let known = place
                .fields
                .iter()
                .find(|(name, _)| name == field.name.as_str())
                .map(|(_, value)| value.and_then(LiteralValue::integer));
            Some(ArgumentKnowledge::Integer(match known {
                Some(Some(value)) => IntegerSet::singleton(value),
                None if place.born_zero => IntegerSet::singleton(0),
                _ => carrier,
            }))
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

        pub(crate) fn data_definition_for_expression<'program>(
            program: &'program dyn crate::CheckingInput,
            machine: &Machine,
            state: Option<&State>,
            expression: ExpressionHandle,
        ) -> Option<&'program DataDefinition> {
            if self_place_spelling(program, expression)
                .as_deref()
                .is_some_and(is_self_receiver)
            {
                let attached = machine.attached_data.as_ref()?;
                return program.data_definition_by_name(attached);
            }
            if let Some(indexed) = expression_table(program).indexed_node(expression) {
                let mut collection_type = crate::value_custody::places::declared_place_type_raw(
                    program,
                    machine,
                    state,
                    indexed.collection,
                )?;
                loop {
                    match program.type_reference_table.type_reference(collection_type) {
                        TypeReferenceNode::Reference { referee, .. } => collection_type = *referee,
                        TypeReferenceNode::Constrained { base_type, .. } => {
                            collection_type = *base_type
                        }
                        TypeReferenceNode::FixedArray { element_type, .. }
                        | TypeReferenceNode::Slice { element_type } => {
                            return data_definition_for_type(program, *element_type);
                        }
                        _ => return None,
                    }
                }
            }
            let receiver_type = crate::value_custody::places::declared_place_type(
                program, machine, state, expression,
            )?;
            data_definition_for_type(program, receiver_type)
        }

        pub(crate) fn data_definition_for_type(
            program: &dyn crate::CheckingInput,
            handle: TypeReferenceHandle,
        ) -> Option<&DataDefinition> {
            match program.type_reference_table.type_reference(handle) {
                TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => program
                    .data_application_definition(handle)
                    .or_else(|| program.closed_data_definition(handle)),
                TypeReferenceNode::Reference { referee, .. } => {
                    data_definition_for_type(program, *referee)
                }
                TypeReferenceNode::Constrained { base_type, .. } => {
                    data_definition_for_type(program, *base_type)
                }
                _ => None,
            }
        }

        pub(crate) fn completed_case_occurrences(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
        ) -> Vec<(usize, ExpressionHandle, TypeReferenceHandle)> {
            let mut occurrences = Vec::new();
            for (statement_index, statement) in program
                .statement_table
                .indexed_statements(state.statement_nodes)
            {
                match statement {
                    typed_trees::statement::StatementNode::Assignment(assignment) => {
                        let Some(spelling) = write_place_spelling(program, assignment.target)
                        else {
                            continue;
                        };
                        if spelling.contains("[*]") {
                            continue;
                        }
                        let Some(reference) = crate::value_custody::places::declared_place_type_raw(
                            program,
                            machine,
                            Some(state),
                            assignment.target,
                        ) else {
                            continue;
                        };
                        if crate::reference_shell(program, reference).is_some()
                            || data_definition_for_expression(
                                program,
                                machine,
                                Some(state),
                                assignment.target,
                            )
                            .is_none()
                        {
                            continue;
                        }
                        if crate::proof_contracts::contract_entailment::completed_value_case_definition(
                    program, reference,
                )
                .is_none()
                {
                    continue;
                }
                        occurrences.push((statement_index, assignment.value, reference));
                    }
                    typed_trees::statement::StatementNode::LocalData(local) => {
                        if !local.initial_value.is_valid() {
                            continue;
                        }
                        if crate::proof_contracts::contract_entailment::completed_value_case_definition(
                    program,
                    local.type_reference,
                )
                .is_none()
                {
                    continue;
                }
                        if let ExpressionNode::StructLiteral(literal) =
                            expression_table(program).expression(local.initial_value)
                            && literal
                                .case_symbol
                                .filter(|symbol| symbol.is_valid())
                                .is_some()
                        {
                            continue;
                        }
                        occurrences.push((
                            statement_index,
                            local.initial_value,
                            local.type_reference,
                        ));
                    }
                    _ => {}
                }
            }
            occurrences
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
            if let Err(reason) = query.as_ref()
                && crate::proof_contracts::contract_entailment::struct_trace()
                && crate::canonical_place_from_expression(program, expression).is_some_and(
                    |place| {
                        place.root.as_symbol() == Some(definition.predicate_subject_symbol)
                            || place
                                .root
                                .as_symbol()
                                .is_some_and(|symbol| program.any_field_owner(symbol).is_some())
                    },
                )
            {
                eprintln!(
                    "DEFAULT predicate-projection-refused owner={} expression={expression:?} reason={reason}",
                    definition.name
                );
            }
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
    pub(crate) mod post_store_predicates {
        use super::TrackedPlace;
        use super::symbolic_values::{
            SymbolicValue, applied_const_argument, applied_const_integer,
        };
        use crate::proof_contracts::arithmetic_domains::BigIntInterval;
        use crate::{
            ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
            ScopedArithmeticHypothesis, ScopedArithmeticValue, StrictArithmeticImplicationJudgment,
            scoped_arithmetic_implication,
        };
        use numerics::bignum::BigInt;
        use symbols::SymbolHandle;
        use typed_trees::data::{DataDefinition, DataMember, TypeParameterKind};
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::machine::Machine;
        use typed_trees::signature::SignatureContractKind;
        use typed_trees::state::State;
        use typed_trees::types::{PrimitiveType, TypeReferenceNode};

        pub(crate) fn post_store_predicate_holds(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
            statement_index: usize,
            receiver: &crate::CanonicalPlace,
            receiver_type: typed_trees::types::TypeReferenceHandle,
            preserves_scalar_value: bool,
            destination: &crate::CanonicalPlace,
            place: &TrackedPlace<'_>,
            written_field: SymbolHandle,
            written_value: ExpressionHandle,
            predicate: ExpressionHandle,
        ) -> bool {
            let Some(completed_prefix) = statement_index.checked_add(1) else {
                return false;
            };
            if !preserves_scalar_value {
                return crate::proof_contracts::contract_entailment::captured_data_predicate_holds(
                    program,
                    machine,
                    state,
                    crate::proof_contracts::contract_entailment::CapturedPredicatePoint::Prefix(
                        completed_prefix,
                    ),
                    receiver,
                    receiver_type,
                    place.definition,
                    predicate,
                );
            }
            let mut arrivals = Vec::new();
            for parameter in program.state_parameters(state) {
                if !parameter.is_mutable
                    && let Some(primitive) =
                        program.primitive_type_reference(parameter.type_reference)
                    && primitive.is_fixed_integer()
                {
                    arrivals.push(scalar_binding(parameter.symbol, primitive));
                }
            }
            for parameter in program.machine_type_parameters(machine) {
                let (TypeParameterKind::Const { type_reference }
                | TypeParameterKind::Value { type_reference }) = parameter.kind
                else {
                    continue;
                };
                if let Some(primitive) = program.primitive_type_reference(type_reference)
                    && primitive.is_fixed_integer()
                {
                    arrivals.push(scalar_binding(parameter.symbol, primitive));
                }
            }

            let written_bounds =
                crate::proof_contracts::arithmetic_domains::occurrence_integer_expression_bounds(
                    program,
                    machine.symbol,
                    state.symbol,
                    statement_index,
                    written_value,
                );
            let declared_bounds =
                source_field_declaration_interval(program, machine, state, written_value);
            let written = ScopedArithmeticValue::Term(ScopedArithmeticExpression {
                owner: machine.symbol,
                expression: written_value,
                bindings: arrivals.clone(),
            });
            let interval = match written_bounds {
                Some((minimum, maximum)) => {
                    let observed = BigIntInterval {
                        low: Some(BigInt::from_i128(i128::from(minimum))),
                        high: Some(BigInt::from_i128(i128::from(maximum))),
                    };
                    Some(match declared_bounds {
                        Some(declared) => observed.intersect(&declared),
                        None => observed,
                    })
                }
                None => declared_bounds,
            };
            let written = match interval {
                Some(interval) => ScopedArithmeticValue::BoundedInteger {
                    value: Box::new(ScopedArithmeticValue::Atom {
                        identity: format!(
                            "\0stored-value:{:?}:{:?}:{statement_index}:{written_value:?}",
                            machine.symbol, state.symbol
                        ),
                        unsigned: crate::expression_result_type_reference(
                            program,
                            machine,
                            state,
                            written_value,
                        )
                        .and_then(|reference| program.primitive_type_reference(reference))
                        .is_some_and(|primitive| !primitive.is_signed_integer()),
                    }),
                    interval,
                },
                None => written,
            };
            let mut fields = Vec::new();
            let relative = destination
                .segments
                .strip_prefix(receiver.segments.as_slice());
            crate::queries::expression_traversal::visit_pre_order(
                program,
                vec![predicate],
                |expression, _| {
                    if let Some(projection) = super::place_queries::predicate_projection(
                        program,
                        place.definition,
                        receiver_type,
                        expression,
                    ) && relative == Some(projection.as_slice())
                        && crate::project_type_reference_from_segments(
                            program,
                            receiver_type,
                            &projection,
                        )
                        .is_some_and(|reference| {
                            program
                                .primitive_type_reference(reference)
                                .is_some_and(PrimitiveType::is_fixed_integer)
                        })
                    {
                        fields.push(ScopedArithmeticBinding {
                            binder: ScopedArithmeticBinder::Occurrence(expression),
                            value: written.clone(),
                        });
                    }
                    false
                },
            );
            for member in program.data_members(place.definition) {
                let Some(field) = member.as_field() else {
                    continue;
                };
                if !program
                    .primitive_type_reference(field.type_reference)
                    .is_some_and(PrimitiveType::is_fixed_integer)
                {
                    continue;
                }
                let value = if field.symbol == written_field
                    && relative.is_some_and(|path| path.len() == 1)
                {
                    Some(written.clone())
                } else if let Some((_, Some(value))) = place
                    .fields
                    .iter()
                    .find(|(name, _)| name == field.name.as_str())
                {
                    value
                        .integer()
                        .map(|value| ScopedArithmeticValue::Integer(BigInt::from_i128(value)))
                } else {
                    place
                        .symbols
                        .iter()
                        .find(|(name, _)| name == field.name.as_str())
                        .and_then(|(_, value)| match value {
                            SymbolicValue::Atom(symbol) => arrivals
                                .iter()
                                .find(|binding| {
                                    binding.binder == ScopedArithmeticBinder::Symbol(*symbol)
                                })
                                .map(|binding| binding.value.clone()),
                            _ => None,
                        })
                };
                if let Some(value) = value {
                    fields.push(ScopedArithmeticBinding {
                        binder: ScopedArithmeticBinder::Symbol(field.symbol),
                        value,
                    });
                }
            }
            if let Some(origin) = place.definition.generic_instance
                && let TypeReferenceNode::Generic { base_symbol, .. } =
                    program.type_reference_table.type_reference(origin)
                && let Some(template) = program.unique_data_definition_by_symbol(*base_symbol)
            {
                for parameter in program.data_type_parameters(template) {
                    let value = if let Some((_, value)) =
                        applied_const_integer(program, place.definition, parameter.symbol)
                    {
                        Some(ScopedArithmeticValue::Integer(BigInt::from_i128(value)))
                    } else {
                        applied_const_argument(program, place.definition, parameter.symbol)
                            .and_then(|(_, actual)| {
                                let TypeReferenceNode::Named { symbol, .. } =
                                    program.type_reference_table.type_reference(actual)
                                else {
                                    return None;
                                };
                                if !symbol.is_valid()
                                    || program.symbols.get(*symbol).kind
                                        != symbols::SymbolKind::TypeParameter
                                {
                                    return None;
                                }
                                arrivals
                                    .iter()
                                    .find(|binding| {
                                        binding.binder == ScopedArithmeticBinder::Symbol(*symbol)
                                    })
                                    .map(|binding| binding.value.clone())
                            })
                    };
                    if let Some(value) = value {
                        fields.push(ScopedArithmeticBinding {
                            binder: ScopedArithmeticBinder::Symbol(parameter.symbol),
                            value,
                        });
                    }
                }
            }
            let mut projections = Vec::new();
            crate::queries::expression_traversal::visit_pre_order(
                program,
                vec![predicate],
                |expression, _| {
                    if let Some(field) = super::place_queries::membership_field(
                        program,
                        place.definition,
                        expression,
                    ) && let Some(binding) = fields.iter().find(|binding| {
                        binding.binder == ScopedArithmeticBinder::Symbol(field.symbol)
                    }) {
                        projections.push(ScopedArithmeticBinding {
                            binder: ScopedArithmeticBinder::Occurrence(expression),
                            value: binding.value.clone(),
                        });
                    }
                    false
                },
            );
            fields.extend(projections);
            let contracts = if program
                .machine_states(machine)
                .first()
                .is_some_and(|entry| entry.symbol == state.symbol)
            {
                program.machine_contracts(machine)
            } else {
                program.state_contracts(state)
            };
            let hypotheses = contracts
                .iter()
                .filter(|contract| contract.kind == SignatureContractKind::Requires)
                .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
                .filter_map(|fact| {
                    Some(ScopedArithmeticHypothesis {
                        proposition: ScopedArithmeticExpression {
                            owner: machine.symbol,
                            expression: *fact.as_expression()?,
                            bindings: arrivals.clone(),
                        },
                        holds: true,
                    })
                })
                .collect::<Vec<_>>();
            scoped_arithmetic_implication(
                program,
                machine,
                &hypotheses,
                &ScopedArithmeticExpression {
                    owner: place.definition.symbol,
                    expression: predicate,
                    bindings: fields,
                },
            ) == StrictArithmeticImplicationJudgment::Proven
                || crate::proof_contracts::contract_entailment::captured_data_predicate_holds(
                    program,
                    machine,
                    state,
                    crate::proof_contracts::contract_entailment::CapturedPredicatePoint::Prefix(
                        completed_prefix,
                    ),
                    receiver,
                    receiver_type,
                    place.definition,
                    predicate,
                )
        }

        fn source_field_declaration_interval(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
            expression: ExpressionHandle,
        ) -> Option<BigIntInterval> {
            let table = crate::frozen_program::expression_table(program);
            if let Some(member) = table.member_node(expression) {
                if !receiver_retains_declared_domain(program, state, member.receiver) {
                    return None;
                }
                let definition = super::place_queries::data_definition_for_expression(
                    program,
                    machine,
                    Some(state),
                    member.receiver,
                )?;
                return declared_field_interval(program, definition, member.member.as_str());
            }
            let path = table.name_path_node(expression)?;
            let members = table.name_path_members(path.members);
            let [field_name] = members else {
                return None;
            };
            if !binding_retains_declared_domain(program, state, path.head_symbol) {
                return None;
            }
            let binding_type = binding_type_reference(program, state, path.head_symbol)?;
            let definition = super::place_queries::data_definition_for_type(program, binding_type)?;
            declared_field_interval(program, definition, field_name.as_str())
        }

        fn declared_field_interval(
            program: &dyn crate::CheckingInput,
            definition: &DataDefinition,
            field_name: &str,
        ) -> Option<BigIntInterval> {
            let field =
                program
                    .data_members(definition)
                    .iter()
                    .find_map(|member| match member {
                        DataMember::Field(field) if field.name.as_str() == field_name => {
                            Some(field.symbol)
                        }
                        DataMember::Field(_) | DataMember::Variant(_) => None,
                    })?;
            let (_, low, high) =
                super::requires_fact_intervals::data_requires_field_intervals(program, definition)?
                    .into_iter()
                    .find(|(symbol, _, _)| *symbol == field)?;
            if low.is_none() && high.is_none() {
                return None;
            }
            Some(BigIntInterval {
                low: low.map(|bound| BigInt::from_i128(i128::from(bound))),
                high: high.map(|bound| BigInt::from_i128(i128::from(bound))),
            })
        }

        fn receiver_retains_declared_domain(
            program: &dyn crate::CheckingInput,
            state: &State,
            mut expression: ExpressionHandle,
        ) -> bool {
            let table = crate::frozen_program::expression_table(program);
            for _ in 0..16 {
                if !expression.is_valid() {
                    return false;
                }
                match table.expression(expression) {
                    typed_trees::expression::ExpressionNode::Member(member) => {
                        expression = member.receiver;
                    }
                    typed_trees::expression::ExpressionNode::Borrow(borrow) => {
                        if borrow.access.is_exclusive() {
                            return false;
                        }
                        expression = borrow.target;
                    }
                    typed_trees::expression::ExpressionNode::Name(path) => {
                        return binding_retains_declared_domain(program, state, path.head_symbol);
                    }
                    _ => return false,
                }
            }
            false
        }

        fn binding_retains_declared_domain(
            program: &dyn crate::CheckingInput,
            state: &State,
            symbol: SymbolHandle,
        ) -> bool {
            let Some(type_reference) = binding_type_reference(program, state, symbol) else {
                return false;
            };
            if let Some(parameter) = program.state_parameter_by_symbol(state, symbol) {
                if parameter.is_mutable {
                    return false;
                }
            } else if let Some((_, local)) = program.state_local_declaration(state, symbol) {
                if local.is_mutable {
                    return false;
                }
            } else {
                return false;
            }
            crate::reference_shell(program, type_reference)
                .is_none_or(|shell| !shell.access.is_exclusive())
        }

        fn binding_type_reference(
            program: &dyn crate::CheckingInput,
            state: &State,
            symbol: SymbolHandle,
        ) -> Option<typed_trees::types::TypeReferenceHandle> {
            if let Some(parameter) = program.state_parameter_by_symbol(state, symbol) {
                return Some(parameter.type_reference);
            }
            program
                .state_local_declaration(state, symbol)
                .map(|(_, local)| local.type_reference)
        }

        fn scalar_binding(
            symbol: SymbolHandle,
            primitive: PrimitiveType,
        ) -> ScopedArithmeticBinding {
            ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::Symbol(symbol),
                value: ScopedArithmeticValue::Atom {
                    identity: format!(
                        "default-domain:{}:{}",
                        symbol.arena_index(),
                        symbol.generation()
                    ),
                    unsigned: matches!(
                        primitive,
                        PrimitiveType::U8
                            | PrimitiveType::U16
                            | PrimitiveType::U32
                            | PrimitiveType::U64
                    ),
                },
            }
        }
    }
    pub(crate) mod standing_requirements {
        use super::place_queries::data_definition_for_expression;
        use crate::frozen_program::expression_table;
        use crate::proof_contracts::collection_predicates::{
            fact_argument_field, predicate_application,
        };
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::machine::Machine;
        use typed_trees::signature::StateParameter;
        use typed_trees::state::State;

        #[derive(PartialEq, Eq)]
        pub(crate) enum RequiredArgument {
            Field(symbols::SymbolHandle),
            Integer(i64),
        }

        pub fn standing_facts_prove_requirement(
            program: &dyn crate::CheckingInput,
            caller: &Machine,
            caller_state: &State,
            callee: symbols::SymbolHandle,
            parameters: &[StateParameter],
            actuals: &[ExpressionHandle],
            requirement: ExpressionHandle,
        ) -> bool {
            let Some(application) = predicate_application(program, callee, requirement) else {
                return false;
            };
            let mut receiver = None;
            let mut required = Vec::with_capacity(application.arguments.len());
            for argument in application.arguments {
                if let Some(value) = expression_table(program).constant_integer_value(*argument) {
                    required.push(RequiredArgument::Integer(value));
                    continue;
                }
                let Some(position) = formal_position(program, parameters, *argument) else {
                    return false;
                };
                let Some(actual) = actuals.get(position) else {
                    return false;
                };
                let Some(place) =
                    collection_observation_place(program, caller, caller_state, *actual)
                else {
                    return false;
                };
                let Some(member) = expression_table(program).member_node(place) else {
                    return false;
                };
                match receiver {
                    None => receiver = Some(member.receiver),
                    Some(existing)
                        if expression_table(program)
                            .expressions_structurally_equal(existing, member.receiver) => {}
                    Some(_) => return false,
                }
                let Some(field) = crate::value_custody::places::declared_member_field_symbol(
                    program,
                    caller,
                    Some(caller_state),
                    place,
                ) else {
                    return false;
                };
                required.push(RequiredArgument::Field(field));
            }
            let Some(receiver) = receiver else {
                return false;
            };
            let Some(definition) =
                data_definition_for_expression(program, caller, Some(caller_state), receiver)
            else {
                return false;
            };
            if definition.zero_gated {
                return false;
            }
            program
                .conjunctive_proof_facts(definition.requires_facts)
                .any(|fact| {
                    let Some(fact) = fact.as_expression() else {
                        return false;
                    };
                    let Some(standing) = predicate_application(program, definition.symbol, *fact)
                    else {
                        return false;
                    };
                    standing.machine == application.machine
                        && standing.arguments.len() == required.len()
                        && standing
                            .arguments
                            .iter()
                            .zip(&required)
                            .all(|(argument, required)| match required {
                                RequiredArgument::Field(symbol) => {
                                    fact_argument_field(program, definition, *argument)
                                        .is_some_and(|field| field.symbol == *symbol)
                                }
                                RequiredArgument::Integer(value) => {
                                    expression_table(program).constant_integer_value(*argument)
                                        == Some(*value)
                                }
                            })
                })
        }

        fn collection_observation_place(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
            mut expression: ExpressionHandle,
        ) -> Option<ExpressionHandle> {
            for _ in 0..128 {
                if let Some(borrow) = expression_table(program).borrow_expression(expression) {
                    if borrow.access == language_core::ReferenceAccess::WriteOnly {
                        return None;
                    }
                    expression = borrow.target;
                } else if let Some(source) =
                    crate::value_custody::places::whole_collection_view_source(
                        program,
                        machine,
                        Some(state),
                        expression,
                    )
                {
                    expression = source;
                } else {
                    return Some(expression);
                }
            }
            None
        }

        fn formal_position(
            program: &dyn crate::CheckingInput,
            parameters: &[StateParameter],
            argument: ExpressionHandle,
        ) -> Option<usize> {
            let symbol = crate::queries::expression_traversal::single_member_argument_symbol(
                program, argument,
            )?;
            typed_trees::signature::state_parameter_position(parameters, symbol)
        }
    }
    pub(crate) mod state_flow {
        use crate::CheckingInput;
        use crate::frozen_program::expression_table;
        use typed_trees::state::State;
        use typed_trees::statement::TransitionTargetNode;

        pub(crate) type PlaceValuation = (
            String,
            Vec<(String, Option<super::symbolic_values::LiteralValue>)>,
            Option<symbols::SymbolHandle>,
        );

        pub(crate) fn canonicalize_valuations(valuations: &mut [PlaceValuation]) {
            valuations.sort_by(|left, right| left.0.cmp(&right.0));
            for (_, fields, _) in valuations {
                fields.sort_by(|left, right| left.0.cmp(&right.0));
            }
        }

        pub(crate) fn meet_valuations(
            left: &[PlaceValuation],
            right: &[PlaceValuation],
        ) -> Vec<PlaceValuation> {
            let mut result = Vec::new();
            for (spelling, left_fields, left_case) in left {
                let Some((_, right_fields, right_case)) =
                    right.iter().find(|(name, _, _)| name == spelling)
                else {
                    continue;
                };
                let mut fields = Vec::new();
                for (field, left_value) in left_fields {
                    if let Some((_, right_value)) =
                        right_fields.iter().find(|(name, _)| name == field)
                        && left_value == right_value
                        && left_value.is_some()
                    {
                        fields.push((field.clone(), *left_value));
                    }
                }
                let active_case = (left_case == right_case).then_some(*left_case).flatten();
                result.push((spelling.clone(), fields, active_case));
            }
            result
        }

        pub(crate) fn state_edges(
            program: &dyn CheckingInput,
            states: &[State],
        ) -> Vec<(usize, usize)> {
            let mut edges = Vec::new();
            for (from, state) in states.iter().enumerate() {
                for statement in program.statement_table.statements(state.statement_nodes) {
                    let Some(transition) = statement.as_transition() else {
                        continue;
                    };
                    for handle in transition.targets() {
                        if let TransitionTargetNode::Named { path, .. } =
                            program.statement_table.transition_target(handle)
                        {
                            let to = states
                                .iter()
                                .position(|candidate| candidate.symbol == path.symbol)
                                .or_else(|| {
                                    expression_table(program)
                                        .name_path_members(path.members)
                                        .last()
                                        .and_then(|target_name| {
                                            states.iter().position(|candidate| {
                                                candidate.name == *target_name
                                            })
                                        })
                                });
                            if let Some(to) = to {
                                edges.push((from, to));
                            }
                        }
                    }
                }
            }
            edges
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
            A store whose value the default-domain window cannot fold from the
            lender's carrier, so the window instead trusts an interval proved
            for the written value. `source_interval` is that interval in the
            stored field's own carrier; `reinterpreted` marks a store whose
            bits cross carriers, where the window's literal and symbol
            tracking does not apply.
        */
        #[derive(Clone, Copy)]
        pub(crate) struct ResidentRequirement {
            pub(crate) source_interval: Interval,
            pub(crate) reinterpreted: bool,
        }

        /*
            Recasts spec, "Shared and mutable views": every value writable
            through an immediately established mutable view must leave the
            lender valid. When the view presents the stored scalar under
            another integer carrier (`i32` storage seen as `u32`), the stored
            bits do not carry the written number into the lender's carrier, so
            the lender's window cannot fold the literal and stays open. The
            written value's proved bounds are carried into the lender's
            carrier only when every value in them has the same bits and the
            same number in both carriers. A wrapping or unbounded value
            translates to nothing and the lender's window stays open, so a
            write the lender's `requires` would not admit is still refused.
        */
        pub(crate) fn reinterpreted_view_store(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
            frames: &crate::CallFrameResolver<'_>,
            statement: &typed_trees::statement::StatementNode,
            statement_index: usize,
        ) -> Option<ResidentRequirement> {
            let assignment = statement.as_assignment()?;
            let origins = frames.assignment_storage_origins(machine, statement).ok()?;
            let [destination] = origins.destinations.as_slice() else {
                return None;
            };
            if destination.collection_coarse || destination.recasts.is_empty() {
                return None;
            }
            let scalar = destination
                .scalar_storage_coordinate_result(program, state)
                .ok()?;
            if scalar.preserves_scalar_value || !scalar.preserves_primitive_bits {
                return None;
            }
            let (low, high) =
                crate::proof_contracts::arithmetic_domains::occurrence_integer_expression_bounds(
                    program,
                    machine.symbol,
                    state.symbol,
                    statement_index,
                    assignment.value,
                )?;
            same_number_interval(
                Interval {
                    low: Some(low),
                    high: Some(high),
                },
                scalar.view_primitive,
                scalar.source_primitive,
            )
            .map(|source_interval| ResidentRequirement {
                source_interval,
                reinterpreted: true,
            })
        }

        fn same_number_interval(
            view_interval: Interval,
            view: typed_trees::types::PrimitiveType,
            source: typed_trees::types::PrimitiveType,
        ) -> Option<Interval> {
            use crate::proof_contracts::arithmetic_domains::primitive_range;
            if !view.is_fixed_integer()
                || !source.is_fixed_integer()
                || view.scalar_byte_size()? != source.scalar_byte_size()?
            {
                return None;
            }
            let interval = view_interval.intersect(primitive_range(view)?);
            let (low, high) = interval.low.zip(interval.high)?;
            let shared = primitive_range(view)?.intersect(primitive_range(source)?);
            (shared.low.is_none_or(|minimum| low >= minimum)
                && shared.high.is_none_or(|maximum| high <= maximum))
            .then_some(interval)
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

        pub(crate) fn stored_field_requires_interval(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
            mut target: ExpressionHandle,
        ) -> Option<Interval> {
            for _ in 0..8 {
                if program.expression_table.member_node(target).is_some() {
                    break;
                }
                let ExpressionNode::Name(path) = expression_table(program).expression(target)
                else {
                    break;
                };
                let [name] = expression_table(program).name_path_members(path.members) else {
                    break;
                };
                let Some(local) = program
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .filter_map(|statement| statement.as_local_data())
                    .find(|local| !local.is_mutable && local.name == *name)
                else {
                    break;
                };
                if let Some(borrow) =
                    expression_table(program).borrow_expression(local.initial_value)
                {
                    if !matches!(
                        borrow.access,
                        language_core::ReferenceAccess::Mutable
                            | language_core::ReferenceAccess::WriteOnly
                    ) {
                        break;
                    }
                    target = borrow.target;
                } else {
                    target = local.initial_value;
                }
            }
            let member = expression_table(program).member_node(target)?;
            if crate::value_custody::recasts::pending_views::withholds_default_predicate(
                program,
                machine,
                state,
                member.receiver,
            ) {
                return None;
            }
            let definition =
                data_definition_for_expression(program, machine, Some(state), member.receiver)?;
            let field = program
                .data_members(definition)
                .iter()
                .find_map(|candidate| match candidate {
                    DataMember::Field(field) if field.name == member.member => Some(field),
                    _ => None,
                })?;
            data_requires_field_intervals(program, definition)?
                .into_iter()
                .find(|(symbol, _, _)| *symbol == field.symbol)
                .map(|(_, low, high)| Interval { low, high })
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
    pub(crate) use place_queries::completed_case_occurrences;
    pub(crate) use place_queries::data_predicate_fields;
    pub(crate) use place_queries::membership_field;
    pub(crate) use place_queries::predicate_projection;
    pub(crate) use requires_fact_intervals::{
        FieldRepresentationBound, atomic_field_interval, data_requires_field_representation_bounds,
        stored_field_requires_interval,
    };
    pub use requires_fact_intervals::{
        case_requires_field_intervals, data_requires_field_intervals, impossible_variant,
    };
    pub use standing_requirements::standing_facts_prove_requirement;
    pub(crate) use symbolic_values::applied_const_integer;

    use diagnostics::Diagnostic;
    use typed_trees::data::DataDefinition;
    use typed_trees::machine::Machine;
    use typed_trees::state::State;
    use typed_trees::statement::StatementNode;

    use crate::proof_contracts::default_domains::assignment_windows::handle_assignment;
    use crate::proof_contracts::default_domains::assignment_windows::preserve_proven_establishment;
    use crate::proof_contracts::default_domains::assignment_windows::refuse_open_windows;
    use crate::proof_contracts::default_domains::data_reads::attached_value_established;
    use crate::proof_contracts::default_domains::data_reads::scan_statement_reads;
    use call_summaries::{StateToMachine, collect_call_summaries, state_to_machine_index};
    use place_queries::{
        is_data_rooted, transition_evaluated_expressions, write_target_index_expressions,
    };
    use state_flow::{PlaceValuation, canonicalize_valuations, meet_valuations, state_edges};
    use symbolic_values::{SymbolicValue, expression_contains_call};

    pub(crate) type InvariantWindow = (String, String, symbols::SymbolHandle);

    pub(crate) type ParameterCallAttempts = std::sync::Mutex<Vec<(symbols::SymbolHandle, bool)>>;

    pub(crate) struct StateExit {
        established: Vec<String>,
        valuations: Vec<PlaceValuation>,
        windows: Vec<InvariantWindow>,
        was_reached: bool,
    }

    pub(crate) struct StateWalk {
        exit: StateExit,
        diagnostics: Vec<Diagnostic>,
        crash_sites: Vec<OpenInvariantCrashSite>,
    }

    fn state_predecessors(edges: &[(usize, usize)], state_count: usize) -> Vec<Vec<usize>> {
        let mut predecessors = vec![Vec::new(); state_count];
        for (from, to) in edges {
            predecessors[*to].push(*from);
        }
        predecessors
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct OpenInvariantCrashSite {
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        statement_ordinal: u32,
        open_data: Vec<symbols::SymbolHandle>,
    }

    impl OpenInvariantCrashSite {
        pub const fn machine(&self) -> symbols::SymbolHandle {
            self.machine
        }

        pub const fn state(&self) -> symbols::SymbolHandle {
            self.state
        }

        pub const fn statement_ordinal(&self) -> u32 {
            self.statement_ordinal
        }

        pub fn open_data(&self) -> &[symbols::SymbolHandle] {
            &self.open_data
        }
    }

    pub(crate) fn validate_default_domain_writes(
        program: &dyn crate::CheckingInput,
        call_frames: Option<&crate::CallFrameResolver<'_>>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        analyze_default_domain_writes_with_frames(
            program,
            call_frames,
            diagnostics,
            &mut Vec::new(),
        );
    }

    fn analyze_default_domain_writes_with_frames(
        program: &dyn crate::CheckingInput,
        call_frames: Option<&crate::CallFrameResolver<'_>>,
        diagnostics: &mut Vec<Diagnostic>,
        crash_sites: &mut Vec<OpenInvariantCrashSite>,
    ) {
        let parameter_call_attempts = ParameterCallAttempts::default();
        let state_to_machine = state_to_machine_index(program);
        let no_summaries = symbols::SymbolKeyMap::default();

        let mut summaries: symbols::SymbolKeyMap<symbols::SymbolHandle, Vec<String>> =
            symbols::SymbolKeyMap::default();
        let machine_summaries: Vec<(symbols::SymbolHandle, Vec<String>)> =
            crate::checking_thread_pool().install(|| {
                use rayon::prelude::*;
                program
                    .machines()
                    .par_iter()
                    .filter_map(|machine| {
                        let states = program.machine_states(machine);
                        if states.is_empty() {
                            return None;
                        }
                        let callee_binding = program
                            .machine_data_binding(machine)
                            .map(|name| name.as_str())?;
                        let mut throwaway = Vec::new();
                        let mut throwaway_crash_sites = Vec::new();
                        let (exits, terminal): (Vec<Vec<String>>, Vec<usize>) = if states.len() == 1
                        {
                            (
                                vec![
                                    walk_state(
                                        program,
                                        call_frames,
                                        &parameter_call_attempts,
                                        machine,
                                        &states[0],
                                        &[],
                                        &[],
                                        &[],
                                        &no_summaries,
                                        &state_to_machine,
                                        false,
                                        true,
                                        &mut throwaway,
                                        &mut throwaway_crash_sites,
                                        false,
                                        &[],
                                    )
                                    .0,
                                ],
                                vec![0],
                            )
                        } else {
                            let edges = state_edges(program, states);
                            let predecessors = state_predecessors(&edges, states.len());
                            let mut entry: Vec<Vec<String>> = vec![Vec::new(); states.len()];
                            let mut exits: Vec<Vec<String>> = vec![Vec::new(); states.len()];
                            let mut stale = vec![true; states.len()];
                            loop {
                                for (index, state) in states.iter().enumerate() {
                                    if !std::mem::take(&mut stale[index]) {
                                        continue;
                                    }
                                    exits[index] = walk_state(
                                        program,
                                        call_frames,
                                        &parameter_call_attempts,
                                        machine,
                                        state,
                                        &entry[index],
                                        &[],
                                        &[],
                                        &no_summaries,
                                        &state_to_machine,
                                        false,
                                        true,
                                        &mut throwaway,
                                        &mut throwaway_crash_sites,
                                        false,
                                        &[],
                                    )
                                    .0;
                                }
                                let mut changed = false;
                                for (index, current_entry) in entry.iter_mut().enumerate().skip(1) {
                                    let predecessors = &predecessors[index];
                                    if predecessors.is_empty() {
                                        continue;
                                    }
                                    let mut meet: Option<Vec<String>> = None;
                                    for predecessor in predecessors {
                                        let exit = &exits[*predecessor];
                                        meet = Some(match meet {
                                            None => exit.clone(),
                                            Some(current) => current
                                                .into_iter()
                                                .filter(|place| exit.contains(place))
                                                .collect(),
                                        });
                                    }
                                    let meet = meet.unwrap_or_default();
                                    if meet != *current_entry {
                                        *current_entry = meet;
                                        stale[index] = true;
                                        changed = true;
                                    }
                                }
                                if !changed {
                                    break;
                                }
                            }
                            let terminal: Vec<usize> = (0..states.len())
                                .filter(|index| !edges.iter().any(|(from, _)| from == index))
                                .collect();
                            (exits, terminal)
                        };
                        if terminal.is_empty() {
                            return None;
                        }
                        let mut definite: Option<Vec<String>> = None;
                        for index in &terminal {
                            let exit = &exits[*index];
                            definite = Some(match definite {
                                None => exit.clone(),
                                Some(current) => current
                                    .into_iter()
                                    .filter(|place| exit.contains(place))
                                    .collect(),
                            });
                        }
                        let data_rooted: Vec<String> = definite
                            .unwrap_or_default()
                            .into_iter()
                            .filter(|spelling| is_data_rooted(Some(callee_binding), spelling))
                            .collect();
                        (!data_rooted.is_empty()).then_some((machine.symbol, data_rooted))
                    })
                    .collect()
            });
        for (symbol, data_rooted) in machine_summaries {
            summaries.insert(symbol, data_rooted);
        }

        let machine_results: Vec<(
            symbols::SymbolHandle,
            Vec<Diagnostic>,
            Vec<OpenInvariantCrashSite>,
        )> = crate::checking_thread_pool().install(|| {
            use rayon::prelude::*;
            program
                .machines()
                .par_iter()
                .filter_map(|machine| {
                    let states = program.machine_states(machine);
                    if states.is_empty() {
                        return None;
                    }
                    let edges = state_edges(program, states);
                    let born_zero =
                        |index: usize| index == 0 && !edges.iter().any(|(_, to)| *to == 0);
                    let mut entry_established: Vec<Vec<String>> = vec![Vec::new(); states.len()];
                    let mut entry_valuations: Vec<Option<Vec<PlaceValuation>>> =
                        vec![None; states.len()];
                    entry_valuations[0] = Some(Vec::new());
                    let mut entry_windows: Vec<Vec<InvariantWindow>> =
                        vec![Vec::new(); states.len()];
                    let is_terminal = |index: usize| !edges.iter().any(|(from, _)| *from == index);
                    let predecessors = state_predecessors(&edges, states.len());
                    let mut walks: Vec<Option<StateWalk>> =
                        (0..states.len()).map(|_| None).collect();
                    let mut stale = vec![true; states.len()];
                    loop {
                        for (index, state) in states.iter().enumerate() {
                            if !std::mem::take(&mut stale[index]) {
                                continue;
                            }
                            let mut state_diagnostics = Vec::new();
                            let mut state_crash_sites = Vec::new();
                            let (established, valuations, windows) = walk_state(
                                program,
                                call_frames,
                                &parameter_call_attempts,
                                machine,
                                state,
                                &entry_established[index],
                                entry_valuations[index].as_deref().unwrap_or(&[]),
                                &entry_windows[index],
                                &summaries,
                                &state_to_machine,
                                born_zero(index),
                                is_terminal(index),
                                &mut state_diagnostics,
                                &mut state_crash_sites,
                                true,
                                &[],
                            );
                            walks[index] = Some(StateWalk {
                                exit: StateExit {
                                    established,
                                    valuations,
                                    windows,
                                    was_reached: entry_valuations[index].is_some(),
                                },
                                diagnostics: state_diagnostics,
                                crash_sites: state_crash_sites,
                            });
                        }
                        let exits = walks
                            .iter()
                            .map(|walk| &walk.as_ref().expect("every state is walked").exit)
                            .collect::<Vec<_>>();
                        let mut changed = false;
                        for index in 1..states.len() {
                            let predecessors = &predecessors[index];
                            if predecessors.is_empty() {
                                continue;
                            }
                            let mut established_meet: Option<Vec<String>> = None;
                            for predecessor in predecessors {
                                let exit = &exits[*predecessor].established;
                                established_meet = Some(match established_meet {
                                    None => exit.clone(),
                                    Some(current) => current
                                        .into_iter()
                                        .filter(|place| exit.contains(place))
                                        .collect(),
                                });
                            }
                            let established_meet = established_meet.unwrap_or_default();
                            if established_meet != entry_established[index] {
                                entry_established[index] = established_meet;
                                stale[index] = true;
                                changed = true;
                            }
                            let mut window_union: Vec<InvariantWindow> = Vec::new();
                            for predecessor in predecessors {
                                for window in &exits[*predecessor].windows {
                                    if !window_union.contains(window) {
                                        window_union.push(window.clone());
                                    }
                                }
                            }
                            window_union.sort_by(|left, right| {
                                (&left.0, &left.1, left.2.arena_index(), left.2.generation()).cmp(
                                    &(
                                        &right.0,
                                        &right.1,
                                        right.2.arena_index(),
                                        right.2.generation(),
                                    ),
                                )
                            });
                            if window_union != entry_windows[index] {
                                entry_windows[index] = window_union;
                                stale[index] = true;
                                changed = true;
                            }
                            let visited: Vec<usize> = predecessors
                                .iter()
                                .copied()
                                .filter(|predecessor| exits[*predecessor].was_reached)
                                .collect();
                            if visited.is_empty() {
                                continue;
                            }
                            let mut valuation_meet: Option<Vec<PlaceValuation>> = None;
                            for predecessor in visited {
                                let exit = &exits[predecessor].valuations;
                                valuation_meet = Some(match valuation_meet {
                                    None => exit.clone(),
                                    Some(current) => meet_valuations(&current, exit),
                                });
                            }
                            let mut valuation_meet = valuation_meet.unwrap_or_default();
                            canonicalize_valuations(&mut valuation_meet);
                            if entry_valuations[index].as_ref() != Some(&valuation_meet) {
                                entry_valuations[index] = Some(valuation_meet);
                                stale[index] = true;
                                changed = true;
                            }
                        }
                        if !changed {
                            break;
                        }
                    }
                    let mut machine_diagnostics = Vec::new();
                    let mut machine_crash_sites = Vec::new();
                    for walk in walks.into_iter().flatten() {
                        machine_diagnostics.extend(walk.diagnostics);
                        machine_crash_sites.extend(walk.crash_sites);
                    }
                    (!machine_diagnostics.is_empty() || !machine_crash_sites.is_empty())
                        .then_some((machine.symbol, machine_diagnostics, machine_crash_sites))
                })
                .collect()
        });
        let proofs = parameter_call_attempts
            .into_inner()
            .expect("parameter call proofs");
        for (symbol, machine_diagnostics, machine_crash_sites) in machine_results {
            let attempts = proofs
                .iter()
                .filter(|(callee, _)| *callee == symbol)
                .map(|(_, proved)| *proved)
                .collect::<Vec<_>>();
            if !attempts.is_empty() && attempts.iter().all(|proved| *proved) {
                continue;
            }
            diagnostics.extend(machine_diagnostics);
            crash_sites.extend(machine_crash_sites);
        }
    }

    #[derive(Clone, PartialEq, Eq)]
    pub(crate) struct DomainRequirement {
        case: symbols::SymbolHandle,
        fact: typed_trees::domain::ProofFact,
    }

    pub(crate) struct TrackedPlace<'program> {
        spelling: String,
        definition: &'program DataDefinition,
        fields: Vec<(String, Option<symbolic_values::LiteralValue>)>,
        established_fields: Vec<symbols::SymbolHandle>,
        pending_requirements: Vec<DomainRequirement>,
        symbols: Vec<(String, SymbolicValue)>,
        measures: Vec<(String, Option<i128>, Option<i128>)>,
        established: bool,
        born_zero: bool,
        window_open: bool,
        opening_write: Option<source::SourceSpan>,
        elements: Vec<(
            symbols::SymbolHandle,
            Vec<crate::proof_contracts::collection_predicates::IntegerSet>,
        )>,
        element_windows: Vec<symbols::SymbolHandle>,
        active_case: Option<symbols::SymbolHandle>,
        alternatives: Vec<Vec<(String, Option<symbolic_values::LiteralValue>)>>,
    }

    fn established_common_fields(
        program: &dyn crate::CheckingInput,
        definition: &DataDefinition,
    ) -> Vec<symbols::SymbolHandle> {
        program
            .data_members(definition)
            .iter()
            .filter_map(|member| member.as_field().map(|field| field.symbol))
            .collect()
    }

    impl TrackedPlace<'_> {
        fn any_window_open(&self) -> bool {
            self.window_open
                || !self.element_windows.is_empty()
                || (self.established && !self.pending_requirements.is_empty())
        }
    }

    fn place_snapshots<'program>(
        tracked: &[TrackedPlace<'program>],
    ) -> Vec<(
        String,
        Vec<(String, Option<symbolic_values::LiteralValue>)>,
        Option<symbols::SymbolHandle>,
        &'program typed_trees::data::DataDefinition,
    )> {
        tracked
            .iter()
            .map(|place| {
                (
                    place.spelling.clone(),
                    place.fields.clone(),
                    place.active_case,
                    place.definition,
                )
            })
            .collect()
    }

    fn seed_transported_initializer<'program>(
        program: &'program dyn crate::CheckingInput,
        local: &typed_trees::statement::TableLocalData,
        tracked: &mut Vec<TrackedPlace<'program>>,
    ) {
        if !local.initial_value.is_valid() {
            return;
        }
        match program.expression_table.expression(local.initial_value) {
            typed_trees::expression::ExpressionNode::StructLiteral(literal) => {
                let holder = crate::field_domains::owned_nominal_data_definition(
                    program,
                    local.type_reference,
                );
                for field in program.expression_table.struct_fields(literal.fields) {
                    let Some((case, fields, symbols)) = case_snapshot(program, field.value) else {
                        continue;
                    };
                    let Some(holder) = holder else {
                        continue;
                    };
                    let Some(declared) = program.data_members(holder).iter().find_map(|member| {
                        member
                            .as_field()
                            .filter(|candidate| candidate.name.as_str() == field.name.as_str())
                    }) else {
                        continue;
                    };
                    let Some(definition) = crate::field_domains::owned_nominal_data_definition(
                        program,
                        declared.type_reference,
                    ) else {
                        continue;
                    };
                    let spelling = format!("{}.{}", local.name.as_str(), field.name.as_str());
                    if tracked.iter().any(|place| place.spelling == spelling) {
                        continue;
                    }
                    tracked.push(TrackedPlace {
                        spelling,
                        definition,
                        fields,
                        established_fields: established_common_fields(program, definition),
                        pending_requirements: Vec::new(),
                        symbols,
                        measures: Vec::new(),
                        established: true,
                        born_zero: false,
                        window_open: false,
                        opening_write: None,
                        elements: Vec::new(),
                        element_windows: Vec::new(),
                        active_case: Some(case),
                        alternatives: Vec::new(),
                    });
                }
            }
            typed_trees::expression::ExpressionNode::Match(matched) => {
                let arms = program.expression_table.match_arms(matched.arms);
                let snapshots = arms
                    .iter()
                    .filter_map(|arm| case_snapshot(program, arm.value))
                    .collect::<Vec<_>>();
                if snapshots.len() != arms.len() || snapshots.is_empty() {
                    return;
                }
                let case = snapshots[0].0;
                if snapshots.iter().any(|snapshot| snapshot.0 != case) {
                    return;
                }
                let Some(definition) = crate::field_domains::owned_nominal_data_definition(
                    program,
                    local.type_reference,
                ) else {
                    return;
                };
                if tracked
                    .iter()
                    .any(|place| place.spelling == local.name.as_str())
                {
                    return;
                }
                tracked.push(TrackedPlace {
                    spelling: local.name.to_string(),
                    definition,
                    fields: Vec::new(),
                    established_fields: established_common_fields(program, definition),
                    pending_requirements: Vec::new(),
                    symbols: Vec::new(),
                    measures: Vec::new(),
                    established: true,
                    born_zero: false,
                    window_open: false,
                    opening_write: None,
                    elements: Vec::new(),
                    element_windows: Vec::new(),
                    active_case: Some(case),
                    alternatives: snapshots.into_iter().map(|(_, fields, _)| fields).collect(),
                });
            }
            typed_trees::expression::ExpressionNode::Call(call) => {
                let Some((case, fields, symbols)) =
                    machine_return_snapshot(program, call.target_symbol)
                else {
                    return;
                };
                let Some(definition) = crate::field_domains::owned_nominal_data_definition(
                    program,
                    local.type_reference,
                ) else {
                    return;
                };
                if tracked
                    .iter()
                    .any(|place| place.spelling == local.name.as_str())
                {
                    return;
                }
                tracked.push(TrackedPlace {
                    spelling: local.name.to_string(),
                    definition,
                    fields,
                    established_fields: established_common_fields(program, definition),
                    pending_requirements: Vec::new(),
                    symbols,
                    measures: Vec::new(),
                    established: true,
                    born_zero: false,
                    window_open: false,
                    opening_write: None,
                    elements: Vec::new(),
                    element_windows: Vec::new(),
                    active_case: Some(case),
                    alternatives: Vec::new(),
                });
            }
            _ => {}
        }
    }

    fn case_snapshot(
        program: &dyn crate::CheckingInput,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<(
        symbols::SymbolHandle,
        Vec<(String, Option<symbolic_values::LiteralValue>)>,
        Vec<(String, symbolic_values::SymbolicValue)>,
    )> {
        let typed_trees::expression::ExpressionNode::StructLiteral(literal) =
            program.expression_table.expression(expression)
        else {
            return None;
        };
        let case = literal.case_symbol.filter(|symbol| symbol.is_valid())?;
        let (fields, symbols) = symbolic_values::struct_literal_snapshot(program, expression);
        Some((case, fields, symbols))
    }

    fn machine_return_snapshot(
        program: &dyn crate::CheckingInput,
        target: symbols::SymbolHandle,
    ) -> Option<(
        symbols::SymbolHandle,
        Vec<(String, Option<symbolic_values::LiteralValue>)>,
        Vec<(String, symbolic_values::SymbolicValue)>,
    )> {
        let machine = program
            .machine(target)
            .or_else(|| program.machine_containing_state(target))?;
        let state = program.machine_states(machine).first()?;
        let statements = program.statement_table.statements(state.statement_nodes);
        let expression = match statements {
            [typed_trees::statement::StatementNode::Expression(expression)] => *expression,
            [typed_trees::statement::StatementNode::Transition(transition)] => {
                let typed_trees::statement::TransitionTargetNode::Value(expression) =
                    program.statement_table.transition_target(transition.target)
                else {
                    return None;
                };
                *expression
            }
            _ => return None,
        };
        case_snapshot(program, expression)
    }

    fn visit_calls(
        program: &dyn crate::CheckingInput,
        expression: typed_trees::expression::ExpressionHandle,
        calls: &mut Vec<typed_trees::expression::TableCallExpression>,
    ) {
        if !program.expression_table.expression_is_valid(expression) {
            return;
        }
        let node = program.expression_table.expression(expression);
        match &node {
            typed_trees::expression::ExpressionNode::Call(call) => calls.push(call.clone()),
            node => crate::queries::expression_traversal::children(program, node, |child| {
                visit_calls(program, child, calls);
            }),
        }
    }

    fn restore_parameter_write<'program>(
        program: &'program dyn crate::CheckingInput,
        call_frames: Option<&crate::CallFrameResolver<'_>>,
        parameter_call_attempts: &ParameterCallAttempts,
        target: symbols::SymbolHandle,
        arguments: &[typed_trees::expression::ExpressionHandle],
        state_to_machine: &StateToMachine,
        saved_places: &[(
            String,
            Vec<(String, Option<symbolic_values::LiteralValue>)>,
            Option<symbols::SymbolHandle>,
            &'program typed_trees::data::DataDefinition,
        )],
        tracked: &mut Vec<TrackedPlace<'program>>,
        diagnostics: &mut Vec<Diagnostic>,
        active_callees: &[symbols::SymbolHandle],
    ) {
        let Some(machine) = program.machine(target).or_else(|| {
            state_to_machine
                .get(&target)
                .and_then(|symbol| program.machine(*symbol))
        }) else {
            return;
        };
        if active_callees.contains(&machine.symbol) {
            parameter_call_attempts
                .lock()
                .expect("parameter call proofs")
                .push((machine.symbol, false));
            return;
        }
        let Some(state) = program.machine_states(machine).first() else {
            return;
        };
        let parameters = program.state_parameters(state);
        let mut entry_valuations = Vec::new();
        let mut actuals = Vec::new();
        for (parameter, argument) in parameters.iter().zip(arguments) {
            let Some(spelling) = place_queries::write_place_spelling(program, *argument) else {
                continue;
            };
            let Some(place) = saved_places.iter().find(|place| place.0 == spelling) else {
                continue;
            };
            entry_valuations.push((
                parameter.name.as_str().to_string(),
                place.1.clone(),
                place.2,
            ));
            actuals.push((spelling, parameter.name.as_str().to_string()));
        }
        if entry_valuations.is_empty() {
            return;
        }
        let Some(call_frames) = call_frames else {
            parameter_call_attempts
                .lock()
                .expect("parameter call proofs")
                .push((machine.symbol, false));
            return;
        };
        let mut callee_diagnostics = Vec::new();
        let mut crash_sites = Vec::new();
        let mut chain = active_callees.to_vec();
        chain.push(machine.symbol);
        walk_state(
            program,
            Some(call_frames),
            parameter_call_attempts,
            machine,
            state,
            &[],
            &entry_valuations,
            &[],
            &symbols::SymbolKeyMap::default(),
            state_to_machine,
            false,
            true,
            &mut callee_diagnostics,
            &mut crash_sites,
            false,
            &chain,
        );
        let proved = callee_diagnostics.is_empty();
        parameter_call_attempts
            .lock()
            .expect("parameter call proofs")
            .push((machine.symbol, proved));
        if !proved {
            diagnostics.extend(callee_diagnostics);
            return;
        }
        let statements = program.statement_table.statements(state.statement_nodes);
        for statement in statements {
            let typed_trees::statement::StatementNode::Assignment(assignment) = statement else {
                continue;
            };
            let Some(target) = place_queries::write_place_spelling(program, assignment.target)
            else {
                continue;
            };
            for (actual, parameter) in &actuals {
                let Some(field) = target.strip_prefix(&format!("{parameter}.")) else {
                    continue;
                };
                let written = symbolic_values::literal_value(program, assignment.value);
                if let Some(place) = tracked.iter_mut().find(|place| place.spelling == *actual) {
                    if let Some(slot) = place.fields.iter_mut().find(|(name, _)| name == field) {
                        slot.1 = written;
                    } else {
                        place.fields.push((field.to_string(), written));
                    }
                    place.established_fields = established_common_fields(program, place.definition);
                    place.window_open = false;
                    place.opening_write = None;
                    place.pending_requirements.clear();
                    place.established = true;
                } else if let Some(saved) = saved_places.iter().find(|place| place.0 == *actual) {
                    let mut fields = saved.1.clone();
                    if let Some(slot) = fields.iter_mut().find(|(name, _)| name == field) {
                        slot.1 = written;
                    } else {
                        fields.push((field.to_string(), written));
                    }
                    tracked.push(TrackedPlace {
                        spelling: actual.clone(),
                        definition: saved.3,
                        fields,
                        established_fields: established_common_fields(program, saved.3),
                        pending_requirements: Vec::new(),
                        symbols: Vec::new(),
                        measures: Vec::new(),
                        established: true,
                        born_zero: false,
                        window_open: false,
                        opening_write: None,
                        elements: Vec::new(),
                        element_windows: Vec::new(),
                        active_case: saved.2,
                        alternatives: Vec::new(),
                    });
                }
            }
        }
    }

    fn walk_state(
        program: &dyn crate::CheckingInput,
        call_frames: Option<&crate::CallFrameResolver<'_>>,
        parameter_call_attempts: &ParameterCallAttempts,
        machine: &Machine,
        state: &State,
        entry_established: &[String],
        entry_valuations: &[PlaceValuation],
        entry_windows: &[InvariantWindow],
        summaries: &symbols::SymbolKeyMap<symbols::SymbolHandle, Vec<String>>,
        state_to_machine: &StateToMachine,
        born_zero: bool,
        exit_is_terminal: bool,
        diagnostics: &mut Vec<Diagnostic>,
        crash_sites: &mut Vec<OpenInvariantCrashSite>,
        record_crash_sites: bool,
        active_callees: &[symbols::SymbolHandle],
    ) -> (Vec<String>, Vec<PlaceValuation>, Vec<InvariantWindow>) {
        let data_binding = program
            .state_data_binding(machine, state)
            .map(|name| name.as_str().to_owned());
        let data_binding = data_binding.as_deref();
        let mut tracked: Vec<TrackedPlace> = Vec::new();
        let mut poisoned_all = false;
        let mut poisoned_paths: Vec<String> = Vec::new();
        let mut call_established: Vec<String> = Vec::new();
        let mut inherited_windows: Vec<InvariantWindow> = entry_windows.to_vec();
        // A state is left through its transition rows or by running off the end
        // of its statements, and each exit is judged by its own kind: a return
        // must find every `requires` window closed, a crash may abandon one.
        // `left_by_crash` records that a crash row ended the dispatch, so nothing
        // after it runs and the end of the list is not a return. `return_refused`
        // keeps a state whose arms all return from repeating one diagnostic per
        // arm.
        let mut left_by_crash = false;
        let mut return_refused = false;
        let statements = program.statement_table.statements(state.statement_nodes);

        for (statement_ordinal, statement) in statements.iter().enumerate() {
            scan_statement_reads(
                program,
                machine,
                state,
                statement,
                statement_ordinal,
                &tracked,
                entry_established,
                &call_established,
                &inherited_windows,
                diagnostics,
            );
            match statement {
                StatementNode::Assignment(assignment) => {
                    let index_expressions =
                        write_target_index_expressions(program, assignment.target);
                    if expression_contains_call(program, assignment.value)
                        || index_expressions
                            .iter()
                            .any(|index| expression_contains_call(program, *index))
                    {
                        refuse_open_windows(
                            &tracked,
                            &inherited_windows,
                            machine,
                            state,
                            Some(statement_ordinal),
                            "a call",
                            diagnostics,
                        );
                        preserve_proven_establishment(&tracked, &mut call_established);
                        tracked.clear();
                        poisoned_all = true;
                        collect_call_summaries(
                            program,
                            assignment.value,
                            state_to_machine,
                            summaries,
                            &mut call_established,
                        );
                        for index in &index_expressions {
                            collect_call_summaries(
                                program,
                                *index,
                                state_to_machine,
                                summaries,
                                &mut call_established,
                            );
                        }
                    }
                    handle_assignment(
                        program,
                        machine,
                        state,
                        statement,
                        call_frames,
                        &mut tracked,
                        entry_valuations,
                        &inherited_windows,
                        poisoned_all,
                        &mut poisoned_paths,
                        born_zero,
                        statement_ordinal,
                        diagnostics,
                    );
                    if let Some(spelling) =
                        place_queries::write_place_spelling(program, assignment.target)
                        && !spelling.contains("[*]")
                        && !call_established.contains(&spelling)
                    {
                        call_established.push(spelling);
                    }
                    inherited_windows.retain(|(spelling, _, _)| {
                        !tracked
                            .iter()
                            .any(|place| place.spelling == *spelling && !place.any_window_open())
                    });
                }
                StatementNode::Call(call) => {
                    let saved_places = place_snapshots(&tracked);
                    refuse_open_windows(
                        &tracked,
                        &inherited_windows,
                        machine,
                        state,
                        Some(statement_ordinal),
                        "a call",
                        diagnostics,
                    );
                    preserve_proven_establishment(&tracked, &mut call_established);
                    let written =
                        call_frames.and_then(|frames| frames.may_write_paths(machine, call));
                    if let Some(written) = written {
                        tracked.retain(|place| {
                            !written.iter().any(|written| {
                                crate::machine_calls::calls::frame_paths_overlap(
                                    &place.spelling,
                                    written,
                                )
                            })
                        });
                        for written in written {
                            if !poisoned_paths.contains(&written) {
                                poisoned_paths.push(written);
                            }
                        }
                    } else {
                        tracked.clear();
                        poisoned_all = true;
                    }
                    let target_machine = state_to_machine
                        .get(&call.target_symbol)
                        .copied()
                        .unwrap_or_else(symbols::SymbolHandle::invalid);
                    if let Some(established) = summaries.get(&target_machine) {
                        call_established.extend(established.iter().cloned());
                    }
                    restore_parameter_write(
                        program,
                        call_frames,
                        parameter_call_attempts,
                        call.target_symbol,
                        program.expression_table.expression_handles(call.arguments),
                        state_to_machine,
                        &saved_places,
                        &mut tracked,
                        diagnostics,
                        active_callees,
                    );
                }
                StatementNode::Expression(expression) => {
                    let saved_places = place_snapshots(&tracked);
                    if expression_contains_call(program, *expression) {
                        refuse_open_windows(
                            &tracked,
                            &inherited_windows,
                            machine,
                            state,
                            Some(statement_ordinal),
                            "a call",
                            diagnostics,
                        );
                        preserve_proven_establishment(&tracked, &mut call_established);
                        tracked.clear();
                        poisoned_all = true;
                        collect_call_summaries(
                            program,
                            *expression,
                            state_to_machine,
                            summaries,
                            &mut call_established,
                        );
                    }
                    if let typed_trees::expression::ExpressionNode::Call(call) =
                        program.expression_table.expression(*expression)
                    {
                        restore_parameter_write(
                            program,
                            call_frames,
                            parameter_call_attempts,
                            call.target_symbol,
                            program.expression_table.expression_handles(call.arguments),
                            state_to_machine,
                            &saved_places,
                            &mut tracked,
                            diagnostics,
                            active_callees,
                        );
                    }
                }
                StatementNode::LocalData(local) => {
                    let saved_places = place_snapshots(&tracked);
                    if local.initial_value.is_valid()
                        && expression_contains_call(program, local.initial_value)
                    {
                        refuse_open_windows(
                            &tracked,
                            &inherited_windows,
                            machine,
                            state,
                            Some(statement_ordinal),
                            "a call",
                            diagnostics,
                        );
                        preserve_proven_establishment(&tracked, &mut call_established);
                        tracked.clear();
                        poisoned_all = true;
                        collect_call_summaries(
                            program,
                            local.initial_value,
                            state_to_machine,
                            summaries,
                            &mut call_established,
                        );
                        let mut calls = Vec::new();
                        visit_calls(program, local.initial_value, &mut calls);
                        for call in calls {
                            restore_parameter_write(
                                program,
                                call_frames,
                                parameter_call_attempts,
                                call.target_symbol,
                                program.expression_table.expression_handles(call.arguments),
                                state_to_machine,
                                &saved_places,
                                &mut tracked,
                                diagnostics,
                                active_callees,
                            );
                        }
                    }
                    let literal_case = local
                        .initial_value
                        .is_valid()
                        .then(
                            || match program.expression_table.expression(local.initial_value) {
                                typed_trees::expression::ExpressionNode::StructLiteral(literal) => {
                                    literal.case_symbol.filter(|symbol| symbol.is_valid())
                                }
                                _ => None,
                            },
                        )
                        .flatten();
                    if local.initial_value.is_valid()
                    && let Some(definition) = crate::field_domains::owned_nominal_data_definition(program, local.type_reference)
                    && program.data_members(definition).iter().any(|member| member.as_variant().is_some())
                    && !tracked.iter().any(|place| place.spelling == local.name.as_str())
                    && let Some(active_case) = literal_case.or_else(|| {
                        crate::proof_contracts::contract_entailment::completed_value_case_at_occurrence(
                            program, machine, state, statement_ordinal, local.initial_value, local.type_reference,
                        )
                    })
                {
                    let (fields, symbols) = symbolic_values::struct_literal_snapshot(
                        program,
                        local.initial_value,
                    );
                tracked.push(TrackedPlace {
                        spelling: local.name.to_string(), definition,
                        fields, established_fields: established_common_fields(program, definition), pending_requirements: Vec::new(), symbols, measures: Vec::new(),
                        established: true, born_zero: false, window_open: false, opening_write: None,
                        elements: Vec::new(), element_windows: Vec::new(), active_case: Some(active_case),
                        alternatives: Vec::new(),
                    });
                }
                    seed_transported_initializer(program, local, &mut tracked);
                }
                StatementNode::Transition(transition) => {
                    let is_crash_row = matches!(
                        transition.exit,
                        typed_trees::statement::TransitionExit::Crash(_)
                    );
                    if is_crash_row {
                        left_by_crash |= crate::declarations::transitions::admitted_dispatch_ends(
                            transition,
                            statements.get(statement_ordinal + 1),
                        );
                        if record_crash_sites {
                            let mut open_data = tracked
                                .iter()
                                .filter(|place| place.any_window_open())
                                .map(|place| place.definition.symbol)
                                .chain(
                                    inherited_windows
                                        .iter()
                                        .map(|(_, _, data_symbol)| *data_symbol),
                                )
                                .collect::<Vec<_>>();
                            open_data
                                .sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
                            open_data.dedup();
                            if !open_data.is_empty() {
                                crash_sites.push(OpenInvariantCrashSite {
                                machine: machine.symbol,
                                state: state.symbol,
                                statement_ordinal: u32::try_from(statement_ordinal).expect(
                                    "state-local statement ordinal exceeds crash evidence range",
                                ),
                                open_data,
                            });
                            }
                        }
                    }
                    let evaluated = transition_evaluated_expressions(program, transition);
                    let saved_places = place_snapshots(&tracked);
                    if evaluated
                        .iter()
                        .any(|expression| expression_contains_call(program, *expression))
                    {
                        refuse_open_windows(
                            &tracked,
                            &inherited_windows,
                            machine,
                            state,
                            Some(statement_ordinal),
                            "a call",
                            diagnostics,
                        );
                        preserve_proven_establishment(&tracked, &mut call_established);
                        tracked.clear();
                        poisoned_all = true;
                        for expression in &evaluated {
                            collect_call_summaries(
                                program,
                                *expression,
                                state_to_machine,
                                summaries,
                                &mut call_established,
                            );
                        }
                    }
                    let mut calls = Vec::new();
                    for expression in &evaluated {
                        visit_calls(program, *expression, &mut calls);
                    }
                    for call in calls {
                        restore_parameter_write(
                            program,
                            call_frames,
                            parameter_call_attempts,
                            call.target_symbol,
                            program.expression_table.expression_handles(call.arguments),
                            state_to_machine,
                            &saved_places,
                            &mut tracked,
                            diagnostics,
                            active_callees,
                        );
                    }
                    // An ordinary row of a state with no sibling-state edge
                    // leaves the machine, so it is a return and is checked
                    // here, where the row runs. Waiting for the end of the list
                    // would let a later crash row answer for it.
                    if exit_is_terminal && !is_crash_row && !left_by_crash && !return_refused {
                        let reported = diagnostics.len();
                        refuse_open_windows(
                            &tracked,
                            &inherited_windows,
                            machine,
                            state,
                            None,
                            "return",
                            diagnostics,
                        );
                        return_refused = diagnostics.len() > reported;
                    }
                }
                _ => {}
            }
        }

        if exit_is_terminal && !left_by_crash && !return_refused {
            refuse_open_windows(
                &tracked,
                &inherited_windows,
                machine,
                state,
                None,
                "return",
                diagnostics,
            );
        }

        let mut exit_established: Vec<String> = entry_established.to_vec();
        exit_established.extend(
            call_established
                .iter()
                .filter(|place| is_data_rooted(data_binding, place))
                .cloned(),
        );
        exit_established.extend(
            tracked
                .iter()
                .filter(|place| place.established && is_data_rooted(data_binding, &place.spelling))
                .map(|place| place.spelling.clone()),
        );
        if let Some(data_binding) = data_binding
            && attached_value_established(
                program,
                machine,
                state,
                &tracked,
                entry_established,
                &call_established,
            )
        {
            exit_established.push(data_binding.to_owned());
        }
        exit_established.sort();
        exit_established.dedup();

        let mut exit_valuations: Vec<PlaceValuation> = tracked
            .iter()
            .filter(|place| is_data_rooted(data_binding, &place.spelling))
            .map(|place| {
                (
                    place.spelling.clone(),
                    place.fields.clone(),
                    place.active_case,
                )
            })
            .collect();
        if !poisoned_all {
            for (spelling, fields, active_case) in entry_valuations {
                let poisoned = poisoned_paths
                    .iter()
                    .any(|written| place_queries::place_spellings_may_overlap(spelling, written));
                if !poisoned && !exit_valuations.iter().any(|(name, _, _)| name == spelling) {
                    exit_valuations.push((spelling.clone(), fields.clone(), *active_case));
                }
            }
        }
        let mut exit_windows = inherited_windows;
        for place in tracked.iter().filter(|place| {
            place.any_window_open() && is_data_rooted(data_binding, &place.spelling)
        }) {
            if !exit_windows
                .iter()
                .any(|(spelling, _, _)| *spelling == place.spelling)
            {
                exit_windows.push((
                    place.spelling.clone(),
                    place.definition.name.as_str().to_owned(),
                    place.definition.symbol,
                ));
            }
        }
        (exit_established, exit_valuations, exit_windows)
    }
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
    pub(crate) mod integer_membership {
        use super::closed_integer_index_bindings;
        use crate::proof_contracts::contract_entailment::scoped_arithmetic_implication_for_declaration;
        use crate::{
            ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
            ScopedArithmeticValue, StrictArithmeticImplicationJudgment,
        };
        use language_semantics::SemanticDomainId;
        use symbols::SymbolHandle;
        use typed_trees::domain::ProofFact;
        use typed_trees::expression::ExpressionNode;
        use typed_trees::types::TypeReferenceHandle;

        pub fn integer_domain_membership(
            program: &dyn crate::CheckingInput,
            domain_symbol: SymbolHandle,
            arguments: &[TypeReferenceHandle],
            semantic_domain: SemanticDomainId,
            value: &ScopedArithmeticValue,
        ) -> StrictArithmeticImplicationJudgment {
            membership(
                program,
                domain_symbol,
                arguments,
                semantic_domain,
                value,
                &mut Vec::new(),
            )
        }

        fn membership(
            program: &dyn crate::CheckingInput,
            domain_symbol: SymbolHandle,
            arguments: &[TypeReferenceHandle],
            semantic_domain: SemanticDomainId,
            value: &ScopedArithmeticValue,
            active_domains: &mut Vec<SymbolHandle>,
        ) -> StrictArithmeticImplicationJudgment {
            use StrictArithmeticImplicationJudgment::{Proven, Refuted, Unknown};

            if !domain_symbol.is_valid() || active_domains.contains(&domain_symbol) {
                return Unknown;
            }
            let Some(definition) = program.domain_definition(domain_symbol) else {
                return Unknown;
            };
            if !definition.establishment_routes.is_empty()
                || !program
                    .primitive_type_reference(definition.target_type)
                    .is_some_and(|primitive| primitive.is_fixed_integer())
            {
                return Unknown;
            }
            let Some(indices) =
                closed_integer_index_bindings(program, definition, arguments, semantic_domain)
            else {
                return Unknown;
            };
            active_domains.push(domain_symbol);
            let judgment = (|| {
                if let Some(alias) = &definition.alias {
                    if alias.constituents.is_empty() {
                        return Unknown;
                    }
                    for constituent in &alias.constituents {
                        let Some(nested) = program.domain_definition(constituent.domain_symbol)
                        else {
                            return Unknown;
                        };
                        let judgment = membership(
                            program,
                            nested.symbol,
                            &[],
                            nested.semantic_id,
                            value,
                            active_domains,
                        );
                        if judgment != Proven {
                            return judgment;
                        }
                    }
                    return Proven;
                }
                if !definition.predicate_presence.is_present() {
                    return if crate::field_domains::domain_is_vacuous(
                        program,
                        definition.symbol,
                        &mut Vec::new(),
                    ) {
                        Proven
                    } else {
                        Unknown
                    };
                }
                let mut bindings = indices
                    .into_iter()
                    .map(|(symbol, value)| ScopedArithmeticBinding {
                        binder: ScopedArithmeticBinder::Symbol(symbol),
                        value: ScopedArithmeticValue::Integer(value),
                    })
                    .collect::<Vec<_>>();
                bindings.push(ScopedArithmeticBinding {
                    binder: ScopedArithmeticBinder::Symbol(definition.subject_symbol),
                    value: value.clone(),
                });
                for fact in program.proof_facts.span_or_empty(definition.facts) {
                    let judgment = match fact {
                        ProofFact::Expression(expression) => {
                            scoped_arithmetic_implication_for_declaration(
                                program,
                                definition.symbol,
                                &[],
                                &ScopedArithmeticExpression {
                                    owner: definition.symbol,
                                    expression: *expression,
                                    bindings: bindings.clone(),
                                },
                            )
                        }
                        ProofFact::Membership(fact) => {
                            let nested_value = if subject_value(program, definition, fact.value) {
                                value.clone()
                            } else {
                                ScopedArithmeticValue::Term(ScopedArithmeticExpression {
                                    owner: definition.symbol,
                                    expression: fact.value,
                                    bindings: bindings.clone(),
                                })
                            };
                            membership(
                                program,
                                fact.domain_symbol,
                                program
                                    .type_reference_table
                                    .type_reference_handles(fact.domain_arguments),
                                parent_instance(program, fact),
                                &nested_value,
                                active_domains,
                            )
                        }
                        ProofFact::Proposition(_) => Unknown,
                    };
                    if judgment == Refuted || judgment == Unknown {
                        return judgment;
                    }
                }
                Proven
            })();
            active_domains.pop();
            judgment
        }

        fn subject_value(
            program: &dyn crate::CheckingInput,
            definition: &typed_trees::domain::DomainDefinition,
            expression: typed_trees::expression::ExpressionHandle,
        ) -> bool {
            let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
                return false;
            };
            let members = program.expression_table.name_path_members(path.members);
            if members.len() != 1 {
                return false;
            }
            path.symbol == definition.subject_symbol || members[0].as_str() == "self"
        }

        fn parent_instance(
            program: &dyn crate::CheckingInput,
            fact: &typed_trees::domain::ProofMembershipFact,
        ) -> SemanticDomainId {
            if fact.semantic_domain.is_valid() {
                return fact.semantic_domain;
            }
            let Some(parent) = program.domain_definition(fact.domain_symbol) else {
                return SemanticDomainId::NULL;
            };
            if parent.index_arguments.is_empty() && fact.domain_arguments.is_empty() {
                parent.semantic_id
            } else {
                SemanticDomainId::NULL
            }
        }
    }
    pub(crate) use closed_instances::closed_integer_index_bindings;
    pub use integer_membership::integer_domain_membership;
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
