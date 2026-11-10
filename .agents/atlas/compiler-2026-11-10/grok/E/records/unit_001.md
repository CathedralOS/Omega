# Atlas unit_001 — bootstrap chain floor through Delta checking frontier

Checkout read against working tree (later than digest commit `457adf4a11f`). Claims marked **read**, **grepped**, or **inferred**.

---

## bootstrap/0_alpha/README.md (lines 1–174, read)

**Purpose.** Documents Alpha as the trusted native tape VM floor: AlphaBootstrapV5 memory, three audited seeds, bounds contract, and edges to Beta (`bootstrap/0_alpha/README.md:1-78`).

**Shape.** Markdown only; no executable types. Retention inventory table at `bootstrap/0_alpha/README.md:158-174`.

**Inputs and outputs.** Consumes `SEMANTICS.md` and native artifacts; points to `tools/bootstrap/alpha/`, `tests/alpha/`, `tests/bootstrap/alpha-beta-edge.sh` (`bootstrap/0_alpha/README.md:147-152`).

**How it decides.** Judgments are documentary (SHA-256 identities, platform recipes); e.g. arm64 extent fixed at `movz …, #0x20` not 32 TiB (`bootstrap/0_alpha/README.md:105-115`).

**Cases by shape.** Distinguishes Windows PE, macOS Mach-O, Linux ELF realizations (`bootstrap/0_alpha/README.md:51-67`).

**Same job elsewhere.** Linux and arm64 sources both implement the same 21-opcode machine (`alpha_x64_linux.s:4-10`, `alpha_arm64_macos.s:4-8`).

**Unused or one-caller code.** N/A (markdown).

**Stated limits.** "Alpha has no textual source language, assembler grammar, type system, or proof kernel" (`bootstrap/0_alpha/README.md:74-78`). Tape hole 16,777,212 bytes (`bootstrap/0_alpha/README.md:88-90`).

**Fail-open risks.** none seen (specification text).

---

## bootstrap/0_alpha/SEMANTICS.md (lines 1–187, read)

**Purpose.** Authoritative small-step semantics for the Alpha register machine: state, opcodes, traps, bounds (`bootstrap/0_alpha/SEMANTICS.md:11-14`).

**Shape.** Configuration `(pc, M, R, sp)` plus I/O streams (`bootstrap/0_alpha/SEMANTICS.md:18-27`). Opcode table `bootstrap/0_alpha/SEMANTICS.md:91-115`.

**Inputs and outputs.** Defines loader tape layout `[4-byte LE L][bytecode]` (`bootstrap/0_alpha/SEMANTICS.md:67-76`); outputs halts, traps, stdout bytes via `write`.

**How it decides.** Opcode `0x00-0x14` legal; `0x15..0xFE` and `0xFF` trap (`bootstrap/0_alpha/SEMANTICS.md:114-115`). Signed `div`/`mod` trap conditions (`bootstrap/0_alpha/SEMANTICS.md:99-100`, `138-144`).

**Cases by shape.** Fixed 21-opcode table; variable instruction widths via width table (`bootstrap/0_alpha/SEMANTICS.md:54-63`, `83-85`).

**Same job elsewhere.** Implementations in `alpha_arm64_macos.s` and `alpha_x64_linux.s` (read below).

**Unused or one-caller code.** N/A.

**Stated limits.** `MEMSIZE` parameter 128 GiB for V5 (`bootstrap/0_alpha/SEMANTICS.md:29-37`). Mutable code legal (`bootstrap/0_alpha/SEMANTICS.md:158-164`).

**Fail-open risks.** none seen at spec level; failed checks trap before effect (`bootstrap/0_alpha/SEMANTICS.md:83-85`, `155-156`).

---

## bootstrap/0_alpha/alpha_arm64_macos.s (lines 1–379, read)

**Purpose.** Hand-authored macOS arm64 Mach-O seed implementing Alpha SEMANTICS (`alpha_arm64_macos.s:1-12`).

