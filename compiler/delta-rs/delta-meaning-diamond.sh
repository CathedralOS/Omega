#!/usr/bin/env sh
# DELTA-MEANING DIAMOND — the first thread of getting delta out of Rust.
#
# rungs/delta.md: delta's meaning is "Written in Delta/Gamma" -- defined by the reference
# interpreter, not the native (Rust on-ramp) backend. This diamond pins that meaning for the
# supported subset (straight-line integer code AND state machines): a Delta program is run TWO ways and the exit codes must match:
#   (1) NATIVE   -- compiled by the delta-rs aarch64 backend and executed
#   (2) GAMMA    -- `DELTA_EMIT=gamma` translates it to a gamma expression, which the Rust-FREE
#                   reference interpreter (interp.beta, built by the alpha->beta->bc pipeline) runs
# Agreement is evidence delta's native execution and its lattice-defined meaning coincide -- the
# same move that put gamma and the checker into the lineage, now reaching up to delta. As the
# supported subset grows (states, mutation, calls), this diamond widens with it.
#
# Skips cleanly off macOS arm64 or without the cargo/clang toolchain (the native route needs them).
set -e
cd "$(dirname "$0")"
case "$(uname -sm)" in "Darwin arm64") ;; *) echo "delta-meaning diamond SKIP — not macOS arm64"; exit 0 ;; esac
for t in cargo clang codesign; do command -v "$t" >/dev/null 2>&1 || { echo "delta-meaning diamond SKIP — no $t"; exit 0; }; done

T=$(mktemp -d); trap 'rm -rf "$T"' EXIT

# the gamma reference interpreter (trust-lineage: alpha seed -> beta asm -> bc -> interp.exe)
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null ) || { echo "delta-meaning diamond FAIL — bc build"; exit 1; }
if ../beta-lang-rs/build/bc.exe < ../gamma/interp.beta > "$T/i.asm" 2>/dev/null \
   && "$ASM" < "$T/i.asm" > "$T/i.tape" 2>/dev/null \
   && stamp_seed "$T/i.tape" "$SEED" "$T/interp.exe" >/dev/null 2>&1; then :; else
  echo "delta-meaning diamond FAIL — could not build interp.beta"; exit 1; fi
cargo build -q 2>/dev/null || { echo "delta-meaning diamond FAIL — cargo build"; exit 1; }

