//! Read structural equality of the proof-only IEEE meanings. Operands keep
//! their scalar sources; neither an IEEE comparison nor equality of raw bits
//! denotes this proposition.

use crate::lowering::{Unserved, refuse};
use crate::source_state::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use crate::source_state::signature::SignatureContractKind;
use crate::source_state::types::{TypeReferenceHandle, TypeReferenceNode};
use language_core::core_vocabulary::CoreVocabulary;
use language_core::sealed_intrinsics::{CompilerPrimitive, SealedIntrinsic};
use numerics::float_projection::FloatProjectionOperation;
use numerics::float_semantics::{FloatSemanticOperation, FloatSemanticValueKind as Kind};
use semantic_vocabulary::{
    FloatMeaningClause, FloatMeaningClauseOperand as Operand, FloatMeaningClauseValue as Value,
    FloatMeaningSlot as Slot, IeeeFloatFormat as Format, IntegerType, MachineId, Proposition,
    ScalarType,
};
use symbols::{FixedCoreDeclaration, SymbolHandle};
use terminal_check_tables::{Checked, Construct, Refusal};

use super::{
    Program,
    clause_calls::{Unfolded, callee_scope},
    contracts::ClauseScope,
};

/// The callable is selected by its sealed file, checked signature and catalog
/// commitment. An ordinary checked machine takes the body-reading route.
enum Primitive {
    Projection(FloatProjectionOperation, ScalarType),
    Application(FloatSemanticOperation),
}

fn unavailable(id: MachineId, detail: &'static str) -> Refusal {
    refuse(id, Construct::Machine, Unserved::Contract(detail))
}

fn intern(id: MachineId, values: &mut Vec<Value>, value: Value) -> Checked<Slot> {
    let position = values
        .iter()
        .position(|known| *known == value)
        .unwrap_or_else(|| {
            let position = values.len();
            values.push(value);
            position
        });
    u32::try_from(position)
        .map(Slot)
        .map_err(|_| unavailable(id, "too many float meaning rows"))
}

