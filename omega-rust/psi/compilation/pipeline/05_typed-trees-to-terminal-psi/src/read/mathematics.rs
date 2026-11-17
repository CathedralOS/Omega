//! Transparent source mathematics in callable premises. This only translates
//! typed source expressions; the ordinary prover and kernel decide their truth.
//! Address interpretation follows the ordinary program carrier or the binding's
//! retained mathematical model, including through nested definitions.

use semantic_vocabulary::{IntegerMathTerm, IntegerValue, MachineId, Proposition, ScalarType};
use symbols::{BuiltinFunction, BuiltinTypeMember, SymbolHandle};
use terminal_check_tables::{Checked, Construct};
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableCallExpression,
};
use typed_trees::mathematical::{MathematicalBody, MathematicalDefinition, MathematicalType};
use typed_trees::types::TypeReferenceNode;

use super::{
    Program,
    contracts::{ClauseScope, ResultTerm},
    logical::{MathematicalAddressContext, MathematicalInputs},
};
use crate::{Unserved, refuse};

/// Embedding reads an integer's value without invoking a carrier operator.
/// Domain qualifications do not change that value. The term reader and checker
/// still enforce formation of the operand before this total projection.
pub(super) fn integer_embedding(
    id: MachineId,
    term: semantic_vocabulary::ScalarTerm,
    value_type: crate::body::ValueType,
) -> Checked<IntegerMathTerm> {
    let unavailable = || {
        refuse(
            id,
            Construct::Machine,
            Unserved::Contract("non-integer mathematical embedding"),
        )
    };
    let crate::body::ValueType::Scalar(ScalarType::Integer(carrier), _) = value_type else {
        return Err(unavailable());
    };
    if term.scalar_type() != ScalarType::Integer(carrier) {
        return Err(refuse(
            id,
            Construct::Machine,
            Unserved::Contract("mathematical embedding changed its source carrier"),
        ));
    }
    IntegerMathTerm::embed(term).ok_or_else(unavailable)
}

impl<'t> Program<'t> {
    fn mathematical_definition(
        &self,
        symbol: SymbolHandle,
    ) -> Option<(usize, &'t MathematicalDefinition)> {
        if !self.typed.symbols.symbols().nodes().is_valid(symbol)
            || self.typed.symbols.get(symbol).kind != symbols::SymbolKind::MathematicalDefinition
        {
            return None;
        }
        self.typed
            .mathematical_definitions()
            .iter()
            .enumerate()
            .find(|(_, definition)| definition.symbol == symbol)
    }

    pub(super) fn address_bound(&self, expression: ExpressionHandle) -> bool {
        let symbol = match self.typed.expression_table.expression(expression) {
            ExpressionNode::Name(name) => name.symbol,
            ExpressionNode::Member(member) => member.selection.field_symbol(),
            _ => return false,
        };
        self.typed.symbols.builtin_type_member(symbol) == Some(BuiltinTypeMember::AddressBound)
    }

    pub(super) fn is_mathematical_term(&self, expression: ExpressionHandle) -> bool {
        if !expression.is_valid() {
            return false;
        }
        if self.address_bound(expression) {
            return true;
        }
        match self.typed.expression_table.expression(expression) {
            ExpressionNode::Call(call) => {
                self.typed
                    .symbols
                    .builtin_function_for_symbol(call.target_symbol)
                    == Some(BuiltinFunction::IntegerEmbed)
                    || self.mathematical_definition(call.target_symbol).is_some()
            }
            ExpressionNode::Binary(binary) => {
                self.is_mathematical_term(binary.left) || self.is_mathematical_term(binary.right)
            }
            ExpressionNode::Name(name) => self.mathematical_definition(name.symbol).is_some(),
            _ => false,
        }
    }

