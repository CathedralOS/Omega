# Omega bootstrap resolved-source lowering, outer version 13

[`OMGRSW4`](OMEGA_BOOTSTRAP_RESOLUTION_V4.md) |
[`CKIR12`](OMEGA_BOOTSTRAP_CHECKED_IR_V12.md)

`OMGLOWD` is the bounded producer relation from exact OMGRSW4 source custody to
the CKIR12 program-static shared-byte-view carrier. It inherits the 32-byte
outer frame and component ceilings of the earlier shared lowerer. The exact
header is magic `OMGLOWD\0`, major 13, minor and flags zero, header size 32,
checked total/OMGCOMP/OMGRSW lengths, and resolution selector 4. Its components
are one exact OMGCOMP followed by its exact canonical OMGRSW4. Cross-version,
cross-selector, trailing-byte, malformed, or over-capacity frames do not widen
this relation.

This first producer is intentionally smaller than general CKIR12. The admitted
closure has one selected machine, no selected entry parameters, and obtains a
program-static view by passing one plain OMGRSW4 literal from an authored block
to an exact shared `&[u8]` named-state parameter. Literals are 0 through 32
plain ASCII payload bytes and lower to the inherited canonical constant DAG
plus one opcode-22 `StaticByteView` per authored literal token.

The selected guard and true arguments are exactly:

```omega
transition view.len > 0 {
    true -> target(view[0], view[1..])
    false -> bypass()
}
```

`view` is the same exact shared-byte state parameter in all four occurrences.
The true authored target has exactly `(u8, &[u8])` parameters and the false
target has none. The guard lowers to one opcode-23 `SliceNonEmpty`; it does not
lower to an integer length or opcode 19. The lowerer inserts one final block
flagged `SYNTHETIC_NONEMPTY_EDGE`, passes only the original view into its sole
parameter, and defers opcode 24 `SliceHead` and opcode 25 `SliceTailOne` to that
true-edge block. The block then jumps to the authored true target with head and
tail. The false edge directly targets the authored bypass, so neither partial
operation is evaluated on an empty view.

No extra synthetic pass-through parameters are admitted. `>= 0`, a different
guard subject, reversed or different indices, open ranges other than `[1..]`,
eager head/tail expressions elsewhere, mutable/other-element views, machine
results, general indexing/subslicing, runtime allocation, pointer identity, or
more than one selected nonempty transition remain outside OMGLOWD. OMGRSW1/2/3
paired outer versions and their emitted CKIR bytes remain unchanged.

Malformed source, type, token, operation, or control relations select status
251 without output. The inherited component/table ceilings and the 33rd
literal byte select 252 without output. Publication begins only after complete
preflight, including the exact selected guard, literal root, synthetic block,
and all four CKIR12 operations.

Deterministic one-byte and empty sources remain under
`../gates/fixtures/ckir12-static-byte-view/`, and the independent CKIR12
reference retains exact opcode and synthetic-block shape. The former producer
wrapper joined these to native/self resolver and lowerer byte identity,
old/new cross-pairs, malformed surface controls, and status 251/252
nonpublication. Replay is suspended until canonical Delta publication.
