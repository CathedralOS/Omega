//! Independently rejoin a use's application to its shared generated carrier.

use super::*;
use arena::{Handle, HandleSpan};
use resolved::types::{FixedArrayLength, TypeConstraint, TypeReference};
use symbols::{SymbolHandle, SymbolKind};

#[cfg(test)]
mod tests;

pub(super) fn application<'source>(
    source: &'source resolved::SymbolResolvedTrees,
    name: &resolved::name::DiagnosticName,
    symbol: SymbolHandle,
) -> Result<Option<&'source TypeReference>, Diagnostic> {
    let mut selected = None;
    for (_, origin) in source.tables.types.generic_application_origins.iter() {
        let references = &source.tables.declarations.child_type_references;
        if !references.is_valid(origin.instance) || !references.is_valid(origin.application) {
            return Err(mismatch(name));
        }
        let instance = references.get(origin.instance);
        let (instance_symbol, instance_name, lifetimes) = match instance {
            TypeReference::Named { symbol, name } => (*symbol, name, &[][..]),
            TypeReference::Generic(value) => (
                value.base_symbol,
                &value.base_name,
                value.lifetime_arguments.as_slice(),
            ),
            _ => return Err(mismatch(name)),
        };
        if instance_symbol != symbol || instance_name.source_span() != name.source_span() {
            continue;
        }
        let application = references.get(origin.application);
        let TypeReference::Generic(arguments) = application else {
            return Err(mismatch(name));
        };
        if lifetimes.len() != arguments.lifetime_arguments.len()
            || !lifetimes
                .iter()
                .zip(&arguments.lifetime_arguments)
                .all(|(left, right)| left.as_str() == right.as_str())
        {
            return Err(mismatch(name));
        }
        let mut definitions = source
            .data_definitions
            .iter()
            .filter(|data| data.symbol == symbol);
        let definition = definitions.next().ok_or_else(|| mismatch(name))?;
        if definitions.next().is_some() {
            return Err(mismatch(name));
        }
        let Some(TypeReference::Generic(shared)) = &definition.generic_instance else {
            return Err(mismatch(name));
        };
        let mut equality = Equality {
            source,
            active: Vec::new(),
            canonical_lifetimes: &definition.lifetime_parameters,
            actual_lifetimes: &arguments.lifetime_arguments,
        };
        if source.symbols.get(arguments.base_symbol).kind != SymbolKind::Data
            || arguments.base_symbol != shared.base_symbol
            || !equality.arguments(arguments.arguments, shared.arguments)
            || selected.is_some_and(|previous| {
                let mut exact = Equality {
                    source,
                    active: Vec::new(),
                    canonical_lifetimes: &[],
                    actual_lifetimes: &[],
                };
                !exact.reference(previous, application)
            })
        {
            return Err(mismatch(name));
        }
        selected = Some(application);
    }
    if selected.is_none()
        && source
            .data_definitions
            .iter()
            .any(|definition| definition.symbol == symbol && definition.generic_instance.is_some())
    {
        return Err(mismatch(name));
    }
    Ok(selected)
}

fn mismatch(name: &resolved::name::DiagnosticName) -> Diagnostic {
    Diagnostic::error(
        "generated generic application does not match its exact retained instance origin",
    )
    .with_source_span(name.source_span())
}

struct Equality<'source> {
    source: &'source resolved::SymbolResolvedTrees,
    active: Vec<(Handle<TypeReference>, Handle<TypeReference>)>,
    canonical_lifetimes: &'source [resolved::name::DiagnosticName],
    actual_lifetimes: &'source [resolved::name::DiagnosticName],
}

