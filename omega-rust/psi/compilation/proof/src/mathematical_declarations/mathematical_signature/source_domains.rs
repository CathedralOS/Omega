//! Source domains survive calculus conversion as transparent carrier aliases.
//! Their bodies remain the existing Int/format/address carriers; only this
//! source view retains their heads while reducing the same checked dependent
//! application spine. This adds carrier/policy obligations, never value laws.

use super::{Elaborator, ScalarCarrier};
use diagnostics::Diagnostic;
use proof_admission::{
    Budget, Context, DEFAULT_CONVERSION_STEPS, Declaration, Level, Signature, Sort, Term,
    TermHandle, convertible_retaining, infer_type, substitute, weak_head_normalize_retaining,
};
use typed_trees::expression::ExpressionHandle;
use typed_trees::type_identity::TypeIdentityRequest;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(super) struct SourceAlias {
    pub reference: TypeReferenceHandle,
    pub declaration: u32,
    pub carrier: ScalarCarrier,
}

pub(super) struct SourceInput {
    pub expression: ExpressionHandle,
    pub term: TermHandle,
    /// A source-typed ordinary value's complete domain. Mathematical function
    /// terms instead use their instantiated calculus telescope directly.
    pub domain: Option<TermHandle>,
}

pub(super) struct SourceInvocation {
    pub expression: ExpressionHandle,
    pub span: Option<source::SourceSpan>,
    pub formation: SourceFormation,
    pub domains: Vec<TermHandle>,
    pub level_arity: u32,
}

pub(super) enum SourceFormation {
    Application {
        callee: TermHandle,
        inputs: Vec<SourceInput>,
    },
    Result {
        expected: TermHandle,
        actual: SourceInput,
    },
}

