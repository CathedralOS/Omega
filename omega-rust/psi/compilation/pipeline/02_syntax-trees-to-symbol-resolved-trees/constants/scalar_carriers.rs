/*
    Scalar eligibility over the selected header or complete symbol's atom.

    The integer carriers are exactly the `LandedIntegerType` names. With `bool`
    they form the structural scalars: carriers whose values have a canonical
    const-index identity. The float formats complete the builtin scalars; their
    values land at a declared format but carry no structural identity.
*/

use symbols::BuiltinTypeAtom;

/* `bool` or an integer carrier. */
pub(crate) fn is_structural_scalar_carrier(atom: BuiltinTypeAtom) -> bool {
    matches!(
        atom,
        BuiltinTypeAtom::Bool | BuiltinTypeAtom::Integer(_) | BuiltinTypeAtom::Address
    )
}

/* A structural scalar or a float format. */
pub(crate) fn is_builtin_scalar_carrier(atom: BuiltinTypeAtom) -> bool {
    is_structural_scalar_carrier(atom)
        || matches!(atom, BuiltinTypeAtom::F32 | BuiltinTypeAtom::F64)
}
