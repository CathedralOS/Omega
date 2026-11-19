# Application identities in published Psi

Source typing writes the canonical type, constant, conformance and callback identities that Psi carries across compilation. Native compilation, the interpreter and package review read those same records. Builtin symbol conversion belongs to [typed trees](../../source-to-psi/pipeline/representations/typed-trees/src/type_identity.rs).

- [Callback declarations](src/callback_declarations.rs) bind a private destination to its selected application.
- [Conformance identities](src/conformance_identity.rs) encode selected applications and compute their commitments.
- [Opaque representations](src/opaque_representations.rs) bind a selected representation to its exact conformance.
- [Constant values](src/const_value.rs) and [type identities](src/type_identity.rs) encode and read the canonical identity forms.

The encoding checks establish framing and commitments over the supplied bytes. Source selection, evaluation and application approval belong to their compiler stages.
