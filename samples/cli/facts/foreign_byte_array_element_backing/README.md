# Original byte-array backing behind a foreign element loan

Parent: **BUILD-OWNED-NATIVE-BINDINGS** in [TASKS.md](../../../../TASKS.md),
requirement: transport original loans of Build-owned native rows through both
execution engines.

Fact: a shared element loan into an owned byte array exports the whole original
array through the thin foreign ABI, so the foreign reader sees every sibling
byte and no byte changes.

`Text::length` is C `strlen`, bound by `Build::bind` on each hosted target
(msvcrt, glibc 2.2.5 and 2.17, libSystem). The consumer copies the transported
`[u8; 8]` value `omega\0xy` into its own `let mut text`, lends `&text[0]` and
`&text[2]`, and checks both counts (5 and 3) and the bytes after the terminator.
A reader that saw only the lent cell, or a copy of it, could not find the
terminator three or five bytes later. Native code passes the address of the
frame array. Terminal's original-storage preparation now exposes that same
allocation: for a lent cell of a frame array it supplies every sibling cell of
the activation's packed array as the allocation image, as it already did for
leaves of a structural owner.

The array is carried through [a local](local/main.omg),
[a field](field/main.omg) between canaries, [a parameter](parameter/main.omg),
[a call result](call_result/main.omg), [a match join](match_join/main.omg)
whose two arms supply texts with distinct counts (83 and 32 after packing),
[a constant name](const_name/main.omg), [a nested machine](other_state/main.omg),
[another machine](other_machine/main.omg) and
[a Build-selected named implementation](target_body/main.omg). Each sibling
`invalid/main.omg` adds only `text[0] = 0;` while `selected` is still live and
must reject for that loan.

Before the repair, on the `bb4c4d50387` producer (the content merged as
`c58a7b2c593`), every positive place exited 0 natively while Terminal failed
with `StructuralPrimitiveStorageMissing` in `realize original host storage`.
The same failure stopped the Terminal leg of `samples/cli/audio/sine_tone` at
ALSA `snd_pcm_open(&mut handle, &name[0], ...)`, whose device name is a frame
byte array.

## Requirement evidence

- Establishment: the loan is formed by ordinary checking at the consumer; the
  twins write the lent cell while the loan is live and reject.
- Identity and lifetime: the foreign reader observes the original allocation,
  not a copy; the bytes after the terminator and the field place's neighbors
  are read back unchanged.
- Composition: every place transports the owned array by its construct and
  forms the loan at the consumer; both match arms execute with distinct counts.
- Soundness: the allocation image is read-only for a shared loan and names only
  cells of the lent cell's own activation array.
- Psi 06–08: the boundary call keeps its scalar loan; Terminal prepares the
  original array allocation for the host.
- Omega 00–02: each target's Build row selects its own `strlen`; locator
  lengths are inferred.
- Omega 03–09: the native image passes the frame array's address and exits 0.

Acceptance is `omega run --both <place>/main.omg` with both exits 0 on each
matching host, plus each twin's rejection. The fixtures are pinned under
`tests/omega/{pass,fail}/facts/foreign_byte_array_element_backing_through_*`.
Linux x86_64 is observed; Windows, macOS ARM64 and Linux ARM64 execution remain
unverified.
