#!/usr/bin/env sh
# OMEGA KERNEL DIAMOND — delta's meaning, now with RUST OFF the meaning route.
#
# The delta-meaning diamond (delta-rs/delta-meaning-diamond.sh) already pins delta's meaning
# against native execution — but its translator, gamma_emit.rs, is RUST, so Rust still sat on the
# meaning side. This diamond removes it: omega/omega2gamma.beta is a Rust-FREE delta->gamma
# translator (built alpha->beta->bc, the same lineage as interp.beta). Each program is run TWO ways
# and the exit codes must agree:
#   (1) NATIVE     — compiled by the delta-rs aarch64 backend and executed (the reference)
#   (2) OMEGA2GAMMA  — omega2gamma.beta (Rust-free) translates it to gamma; interp.beta (Rust-free) runs it
# Both artifacts on route (2) execute without Rust, so Delta's supported meaning
# subset has a Rust-free steady route. The bc cold-start refinement edge remains
# separately explicit. As a bonus cross-check we also confirm the Rust-free route
# agrees with the existing Rust gamma_emit.rs route (DELTA_EMIT=gamma) — the two translators converge.
#
# SLICE 0: straight-line integer `main` (lets + exit_process terminal; + - * / %, parens, locals).
# The subset grows exactly as omega2gamma.beta grows (comparisons, states, calls, ... — later slices).
#
# Skips cleanly off macOS arm64 or without the cargo/clang toolchain (the native route needs them).
set -e
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "bootstrap paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh" || exit $?
cd "$OMEGA_GATE_DIR"
case "$(uname -sm)" in "Darwin arm64") ;; *) echo "omega kernel diamond SKIP — not macOS arm64"; exit 0 ;; esac
for t in cargo clang codesign; do command -v "$t" >/dev/null 2>&1 || { echo "omega kernel diamond SKIP — no $t"; exit 0; }; done

T=$(mktemp -d); trap 'rm -rf "$T"' EXIT

# Rust-free steady execution via the persisted lattice Beta compiler artifact.
. "${OMEGA_PATH_BETA}"/artifact_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_BETA_ASSEMBLER}"/$BETA_SEED
stamp_beta_compiler "$T/bc.exe" >/dev/null || { echo "omega kernel diamond FAIL — Beta compiler artifact"; exit 1; }
BC="$T/bc.exe"
build_beta() { # src.beta  ->  out.exe   (bc -> assemble -> stamp)
  "$BC" < "$1" > "$T/b.asm" 2>/dev/null && "$ASM" < "$T/b.asm" > "$T/b.tape" 2>/dev/null \
    && stamp_seed "$T/b.tape" "$SEED" "$2" >/dev/null 2>&1
}
build_beta "${OMEGA_PATH_GAMMA}"/interp.beta "$T/interp.exe" || { echo "omega kernel diamond FAIL — build interp.beta"; exit 1; }
build_beta "${OMEGA_PATH_OMEGA_BOOTSTRAP}/meaning/omega2gamma.beta" "$T/omega2gamma.exe" \
  || { echo "omega kernel diamond FAIL — build omega2gamma.beta"; exit 1; }

# native reference backend (Rust on-ramp — this is the thing being CHECKED, not trusted)
( cd "${OMEGA_PATH_DELTA_RUST}" && cargo build -q 2>/dev/null ) || { echo "omega kernel diamond FAIL — cargo build"; exit 1; }
BE="${OMEGA_PATH_DELTA_RUST}"/target/debug/delta

