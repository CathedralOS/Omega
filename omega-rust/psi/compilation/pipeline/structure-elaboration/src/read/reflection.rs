//! Declaration descriptions become ordinary values in the source compiler.
//! The selected declaration and its source stay explicit; no callable body is
//! instantiated, and these byte values grant no permission.

use language_core::core_vocabulary::{CoreVocabulary, ReflectionDeclaration};
use semantic_vocabulary::MachineId;
use terminal_check_tables::{Checked, Construct, Refusal, RefusalReason};
use typed_trees::expression::{ExpressionHandle, TableCallExpression};
use typed_trees::references::StaticArgumentCall;

use super::Program;
use crate::{Expression, ExpressionKind, SourceOrigin, ValueType};

fn invalid(machine: MachineId, detail: impl Into<String>) -> Refusal {
    Refusal {
        reason: RefusalReason::ForeignIdentity,
        machine,
        concerns: Construct::Machine,
        detail: detail.into(),
    }
}

impl Program<'_> {
    fn schema_description(
        &mut self,
        machine: MachineId,
        query: symbols::SymbolHandle,
        call: &TableCallExpression,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        use typed_trees::reflection::SchemaQueryAuthority;
        let fail = |detail| invalid(machine, detail);
        if !self
            .typed
            .expression_table
            .expression_handles(call.arguments)
            .is_empty()
        {
            return Err(fail(
                "a schema query takes one type argument and no runtime values",
            ));
        }
        let application = StaticArgumentCall::Expression(ExpressionHandle::invalid(), call);
        let [selected] = application.arguments(self.typed) else {
            return Err(fail(
                "a schema query requires its one complete type argument",
            ));
        };
        let reference = selected.type_reference;
        if !self
            .typed
            .type_reference_table
            .contains_type_reference(reference)
            || selected.const_literal.is_some()
            || selected.evidence_projection.is_some()
            || selected
                .application
                .as_ref()
                .is_some_and(|application| application.member.is_valid())
        {
            return Err(fail(
                "a schema query lost its exact selected type application",
            ));
        }
        let data = self
            .typed
            .closed_data_definition(reference)
            .ok_or_else(|| fail("a schema query requires its retained data application"))?;
        let author = application.lexical_owner(self.typed);
        let span = self
            .typed
            .symbols
            .symbol_provenance_source_span(author)
            .ok_or_else(|| fail("a schema query lost its actual source author"))?;
        let declared = self
            .typed
            .symbols
            .symbol_provenance_source_span(data.symbol)
            .ok_or_else(|| fail("a schema query lost its declaration source"))?;
        if self.typed.symbols.source_file(span).is_none()
            || self.typed.symbols.source_file(declared).is_none()
            || !self
                .typed
                .symbols
                .source_reference_may_select_symbol(span, data.symbol)
        {
            return Err(fail(
                "a schema query is outside its author's declaration scope",
            ));
        }
        let requester_identity = self
            .typed
            .normalized_hermetic_symbol_identity(author)
            .map_err(|reason| invalid(machine, reason))?;
        let authority = if self
            .typed
            .symbols
            .same_symbol_source_package(author, data.symbol)
        {
            SchemaQueryAuthority::OwningScope { requester_identity }
        } else {
            SchemaQueryAuthority::ForeignScope { requester_identity }
        };
        let graph =
            typed_trees::reflection::construction::construct_semantic_schema_graph_for_type(
                self.typed, reference, authority,
            )
            .map_err(|reason| invalid(machine, reason))?;
        // Describing and forming this type select the same actual application.
        // This constructs no inhabitant and establishes no domain or layout
        // permission.
        self.record_at(machine, reference)?;
        let (_, entry) = self.declaration(machine, query)?;
        let result_type = entry.return_type;
        let result = self
            .typed
            .closed_data_definition(result_type)
            .ok_or_else(|| fail("a schema query lost its result declaration"))?;
        if self.typed.core_vocabulary(result.symbol)
            != Some(CoreVocabulary::Reflection(
                ReflectionDeclaration::TypeSchema,
            ))
        {
            return Err(fail("a schema query lost its toolchain result type"));
        }
        let ValueType::Record(record) = self.value_type(machine, result_type)? else {
            return Err(fail("a schema description requires structured storage"));
        };
        let fields = self.data_type(machine, record)?.fields();
        let [field] = fields else {
            return Err(fail("a schema result has another field list"));
        };
        if field.identity != "storage" || !self.data_type(machine, record)?.cases().is_empty() {
            return Err(fail("a schema result lost its immutable byte storage"));
        }
        let value_type = field.value_type;
        let bytes = graph.runtime_image();
        let value = self.byte_literal(machine, &bytes, Some(value_type), source)?;
        Ok(Expression {
            kind: ExpressionKind::Record {
                fields: vec![(0, value)],
            },
            result: Some(ValueType::Record(record)),
            source,
        })
    }

    pub(super) fn declaration_query(
        &mut self,
        machine: MachineId,
        call: &TableCallExpression,
        source: SourceOrigin,
    ) -> Checked<Option<Expression>> {
        let declaration = self
            .typed
            .machine_by_symbol(call.target_symbol)
            .or_else(|| self.typed.machine_holding_state(call.target_symbol));
        let Some(declaration) = declaration else {
            return Ok(None);
        };
        let Some(CoreVocabulary::Reflection(
            query @ (ReflectionDeclaration::Callable | ReflectionDeclaration::Schema),
        )) = self.typed.core_vocabulary(declaration.symbol)
        else {
            return Ok(None);
        };
        let fail = |detail| invalid(machine, detail);
        if call.receiver.is_valid()
            || !call.evidence_arguments.is_empty()
            || call.static_requirement_dispatch.is_some()
            || call.build_selection.is_valid()
            || call.quotient_operation.is_some()
            || call.private_layout_operation.is_some()
            || call.installed_interrupt_operation.is_some()
            || call.layout_query.is_some()
            || call.native_code_layout.is_some()
            || call.static_machine_parameter.is_valid()
            || call.requirement_application.is_valid()
            || call.exact_coercion_target.is_valid()
            || call.operational_acknowledgement.acknowledges_suspend
            || call.operational_acknowledgement.acknowledges_block
        {
            return Err(fail(
                "declaration descriptions require their ordinary argument form and no other call feature",
            ));
        }
        self.direct_selection(
            machine,
            call.target_symbol,
            call.authored_call_selection,
            call.authored_call_binding,
        )?;
        self.visible_to(machine, declaration.symbol)?;
        if query == ReflectionDeclaration::Schema {
            return self
                .schema_description(machine, declaration.symbol, call, source)
                .map(Some);
        }
        if !call.machine_arguments.is_empty() {
            return Err(fail(
                "callable description accepts no static query arguments",
            ));
        }
        let [operand] = self
            .typed
            .expression_table
            .expression_handles(call.arguments)
        else {
            return Err(fail(
                "callable description requires exactly one declaration path",
            ));
        };
        let selected = self
            .typed
            .declaration_operand_from_expression(*operand)
            .map_err(|reason| invalid(machine, reason))?;
        let author = StaticArgumentCall::Expression(ExpressionHandle::invalid(), call)
            .lexical_owner(self.typed);
        let author_span = self
            .typed
            .symbols
            .symbol_provenance_source_span(author)
            .ok_or_else(|| fail("callable description requires its retained lexical author"))?;
        if !selected.symbol.is_valid()
            || !self
                .typed
                .symbols
                .source_reference_may_select_symbol(author_span, selected.symbol)
        {
            return Err(fail(
                "callable description is outside its author's declaration scope",
            ));
        }
        let (requirement, identities) =
            typed_trees::reflection::construction::callable_description_identity(
                self.typed, &selected, author,
            )
            .map_err(|reason| invalid(machine, reason))?;
        let (_, entry) = self.declaration(machine, declaration.symbol)?;
        let result_type = entry.return_type;
        let data = self
            .typed
            .closed_data_definition(result_type)
            .ok_or_else(|| fail("callable description has no exact result declaration"))?;
        if self.typed.core_vocabulary(data.symbol)
            != Some(CoreVocabulary::Reflection(
                ReflectionDeclaration::CallableInfo,
            ))
        {
            return Err(fail(
                "callable description lost its toolchain result vocabulary",
            ));
        }
        let Some(ValueType::Record(record)) = Some(self.value_type(machine, result_type)?) else {
            return Err(fail(
                "callable description result is not structured storage",
            ));
        };
        let alternative = if requirement {
            "Requirement"
        } else {
            "Machine"
        };
        let mut cases = self
            .data_type(machine, record)?
            .cases()
            .iter()
            .enumerate()
            .filter(|(_, case)| case.identity == alternative);
        let (case_index, case) = cases
            .next()
            .ok_or_else(|| fail("callable description lost its result alternative"))?;
        let case = case.clone();
        if cases.next().is_some() {
            return Err(fail("callable description repeats its result alternative"));
        }
        let mut fields = Vec::new();
        for (name, bytes) in [
            "declaration_identity",
            "callable_identity",
            "source_revision",
        ]
        .into_iter()
        .zip(identities)
        {
            let mut matches = case
                .fields
                .iter()
                .enumerate()
                .filter(|(_, field)| field.identity == name);
            let (position, field) = matches
                .next()
                .ok_or_else(|| fail("callable description lost an identity field"))?;
            if matches.next().is_some() {
                return Err(fail("callable description repeats an identity field"));
            }
            fields.push((
                position,
                self.byte_literal(machine, &bytes, Some(field.value_type), source)?,
            ));
        }
        if fields.len() != case.fields.len() {
            return Err(fail("callable description result has an unexpected field"));
        }
        Ok(Some(Expression {
            kind: ExpressionKind::Case {
                case: case_index,
                fields,
            },
            result: Some(ValueType::Record(record)),
            source,
        }))
    }
}
