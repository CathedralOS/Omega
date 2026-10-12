//! Independently rejoin a use's application to its shared generated carrier.
//!
//! Structural construction retains the original application before its complete
//! typed context exists. Range equality finishes after symbols and operators are
//! installed, on both ordinary and seeded paths. Never evaluate a landed bound
//! using the partially built program: it can lack its carrier or an authored
//! operator which vetoes builtin arithmetic. Final replay uses original operands,
//! not the syntax producer's interval observations or its synthetic names.
//! Closed Const slots may compare completed result correspondence to retain
//! shared storage after original operands replace display atoms. This is not
//! application admission: the typing entrance retains each full original tuple
//! and the shared checker checks it before publishing a checked program. Other
//! slots and noninteger result encodings keep their exact original comparison.
use super::{Diagnostic, resolved, typed};
use arena::{Handle, HandleSpan};
use resolved::types::{FixedArrayLength, TypeConstraint, TypeReference};
use symbols::{SymbolHandle, SymbolKind};

/// Complete the range equality deferred by structural type construction.
/// Reuse the retained source roster rather than adding a second obligation IR.
pub(crate) fn validate_range_arguments(
    source: &resolved::SymbolResolvedTrees,
    typed: &typed::TypedTrees,
) -> Result<(), Diagnostic> {
    if source.tables.types.generic_application_origins.is_empty()
        || !source
            .tables
            .types
            .constraints
            .iter()
            .any(|(_, constraint)| matches!(constraint, TypeConstraint::Range { .. }))
    {
        return Ok(());
    }
    // Lowering retained source operands needs mutable construction storage.
    // One private replay copy retains complete operator meaning without adding
    // duplicate operands or warning occurrences to the published typed program.
    let mut replay = typed.clone();
    let mut definitions = source.data_definitions.iter().collect::<Vec<_>>();
    definitions.sort_by_key(|definition| symbol_key(definition.symbol));
    let mut origins = Vec::new();
    let mut malformed_origin = false;
    for (_, origin) in source.tables.types.generic_application_origins.iter() {
        let references = &source.tables.declarations.child_type_references;
        if !references.is_valid(origin.instance) || !references.is_valid(origin.application) {
            malformed_origin = true;
            break;
        }
        let instance = source
            .tables
            .declarations
            .child_type_references
            .get(origin.instance);
        let (name, symbol) = match instance {
            TypeReference::Named { name, symbol, .. } => (name, *symbol),
            TypeReference::Generic(value) => (&value.base_name, value.base_symbol),
            _ => {
                malformed_origin = true;
                break;
            }
        };
        origins.push((name, symbol, origin));
    }
    if origins.is_empty() && malformed_origin {
        if let Some((_, origin)) = source
            .tables
            .types
            .generic_application_origins
            .iter()
            .next()
        {
            if source
                .tables
                .declarations
                .child_type_references
                .is_valid(origin.instance)
            {
                match source.child_type_reference(origin.instance) {
                    TypeReference::Named { name, .. } => return Err(mismatch(name)),
                    TypeReference::Generic(value) => return Err(mismatch(&value.base_name)),
                    _ => {}
                }
            }
        }
        return Err(Diagnostic::error(
            "generated instance lost its retained type",
        ));
    }
    // Each row owns one use, even when copies share diagnostic spans and
    // generated storage. Replay only against that storage's declaration;
    // another occurrence's tuple supplies no premise for this one.
    for (name, symbol, origin) in &origins {
        let definition_start = definitions
            .partition_point(|definition| symbol_key(definition.symbol) < symbol_key(*symbol));
        let definition = definitions
            .get(definition_start)
            .filter(|definition| definition.symbol == *symbol)
            .ok_or_else(|| mismatch(name))?;
        if definitions
            .get(definition_start + 1)
            .is_some_and(|next| next.symbol == *symbol)
        {
            return Err(mismatch(name));
        }
        validate_origin(source, Some(&mut replay), name, origin, definition)?;
    }
    if malformed_origin {
        return Err(mismatch(origins.last().expect("nonempty valid prefix").0));
    }
    Ok(())
}

fn symbol_key(symbol: SymbolHandle) -> (u32, u32) {
    (symbol.arena_index(), symbol.generation())
}

