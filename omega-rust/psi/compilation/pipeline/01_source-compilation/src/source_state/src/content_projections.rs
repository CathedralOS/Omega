//! Form the declared meaning of a resource-content projection from typed source.
//! Both public contract publication and Terminal lowering use this owner. A
//! normalized expression is a definition, not evidence of separation, custody or
//! even that its partial arithmetic is formed. Checking must prove those goals.

use crate::source_state::TypedTrees;
use crate::source_state::data::DataMember;
use crate::source_state::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use crate::source_state::statement::StatementNode;
use crate::source_state::type_system::source_projections;
use crate::source_state::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
use declaration_contracts::content::{
    ContentAlgebraIdentity, ContentArithmeticOperator, ContentFieldSegment,
    ContentIntervalExpression, ContentProjectionExpression, ContentScalarExpression,
};
use language_core::core_vocabulary::{ContentDeclaration, CoreVocabulary};
use symbols::{BuiltinFunction, SymbolHandle};

/// Partial source operands remain proof questions even when their final
/// mathematical expression simplifies to a total one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentProjectionFormation {
    Natural(ContentScalarExpression),
    OrderedInterval {
        start: ContentScalarExpression,
        end: ContentScalarExpression,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentProjection {
    pub domain: SymbolHandle,
    pub machine: SymbolHandle,
    pub subject: TypeReferenceHandle,
    pub algebra: ContentAlgebraIdentity,
    pub expression: ContentProjectionExpression,
    pub formation: Vec<ContentProjectionFormation>,
}

impl TypedTrees {
    /// Identify an already substituted algebra occurrence. This supplies no
    /// separation or arithmetic evidence and never evaluates an index.
    pub fn content_algebra_identity(
        &self,
        reference: TypeReferenceHandle,
    ) -> Result<Option<ContentAlgebraIdentity>, &'static str> {
        if !self.contains_type_reference(reference) {
            return Err("content algebra lost its retained type reference");
        }
        let TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            lifetime_arguments,
            ..
        } = self.type_reference_table.type_reference(reference)
        else {
            return Ok(None);
        };
        let kind = self.symbols.core_vocabulary(*base_symbol);
        if !matches!(
            kind,
            Some(CoreVocabulary::Content(
                ContentDeclaration::IntervalSet | ContentDeclaration::CountedQuantity
            ))
        ) {
            return Ok(None);
        }
        let [parameter] = self.type_reference_table.type_reference_handles(*arguments) else {
            return Err("content algebra needs one exact coordinate space or unit");
        };
        if !lifetime_arguments.is_empty() || !self.contains_type_reference(*parameter) {
            return Err("content algebra lost its complete parameter application");
        }
        let parameter = self.normalized_type_identity_text(*parameter);
        Ok(Some(match kind {
            Some(CoreVocabulary::Content(ContentDeclaration::IntervalSet)) => {
                ContentAlgebraIdentity::IntervalSet {
                    coordinate_space: parameter,
                }
            }
            Some(CoreVocabulary::Content(ContentDeclaration::CountedQuantity)) => {
                ContentAlgebraIdentity::CountedQuantity { unit: parameter }
            }
            _ => unreachable!("the exact algebra kind was checked above"),
        }))
    }
    /// Prepare the owner-unique projection roster once for publication or
    /// checking. Qualified machines are flat declarations, not domain children.
    pub fn content_projections(&self) -> Result<Vec<ContentProjection>, &'static str> {
        let mut projections = self
            .machines()
            .iter()
            .filter_map(|machine| self.content_projection(machine.symbol).transpose())
            .collect::<Result<Vec<_>, _>>()?;
        projections.sort_unstable_by_key(|projection| {
            (
                projection.domain.generation(),
                projection.domain.arena_index(),
            )
        });
        if projections
            .windows(2)
            .any(|pair| pair[0].domain == pair[1].domain)
        {
            return Err("a qualification has more than one Content projection");
        }
        Ok(projections)
    }

    /// Translate the closed projection fragment. The returned field paths keep
    /// exact source identities for the consumer to join to its structural rows.
    pub fn content_projection(
        &self,
        symbol: SymbolHandle,
    ) -> Result<Option<ContentProjection>, &'static str> {
        let Some(machine) = self.machine_by_symbol(symbol).or_else(|| {
            self.machine_by_entry_state_symbol(symbol)
                .map(|(machine, _)| machine)
        }) else {
            return Ok(None);
        };
        let symbol = machine.symbol;
        let mut edges = self
            .machine_trait_conformances(machine)
            .iter()
            .filter(|edge| {
                self.symbols.core_vocabulary(edge.symbol)
                    == Some(CoreVocabulary::Content(ContentDeclaration::Interface))
            });
        let Some(edge) = edges.next() else {
            return Ok(None);
        };
        if edges.next().is_some() {
            return Err("a content projection repeats its Content conformance");
        }
        if self.symbols.core_vocabulary(edge.requirement_symbol)
            != Some(CoreVocabulary::Content(ContentDeclaration::Project))
        {
            return Err("content projection has no exact Content project requirement");
        }
        let projection_span = self
            .symbols
            .symbol_source_span(symbol)
            .ok_or("content projection lost its authored source")?;
        let namespace = machine
            .attached_data
            .as_ref()
            .ok_or("a Content projection needs its qualification namespace")?;
        let domain = match crate::source_symbols::membership_domain_lookup(
            &self.symbols,
            namespace.as_str(),
            projection_span,
        ) {
            crate::source_symbols::DomainMembership::Unique(domain) => domain,
            _ => return Err("content projection namespace has no unique visible qualification"),
        };
        let definition = self
            .domain_definition(domain)
            .ok_or("a Content projection needs its qualification namespace")?;
        let domain_span = self
            .symbols
            .symbol_source_span(domain)
            .ok_or("content qualification lost its authored source")?;
        if !self
            .symbols
            .same_source_package(domain_span, projection_span)
        {
            return Err("only the qualification owner may define its content projection");
        }
        let arguments = self
            .type_reference_table
            .type_reference_handles(edge.arguments);
        let [subject, algebra] = arguments else {
            return Err("Content projection lost its complete owner and algebra application");
        };
        if ![*subject, *algebra, definition.target_type]
            .into_iter()
            .all(|reference| self.contains_type_reference(reference))
        {
            return Err("content projection lost an owner or algebra type reference");
        }
        if self.normalized_type_identity(*subject)
            != self.normalized_type_identity(definition.target_type)
        {
            return Err("Content projection selects another qualification carrier");
        }
        let states = self.machine_states(machine);
        if states.len() != 1 {
            return Err("a content projection must have one expression body");
        }
        let entry = states.first().ok_or("content projection lost its entry")?;
        let parameters = self.state_parameters(entry);
        if parameters.len() != 1 {
            return Err("a content projection must have one shared subject");
        }
        let parameter = parameters
            .first()
            .ok_or("content projection lost its subject")?;
        if parameter.is_const || parameter.relevance.is_erased() {
            return Err("a content projection needs its shared executable subject");
        }
        if !self.contains_type_reference(parameter.type_reference)
            || !self.contains_type_reference(entry.return_type)
        {
            return Err("content projection lost its subject or result type");
        }
        let TypeReferenceNode::Reference {
            referee, access, ..
        } = self
            .type_reference_table
            .type_reference(parameter.type_reference)
        else {
            return Err("a content projection subject must be a shared reference");
        };
        if !self.contains_type_reference(*referee) {
            return Err("content projection reference lost its exact subject type");
        }
        if *access != language_core::ReferenceAccess::Shared
            || self.normalized_type_identity(*referee) != self.normalized_type_identity(*subject)
        {
            return Err("content projection subject changes its declared owner or access");
        }
        if self.normalized_type_identity(entry.return_type)
            != self.normalized_type_identity(*algebra)
        {
            return Err("content projection returns another algebra application");
        }
        if machine.suspends || machine.blocks || !machine.invokes.is_empty() {
            return Err("a content projection cannot suspend, block or invoke a service");
        }
        let [StatementNode::Expression(body)] =
            self.statement_table.statements(entry.statement_nodes)
        else {
            return Err("a content projection body is not one closed expression");
        };
        if !self.expression_table.expression_is_valid(*body) {
            return Err("content projection lost its body expression");
        }
        let mut reader = ProjectionReader {
            typed: self,
            owner: symbol,
            parameter: parameter.symbol,
            subject: *subject,
            formation: Vec::new(),
        };
        let (algebra, expression) = reader.expression(*algebra, *body)?;
        Ok(Some(ContentProjection {
            domain,
            machine: symbol,
            subject: *subject,
            algebra,
            expression,
            formation: reader.formation,
        }))
    }
}

