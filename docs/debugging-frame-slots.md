# Debugging frame slots (runtime storage layout)

The Omega backend is a stackless state-machine model: every runtime value lives in
a statically-allocated region. Locals, parameters, and call results are *frame
slots* in a single `omega_runtime_frame_storage` data region, each assigned an
absolute byte offset by `stack_runtime_storage_by_call_context` in the current
Rust on-ramp
(`bootstrap/onramps/omega-rust/omega/orchestration/omega-backend-pipeline/src/builder.rs`).

When a runtime value is wrong but you can't tell *which slot* holds it, you need a
map from a logical slot (machine / state / param / local name) to its runtime byte
offset. There is no DWARF/PDB in the emitted image, so two tools produce that map.

## 1. `OMEGA_DUMP_SLOTS` — stderr dump at compile time

Set the env var when compiling. It is inert by default (unset = zero output, zero
behavior change):

```sh
OMEGA_DUMP_SLOTS=1 cargo run -p omega-compiler -- build samples/<your_sample>
```

```powershell
$env:OMEGA_DUMP_SLOTS = "1"; cargo run -p omega-compiler -- build samples\<your_sample>
```

It prints, to stderr, one line per frame slot:

```
# Omega frame-slot layout (region: omega_runtime_frame_storage)
# absolute runtime address of a slot = (relocated region base) + byte_offset.
# ...
# context  dispatch  stmt  machine#  state#  seg  kind   name          type    offset  end   size
0         3         0     12        47      0    param  level         Level   0       8     8
0         3         1     12        47      0    local  room_count    i32     8       12    4
...
```

Columns: `context` is the `CallContext` (specialized-clone id; ROOT=0),
`dispatch` is the dispatch index (the state's arena index in the runtime flow),
`stmt` is the statement index, `machine#`/`state#` are the `StateKey` symbol arena
indices, `seg` is the state segment index, `kind` is `param` / `local` /
`call-result(Role#ordinal)`, `name`/`type` are the slot's identifier and type, and
`offset..end (size)` is the byte range inside the frame region. Rows are sorted by
`(context, dispatch_index, byte_offset)`. The header also prints
`frame_scratch_base` / `frame_scratch_size`.

## 2. `slots.txt` — build-dir side table

On the normal compile-to-disk path (`write_output`), the same table is written to
`slots.txt` in the build dir, alongside `00_pipeline.html`, `12_emission.txt`,
`omega-program.exe`, etc. A debugger/script can read it to translate
`level.room_count` -> its absolute frame offset without disassembly. Same content
and format as the stderr dump.

## Recovering the runtime address

The `byte_offset` is *region-relative*. The absolute runtime address is:

```
addr = (relocated region base) + byte_offset
```

The region base is a relocation resolved in the image. Recover it either way:

- Disassemble and read the `movabsq $imm64,%r15` the dispatch loop executes to
  load the frame-storage base (that `imm64` is the base). (`%r15` = frame storage;
  `%r14` is used for other regions in some paths — confirm from the disassembly.)
- Or read the address of the `omega_runtime_frame_storage` symbol from the image's
  symbol table.

## cdb recipe (Windows, no ASLR, image base `0x140000000`)

```
& "C:\Program Files (x86)\Windows Kits\10\Debuggers\x64\cdb.exe" <abs path to omega-program.exe>
```

Inside cdb:

```
bp <code addr>          ; break where the slot is live (e.g. a guard/write site)
g                       ; run to it
r r15                   ; read the frame-storage base out of r15 (or read the symbol)
dd (<base>+<byte_offset>) L1   ; dump the slot (L<N> words for larger slots)
```

So for a slot at `byte_offset` 8 with the frame base in `r15`, `dd @r15+8 L1`
prints its current value. Cross-reference `byte_offset` from `slots.txt` (or the
`OMEGA_DUMP_SLOTS` dump) for the slot you care about.

> Note: do not probe runtime values via `exit_process(v)` for a non-constant `v` —
> that path silently exits 0 and masks the real value. Use guard-vs-constant +
> literal-only exits, or read the slot directly with cdb as above.