PASS=0; FAIL=0
# _check DESC EXPECT : assumes $T/p.alp is written; native exit, Rust-free omega2gamma-route exit, the Rust
# gamma_emit route, and EXPECT must all agree.
_check() {
  DELTA_ARCH=aarch64 "$BE" "$T/p.alp" "$T/p" >/dev/null 2>&1 || { FAIL=$((FAIL+1)); echo "  FAIL $1 : native compile"; return; }
  chmod +x "$T/p"; set +e; "$T/p"; nat=$?; set -e
  g=$("$T/omega2gamma.exe" < "$T/p.alp" 2>/dev/null)
  if [ -z "$g" ]; then FAIL=$((FAIL+1)); echo "  FAIL $1 : omega2gamma emitted nothing"; return; fi
  set +e; printf '%s\n' "$g" | "$T/interp.exe" >/dev/null; mine=$?; set -e
  rg=$(DELTA_EMIT=gamma "$BE" "$T/p.alp" 2>/dev/null); set +e; printf '%s\n' "$rg" | "$T/interp.exe" >/dev/null; rgi=$?; set -e
  if [ "$nat" = "$mine" ] && [ "$nat" = "$2" ] && [ "$nat" = "$rgi" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : native=$nat omega2gamma=$mine rustgamma=$rgi expect=$2"; fi
}
# dia DESC BODY EXPECT : BODY is the main body; Main has no scalar fields.
dia() {
  printf 'boundary trait Console { machine exit_process(return_code: i32); }\ndata Main { console: Console; }\nmachine Main::main(&mut self) {\n%s\n}\n' "$2" > "$T/p.alp"
  _check "$1" "$3"
}
# diaf DESC BODY EXPECT : like dia but Main also has scalar i32 fields `i` and `s` (self data slice).
diaf() {
  printf 'boundary trait Console { machine exit_process(return_code: i32); }\ndata Main { console: Console; i: i32; s: i32; }\nmachine Main::main(&mut self) {\n%s\n}\n' "$2" > "$T/p.alp"
  _check "$1" "$3"
}
# diac DESC MACHINES EXPECT : MACHINES is the full machine section (free machines + Main::main) — for
# the cross-machine-call slice, where the body needs sibling `machine name(..) -> i32 { .. }` definitions.
diac() {
  printf 'boundary trait Console { machine exit_process(return_code: i32); }\ndata Main { console: Console; }\n%s\n' "$2" > "$T/p.alp"
  _check "$1" "$3"
}
# diaa DESC BODY EXPECT : like diaf but Main also has a self-array field `buf: [i32; 5]` (self-array slice).
diaa() {
  printf 'boundary trait Console { machine exit_process(return_code: i32); }\ndata Main { console: Console; i: i32; s: i32; buf: [i32; 5]; }\nmachine Main::main(&mut self) {\n%s\n}\n' "$2" > "$T/p.alp"
  _check "$1" "$3"
}
# diar DESC BODY "b0 b1 .." EXPECT : the read_byte slice. Console also exposes read_byte(); the SAME bytes
# feed native stdin AND both gamma routes (Rust via DELTA_GAMMA_INPUT; Rust-free by substituting omega2gamma's
# STDIN placeholder with the (Cons b0 .. Nil) list). Main has scalar fields c, s.
diar() {
  printf 'boundary trait Console { machine exit_process(return_code: i32); machine read_byte() -> i32; }\ndata Main { console: Console; c: i32; s: i32; }\nmachine Main::main(&mut self) {\n%s\n}\n' "$2" > "$T/p.alp"
  DELTA_ARCH=aarch64 "$BE" "$T/p.alp" "$T/p" >/dev/null 2>&1 || { FAIL=$((FAIL+1)); echo "  FAIL $1 : native compile"; return; }
  chmod +x "$T/p"
  raw=""; for b in $3; do raw="$raw$(printf '\\%03o' "$b")"; done
  set +e; printf "$raw" | "$T/p"; nat=$?; set -e
  # build the gamma byte list (first byte outermost): reverse, then cons
  rev=""; for b in $3; do rev="$b $rev"; done
  list="Nil"; for b in $rev; do list="(Cons $b $list)"; done
  g=$("$T/omega2gamma.exe" < "$T/p.alp" 2>/dev/null | sed "s/STDIN/$list/")
  if [ -z "$g" ]; then FAIL=$((FAIL+1)); echo "  FAIL $1 : omega2gamma emitted nothing"; return; fi
  set +e; printf '%s\n' "$g" | "$T/interp.exe" >/dev/null; mine=$?; set -e
  rg=$(DELTA_GAMMA_INPUT="$3" DELTA_EMIT=gamma "$BE" "$T/p.alp" 2>/dev/null); set +e; printf '%s\n' "$rg" | "$T/interp.exe" >/dev/null; rgi=$?; set -e
  if [ "$nat" = "$mine" ] && [ "$nat" = "$4" ] && [ "$nat" = "$rgi" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : native=$nat omega2gamma=$mine rustgamma=$rgi expect=$4"; fi
}
# diao DESC FULLSRC "in-bytes" "out-bytes" : STDOUT programs. Compares the native process's raw stdout bytes
# to the gamma route's OUTPUT list (the program returns `(rev out Nil)`; interp prints it; decode the ints).
# FULLSRC is the entire program (varied data decls / arrays), like diac.
# dual-channel returns: output-mode programs yield (Pair <exit> <stdout list>) — strip the exit
# component before collecting the stdout bytes (the pair's first number is NOT an output byte).
decode_list() { printf '%s\n' "$1" | "$T/interp.exe" 2>/dev/null | sed 's/^(Pair [0-9]* //' | grep -oE '[0-9]+' | tr '\n' ' ' | sed 's/ *$//'; }
diao() {
  printf '%s\n' "$2" > "$T/p.alp"
  DELTA_ARCH=aarch64 "$BE" "$T/p.alp" "$T/p" >/dev/null 2>&1 || { FAIL=$((FAIL+1)); echo "  FAIL $1 : native compile"; return; }
  chmod +x "$T/p"
  raw=""; for b in $3; do raw="$raw$(printf '\\%03o' "$b")"; done
  nout=$(printf "$raw" | "$T/p" | od -An -tu1 | tr ' ' '\n' | grep -vE '^$' | tr '\n' ' '); nout=${nout% }
  rev=""; for b in $3; do rev="$b $rev"; done; list="Nil"; for b in $rev; do list="(Cons $b $list)"; done
  mout=$(decode_list "$("$T/omega2gamma.exe" < "$T/p.alp" 2>/dev/null | sed "s/STDIN/$list/")")
  rout=$(decode_list "$(DELTA_GAMMA_INPUT="$3" DELTA_EMIT=gamma "$BE" "$T/p.alp" 2>/dev/null)")
  if [ "$nout" = "$mout" ] && [ "$nout" = "$4" ] && [ "$nout" = "$rout" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : native=[$nout] omega2gamma=[$mout] rustgamma=[$rout] want=[$4]"; fi
}

dia "literal"            '    self.console.exit_process(42)' 42
dia "add"                '    self.console.exit_process(40 + 2)' 42
dia "sub"                '    self.console.exit_process(50 - 8)' 42
dia "mul"                '    self.console.exit_process(6 * 7)' 42
dia "div"                '    self.console.exit_process(84 / 2)' 42
dia "mod"                '    self.console.exit_process(142 % 100)' 42
dia "precedence"         '    self.console.exit_process(2 + 8 * 5)' 42
dia "parens"             '    self.console.exit_process((2 + 4) * 7)' 42
dia "left-assoc sub"     '    self.console.exit_process(50 - 3 - 5)' 42
dia "one local"          '    let a: i32 = 6 * 7;
    self.console.exit_process(a)' 42
dia "local chain"        '    let a: i32 = 6 * 7;
    let b: i32 = a - 2;
    let c: i32 = (a + b) / 2;
    self.console.exit_process(a + b + c - 81)' 42
dia "local in arith"     '    let x: i32 = 10;
    let y: i32 = x * x;
    self.console.exit_process(y - 58)' 42

# slice 1 — comparisons (faithfully from interp's only two primitives eq/lt).
dia "lt true"            '    let c: i32 = 3 < 5;
    self.console.exit_process(c + 41)' 42
dia "gt false"           '    let c: i32 = 3 > 5;
    self.console.exit_process(c + 42)' 42
dia "eq true"            '    let c: i32 = 7 == 7;
    self.console.exit_process(c * 42)' 42
dia "ne / eq combo"      '    let a: i32 = 4 == 4;
    let b: i32 = 4 != 4;
    self.console.exit_process(a * 42 + b)' 42
dia "le boundary"        '    let a: i32 = 5 <= 5;
    let b: i32 = 6 <= 5;
    self.console.exit_process(a * 42 + b)' 42
dia "ge boundary"        '    let a: i32 = 5 >= 5;
    let b: i32 = 4 >= 5;
    self.console.exit_process(a * 42 + b)' 42
dia "cmp under arith"    '    let a: i32 = 10;
    let b: i32 = (a > 5) * 30 + (a < 5) * 7 + 12;
    self.console.exit_process(b)' 42

# slice 2 — state machines (mutually-recursive gamma defs, SSA-threaded locals, guarded transitions).
dia "loop sum 1..4"      '    let i: i32 = 0;
    let s: i32 = 0;
    transition 0 { _ -> lp() }
    state lp() { transition i < 4 { true -> bd()  false -> dn() } }
    state bd() { i = i + 1; s = s + i; transition 0 { _ -> lp() } }
    state dn() { self.console.exit_process(s + 32); }' 42
dia "factorial-ish"      '    let i: i32 = 1;
    let a: i32 = 1;
    transition 0 { _ -> lp() }
    state lp() { transition i <= 5 { true -> bd()  false -> dn() } }
    state bd() { a = a * i; i = i + 1; transition 0 { _ -> lp() } }
    state dn() { self.console.exit_process(a - 78); }' 42
dia "gcd then offset"    '    let a: i32 = 90;
    let b: i32 = 48;
    let t: i32 = 0;
    transition 0 { _ -> lp() }
    state lp() { transition b == 0 { true -> dn()  false -> st() } }
    state st() { t = a % b; a = b; b = t; transition 0 { _ -> lp() } }
    state dn() { self.console.exit_process(a + 36); }' 42
dia "int-pattern switch" '    let x: i32 = 2;
    let r: i32 = 0;
    transition 0 { _ -> pick() }
    state pick() { transition x { 0 -> za()  1 -> ob()  _ -> tw() } }
    state za() { r = 1; transition 0 { _ -> dn() } }
    state ob() { r = 7; transition 0 { _ -> dn() } }
    state tw() { r = 42; transition 0 { _ -> dn() } }
    state dn() { self.console.exit_process(r); }' 42

# slice 3 — self data fields (threaded g{i} slots alongside locals, zero-initialised).
diaf "field loop sum"    '    transition 0 { _ -> lp() }
    state lp() { transition self.i < 5 { true -> bd()  false -> dn() } }
    state bd() { self.i = self.i + 1; self.s = self.s + self.i; transition 0 { _ -> lp() } }
    state dn() { self.console.exit_process(self.s + 27); }' 42
diaf "field+local mix"   '    let k: i32 = 3;
    transition 0 { _ -> lp() }
    state lp() { transition self.i < k { true -> bd()  false -> dn() } }
    state bd() { self.i = self.i + 1; self.s = self.s + self.i * k; transition 0 { _ -> lp() } }
    state dn() { self.console.exit_process(self.s + 24); }' 42
diaf "field cmp + arith" '    transition 0 { _ -> setup() }
    state setup() { self.i = 20; self.s = 22; transition 0 { _ -> dn() } }
    state dn() { self.console.exit_process(self.i + self.s); }' 42

# slice 4 — cross-machine calls (each reachable machine its own m{idx}_* defs; a call passes args + zeros).
diac "call chain (nested)" 'machine addk(a: i32, b: i32) -> i32 { return a + b; }
machine dbl(x: i32) -> i32 { return x + x; }
machine Main::main(&mut self) { let r: i32 = dbl(addk(20, 22)); self.console.exit_process(r - 42); }' 42
diac "recursive factorial" 'machine fact(n: i32) -> i32 { transition n < 2 { true -> one()  false -> rec() } state one() { return 1; } state rec() { return n * fact(n - 1); } }
machine Main::main(&mut self) { self.console.exit_process(fact(5) - 78); }' 42
diac "call inside a loop" 'machine inc(x: i32) -> i32 { return x + 1; }
machine Main::main(&mut self) { let i: i32 = 0; let s: i32 = 0; transition 0 { _ -> lp() } state lp() { transition i < 5 { true -> bd()  false -> dn() } } state bd() { i = inc(i); s = s + i; transition 0 { _ -> lp() } } state dn() { self.console.exit_process(s + 27); } }' 42
diac "two-arg helper twice" 'machine amax(a: i32, b: i32) -> i32 { transition a < b { true -> hb()  false -> ha() } state ha() { return a; } state hb() { return b; } }
machine Main::main(&mut self) { let x: i32 = amax(10, 40); let y: i32 = amax(x, 2); self.console.exit_process(y + 2); }' 42

# slice 5 — self arrays (threaded as gamma lists with nth/setl; entry-only, zero-initialised).
diaa "array sum-of-squares" '    transition 0 { _ -> fl() }
    state fl() { transition self.i < 5 { true -> wr()  false -> rs() } }
    state wr() { self.buf[self.i] = self.i * self.i; self.i = self.i + 1; transition 0 { _ -> fl() } }
    state rs() { self.i = 0; transition 0 { _ -> sl() } }
    state sl() { transition self.i < 5 { true -> ad()  false -> dn() } }
    state ad() { self.s = self.s + self.buf[self.i]; self.i = self.i + 1; transition 0 { _ -> sl() } }
    state dn() { self.console.exit_process(self.s + 12); }' 42
diaa "array index pick" '    transition 0 { _ -> fl() }
    state fl() { transition self.i < 4 { true -> wr()  false -> dn() } }
    state wr() { self.buf[self.i] = self.i + 1; self.i = self.i + 1; transition 0 { _ -> fl() } }
    state dn() { self.console.exit_process(self.buf[0] * 10 + self.buf[3] + 28); }' 42

# slice 6 — read_byte (stdin threaded as a gamma list; -1 at EOF). Same bytes drive native + both gamma routes.
diar "sum input bytes" '    transition 0 { _ -> rd() }
    state rd() { self.c = read_byte(); transition self.c < 0 { true -> dn()  false -> ac() } }
    state ac() { self.s = self.s + self.c; transition 0 { _ -> rd() } }
    state dn() { self.console.exit_process(self.s); }' "10 20 12" 42
diar "echo first byte (unsigned)" '    transition 0 { _ -> rd() }
    state rd() { self.c = read_byte(); transition 0 { _ -> dn() } }
    state dn() { self.console.exit_process(self.c); }' "200" 200
diar "count bytes to EOF" '    transition 0 { _ -> rd() }
    state rd() { self.c = read_byte(); transition self.c < 0 { true -> dn()  false -> ac() } }
    state ac() { self.s = self.s + 1; transition 0 { _ -> rd() } }
    state dn() { self.console.exit_process(self.s); }' "5 5 5 5 5 5 5 5 5 5 5 5 5 5 5 5 5 5 5 5 5 5 5 5 5 5 5 5 5 5 5 5 5 5 5 5 5 5 5 5 5 5" 42

# slice 7 — stdout (write_byte/write_line accumulate an output list; the program returns `(rev out Nil)`).
W='boundary trait Console { machine exit_process(return_code: i32); machine read_byte() -> i32; machine write_byte(b: i32); machine write_line(text: &[u8]); } data Main { console: Console; c: i32; }'
diao "count-up output" "$W machine Main::main(&mut self) { transition 0 { _ -> lp() } state lp() { transition self.c < 3 { true -> em()  false -> dn() } } state em() { self.console.write_byte(self.c + 65); self.c = self.c + 1; transition 0 { _ -> lp() } } state dn() { self.console.exit_process(0); } }" "" "65 66 67"
diao "write_line + byte" "$W machine Main::main(&mut self) { write_line(\"Hi\"); self.console.write_byte(33); self.console.exit_process(0); }" "" "72 105 10 33"
diao "echo +1 filter" "$W machine Main::main(&mut self) { transition 0 { _ -> rd() } state rd() { self.c = read_byte(); transition self.c < 0 { true -> dn()  false -> ec() } } state ec() { self.console.write_byte(self.c + 1); transition 0 { _ -> rd() } } state dn() { self.console.exit_process(0); } }" "65 66 67" "66 67 68"
I='boundary trait Console { machine exit_process(return_code: i32); machine read_byte() -> i32; machine write_byte(b: i32); } data Main { console: Console; buf: [i32; 16]; n: i32; i: i32; c: i32; }'
diao "reverse filter (in+out+array)" "$I machine Main::main(&mut self) { transition 0 { _ -> rd() } state rd() { self.c = read_byte(); transition self.c < 0 { true -> su()  false -> st() } } state st() { self.buf[self.n] = self.c; self.n = self.n + 1; transition 0 { _ -> rd() } } state su() { self.i = self.n - 1; transition 0 { _ -> em() } } state em() { transition self.i < 0 { true -> dn()  false -> wr() } } state wr() { self.console.write_byte(self.buf[self.i]); self.i = self.i - 1; transition 0 { _ -> em() } } state dn() { self.console.exit_process(0); } }" "65 66 67 68" "68 67 66 65"
diao "doubled filter (out+call)" "$I machine inc2(x: i32) -> i32 { return x + x; } machine Main::main(&mut self) { transition 0 { _ -> rd() } state rd() { self.c = read_byte(); transition self.c < 0 { true -> dn()  false -> ed() } } state ed() { self.console.write_byte(inc2(self.c)); transition 0 { _ -> rd() } } state dn() { self.console.exit_process(0); } }" "10 20 30" "20 40 60"

# slice 8 (capstone) — SELF-METHODS: `self.m(args)` shares & mutates `self`, threading the unified self-state.
M='boundary trait Console { machine exit_process(return_code: i32); machine read_byte() -> i32; machine write_byte(b: i32); } data Main { console: Console; tmp: i32; }'
diao "self-method emits pair" "$M machine Main::emitpair(&mut self, v: i32) { self.tmp = v; self.console.write_byte(self.tmp + 65); self.console.write_byte(self.tmp + 66); } machine Main::main(&mut self) { self.emitpair(0); self.emitpair(1); self.console.exit_process(0); }" "" "65 66 66 67"
diao "self-method echos input" "$M machine Main::emit(&mut self, v: i32) { self.console.write_byte(v); } machine Main::main(&mut self) { transition 0 { _ -> rd() } state rd() { self.tmp = read_byte(); transition self.tmp < 0 { true -> dn()  false -> ec() } } state ec() { self.emit(self.tmp + 1); transition 0 { _ -> rd() } } state dn() { self.console.exit_process(0); } }" "65 66 67" "66 67 68"

echo "omega kernel diamond (native == Rust-free omega2gamma->interp == Rust gamma_emit): $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
