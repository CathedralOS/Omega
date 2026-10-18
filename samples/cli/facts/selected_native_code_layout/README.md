# Selected native code layout

The exact selected executable application supplies a nonzero native entry and finite attributed code regions at runtime.

The original root is preserved byte for byte. Each of the nine projects consumes
`code_layout<selected<u64, 7>>()` through its named place: local, field, parameter,
call result, match join, const name, another state, another machine, and another
target body. Both match arms execute. The const-name place uses the constant only
to select the same application; metadata remains runtime data. Each native
oracle is exit 0 after checking the nonzero entry, nonempty region sequence,
entry attribution and positive instruction length.

Each adjacent `invalid/` twin changes only the receiving evaluation stage: the
same witness is consumed by `const OBSERVED: bool = witness()`. Rejection must
name unavailable late native metadata in same-product semantic evaluation.
An unrelated parse, contract, or lowering error does not count.

The otherwise uncalled selected specialization must survive into native text.
Metadata comes from final selected instruction spans plus final frame spans,
including branch reflow. The image producer relocates entry and region bases;
no symbol-size subtraction, second compilation, address authority, or lifetime
extension is involved. Runtime copies use the ordinary record/slice ABI after
checking its actual field offsets and element stride.

Canonical authority, captured at `7875d3411f26556b6c6711494cd659032e274abe`:
`wiki/spec/language/counts_and_addresses.md`, blob
`348dfb1fa43ef749374a9ff46078b211443fcf6c`:

> `code_layout<SelectedMachine<Arguments>>()` describes the exact selected native
> realization. Like `address_of`, it is a compiler-known core query, not a keyword
> or an invocation of that machine. Its retained use creates the same executable
> demand.

> Regions cover the emitted instruction spans attributed to that realization,
> including its retained entry and any split instruction regions. They do not
> include unrelated intervening bytes, padding, data pools or transitively called
> machines merely because those lie nearby or are needed for execution.

> Lengths are finalized by native lowering, layout and any later size-changing
> realization; bases may remain link/load relocations. This is late artifact
> metadata, not hermetic evaluation of the program being produced. Neither its
> sizes nor its addresses may feed that same program's semantic constants, type
> arguments, layout policies or optimization decisions through a circular query.

The focused regressions also cover static type/const/machine/lifetime forwarding,
ordinary declaration shadowing, unavailable/proof-only/incomplete selections,
frame instructions, uncalled boundary-body realization and portable family
progress contracts. Pure Terminal interpretation declines this native query;
it must not fabricate a native address or byte region from an interpreter ID.

```sh
omega run samples/cli/facts/selected_native_code_layout/local/main.omg
python3 tools/corpus_gate.py --filter facts/selected_native_code_layout_through_
python3 tools/corpus_gate.py --native --filter facts/selected_native_code_layout_through_
```

Direct metadata queries of a bodyless target-family head remain unsupported and
reject explicitly. Queries of the same exact ordinary application from each
family member are covered here. This follows the canonical refusal rule:
“Unsupported or unavailable realization metadata rejects rather than inventing
a region.”
