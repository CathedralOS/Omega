//! Resolve the closed Content fragment into the existing canonical vocabulary.
//! This reader records expressions and exact owners; the lowered-form checker
//! separately proves their definedness and each requested conservation equation.

use language_core::core_vocabulary::{ContentDeclaration, CoreVocabulary};
use semantic_vocabulary::{
    ContentAlgebra, ContentAlgebraKind, ContentDomainId, ContentProjectionExpression,
    ContentProjectionIdentity, ContentProjectionScalar, MachineId,
};
use symbols::{BuiltinFunction, SymbolHandle, SymbolKind};
use terminal_check_tables::{Checked, Construct};
use terminal_psi::StructuralContentProjection;
use typed_trees::data::DataMember;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

use super::Program;
use crate::body::DomainInstance;
use crate::{Unserved, refuse};

pub(super) struct ProjectionDefinition {
    pub owner: DomainInstance,
    pub machine: SymbolHandle,
    pub projection: StructuralContentProjection,
}

fn unserved(id: MachineId, reason: &'static str) -> terminal_check_tables::Refusal {
    refuse(id, Construct::Machine, Unserved::Contract(reason))
}

fn application(
    typed: &typed_trees::TypedTrees,
    mut reference: TypeReferenceHandle,
) -> Option<(SymbolHandle, &[TypeReferenceHandle])> {
    for _ in 0..64 {
        if !typed
            .type_reference_table
            .contains_type_reference(reference)
        {
            return None;
        }
        match typed.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } => {
                let rows = typed.type_reference_handles(*arguments);
                return (rows.len() == arguments.len()).then_some((*base_symbol, rows));
            }
            TypeReferenceNode::Named { symbol, .. } => {
                let data = typed.closed_data_definition(reference)?;
                if let Some(instance) = data.generic_instance {
                    if instance == reference {
                        return None;
                    }
                    reference = instance;
                } else {
                    return Some((*symbol, &[]));
                }
            }
            _ => return None,
        }
    }
    None
}

