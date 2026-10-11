# build_facet_swap

Assigning a fabricated value over a compiler-issued facet field
(`builder.source`, `builder.inputs`, `builder.log`, `builder.product`,
`builder.output`) is legal — this sample swaps all four inertly. The
swap is not silent forgery: the evaluator binds the ACTIVATION facet,
not the field contents.

- `invalid_swapped_output`: after `builder.output = BuildOutput {}`,
  `BuildOutput::resolve` on the replacement rejects "Build path
  operation received a different activation facet". The first
  operation on a swapped facet detects the forgery — assignments are
  inert only because no operation touches them.
