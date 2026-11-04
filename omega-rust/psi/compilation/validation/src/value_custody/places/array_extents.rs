//! An array field uses the const argument supplied to its exact receiver.
//! Keep enclosing applications while following a projection; a field's own
//! declaration binder is not the caller's bound, even when both have one name.

use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
use numerics::bignum::BigInt;
use symbols::SymbolHandle;
use typed_trees::data::TypeParameterKind;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::state::State;
use typed_trees::type_identity::{AppliedTypeIdentityScope, IndexIdentityArgument};
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

/// Static geometry alone grants neither element access nor initialized content.
/// An open extent keeps its declared carrier for mathematical integer embedding.
#[derive(Clone, Debug)]
pub enum ResolvedArrayExtent {
    Closed(BigInt),
    Bound {
        symbol: SymbolHandle,
        type_reference: TypeReferenceHandle,
    },
}

/// Keep a checked value's closed length, or resolve its open field type through
/// the applications supplying its arguments. A specialized array's literal
/// geometry must not be replaced by its original template binder.
pub fn resolved_array_extent_at(
    program: &dyn crate::CheckingInput,
    state: &State,
    statement_index: usize,
    collection: ExpressionHandle,
) -> Result<ResolvedArrayExtent, &'static str> {
    let (machine, _) = crate::find_state(program, state.symbol)
        .ok_or("array collection has no resolved executable owner")?;
    let reference = crate::expression_result_type_reference(program, machine, state, collection)
        .ok_or("array collection has no checked result type")?;
    let source = super::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        collection,
    )
    .and_then(|place| {
        super::canonical_place_source_type_application(
            program,
            state.symbol,
            statement_index,
            &place,
        )
    });
    resolved_projected_array_extent(program, reference, source)
}

/// Closed geometry is intrinsic to the checked value's type, including call
/// results and contract projections with no executable storage path. Only an
/// open extent needs the original declaration and its supplying applications.
pub fn resolved_projected_array_extent(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    source: Option<(TypeReferenceHandle, Vec<TypeReferenceHandle>)>,
) -> Result<ResolvedArrayExtent, &'static str> {
    if matches!(
        super::fixed_array_type_extent(program, reference),
        Some(FixedArrayLength::Literal(_))
    ) {
        return resolved_array_extent(program, reference, &[]);
    }
    let (source, applications) =
        source.ok_or("open array extent has no supplying type applications")?;
    resolved_array_extent(program, source, &applications)
}

/// Resolve the extent of an already authenticated source projection. Each
/// application supplies simultaneous actuals from its outer prefix; following
/// one actual cannot apply that same frame's sibling replacements to it.
pub fn resolved_array_extent(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    applications: &[TypeReferenceHandle],
) -> Result<ResolvedArrayExtent, &'static str> {
    let length = super::fixed_array_type_extent(program, reference)
        .ok_or("collection is not an always-full fixed array")?;
    program
        .with_enclosing_data_identity_scopes(applications, |scopes| {
            let resolver = ExtentResolver { program, scopes };
            match length {
                FixedArrayLength::Literal(value) => u64::try_from(*value)
                    .map(|value| ResolvedArrayExtent::Closed(BigInt::from_u64(value)))
                    .map_err(|_| "array extent is not representable"),
                FixedArrayLength::ConstParameter { symbol, .. } => {
                    resolver.symbol(*symbol, scopes.len())
                }
                FixedArrayLength::Expression(reference) => {
                    resolver.reference(*reference, scopes.len())
                }
                _ => Err("array extent still requires selected constant evaluation"),
            }
        })
        .ok_or("array extent has an unauthenticated enclosing application")?
}

struct ExtentResolver<'program, 'scope, 'argument> {
    program: &'program dyn crate::CheckingInput,
    scopes: &'scope [AppliedTypeIdentityScope<'argument>],
}

impl ExtentResolver<'_, '_, '_> {
    fn symbol(
        &self,
        symbol: SymbolHandle,
        visible: usize,
    ) -> Result<ResolvedArrayExtent, &'static str> {
        if !symbol.is_valid() {
            return Err("array extent has no resolved declaration");
        }
        for (position, scope) in self.scopes[..visible].iter().enumerate().rev() {
            let mut bindings = scope
                .arguments
                .iter()
                .filter(|(formal, _)| *formal == symbol);
            let Some((_, actual)) = bindings.next() else {
                continue;
            };
            if bindings.next().is_some() {
                return Err("array extent has ambiguous supplying arguments");
            }
            let parameter = self
                .program
                .data_type_parameter_by_symbol(symbol)
                .ok_or("array extent argument has no declaration")?;
            if !matches!(
                parameter.kind,
                TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. }
            ) {
                return Err("array extent argument is not a const or value parameter");
            }
            let IndexIdentityArgument::TypeReference(actual) = actual else {
                return Err("array extent argument is not a static value operand");
            };
            return self.reference(*actual, position);
        }
        if let Some(parameter) = self.program.data_type_parameter_by_symbol(symbol) {
            let (TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference }) = parameter.kind
            else {
                return Err("array extent names neither a const nor a value parameter");
            };
            if !self
                .program
                .primitive_type_reference(type_reference)
                .is_some_and(|primitive| primitive.is_fixed_integer())
            {
                return Err("array extent parameter has no fixed integer carrier");
            }
            return Ok(ResolvedArrayExtent::Bound {
                symbol,
                type_reference,
            });
        }
        let declaration = self
            .program
            .const_declaration_by_symbol(symbol)
            .ok_or("array extent names no const declaration or parameter")?;
        let encoding = declaration
            .evaluated_value
            .as_ref()
            .and_then(|value| value.encoding())
            .ok_or("array extent constant has no evaluated value")?;
        self.decoded(CanonicalConstValue::new("", encoding.to_owned(), "").decode_encoding())
    }

    fn reference(
        &self,
        reference: TypeReferenceHandle,
        visible: usize,
    ) -> Result<ResolvedArrayExtent, &'static str> {
        if !self
            .program
            .type_reference_table
            .contains_type_reference(reference)
        {
            return Err("array extent argument has no retained type operand");
        }
        match self.program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } if symbol.is_valid() => {
                self.symbol(*symbol, visible)
            }
            TypeReferenceNode::Named { name, .. } => {
                // Symbol-less decimal atoms are retained evaluated const data,
                // not declaration names. Structured atoms use the same decoder
                // as the canonical type-identity owner.
                if let Ok(value) = name.as_str().parse::<i128>() {
                    return Ok(ResolvedArrayExtent::Closed(BigInt::from_i128(value)));
                }
                self.decoded(
                    CanonicalConstValue::from_atom(name.as_str())
                        .and_then(|value| value.decode_encoding()),
                )
            }
            TypeReferenceNode::ConstExpression(expression) => {
                match self.program.expression_table.expression(*expression) {
                    ExpressionNode::Name(path) => self.symbol(path.symbol, visible),
                    ExpressionNode::Integer(literal) => literal
                        .value_bignum()
                        .map(ResolvedArrayExtent::Closed)
                        .ok_or("array extent literal is not an integer"),
                    _ => Err("array extent expression has not been reduced to a static value"),
                }
            }
            _ => Err("array extent argument is not a retained integer value"),
        }
    }

    fn decoded(
        &self,
        value: Option<DecodedCanonicalConstValue>,
    ) -> Result<ResolvedArrayExtent, &'static str> {
        match value {
            Some(DecodedCanonicalConstValue::Integer { value, .. }) => {
                Ok(ResolvedArrayExtent::Closed(BigInt::from_i128(value)))
            }
            _ => Err("array extent constant is not a canonical integer"),
        }
    }
}