**Shape.** Globals `_main`, `_tape`; register roles x19–x24, x20=M, x21=pc, x22=sp, x23=MEMSIZE (`alpha_arm64_macos.s:15-18`). Entry `_main` `alpha_arm64_macos.s:44-87`; dispatch `next` `alpha_arm64_macos.s:96-152`; width table `Lwidths` `alpha_arm64_macos.s:154-155`.

**Inputs and outputs.** `mmap` for M, copies `_tape` hole, `read`/`write` syscalls, `halt` exits with register code (`alpha_arm64_macos.s:51-87`, `316-343`, `364-372`).

**How it decides.** Unsigned pc offset vs x23 before fetch (`alpha_arm64_macos.s:97-108`); opcode `cmp w9, #20` then `b.hi Lbounds` (`alpha_arm64_macos.s:102-103`). Div/mod pre-checks mirror x86 `#DE` (`alpha_arm64_macos.s:193-209`).

**Cases by shape.** Handler per opcode 0–20; unknown/`>0x14` → `Lbounds`/`udf` (`alpha_arm64_macos.s:110-152`, `152-153`).

**Same job elsewhere.** Parallels `alpha_x64_linux.s` dispatch and handlers (read).

**Unused or one-caller code.** N/A (single `_main` entry).

**Stated limits.** MEMSIZE `movz x1, #0x20, lsl #32` (`alpha_arm64_macos.s:58-59`, `67`). Tape hole `.space 0xfffffc` (`alpha_arm64_macos.s:377-379`).

**Fail-open risks.** none seen; out-of-range uses `udf`/`Lbounds` before memory effect (`alpha_arm64_macos.s:152-153`, `230-231`).

---

## bootstrap/0_alpha/alpha_arm64_macos.lst (lines 1–310, read)

**Purpose.** Committed disassembly audit companion for `alpha_arm64_macos.s` (`alpha_arm64_macos.s:10-12`).

**Shape.** `otool`-style listing: `__TEXT,__text` from `_main` through `h_halt` `ret` (`alpha_arm64_macos.lst:1-4`, `302-310`).

**Inputs and outputs.** Static bytes; maps symbols to addresses (e.g. `next` at `0x10000054c` `alpha_arm64_macos.lst:44-45`).

**How it decides.** Listing mirrors source control flow (e.g. `b.hs 0x100000624` at bounds `alpha_arm64_macos.lst:18`, `47-50`).

**Cases by shape.** One block per handler label matching source.

**Same job elsewhere.** `alpha_x64_linux.lst` for Linux seed.

**Unused or one-caller code.** N/A.

**Stated limits.** Comment in README: disassembly reflects CommandLineTools linker layout (`bootstrap/0_alpha/README.md:98-103`).

**Fail-open risks.** none seen (not executable logic).

---

## bootstrap/0_alpha/alpha_x64_linux.s (lines 1–406, read)

**Purpose.** Static ELF64 Linux Alpha VM, syscall I/O, same semantics as Windows hex listing (`alpha_x64_linux.s:1-10`).

**Shape.** `_start` at `alpha_x64_linux.s:30`; registers rbx=vregs, rdi=M, rsi=pc, r12=sp, r13/r14 bounds (`alpha_x64_linux.s:17-19`). `next` dispatch `alpha_x64_linux.s:74-129`; `widths` `alpha_x64_linux.s:382-383`; `tape_hole` `alpha_x64_linux.s:403-404`.

**Inputs and outputs.** `mmap` with `MAP_NORESERVE` (`alpha_x64_linux.s:34-48`); loader loop `alpha_x64_linux.s:57-68`; `getbyte`/`putbyte` syscalls (`alpha_x64_linux.s:348-379`).

**How it decides.** `cmp eax, 0x14` / `ja trap` before width table (`alpha_x64_linux.s:80-81`); `cmp rdx, r13` on fetches (`alpha_x64_linux.s:77-78`).

**Cases by shape.** Mirrors arm64 opcode set; `trap` via `ud2` (`alpha_x64_linux.s:130-131`).

**Same job elsewhere.** arm64 macOS source (read).

**Unused or one-caller code.** N/A.

**Stated limits.** MEMSIZE `0x2000000000` in r13 (`alpha_x64_linux.s:40`, `70`); tape cmp `0xfffffc` (`alpha_x64_linux.s:53-54`).

