Returning a generic run descriptor exposed loss of constructor type arguments
that do not occur in its fields. This isolates that prerequisite for
`EXTENT-LIBRARY-TRANSFORMS`: a generic owner receives a freshly constructed linear
child, then two distinct applications pass through nested calls and native
storage. Inline constructors also receive the caller's selected arguments,
not the callee's unbound formal names. The runtime checks four payloads and
their neighboring fields.

The matching `fail/generics/phantom_owned_constructor_unproved` changes only the
constructed count. A receiving type supplies generic arguments, not evidence
that the resulting value satisfies its default domain. This is constructor
evidence, not completion of multi-cell run custody or generic Vec.