    fn mathematical_application<'a>(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'a>,
        call: &TableCallExpression,
    ) -> Checked<(
        usize,
        &'t MathematicalDefinition,
        Vec<(SymbolHandle, semantic_vocabulary::ScalarTerm)>,
    )> {
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Contract(what));
        let (definition_index, definition) = self
            .mathematical_definition(call.target_symbol)
            .ok_or_else(|| {
                unserved("call in a premise without a transparent mathematical declaration")
            })?;
        // A definition typing could not finish holds a deferred request
        // (`typed_trees::deferred_refusals`); its body is never read through.
        self.served(id, definition.symbol)?;
        if !definition.binders.is_empty()
            || call.receiver.is_valid()
            || !call.machine_arguments.is_empty()
            || !call.evidence_arguments.is_empty()
        {
            return Err(unserved("open mathematical premise application"));
        }
        self.applies_nothing(
            id,
            call.target_symbol,
            call.authored_machine_application,
            &call.machine_arguments,
        )?;
        self.direct_selection(
            id,
            call.target_symbol,
            call.authored_call_selection,
            call.authored_call_binding,
        )?;
        if call.requirement_application.is_valid()
            || call.exact_coercion_target.is_valid()
            || call.build_selection.is_valid()
            || call.operational_acknowledgement.acknowledges_suspend
            || call.operational_acknowledgement.acknowledges_block
            || call.static_machine_parameter.is_valid()
            || call.static_requirement_dispatch.is_some()
            || call.quotient_operation.is_some()
            || call.private_layout_operation.is_some()
            || call.installed_interrupt_operation.is_some()
            || call.layout_query.is_some()
            || call.native_code_layout.is_some()
        {
            return Err(unserved(
                "mathematical premise has an unformed special application",
            ));
        }
        let parameters = self.typed.mathematical_parameters(definition.parameters);
        let arguments = self
            .typed
            .expression_table
            .expression_handles(call.arguments);
        if parameters.len() != definition.parameters.len()
            || arguments.len() != call.arguments.len()
            || parameters.len() != arguments.len()
        {
            return Err(unserved("mathematical premise argument correspondence"));
        }
        let mut bound = Vec::with_capacity(parameters.len());
        for (parameter, argument) in parameters.iter().zip(arguments) {
            if !self.typed.mathematical_types.is_valid(parameter.ty) {
                return Err(unserved("mathematical premise input has no formed type"));
            }
            let MathematicalType::Ordinary(reference) = self.typed.mathematical_type(parameter.ty)
            else {
                return Err(unserved("higher-order mathematical premise input"));
            };
            let scalar = super::logical::scalar_type(
                self.typed,
                id,
                *reference,
                MathematicalAddressContext::of(scope),
            )?;
            let (term, actual) = self.term(id, scope, *argument, Some(scalar))?;
            if actual != crate::body::ValueType::Scalar(scalar, crate::body::Qualification::NONE) {
                return Err(unserved("mathematical premise input carrier differs"));
            }
            bound.push((parameter.symbol, term));
        }
        Ok((definition_index, definition, bound))
    }

    pub(super) fn mathematical_clause(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        call: &TableCallExpression,
    ) -> Checked<Proposition> {
        let (definition_index, definition, bound) =
            self.mathematical_application(id, scope, call)?;
        let address = MathematicalAddressContext::of(scope);
        let expanding = scope.logical.map_or(&[][..], |logical| logical.expanding);
        let expression =
            self.mathematical_predicate_definition(id, definition_index, address, expanding)?;
        let mut expanding = expanding.to_vec();
        expanding.push(definition.symbol);
        let actual_inputs = MathematicalInputs::empty();
        let scope = ClauseScope {
            slots: &[],
            values: &[],
            bound: &bound,
            owner: definition.symbol,
            result: ResultTerm::Absent("result in a mathematical predicate"),
            subject: None,
            formal_fields: true,
            logical: Some(actual_inputs.scope(id, address, &expanding)),
        };
        self.clause(id, &scope, expression)
    }

    /// Retain a transparent predicate under its own formals before unfolding
    /// it at a caller. The checker judges these operands independently of
    /// caller facts, including when the subject never uses the declaration.
    pub(super) fn mathematical_predicate_definition(
        &mut self,
        id: MachineId,
        definition_index: usize,
        address: MathematicalAddressContext<'_>,
        expanding: &[SymbolHandle],
    ) -> Checked<ExpressionHandle> {
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Contract(what));
        let definition = self
            .typed
            .mathematical_definitions()
            .get(definition_index)
            .ok_or_else(|| unserved("mathematical predicate has no source declaration"))?;
        if !self
            .typed
            .symbols
            .symbols()
            .nodes()
            .is_valid(definition.symbol)
            || self.typed.symbols.get(definition.symbol).kind
                != symbols::SymbolKind::MathematicalDefinition
        {
            return Err(unserved("mathematical predicate has no exact declaration"));
        }
        if !definition.binders.is_empty() {
            return Err(unserved("open mathematical predicate declaration"));
        }
        let parameters = self.typed.mathematical_parameters(definition.parameters);
        if parameters.len() != definition.parameters.len() {
            return Err(unserved("mathematical predicate formal correspondence"));
        }
        if !self.predicate_result(definition) {
            return Err(unserved(
                "mathematical premise requires a formed Core Strict<0> result",
            ));
        }
        let MathematicalBody::Definition(expression) = definition.body else {
            return Err(unserved("uninterpreted mathematical premise theory"));
        };
        if expanding.contains(&definition.symbol) {
            return Err(unserved("recursive mathematical premise theory"));
        }
        let mut expanding = expanding.to_vec();
        expanding.push(definition.symbol);
        // This environment has only declaration formals. Actual argument terms
        // enter later by exact symbol substitution, without caller assumptions.
        let formal_inputs = MathematicalInputs::new(self.typed, id, parameters, address)?;
        let declaration_scope = ClauseScope {
            slots: &[],
            values: &[],
            bound: &[],
            owner: definition.symbol,
            result: ResultTerm::Absent("result in a mathematical predicate"),
            subject: None,
            formal_fields: true,
            logical: Some(formal_inputs.scope(id, address, &expanding)),
        };
        let formed = self.clause(id, &declaration_scope, expression)?;
        let parameters = formal_inputs.validate(id, &formed)?;
        let output = self
            .predicate_definitions
            .get_mut(definition_index)
            .ok_or_else(|| unserved("mathematical predicate has no declaration output slot"))?;
        if let Some(retained) = output {
            if retained.symbol != definition.symbol
                || retained.parameters != parameters
                || retained.body != formed
            {
                return Err(unserved(
                    "mathematical definition has inconsistent formal meaning",
                ));
            }
        } else {
            *output = Some(super::PredicateDefinition {
                symbol: definition.symbol,
                diagnostic: id,
                parameters,
                body: formed,
            });
        }
        Ok(expression)
    }

    fn predicate_result(&self, definition: &MathematicalDefinition) -> bool {
        if !self.typed.mathematical_types.is_valid(definition.result) {
            return false;
        }
        let MathematicalType::Ordinary(reference) = self.typed.mathematical_type(definition.result)
        else {
            return false;
        };
        if !self
            .typed
            .type_reference_table
            .contains_type_reference(*reference)
        {
            return false;
        }
        let TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            lifetime_arguments,
            ..
        } = self.typed.type_reference_table.type_reference(*reference)
        else {
            return false;
        };
        if self
            .typed
            .symbols
            .fixed_core_declaration_symbol(symbols::FixedCoreDeclaration::Strict)
            != Some(*base_symbol)
            || !lifetime_arguments.is_empty()
            || arguments.len() != 1
        {
            return false;
        }
        let arguments = self
            .typed
            .type_reference_table
            .type_reference_handles(*arguments);
        let [argument] = arguments else { return false };
        if !self
            .typed
            .type_reference_table
            .contains_type_reference(*argument)
        {
            return false;
        }
        let TypeReferenceNode::ConstExpression(expression) =
            self.typed.type_reference_table.type_reference(*argument)
        else {
            return false;
        };
        self.typed.expression_table.expression_is_valid(*expression)
            && matches!(self.typed.expression_table.expression(*expression), ExpressionNode::Integer(literal) if literal.value_u64() == Some(0))
    }

    pub(super) fn logical_mathematical_term(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        expression: ExpressionHandle,
    ) -> Checked<IntegerMathTerm> {
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Contract(what));
        if !expression.is_valid() {
            return Err(unserved("missing mathematical premise term"));
        }
        if self.address_bound(expression) {
            let address = MathematicalAddressContext::of(scope).carrier(id)?;
            let bound = 1_u128
                .checked_shl(u32::from(address.bits()))
                .ok_or_else(|| unserved("address bound outside mathematical literal support"))?;
            return Ok(IntegerMathTerm::literal(IntegerValue::Unsigned(bound)));
        }
        match self.typed.expression_table.expression(expression) {
            ExpressionNode::Call(call)
                if self
                    .typed
                    .symbols
                    .builtin_function_for_symbol(call.target_symbol)
                    == Some(BuiltinFunction::IntegerEmbed) =>
            {
                if call.receiver.is_valid()
                    || call.requirement_application.is_valid()
                    || call.exact_coercion_target.is_valid()
                    || call.build_selection.is_valid()
                    || call.operational_acknowledgement.acknowledges_suspend
                    || call.operational_acknowledgement.acknowledges_block
                    || !call.evidence_arguments.is_empty()
                    || call.static_machine_parameter.is_valid()
                    || call.static_requirement_dispatch.is_some()
                    || call.quotient_operation.is_some()
                    || call.private_layout_operation.is_some()
                    || call.installed_interrupt_operation.is_some()
                    || call.layout_query.is_some()
                    || call.native_code_layout.is_some()
                {
                    return Err(unserved("integer embedding has an unformed application"));
                }
                self.applies_nothing(
                    id,
                    call.target_symbol,
                    call.authored_machine_application,
                    &call.machine_arguments,
                )?;
                let arguments = self
                    .typed
                    .expression_table
                    .expression_handles(call.arguments);
                let [argument] = arguments else {
                    return Err(unserved("integer embedding argument correspondence"));
                };
                let (term, carrier) = self.term(id, scope, *argument, None)?;
                integer_embedding(id, term, carrier)
            }
            ExpressionNode::Binary(binary) => {
                // Only explicit mathematical arithmetic is total here. A
                // fixed-width partial operand inside a mathematical clause
                // must not be reinterpreted as an unbounded operation.
                if binary.policy != numerics::arithmetic::ArithmeticDomain::Exact
                    || !(self.is_mathematical_term(binary.left)
                        || self.is_mathematical_term(binary.right))
                {
                    return Err(unserved(
                        "mathematical premise contains computational arithmetic",
                    ));
                }
                let left = Box::new(self.logical_mathematical_term(id, scope, binary.left)?);
                let right = Box::new(self.logical_mathematical_term(id, scope, binary.right)?);
                match binary.operator {
                    BinaryOperator::Add => Ok(IntegerMathTerm::Add(left, right)),
                    BinaryOperator::Subtract => Ok(IntegerMathTerm::Subtract(left, right)),
                    BinaryOperator::Multiply => Ok(IntegerMathTerm::Multiply(left, right)),
                    _ => Err(unserved("mathematical premise arithmetic")),
                }
            }
            ExpressionNode::Integer(literal) => {
                let (value, _) = self.literal(id, literal, None)?;
                Ok(IntegerMathTerm::literal(value))
            }
            _ => Err(unserved(
                "mathematical premise term without explicit integer embedding",
            )),
        }
    }
}