**Fail-open risks.** none seen; call checks sp before write (`alpha_x64_linux.s:314-316`).

---

## bootstrap/0_alpha/alpha_x64_linux.lst (lines 1–321, read)

**Purpose.** Committed `objdump` disassembly of `alpha_x64_linux` (`alpha_x64_linux.lst:1-7`).

**Shape.** `.text` from `0x401000` through syscall helpers and `widths` rodata tail (`alpha_x64_linux.lst:311-321`).

**Inputs and outputs.** Audit bytes only.

**How it decides.** Matches source trap at `0x401189` `ud2` (`alpha_x64_linux.lst:99`).

**Same job elsewhere.** `alpha_arm64_macos.lst`.

**Fail-open risks.** none seen.

---

## bootstrap/1_beta/README.md (lines 1–74, read)

**Purpose.** Beta as human-readable Alpha assembly layer; trusted compiler tape (`bootstrap/1_beta/README.md:1-15`).

**Shape.** Pipeline diagram `audited native Alpha VM + beta_compiler_bytecode.tape + program.beta` (`bootstrap/1_beta/README.md:17-22`).

**Inputs and outputs.** Points to `LANGUAGE.md`, `AUDIT.md`, `beta_compiler.beta`, tests under `tests/beta/compiler/` (`bootstrap/1_beta/README.md:36-65`).

**How it decides.** Byte-for-byte reconstruction obligation (`bootstrap/1_beta/README.md:24-27`).

**Stated limits.** No symbols/relocations (`bootstrap/1_beta/README.md:56-58` via LANGUAGE cross-ref).

**Fail-open risks.** none seen in README.

---

## bootstrap/1_beta/AUDIT.md (lines 1–121, read)

**Purpose.** Finite audit of admitted `beta_compiler_bytecode.tape` and source (`bootstrap/1_beta/AUDIT.md:1-5`).

**Shape.** SHA-256 table `bootstrap/1_beta/AUDIT.md:9-12`; instruction inventory `bootstrap/1_beta/AUDIT.md:25-33`; routine map `bootstrap/1_beta/AUDIT.md:42-53`.

**How it decides.** Exit statuses 0,7,8,9 documented (`bootstrap/1_beta/AUDIT.md:67-70`); status-gated publication (`bootstrap/1_beta/AUDIT.md:70-71`).

**Cases by shape.** All 253 instructions reachable; no `div`/`mod` in compiler (`bootstrap/1_beta/AUDIT.md:94-96`).

**Stated limits.** Source buffer `0x100000..0x4100000`; output max `0xfffffc` (`bootstrap/1_beta/AUDIT.md:79-80`).

**Fail-open risks.** **stated**: late failure may stream prefix but only status 0 publishes artifact (`bootstrap/1_beta/AUDIT.md:69-71`, `LANGUAGE.md:106-108`).

---

## bootstrap/1_beta/LANGUAGE.md (lines 1–131, read)

**Purpose.** Partial function `assemble(P)=T` from Beta text to raw tape bytes (`bootstrap/1_beta/LANGUAGE.md:7-12`).

**Shape.** Grammar `program := item*` etc. (`bootstrap/1_beta/LANGUAGE.md:29-39`); mnemonic/opcode table `bootstrap/1_beta/LANGUAGE.md:61-83`.

**How it decides.** Address assertions require `a == output length` (`bootstrap/1_beta/LANGUAGE.md:94-96`); lowercase hex only (`bootstrap/1_beta/LANGUAGE.md:41-44`).

**Cases by shape.** Fixed mnemonic operand width list per instruction (`bootstrap/1_beta/LANGUAGE.md:58-59`).

**Stated limits.** Source max `0x4000000`, output `0xfffffc` (`bootstrap/1_beta/LANGUAGE.md:110-112`).

**Fail-open risks.** **stated**: one-pass may write rejected stdout prefix on late failure (`bootstrap/1_beta/LANGUAGE.md:106-108`); not accepted assembly.

---

## bootstrap/1_beta/beta_compiler.beta (lines 1–383, read)

