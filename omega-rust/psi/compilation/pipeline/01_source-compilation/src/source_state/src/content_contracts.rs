//! Retain authored conservation equations without treating them as evidence.
//! The exact projection definitions travel with the equation; report hashes are
//! diagnostic commitments, not substitutes for checking those definitions.

use crate::source_state::TypedTrees;
use crate::source_state::content_projections::ContentProjection;
use crate::source_state::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use crate::source_state::signature::StateParameter;
use declaration_contracts::content::{
    ContentAlgebraIdentity, ContentConservationEquation, ContentConservationTerm, ContentPlaceRoot,
    ContentPlaceVersion, projection_report_fingerprint,
};
use symbols::{BuiltinFunction, FixedCoreDeclaration, SymbolHandle};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentConservationClause {
    pub algebra: ContentAlgebraIdentity,
    pub equation: ContentConservationEquation,
    pub projections: Vec<ContentProjection>,
}

impl TypedTrees {
    /// Read a fixed kernel identity over a content algebra. Ordinary identities
    /// remain with their own reader. Parameter/result formation, conservation,
    /// and the complete callable contract still need their normal judgments.
    pub fn content_conservation_clause(
        &self,
        owner: SymbolHandle,
        parameters: arena::RootRows<'_, StateParameter>,
        expression: ExpressionHandle,
    ) -> Result<Option<ContentConservationClause>, &'static str> {
        if !self.expression_table.expression_is_valid(expression) {
            return Err("content clause lost its expression");
        }
        let ExpressionNode::Call(call) = self.expression_table.expression(expression) else {
            return Ok(None);
        };
        if self
            .symbols
            .fixed_core_declaration_for_symbol(call.target_symbol)
            != Some(FixedCoreDeclaration::Identity)
        {
            return Ok(None);
        }
        if !plain_route(call) || !call.evidence_arguments.is_empty() {
            return Err("content identity has an extra execution or evidence selection");
        }
        let [argument] = call.machine_arguments.as_ref() else {
            return Err("kernel identity needs its one complete carrier argument");
        };
        let Some(algebra) = self.content_algebra_identity(argument.type_reference)? else {
            return Ok(None);
        };
        let [left, right] = self.expression_table.expression_handles(call.arguments) else {
            return Err("content identity needs two exact terms");
        };
        let mut reader = ClauseReader {
            typed: self,
            owner,
            parameters,
            algebra: &algebra,
            projections: Vec::new(),
            remaining: self.expression_table.expression_count(),
        };
        let left = reader.term(*left)?;
        let right = reader.term(*right)?;
        Ok(Some(ContentConservationClause {
            algebra: algebra.clone(),
            equation: ContentConservationEquation::new(left, right),
            projections: reader.projections,
        }))
    }
}

fn plain_route(call: &TableCallExpression) -> bool {
    !call.receiver.is_valid()
        && !call.static_machine_parameter.is_valid()
        && call.selects_only_nominal_route()
        && !call.requirement_application.is_valid()
        && !call.build_selection.is_valid()
        && !call.exact_coercion_target.is_valid()
        && !call.operational_acknowledgement.acknowledges_suspend
        && !call.operational_acknowledgement.acknowledges_block
}

struct ClauseReader<'a, 'parameters> {
    typed: &'a TypedTrees,
    owner: SymbolHandle,
    parameters: arena::RootRows<'parameters, StateParameter>,
    algebra: &'a ContentAlgebraIdentity,
    projections: Vec<ContentProjection>,
    remaining: usize,
}