impl Program<'_> {
    pub(super) fn float_meaning_identity(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> Checked<Proposition> {
        let mut values = Vec::new();
        let left = self.meaning_term(id, scope, left, &mut values, None)?;
        let right = self.meaning_term(id, scope, right, &mut values, None)?;
        let clause = FloatMeaningClause {
            values,
            left: left.min(right),
            right: left.max(right),
        };
        clause
            .validate()
            .map_err(|detail| unavailable(id, detail))?;
        Ok(Proposition::FloatMeaningEqual(clause))
    }

    fn meaning_term(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        term: ExpressionHandle,
        values: &mut Vec<Value>,
        result: Option<Slot>,
    ) -> Checked<Slot> {
        let typed = self.typed;
        if !typed.expression_table.expression_is_valid(term) {
            return Err(unavailable(id, "missing float meaning subject"));
        }
        let call = match typed.expression_table.expression(term) {
            ExpressionNode::Name(name)
                if name.reserved_result_of.is_valid() && !name.static_application.is_valid() =>
            {
                self.require_result_owner(id, scope, name.reserved_result_of)?;
                return result.ok_or_else(|| {
                    unavailable(id, "float meaning result outside its return clause")
                });
            }
            ExpressionNode::Call(call) => call,
            _ => {
                return Err(unavailable(
                    id,
                    "float meaning without a projection or semantic application",
                ));
            }
        };
        let machine = self.clause_call_machine(id, call)?;
        match self.meaning_primitive(id, machine, call.target_symbol)? {
            Some(Primitive::Projection(projection, expected)) => {
                let [argument] = typed.expression_table.expression_handles(call.arguments) else {
                    return Err(unavailable(id, "float projection without its one operand"));
                };
                let (source, supplied) = self.term(id, scope, *argument, Some(expected))?;
                if supplied.scalar() != Some(expected) {
                    return Err(unavailable(
                        id,
                        "float projection changes its source carrier",
                    ));
                }
                intern(
                    id,
                    values,
                    Value::Projection {
                        source,
                        contract: projection.contract_identity(),
                    },
                )
            }
            Some(Primitive::Application(operation)) => {
                self.meaning_application(id, scope, operation, call, values, result)
            }
            None => self.unfold_meaning(id, scope, call, values),
        }
    }

    fn meaning_type(&self, reference: TypeReferenceHandle) -> Option<SymbolHandle> {
        if !self.typed.contains_type_reference(reference) {
            return None;
        }
        match self.typed.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
            _ => None,
        }
    }

    /// Classify the declaration itself, whether reached from a proof clause
    /// or independently during package review. Neither path needs a runtime ABI.
    fn meaning_primitive(
        &mut self,
        id: MachineId,
        machine: SymbolHandle,
        target: SymbolHandle,
    ) -> Checked<Option<Primitive>> {
        let typed = self.typed;
        let Some(SealedIntrinsic::CompilerPrimitive(
            family @ (CompilerPrimitive::FloatProjection | CompilerPrimitive::FloatSemantics),
        )) = typed.symbols.sealed_intrinsic(machine)
        else {
            return Ok(None);
        };
        let (definition, entry) = self.declaration(id, machine)?;
        if definition.supply_mode != source_declarations::MachineSupplyMode::CompilerProvided
            || definition.body_is_present
            || (target != machine && target != entry.symbol)
            || !self.returns_float_meaning(entry.return_type)
        {
            return Err(unavailable(
                id,
                "float meaning call does not select its canonical declaration",
            ));
        }
        let path = typed.symbols.display_path(machine, "::");
        let (namespace, name) = path
            .rsplit_once("::")
            .ok_or_else(|| unavailable(id, "float meaning primitive without its namespace"))?;
        let parameters = typed.state_parameters(entry);
        if parameters
            .iter()
            .any(|parameter| parameter.is_const || parameter.is_mutable)
        {
            return Err(unavailable(
                id,
                "float meaning primitive changes its value parameter",
            ));
        }
        let primitive = if family == CompilerPrimitive::FloatProjection {
            let projection = FloatProjectionOperation::from_source_identity(namespace, name)
                .ok_or_else(|| unavailable(id, "float projection outside its sealed catalog"))?;
            let Some([parameter]) = parameters.as_array() else {
                return Err(unavailable(
                    id,
                    "float projection without its one declared parameter",
                ));
            };
            let declared = self
                .meaning_type(parameter.type_reference)
                .and_then(|ty| typed.symbols.builtin_type_atom(ty));
            let format = match (projection, declared) {
                (FloatProjectionOperation::Meaning32, Some(symbols::BuiltinTypeAtom::F32)) => {
                    Format::Binary32
                }
                (FloatProjectionOperation::Meaning64, Some(symbols::BuiltinTypeAtom::F64)) => {
                    Format::Binary64
                }
                _ => {
                    return Err(unavailable(
                        id,
                        "float projection changes its declared source carrier",
                    ));
                }
            };
            Primitive::Projection(projection, ScalarType::IeeeFloat(format))
        } else {
            let signature = parameters
                .iter()
                .map(|parameter| {
                    let ty = self
                        .meaning_type(parameter.type_reference)
                        .ok_or_else(|| unavailable(id, "unknown float semantic parameter type"))?;
                    match typed.core_vocabulary(ty) {
                        Some(CoreVocabulary::FloatFormat) => Ok(Kind::Format),
                        Some(CoreVocabulary::FloatMeaning) => Ok(Kind::Meaning),
                        _ => typed
                            .symbols
                            .builtin_type_atom(ty)
                            .and_then(|t| t.fixed_integer())
                            .map(Kind::Integer)
                            .ok_or_else(|| {
                                unavailable(id, "unknown float semantic parameter carrier")
                            }),
                    }
                })
                .collect::<Checked<Vec<_>>>()?;
            let op = FloatSemanticOperation::from_source_identity(
                namespace,
                name,
                &signature,
                Kind::Meaning,
            )
            .ok_or_else(|| {
                unavailable(
                    id,
                    "float meaning application outside its sealed catalog signature",
                )
            })?;
            Primitive::Application(op)
        };
        self.admitted_declarations.include(machine);
        Ok(Some(primitive))
    }

    /// Named constants are materialized before this reader. Read the format
    /// from the checked record, including the declaration attached to this
    /// particular use. An equivalent authored record has the same meaning.
    fn meaning_format(&self, id: MachineId, argument: ExpressionHandle) -> Checked<Format> {
        use crate::declaration_selection::AuthoredDeclarationSelectionKind::StaticPathSegment;
        use application_identity::const_value::decode_declaration_value;

        let typed = self.typed;
        let expressions = &typed.expression_table;
        let unknown = || {
            unavailable(
                id,
                "float semantic format outside the admitted core records",
            )
        };
        let mut selected = match expressions.expression(argument) {
            ExpressionNode::Name(name) if !name.static_application.is_valid() => {
                name.resolved_symbol()
            }
            ExpressionNode::StructLiteral(_) => None,
            _ => return Err(unknown()),
        };
        let source = expressions.source_span(argument);
        for occurrence in expressions.authored_selection_occurrences(argument) {
            let row = typed
                .authored_declaration_selections()
                .get(occurrence)
                .ok_or_else(unknown)?;
            if row.kind() != StaticPathSegment || row.source_span() != source {
                continue;
            }
            let Some(symbol) = row
                .target()
                .as_resolved()
                .map(|target| target.selected_symbol())
            else {
                continue;
            };
            if typed.const_declaration_by_symbol(symbol).is_some()
                && selected
                    .replace(symbol)
                    .is_some_and(|previous| previous != symbol)
            {
                return Err(unknown());
            }
        }
        let selected = selected
            .map(|symbol| {
                typed
                    .const_declaration_by_symbol(symbol)
                    .ok_or_else(unknown)
            })
            .transpose()?;
        let materialized = match expressions.expression(argument) {
            ExpressionNode::Name(_) => selected.ok_or_else(unknown)?.materialized_initializer,
            _ => argument,
        };
        let matches = |constant: &crate::source_state::constant::ConstDeclaration| {
            self.meaning_type(constant.declared_type)
                .and_then(|symbol| typed.core_vocabulary(symbol))
                == Some(CoreVocabulary::FloatFormat)
                && constant
                    .evaluated_value
                    .as_ref()
                    .and_then(numeric_literals::EvaluatedConstant::encoding)
                    .and_then(decode_declaration_value)
                    .is_some_and(|value| {
                        typed.canonical_const_materialization_matches(
                            constant.declared_type,
                            materialized,
                            &value,
                        )
                    })
        };
        let format_of = |symbol| match typed.symbols.core_vocabulary(symbol) {
            Some(CoreVocabulary::FloatBinary32) => Some(Format::Binary32),
            Some(CoreVocabulary::FloatBinary64) => Some(Format::Binary64),
            _ => None,
        };
        if let Some(constant) = selected {
            if !matches(constant) {
                return Err(unavailable(
                    id,
                    "float format record differs from its selected constant",
                ));
            }
            if let Some(format) = format_of(constant.symbol) {
                return Ok(format);
            }
        }
        typed
            .const_declarations()
            .iter()
            .find_map(|constant| {
                let format = format_of(constant.symbol)?;
                matches(constant).then_some(format)
            })
            .ok_or_else(unknown)
    }

    fn meaning_application(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        operation: FloatSemanticOperation,
        call: &TableCallExpression,
        values: &mut Vec<Value>,
        result: Option<Slot>,
    ) -> Checked<Slot> {
        let typed = self.typed;
        let args = typed.expression_table.expression_handles(call.arguments);
        if operation.parameters().len() != args.len() {
            return Err(unavailable(id, "incomplete float semantic signature"));
        }
        let mut operands = Vec::with_capacity(args.len());
        let mut declared_format = None;
        let mut projected_format = None;
        for (kind, &argument) in operation.parameters().iter().zip(args) {
            if !typed.expression_table.expression_is_valid(argument) {
                return Err(unavailable(id, "missing float semantic argument"));
            }
            let operand = match *kind {
                Kind::Format => {
                    let format = self.meaning_format(id, argument)?;
                    declared_format = Some(format);
                    Operand::Format(format)
                }
                Kind::Meaning => {
                    let slot = self.meaning_term(id, scope, argument, values, result)?;
                    projected_format = projected_format.or(values[slot.0 as usize].format());
                    Operand::Meaning(slot)
                }
                Kind::Integer(carrier) => {
                    let expected =
                        ScalarType::Integer(IntegerType::from_fixed(carrier).ok_or_else(|| {
                            unavailable(id, "unknown float semantic fixed carrier")
                        })?);
                    let (term, supplied) = self.term(id, scope, argument, Some(expected))?;
                    if supplied.scalar() != Some(expected) {
                        return Err(unavailable(
                            id,
                            "float semantic integer operand changes carrier",
                        ));
                    }
                    Operand::Integer(term)
                }
                _ => {
                    return Err(unavailable(
                        id,
                        "float meaning catalog argument has no source representation",
                    ));
                }
            };
            operands.push(operand);
        }
        let format = declared_format
            .or(projected_format)
            .ok_or_else(|| unavailable(id, "float meaning application without a precision"))?;
        intern(
            id,
            values,
            Value::Application {
                contract: operation.contract_identity(),
                format,
                operands,
            },
        )
    }

    fn meaning_definition(
        &mut self,
        id: MachineId,
        target: SymbolHandle,
        machine: SymbolHandle,
    ) -> Checked<Unfolded<(SymbolHandle, Vec<ExpressionHandle>)>> {
        self.unfold_definition(id, target, machine, |program, declaration, state| {
            let typed = program.typed;
            // Library namespace membership can identify a candidate target
            // operation. Only a checked body can implement an ordinary view.
            if declaration.supply_mode != source_declarations::MachineSupplyMode::CheckedBody {
                return Err(unavailable(
                    id,
                    "float meaning wrapper has no checked source body",
                ));
            }
            if program
                .meaning_type(state.return_type)
                .and_then(|symbol| typed.core_vocabulary(symbol))
                != Some(CoreVocabulary::FloatMeaning)
            {
                return Err(unavailable(
                    id,
                    "float meaning call returns a different declared type",
                ));
            }
            let mut guarantees = Vec::new();
            for contract in typed
                .machine_contracts(declaration)
                .iter()
                .chain(typed.state_contracts(state))
            {
                if contract.kind != SignatureContractKind::Ensures || contract.binding.is_some() {
                    return Err(unavailable(
                        id,
                        "float meaning call has an unserved requirement or guarantee",
                    ));
                }
                for fact in super::span(
                    |span| typed.proof_facts.span(span),
                    contract.facts,
                    id,
                    "float meaning guarantees",
                )? {
                    guarantees.push(program.fact(id, fact)?);
                }
            }
            // Stage 02 binds reserved result in a machine contract to its
            // entry state, as it does for the ordinary signature reader.
            Ok((state.symbol, guarantees))
        })
    }

    /// A proof-only declaration still owes its own body and guarantees in
    /// whole-package review, even when no executable entry calls it.
    pub(super) fn float_meaning_subject(
        &mut self,
        id: MachineId,
        declaration: &crate::source_state::machine::Machine,
    ) -> Checked<bool> {
        let is_proof = self
            .typed
            .machine_states
            .span(declaration.states)
            .and_then(|states| states.first())
            .is_some_and(|entry| self.returns_float_meaning(entry.return_type));
        if !is_proof {
            return Ok(false);
        }
        let machine = declaration.symbol;
        if self.meaning_primitive(id, machine, machine)?.is_some() {
            return Ok(true);
        }
        let form = self.meaning_definition(id, machine, machine)?;
        self.verify_meaning_wrapper(id, &form)?;
        Ok(true)
    }

    pub(super) fn returns_float_meaning(&self, reference: TypeReferenceHandle) -> bool {
        self.meaning_type(reference)
            .and_then(|name| self.typed.core_vocabulary(name))
            == Some(CoreVocabulary::FloatMeaning)
    }

    fn unfold_meaning(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        call: &TableCallExpression,
        values: &mut Vec<Value>,
    ) -> Checked<Slot> {
        let machine = self.clause_call_machine(id, call)?;
        let mut unfolded = self.meaning_definition(id, call.target_symbol, machine)?;
        self.verify_meaning_wrapper(id, &unfolded)?;
        self.bind_call_arguments(id, scope, call, &mut unfolded.bound)?;
        let (slot, _) = self.within_unfolding(id, machine, unfolded.bound.len(), |program| {
            let actual = callee_scope(&unfolded.bound, machine);
            program.meaning_term(id, &actual, unfolded.body, values, None)
        })?;
        self.retain_call_arguments(id, unfolded.bound)?;
        Ok(slot)
    }

    /// A literal or two equal actuals can make a false declaration appear
    /// true at one call. Judge the body once with distinct, unconstrained
    /// formals; only the body instantiated at the call enters its proof row.
    fn verify_meaning_wrapper(
        &mut self,
        id: MachineId,
        unfolded: &Unfolded<(SymbolHandle, Vec<ExpressionHandle>)>,
    ) -> Checked<()> {
        if self.admitted_bodies.contains(unfolded.machine) {
            return Ok(());
        }
        self.within_unfolding(id, unfolded.machine, unfolded.bound.len(), |program| {
            let mut nested = callee_scope(&unfolded.bound, unfolded.machine);
            let (proof, pending) = program.with_clause_arguments(id, |program| {
                let mut values = Vec::new();
                let body = program.meaning_term(id, &nested, unfolded.body, &mut values, None)?;
                nested.owner = unfolded.result.0;
                nested.formal_fields = true;
                for &expression in &unfolded.result.1 {
                    let typed = program.typed;
                    let ExpressionNode::Call(call) = typed.expression_table.expression(expression)
                    else {
                        return Err(unavailable(
                            id,
                            "float meaning wrapper has an unserved guarantee",
                        ));
                    };
                    if typed
                        .symbols
                        .fixed_core_declaration_for_symbol(call.target_symbol)
                        != Some(FixedCoreDeclaration::Identity)
                    {
                        return Err(unavailable(
                            id,
                            "float meaning guarantee has no exact identity",
                        ));
                    }
                    let (ty, [left, right], _) =
                        program.identity_inputs(id, &nested, expression, call)?;
                    if typed.core_vocabulary(ty) != Some(CoreVocabulary::FloatMeaning) {
                        return Err(unavailable(
                            id,
                            "float meaning guarantee changes its subject type",
                        ));
                    }
                    let left = program.meaning_term(id, &nested, left, &mut values, Some(body))?;
                    let right =
                        program.meaning_term(id, &nested, right, &mut values, Some(body))?;
                    if left != right {
                        return Err(unavailable(
                            id,
                            "float meaning wrapper does not establish its result identity",
                        ));
                    }
                }
                Ok(FloatMeaningClause {
                    values,
                    left: body,
                    right: body,
                })
            })?;
            // The established identities may reuse the body's terms. A guarantee
            // introducing independent terms needs a separate formation proof.
            proof.validate().map_err(|detail| unavailable(id, detail))?;
            if !pending.is_empty()
                || proof
                    .sources()
                    .any(super::contracts::needs_call_argument_formation)
            {
                return Err(unavailable(
                    id,
                    "float meaning wrapper requires a separate scalar formation proof",
                ));
            }
            Ok(())
        })?;
        Ok(())
    }
}