**Purpose.** Self-hosted Beta compiler: read stdin source, emit Alpha tape to stdout (`beta_compiler.beta:1-8`, `32-70`).

**Shape.** Routines by address: `read_loop` `0x53`, `scan_program` `0x123`, `next_token` `0x1e4`, `parse_hex_word` `0x353`, `parse_op` `0x47f`, `emit_operand` `0x550`, `mnemonic_table` `0x64d` (`beta_compiler.beta:22-383`).

**Inputs and outputs.** High registers hold source cursor/output (`beta_compiler.beta:10-19`); writes opcodes via `write` (`beta_compiler.beta:81-83`).

**How it decides.** Source bytes rejected outside envelope (`beta_compiler.beta:46-50`); unknown mnemonic `halt 8` (`beta_compiler.beta:297-299`); operand errors status 7 (`beta_compiler.beta:348-350`).

**Cases by shape.** Mnemonic table encodes 21 Alpha ops + `dw` (`beta_compiler.beta:361-383`); operand widths from NUL-terminated ASCII digit lists (`beta_compiler.beta:85-93`).

**Same job elsewhere.** Only implementation of Beta assemble in repo (**grepped** `parse_op`: 1 file `beta_compiler.beta`).

**Unused or one-caller code.** Beta assembly labels are not Rust `pub`; no omega-rust counterpart (**grepped**).

**Stated limits.** Comments document register ABI and buffer extents (`beta_compiler.beta:10-19`).

**Fail-open risks.** **stated**: output count advanced before operand write checks can leave one-byte prefix (`AUDIT.md:88-92`); status-gated publication prevents artifact.

---

## bootstrap/2_gamma/COMPOSED_ARTIFACT.md (lines 1–60, read)

**Purpose.** `GammaComposedV2` triple identity and invocation framing (`bootstrap/2_gamma/COMPOSED_ARTIFACT.md:3-8`).

**Shape.** Manifest fields evaluator/source/support SHA and lengths (`bootstrap/2_gamma/COMPOSED_ARTIFACT.md:12-18`).

**How it decides.** Plumbing may publish stdout only when status zero or nonempty, per evaluator convention (`bootstrap/2_gamma/COMPOSED_ARTIFACT.md:47-55`).

**Fail-open risks.** **stated**: empty nonzero stdout leaves destination unchanged (`bootstrap/2_gamma/COMPOSED_ARTIFACT.md:49`).

---

## bootstrap/2_gamma/EVALUATOR_PROFILE.md (lines 1–302, read)

**Purpose.** Request/observation contract and private memory map for `gamma_evaluator.beta` (`bootstrap/2_gamma/EVALUATOR_PROFILE.md:1-14`).

**Shape.** Partition table `bootstrap/2_gamma/EVALUATOR_PROFILE.md:59-68`; capacity table `bootstrap/2_gamma/EVALUATOR_PROFILE.md:90-100`.

**How it decides.** Status codes 0–4 and application remapping 248–254 (`bootstrap/2_gamma/EVALUATOR_PROFILE.md:31-53`).

**Stated limits.** Request 137,363,456 bytes; function census 2,097,152 rows (`bootstrap/2_gamma/EVALUATOR_PROFILE.md:13-14`, `93-94`). Nested-call policy retains explicit frames (`bootstrap/2_gamma/EVALUATOR_PROFILE.md:268-274`).

**Fail-open risks.** none seen for acceptance paths; exhaustive preflight before stores (`bootstrap/2_gamma/EVALUATOR_PROFILE.md:108-113`).

---

## bootstrap/2_gamma/LANGUAGE.md (lines 1–114, read)

**Purpose.** Gamma typed scalar/effect language: `def` functions, `Int` only, effects `input`/`read`/`write` (`bootstrap/2_gamma/LANGUAGE.md:1-6`).

**How it decides.** `$application` marker and `(pair status publish)` convention (`bootstrap/2_gamma/LANGUAGE.md:93-101`).

**Cases by shape.** Closed expression grammar (`bootstrap/2_gamma/LANGUAGE.md:16-34`).

---

## bootstrap/2_gamma/README.md (lines 1–67, read)

