//! Closed integer positions establish every range and domain before snapshots
//! erase types.
//!
//! A range refines values of the same integer carrier. Parameter positions may
//! also retain one Wrapping or Saturating policy: anonymous arguments still
//! land exactly before invocation, and the original callee signature supplies
//! the interpreter's arithmetic meaning. Result positions reject these policies
//! because the surrounding closed scalar evaluator does not carry policy-bearing
//! results; explicit erasure in the callee must precede publication.
//! A declared, argument-free integer domain requires concrete membership through
//! the shared domain-fact evaluator before invocation or folding. Parameterized
//! domains and compiler-owned domain subjects remain outside this route. Bounds
//! retain original source selections while values read completed substitutions.

use numerics::{arithmetic::ArithmeticDomain, bignum::BigInt};
use typed_trees::{
    TypedTrees,
    types::{
        DomainConstraintSubject, PrimitiveType, TypeConstraintNode, TypeReferenceHandle,
        TypeReferenceNode,
    },
};

use crate::BuildTimeAdmissionPlan;

/// A callee parameter, or an argument-position call result, is either an
/// integer position or a bare Boolean. The range bound itself never takes
/// this shape: the shared endpoint evaluator requires its completed result
/// to be an exact integer, so a Boolean call cannot become a range endpoint.
pub(super) enum ScalarPosition {
    Integer(IntegerPosition),
    Boolean,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PositionRole {
    Parameter,
    Result,
}

impl ScalarPosition {
    pub(super) fn prepare(
        program: &TypedTrees,
        original: &TypedTrees,
        reference: TypeReferenceHandle,
        authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
        role: PositionRole,
    ) -> Result<Self, String> {
        // A qualified Boolean (`bool in Domain`) has no closed proof route
        // here; only the bare builtin leaf is a Boolean position.
        if program
            .type_reference_table
            .contains_type_reference(reference)
            && crate::const_evaluation::const_generic_expressions::exact_probe_destination(
                program, reference,
            ) == Some(PrimitiveType::Bool)
        {
            return Ok(Self::Boolean);
        }
        IntegerPosition::prepare_for_role(program, original, reference, authority, role)
            .map(Self::Integer)
    }
}

pub(super) struct IntegerPosition {
    pub(super) primitive: PrimitiveType,
    pub(super) policy: ArithmeticDomain,
    ranges: Vec<(BigInt, BigInt)>,
    /// Declared domain symbols with their diagnostic spellings.
    domains: Vec<(symbols::SymbolHandle, String)>,
}

impl IntegerPosition {
    #[cfg(test)]
    pub(super) fn prepare(
        program: &TypedTrees,
        original: &TypedTrees,
        reference: TypeReferenceHandle,
        authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
    ) -> Result<Self, String> {
        Self::prepare_for_role(
            program,
            original,
            reference,
            authority,
            PositionRole::Result,
        )
    }

