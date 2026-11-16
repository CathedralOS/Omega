//! Equality reads the same applied fields as ordinary member access. Run only
//! after data applications have their substituted views; the resolved constructor
//! alone cannot select a comparison for an exact generic application.

use diagnostics::Diagnostic;
use language_core::sealed_intrinsics::SealedIntrinsic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataField, DataMember, DataShapeKind, DataVariant};
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, MemberSelection, TableBinaryExpression,
    TableBorrowExpression, TableCallExpression, TableMemberExpression, TableNamePath,
};
use typed_trees::trait_definition::ConformanceRowSource;
use typed_trees::type_identity::NormalizedTypeIdentity;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[derive(Clone)]
struct Selection {
    conformance: SymbolHandle,
    requirement: SymbolHandle,
    synthesized: bool,
    name: typed_trees::name::Identifier,
}

#[derive(Clone)]
struct PreparedSelection {
    identity: NormalizedTypeIdentity,
    application: TypeReferenceHandle,
    ordinal: usize,
    selection: Selection,
}

fn prepare_selections(
    program: &TypedTrees,
) -> Result<
    (
        Vec<PreparedSelection>,
        Vec<(SymbolHandle, NormalizedTypeIdentity)>,
    ),
    Diagnostic,
> {
    let mut family = Vec::new();
    let mut realizations = Vec::new();
    for (ordinal, conformance) in program.conformances().iter().enumerate() {
        if program.symbols.sealed_intrinsic(conformance.trait_symbol)
            != Some(SealedIntrinsic::EqualityProtocol)
        {
            continue;
        }
        let [application] = program
            .type_reference_table
            .type_reference_handles(conformance.arguments)
        else {
            return Err(Diagnostic::error(
                "core equality implementation lost its exact operand application",
            ));
        };
        let application = *application;
        let identity = program.package_qualified_type_identity(application);
        if !identity.is_valid() {
            return Err(Diagnostic::error(
                "core equality application lost its complete type identity",
            ));
        }
        let [row] = conformance.rows.as_slice() else {
            return Err(Diagnostic::error(
                "core equality implementation lost its unique equals row",
            ));
        };
        let requirement = program
            .trait_definition_by_symbol(row.declaring_trait)
            .and_then(|declaration| {
                program.trait_machine_signature_by_symbol(declaration, row.requirement)
            })
            .ok_or_else(|| Diagnostic::error("the comparison row lost its declared signature"))?;
        let parameters = program.state_signature_parameters(requirement);
        if parameters.len() != 2
            || parameters.iter().any(|formal| {
                !matches!(
                    program
                        .type_reference_table
                        .type_reference(formal.type_reference),
                    TypeReferenceNode::Reference {
                        access: language_core::ReferenceAccess::Shared,
                        ..
                    }
                )
            })
        {
            return Err(Diagnostic::error(
                "core equality must borrow exactly two operands shared",
            ));
        }

        let selection = Selection {
            conformance: conformance.symbol,
            requirement: row.requirement,
            synthesized: row.source == ConformanceRowSource::Synthesized,
            name: conformance.alias.clone().unwrap_or_else(|| {
                typed_trees::name::Identifier::generated(
                    program.symbols.display_path(conformance.symbol, "::"),
                )
            }),
        };
        if selection.synthesized {
            realizations.push((row.realization_machine, identity));
            realizations.push((row.realization_state, identity));
        }
        family.push(PreparedSelection {
            identity,
            application,
            ordinal,
            selection,
        });
    }
    family.sort_unstable_by_key(|entry| entry.identity);
    realizations.sort_unstable_by_key(|(symbol, _)| (symbol.arena_index(), symbol.generation()));
    Ok((family, realizations))
}

