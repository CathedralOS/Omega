# build_facet_swap

The root sample replaces `builder.source`, `builder.log` and
`builder.declarations` with newly constructed values. The Build completes
with all three replacements unused.

The `invalid_swapped_output` control assigns `BuildOutput {}` to
`builder.output`, then calls `BuildOutput::resolve` through that field.
The call requires the original compiler-issued output facet, so the
host rejects the replacement.
