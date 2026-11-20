# Epsilon compiler

The compiler is written in Delta. It checks Epsilon source and emits a Delta
module. The compiler reports either `compiled` for its pure calling convention
or `compiled-storage` for programs that need storage and effects. A refusal
reports its class and source offset. [Compiler entry](entry.delta)

After checking, the compiler builds the same source-coordinate index that
the interpreter uses for resolved references. Lowering retrieves local, field,
callable and control facts from the bucket for each source start. The lookup
still checks the expression kind and both span endpoints. A field and its
receiver can start at the same byte. [Reference lookup](../interpreter/execution/references/lookup.delta)

## Assemble and run a program

`compiler.parts` lists the complete compiler source in order. Paths are relative
to this directory. Each line contributes the named file's exact bytes. An empty
line contributes one LF. The checker files under `../interpreter/` are checked
against the interpreter's source manifest. [Compiler source list](compiler.parts)

The two program lists specify the declarations, implementation and entry for
each calling convention. A materializer inserts the emitted Delta module at
`@module` and appends the standard entry files following `@entry`. A diagnostic
test may replace that final section to probe a particular machine directly.
[Pure program](runtime/pure.parts) · [Stored program](runtime/storage.parts)

On Linux x86-64, run this from the repository root with a complete Epsilon
program in `program.epsilon`. It builds both native tools from the seed. The
only host code in `build_delta` frames a source byte count and appends the
bound support section. Both compilations run in the source-built tools.
[Cold construction](../../../tests/bootstrap/linux_x86_64/README.md)

```bash
set -euo pipefail
out=$(mktemp -d)
tests/bootstrap/linux_x86_64/bootstrap --delta . "$out/host"
parts=tests/bootstrap/independent/epsilon/composition.py
support=bootstrap/delta/compiler/support
awk '$1 == "member" {print $5}' "$support/support.gamma.sources" |
    while read -r name; do cat "$support/$name"; done > "$out/support.gamma"

build_delta() {
    python3 -c 'import os, struct, sys
sys.stdout.buffer.write(b"DCREQ\x01\0\0" + struct.pack("<II", 1, os.path.getsize(sys.argv[1])))' \
        "$1" > "$out/header"
    cat "$out/header" "$1" "$out/support.gamma" | "$out/host/program" > "$2.gamma"
    "$out/host/gamma-compiler" < "$2.gamma" > "$2"
    chmod +x "$2"
}

python3 "$parts" compiler > "$out/compiler.delta"
build_delta "$out/compiler.delta" "$out/epsilon-compiler"
"$out/epsilon-compiler" < program.epsilon > "$out/module.observation"
python3 "$parts" program < "$out/module.observation" > "$out/program.delta"
build_delta "$out/program.delta" "$out/program"
"$out/program" < /dev/null | od -An -tu1
```

For example, this `program.epsilon` writes byte 90 and exits with code 42.
The resulting observation bytes are `0 42 0 0 0 90`.

```epsilon
boundary trait Console {
    machine exit_process(return_code: i32) -> never;
    machine write_byte(value: i32);
    machine read_byte() -> i32;
    machine write_line(text: &[u8]);
}
data Main { console: Console; }
machine Main::main(&mut self) {
    self.console.write_byte(90);
    self.console.exit_process(42);
}
```

The final Delta `main` receives the program's stdin bytes. A stored program
enters the compiler-generated `epsilon_compiled_entry`, which initializes
the checked `Main` receiver and calls the authored `Main::main`. In the pure
profile an empty receiver body compiles as a resultless function. Its entry
invokes that function and encodes the outcome. Both profiles publish the same
Epsilon exit and trap observations. [Stored entry generation](entry.delta) ·
[Pure entry](runtime/pure_entry.delta) · [Delta process entry](runtime/main.delta)