pub(crate) fn prepare(
    program: &mut TypedTrees,
    frontier: usize,
    conformance_frontier: usize,
) -> Result<(), Diagnostic> {
    let (family, realizations) = prepare_selections(program)?;
    let synthesized = family
        .iter()
        .filter(|entry| entry.ordinal >= conformance_frontier && entry.selection.synthesized)
        .map(|entry| entry.application)
        .collect::<Vec<_>>();
    let sites = program
        .expression_table
        .iter_expressions()
        .filter_map(|(handle, node)| {
            if handle.arena_index() as usize <= frontier {
                return None;
            }
            match node {
                ExpressionNode::Binary(binary)
                    if matches!(
                        binary.operator,
                        BinaryOperator::Equal | BinaryOperator::NotEqual
                    ) =>
                {
                    Some((handle, *binary))
                }
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    if synthesized.is_empty() && sites.is_empty() {
        return Ok(());
    }
    let owners = typed_trees::references::collect_program_operand_owners(program);
    let mut types = crate::typing_queries::DeclaredExpressionTypes::default();
    let mut preparation = Preparation {
        program,
        family: &family,
    };
    for application in synthesized {
        preparation.validate(application, &mut Vec::new())?;
    }
    for (handle, binary) in sites {
        let mut operands = None;
        let mut operand_identities = None;
        let mut own_synthesis = None;
        for owner in owners.expression_owners(handle) {
            let pair = (
                types.get(preparation.program, owner, binary.left),
                types.get(preparation.program, owner, binary.right),
            );
            let identities = (
                pair.0.map(|reference| {
                    preparation
                        .program
                        .package_qualified_type_identity(reference)
                }),
                pair.1.map(|reference| {
                    preparation
                        .program
                        .package_qualified_type_identity(reference)
                }),
            );
            if [identities.0, identities.1]
                .into_iter()
                .flatten()
                .any(|identity| !identity.is_valid())
            {
                return Err(Diagnostic::error(
                    "equality operand lost its complete type identity",
                ));
            }
            if operand_identities.is_some_and(|previous| previous != identities) {
                return Err(Diagnostic::error(
                    "equality occurrence has conflicting operand contexts",
                )
                .with_source_span(preparation.program.expression_table.source_span(handle)));
            }
            operands = Some(pair);
            operand_identities = Some(identities);
            let key = (owner.arena_index(), owner.generation());
            let position = realizations
                .partition_point(|(symbol, _)| (symbol.arena_index(), symbol.generation()) < key);
            let own = realizations
                .get(position)
                .filter(|(symbol, _)| *symbol == owner)
                .map(|(_, identity)| *identity);
            if own_synthesis.is_some_and(|previous| previous != own) {
                return Err(Diagnostic::error(
                    "equality occurrence has conflicting synthesis owners",
                ));
            }
            own_synthesis = Some(own);
        }
        let Some((Some(left), Some(right))) = operands else {
            // Keep the original comparison for the lowered-form checker. An
            // unknown type supplies neither equality eligibility nor a result.
            continue;
        };
        let own_synthesis = own_synthesis.flatten();
        let observed = |reference| {
            let Some(expected) = own_synthesis else {
                // Equality observes the borrowed value, just as its ordinary
                // shared-operand machine does. Retain the complete referent
                // type, including qualifications; this supplies no read loan.
                // The lowered-form checker still admits the actual read.
                if let TypeReferenceNode::Reference {
                    referee, access, ..
                } = preparation
                    .program
                    .type_reference_table
                    .type_reference(reference)
                    && access.is_readable()
                {
                    return Ok(*referee);
                }
                return Ok(reference);
            };
            let TypeReferenceNode::Reference {
                referee,
                access: language_core::ReferenceAccess::Shared,
                ..
            } = preparation
                .program
                .type_reference_table
                .type_reference(reference)
            else {
                return Err(Diagnostic::error(
                    "synthesized comparison lost its exact borrowed operand",
                ));
            };
            if preparation
                .program
                .package_qualified_type_identity(*referee)
                != expected
            {
                return Err(Diagnostic::error(
                    "synthesized comparison borrowed a different data application",
                ));
            }
            Ok(*referee)
        };
        let left_type = observed(left)?;
        let right_type = observed(right)?;
        let left_data = preparation.data(left_type);
        let right_data = preparation.data(right_type);
        if left_data.is_none() && right_data.is_none() {
            continue;
        }
        if preparation
            .program
            .package_qualified_type_identity(left_type)
            != preparation
                .program
                .package_qualified_type_identity(right_type)
        {
            return Err(Diagnostic::error("cannot compare values with `==`/`!=`: the operands are different data types (equality requires both operands to be the same type)").with_source_span(preparation.program.expression_table.source_span(handle)));
        }
        if preparation.is_implicit(left_type) {
            continue;
        }
        let comparison =
            preparation.compare(left_type, binary.left, binary.right, own_synthesis)?;
        let comparison = if binary.operator == BinaryOperator::NotEqual {
            let false_value = preparation.insert(ExpressionNode::Boolean(false));
            preparation.binary(comparison, BinaryOperator::Equal, false_value)
        } else {
            comparison
        };
        let node = preparation
            .program
            .expression_table
            .expression(comparison)
            .clone();
        *preparation.program.expression_table.expression_mut(handle) = node;
    }
    Ok(())
}

struct Preparation<'program> {
    program: &'program mut TypedTrees,
    family: &'program [PreparedSelection],
}

impl Preparation<'_> {
    fn data(&self, reference: TypeReferenceHandle) -> Option<&DataDefinition> {
        self.program.closed_data_definition(
            self.program
                .type_reference_table
                .unconstrained_type_reference(reference),
        )
    }

    fn selection(&self, reference: TypeReferenceHandle) -> Result<&Selection, Diagnostic> {
        let identity = self.program.package_qualified_type_identity(reference);
        let first = self
            .family
            .partition_point(|entry| entry.identity < identity);
        let Some(entry) = self
            .family
            .get(first)
            .filter(|entry| entry.identity == identity)
        else {
            return Err(Diagnostic::error(format!(
                "type `{}` is not Equatable: structural equality requires an exact named core Equatable implementation",
                self.program.display_type_reference(reference)
            )));
        };
        if self
            .family
            .get(first + 1)
            .is_some_and(|entry| entry.identity == identity)
        {
            return Err(Diagnostic::error(format!(
                "type `{}` has multiple Equatable implementations; the operator selects at most one",
                self.program.display_type_reference(reference)
            )));
        }
        Ok(&entry.selection)
    }

    fn is_primitive(&self, reference: TypeReferenceHandle) -> bool {
        let reference = self
            .program
            .type_reference_table
            .unconstrained_type_reference(reference);
        !matches!(
            self.program.type_reference_table.type_reference(reference),
            TypeReferenceNode::Reference { .. }
        ) && self.program.primitive_type_reference(reference).is_some()
    }

    fn is_implicit(&self, reference: TypeReferenceHandle) -> bool {
        let Some(data) = self.data(reference) else {
            return false;
        };
        if data.quotient.is_some() {
            return false;
        }
        let members = self.program.data_members(data);
        DataDefinition::shape_kind_from_members(members) == DataShapeKind::Enum
            && members.iter().all(|member| matches!(member, DataMember::Variant(variant) if self.program.data_payload_fields(variant).is_empty()))
    }

    fn is_text(&self, reference: TypeReferenceHandle) -> bool {
        let table = &self.program.type_reference_table;
        match table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. } => {
                match table.type_reference(table.unconstrained_type_reference(*referee)) {
                    TypeReferenceNode::Slice { element_type } => {
                        self.program.primitive_type_reference(*element_type)
                            == Some(symbols::PrimitiveType::U8)
                    }
                    _ => false,
                }
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } if table.constraints(*constraints).iter().any(|constraint| {
                matches!(
                    constraint,
                    typed_trees::types::TypeConstraintNode::Domain(_)
                )
            }) =>
            {
                match table.type_reference(*base_type) {
                    TypeReferenceNode::FixedArray {
                        element_type,
                        length: typed_trees::types::FixedArrayLength::Literal(_),
                    } => {
                        self.program.primitive_type_reference(*element_type)
                            == Some(symbols::PrimitiveType::U8)
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn validate(
        &mut self,
        application: TypeReferenceHandle,
        visiting: &mut Vec<NormalizedTypeIdentity>,
    ) -> Result<(), Diagnostic> {
        let identity = self.program.package_qualified_type_identity(application);
        if visiting.contains(&identity) {
            return Err(Diagnostic::error(
                "recursive Equatable implementations cannot synthesize terminating structural equality",
            ));
        }
        let data = self.data(application).ok_or_else(|| {
            Diagnostic::error("structural equality needs its complete closed data application")
        })?;
        self.require_ordinary_equality(application)?;
        let owner = data.name.as_str().to_owned();
        let members = self.program.data_members(data);
        let mut fields = Vec::with_capacity(members.len());
        for member in members {
            match member {
                DataMember::Field(field) => fields.push(field.clone()),
                DataMember::Variant(variant) => {
                    fields.extend(self.program.data_payload_fields(variant).iter().cloned())
                }
            }
        }
        visiting.push(identity);
        for field in fields {
            if self.is_primitive(field.type_reference)
                || self.is_text(field.type_reference)
                || self.is_implicit(field.type_reference)
            {
                continue;
            }
            let selected = self.selection(field.type_reference).map_err(|error| {
                Diagnostic::error(format!(
                    "field `{}` of `{owner}` is not Equatable: {}",
                    field.name, error.message
                ))
            })?;
            if selected.synthesized {
                self.validate(field.type_reference, visiting)?;
            }
        }
        visiting.pop();
        Ok(())
    }

    fn compare(
        &mut self,
        application: TypeReferenceHandle,
        left: ExpressionHandle,
        right: ExpressionHandle,
        own_synthesis: Option<NormalizedTypeIdentity>,
    ) -> Result<ExpressionHandle, Diagnostic> {
        self.require_ordinary_equality(application)?;
        if self.is_primitive(application)
            || self.is_text(application)
            || self.is_implicit(application)
        {
            return Ok(self.binary(left, BinaryOperator::Equal, right));
        }
        let identity = self.program.package_qualified_type_identity(application);
        if own_synthesis != Some(identity) {
            let selected = self.selection(application)?;
            if !selected.synthesized {
                let selected = selected.clone();
                return self.call(selected, left, right);
            }
        }
        let data = self
            .data(application)
            .ok_or_else(|| Diagnostic::error("equality lost its closed application"))?;
        let name = data.name.clone();
        let symbol = data.symbol;
        let members = self.program.data_members(data).to_vec();
        let fields = members
            .iter()
            .filter_map(|member| match member {
                DataMember::Field(field) => Some(field.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        if !fields.is_empty() && fields.iter().all(|field| field.relevance.is_erased()) {
            return Err(Diagnostic::error(
                "structural equality cannot compare a record whose every field is erased",
            ));
        }
        let mut comparisons = self.fields(&fields, None, left, right)?;
        let variants = members
            .iter()
            .filter_map(|member| match member {
                DataMember::Variant(variant) => Some(variant.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        if !variants.is_empty() {
            let mut alternatives = Vec::with_capacity(variants.len());
            for variant in variants {
                if [left, right].into_iter().any(|value| {
                    self.literal_case(value)
                        .is_some_and(|case| case != variant.symbol)
                }) {
                    continue;
                }
                let fields = self.program.data_payload_fields(&variant).to_vec();
                let left_tag = self.tag(symbol, &name, &variant, left);
                let right_tag = self.tag(symbol, &name, &variant, right);
                let mut same = vec![left_tag, right_tag];
                same.extend(self.fields(&fields, Some(&variant), left, right)?);
                alternatives.push(self.fold(same, BinaryOperator::And, true));
            }
            comparisons.push(self.fold(alternatives, BinaryOperator::Or, false));
        }
        Ok(self.fold(comparisons, BinaryOperator::And, true))
    }

    fn require_ordinary_equality(
        &self,
        application: TypeReferenceHandle,
    ) -> Result<(), Diagnostic> {
        if self
            .data(application)
            .is_some_and(|data| data.quotient.is_some())
        {
            return Err(Diagnostic::error(
                "quotient equality needs a named lifted operation with its checked equivalence decision law; an ordinary Equatable row cannot bind the operator",
            ));
        }
        Ok(())
    }

    fn fields(
        &mut self,
        fields: &[DataField],
        variant: Option<&DataVariant>,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> Result<Vec<ExpressionHandle>, Diagnostic> {
        let mut comparisons = Vec::with_capacity(fields.len());
        for field in fields {
            if field.relevance.is_erased() {
                continue;
            }
            let left = self.field(field, variant, left)?;
            let right = self.field(field, variant, right)?;
            comparisons.push(self.compare(field.type_reference, left, right, None)?);
        }
        Ok(comparisons)
    }

    fn field(
        &mut self,
        field: &DataField,
        variant: Option<&DataVariant>,
        mut receiver: ExpressionHandle,
    ) -> Result<ExpressionHandle, Diagnostic> {
        while let ExpressionNode::Borrow(borrow) =
            self.program.expression_table.expression(receiver)
        {
            receiver = borrow.target;
        }
        if let ExpressionNode::StructLiteral(literal) =
            self.program.expression_table.expression(receiver)
        {
            return self
                .program
                .expression_table
                .struct_fields(literal.fields)
                .iter()
                .find(|value| value.name == field.name)
                .map(|value| value.value)
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "structural equality literal omits field `{}`",
                        field.name
                    ))
                });
        }
        Ok(self.insert(ExpressionNode::Member(TableMemberExpression {
            receiver,
            member: field.name.clone(),
            selection: MemberSelection::Field(field.symbol),
            case_variant: variant.map(|variant| variant.name.clone()),
        })))
    }

    fn tag(
        &mut self,
        data: SymbolHandle,
        name: &typed_trees::name::Identifier,
        variant: &DataVariant,
        value: ExpressionHandle,
    ) -> ExpressionHandle {
        if let Some(case) = self.literal_case(value) {
            return self.insert(ExpressionNode::Boolean(case == variant.symbol));
        }
        let mut members = arena::HandleSpan::empty();
        self.program
            .expression_table
            .push_name_path_member(&mut members, name.clone());
        self.program
            .expression_table
            .push_name_path_member(&mut members, variant.name.clone());
        let mut member_symbols = arena::HandleSpan::empty();
        self.program
            .expression_table
            .push_name_path_member_symbol(&mut member_symbols, data);
        self.program
            .expression_table
            .push_name_path_member_symbol(&mut member_symbols, variant.symbol);
        let case = self.insert(ExpressionNode::Name(TableNamePath::new(
            members,
            member_symbols,
            data,
            variant.symbol,
        )));
        self.binary(value, BinaryOperator::CaseMembership, case)
    }

    fn literal_case(&self, mut value: ExpressionHandle) -> Option<SymbolHandle> {
        while let ExpressionNode::Borrow(borrow) = self.program.expression_table.expression(value) {
            value = borrow.target;
        }
        match self.program.expression_table.expression(value) {
            ExpressionNode::StructLiteral(literal) => literal.case_symbol,
            ExpressionNode::Name(path)
                if path.symbol.is_valid()
                    && self.program.symbols.get(path.symbol).kind
                        == symbols::SymbolKind::Variant =>
            {
                Some(path.symbol)
            }
            _ => None,
        }
    }

    fn call(
        &mut self,
        selected: Selection,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> Result<ExpressionHandle, Diagnostic> {
        let arguments = [left, right].map(|target| {
            self.insert(ExpressionNode::Borrow(TableBorrowExpression {
                target,
                access: language_core::ReferenceAccess::Shared,
            }))
        });
        let arguments = self
            .program
            .expression_table
            .insert_expression_handles(arguments);
        let mut members = arena::HandleSpan::empty();
        self.program
            .expression_table
            .push_name_path_member(&mut members, selected.name);
        let mut member_symbols = arena::HandleSpan::empty();
        self.program
            .expression_table
            .push_name_path_member_symbol(&mut member_symbols, selected.conformance);
        let receiver = self.insert(ExpressionNode::Name(TableNamePath::new(
            members,
            member_symbols,
            selected.conformance,
            selected.conformance,
        )));
        Ok(self.insert(ExpressionNode::Call(TableCallExpression {
            receiver,
            target_symbol: selected.requirement,
            target: typed_trees::name::Identifier::generated("equals"),
            arguments,
            operational_acknowledgement: language_semantics::CallOperationalAcknowledgement {
                origin:
                    language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized,
                ..Default::default()
            },
            authored_call_selection: None,
            authored_call_binding: None,
            authored_machine_application: Default::default(),
            requirement_application: Default::default(),
            exact_coercion_target: Default::default(),
            static_machine_parameter: Default::default(),
            static_requirement_dispatch: None,
            machine_arguments: Box::default(),
            runtime_call: None,
            quotient_operation: None,
            private_layout_operation: None,
            installed_interrupt_operation: None,
            layout_query: None,
            native_code_layout: None,
            build_selection: Default::default(),
            evidence_arguments: Box::default(),
        })))
    }

    fn insert(&mut self, expression: ExpressionNode) -> ExpressionHandle {
        self.program.expression_table.insert(expression)
    }
    fn binary(
        &mut self,
        left: ExpressionHandle,
        operator: BinaryOperator,
        right: ExpressionHandle,
    ) -> ExpressionHandle {
        self.insert(ExpressionNode::Binary(TableBinaryExpression {
            policy: numerics::arithmetic::ArithmeticDomain::Exact,
            left,
            operator,
            right,
        }))
    }
    fn fold(
        &mut self,
        expressions: Vec<ExpressionHandle>,
        operator: BinaryOperator,
        empty: bool,
    ) -> ExpressionHandle {
        let mut expressions = expressions.into_iter();
        let Some(first) = expressions.next() else {
            return self.insert(ExpressionNode::Boolean(empty));
        };
        expressions.fold(first, |left, right| self.binary(left, operator, right))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn borrowed_enum_comparison_observes_the_exact_referent() {
        let source = "data Kind [copy] { case First; case Last; } machine shared(kind: &Kind) -> bool { kind == Kind::First } machine exclusive(kind: &mut Kind) -> bool { kind != Kind::Last }";
        let checked =
            crate::qualified_operator_home_tests::checked_with_equality(&[("main.omg", source)]);
        assert!(checked.is_ok(), "{checked:?}");

        let different = format!(
            "{source} data Other [copy] {{ case First; }} machine different(kind: &Kind) -> bool {{ kind == Other::First }}"
        );
        let error = crate::qualified_operator_home_tests::checked_with_equality(&[(
            "main.omg", &different,
        )])
        .unwrap_err();
        assert!(
            error.contains("operands are different data types"),
            "{error}"
        );
    }

    #[test]
    fn an_authored_row_cannot_replace_missing_quotient_observer_evidence() {
        let source = "data Point [copy] { x: i32; } Equality satisfies Equatable<Point> { machine equals(left: &Point, right: &Point) -> bool { false } }";
        let mut program =
            crate::qualified_operator_home_tests::checked_with_equality(&[("main.omg", source)])
                .unwrap();
        let application = program
            .type_reference_table
            .type_reference_handles(program.conformances()[0].arguments)[0];
        let typed_trees::data::DataMember::Field(field) =
            &program.data_members(&program.data_definitions()[0])[0]
        else {
            panic!("Point has its scalar carrier field");
        };
        let carrier = field.type_reference;
        program
            .tables
            .data_definitions
            .get_mut(program.roots.data_definitions.start())
            .quotient = Some(typed_trees::data::QuotientDefinition {
            carrier,
            relation: Vec::new(),
            relation_symbol: Default::default(),
            equivalence: None,
        });
        let (family, _) = super::prepare_selections(&program).unwrap();
        let mut preparation = super::Preparation {
            program: &mut program,
            family: &family,
        };
        let missing = typed_trees::expression::ExpressionHandle::invalid();
        let error = preparation
            .compare(application, missing, missing, None)
            .unwrap_err();
        assert!(
            error.message.contains("checked equivalence decision law"),
            "{error:?}"
        );
    }

    #[test]
    fn preparing_the_family_keeps_reference_arguments_distinct() {
        let source = "data Point [copy] { x: i32; } ValueEquality satisfies Equatable<Point> { machine equals(left: &Point, right: &Point) -> bool { false } } ReferenceEquality satisfies Equatable<Point> { machine equals(left: &Point, right: &Point) -> bool { true } }";
        let mut program =
            crate::qualified_operator_home_tests::checked_with_equality(&[("main.omg", source)])
                .unwrap();
        let operand = program
            .type_reference_table
            .type_reference_handles(program.conformances()[0].arguments)[0];
        let reference =
            program
                .type_reference_table
                .insert(typed_trees::types::TypeReferenceNode::Reference {
                    referee: operand,
                    access: language_core::ReferenceAccess::Shared,
                    lifetime: None,
                });
        let arguments = program
            .type_reference_table
            .insert_type_reference_handles([reference]);
        // Exercise declaration selection only. The ordinary callable checker
        // independently judges whether a row matches this changed application.
        program
            .tables
            .conformances
            .get_mut(program.roots.conformances.handle_at(1))
            .arguments = arguments;
        let (family, _) = super::prepare_selections(&program).unwrap();
        assert_eq!(family.len(), 2);
        assert_ne!(family[0].identity, family[1].identity);
        let preparation = super::Preparation {
            program: &mut program,
            family: &family,
        };
        assert_ne!(
            preparation.selection(operand).unwrap().conformance,
            preparation.selection(reference).unwrap().conformance
        );
    }

    #[test]
    fn substituted_type_fields_and_explicit_rows_keep_one_comparison_owner() {
        let source = "data Holder<T [copy]> [copy] { value: T; } HolderEquality satisfies Equatable<Holder<i32>> { } data Coin [copy] { value: i32; } CoinStructural satisfies Equatable<Coin> { } CoinFalse satisfies Equatable<Coin> { machine equals(left: &Coin, right: &Coin) -> bool { false } } machine compare(left: Holder<i32>, right: Holder<i32>) -> bool { left == right } machine invoke(left: Coin, right: Coin) -> bool { CoinStructural::equals(&left, &right) }";
        let checked =
            crate::qualified_operator_home_tests::checked_with_equality(&[("main.omg", source)]);
        assert!(checked.is_ok(), "{checked:?}");
        let ambiguous = format!(
            "{source} machine ambiguous(left: Coin, right: Coin) -> bool {{ left == right }}"
        );
        let error = crate::qualified_operator_home_tests::checked_with_equality(&[(
            "main.omg", &ambiguous,
        )])
        .unwrap_err();
        assert!(
            error.contains("multiple Equatable implementations"),
            "{error}"
        );
    }

    #[test]
    fn qualified_rows_do_not_share_a_comparison_with_each_other_or_the_plain_record() {
        let source = "data Point [copy] { x: i32; } domain Point::A; domain Point::B; AEquality satisfies Equatable<Point in A> { machine equals(left: &Point in A, right: &Point in A) -> bool { false } } BEquality satisfies Equatable<Point in B> { machine equals(left: &Point in B, right: &Point in B) -> bool { true } }";
        let mut program =
            crate::qualified_operator_home_tests::checked_with_equality(&[("main.omg", source)])
                .unwrap();
        let (family, _) = super::prepare_selections(&program).unwrap();
        assert_eq!(family.len(), 2);
        assert_ne!(family[0].identity, family[1].identity);
        let preparation = super::Preparation {
            program: &mut program,
            family: &family,
        };
        for row in &family {
            assert_eq!(
                preparation.selection(row.application).unwrap().conformance,
                row.selection.conformance
            );
        }
        let plain = preparation
            .program
            .type_reference_table
            .peeled_type_reference(family[0].application);
        assert!(
            preparation.selection(plain).is_err(),
            "qualified evidence cannot select plain Point equality"
        );
    }
}
