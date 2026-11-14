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
    fn field_description(
        &mut self,
        machine: MachineId,
        query: symbols::SymbolHandle,
        call: &TableCallExpression,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        use semantic_vocabulary::{IntegerValue, ScalarType};
        use symbols::SymbolKind;
        use typed_trees::expression::StaticMachineArgument;
        use typed_trees::reflection::construction::{
            construct_semantic_schema_graph, construct_semantic_schema_graph_for_type,
            declaration_source_revision, exact_symbol_identity,
        };
        use typed_trees::reflection::{SchemaNode, SchemaQueryAuthority};
        use typed_trees::type_identity::{IndexIdentityArgument, portable_applied_index_identity};

        let fail = |detail| invalid(machine, detail);
        if !call.machine_arguments.is_empty() {
            return Err(fail("a field query accepts no static query arguments"));
        }
        let [operand] = self
            .typed
            .expression_table
            .expression_handles(call.arguments)
        else {
            return Err(fail("a field query requires exactly one declaration path"));
        };
        let operand = self
            .typed
            .declaration_operand_from_expression(*operand)
            .map_err(|reason| invalid(machine, reason))?;
        let symbols = self.typed.symbols.symbols().nodes();
        let author = StaticArgumentCall::Expression(ExpressionHandle::invalid(), call)
            .lexical_owner(self.typed);
        if !symbols.is_valid(author) {
            return Err(fail("a field query lost its lexical owner"));
        }
        let mut selected = &operand;
        let mut declaring_type = None;
        let mut members = Vec::new();
        while let Some(application) = &selected.application
            && application.member.is_valid()
        {
            if members.contains(&application.member) {
                return Err(fail("a field query has a cyclic member selection"));
            }
            members.push(application.member);
            if !symbols.is_valid(selected.symbol) {
                return Err(fail("a field query lost its supplying declaration"));
            }
            match self.typed.symbols.get(selected.symbol).kind {
                SymbolKind::Data | SymbolKind::TypeAlias => {
                    if declaring_type.is_some() {
                        return Err(fail(
                            "a field query has an unjoined enclosing type application",
                        ));
                    }
                    let mut head = selected.clone();
                    head.application
                        .as_mut()
                        .expect("selected member has an application")
                        .member = arena::Handle::invalid();
                    if !self
                        .typed
                        .type_reference_table
                        .contains_type_reference(head.type_reference)
                        || !self
                            .typed
                            .type_reference_table
                            .operand_lifetime_owner(head.type_reference)
                            .is_valid()
                    {
                        return Err(fail(
                            "a field query lost its prepared supplying type and scope",
                        ));
                    }
                    self.typed
                        .static_operand_region_supplier(&head, author)
                        .map_err(|reason| invalid(machine, reason))?;
                    declaring_type = Some(head.type_reference);
                }
                SymbolKind::Variant
                    if application.arguments.is_empty()
                        && application.lifetime_arguments.is_empty() => {}
                _ => return Err(fail("a field query has a non-type member supplier")),
            }
            selected = self
                .typed
                .expression_table
                .static_member(application.member)
                .ok_or_else(|| fail("a field query lost its selected member"))?;
        }
        if !symbols.is_valid(selected.symbol) {
            return Err(fail("a field query lost its selected declaration"));
        }
        if self.typed.symbols.get(selected.symbol).kind != SymbolKind::Field {
            return Err(fail(
                "a field query requires its exact resolved field declaration",
            ));
        }
        if selected.const_literal.is_some() || selected.evidence_projection.is_some() {
            return Err(fail(
                "a field query cannot describe a value or evidence projection",
            ));
        }
        if selected.application.is_some() {
            return Err(fail("a field query needs its resolved owner application"));
        }
        if selected.type_reference.is_valid() {
            return Err(fail(
                "a field query has an unjoined supplying type reference",
            ));
        }
        let parent = self.typed.symbols.get(selected.symbol).parent;
        if !symbols.is_valid(parent) {
            return Err(fail("a field query lost its declaring owner"));
        }
        let owner = if self.typed.symbols.get(parent).kind == SymbolKind::Variant {
            self.typed.symbols.get(parent).parent
        } else {
            parent
        };
        if !symbols.is_valid(owner) || self.typed.symbols.get(owner).kind != SymbolKind::Data {
            return Err(fail("a field query has no data declaration owner"));
        }
        let data = match declaring_type {
            Some(reference) => {
                if self
                    .typed
                    .closed_data_constructor_field(reference, owner, selected.symbol)
                    .is_none()
                {
                    return Err(fail(
                        "a field query does not belong to its closed owner application",
                    ));
                }
                self.typed
                    .closed_data_definition(reference)
                    .ok_or_else(|| fail("a field query lost its closed declaring data"))?
            }
            None => self
                .typed
                .unique_data_definition_by_symbol(owner)
                .ok_or_else(|| fail("a field query has no unique declaring data"))?,
        };
        let mut occurrences = 0;
        for member in self.typed.data_members(data) {
            match member {
                typed_trees::data::DataMember::Field(field)
                    if parent == owner && field.symbol == selected.symbol =>
                {
                    occurrences += 1
                }
                typed_trees::data::DataMember::Variant(case) if case.symbol == parent => {
                    occurrences += self
                        .typed
                        .data_payload_fields(case)
                        .iter()
                        .filter(|field| field.symbol == selected.symbol)
                        .count();
                }
                _ => {}
            }
        }
        if occurrences != 1 {
            return Err(fail(
                "a field query is not its declaring owner's exact field row",
            ));
        }
        if data.generic_instance.is_none()
            && (!self.typed.data_type_parameters(data).is_empty()
                || !data.lifetime_parameters.is_empty())
        {
            return Err(fail(
                "a field query requires its complete owner application",
            ));
        }
        let author_span = self
            .typed
            .symbols
            .symbol_provenance_source_span(author)
            .ok_or_else(|| fail("a field query lost its lexical author"))?;
        if !self
            .typed
            .symbols
            .source_reference_may_select_symbol(author_span, selected.symbol)
        {
            return Err(fail(
                "a field query is outside its author's declaration scope",
            ));
        }
        let requester_identity = self
            .typed
            .normalized_hermetic_symbol_identity(author)
            .map_err(|reason| invalid(machine, reason))?;
        let authority = if self.typed.symbols.same_symbol_source_package(author, owner) {
            SchemaQueryAuthority::OwningScope { requester_identity }
        } else {
            SchemaQueryAuthority::ForeignScope { requester_identity }
        };
        let graph = match declaring_type {
            Some(reference) => {
                construct_semantic_schema_graph_for_type(self.typed, reference, authority)
            }
            None => construct_semantic_schema_graph(self.typed, owner, authority),
        }
        .map_err(|reason| invalid(machine, reason))?;
        let member = exact_symbol_identity(self.typed, selected.symbol)
            .map_err(|reason| invalid(machine, reason))?
            .0;
        let mut described = graph.nodes().iter().filter_map(|node| match node {
            SchemaNode::Field(field) if field.member_identity == member => Some(field),
            _ => None,
        });
        let field = described
            .next()
            .ok_or_else(|| fail("field query lost its exact schema member"))?;
        if described.next().is_some() {
            return Err(fail("field query has ambiguous schema correspondence"));
        }
        let case_identity = if field.owner_case.is_valid() {
            let SchemaNode::Case(case) = graph.node(field.owner_case) else {
                return Err(fail("field query lost its exact case owner"));
            };
            case.member_identity.as_bytes().to_vec()
        } else {
            Vec::new()
        };
        let owner_application = match &graph.declaration().application_identity {
            Some(identity) => identity.clone(),
            None => portable_applied_index_identity(
                self.typed,
                IndexIdentityArgument::StaticArgument(&StaticMachineArgument {
                    symbol: owner,
                    ..Default::default()
                }),
                author,
                &[],
            )
            .and_then(|identity| self.typed.type_identity_text(identity))
            .ok_or_else(|| fail("field query lost its complete owner type identity"))?,
        };
        let revision = declaration_source_revision(self.typed, owner)
            .map_err(|reason| invalid(machine, reason))?;
        let bytes = [
            ("member_identity", field.member_identity.as_bytes().to_vec()),
            ("case_identity", case_identity),
            (
                "owner_identity",
                graph.declaration().owner_identity.as_bytes().to_vec(),
            ),
            ("owner_application", owner_application.into_bytes()),
            ("name", field.name.as_bytes().to_vec()),
            ("qualified_type", field.qualified_type.as_bytes().to_vec()),
            ("source_revision", revision),
        ];
        let position = u64::from(field.declaration_position);
        let stable_number = field.stable_number;
        let erased = field.erased;
        // A field description comes from the declaration, including erased
        // members whose runtime storage cannot supply a value.
        let (_, entry) = self.declaration(machine, query)?;
        let result_type = entry.return_type;
        let result = self
            .typed
            .closed_data_definition(result_type)
            .ok_or_else(|| fail("a field query lost its result declaration"))?;
        if self.typed.core_vocabulary(result.symbol)
            != Some(CoreVocabulary::Reflection(ReflectionDeclaration::FieldInfo))
        {
            return Err(fail("a field query lost its toolchain FieldInfo result"));
        }
        let ValueType::Record(record, _, regions) = self.value_type(machine, result_type)? else {
            return Err(fail("a field description requires structured storage"));
        };
        let fields = self.data_type(machine, record)?.fields().to_vec();
        if fields.len() != 11 || !self.data_type(machine, record)?.cases().is_empty() {
            return Err(fail("a field description has another result field roster"));
        }
        let mut values = Vec::with_capacity(fields.len());
        for (name, bytes) in bytes {
            let mut matches = fields
                .iter()
                .enumerate()
                .filter(|(_, field)| field.identity == name);
            let (position, field) = matches
                .next()
                .ok_or_else(|| fail("field description lost a byte field"))?;
            if matches.next().is_some() {
                return Err(fail("field description repeats a byte field"));
            }
            values.push((
                position,
                self.byte_literal(machine, &bytes, Some(field.value_type), source)?,
            ));
        }
        for (name, kind) in [
            (
                "declaration_position",
                ExpressionKind::Integer(IntegerValue::Unsigned(u128::from(position))),
            ),
            (
                "stable_number",
                ExpressionKind::Integer(IntegerValue::Unsigned(u128::from(
                    stable_number.unwrap_or(0),
                ))),
            ),
            (
                "has_stable_number",
                ExpressionKind::Boolean(stable_number.is_some()),
            ),
            ("is_erased", ExpressionKind::Boolean(erased)),
        ] {
            let mut matches = fields
                .iter()
                .enumerate()
                .filter(|(_, field)| field.identity == name);
            let (position, field) = matches
                .next()
                .ok_or_else(|| fail("field description lost a scalar field"))?;
            if matches.next().is_some() {
                return Err(fail("field description repeats a scalar field"));
            }
            let expected = match kind {
                ExpressionKind::Integer(_) => {
                    matches!(field.value_type, ValueType::Scalar(ScalarType::Integer(integer), qualification) if integer.bits() == 64 && !integer.is_address() && integer.sign() == semantic_vocabulary::IntegerSign::Unsigned && qualification.is_none())
                }
                ExpressionKind::Boolean(_) => {
                    field.value_type
                        == ValueType::Scalar(ScalarType::Boolean, crate::body::Qualification::NONE)
                }
                _ => false,
            };
            if !expected {
                return Err(fail("field description scalar has another declared type"));
            }
            values.push((
                position,
                Expression {
                    call_context: None,
                    kind,
                    result: Some(field.value_type),
                    source,
                },
            ));
        }
        values.sort_by_key(|(position, _)| *position);
        Ok(Expression {
            call_context: None,
            kind: ExpressionKind::Record { fields: values },
            result: Some(ValueType::Record(
                record,
                crate::body::Qualification::NONE,
                regions,
            )),
            source,
        })
    }

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
        let ValueType::Record(record, _, _) = self.value_type(machine, result_type)? else {
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
            call_context: None,
            kind: ExpressionKind::Record {
                fields: vec![(0, value)],
            },
            result: Some(ValueType::record(record)),
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
            query @ (ReflectionDeclaration::Callable
            | ReflectionDeclaration::Schema
            | ReflectionDeclaration::Field),
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
        if query == ReflectionDeclaration::Field {
            return self
                .field_description(machine, declaration.symbol, call, source)
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
        let Some(ValueType::Record(record, _, _)) = Some(self.value_type(machine, result_type)?)
        else {
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
            call_context: None,
            kind: ExpressionKind::Case {
                case: case_index,
                fields,
            },
            result: Some(ValueType::record(record)),
            source,
        }))
    }
}