**Purpose.** Gamma as Beta-implemented evaluator for transformers and Delta stage (`bootstrap/2_gamma/README.md:1-13`).

**Shape.** Retention inventory (`bootstrap/2_gamma/README.md:30-38`).

---

## bootstrap/2_gamma/gamma_evaluator.beta (lines 1–1666, read)

**Purpose.** Direct Beta interpreter for Gamma: request framing, census, validate, eval, tail calls, pairs (`gamma_evaluator.beta:1-5`, `23-82`).

**Shape.** Major routines: `read_u32` `0x1b0`, `next_token` `0x28c`, `census` `0xa29`, `validate_program` `0xbfc`, `enter_function` `0x1025`, `eval_expression` `0x1455`, `finish_main` `0x1af2`, `indexed_find_function` `0x1f77` (`gamma_evaluator.beta` throughout).

**Inputs and outputs.** Reads sealed request at `0x10000000`; buffers output `0x18300000` (`gamma_evaluator.beta:26-35`, `1371-1390`); pair heap from `ra0` (`gamma_evaluator.beta:32-33`, `1217-1235`).

**How it decides.** `classify_list_head` table `0x1c63` (`gamma_evaluator.beta:1393-1425`); invalid → `halt 1` (`gamma_evaluator.beta:1305-1307`); div/mod → `authored_failure` status 2 (`gamma_evaluator.beta:1164-1172`, `1461-1464`).

**Cases by shape.** List-head dispatch for `if`/`let`/operators/`pair`/I/O (`gamma_evaluator.beta:501-572`, `1046-1065`). Application vs scalar `main` (`gamma_evaluator.beta:1320-1369`).

**Same job elsewhere.** **grepped** `next_token` in repo: also `beta_compiler.beta` (different language). No duplicate Gamma evaluator.

**Unused or one-caller code.** `find_function` at `0x8b5` jumps to indexed path only (`gamma_evaluator.beta:470-472`); dead `mov r0,r0` padding after some failure paths (`gamma_evaluator.beta:1309-1316`) — not claimed unused without separate reachability proof.

**Stated limits.** Profile cross-referenced in header comments and `EVALUATOR_PROFILE.md` (`gamma_evaluator.beta:1537-1539`).

**Fail-open risks.** **traced**: `finish_main` validates before flush for application pairs (`gamma_evaluator.beta:1334-1369`); scalar requires `0..255` (`gamma_evaluator.beta:1322-1326`).

---

## bootstrap/3_delta/delta_compiler.composed (lines 1–6, read)

**Purpose.** Pinned `GammaComposedV2` manifest for canonical Delta compiler closure (`delta_compiler.composed:1-6`).

**Shape.** Five lines: evaluator SHA, source SHA/length, support SHA/length.

---

## bootstrap/3_delta/delta_compiler.gamma (lines 1–22, read)

**Purpose.** Canonical DCREQ entry: admit request, compile, publish Gamma receipt or DCOUT failure (`delta_compiler.gamma:4-12`).

**Shape.** `main`, `source_length`, `source_read`, `compilation_profile` (`delta_compiler.gamma:4-22`).

**Inputs and outputs.** Delegates to `validate_compilation_request`, `compile_admitted_source`, `publish_compiler_failure` (Gamma definitions in implementation closure, not in this file).

---

## bootstrap/3_delta/LANGUAGE.md (lines 1–432, read)

**Purpose.** Normative Delta language, DCREQ framing, DCOUT boundary family (`bootstrap/3_delta/LANGUAGE.md:1-6`, `237-267`).

**How it decides.** Exhaustive match obligation (`bootstrap/3_delta/LANGUAGE.md:99-102`); compiler-application profile bytes (`bootstrap/3_delta/LANGUAGE.md:170-177`).

**Stated limits.** ConformanceBytesV1 4 MiB in/out (`bootstrap/3_delta/LANGUAGE.md:199-206`). Open edge noted (`bootstrap/3_delta/LANGUAGE.md:430-432`).

---

## bootstrap/3_delta/README.md (lines 1–470, read)

**Purpose.** Delta staged compiler organization, identity hashes, implemented slice description (`bootstrap/3_delta/README.md:1-20`, `50-57`).