PASS=0; FAIL=0
# dia DESC  SRC  EXPECT : native exit, gamma-interp exit, and EXPECT must all agree (exit codes are
# the low byte, so keep the result in 0..255).
dia() {
  printf '%s' "$2" > "$T/p.alp"
  DELTA_ARCH=aarch64 ./target/debug/delta "$T/p.alp" "$T/p" >/dev/null 2>&1 || { FAIL=$((FAIL+1)); echo "  FAIL $1 : native compile"; return; }
  chmod +x "$T/p"; set +e; "$T/p"; nat=$?; set -e
  g=$(DELTA_EMIT=gamma ./target/debug/delta "$T/p.alp" 2>/dev/null)
  if [ -z "$g" ]; then FAIL=$((FAIL+1)); echo "  FAIL $1 : no gamma emitted (outside the supported subset?)"; return; fi
  set +e; printf '%s\n' "$g" | "$T/interp.exe" >/dev/null; gi=$?; set -e
  if [ "$nat" = "$gi" ] && [ "$nat" = "$3" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : native=$nat gamma=$gi expect=$3"; fi
}

# diar DESC SRC "b0 b1 …" EXPECT : like `dia` but feeds the decimal bytes to native stdin AND bakes them
# into the gamma program via DELTA_GAMMA_INPUT (the read_byte slice) -- both routes see the same input.
diar() {
  printf '%s' "$2" > "$T/p.alp"
  DELTA_ARCH=aarch64 ./target/debug/delta "$T/p.alp" "$T/p" >/dev/null 2>&1 || { FAIL=$((FAIL+1)); echo "  FAIL $1 : native compile"; return; }
  chmod +x "$T/p"
  bytes=""; for b in $3; do bytes="$bytes$(printf '\\%03o' "$b")"; done
  set +e; printf "$bytes" | "$T/p"; nat=$?; set -e
  g=$(DELTA_GAMMA_INPUT="$3" DELTA_EMIT=gamma ./target/debug/delta "$T/p.alp" 2>/dev/null)
  if [ -z "$g" ]; then FAIL=$((FAIL+1)); echo "  FAIL $1 : no gamma emitted"; return; fi
  set +e; printf '%s\n' "$g" | "$T/interp.exe" >/dev/null; gi=$?; set -e
  if [ "$nat" = "$gi" ] && [ "$nat" = "$4" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : native=$nat gamma=$gi expect=$4"; fi
}

# diao DESC SRC "in-bytes" "out-bytes" : OUTPUT programs. Compares native STDOUT (raw bytes) to the
# gamma route, where the program returns its output as a LIST that interp prints -- decoded back to bytes.
diao() {
  printf '%s' "$2" > "$T/p.alp"
  DELTA_ARCH=aarch64 ./target/debug/delta "$T/p.alp" "$T/p" >/dev/null 2>&1 || { FAIL=$((FAIL+1)); echo "  FAIL $1 : native compile"; return; }
  chmod +x "$T/p"
  bytes=""; for b in $3; do bytes="$bytes$(printf '\\%03o' "$b")"; done
  nout=$(printf "$bytes" | "$T/p" | od -An -tu1 | tr ' ' '\n' | grep -vE '^$' | tr '\n' ' '); nout=${nout% }
  g=$(DELTA_GAMMA_INPUT="$3" DELTA_EMIT=gamma ./target/debug/delta "$T/p.alp" 2>/dev/null)
  if [ -z "$g" ]; then FAIL=$((FAIL+1)); echo "  FAIL $1 : no gamma emitted"; return; fi
  gout=$(printf '%s\n' "$g" | "$T/interp.exe" | grep -oE '[0-9]+' | tr '\n' ' '); gout=${gout% }
  if [ "$nout" = "$gout" ] && [ "$nout" = "$4" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : native=[$nout] gamma=[$gout] expect=[$4]"; fi
}

# diaf DESC FILE "in-bytes" : a REAL sample program from samples/. Checks native stdout == the gamma
# route (no hardcoded expected output — the two meanings agreeing IS the check). Skips if gamma is empty
# (an unsupported feature) or interp runs out of memory on a large certificate.
diaf() {
  DELTA_ARCH=aarch64 ./target/debug/delta "samples/$2" "$T/p" >/dev/null 2>&1 || { FAIL=$((FAIL+1)); echo "  FAIL $1 : native compile"; return; }
  chmod +x "$T/p"
  raw=""; for b in $3; do raw="$raw$(printf '\\%03o' "$b")"; done
  nout=$(printf "$raw" | "$T/p" | od -An -tu1 | tr ' ' '\n' | grep -vE '^$' | tr '\n' ' '); nout=${nout% }
  g=$(DELTA_GAMMA_INPUT="$3" DELTA_EMIT=gamma ./target/debug/delta "samples/$2" 2>/dev/null)
  if [ -z "$g" ]; then FAIL=$((FAIL+1)); echo "  FAIL $1 : gamma emitted nothing"; return; fi
  gout=$(printf '%s\n' "$g" | "$T/interp.exe" 2>/dev/null | grep -oE '[0-9]+' | tr '\n' ' '); gout=${gout% }
  if [ "$nout" = "$gout" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : native!=gamma"; fi
}

# diax DESC FILE EXPECT : compare the native and Gamma exit results for a real
# sample whose observable result is its process status rather than stdout.
diax() {
  DELTA_ARCH=aarch64 ./target/debug/delta "samples/$2" "$T/p" >/dev/null 2>&1 || { FAIL=$((FAIL+1)); echo "  FAIL $1 : native compile"; return; }
  chmod +x "$T/p"; set +e; "$T/p"; nat=$?; set -e
  g=$(DELTA_EMIT=gamma ./target/debug/delta "samples/$2" 2>/dev/null)
  if [ -z "$g" ]; then FAIL=$((FAIL+1)); echo "  FAIL $1 : gamma emitted nothing"; return; fi
  set +e; printf '%s\n' "$g" | "$T/interp.exe" >/dev/null; gi=$?; set -e
  if [ "$nat" = "$gi" ] && [ "$nat" = "$3" ]; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : native=$nat gamma=$gi expect=$3"; fi
}

H='boundary trait Console { machine exit_process(return_code: i32); } data Main { console: Console; }'
dia "const"        "$H machine Main::main(&mut self) { self.console.exit_process(42); }" 42
dia "add"          "$H machine Main::main(&mut self) { let a: i32 = 2 + 3; self.console.exit_process(a); }" 5
dia "chain *,-"    "$H machine Main::main(&mut self) { let a: i32 = 2 + 3; let b: i32 = a * 4; let c: i32 = b - 1; self.console.exit_process(c); }" 19
dia "nested arith" "$H machine Main::main(&mut self) { let a: i32 = (2 + 3) * (4 + 1); self.console.exit_process(a); }" 25
dia "reuse local"  "$H machine Main::main(&mut self) { let a: i32 = 7; let b: i32 = a * a; let c: i32 = b - a; self.console.exit_process(c); }" 42
# division, modulo, and the full comparison set (faithfully encoded from lt/eq in gamma)
dia "div,mod"      "$H machine Main::main(&mut self) { let q: i32 = 17 / 5; let r: i32 = 17 % 5; self.console.exit_process(q * 10 + r); }" 32
dia "lt true"      "$H machine Main::main(&mut self) { let c: i32 = 3 < 5; self.console.exit_process(c); }" 1
dia "gt false"     "$H machine Main::main(&mut self) { let c: i32 = 3 > 5; self.console.exit_process(c); }" 0
dia "eq/ne"        "$H machine Main::main(&mut self) { let a: i32 = 4 == 4; let b: i32 = 4 != 4; self.console.exit_process(a * 2 + b); }" 2
dia "le boundary"  "$H machine Main::main(&mut self) { let a: i32 = 5 <= 5; let b: i32 = 6 <= 5; self.console.exit_process(a * 2 + b); }" 2
dia "ge boundary"  "$H machine Main::main(&mut self) { let a: i32 = 5 >= 5; let b: i32 = 4 >= 5; self.console.exit_process(a * 2 + b); }" 2
# min/max builtins — the `(if (lt a b) ..)` select; both routes agree, incl. the nested clamp idiom.
dia "min pick"     "$H machine Main::main(&mut self) { self.console.exit_process(min(9, 4)); }" 4
dia "max pick"     "$H machine Main::main(&mut self) { self.console.exit_process(max(9, 4)); }" 9
dia "clamp idiom"  "$H machine Main::main(&mut self) { let a: i32 = max(0, min(80, 60)); let b: i32 = max(0, min(50, 60)); self.console.exit_process(a + b); }" 110
# STATE MACHINES — loops with mutation + guarded transitions, modeled as mutually-recursive gamma defs
dia "sum 1..4"     "$H machine Main::main(&mut self) { let i: i32 = 0; let s: i32 = 0; transition 0 { _ -> lp() } state lp() { transition i < 4 { true -> bd()  false -> dn() } } state bd() { i = i + 1; s = s + i; transition 0 { _ -> lp() } } state dn() { self.console.exit_process(s); } }" 10
dia "sum 1..10"    "$H machine Main::main(&mut self) { let i: i32 = 0; let s: i32 = 0; transition 0 { _ -> lp() } state lp() { transition i < 10 { true -> bd()  false -> dn() } } state bd() { i = i + 1; s = s + i; transition 0 { _ -> lp() } } state dn() { self.console.exit_process(s); } }" 55
dia "factorial 5"  "$H machine Main::main(&mut self) { let i: i32 = 1; let a: i32 = 1; transition 0 { _ -> lp() } state lp() { transition i <= 5 { true -> bd()  false -> dn() } } state bd() { a = a * i; i = i + 1; transition 0 { _ -> lp() } } state dn() { self.console.exit_process(a); } }" 120
dia "gcd 48,36"    "$H machine Main::main(&mut self) { let a: i32 = 48; let b: i32 = 36; let t: i32 = 0; transition 0 { _ -> lp() } state lp() { transition b == 0 { true -> dn()  false -> st() } } state st() { t = a % b; a = b; b = t; transition 0 { _ -> lp() } } state dn() { self.console.exit_process(a); } }" 12
# min/max INSIDE a loop with mutation (the real clamp_sum shape) — running max, and a clamp-and-accumulate.
dia "running max"  "$H machine Main::main(&mut self) { let i: i32 = 1; let m: i32 = 0; transition 0 { _ -> lp() } state lp() { transition i < 5 { true -> bd()  false -> dn() } } state bd() { m = max(m, i * (6 - i)); i = i + 1; transition 0 { _ -> lp() } } state dn() { self.console.exit_process(m); } }" 9
dia "clamp+accum"  "$H machine Main::main(&mut self) { let i: i32 = 1; let s: i32 = 0; transition 0 { _ -> lp() } state lp() { transition i < 5 { true -> bd()  false -> dn() } } state bd() { s = s + max(0, min(i * 20 - 30, 60)); i = i + 1; transition 0 { _ -> lp() } } state dn() { self.console.exit_process(s); } }" 90
# SELF DATA FIELDS — zero-initialised, threaded through the state vector alongside locals
F='boundary trait Console { machine exit_process(return_code: i32); } data Main { console: Console; i: i32; s: i32; }'
dia "field write-read" "boundary trait Console { machine exit_process(return_code: i32); } data Main { console: Console; acc: i32; } machine Main::main(&mut self) { self.acc = 5; self.acc = self.acc + 3; self.console.exit_process(self.acc); }" 8
dia "field loop sum"   "$F machine Main::main(&mut self) { transition 0 { _ -> lp() } state lp() { transition self.i < 5 { true -> bd()  false -> dn() } } state bd() { self.i = self.i + 1; self.s = self.s + self.i; transition 0 { _ -> lp() } } state dn() { self.console.exit_process(self.s); } }" 15
dia "field+local mix"  "$F machine Main::main(&mut self) { let k: i32 = 3; transition 0 { _ -> lp() } state lp() { transition self.i < k { true -> bd()  false -> dn() } } state bd() { self.i = self.i + 1; self.s = self.s + self.i * k; transition 0 { _ -> lp() } } state dn() { self.console.exit_process(self.s); } }" 18
# CROSS-MACHINE CALLS — each reachable free machine becomes its own m{idx}_* defs; a call returns a value
dia "call chain"     "$H machine addk(a: i32, b: i32) -> i32 { return a + b; } machine dbl(x: i32) -> i32 { return x + x; } machine Main::main(&mut self) { let r: i32 = dbl(addk(20, 22)); self.console.exit_process(r - 80); }" 4
dia "recursive fact" "$H machine fact(n: i32) -> i32 { transition n < 2 { true -> one()  false -> rec() } state one() { return 1; } state rec() { return n * fact(n - 1); } } machine Main::main(&mut self) { self.console.exit_process(fact(5)); }" 120
dia "call in loop"   "$H machine inc(x: i32) -> i32 { return x + 1; } machine Main::main(&mut self) { let i: i32 = 0; let s: i32 = 0; transition 0 { _ -> lp() } state lp() { transition i < 5 { true -> bd()  false -> dn() } } state bd() { i = inc(i); s = s + i; transition 0 { _ -> lp() } } state dn() { self.console.exit_process(s); } }" 15
# SELF ARRAYS — modeled as a threaded gamma list with emitted nth/setl helpers
A='boundary trait Console { machine exit_process(return_code: i32); } data Main { console: Console; buf: [i32; 8]; i: i32; s: i32; }'
dia "array sum-of-sq" "$A machine Main::main(&mut self) { transition 0 { _ -> fl() } state fl() { transition self.i < 5 { true -> wr()  false -> rs() } } state wr() { self.buf[self.i] = self.i * self.i; self.i = self.i + 1; transition 0 { _ -> fl() } } state rs() { self.i = 0; transition 0 { _ -> sl() } } state sl() { transition self.i < 5 { true -> ad()  false -> dn() } } state ad() { self.s = self.s + self.buf[self.i]; self.i = self.i + 1; transition 0 { _ -> sl() } } state dn() { self.console.exit_process(self.s); } }" 30
dia "array index pick" "$A machine Main::main(&mut self) { transition 0 { _ -> fl() } state fl() { transition self.i < 4 { true -> wr()  false -> dn() } } state wr() { self.buf[self.i] = self.i + 1; self.i = self.i + 1; transition 0 { _ -> fl() } } state dn() { self.console.exit_process(self.buf[0] * 10 + self.buf[3]); } }" 14
diax "bootstrap fixed-backing storage" bootstrap-storage.alp 42
# READ_BYTE — the input stream threaded as a list; read_byte() consumes the head (or -1 at EOF)
R='boundary trait Console { machine exit_process(return_code: i32); machine read_byte() -> i32; } data Main { console: Console; c: i32; s: i32; }'
diar "sum input bytes" "$R machine Main::main(&mut self) { transition 0 { _ -> rd() } state rd() { self.c = read_byte(); transition self.c < 0 { true -> dn()  false -> ac() } } state ac() { self.s = self.s + self.c; transition 0 { _ -> rd() } } state dn() { self.console.exit_process(self.s); } }" "10 20 12" 42
diar "first byte"      "$R machine Main::main(&mut self) { self.c = read_byte(); self.console.exit_process(self.c); }" "200" 200
diar "count to EOF"    "$R machine Main::main(&mut self) { transition 0 { _ -> rd() } state rd() { self.c = read_byte(); transition self.c < 0 { true -> dn()  false -> ct() } } state ct() { self.s = self.s + 1; transition 0 { _ -> rd() } } state dn() { self.console.exit_process(self.s); } }" "7 7 7 7 7" 5
# STDOUT — write_byte/write_line modeled as an accumulated output list the program returns; interp prints
# it, the diamond decodes it back to bytes and compares to native's raw stdout.
W='boundary trait Console { machine exit_process(return_code: i32); machine read_byte() -> i32; machine write_byte(b: i32); machine write_line(text: &[u8]); } data Main { console: Console; c: i32; }'
diao "echo +1"     "$W machine Main::main(&mut self) { transition 0 { _ -> rd() } state rd() { self.c = read_byte(); transition self.c < 0 { true -> dn()  false -> ec() } } state ec() { self.console.write_byte(self.c + 1); transition 0 { _ -> rd() } } state dn() { self.console.exit_process(0); } }" "65 66 67" "66 67 68"
diao "write_line+byte" "$W machine Main::main(&mut self) { write_line(\"Hi\"); self.console.write_byte(33); self.console.exit_process(0); }" "" "72 105 10 33"
diao "count up output"  "$W machine Main::main(&mut self) { transition 0 { _ -> lp() } state lp() { transition self.c < 3 { true -> em()  false -> dn() } } state em() { self.console.write_byte(self.c + 65); self.c = self.c + 1; transition 0 { _ -> lp() } } state dn() { self.console.exit_process(0); } }" "" "65 66 67"
# INTEGRATION — exercise several features together (stdin + arrays + stdout + multiple loops + a call)
I='boundary trait Console { machine exit_process(return_code: i32); machine read_byte() -> i32; machine write_byte(b: i32); } data Main { console: Console; buf: [i32; 16]; n: i32; i: i32; c: i32; }'
diao "reverse filter" "$I machine Main::main(&mut self) { transition 0 { _ -> rd() } state rd() { self.c = read_byte(); transition self.c < 0 { true -> su()  false -> st() } } state st() { self.buf[self.n] = self.c; self.n = self.n + 1; transition 0 { _ -> rd() } } state su() { self.i = self.n - 1; transition 0 { _ -> em() } } state em() { transition self.i < 0 { true -> dn()  false -> wr() } } state wr() { self.console.write_byte(self.buf[self.i]); self.i = self.i - 1; transition 0 { _ -> em() } } state dn() { self.console.exit_process(0); } }" "65 66 67 68" "68 67 66 65"
diao "doubled filter" "$I machine inc2(x: i32) -> i32 { return x + x; } machine Main::main(&mut self) { transition 0 { _ -> rd() } state rd() { self.c = read_byte(); transition self.c < 0 { true -> dn()  false -> ed() } } state ed() { self.console.write_byte(inc2(self.c)); transition 0 { _ -> rd() } } state dn() { self.console.exit_process(0); } }" "10 20 30" "20 40 60"
# SELF-METHOD CALLS — self.m() mutates the shared self (fields + stdout); threaded as a Pair tuple in/out
M='boundary trait Console { machine exit_process(return_code: i32); machine read_byte() -> i32; machine write_byte(b: i32); } data Main { console: Console; tmp: i32; }'
diao "emit pair method" "$M machine Main::emitpair(&mut self, v: i32) { self.tmp = v; self.console.write_byte(self.tmp + 65); self.console.write_byte(self.tmp + 66); } machine Main::main(&mut self) { self.emitpair(0); self.emitpair(1); self.console.exit_process(0); }" "" "65 66 66 67"
diao "method echos input" "$M machine Main::emit(&mut self, v: i32) { self.console.write_byte(v); } machine Main::main(&mut self) { transition 0 { _ -> rd() } state rd() { self.tmp = read_byte(); transition self.tmp < 0 { true -> dn()  false -> ec() } } state ec() { self.emit(self.tmp + 1); transition 0 { _ -> rd() } } state dn() { self.console.exit_process(0); } }" "65 66 67" "66 67 68"

# REAL PROGRAMS — actual certifiers from samples/ (read stdin, compute, emit a proof certificate via
# emit_nat methods + write_line). The diamond reproduces their byte-exact output through the lattice.
# (Small inputs only: a large certificate's unary numerals exhaust interp's arena.)
diaf "certify-add (real)" certify-add.alp "50 32 51"   # '2 3' -> a+b certificate
diaf "certify-mul (real)" certify-mul.alp "50 32 52 32 53 32 54"   # '2 4 5 6' (a<B,b<C) -> overflow-linkage cert

echo "delta-meaning diamond (native execution vs gamma reference interpreter): $PASS agree, $FAIL disagree"
[ "$FAIL" = 0 ] || exit 1
