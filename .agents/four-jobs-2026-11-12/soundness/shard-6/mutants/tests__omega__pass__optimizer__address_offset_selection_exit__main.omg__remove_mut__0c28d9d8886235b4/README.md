This control selects `SelectedAddressOffsetFoldV1` from an authored Build.
`main` passes the bounded byte field `context.out` as a shared `[u8]` view to
`measure`; staging that call descriptor projects the field's live-length
header with an `AddressOffset` over the record base, then reads the length
through `Load64` and forms the data pointer with a second `AddressOffset` —
two displaced consumers of one producer, the surface this rule folds. The
expected native and interpreted exit is 0; a fold that read the wrong
displacement would observe the wrong length or byte and trap instead.

Run from the repository root with a current Omega binary:

```sh
omega update --project tests/omega/pass/optimizer/address_offset_selection_exit --target linux_x86_64
omega run --both --keep tests/omega/pass/optimizer/address_offset_selection_exit/main.omg
```

The selection must commit: compare a normal build with the same Build under
`--disable-optimization SelectedAddressOffsetFoldV1`. In the initial x86-64
witness, the selected text reads the field's live length as
`mov 0x8(%rax),%rcx` and forms the data pointer as `lea 0x10(%rax)` directly
off the record base; the disabled text first computes
`lea 0x8(%rbx),%rcx` and then reads `mov 0x0(%rcx)` and `lea 0x8(%rcx)` —
the unfolded producer/consumer chain. AArch64 shows the same fold as
`ldr x1, [x0, #0x8]` plus `add x0, x0, #0x10` against `ldr x0, [x1]` plus
`add x1, x1, #0x8` — the combined displacements stay inside each target's
encoding bounds (scaled uimm12 on AArch64, disp32 on x86-64). Neither run
changes the source or its exit oracle.

The fold keeps the producer instruction and every register; the observable
change is the rebased operand and its encoded displacement, not a removed
instruction. Ordinary `bytes[i]` reads still select `Load8Indexed`, whose
indexed forms carry no encodable displacement on either target — that path
remains deliberately outside this rule's admitted consumer table.