**Shape.** Maps `implementation/` subsystems (`bootstrap/3_delta/README.md:64-80`); measurements table `bootstrap/3_delta/README.md:431-469`.

---

## bootstrap/3_delta/implementation/boundary/README.md (lines 1–590, read)

**Purpose.** DCOUT V1 request admission order, frontend rejection codes, resource ownership disposition (`bootstrap/3_delta/implementation/boundary/README.md:1-10`, `351-385`).

**How it decides.** Admission order: header → profile → length → body → support section (`bootstrap/3_delta/implementation/boundary/README.md:12-25`). Resource table codes 1–12 (`bootstrap/3_delta/implementation/boundary/README.md:358-371`).

**Stated limits.** Zero-use resources 9–11 in this producer (`bootstrap/3_delta/implementation/boundary/README.md:384-396`).

**Fail-open risks.** **stated**: canonical publisher required; empty evaluator stdout not decoded as DCOUT (`bootstrap/3_delta/implementation/boundary/README.md:585-586`).

---

## bootstrap/3_delta/implementation/boundary/execution_storage.md (lines 1–267, read)

**Purpose.** Source-level audit of Delta compiler execution on Gamma profile: contexts, lexicals, pair arena obligation (`bootstrap/3_delta/implementation/boundary/execution_storage.md:1-6`).

**Shape.** Fixed maxima table (`execution_storage.md:39-45`); bound `live call contexts <= 145` (`execution_storage.md:87-89`).

**Stated limits.** General pair containment proof **open**; measured worst-shape 417M pairs (`execution_storage.md:156-167`, `219-248`).

---

## bootstrap/3_delta/implementation/boundary/outcome.gamma (lines 1–46, read)

**Purpose.** DCOUT frame publisher and outcome carriers (`outcome.gamma:1-46`).

**Shape.** `compiler_failure`, `publish_compiler_failure`, `write_unsigned_little_endian` (`outcome.gamma:19-46`).

**How it decides.** Tag/space/code/coordinate/limit/requested packed then `pair tag 1` publish flag (`outcome.gamma:29-46`).

---

## bootstrap/3_delta/implementation/boundary/request.gamma (lines 1–57, read)

**Purpose.** DCREQ validation without output (`request.gamma:1-3`).

**Shape.** `validate_compilation_request` → header magic `DCREQ` bytes, profile 1 only, length cap 4,194,304 (`request.gamma:50-57`, `41-48`).

**How it decides.** `request_header_difference` finds first wrong byte (`request.gamma:9-14`); body/support extent checks (`request.gamma:30-39`).

**Fail-open risks.** none seen; failures return `compiler_failure` without emission (comment `request.gamma:1-2`).

---

## bootstrap/3_delta/implementation/checking/* (unit gamma/md ranges, read)

### checking/README.md (1–142)
Pair accounting rollup for census, resolution, typing (`checking/README.md:14-24`, `112-118`).

### checking/collection.gamma (1–109)
`collect_global_identities` walks syntax program; provisions type/constructor/function rows with limits 65536/32768 (`collection.gamma:4-30`, `50-63`, `79-88`, `103-109`).

### checking/declarations.gamma (1–40)
`resolve_collected_declarations` ordered pass over forms (`declarations.gamma:4-40`).

### checking/declarations/data.gamma (1–85), functions.gamma (1–75), types.gamma (1–21)
Data constructor resolution with cursor replace (`data.gamma:40-85`). Parameter catalog with conflict before annotation (`functions.gamma:5-32`). `lookup_type_option` for Int/Bytes/nominal (`types.gamma:4-21`).

### checking/environments.gamma (1–26)
`local_environment_provision` caps at 65536 bindings (`environments.gamma:14-17`).

### checking/lexical.gamma (1–110)
`validate_source_tokens` recursive scan; integer range after spelling (`lexical.gamma:6-19`, `44-47`, `103-110`).

### checking/names.gamma (1–226)
Sparse immutable `name_trie_*` API; metadata accessors (`names.gamma:1-227`).