/// Resolve the occurrence-owned row. Diagnostic names locate errors only;
/// neither generated symbols nor source spans select a later use's tuple.
pub(super) fn application<'source>(
    source: &'source resolved::SymbolResolvedTrees,
    name: &resolved::name::DiagnosticName,
    symbol: SymbolHandle,
    origin: Handle<resolved::types::GenericApplicationOrigin>,
) -> Result<Option<&'source TypeReference>, Diagnostic> {
    let mut definitions = source
        .data_definitions
        .iter()
        .filter(|definition| definition.symbol == symbol);
    let definition = definitions.next();
    if definitions.next().is_some() {
        return Err(mismatch(name));
    }
    if !origin.is_valid() {
        return if definition.is_some_and(|definition| definition.generic_instance.is_some()) {
            Err(mismatch(name))
        } else {
            Ok(None)
        };
    }
    if !source
        .tables
        .types
        .generic_application_origins
        .is_valid(origin)
    {
        return Err(mismatch(name));
    }
    let origin = source.tables.types.generic_application_origins.get(origin);
    let references = &source.tables.declarations.child_type_references;
    if !references.is_valid(origin.instance) || !references.is_valid(origin.application) {
        return Err(mismatch(name));
    }
    let actual_symbol = match references.get(origin.instance) {
        TypeReference::Named { symbol, .. } => *symbol,
        TypeReference::Generic(application) => application.base_symbol,
        _ => return Err(mismatch(name)),
    };
    if actual_symbol != symbol {
        return Err(mismatch(name));
    }
    let definition = definition.ok_or_else(|| mismatch(name))?;
    validate_origin(source, None, name, origin, definition).map(Some)
}

fn validate_origin<'source>(
    source: &'source resolved::SymbolResolvedTrees,
    typed: Option<&mut typed::TypedTrees>,
    name: &resolved::name::DiagnosticName,
    origin: &resolved::types::GenericApplicationOrigin,
    definition: &'source resolved::data::DataDefinition,
) -> Result<&'source TypeReference, Diagnostic> {
    let references = &source.tables.declarations.child_type_references;
    let lifetimes = match references.get(origin.instance) {
        TypeReference::Named { .. } => &[][..],
        TypeReference::Generic(value) => value.lifetime_arguments.as_slice(),
        _ => return Err(mismatch(name)),
    };
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
    let Some(TypeReference::Generic(shared)) = &definition.generic_instance else {
        return Err(mismatch(name));
    };
    let mut equality = Equality {
        source,
        typed,
        active: Vec::new(),
        canonical_lifetimes: &definition.lifetime_parameters,
        actual_lifetimes: &arguments.lifetime_arguments,
    };
    if source.symbols.get(arguments.base_symbol).kind != SymbolKind::Data
        || arguments.base_symbol != shared.base_symbol
        || !equality.application_arguments(
            arguments.base_symbol,
            arguments.arguments,
            shared.arguments,
        )
    {
        return Err(mismatch(name));
    }
    Ok(application)
}

fn mismatch(name: &resolved::name::DiagnosticName) -> Diagnostic {
    Diagnostic::error(
        "generated generic application does not match its exact retained instance origin",
    )
    .with_source_span(name.source_span())
}

struct Equality<'source, 'typed> {
    source: &'source resolved::SymbolResolvedTrees,
    typed: Option<&'typed mut typed::TypedTrees>,
    active: Vec<(Handle<TypeReference>, Handle<TypeReference>)>,
    canonical_lifetimes: &'source [resolved::name::DiagnosticName],
    actual_lifetimes: &'source [resolved::name::DiagnosticName],
}

impl Equality<'_, '_> {
    fn application_arguments(
        &mut self,
        declaration: SymbolHandle,
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
        let source = self.source;
        let mut definitions = source
            .data_definitions
            .iter()
            .filter(|definition| definition.symbol == declaration);
        let Some(definition) = definitions.next() else {
            return self.arguments(left, right);
        };
        if definitions.next().is_some() {
            return false;
        }
        let parameters = source.data_type_parameters(definition.type_parameters);
        if parameters.len() != left.len() {
            // Non-telescope applications retain the original exact graph
            // comparison. No Const slot exists to supply result correspondence.
            return self.arguments(left, right);
        }
        parameters
            .iter()
            .zip(left.handles())
            .zip(right.handles())
            .all(|((parameter, left), right)| {
                self.child(left, right)
                    || matches!(
                        parameter.kind,
                        resolved::data::TypeParameterKind::Const { .. }
                    ) && self.normalized_const_results_match(left, right)
            })
    }