struct ProjectionReader<'a> {
    typed: &'a TypedTrees,
    owner: SymbolHandle,
    parameter: SymbolHandle,
    subject: TypeReferenceHandle,
    formation: Vec<ContentProjectionFormation>,
}

impl ProjectionReader<'_> {
    fn expression(
        &mut self,
        algebra: TypeReferenceHandle,
        body: ExpressionHandle,
    ) -> Result<(ContentAlgebraIdentity, ContentProjectionExpression), &'static str> {
        let typed = self.typed;
        let identity = typed
            .content_algebra_identity(algebra)?
            .ok_or("content projection uses an unrecognized algebra declaration")?;
        let TypeReferenceNode::Generic { base_symbol, .. } =
            typed.type_reference_table.type_reference(algebra)
        else {
            return Err("content projection algebra is not a complete application");
        };
        let ExpressionNode::StructLiteral(literal) = typed.expression_table.expression(body) else {
            return Err("content projection does not construct its declared algebra");
        };
        if !typed.contains_type_reference(literal.application_type) {
            return Err("content projection constructor lost its applied result type");
        }
        if literal.zero_omitted_fields
            || literal.case_symbol.is_some()
            || literal.case_name.is_some()
            || typed.normalized_type_identity(literal.application_type)
                != typed.normalized_type_identity(algebra)
        {
            return Err("content projection constructor changes its exact algebra application");
        }
        let data = typed
            .unique_data_definition_by_symbol(*base_symbol)
            .ok_or("content algebra lost its exact declaration")?;
        let mut values = Vec::with_capacity(typed.data_members(data).len());
        let fields = typed.expression_table.struct_fields(literal.fields);
        if fields.len() != typed.data_members(data).len() {
            return Err("content algebra construction omits or repeats a field");
        }
        for member in typed.data_members(data) {
            let DataMember::Field(field) = member else {
                return Err("content algebra declaration contains a case");
            };
            let actual = typed
                .data_constructor_field(algebra, *base_symbol, field.symbol)
                .ok_or("content algebra field lost its applied declaration")?;
            let mut selected = fields
                .iter()
                .filter(|value| value.field_symbol == actual.symbol);
            let value = selected
                .next()
                .ok_or("content algebra construction names another field")?;
            if selected.next().is_some() {
                return Err("content algebra construction repeats a field");
            }
            values.push(self.scalar(value.value, typed.expression_table.expression_count())?);
        }
        match (identity, values.as_slice()) {
            (identity @ ContentAlgebraIdentity::IntervalSet { .. }, [start, end]) => {
                self.formation
                    .push(ContentProjectionFormation::Natural(start.clone()));
                self.formation
                    .push(ContentProjectionFormation::Natural(end.clone()));
                self.formation
                    .push(ContentProjectionFormation::OrderedInterval {
                        start: start.clone(),
                        end: end.clone(),
                    });
                Ok((
                    identity,
                    ContentProjectionExpression::IntervalSet {
                        members: vec![ContentIntervalExpression::new(start.clone(), end.clone())],
                    },
                ))
            }
            (identity @ ContentAlgebraIdentity::CountedQuantity { .. }, [magnitude]) => {
                self.formation
                    .push(ContentProjectionFormation::Natural(magnitude.clone()));
                Ok((
                    identity,
                    ContentProjectionExpression::CountedQuantity {
                        magnitude: magnitude.clone(),
                    },
                ))
            }
            _ => Err("content projection uses an unrecognized algebra declaration or shape"),
        }
    }

    fn scalar(
        &mut self,
        expression: ExpressionHandle,
        remaining: usize,
    ) -> Result<ContentScalarExpression, &'static str> {
        if remaining == 0 || !self.typed.expression_table.expression_is_valid(expression) {
            return Err("content projection contains a missing or cyclic expression");
        }
        let remaining = remaining - 1;
        match self.typed.expression_table.expression(expression) {
            ExpressionNode::Integer(value) if value.landing().is_none() && !value.is_negative() => {
                let value = value
                    .value_bignum()
                    .ok_or("content projection has an invalid natural literal")?;
                Ok(ContentScalarExpression::Natural(value.to_string()))
            }
            ExpressionNode::Cast(cast) => {
                if !self.typed.contains_type_reference(cast.target_type) {
                    return Err("content projection conversion lost its target type");
                }
                if cast.form != language_core::cast_form::CastForm::Value
                    || cast.domain != numerics::arithmetic::ArithmeticDomain::Exact
                    || !cast.semantic_domain.is_empty()
                    || !cast.semantic_domain_arguments.is_empty()
                    || cast.semantic_domain_symbol.is_valid()
                {
                    return Err("content projection conversion changes a qualification or view");
                }
                let TypeReferenceNode::Named { symbol, .. } = self
                    .typed
                    .type_reference_table
                    .type_reference(cast.target_type)
                else {
                    return Err("content projection conversion has no exact natural target");
                };
                if self.typed.symbols.core_vocabulary(*symbol)
                    != Some(CoreVocabulary::NaturalNumber)
                {
                    return Err(
                        "content projection conversion does not target core natural numbers",
                    );
                }
                let value = self.scalar(cast.value, remaining)?;
                self.formation
                    .push(ContentProjectionFormation::Natural(value.clone()));
                Ok(value)
            }
            ExpressionNode::Call(call)
                if self
                    .typed
                    .symbols
                    .builtin_function_for_symbol(call.target_symbol)
                    == Some(BuiltinFunction::IntegerEmbed) =>
            {
                if !call.is_ordinary_positional_call()
                    || call.requirement_application.is_valid()
                    || call.build_selection.is_valid()
                    || call.exact_coercion_target.is_valid()
                    || call.operational_acknowledgement.acknowledges_suspend
                    || call.operational_acknowledgement.acknowledges_block
                {
                    return Err(
                        "content embedding has an extra execution or application selection",
                    );
                }
                let [argument] = self
                    .typed
                    .expression_table
                    .expression_handles(call.arguments)
                else {
                    return Err("content embedding needs one exact subject field");
                };
                let (fields, carrier) = self.path(*argument, remaining)?;
                let supported = match self.typed.primitive_type_reference(carrier) {
                    Some(PrimitiveType::Addr) => true,
                    Some(PrimitiveType::Integer(carrier)) => {
                        !carrier.is_signed() && matches!(carrier.width(), 8 | 16 | 32 | 64)
                    }
                    _ => false,
                };
                if !supported {
                    return Err("content embedding field has an unsupported integer carrier");
                }
                Ok(ContentScalarExpression::RuntimeScalarEmbedding(fields))
            }
            ExpressionNode::Binary(binary) => {
                if binary.policy != numerics::arithmetic::ArithmeticDomain::Exact {
                    return Err("content projection arithmetic must be mathematical and exact");
                }
                let operator = match binary.operator {
                    BinaryOperator::Add => ContentArithmeticOperator::Add,
                    BinaryOperator::Subtract => ContentArithmeticOperator::Subtract,
                    BinaryOperator::Multiply => ContentArithmeticOperator::Multiply,
                    _ => return Err("content projection uses an unsupported arithmetic operation"),
                };
                Ok(ContentScalarExpression::Arithmetic {
                    operator,
                    left: Box::new(self.scalar(binary.left, remaining)?),
                    right: Box::new(self.scalar(binary.right, remaining)?),
                })
            }
            _ => Err("content projection uses a call, branch or value outside the closed fragment"),
        }
    }

    fn path(
        &self,
        expression: ExpressionHandle,
        remaining: usize,
    ) -> Result<(Vec<ContentFieldSegment>, TypeReferenceHandle), &'static str> {
        use declaration_contracts::content::{ContentPlaceRoot, ContentPlaceSegment};
        if remaining == 0 {
            return Err("content projection contains a cyclic field path");
        }
        let subject = super::contract_structure::structural_subject_with(
            self.typed,
            self.owner,
            expression,
            |name| {
                if name.symbol != self.parameter || name.reserved_result_of.is_valid() {
                    return Err(
                        "content projection reads something other than its subject's fields",
                    );
                }
                Ok(ContentPlaceRoot::Parameter {
                    position: 0,
                    symbol: self.parameter,
                    name: self.typed.symbols.name(self.parameter).to_owned(),
                })
            },
        )?;
        let mut carrier = self.subject;
        let mut path = Vec::with_capacity(subject.segments.len());
        for segment in subject.segments {
            let ContentPlaceSegment::Field(field) = segment else {
                return Err("content projection needs unconditional subject fields");
            };
            carrier = source_projections::field_type_reference(
                self.typed,
                carrier,
                field.symbol,
                &mut Vec::new(),
            )
            .ok_or("content projection field is not a member of its exact subject")?;
            path.push(field);
        }
        Ok((path, carrier))
    }
}