impl Elaborator<'_> {
    /// Construct all mathematical Apply spines and retain their source-domain
    /// obligations together. Ordinary calls, generic terms and dependent type
    /// applications differ in argument roles, never in application formation.
    pub(super) fn apply_source_inputs(
        &mut self,
        expression: ExpressionHandle,
        callee: TermHandle,
        inputs: Vec<SourceInput>,
    ) -> TermHandle {
        let mut function = callee;
        for input in &inputs {
            function = self.arena.insert(Term::Apply {
                function,
                argument: input.term,
            });
        }
        self.source_invocations.push(SourceInvocation {
            expression,
            span: if expression.is_valid() {
                Some(self.program.expression_table.source_span(expression))
            } else {
                self.span
            },
            formation: SourceFormation::Application { callee, inputs },
            domains: self.scope.iter().map(|entry| entry.domain).collect(),
            level_arity: self.level_binders.len() as u32 + self.generalized_levels,
        });
        function
    }

    pub(super) fn source_expression_input(
        &mut self,
        expression: ExpressionHandle,
    ) -> Result<SourceInput, Vec<Diagnostic>> {
        let term = self.elaborate_expression(expression)?;
        let domain = self.source_expression_domain(expression)?;
        Ok(SourceInput {
            expression,
            term,
            domain,
        })
    }

    pub(super) fn source_expression_domain(
        &mut self,
        expression: ExpressionHandle,
    ) -> Result<Option<TermHandle>, Vec<Diagnostic>> {
        let node = self.program.expression_table.expression(expression);
        // A mathematical function's instantiated type and a type-valued
        // declaration are calculus terms. Ordinary values keep the source
        // result owner's complete reference, including arithmetic policy.
        let reference = if matches!(node, typed_trees::expression::ExpressionNode::Call(call)
            if self.authored_index.contains_key(&call.target_symbol))
            || matches!(node, typed_trees::expression::ExpressionNode::Name(path)
                if self.program.symbols.builtin_type_atom(path.symbol).is_some())
        {
            None
        } else {
            validation::mathematical_applications::operand_type_reference(self.program, expression)
        };
        reference
            .map(|reference| self.elaborate_type_reference(reference))
            .transpose()
    }

    pub(super) fn retain_source_result(
        &mut self,
        expression: ExpressionHandle,
        expected: TermHandle,
        term: TermHandle,
        strict_result: bool,
    ) -> Result<(), Vec<Diagnostic>> {
        let domain = if strict_result {
            None
        } else {
            self.source_expression_domain(expression)?
        };
        self.source_invocations.push(SourceInvocation {
            expression,
            span: Some(self.program.expression_table.source_span(expression)),
            formation: SourceFormation::Result {
                expected,
                actual: SourceInput {
                    expression,
                    term,
                    domain,
                },
            },
            domains: self.scope.iter().map(|entry| entry.domain).collect(),
            level_arity: self.level_binders.len() as u32 + self.generalized_levels,
        });
        Ok(())
    }

    pub(super) fn source_type_alias(
        &mut self,
        reference: TypeReferenceHandle,
    ) -> Result<Option<TermHandle>, Vec<Diagnostic>> {
        let projected = validation::peel_arithmetic_policy(self.program, reference);
        if !matches!(
            self.program.type_reference_table.type_reference(projected),
            TypeReferenceNode::Named { .. } | TypeReferenceNode::Unit
        ) {
            return Ok(None);
        }
        let Some(carrier) = self.scalar_carrier_of_type_reference(projected) else {
            return Ok(None);
        };
        // Named nominal carriers keep their own declaration identity. Fixed
        // source carriers need aliases because calculus Int intentionally
        // merges widths and source arithmetic policies.
        if matches!(carrier, ScalarCarrier::Carrier(_)) {
            return Ok(None);
        }
        if let Some(alias) = self
            .source_aliases
            .iter()
            .find(|alias| self.source_references_correspond(reference, alias.reference))
        {
            return Ok(Some(self.constant(alias.declaration)));
        }
        let body = self.carrier_term(carrier);
        let ty = self
            .arena
            .insert(Term::Sort(Sort::Type(Level::Constant(0))));
        let declaration = self.declarations.len() as u32;
        self.declarations.push(Declaration {
            level_arity: 0,
            ty,
            body: Some(body),
        });
        self.source_aliases.push(SourceAlias {
            reference,
            declaration,
            carrier,
        });
        Ok(Some(self.constant(declaration)))
    }

    /// The signature was checked once. Read its exact application domains,
    /// substituting each whole actual before inspecting the next domain. A
    /// returned function, generic domain or type-family application follows
    /// this same spine; authored parameter counts select no alternate route.
    pub(super) fn check_source_domains(
        &mut self,
        signature: &Signature,
    ) -> Result<(), Vec<Diagnostic>> {
        let invocations = std::mem::take(&mut self.source_invocations);
        for invocation in invocations {
            self.span = invocation.span;
            let context = invocation.domains.iter().fold(
                Context::with_level_arity(invocation.level_arity).with_signature(signature.clone()),
                |context, domain| context.extend(*domain),
            );
            let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
            let (callee, inputs) = match invocation.formation {
                SourceFormation::Application { callee, inputs } => (callee, inputs),
                SourceFormation::Result { expected, actual } => {
                    let domain = match actual.domain {
                        Some(domain) => domain,
                        None => infer_type(&mut self.arena, &context, actual.term, &mut budget)
                            .map_err(|error| {
                                self.source_domain_error(
                                    invocation.expression,
                                    format!("{error:?}"),
                                )
                            })?,
                    };
                    if !self.source_domains_correspond(
                        signature,
                        &context,
                        expected,
                        domain,
                        Some(actual.expression),
                        &mut budget,
                    )? {
                        return Err(self.source_domain_error(
                            invocation.expression,
                            "mathematical result does not preserve its declared carrier and policy"
                                .to_owned(),
                        ));
                    }
                    continue;
                }
            };
            let mut function_type = infer_type(&mut self.arena, &context, callee, &mut budget)
                .map_err(|error| {
                    self.source_domain_error(invocation.expression, format!("{error:?}"))
                })?;
            for input in inputs {
                let function_head =
                    self.source_domain_head(signature, function_type, &mut budget)?;
                let Term::Pi { domain, codomain } = self.arena.get(function_head) else {
                    return Err(self.source_domain_error(
                        invocation.expression,
                        "excess arguments do not apply a remaining mathematical function"
                            .to_owned(),
                    ));
                };
                let actual = match input.domain {
                    Some(domain) => domain,
                    None => infer_type(&mut self.arena, &context, input.term, &mut budget)
                        .map_err(|error| {
                            self.source_domain_error(invocation.expression, format!("{error:?}"))
                        })?,
                };
                if !self.source_domains_correspond(
                    signature,
                    &context,
                    domain,
                    actual,
                    input.expression.is_valid().then_some(input.expression),
                    &mut budget,
                )? {
                    return Err(self.source_domain_error(
                        invocation.expression,
                        "mathematical argument does not preserve its declared carrier and policy"
                            .to_owned(),
                    ));
                }
                function_type = substitute(&mut self.arena, codomain, input.term);
            }
        }
        Ok(())
    }

    fn source_domain_head(
        &mut self,
        signature: &Signature,
        term: TermHandle,
        budget: &mut Budget,
    ) -> Result<TermHandle, Vec<Diagnostic>> {
        let aliases = &self.source_aliases;
        let families = &self.indexed_families;
        weak_head_normalize_retaining(&mut self.arena, signature, term, budget, &|declaration| {
            aliases.iter().any(|alias| alias.declaration == declaration)
                || families
                    .values()
                    .any(|family| family.declaration == declaration)
        })
        .map_err(|error| {
            self.refuse(format!(
                "mathematical source-domain reduction failed: {error:?}"
            ))
        })
    }

    pub(super) fn source_domains_correspond(
        &mut self,
        signature: &Signature,
        context: &Context,
        expected: TermHandle,
        actual: TermHandle,
        expression: Option<ExpressionHandle>,
        budget: &mut Budget,
    ) -> Result<bool, Vec<Diagnostic>> {
        let expected = self.source_domain_head(signature, expected, budget)?;
        if let Some(reference) = self.source_alias_reference(expected)
            && let Some(expression) = expression
            && validation::has_anonymous_numeric_results(self.program, expression)
        {
            // Literal landing is a separate source formation judgment. A
            // typed Int or a function never borrows the demanded fixed carrier.
            return Ok(validation::checked_argument_matches_type_reference(
                self.program,
                expression,
                reference,
            ));
        }
        let shared_type =
            infer_type(&mut self.arena, context, expected, budget).map_err(|error| {
                self.refuse(format!("mathematical source domain has no type: {error:?}"))
            })?;
        let aliases = &self.source_aliases;
        let families = &self.indexed_families;
        convertible_retaining(
            &mut self.arena,
            context,
            expected,
            actual,
            shared_type,
            budget,
            &|declaration| {
                aliases.iter().any(|alias| alias.declaration == declaration)
                    || families
                        .values()
                        .any(|family| family.declaration == declaration)
            },
        )
        .map_err(|error| {
            self.refuse(format!(
                "mathematical source-domain conversion failed: {error:?}"
            ))
        })
    }

    fn source_references_correspond(
        &self,
        left: TypeReferenceHandle,
        right: TypeReferenceHandle,
    ) -> bool {
        match (
            self.program.primitive_type_reference(left),
            self.program.primitive_type_reference(right),
        ) {
            (Some(left_carrier), Some(right_carrier)) => {
                left_carrier == right_carrier
                    && self.program.type_reference_table.arithmetic_domain(left)
                        == self.program.type_reference_table.arithmetic_domain(right)
            }
            _ => {
                self.program
                    .type_identity(TypeIdentityRequest::ordinary(left))
                    == self
                        .program
                        .type_identity(TypeIdentityRequest::ordinary(right))
            }
        }
    }

    fn source_alias_reference(&self, term: TermHandle) -> Option<TypeReferenceHandle> {
        let Term::Constant { declaration, .. } = self.arena.get(term) else {
            return None;
        };
        self.source_aliases
            .iter()
            .find_map(|alias| (alias.declaration == declaration).then_some(alias.reference))
    }

    fn source_domain_error(&self, expression: ExpressionHandle, reason: String) -> Vec<Diagnostic> {
        if !expression.is_valid() {
            return self.refuse(format!(
                "invalid dependent mathematical application: {reason}"
            ));
        }
        let mut diagnostics = self.refuse(format!(
            "invalid mathematical application `{}`: {reason}",
            self.program.render_proof_expression(
                expression,
                typed_trees::proposition::ProofSubstitutions::None
            )
        ));
        for diagnostic in &mut diagnostics {
            diagnostic.source_span = Some(self.program.expression_table.source_span(expression));
        }
        diagnostics
    }
}