    /// Storage sharing never supplies admission for the later use. Its complete
    /// original application is retained on the typed occurrence and checked at
    /// the same declaration telescope before carrier identity is consumed.
    fn normalized_const_results_match(
        &self,
        left: Handle<TypeReference>,
        right: Handle<TypeReference>,
    ) -> bool {
        let result = |argument| {
            let mut rows = self
                .source
                .tables
                .types
                .normalized_const_arguments
                .iter()
                .filter_map(|(_, row)| (row.argument == argument).then_some(row));
            let row = rows.next()?;
            if rows.next().is_some() {
                return None;
            }
            Some(row.canonical_result_encoding.as_str())
        };
        let (Some(left), Some(right)) = (result(left), result(right)) else {
            return false;
        };
        use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
        let left_value = CanonicalConstValue::new("", left, "");
        let right_value = CanonicalConstValue::new("", right, "");
        match (left_value.decode_encoding(), right_value.decode_encoding()) {
            (
                Some(DecodedCanonicalConstValue::Integer { value: left, .. }),
                Some(DecodedCanonicalConstValue::Integer { value: right, .. }),
            ) => left == right,
            (Some(_), Some(_)) => left == right,
            _ => false,
        }
    }

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
        (0..left.count()).all(|offset| self.child(left.handle_at(offset), right.handle_at(offset)))
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
                    ..
                },
                TypeReference::Named {
                    symbol: right,
                    name: right_name,
                    ..
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
                    && self.application_arguments(left.base_symbol, left.arguments, right.arguments)
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
                    arguments: left_arguments,
                    ..
                },
                TypeReference::DynamicTrait {
                    symbol: right,
                    arguments: right_arguments,
                    ..
                },
            ) => {
                self.live(*left)
                    && left == right
                    && self.arguments(*left_arguments, *right_arguments)
            }
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
                    end_inclusive: left_inclusive,
                },
                TypeConstraint::Range {
                    minimum: third,
                    maximum: fourth,
                    end_inclusive: right_inclusive,
                },
                // Numeric equality must wait for completed symbols and operators;
                // validate_range_arguments performs that rejoin before publication.
            ) => {
                self.typed.is_none()
                    || self
                        .closed_range(*first, *second, *left_inclusive)
                        .is_some_and(|left| {
                            Some(left) == self.closed_range(*third, *fourth, *right_inclusive)
                        })
            }
            _ => false,
        }
    }

    // Recompute from each retained application, not from syntax synthesis's
    // observation or expression-handle equality. Equivalent exclusive/inclusive
    // spellings must agree, while an invalid landed endpoint cannot be erased
    // by sharing a previously generated carrier.
    fn closed_range(
        &mut self,
        minimum: resolved::expression::ExpressionHandle,
        maximum: resolved::expression::ExpressionHandle,
        end_inclusive: bool,
    ) -> Option<(numerics::bignum::BigInt, numerics::bignum::BigInt)> {
        let expressions = &self.source.tables.bodies.expressions;
        let typed = self.typed.as_deref_mut()?;
        let minimum = crate::expressions::expression::lower_expression_handle_from_table(
            self.source,
            expressions,
            typed,
            minimum,
        )
        .ok()?;
        let maximum = crate::expressions::expression::lower_expression_handle_from_table(
            self.source,
            expressions,
            typed,
            maximum,
        )
        .ok()?;
        Some((
            typed.closed_integer_expression_value(minimum)?,
            typed.closed_integer_range_endpoint(maximum, end_inclusive)?,
        ))
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
            // A generic carrier is the family's first type binder. The outer
            // reference equality already compares the actual carrier; this
            // lookup selects only its domain declaration, not a replacement
            // carrier or a discharged membership predicate.
            (matches!(&domain.target_type, TypeReference::Named { symbol, .. } if symbol == carrier)
                || self.source.data_type_parameters(domain.type_parameters).first().is_some_and(|parameter| {
                    matches!(parameter.kind, resolved::data::TypeParameterKind::Type)
                        && matches!(&domain.target_type, TypeReference::Named { symbol, .. } if *symbol == parameter.symbol)
                }))
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