#[cfg(test)]
mod address_context_tests {
    use super::*;
    use crate::body::ValueType;
    use crate::read::Slot;
    use numerics::literals::IntegerLiteral;
    use semantic_vocabulary::{ClosedIntegerType, IntegerSign, ScalarAddressModel, ValueId};
    use symbols::{
        BuiltinFunction, BuiltinTypeAtom, FixedCoreDeclaration, SymbolKind, SymbolNameRef,
        SymbolTableBuilder, builtin_function_symbols, builtin_type_member_symbols,
        builtin_type_symbols, fixed_core_declaration_symbols,
    };
    use typed_trees::TypedTrees;
    use typed_trees::expression::{TableBinaryExpression, TableNamePath};
    use typed_trees::mathematical::{MathematicalParameter, MathematicalType};

    fn fixture() -> (
        TypedTrees,
        [SymbolHandle; 2],
        ExpressionHandle,
        ExpressionHandle,
    ) {
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = builder.insert_children(
            root,
            builtin_type_symbols()
                .into_iter()
                .chain(builtin_function_symbols())
                .chain(fixed_core_declaration_symbols())
                .chain([
                    (
                        SymbolKind::MathematicalDefinition,
                        SymbolNameRef::Static("within"),
                    ),
                    (
                        SymbolKind::MathematicalDefinition,
                        SymbolNameRef::Static("outer"),
                    ),
                    (SymbolKind::Machine, SymbolNameRef::Static("caller")),
                ]),
        );
        let first = u32::try_from(
            symbols::BUILTIN_TYPE_COUNT + BuiltinFunction::COUNT + FixedCoreDeclaration::COUNT,
        )
        .unwrap();
        let inner = children.handle_at(first);
        let outer = children.handle_at(first + 1);
        let caller = children.handle_at(first + 2);
        let address_symbol =
            children.handle_at(u32::try_from(BuiltinTypeAtom::Address.ordinal()).unwrap());
        let bound = builder
            .insert_children(
                address_symbol,
                builtin_type_member_symbols(BuiltinTypeAtom::Address),
            )
            .start();
        let parameters = [inner, outer, caller].map(|owner| {
            let rows = builder.insert_children(
                owner,
                [
                    (SymbolKind::Parameter, SymbolNameRef::Static("base")),
                    (SymbolKind::Parameter, SymbolNameRef::Static("length")),
                ],
            );
            [rows.handle_at(0), rows.handle_at(1)]
        });
        let mut typed = TypedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        let address = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: address_symbol,
            name: "addr".into(),
        });
        let count_symbol = typed
            .symbols
            .builtin_atom_symbol(BuiltinTypeAtom::U64)
            .unwrap();
        let count = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: count_symbol,
            name: "u64".into(),
        });
        let address_type = typed
            .mathematical_types
            .insert(MathematicalType::Ordinary(address));
        let count_type = typed
            .mathematical_types
            .insert(MathematicalType::Ordinary(count));
        let zero = typed
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(0)));
        let zero_type = typed
            .type_reference_table
            .insert(TypeReferenceNode::ConstExpression(zero));
        let arguments = typed
            .type_reference_table
            .insert_type_reference_handles([zero_type]);
        let strict_symbol = typed
            .symbols
            .fixed_core_declaration_symbol(FixedCoreDeclaration::Strict)
            .unwrap();
        let strict = typed
            .type_reference_table
            .insert(TypeReferenceNode::Generic {
                base_symbol: strict_symbol,
                base_name: "core::Strict".into(),
                lifetime_arguments: Vec::new(),
                arguments,
            });
        let result = typed
            .mathematical_types
            .insert(MathematicalType::Ordinary(strict));
        fn name(typed: &mut TypedTrees, symbol: SymbolHandle, spelling: &str) -> ExpressionHandle {
            let mut members = Default::default();
            typed
                .expression_table
                .push_name_path_member(&mut members, spelling.into());
            typed
                .expression_table
                .insert(ExpressionNode::Name(TableNamePath {
                    symbol,
                    head_symbol: symbol,
                    members,
                    ..Default::default()
                }))
        }
        fn call(
            typed: &mut TypedTrees,
            target: SymbolHandle,
            actuals: &[ExpressionHandle],
        ) -> ExpressionHandle {
            let arguments = typed
                .expression_table
                .insert_expression_handles(actuals.iter().copied());
            let target_name = typed.symbols.name(target).into();
            typed
                .expression_table
                .insert(ExpressionNode::Call(TableCallExpression {
                    target_symbol: target,
                    target: target_name,
                    arguments,
                    ..Default::default()
                }))
        }
        let bound = name(&mut typed, bound, "Bound");
        let embed = typed
            .symbols
            .builtin_function_symbol(BuiltinFunction::IntegerEmbed)
            .unwrap();
        let base = name(&mut typed, parameters[0][0], "base");
        let length = name(&mut typed, parameters[0][1], "length");
        let base = call(&mut typed, embed, &[base]);
        let length = call(&mut typed, embed, &[length]);
        let endpoint =
            typed
                .expression_table
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: base,
                    operator: BinaryOperator::Add,
                    right: length,
                    policy: numerics::arithmetic::ArithmeticDomain::Exact,
                }));
        let body = typed
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: endpoint,
                operator: BinaryOperator::LessOrEqual,
                right: bound,
                policy: numerics::arithmetic::ArithmeticDomain::Exact,
            }));
        let base = name(&mut typed, parameters[1][0], "base");
        let length = name(&mut typed, parameters[1][1], "length");
        let outer_body = call(&mut typed, inner, &[base, length]);
        for (symbol, inputs, expression) in [
            (inner, parameters[0], body),
            (outer, parameters[1], outer_body),
        ] {
            let parameters = typed.mathematical_parameters.insert_many([
                MathematicalParameter {
                    symbol: inputs[0],
                    name: "base".into(),
                    ty: address_type,
                    ..Default::default()
                },
                MathematicalParameter {
                    symbol: inputs[1],
                    name: "length".into(),
                    ty: count_type,
                    ..Default::default()
                },
            ]);
            typed.push_mathematical_definition(MathematicalDefinition {
                symbol,
                name: typed.symbols.name(symbol).into(),
                parameters,
                result,
                body: MathematicalBody::Definition(expression),
                ..Default::default()
            });
        }
        let base = name(&mut typed, parameters[2][0], "base");
        let length = name(&mut typed, parameters[2][1], "length");
        let application = call(&mut typed, outer, &[base, length]);
        (typed, parameters[2], application, bound)
    }

    fn expected_predicate(address: semantic_vocabulary::IntegerType) -> Proposition {
        Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::Add(
                Box::new(IntegerMathTerm::MathValue {
                    source_type: address,
                    value: ValueId::new(1).unwrap(),
                }),
                Box::new(IntegerMathTerm::MathValue {
                    source_type: semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 64)
                        .unwrap(),
                    value: ValueId::new(2).unwrap(),
                }),
            ),
            IntegerMathTerm::literal(IntegerValue::Unsigned(1_u128 << address.bits().unwrap())),
        )
    }

    #[test]
    fn ordinary_address_predicates_keep_their_context_through_nested_definitions() {
        let (typed, inputs, application, _) = fixture();
        let id = MachineId::new(1).unwrap();
        let slots = inputs
            .into_iter()
            .zip([
                ScalarType::Integer(
                    semantic_vocabulary::IntegerType::supported_address_representation(),
                ),
                ScalarType::Integer(
                    semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                ),
            ])
            .map(|(symbol, scalar)| Slot {
                symbol,
                value_type: ValueType::plain(scalar),
                assignable: false,
                homed: false,
                erased: false,
                borrowed: None,
                borrowed_regions: crate::body::Regions::NONE,
            })
            .collect::<Vec<_>>();
        let values = [ValueId::new(1), ValueId::new(2)];
        let scope = ClauseScope {
            slots: &slots,
            values: &values,
            bound: &[],
            owner: SymbolHandle::invalid(),
            result: ResultTerm::Absent("entry"),
            subject: None,
            formal_fields: true,
            logical: None,
        };
        let mut program = Program::new(&typed, false);
        let proposition = program.clause(id, &scope, application).unwrap();
        assert_eq!(
            proposition,
            expected_predicate(
                semantic_vocabulary::IntegerType::supported_address_representation()
            )
        );
        assert_eq!(
            program
                .predicate_definitions
                .iter()
                .filter(|row| row.is_some())
                .count(),
            2
        );
        assert!(program.symbols.is_empty());
    }

    #[test]
    fn binding_address_scalars_and_bound_only_terms_require_the_actual_model() {
        let (typed, caller_parameters, application, bound) = fixture();
        let id = MachineId::new(1).unwrap();
        let address_symbol = typed
            .symbols
            .builtin_atom_symbol(BuiltinTypeAtom::Address)
            .unwrap();
        let mut typed = typed;
        let reference = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: address_symbol,
            name: "addr".into(),
        });
        // Distinct 32-bit fixture facts make accidental use of the ordinary
        // 64-bit carrier observable. They provide no native target authority.
        let model = ScalarAddressModel {
            target: "test-address-model".into(),
            carrier: ClosedIntegerType::address(32).unwrap(),
            semantics_version: 1,
            commitment: [7; 32],
        };
        let mut malformed = Vec::new();
        let mut invalid = model.clone();
        invalid.carrier = ClosedIntegerType::new(IntegerSign::Unsigned, 32).unwrap();
        malformed.push(invalid);
        let mut invalid = model.clone();
        invalid.semantics_version = 0;
        malformed.push(invalid);
        let mut invalid = model.clone();
        invalid.commitment = [0; 32];
        malformed.push(invalid);
        for supplied in std::iter::once(None).chain(malformed.iter().map(Some)) {
            let context = MathematicalAddressContext::Binding(supplied);
            assert!(super::super::logical::scalar_type(&typed, id, reference, context).is_err());
            let inputs = MathematicalInputs::empty();
            let scope = ClauseScope {
                slots: &[],
                values: &[],
                bound: &[],
                owner: SymbolHandle::invalid(),
                result: ResultTerm::Absent("entry"),
                subject: None,
                formal_fields: true,
                logical: Some(inputs.scope(id, context, &[])),
            };
            let mut program = Program::new(&typed, false);
            let refusal = program
                .logical_mathematical_term(id, &scope, bound)
                .unwrap_err();
            assert!(
                refusal
                    .detail
                    .contains("address without its binding's mathematical model")
            );
        }
        let context = MathematicalAddressContext::Binding(Some(&model));
        assert_eq!(
            super::super::logical::scalar_type(&typed, id, reference, context).unwrap(),
            ScalarType::Integer(model.carrier.as_checking())
        );
        let inputs = MathematicalInputs::empty();
        let scope = ClauseScope {
            slots: &[],
            values: &[],
            bound: &[],
            owner: SymbolHandle::invalid(),
            result: ResultTerm::Absent("entry"),
            subject: None,
            formal_fields: true,
            logical: Some(inputs.scope(id, context, &[])),
        };
        let mut program = Program::new(&typed, false);
        assert_eq!(
            program
                .logical_mathematical_term(id, &scope, bound)
                .unwrap(),
            IntegerMathTerm::literal(IntegerValue::Unsigned(1_u128 << 32))
        );
        assert!(program.symbols.is_empty());

        let source_parameters =
            typed.mathematical_parameters(typed.mathematical_definitions()[0].parameters);
        let parameters = source_parameters
            .iter()
            .zip(caller_parameters)
            .map(|(formal, symbol)| MathematicalParameter {
                symbol,
                ..formal.clone()
            })
            .collect::<Vec<_>>();
        let inputs = MathematicalInputs::new(&typed, id, &parameters, context).unwrap();
        let scope = ClauseScope {
            logical: Some(inputs.scope(id, context, &[])),
            ..scope
        };
        let mut program = Program::new(&typed, false);
        assert_eq!(
            program.clause(id, &scope, application).unwrap(),
            expected_predicate(model.carrier.as_checking())
        );
        assert!(program.symbols.is_empty());
    }
}