impl ClauseReader<'_, '_> {
    fn term(
        &mut self,
        expression: ExpressionHandle,
    ) -> Result<ContentConservationTerm, &'static str> {
        if self.remaining == 0 || !self.typed.expression_table.expression_is_valid(expression) {
            return Err("content term expansion exceeds its retained source or is cyclic");
        }
        self.remaining -= 1;
        let ExpressionNode::Call(call) = self.typed.expression_table.expression(expression) else {
            return Err("content term is not an exact projection or separation");
        };
        if !plain_route(call) || !call.carries_only_positional_arguments() {
            return Err("content term has an extra execution or application selection");
        }
        let arguments = self
            .typed
            .expression_table
            .expression_handles(call.arguments);
        if self
            .typed
            .symbols
            .builtin_function_for_symbol(call.target_symbol)
            == Some(BuiltinFunction::ContentSeparate)
        {
            if arguments.len() < 2 {
                return Err("content separation needs at least two terms");
            }
            let terms = arguments
                .iter()
                .map(|argument| self.term(*argument))
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(ContentConservationTerm::separate(terms));
        }
        let projection = self
            .typed
            .content_projection(call.target_symbol)?
            .ok_or("content term does not name an exact Content projection")?;
        if &projection.algebra != self.algebra {
            return Err("content equation mixes distinct algebras, coordinate spaces or units");
        }
        let [argument] = arguments else {
            return Err("content projection call needs its one exact subject");
        };
        let mut subject = *argument;
        if !self.typed.expression_table.expression_is_valid(subject) {
            return Err("content projection subject is missing");
        }
        let mut version = ContentPlaceVersion::Current;
        if let ExpressionNode::Call(old) = self.typed.expression_table.expression(subject)
            && self
                .typed
                .symbols
                .builtin_function_for_symbol(old.target_symbol)
                == Some(BuiltinFunction::ContentOld)
        {
            if !plain_route(old) || !old.carries_only_positional_arguments() {
                return Err("old content subject has an extra execution or application selection");
            }
            let [argument] = self
                .typed
                .expression_table
                .expression_handles(old.arguments)
            else {
                return Err("old content subject needs one exact place");
            };
            subject = *argument;
            version = ContentPlaceVersion::Entry;
        }
        if !self.typed.expression_table.expression_is_valid(subject) {
            return Err("content projection subject is missing");
        }
        let ExpressionNode::Borrow(borrow) = self.typed.expression_table.expression(subject) else {
            return Err("content projection subject must name a shared structural place");
        };
        if borrow.access != language_core::ReferenceAccess::Shared {
            return Err("content projection subject changes access through its proof-only borrow");
        }
        let mut subject = super::contract_structure::structural_subject_with(
            self.typed,
            self.owner,
            borrow.target,
            |name| {
                if name.reserved_result_of.is_valid() {
                    if name.reserved_result_of != self.owner {
                        return Err("content result belongs to another contract");
                    }
                    return Ok(ContentPlaceRoot::Result);
                }
                let (position, parameter) = self
                    .parameters
                    .iter()
                    .enumerate()
                    .find(|(_, parameter)| parameter.symbol == name.symbol)
                    .ok_or("content subject is not an ordinary contract parameter")?;
                if parameter.is_const || parameter.relevance.is_erased() {
                    return Err("content subject is not an executable parameter");
                }
                Ok(ContentPlaceRoot::Parameter {
                    position: u32::try_from(position)
                        .map_err(|_| "content parameter ordinal is too large")?,
                    symbol: parameter.symbol,
                    name: parameter.name.to_string(),
                })
            },
        )?;
        if version == ContentPlaceVersion::Entry && matches!(subject.root, ContentPlaceRoot::Result)
        {
            return Err("old content cannot name the normal result");
        }
        subject.version = version;
        let domain = self
            .typed
            .domain_definition(projection.domain)
            .ok_or("content projection lost its exact qualification declaration")?;
        let term = ContentConservationTerm::Projection {
            domain: projection.domain,
            semantic_domain: domain.semantic_id,
            projection_machine: projection.machine,
            projection_report_fingerprint: projection_report_fingerprint(
                &projection.algebra,
                &projection.expression,
            ),
            subject,
        };
        if !self
            .projections
            .iter()
            .any(|existing| existing == &projection)
        {
            self.projections.push(projection);
        }
        Ok(term)
    }
}
