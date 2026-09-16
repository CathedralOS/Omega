//! Closed integer positions establish every range and domain before snapshots
//! erase types.
//!
//! A range refines values of the same integer carrier; an arithmetic policy
//! is not interchangeable with that refinement and stays rejected. A declared,
//! argument-free integer domain is also a refinement of the same carrier: its
//! membership for the concrete value is proved through the shared domain-fact
//! evaluator before invocation or folding, so the callee's ordinary body
//! checking keeps its qualification while the endpoint route never strips it
//! to the bare carrier. Parameterized domains and compiler-owned domain
//! subjects have no closed-value proof here and remain outside this route.
//! Only such shells over an exact builtin leaf enter. Bounds retain their
//! original source selections, while their values read the completed working
//! substitutions.

use numerics::bignum::BigInt;
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
/// this shape: `range_endpoints` prepares its result as an `IntegerPosition`
/// so a Boolean-returning call cannot become a range endpoint.
pub(super) enum ScalarPosition {
    Integer(IntegerPosition),
    Boolean,
}

impl ScalarPosition {
    pub(super) fn prepare(
        program: &TypedTrees,
        original: &TypedTrees,
        reference: TypeReferenceHandle,
        authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
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
        IntegerPosition::prepare(program, original, reference, authority).map(Self::Integer)
    }
}

pub(super) struct IntegerPosition {
    pub(super) primitive: PrimitiveType,
    ranges: Vec<(BigInt, BigInt)>,
    /// Declared domain symbols with their diagnostic spellings.
    domains: Vec<(symbols::SymbolHandle, String)>,
}

impl IntegerPosition {
    pub(super) fn prepare(
        program: &TypedTrees,
        original: &TypedTrees,
        mut reference: TypeReferenceHandle,
        authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
    ) -> Result<Self, String> {
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
                        for expression in [*minimum, *maximum] {
                            crate::machine_execution::admission::require_closed_integer_argument(
                                original, program, expression, authority,
                            )?;
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
