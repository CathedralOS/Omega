# build_typed_fields

Builder fields are typed places: `builder.freestanding = "yes"`
rejects "stores text into a `bool` place" — field writes are
type-checked like any other assignment, not swallowed as
configuration strings.