### checking/names/README.md (1–166), cursor*.gamma (full ranges)
Cursor zipper for prefix reuse (`names/README.md:26-38`). `identity_cursor_seek` (`cursor_navigation.gamma:13-46`). `identity_cursor_commit` (`cursor_commit.gamma:5-19`).

### checking/profile.gamma (1–29)
`validate_application_profile` requires `main : Bytes -> Bytes` for profile 1 (`profile.gamma:12-29`).

### checking/source.gamma (1–253)
Envelope `validate_source_envelope` (`source.gamma:10-15`); keyword token predicates via packed ASCII (`source.gamma:77-167`).

**Collective purpose (checking unit slice).** Static frontend fragment: bytes → tokens → (syntax elsewhere) → global census → declaration typing metadata → profile gate. Phases return `compiler_complete` or `compiler_failure` pairs, not stdout (`boundary/README.md:7-9`).

**Same job elsewhere.** **grepped** `validate_compilation_request`, `DCOUT`, `compiler_failure` in `omega-rust`: **0 matches**. Product Rust compiler checking is separate subsystem (**suspected**, digest `area_01` / `comp/` — not opened this unit).

**Fail-open risks (checking).** Fail-closed: duplicate lookups precede provision (`collection.gamma:77-78`, `101-102`); `(def fail)` in `source.gamma` reads past input for tests (`source.gamma:1-3`) — test-only path if wired.

---

## Does the same job as Rust code

| Bootstrap artifact | Rust / product analogue | Verdict |
| --- | --- | --- |
| Alpha VM (`*.s`, SEMANTICS) | `omega-rust/omega/compilation/backend/target/` profile `alpha_bootstrap` is catalog identity only, **no native realization** (`area_01.md` citing `target_profile.rs:5-8`, `432` — **suspected**, line numbers from digest) | **DIFFERENT JOBS**: bootstrap seeds execute tapes; Rust backend emits hosted binaries. |
| Beta `beta_compiler.beta` | No `omega-rust` one-pass textual→tape assembler (**grepped**) | **DIFFERENT JOBS** |
| Gamma `gamma_evaluator.beta` | Terminal Psi interpreter in `build-settings` activation (`area_01.md` `bs/activation.rs` — **suspected**) interprets Build machines, not Gamma surface syntax | **DIFFERENT JOBS** |
| Delta `request.gamma` / `outcome.gamma` | Corpus CLI compile/refusal framing in `omega-rust/omega/application/cli/` (`area_01.md` — **suspected**) uses different outcome types | **DIFFERENT JOBS** |
| Delta `names.gamma` tries | Rust compiler uses dense tables/IDs per `tools/dense_id_maps.txt` policy in AGENTS.md — **inferred**, not traced to a single crate this unit | **UNDECIDED** for exact trie duplicate |

---

## Coverage

**Read in full (line counts from unit TSV):**