fn algebra(
    typed: &typed_trees::TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<ContentAlgebra> {
    let (declaration, [parameter]) = application(typed, reference)? else {
        return None;
    };
    let kind = match typed.core_vocabulary(declaration)? {
        CoreVocabulary::Content(ContentDeclaration::IntervalSet) => ContentAlgebraKind::IntervalSet,
        CoreVocabulary::Content(ContentDeclaration::CountedQuantity) => {
            ContentAlgebraKind::CountedQuantity
        }
        _ => return None,
    };
    let parameter = typed.type_identity_text(typed.package_qualified_type_identity(*parameter))?;
    (!parameter.is_empty()).then_some(ContentAlgebra { kind, parameter })
}

fn same_type(
    typed: &typed_trees::TypedTrees,
    left: TypeReferenceHandle,
    right: TypeReferenceHandle,
) -> bool {
    let left = typed.normalized_type_identity(left);
    left.is_valid() && left == typed.normalized_type_identity(right)
}

fn field_identity(field: &typed_trees::data::DataField) -> String {
    field
        .identity
        .map_or_else(|| field.name.as_str().to_owned(), |id| format!("#{id}"))
}

/// A path is accepted through selected field symbols of the exact source
/// carrier. Display names never select a field or an owning qualification.
fn subject_path(
    typed: &typed_trees::TypedTrees,
    subject: SymbolHandle,
    carrier: TypeReferenceHandle,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<(Vec<String>, TypeReferenceHandle)> {
    if depth > 256 || !expression.is_valid() {
        return None;
    }
    match typed.expression_table.expression(expression) {
        ExpressionNode::Name(name)
            if name.symbol == subject
                && !name.static_application.is_valid()
                && !name.reserved_result_of.is_valid()
                && typed.expression_table.name_path_members(name.members).len() == 1 =>
        {
            Some((Vec::new(), carrier))
        }
        ExpressionNode::Member(member) if member.case_variant.is_none() => {
            let (mut path, parent) =
                subject_path(typed, subject, carrier, member.receiver, depth + 1)?;
            let data = typed.closed_data_definition(parent)?;
            let field = typed.data_field(data, member.selection.field_symbol())?;
            if field.relevance.is_erased() {
                return None;
            }
            path.push(field_identity(field));
            Some((path, field.type_reference))
        }
        _ => None,
    }
}

fn natural_type(typed: &typed_trees::TypedTrees, reference: TypeReferenceHandle) -> bool {
    application(typed, reference).is_some_and(|(declaration, arguments)| {
        arguments.is_empty()
            && typed.core_vocabulary(declaration) == Some(CoreVocabulary::ProofNatural)
    })
}

fn scalar(
    typed: &typed_trees::TypedTrees,
    subject: SymbolHandle,
    carrier: TypeReferenceHandle,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<ContentProjectionScalar> {
    if depth > 256 || !expression.is_valid() {
        return None;
    }
    match typed.expression_table.expression(expression) {
        ExpressionNode::Integer(literal)
            if !literal.is_negative() && literal.landing().is_none() =>
        {
            Some(ContentProjectionScalar::Natural(
                literal.value_u64()?.to_string(),
            ))
        }
        ExpressionNode::Binary(binary)
            if matches!(
                crate::policy::suffix(binary.policy),
                crate::policy::Suffix::Unwritten
            ) =>
        {
            let left = Box::new(scalar(typed, subject, carrier, binary.left, depth + 1)?);
            let right = Box::new(scalar(typed, subject, carrier, binary.right, depth + 1)?);
            Some(match binary.operator {
                BinaryOperator::Add => ContentProjectionScalar::Add(left, right),
                BinaryOperator::Subtract => ContentProjectionScalar::Subtract(left, right),
                BinaryOperator::Multiply => ContentProjectionScalar::Multiply(left, right),
                _ => return None,
            })
        }
        ExpressionNode::Call(call)
            if typed
                .symbols
                .builtin_function_for_symbol(call.target_symbol)
                == Some(BuiltinFunction::IntegerEmbed)
                && !call.receiver.is_valid()
                && call.machine_arguments.is_empty()
                && call.evidence_arguments.is_empty() =>
        {
            let [argument] = typed.expression_table.expression_handles(call.arguments) else {
                return None;
            };
            let (path, reference) = subject_path(typed, subject, carrier, *argument, depth + 1)?;
            let TypeReferenceNode::Named { symbol, .. } =
                typed.type_reference_table.type_reference(reference)
            else {
                return None;
            };
            match typed.symbols.builtin_type_atom(*symbol)? {
                symbols::BuiltinTypeAtom::Address => {}
                symbols::BuiltinTypeAtom::Integer(integer)
                    if [8, 16, 32, 64].contains(&integer.width()) && !integer.is_signed() => {}
                _ => return None,
            }
            (!path.is_empty()).then_some(ContentProjectionScalar::RuntimeScalarEmbedding(path))
        }
        ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
            let (path, reference) = subject_path(typed, subject, carrier, expression, depth + 1)?;
            (natural_type(typed, reference) && !path.is_empty())
                .then_some(ContentProjectionScalar::SubjectField(path))
        }
        _ => None,
    }
}

fn natural_scalar(
    typed: &typed_trees::TypedTrees,
    subject: SymbolHandle,
    carrier: TypeReferenceHandle,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<ContentProjectionScalar> {
    match typed.expression_table.expression(expression) {
        ExpressionNode::Cast(cast)
            if natural_type(typed, cast.target_type)
                && cast.form.entry().is_none()
                && cast.form.recast_access().is_none()
                && !cast.semantic_domain_symbol.is_valid()
                && cast.semantic_domain.is_empty()
                && matches!(
                    crate::policy::suffix(cast.domain),
                    crate::policy::Suffix::Unwritten
                ) =>
        {
            // The canonical scalar retains the mathematical expression. Its
            // natural definedness is a proof obligation, not supplied by this cast.
            scalar(typed, subject, carrier, cast.value, depth + 1)
        }
        ExpressionNode::Integer(literal)
            if !literal.is_negative() && literal.landing().is_none() =>
        {
            scalar(typed, subject, carrier, expression, depth)
        }
        ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
            let (_, reference) = subject_path(typed, subject, carrier, expression, depth)?;
            natural_type(typed, reference)
                .then(|| scalar(typed, subject, carrier, expression, depth))
                .flatten()
        }
        _ => None,
    }
}

impl Program<'_> {
    /// Exact owner projection, prepared once in this source-stage scratch.
    /// An unrelated package cannot attach a second denominator to this owner.
    pub(crate) fn published_content_projection(
        &self,
        owner: DomainInstance,
    ) -> Option<&StructuralContentProjection> {
        self.content_projections
            .iter()
            .find(|row| row.owner == owner)
            .map(|row| &row.projection)
    }

    pub(super) fn prepare_domain_content(
        &mut self,
        id: MachineId,
        owner: DomainInstance,
    ) -> Checked<()> {
        if self
            .content_projections
            .iter()
            .any(|row| row.owner == owner)
        {
            return Ok(());
        }
        let candidates: Vec<_> = self
            .typed
            .machines()
            .iter()
            .filter(|machine| {
                self.typed
                    .symbols
                    .symbols()
                    .nodes()
                    .is_valid(machine.symbol)
                    && self.typed.symbols.get(machine.symbol).parent == owner.declaration
                    && self
                        .typed
                        .machine_trait_conformances(machine)
                        .iter()
                        .any(|row| {
                            self.typed.core_vocabulary(row.symbol)
                                == Some(CoreVocabulary::Content(ContentDeclaration::Content))
                        })
            })
            .map(|machine| machine.symbol)
            .collect();
        match candidates.as_slice() {
            [] => Ok(()),
            [machine] => {
                let index = self
                    .prepare_content_projection(id, *machine)?
                    .ok_or_else(|| unserved(id, "declared owner projection was not published"))?;
                if self.content_projections[index].owner != owner {
                    return Err(unserved(
                        id,
                        "content projection changed its exact domain application",
                    ));
                }
                Ok(())
            }
            _ => Err(unserved(
                id,
                "qualification owner publishes more than one content projection",
            )),
        }
    }

    pub(super) fn prepare_content_projection(
        &mut self,
        id: MachineId,
        machine_symbol: SymbolHandle,
    ) -> Checked<Option<usize>> {
        if let Some(index) = self
            .content_projections
            .iter()
            .position(|row| row.machine == machine_symbol)
        {
            return Ok(Some(index));
        }
        let typed = self.typed;
        let Some(machine) = typed.machine_by_symbol(machine_symbol) else {
            return Ok(None);
        };
        let conformances = typed.machine_trait_conformances(machine);
        let mut content = conformances.iter().filter(|row| {
            typed.core_vocabulary(row.symbol)
                == Some(CoreVocabulary::Content(ContentDeclaration::Content))
        });
        let Some(conformance) = content.next() else {
            return Ok(None);
        };
        if content.next().is_some() || conformances.len() != machine.satisfies.len() {
            return Err(unserved(
                id,
                "content projection has no unique formed conformance",
            ));
        }
        let Some(typed_trees::machine::SatisfiedDeclaration::Trait {
            definition,
            requirement,
        }) = typed_trees::machine::resolve_satisfied_declaration(typed, machine, conformance)
        else {
            return Err(unserved(
                id,
                "content projection lost its exact requirement",
            ));
        };
        if typed.core_vocabulary(definition.symbol)
            != Some(CoreVocabulary::Content(ContentDeclaration::Content))
            || typed.core_vocabulary(requirement.symbol)
                != Some(CoreVocabulary::Content(ContentDeclaration::Project))
        {
            return Err(unserved(
                id,
                "content projection changed its selected Core requirement",
            ));
        }
        if !typed.symbols.symbols().nodes().is_valid(machine.symbol) {
            return Err(unserved(id, "content projection has no owned declaration"));
        }
        let owner_symbol = typed.symbols.get(machine.symbol).parent;
        if !typed.symbols.symbols().nodes().is_valid(owner_symbol) {
            return Err(unserved(
                id,
                "content projection has no owning qualification",
            ));
        }
        let Some(owner) = typed.domain_definition(owner_symbol) else {
            return Err(unserved(
                id,
                "content projection is not declared by a qualification owner",
            ));
        };
        if typed.symbols.get(owner_symbol).kind != SymbolKind::Domain
            || typed.symbols.symbol_package_identity(machine.symbol)
                != typed.symbols.symbol_package_identity(owner_symbol)
            || !owner.type_parameters.is_empty()
            || !machine.type_parameters.is_empty()
            || !machine.lifetime_parameters.is_empty()
            || !machine.owned_data.is_empty()
            || machine.suspends
            || machine.blocks
            || !machine.contracts.is_empty()
            || !machine.body_is_present
        {
            return Err(unserved(
                id,
                "content projection is not a closed owner-local proof expression",
            ));
        }
        let mut published = typed.machines().iter().filter(|candidate| {
            typed.symbols.symbols().nodes().is_valid(candidate.symbol)
                && typed.symbols.get(candidate.symbol).parent == owner_symbol
                && typed
                    .machine_trait_conformances(candidate)
                    .iter()
                    .any(|row| {
                        typed.core_vocabulary(row.symbol)
                            == Some(CoreVocabulary::Content(ContentDeclaration::Content))
                    })
        });
        if published.next().map(|candidate| candidate.symbol) != Some(machine_symbol)
            || published.next().is_some()
        {
            return Err(unserved(
                id,
                "qualification owner publishes more than one content projection",
            ));
        }
        self.served(id, machine.symbol)?;
        let states = typed.machine_states(machine);
        let Some([state]) = states.as_array() else {
            return Err(unserved(id, "content projection has control flow"));
        };
        let parameters = typed.state_parameters(state);
        let Some([parameter]) = parameters.as_array() else {
            return Err(unserved(
                id,
                "content projection has other than one subject",
            ));
        };
        let TypeReferenceNode::Reference {
            referee,
            access: language_core::ReferenceAccess::Shared,
            ..
        } = typed
            .type_reference_table
            .type_reference(parameter.type_reference)
        else {
            return Err(unserved(
                id,
                "content projection does not borrow its owner carrier",
            ));
        };
        let arguments = typed.type_reference_handles(conformance.arguments);
        let [declared_owner, declared_algebra] = arguments else {
            return Err(unserved(
                id,
                "content projection has an incomplete application",
            ));
        };
        if !same_type(typed, *referee, owner.target_type)
            || !same_type(typed, *declared_owner, owner.target_type)
            || !same_type(typed, *declared_algebra, state.return_type)
            || !state.contracts.is_empty()
        {
            return Err(unserved(
                id,
                "content projection changes its carrier or algebra",
            ));
        }
        let algebra = algebra(typed, state.return_type)
            .ok_or_else(|| unserved(id, "content projection selects an unsupported algebra"))?;
        let statements = typed.statement_table.statements(state.statement_nodes);
        let [typed_trees::statement::StatementNode::Expression(expression)] = statements else {
            return Err(unserved(
                id,
                "content projection body contains executable statements",
            ));
        };
        let ExpressionNode::StructLiteral(literal) = typed.expression_table.expression(*expression)
        else {
            return Err(unserved(
                id,
                "content projection body does not construct its algebra",
            ));
        };
        let declaration = typed
            .closed_data_definition(state.return_type)
            .ok_or_else(|| unserved(id, "content algebra has no closed constructor"))?;
        if typed.closed_data_constructor_definition(state.return_type, literal.type_symbol)
            != Some(declaration)
            || literal.case_symbol.is_some()
            || literal.case_name.is_some()
        {
            return Err(unserved(
                id,
                "content projection constructs another carrier",
            ));
        }
        let declared_fields: Vec<_> = typed
            .data_members(declaration)
            .iter()
            .filter_map(|row| match row {
                DataMember::Field(field) => Some(field),
                _ => None,
            })
            .collect();
        let fields = typed.expression_table.struct_fields(literal.fields);
        if fields.len() != literal.fields.len() || fields.len() != declared_fields.len() {
            return Err(unserved(
                id,
                "content projection constructor has an incomplete field roster",
            ));
        }
        let mut values = Vec::with_capacity(declared_fields.len());
        for field in declared_fields {
            let mut matching = fields.iter().filter(|row| row.field_symbol == field.symbol);
            let value = matching
                .next()
                .ok_or_else(|| unserved(id, "content projection omitted an algebra field"))?;
            if matching.next().is_some() {
                return Err(unserved(id, "content projection repeated an algebra field"));
            }
            values.push(
                natural_scalar(typed, parameter.symbol, *referee, value.value, 0).ok_or_else(
                    || unserved(id, "content projection leaves the closed proof fragment"),
                )?,
            );
        }
        let expression = match (algebra.kind, values.as_slice()) {
            (ContentAlgebraKind::IntervalSet, [start, end]) => {
                ContentProjectionExpression::IntervalSet(vec![(start.clone(), end.clone())])
            }
            (ContentAlgebraKind::CountedQuantity, [magnitude]) => {
                ContentProjectionExpression::CountedQuantity(magnitude.clone())
            }
            _ => {
                return Err(unserved(
                    id,
                    "content algebra constructor differs from its closed declaration",
                ));
            }
        };
        let owner = DomainInstance {
            declaration: owner_symbol,
            identity: owner.semantic_id,
        };
        if self
            .content_projections
            .iter()
            .any(|row| row.owner == owner)
        {
            return Err(unserved(
                id,
                "qualification publishes more than one content projection",
            ));
        }
        // This compact value is a report hint only. The expression and exact
        // owner are published alongside it and independently judged downstream.
        let mut fingerprint = 0xcbf29ce484222325_u64;
        for byte in format!("{algebra:?}{expression:?}").bytes() {
            fingerprint = (fingerprint ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
        let index = self.content_projections.len();
        let domain = ContentDomainId::new(index as u64 + 1)
            .ok_or_else(|| unserved(id, "content projection identity overflow"))?;
        self.admitted_declarations.include(machine_symbol);
        self.admitted_declarations.include(owner_symbol);
        self.content_projections.push(ProjectionDefinition {
            owner,
            machine: machine_symbol,
            projection: StructuralContentProjection {
                identity: ContentProjectionIdentity {
                    domain,
                    projection_report_fingerprint: fingerprint,
                },
                algebra,
                expression,
            },
        });
        Ok(Some(index))
    }
}

impl Program<'_> {
    pub(super) fn content_conservation_clause(
        &mut self,
        id: MachineId,
        scope: &super::contracts::ClauseScope<'_>,
        expression: ExpressionHandle,
    ) -> Checked<Option<semantic_vocabulary::Proposition>> {
        let typed = self.typed;
        let ExpressionNode::Call(call) = typed.expression_table.expression(expression) else {
            return Ok(None);
        };
        if typed
            .symbols
            .fixed_core_declaration_for_symbol(call.target_symbol)
            != Some(symbols::FixedCoreDeclaration::Identity)
        {
            return Ok(None);
        }
        let [argument] = call.machine_arguments.as_ref() else {
            return Ok(None);
        };
        let Some(algebra) = algebra(typed, argument.type_reference) else {
            return Ok(None);
        };
        if call.receiver.is_valid()
            || !call.evidence_arguments.is_empty()
            || argument.const_literal.is_some()
            || argument.evidence_projection.is_some()
        {
            return Err(unserved(
                id,
                "content identity contains executable or evidence arguments",
            ));
        }
        let [left, right] = typed.expression_table.expression_handles(call.arguments) else {
            return Err(unserved(
                id,
                "content identity requires exactly two content terms",
            ));
        };
        let left = self.content_term(id, scope, *left, &algebra, 0)?;
        let right = self.content_term(id, scope, *right, &algebra, 0)?;
        Ok(Some(semantic_vocabulary::Proposition::ContentConservation(
            semantic_vocabulary::ContentConservation::new(algebra, left, right),
        )))
    }

    fn content_term(
        &mut self,
        id: MachineId,
        scope: &super::contracts::ClauseScope<'_>,
        expression: ExpressionHandle,
        algebra: &ContentAlgebra,
        depth: usize,
    ) -> Checked<semantic_vocabulary::ContentTerm> {
        if depth > 256 || !expression.is_valid() {
            return Err(unserved(id, "content term exceeds the closed fragment"));
        }
        let typed = self.typed;
        let ExpressionNode::Call(call) = typed.expression_table.expression(expression) else {
            return Err(unserved(
                id,
                "content term does not call its exact owner projection",
            ));
        };
        if call.receiver.is_valid()
            || !call.machine_arguments.is_empty()
            || !call.evidence_arguments.is_empty()
        {
            return Err(unserved(
                id,
                "content term has an unclosed callable application",
            ));
        }
        let arguments = typed.expression_table.expression_handles(call.arguments);
        if arguments.len() != call.arguments.len() {
            return Err(unserved(id, "content term has an invalid argument span"));
        }
        if typed
            .symbols
            .builtin_function_for_symbol(call.target_symbol)
            == Some(BuiltinFunction::ContentSeparate)
        {
            let mut terms = Vec::with_capacity(arguments.len());
            for argument in arguments {
                terms.push(self.content_term(id, scope, *argument, algebra, depth + 1)?);
            }
            return semantic_vocabulary::ContentTerm::separate(terms)
                .map_err(|_| unserved(id, "content separation repeats a subject or has no terms"));
        }
        let Some(projection_index) = self.prepare_content_projection(id, call.target_symbol)?
        else {
            return Err(unserved(
                id,
                "content term calls no owner-published Core Content projection",
            ));
        };
        let declared = &self.content_projections[projection_index];
        if declared.projection.algebra != *algebra {
            return Err(unserved(
                id,
                "content terms use different coordinate spaces or units",
            ));
        }
        let projection = declared.projection.identity;
        let [subject] = arguments else {
            return Err(unserved(
                id,
                "content projection requires exactly one subject",
            ));
        };
        let mut subject = *subject;
        let mut version = semantic_vocabulary::ContentPlaceVersion::Current;
        if let ExpressionNode::Call(old) = typed.expression_table.expression(subject)
            && typed.symbols.builtin_function_for_symbol(old.target_symbol)
                == Some(BuiltinFunction::ContentOld)
        {
            let [entry] = typed.expression_table.expression_handles(old.arguments) else {
                return Err(unserved(id, "old content subject requires one place"));
            };
            if old.receiver.is_valid()
                || !old.machine_arguments.is_empty()
                || !old.evidence_arguments.is_empty()
            {
                return Err(unserved(
                    id,
                    "old content subject contains executable application metadata",
                ));
            }
            version = semantic_vocabulary::ContentPlaceVersion::Entry;
            subject = *entry;
        }
        let ExpressionNode::Borrow(borrow) = typed.expression_table.expression(subject) else {
            return Err(unserved(
                id,
                "content projection requires an explicit shared subject loan",
            ));
        };
        if borrow.access != language_core::ReferenceAccess::Shared {
            return Err(unserved(
                id,
                "content projection cannot take an exclusive subject loan",
            ));
        }
        let subject_scope = if version == semantic_vocabulary::ContentPlaceVersion::Entry {
            super::contracts::ClauseScope {
                result: super::contracts::ResultTerm::Absent("old content cannot name the result"),
                ..*scope
            }
        } else {
            *scope
        };
        let Some((root, path, value_type)) = self.projected(id, &subject_scope, borrow.target)?
        else {
            return Err(unserved(
                id,
                "content subject has no exact structural place",
            ));
        };
        if !matches!(value_type, crate::body::ValueType::Record(..)) {
            return Err(unserved(
                id,
                "content subject is not its declared structural carrier",
            ));
        }
        let segments = self.content_path(id, &path)?;
        Ok(semantic_vocabulary::ContentTerm::Projection {
            projection,
            subject: semantic_vocabulary::ContentStructuralPlace {
                version,
                root,
                segments,
            },
        })
    }

    fn content_path(
        &self,
        id: MachineId,
        path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
    ) -> Checked<Vec<semantic_vocabulary::ContentPlaceSegment>> {
        use semantic_vocabulary::{
            CanonicalStructuralPathSegment as Path, ContentPlaceSegment as Content,
        };
        let mut output = Vec::with_capacity(path.len());
        for segment in path {
            output.push(match segment {
                Path::FixedIndex(index) => Content::FixedIndex(*index),
                Path::Field(field) => {
                    let mut fields = self
                        .records
                        .iter()
                        .flat_map(|(_, record)| {
                            record
                                .fields()
                                .iter()
                                .chain(record.cases().iter().flat_map(|case| &case.fields))
                        })
                        .filter(|candidate| candidate.id == *field);
                    let Some(declared) = fields.next() else {
                        return Err(unserved(
                            id,
                            "content subject lost a canonical field identity",
                        ));
                    };
                    if fields.next().is_some() {
                        return Err(unserved(
                            id,
                            "content subject has a repeated field identity",
                        ));
                    }
                    Content::Field(declared.identity.clone())
                }
                Path::Case(case) => {
                    let mut cases = self
                        .records
                        .iter()
                        .flat_map(|(_, record)| match &record.shape {
                            crate::body::StorageShape::Sum(cases) => cases.as_slice(),
                            _ => &[],
                        })
                        .filter(|candidate| candidate.id == *case);
                    let Some(declared) = cases.next() else {
                        return Err(unserved(
                            id,
                            "content subject lost a canonical case identity",
                        ));
                    };
                    if cases.next().is_some() {
                        return Err(unserved(id, "content subject has a repeated case identity"));
                    }
                    Content::Case(declared.identity.clone())
                }
                Path::Referent => {
                    return Err(unserved(id, "content subject crosses a retained reference"));
                }
            });
        }
        Ok(output)
    }
}

/// Add independent qualification facets to the already assigned physical
/// claims. A carrier with two content qualifications keeps one claim ID and
/// two projections; neither its geometry nor a sibling facet creates authority.
pub(crate) fn publish_content_entry_claims(
    diagnostic: MachineId,
    module: &mut terminal_psi::TerminalModule,
) -> Checked<()> {
    let types = &module.structural_types;
    let domains = &module.structural_domains;
    let identities = &module.identities;
    for header in &mut module.declaration_contracts {
        publish_header_content(diagnostic, types, domains, identities, header)?;
    }
    for machine in &mut module.machines {
        publish_header_content(
            machine.id,
            types,
            domains,
            identities,
            &mut machine.contract,
        )?;
    }
    Ok(())
}

fn publish_header_content(
    diagnostic: MachineId,
    types: &[terminal_psi::StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    identities: &terminal_psi::TerminalIdentityTable,
    header: &mut terminal_psi::MachineContract,
) -> Checked<()> {
    use semantic_vocabulary::{ContentPlaceSegment, ContentPlaceVersion, ContentStructuralPlace};
    use terminal_psi::{
        ClaimContentProjection, ContentEntryClaim, StructuralAccess, StructuralPathSegment,
    };
    let mut entries = Vec::new();
    for claim in &header.entry_claims {
        let formal = header
            .structural_parameters
            .iter()
            .find(|formal| formal.place == claim.input)
            .ok_or_else(|| {
                unserved(
                    diagnostic,
                    "content facet has no exact ordinary formal claim",
                )
            })?;
        if formal.access != StructuralAccess::Owned {
            return Err(unserved(
                diagnostic,
                "content facet changes borrowed input into ownership",
            ));
        }
        let carrier = terminal_semantics::runtime_structural_path_tip(
            types.iter(),
            formal.structural_type,
            &claim.path,
        )
        .ok_or_else(|| unserved(diagnostic, "content facet has no exact typed claim path"))?;
        let (memberships, _) = terminal_psi::StructuralPathQualification::select(
            &formal.qualifications,
            &formal.projected_qualifications,
            &claim.path,
        );
        let mut projections = Vec::new();
        for membership in memberships {
            let owner = domains
                .iter()
                .find(|domain| domain.id == membership)
                .ok_or_else(|| {
                    unserved(diagnostic, "content facet names a missing qualification")
                })?;
            let Some(projection) = &owner.content_projection else {
                continue;
            };
            if owner.carrier != carrier.id {
                return Err(unserved(
                    diagnostic,
                    "content projection changes its qualified carrier",
                ));
            }
            projections.push(ClaimContentProjection {
                projection: projection.identity,
                algebra: projection.algebra.clone(),
            });
        }
        if projections.is_empty() {
            continue;
        }
        projections.sort_unstable();
        if projections.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(unserved(diagnostic, "content facet repeats a projection"));
        }
        let mut segments = Vec::with_capacity(claim.path.len());
        let mut prefix = Vec::new();
        for step in &claim.path {
            segments.push(match step {
                StructuralPathSegment::Field(field) => ContentPlaceSegment::Field(
                    identities
                        .text(*field)
                        .ok_or_else(|| {
                            unserved(diagnostic, "content facet field has no canonical identity")
                        })?
                        .to_owned(),
                ),
                StructuralPathSegment::FixedIndex(index) => ContentPlaceSegment::FixedIndex(*index),
                StructuralPathSegment::Case(case) => {
                    let parent = terminal_semantics::runtime_structural_path_tip(
                        types.iter(),
                        formal.structural_type,
                        &prefix,
                    )
                    .ok_or_else(|| {
                        unserved(diagnostic, "content facet case has no canonical parent")
                    })?;
                    let selected = parent
                        .shape
                        .sum_cases()
                        .and_then(|cases| cases.iter().find(|row| row.id == *case))
                        .ok_or_else(|| {
                            unserved(diagnostic, "content facet case belongs to another parent")
                        })?;
                    ContentPlaceSegment::Case(
                        identities
                            .text(selected.identity)
                            .ok_or_else(|| {
                                unserved(diagnostic, "content facet case has no identity")
                            })?
                            .to_owned(),
                    )
                }
                StructuralPathSegment::AllElements
                | StructuralPathSegment::FixedRange { .. }
                | StructuralPathSegment::RuntimeIndex { .. }
                | StructuralPathSegment::RepresentationView { .. }
                | StructuralPathSegment::Referent => {
                    return Err(unserved(
                        diagnostic,
                        "content facet is not one closed owned structural path",
                    ));
                }
            });
            prefix.push(step.clone());
        }
        entries.push(ContentEntryClaim {
            claim: claim.claim,
            input: ContentStructuralPlace {
                version: ContentPlaceVersion::Entry,
                root: claim.input,
                segments,
            },
            projections,
        });
    }
    if !header.content_entry_claims.is_empty() && header.content_entry_claims != entries {
        return Err(unserved(
            diagnostic,
            "content entry facets disagree with their ordinary claim owners",
        ));
    }
    header.content_entry_claims = entries;
    Ok(())
}