    fn prepare_for_role(
        program: &TypedTrees,
        original: &TypedTrees,
        mut reference: TypeReferenceHandle,
        authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
        role: PositionRole,
    ) -> Result<Self, String> {
        let mut policy = None;
        let mut ranges = Vec::new();
        let mut domains = Vec::new();
        let mut visited = Vec::new();
        loop {
            if !program
                .type_reference_table
                .contains_type_reference(reference)
                || visited.contains(&reference)
            {
                return Err("invalid or cyclic range endpoint integer type".to_owned());
            }
            visited.push(reference);
            let TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } = program.type_reference_table.type_reference(reference)
            else {
                break;
            };
            let rows = program.type_reference_table.constraints(*constraints);
            if rows.is_empty() || rows.len() != constraints.count() as usize {
                return Err("range endpoint integer type has invalid constraints".to_owned());
            }
            for constraint in rows {
                match constraint {
                    TypeConstraintNode::Range {
                        minimum,
                        maximum,
                        end_inclusive,
                    } => {
                        // A callee's own signature bound must already be closed in
                        // the tree these positions read. Under an explicit static
                        // application that tree is the prepared program, whose
                        // cloned instance bound never sees the working tree's
                        // folds, so a named endpoint call inside a template's
                        // bound is reported here as an unclosed signature bound.
                        for expression in [*minimum, *maximum] {
                            crate::machine_execution::admission::require_closed_integer_argument(
                                original, program, expression, authority,
                            )
                            .map_err(|reason| {
                                format!("range endpoint signature bound is not closed: {reason}")
                            })?;
                        }
                        let minimum = validation::closed_integer_range_bound(program, *minimum)
                            .ok_or("range endpoint type needs a closed minimum")?;
                        let maximum = validation::closed_integer_range_maximum(
                            program,
                            *maximum,
                            *end_inclusive,
                        )
                        .ok_or("range endpoint type needs a closed maximum")?;
                        ranges.push((minimum, maximum));
                    }
                    // The typed lowering's carrier-aware normalization binds a
                    // declared domain's exact symbol before this pass; an
                    // unbound or compiler-owned subject has no proof route.
                    TypeConstraintNode::Domain(domain)
                        if domain.subject == DomainConstraintSubject::Declared
                            && domain.symbol.is_valid()
                            && domain.arguments.is_empty() =>
                    {
                        domains.push((domain.symbol, domain.name.as_str().to_owned()));
                    }
                    TypeConstraintNode::ArithmeticDomain(domain)
                        if role == PositionRole::Parameter
                            && matches!(
                                domain,
                                ArithmeticDomain::Wrapping | ArithmeticDomain::Saturating
                            ) =>
                    {
                        if policy.replace(*domain).is_some() {
                            return Err(
                                "range endpoint parameter has multiple arithmetic policies".into(),
                            );
                        }
                    }
                    _ => {
                        return Err(
                            "range endpoint integer types admit only range refinements and declared integer domains"
                                .to_owned(),
                        );
                    }
                }
            }
            reference = *base_type;
        }
        let primitive =
            crate::const_evaluation::const_generic_expressions::exact_probe_destination(
                program, reference,
            )
            .filter(|primitive| primitive.accepts_integer_literal())
            .ok_or("range endpoint position requires an exact builtin integer carrier")?;
        Ok(Self {
            primitive,
            policy: policy.unwrap_or(ArithmeticDomain::Exact),
            ranges,
            domains,
        })
    }

    /// Whether this position carries declared domain qualifications that need
    /// the admission plan to prove membership.
    #[cfg(test)]
    pub(super) fn has_domains(&self) -> bool {
        !self.domains.is_empty()
    }

    /// `program` and `admission` are the prepared execution program and the
    /// plan inferred over it: domain facts may invoke admitted machines.
    pub(super) fn require_value(
        &self,
        program: &TypedTrees,
        admission: &BuildTimeAdmissionPlan,
        value: &BigInt,
    ) -> Result<(), String> {
        // Snapshot integers use i64 bits even for u64. Check the decoded exact
        // value, never a signed compatibility interval or merely its kind.
        typed_trees::closed_numeric::land_integer(value, self.primitive)
            .ok_or("range endpoint value does not fit its declared integer carrier")?;
        for (minimum, maximum) in &self.ranges {
            if value < minimum || value > maximum {
                return Err(format!(
                    "range endpoint value `{value}` is outside declared range `{minimum}..={maximum}`"
                ));
            }
        }
        for (symbol, name) in &self.domains {
            match crate::const_evaluation::const_domain_facts::evaluate_closed_membership(
                program, admission, *symbol, value,
            )? {
                Some(true) => {}
                Some(false) => {
                    return Err(format!(
                        "range endpoint value `{value}` is outside domain `{name}`"
                    ));
                }
                None => {
                    return Err(format!(
                        "range endpoint value `{value}` cannot prove membership in domain `{name}` at build time"
                    ));
                }
            }
        }
        Ok(())
    }
}