- `bootstrap/0_alpha/README.md` 1–174 (174)
- `bootstrap/0_alpha/SEMANTICS.md` 1–187 (187)
- `bootstrap/0_alpha/alpha_arm64_macos.lst` 1–310 (310)
- `bootstrap/0_alpha/alpha_arm64_macos.s` 1–379 (379)
- `bootstrap/0_alpha/alpha_x64_linux.lst` 1–321 (321)
- `bootstrap/0_alpha/alpha_x64_linux.s` 1–406 (406)
- `bootstrap/1_beta/AUDIT.md` 1–121 (121)
- `bootstrap/1_beta/LANGUAGE.md` 1–131 (131)
- `bootstrap/1_beta/README.md` 1–74 (74)
- `bootstrap/1_beta/beta_compiler.beta` 1–383 (383)
- `bootstrap/2_gamma/COMPOSED_ARTIFACT.md` 1–60 (60)
- `bootstrap/2_gamma/EVALUATOR_PROFILE.md` 1–302 (302)
- `bootstrap/2_gamma/LANGUAGE.md` 1–114 (114)
- `bootstrap/2_gamma/README.md` 1–67 (67)
- `bootstrap/2_gamma/gamma_evaluator.beta` 1–1666 (1666)
- `bootstrap/3_delta/LANGUAGE.md` 1–432 (432)
- `bootstrap/3_delta/README.md` 1–470 (470)
- `bootstrap/3_delta/delta_compiler.composed` 1–6 (6)
- `bootstrap/3_delta/delta_compiler.gamma` 1–22 (22)
- `bootstrap/3_delta/implementation/boundary/README.md` 1–590 (590)
- `bootstrap/3_delta/implementation/boundary/execution_storage.md` 1–267 (267)
- `bootstrap/3_delta/implementation/boundary/outcome.gamma` 1–46 (46)
- `bootstrap/3_delta/implementation/boundary/request.gamma` 1–57 (57)
- `bootstrap/3_delta/implementation/checking/README.md` 1–142 (142)
- `bootstrap/3_delta/implementation/checking/collection.gamma` 1–109 (109)
- `bootstrap/3_delta/implementation/checking/declarations.gamma` 1–40 (40)
- `bootstrap/3_delta/implementation/checking/declarations/data.gamma` 1–85 (85)
- `bootstrap/3_delta/implementation/checking/declarations/functions.gamma` 1–75 (75)
- `bootstrap/3_delta/implementation/checking/declarations/types.gamma` 1–21 (21)
- `bootstrap/3_delta/implementation/checking/environments.gamma` 1–26 (26)
- `bootstrap/3_delta/implementation/checking/lexical.gamma` 1–110 (110)
- `bootstrap/3_delta/implementation/checking/names.gamma` 1–226 (226)
- `bootstrap/3_delta/implementation/checking/names/README.md` 1–166 (166)
- `bootstrap/3_delta/implementation/checking/names/cursor.gamma` 1–39 (39)
- `bootstrap/3_delta/implementation/checking/names/cursor_commit.gamma` 1–28 (28)
- `bootstrap/3_delta/implementation/checking/names/cursor_navigation.gamma` 1–46 (46)
- `bootstrap/3_delta/implementation/checking/profile.gamma` 1–29 (29)
- `bootstrap/3_delta/implementation/checking/source.gamma` 1–253 (253)

**Skimmed:** none 
**Not read:** none 

**Greps run (whole repository unless noted):** `next_token` (5 matches), `gamma_evaluator` (many test/tool refs), `validate_compilation_request|DCOUT|compiler_failure` in `omega-rust` (0), `name_trie` (bootstrap delta only), `parse_op` (beta_compiler only), `alpha_x64_linux` in `omega-rust` (0).

---

## Five things a maintainer most needs to know

1. Trust chain here is **documented semantics + audited bytes**, not Rust types: Alpha `SEMANTICS.md` is the executable contract (`bootstrap/0_alpha/SEMANTICS.md:3-9`).
2. Beta and Gamma **materialize as fixed-size Alpha tapes** with byte-identical reconstruction gates (`bootstrap/1_beta/AUDIT.md:100-110`, `bootstrap/2_gamma/EVALUATOR_PROFILE.md:209-217`).
3. Gamma evaluator **preflights every partition** (request, stacks, pairs, output) before use; status codes are part of the public observation contract (`bootstrap/2_gamma/EVALUATOR_PROFILE.md:80-113`).
4. Delta canonical entry is **`delta_compiler.gamma` inside a 147,840-byte composed closure**, not the 22-line stub alone (`bootstrap/3_delta/README.md:50-57`, `delta_compiler.composed:1-6`).
5. Delta DCOUT failures are **40-byte framed** and precede compilation phases when request admission fails (`request.gamma:50-57`, `outcome.gamma:29-46`, `boundary/README.md:47-51`).

---

## What you are unsure about

- Whether any `omega-rust` crate duplicates Beta mnemonic parsing logic (**not grepped** beyond `parse_op`; **UNDECIDED**).
- Exact re-location of digest-cited `target_profile.rs` lines vs current checkout for `alpha_bootstrap` (**suspected** from `area_01.md`, not re-opened).
- Reachability of dead padding instructions in `gamma_evaluator.beta` after `jmp` to failure handlers (`gamma_evaluator.beta:1309-1316`) without running conformance tapes (**UNDECIDED**).

UNIT COMPLETE