impl Equality<'_> {
    fn arguments(
        &mut self,
        left: HandleSpan<TypeReference>,
        right: HandleSpan<TypeReference>,
    ) -> bool {
        let arena = &self.source.tables.declarations.child_type_references;
        if left.count() != right.count()
            || arena.span(left).is_none()
            || arena.span(right).is_none()
        {
            return false;
        }
        (0..left.count()).all(|offset| {
            let left = Handle::from_parts(
                left.start().arena_index() + offset,
                left.start().generation(),
            );
            let right = Handle::from_parts(
                right.start().arena_index() + offset,
                right.start().generation(),
            );
            self.child(left, right)
        })
    }

    fn child(&mut self, left: Handle<TypeReference>, right: Handle<TypeReference>) -> bool {
        let arena = &self.source.tables.declarations.child_type_references;
        if !arena.is_valid(left) || !arena.is_valid(right) || self.active.contains(&(left, right)) {
            return false;
        }
        self.active.push((left, right));
        let equal = self.reference(arena.get(left), arena.get(right));
        self.active.pop();
        equal
    }

    fn reference(&mut self, left: &TypeReference, right: &TypeReference) -> bool {
        match (left, right) {
            (
                TypeReference::Named {
                    symbol: left,
                    name: left_name,
                },
                TypeReference::Named {
                    symbol: right,
                    name: right_name,
                },
            ) => {
                if left.is_valid() || right.is_valid() {
                    return self.live(*left) && left == right;
                }
                use language_semantics::const_value::CanonicalConstValue;
                if let (Some(left), Some(right)) = (
                    CanonicalConstValue::from_atom(left_name.as_str()),
                    CanonicalConstValue::from_atom(right_name.as_str()),
                ) {
                    return left == right;
                }
                match (
                    left_name.as_str().parse::<i128>(),
                    right_name.as_str().parse::<i128>(),
                ) {
                    (Ok(left), Ok(right)) => left == right,
                    _ => false,
                }
            }
            (
                TypeReference::SelfType { symbol: left },
                TypeReference::SelfType { symbol: right },
            ) => self.live(*left) && left == right,
            (TypeReference::Reference(left), TypeReference::Reference(right)) => {
                left.access == right.access
                    && match (&left.lifetime, &right.lifetime) {
                        (Some(left), Some(right)) => self.lifetime(left, right),
                        (None, None) => true,
                        _ => false,
                    }
                    && self.child(left.referee, right.referee)
            }
            (TypeReference::Slice(left), TypeReference::Slice(right)) => {
                self.child(left.element_type, right.element_type)
            }
            (TypeReference::FixedArray(left), TypeReference::FixedArray(right)) => {
                let length = match (&left.length, &right.length) {
                    (FixedArrayLength::Literal(left), FixedArrayLength::Literal(right)) => {
                        left == right
                    }
                    (
                        FixedArrayLength::ConstParameter { symbol: left, .. },
                        FixedArrayLength::ConstParameter { symbol: right, .. },
                    ) => self.live(*left) && left == right,
                    _ => false,
                };
                length && self.child(left.element_type, right.element_type)
            }
            (TypeReference::Generic(left), TypeReference::Generic(right)) => {
                self.live(left.base_symbol)
                    && left.base_symbol == right.base_symbol
                    && left.lifetime_arguments.len() == right.lifetime_arguments.len()
                    && left
                        .lifetime_arguments
                        .iter()
                        .zip(&right.lifetime_arguments)
                        .all(|(left, right)| self.lifetime(left, right))
                    && self.arguments(left.arguments, right.arguments)
            }
            (TypeReference::Constrained(left), TypeReference::Constrained(right)) => {
                let constraints = &self.source.tables.types.constraints;
                let (Some(first), Some(second)) = (
                    constraints.span(left.constraints),
                    constraints.span(right.constraints),
                ) else {
                    return false;
                };
                self.child(left.base_type, right.base_type)
                    && first.len() == second.len()
                    && first.iter().zip(second).all(|(first, second)| {
                        self.constraint(first, second, left.base_type, right.base_type)
                    })
            }
            (
                TypeReference::DynamicTrait {
                    symbol: left,
                    conformance: left_conformance,
                    ..
                },
                TypeReference::DynamicTrait {
                    symbol: right,
                    conformance: right_conformance,
                    ..
                },
            ) => self.live(*left) && left == right && left_conformance == right_conformance,
            (TypeReference::ConstExpression(left), TypeReference::ConstExpression(right)) => {
                left.is_valid() && left == right
            }
            (TypeReference::Unit, TypeReference::Unit) => true,
            _ => false,
        }
    }

    fn constraint(
        &mut self,
        left: &TypeConstraint,
        right: &TypeConstraint,
        left_carrier: Handle<TypeReference>,
        right_carrier: Handle<TypeReference>,
    ) -> bool {
        match (left, right) {
            (TypeConstraint::ArithmeticDomain(left), TypeConstraint::ArithmeticDomain(right)) => {
                left == right
            }
            (TypeConstraint::Named(left), TypeConstraint::Named(right)) => self
                .domain(left, left_carrier)
                .is_some_and(|symbol| Some(symbol) == self.domain(right, right_carrier)),
            (TypeConstraint::Domain(left), TypeConstraint::Domain(right)) => {
                self.domain(&left.name, left_carrier)
                    .is_some_and(|symbol| Some(symbol) == self.domain(&right.name, right_carrier))
                    && self.arguments(left.arguments, right.arguments)
            }
            (
                TypeConstraint::Range {
                    minimum: first,
                    maximum: second,
                },
                TypeConstraint::Range {
                    minimum: third,
                    maximum: fourth,
                },
            ) => first == third && second == fourth,
            _ => false,
        }
    }

    // The shared carrier binds regions positionally; the use retains the
    // actual lexical region arguments. This rejoin preserves the mapping
    // without treating erased layout identity as semantic lifetime equality.
    fn lifetime(
        &self,
        actual: &resolved::name::DiagnosticName,
        canonical: &resolved::name::DiagnosticName,
    ) -> bool {
        self.canonical_lifetimes
            .iter()
            .position(|parameter| parameter.as_str() == canonical.as_str())
            .and_then(|ordinal| self.actual_lifetimes.get(ordinal))
            .map_or_else(
                || actual.as_str() == canonical.as_str(),
                |expected| actual.as_str() == expected.as_str(),
            )
    }

    fn live(&self, symbol: SymbolHandle) -> bool {
        symbol.is_valid() && self.source.symbols.get(symbol).kind != SymbolKind::Unknown
    }

    fn domain(
        &self,
        name: &resolved::name::DiagnosticName,
        carrier: Handle<TypeReference>,
    ) -> Option<SymbolHandle> {
        let TypeReference::Named {
            symbol: carrier, ..
        } = self.source.child_type_reference(carrier)
        else {
            return None;
        };
        if !self.live(*carrier) {
            return None;
        }
        let qualified = self
            .source
            .symbols
            .find_top_level_by_name_and_kinds_from_source(
                name.as_str(),
                &[SymbolKind::Domain],
                name.source_span(),
            );
        let mut candidates = self.source.domain_definitions.iter().filter(|domain| {
            matches!(&domain.target_type, TypeReference::Named { symbol, .. } if symbol == carrier)
                && (qualified == Some(domain.symbol)
                    || (!name.as_str().contains("::")
                        && domain.name.as_str().rsplit("::").next() == Some(name.as_str())))
                && self
                    .source
                    .symbols
                    .source_reference_can_see_symbol(name.source_span(), domain.symbol)
        });
        let selected = candidates.next()?.symbol;
        candidates.next().is_none().then_some(selected)
    }
}
