Built from `records/unit_084.md` (Part D only), `unit_085.md`, `unit_086.md`, `unit_087.md`, `unit_088.md`, `unit_089.md` (Part A only) and `crate-map/17-omega-source.md`, at commit `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. No second-pass records cover this area.

# Digest: 17-omega-source

Paths are relative to the checkout. Short prefixes used below:
- `S2P/` = `source/subsystems/compiler/source_to_psi/` (Omega package `psi`). Its stages: `lex/` = `S2P/pipeline/source_files_to_tokens/`, `parse/` = `.../tokens_to_syntax_trees/`, `resolve/` = `.../syntax_trees_to_symbol_resolved_trees/`, `type/` = `.../symbol_resolved_trees_to_typed_trees/`, `check/` = `.../typed_trees_to_checked_trees/`, `lower/` = `.../checked_trees_to_scalar_operations/`; `rep/` = `S2P/representations/`.
- `compiler/` = `source/subsystems/compiler/` (for `alpha/`, `driver/`, `request/`).
- `core/` = `source/library/core/`, `std/` = `source/library/std/`, `win/` = `std/targets/windows_x86_64/`.

Words: a **machine** is an Omega function; `transition x { pat -> target }` is its branch. A **boundary machine** has no body; the compiler supplies its behaviour. A **fail-open** place accepts, or returns a plausible default, when it does not know the answer. A **zone** is the checker's table of difference bounds (`x - y <= c`) over integer "subjects". "Read", "grepped", "inferred", "suspected", "open" and "not traced" are the record readers' words and keep their meaning.

## 1. What this area does

`source/` holds Omega-written code that a Rust or bootstrap compiler builds; it has no `Cargo.toml`. There are two groups. The first is a small Omega-written compiler: a lexer, parser, resolver, typer, checker and scalar lowerer (package `psi`), an Alpha tape emitter, a driver that chains them, an OCREQ request decoder that is separate from all of them, a 4-byte word decoder, and two hosted applications that wrap the driver and the decoder. The second group is the bundled libraries: `core` (declarations every program imports as `omega::language::core`, many of them bodyless and supplied by the compiler), `std` (console, filesystem, clock, time, per-target entry and calling policies), `alloc` (bump allocator and a `u32` vector) and `blocking-executor` (contracts only). Every compiler stage is a fixed-capacity `data` record filled by machines that `crashes Trap`; the first rejection stops the stage (record 087, "Common pattern"). The compiler accepts only a narrow subset: the lowerer refuses the whole program if any machine anywhere carries a `requires`, `ensures`, `terminates` or `satisfies` clause (`lower/scalar.omg:152-180,272-274`), and only unsigned integers, Bool, arrays, references and plain records get a layout (`lower/layout.omg:108-126`).

**`source/foundation/wire`** (11 lines). Four bytes in, one little-endian `u32` out: `u32_from_le_bytes` (`codec.omg:2-8`). No contract; the caller must have the bytes. Used by `application/construction/main.omg` and `compiler/request/protocol.omg` (2 lines each, grepped, record 084 D5).

**Lexer** (`lex/`, 1,504 lines). Source unit bytes in, `TokenStream` out (`rep/tokens/tokens.omg:150`). `tokenize` (`lex/lexer.omg:1402`) first requires the whole unit to be valid UTF-8 (`:72-307,338-369`), then admits only ASCII outside strings and comments (`:1349-1356`). Integer literals carry a magnitude and an `exact` flag instead of wrapping (`:818-840`).

**Parser** (`parse/`, 17 files, 11,427 lines). Tokens in, `SyntaxTrees` out (`rep/syntax_trees/syntax_trees.omg:973`). `parse` (`parse/parser.omg:299`) classifies each root item by its first token or contextual word (`:342-475`); expressions go through one shift-reduce parser with explicit stacks, `parse_expression` (`parse/expression.omg:88`).

**Resolver** (`resolve/`, 19 files, 9,187 lines). Syntax tables in, `NameResolution` out (`rep/symbol_resolved_trees/symbol_resolved_trees.omg:293`). `resolve` (`resolve/resolver.omg:26`) runs 22 passes in a fixed order, each stopping on the first rejection; `bind_visible_overloads` must run last because it rewrites `Machine` bindings to `Overloads` (`resolve/overloads.omg:63`).

**Typer** (`type/`, 19 files, 9,203 lines). Resolution in, `Typing` out (`rep/typed_trees/typed_trees.omg:188`). `type_check` (`type/typer.omg:63`) types references, constants, bodies, facts, then clauses. Type identity is the interned row handle (`rep/typed_trees/typed_trees.omg:14-16`).

**Checker** (`check/`, 38 files, 22,631 lines). Typed trees in, `Checks` out (`rep/checks/checks.omg:99`). `checker_check` (`check/checker.omg:208`) derives write frames and completion summaries, then per root machine runs fact-gathering walks (no rejections) to a fixed point of arrival facts, one checking walk, and an ownership walk (`check/checker.omg:346-457`; record 088 "Five things" 3). Proofs are zone queries (`check/zone.omg`, `check/evaluation.omg:615-647`). The checker records at most one diagnostic per machine and stops after 64 (`rep/checks/checks.omg:146-166`, `check/checker.omg:218`).

**Scalar lowerer** (`lower/`, 5 files, 2,113 lines). Checked tables in, `ScalarProgram` out (`rep/lowered_program/scalar.omg:67`). `scalar_lower` (`lower/scalar.omg:184`) starts at status `Unchecked`, needs all four upstream `accepted` flags (`:193`), checks every machine against a fixed shape filter, refuses recursion (`:338-421`) and only then sets `Complete`.

**Alpha emitter** (`compiler/alpha/emit.omg`, 701 lines). `ScalarProgram` in, `AlphaEmitter` tape bytes out. `alpha_emit` (`:382`) runs only when `program.status == ScalarStatus::Complete` (`:388-391`) and writes in two passes so forward branches need no patch table (`:15-19`).

**Driver** (`compiler/driver/`, 220 lines). Units in, `Compilation` out (`compiler.omg:43`). `compilation_add_unit` (`:62`) lexes and parses each unit as it arrives; `compilation_finish` (`:112`) picks the one top-level machine whose name spells `entry_name` in `entry_unit` (`:127-146`), then resolves, types, checks, lowers and emits (`:150-183`). `slot_review.omg` is a wrapper with its own empty `OmegaSlotReview` data whose `review` argument is never read (`:5-14`).

**Request decoder** (`compiler/request/`, 1,266 lines). Staged OCREQ bytes in, `Outcome` out (`outcome.omg:172`). `compile_request` (`protocol.omg:532`) checks the envelope (`:64`) and the field/tag structure (`:366`) and then always ends at `coverage_request_semantics` (`:526-528`); it never returns a verdict. It depends only on `wire`.

**`application/construction`** (799 lines of `.omg`). A source-set packet on stdin in; Alpha tape on stdout and an exit status out. `entry_start` (`main.omg:562`) reads the packet (`read_input` `:65`), adds packages, edges and units to the driver, and `publish` (`:534`) writes the tape. Exit codes: 252 capacity, 251 lexical, 250 parsing, 248 resolution, 249 typing, 247 checking, 246 otherwise, 253 malformed (`:520-532`). A version-2 packet always sets `has_build_sources` (`main.omg:87-89`), and `check_build_input` then exits 246 or 253 before compiling (`build_sources.omg:184-196`).

**`application/ocreq`** (85 lines). Stdin bytes in; outcome frame on stdout and the outcome tag as exit code out. `entry_start` (`main.omg:16`) stages bytes with `request_stage`, calls `compile_request`, writes the frame (`write_frame` `:45`). A read error exits 74 (`:24`).

**`core`** (85 files, 13,441 lines). Declarations only; no entry function. Mostly contracts, proof libraries (`nat.omg`, `rat.omg`, `cauchy.omg`, `seq.omg`) and bodyless rows the compiler catalog supplies (`float_operations.omg:24-28`, `indexing/indexing.omg:1`, `byte_observation/byte_observation.omg:1-3`). Running code lives in `CompactBinary/compact_binary.omg` (984), `sha256/sha256.omg` (462), `math.omg`, `linear_algebra/`, `numeric_conversion.omg` (1,450), `float/float.omg`, `float_accuracy/`, `complex/`, `matrix_layout.omg` and `reflect/reflect.omg`.

**`std`** (108 files, 17,530 lines). Boundary traits and their per-target providers. `build.omg` (833 lines) selects providers and binds raw requirements to syscall numbers or DLL symbols per target (`build.omg:4-833`). The running code is `console.omg`, `posix_filesystem_adapter.omg` (1,858), the three POSIX target adapters (34 machines each), the Windows provider closure (about 45 files under `win/`), `time.omg` (1,068), the clock and time-host files, `macos_gui.omg`, and the per-target entry calling policies.

**`alloc`** (346 lines). `Bump` over an `Extent in Granted & Vacant` (`bump/bump.omg:12-23`) and a `u32` `Vec` over caller backing (`vec/vec.omg:91`). The only run-time branch is `try_allocate` (`bump.omg:42`).

**`blocking-executor`** (308 lines). Contracts only: `BoundedQueue`, `Submission`, `Ticket`, `Executor`, traits `WorkerProvider`, `BlockingExecutor` (`executor.omg:24-188`, `queue.omg:19-27`). No machine bodies.

## 2. How it decides

**By identity (handle, row or table index).**
- Typer: two types are equal exactly when their interned handles are (`rep/typed_trees/typed_trees.omg:14-16`); interning is a linear scan comparing every field (`:735-749`). Admission tests handle equality first (`type/admission.omg:57`).
- Checker: record 088 saw no decision keyed on a name string anywhere in its 36 files (read, not grepped). Place identity is the triple (kind, handle, parent) (`check/places.omg:336`); ownership place keys are `statement*4+1`, `parameter*4+2`, `field*4+3` (`check/owned_places.omg:237-239`). A service is permitted by its `u16` trait identity (`check/callable_effects.omg:272,303-308`). The invocation graph records each expression slot once with an owner and target and goes incomplete on a mismatch (`check/invocations.omg:129-150`). A returned-borrow slot origin is the first reference parameter whose referent handle equals the returned referent's (`check/slots.omg:127,167-169`). `checks_reject` keys on the per-machine `machine_accepted` flag (`rep/checks/checks.omg:146-166`).
- Resolver: an arm pattern's case is the data declaration's identity (`type/statements.omg:591-607`); overload families link by machine handle (`rep/symbol_resolved_trees/symbol_resolved_trees.omg:128,345-362`).
- Zone: keyed on dense subject index, never on a name (`check/zone.omg:166,196,353`).

**By place (value placement in the checked program).**
- Write frames: a write's root is walked through `Field`, `Index`, borrow `Unary`, `Name`; the first field met is the written field, otherwise the parameter is written whole (`check/frames.omg:326-331,410-412`).
- Holds and loans: a conflict is an overlapping path where either side is exclusive (`check/holds.omg:502-506`, `check/loans.omg:953-972`). Overlap compares root, depth, per-step field and "alike" indexes; indexes are alike unless both are different integer literals (`check/owned_places.omg:428-444`).
- Snapshots: a scalar term is captured iff its place lies under the written place (`check/snapshots.omg:41-68`).
- Domains: a place root is "established" only for kinds Parameter, Binder, Call, View, Construction, Snapshot, or an initialized Local (`check/domains.omg:187-196`).

**By value (numbers, intervals, bytes).**
- Checker arithmetic: policy from `row.domain` (Trapping, Wrapping, Saturating, else Exact) and fit from the interval (`check/arithmetic.omg:207-226`). Proofs combine zone relations and operand intervals (`check/evaluation.omg:615-647`).
- Lexer: whole-unit UTF-8 rules, ASCII-only syntax, escape set, integer exactness (`lex/lexer.omg:72-369,818-888,1269-1313`).
- Request decoder: eight identity bytes by position, length words with the high bit clear, tag ranges per field (`compiler/request/protocol.omg:80-155,290-309`).
- `CompactBinary`: canonical LEB128 (no zero final byte except the first, at most 10 groups, payload `<= 1` at shift 63), era word 0, ascending stable numbers, UTF-8 ranges (`core/CompactBinary/compact_binary.omg:77-89,499,551,596,381-414`).
- `std`: errno numbers (`posix_filesystem_adapter.omg:603,797,927`), per-target errno lists with everything else `Other` (`targets/linux_x86_64/filesystem_impl.omg:150-163`, `targets/macos_arm64/filesystem_impl.omg:109-124`), Win32 codes (`win/filesystem/errors.omg:11-35`), attribute masks (`win/filesystem/directory.omg:60-61`), reparse tags (`win/filesystem/metadata.omg:29-35`), syscall numbers in `std/build.omg:385-495`.
- `time.omg`: wrapped-compare overflow (`sum < a`) and biased signed comparison (`std/time.omg:22-24,772-781`).

**By position (order, count, word offset).**
- Resolver: machine owners by table range (`resolve/containers.omg:133`), type-reference owners by one advancing cursor over contiguous runs (`resolve/type_reference_owners.omg:389-463`), a detached expression's scope by `first_row <= index <= root` (`:699`).
- Entry calling policies select the entry form by parameter count: Windows 0 or 2 (`win/entry.omg:28-32`), Linux 1 or 2, macOS 4 or 2 (`std/targets/linux_x86_64/entry.omg:27-31`, `std/targets/macos_arm64/entry.omg:25-29`), UEFI exactly 2 (`std/uefi_x86_64_entry.omg:44-47`).
- Construction packet: version from the first word, then fixed word offsets per row kind (`application/construction/main.omg:71-137`; `build_sources.omg:8-12`).
- Match events: index `2*arm` and `2*arm+1` (`check/matches.omg:177-190`).

**By spelling (source bytes of a name).** Counts are of places cited.
- Resolver and typer: 97 lines in 21 files call `spans_equal` or `span_spells`, 83 of them in `resolve/` (record 087, grepped). Spelled vocabulary: `self`, `result`, `size_of`, `align_of`, `valid_utf8`, `no_nul`, `ascii_only`, `non_empty` (`resolve/names.omg:820,828,1265-1295`); 21 builtin type names (`resolve/type_names.omg:589-688`), plus any `i`/`u` followed by digits counted as taken (`:554-573`); four compiler-known domains `Wrapping`, `Saturating`, `Trapping`, `Finite` (`resolve/domains.omg:136-172`); ranking views `Nat`, `Slice`, `Descending`, `BoundedDistance`, `IncreasingTo`, `Length` (`resolve/clauses.omg:469-474`); the import mount `omega::language::core` (`resolve/imports.omg:189-192`). Pattern-binder marking is by spelling, so a same-spelled `let` inside the arm is also marked (inferred, `resolve/pattern_binders.omg:143-150`).
- Typer: construction and pattern fields (`type/calls.omg:896,928,952`, `type/patterns.omg:44`), `.length` (`type/expressions.omg:508`), record and payload fields (`:558,573`), names in `same_tree` for guard coverage (`type/coverage.omg:640,668,686`), measure owner (`type/clauses.omg:465`).
- Lexer: 30 keywords by a string-compare chain (`lex/lexer.omg:445-476`). Parser: contextual words via `at_word` (`parse/input.omg:337`), for example `boundary`, `trait`, `domain`, `const`, `measure` (`parse/parser.omg:376-475`), `suspend`, `block`, `move`, `in` (`parse/expression.omg:141-148,496-500`).
- Driver: the entry machine by spelled name (`compiler/driver/compiler.omg:127-129`).
- Construction: package directory names by an ASCII rule plus a refusal list (`self`, `bool`, `addr`, `f32`, ..., `AtomicU64`) (`application/construction/main.omg:313,337-341`); Build files only at the literal paths `build.omg` and `defaults/build.omg` (`build_sources.omg:172-173`).
- `std/build.omg` keys on target identifiers and trait/member names (`:280` in record 085); `core/float_provider.omg` matches rows to requirements by member name plus result domain (`:374-377`).
- Source bytes are the identity: "Identifier spellings are their source bytes under LEXICAL-PROFILE-V1" (`rep/source_files/source.omg:473-475`).

## 3. The same job in more than one place

Compiler (`S2P/`):
- Unique-or-ambiguous declaration scan, five copies: `resolve/imports.omg:650` (`declared_named`), `resolve/overloads.omg:160` (`machine_named`), `resolve/domains.omg:176,504,609` (records 087 B5, B10).
- Pairwise duplicate-name scan: `resolve/declaration_names.omg:114-149,348-365,391-425`, `resolve/generic_scopes.omg:125-141`, `resolve/machine_signatures.omg:104-134,181-215`, `resolve/domains.omg:100-126`, `resolve/identities.omg:202-235`.
- Spelling chains for builtin names: `resolve/type_names.omg:586-699`, `resolve/domains.omg:136-172`, `resolve/clauses.omg:466-516`.
- Type-run comparison: `resolve/machine_signatures.omg:388-560` and `resolve/domains.omg:252-414`.
- Lexical ancestor walks: `resolve/containers.omg:354-421`, `resolve/names.omg:1118,1573,1592`, `resolve/generic_scopes.omg:342,373`.
- `NameScope` literals: `resolve/names.omg:159-173,280-294,331-345,348-363` and `resolve/facts.omg:121-174`.
- Stage-then-commit scratch copies, six: `parse/types.omg:818,1049`, `parse/expression.omg:2186,2309,2363`, `parse/clause.omg:1119`.
- Token-set lookahead scan: `parse/input.omg:518-596` and `parse/expression.omg:2459-2521`.
- "Is a fixed integer": `S2P/foundation/integers.omg:191` (`is_carrier`), `type/references.omg:423`, `type/expressions.omg:832,1608`, `lower/expressions.omg:577`.
- Escape and UTF-8 decoding: `lex/lexer.omg:72-307` and `type/literals.omg:74-362`; UTF-8 again in `win/windows_filesystem_paths.omg:17-169`.
- Transition-arm walkers, four: `check/flow.omg:1231`, `check/loans.omg:105`, `check/demand.omg:325`, `check/ownership.omg:316`.
- Clause-run scans: `check/contracts.omg:80` (`declares`) and `check/callable_effects.omg:311` (`callable_has_clause`), plus `check/termination.omg:99,636`; clause-run iterators `check/contracts.omg:281,337` and `check/domains.omg:216`.
- "This use is exclusive": `check/frames.omg:479`, `check/holds.omg:523`, `check/loans.omg:1362`; held place: `check/holds.omg:354` and `check/loans.omg:708`.
- Root of a place expression: `check/frames.omg:309`, `check/owned_places.omg:160`, `check/references.omg:41` (and `:225`, near-duplicate unplaced walks `:54-66` vs `:238-251`); ancestor tests `check/places.omg:545` vs `check/reference_origins.omg:188`; path from members `check/demand.omg:97` vs `check/arrivals.omg:202`.
- Match machinery twice: `check/matches.omg` and `check/ownership.omg:1283-1436`.
- Frame push/pop evaluate-tree pattern: `check/flow.omg:182-206,235-259,1040-1050`, `check/termination.omg:345-374,399-437`.
- Graph walks over `InvocationGraph`: `check/invocations.omg:214,541`, `check/completions.omg`, `check/callable_effects.omg:96-207` (the last reads graph fields directly, `:52,111,219`).
- Value per node: `check/obligations.omg:40` and `check/fact_values.omg:58` (stated intentional, `:24-29`); carrier-range locals `check/arithmetic.omg:118,197,251`, `check/obligations.omg:623`, `check/fact_values.omg:199`.
- Byte-compare loops: `rep/source_files/source.omg:476,510,666`; guarded reads `:441-466,554-584,700-715`; the guarded-accessor pattern about 100 times across `rep/` (record 089 "Five things" 1).

Libraries:
- 48 float-to-integer rows three times: `core/float_operations.omg:703-957`, `core/float_provider.omg:57-168,370-545`.
- `is_finite`: `core/float/float.omg:211,285`, `core/math.omg:486`, `core/linear_algebra/linear_algebra.omg:21`, `core/float_operations.omg:978,981` (grepped `machine is_finite`: 4 matches in 3 files).
- `Sign` twice with opposite case order: `core/float/float.omg:6-9`, `core/float_meaning.omg:13-16`.
- Slice index and range: `core/indexing/indexing.omg:3,10`, `core/slice.omg:6,11`, `core/slice_views/slice_views.omg:1,9`.
- `valid_utf8` twice in core: `core/CompactBinary/compact_binary.omg:360` (with body) and `core/byte_observation/byte_observation.omg:4` (bodyless).
- f32 and f64 halves line for line: `core/float_accuracy/float_accuracy.omg:77-105` vs `:135-163`, `:170-210` vs `:217-257`; `core/complex/complex.omg:55-113` vs `:115-173`.
- `Vacant::content` and `Granted::content` identical (`core/extent.omg:47,275`); three x86 control wrappers (`core/code_segment.omg`, `core/data_segment.omg`, `core/task_register.omg`).
- Six `Console` providers (`std/console.omg:120,141,161,182,203,224`); four byte-view `plan` machines (`std/posix_byte_view_calling.omg:6-58`).
- Entry plans four times with clobber tables typed out (`std/targets/{linux_x86_64,linux_arm64,macos_arm64,macos_x86_64}/entry.omg`); extent-shape test twice (`std/entry_shapes.omg:18`, `std/targets/macos_x86_64/entry.omg:216-251`, grepped 10 occurrences in 6 files); Windows x64 clobbers in `win/calling.omg:145-158`, `win/entry.omg:123-149`, and a 7-register version in `std/uefi_x86_64_entry.omg:114-121`.
- 34 POSIX adapter machines per target, three targets; three stat decoders; Linux errno list twice (`targets/linux_x86_64/filesystem_impl.omg:150-163` = `targets/linux_arm64/filesystem_impl.omg:164-178`).
- Seven sleep bodies, differing in retry: `std/linux_x86_64_clock.omg:8`, `std/linux_arm64_clock.omg:9`, `std/macos_arm64_clock.omg:8`, `std/linux_x86_64_time_host.omg:78`, `std/linux_arm64_time_host.omg:74`, `std/macos_arm64_time_host.omg:36`, `std/macos_gui.omg:626`; plus the Windows `Sleep` split twice (`std/windows_x86_64_clock.omg:10-23`, `std/windows_x86_64_time_host.omg:64-83`). Five poll requirement traits bound to the same syscall or symbol (`std/build.omg:111-118,153-168,782-786`).
- Windows wide-path preamble: 17 `windows_wide_path(` matches in 7 files, one the definition (`win/windows_filesystem_paths.omg:17`; checked, section 10). Directory record decoding twice (`win/windows_filesystem_directory.omg:104` and `win/windows_filesystem_directory_entry.omg:98`). The 17-service `reaches` list repeated in `win/windows_gui_impl.omg` and `std/windows_gui_host.omg`; 22 reach lists in `std/macos_gui.omg`.
- Path component rules twice in construction: `build_sources.omg:54-83` and `main.omg:346-421`; directory-name rule is a copy of the Rust source-files reader (suspected, `main.omg:294`).

## 4. Fixed lists of shapes

Compiler:
- Lowerer, per machine (every machine, not only those reachable from the entry): `has_body`, no generics, not boundary, no target, no owner, container `Root`, no contracts, no measure components, and clauses only `CrashesNone`, `Reaches`, `Crashes Trap` (`lower/scalar.omg:152-180,272-274`). Entry: `parent == 0`, no parameters, result Unit or unqualified `U8` (`:131-149`). Roots: Use, Machine, plain Data, and boundary non-refinement Traits with nothing in them (`:234-257`).
- Lowerer layouts: Unit, Bool, U8/U16/U32/U64, static-extent arrays of non-references, references to non-references, plain records; signed integers and floats are `Unsupported` (`lower/layout.omg:97-126,194-196`).
- Lowerer statements: Let, Assign, Expression, Discard, Tail, Crash (`Trap` only), Transition (`lower/statements.omg:90-99,189`); arm patterns Binder, Wildcard, Guard, Literal (`:284-289`).
- Lowerer expressions: Integer, Boolean, Name, Field, Index, Unary (`Not` and three borrows), Cast, Binary, Call, Array, Construct (`lower/expressions.omg:89-101,171-173`); unsigned arithmetic only; `Divide`/`Modulo` refused for U64 ("Alpha division is signed", `:691-697`); `BitAnd` only against masks 0xFF, 0xFFFF, 0xFFFF_FFFF (`:585-599`).
- Emitter: the operation kinds at `compiler/alpha/emit.omg:589-607,684-693`; other kinds `crash Trap`. Load/store widths 1, 2, 4, 8 (`:232-238,277-283`).
- Scalar operation set: 24 kinds (`rep/lowered_program/scalar.omg:9-36`).
- Lexer: 30 keywords, 28 punctuation forms, escapes `\" \\ \n \r \t \0 \xNN` (`lex/lexer.omg:445-476,1125-1313`).
- Parser operator tokens: `+ - * / % == != < <= > >= [] [..]` (`parse/machine.omg:492-527`); resolver binds only nine of them, Add through GreaterEqual (`resolve/forms.omg:282-294`); typer `operator_token` the same nine (`type/operators.omg:249-261`).
- Clause admission by owner (`parse/clause.omg:867-909`); crash causes `Trap`, `Abort`, `none` (`:616-642`); callable-contract clauses in the resolver: Requires, Ensures, Crashes, CrashesNone, Suspends, Blocks (`resolve/forms.omg:258-268`).
- Paths: machine and value paths of one or two members only ("A longer path crosses a package", `resolve/containers.omg:235-243`, `resolve/names.omg:716-719`, `resolve/domains.omg:474-482`, `resolve/type_names.omg:439`).
- Ranking views: `Measure`, `SliceLength`, `BoundedDistance`, `IncreasingTo`, `Descending` (`check/ranking.omg:78-85`); default view by first argument (`check/termination.omg:258-282`).
- Checker arithmetic: Add, Subtract, Multiply, Divide, Modulo, BitAnd, ShiftLeft, ShiftRight; others get the carrier range (`check/arithmetic.omg:33-43`).
- Request decoder tag ranges and minimum row widths (`compiler/request/protocol.omg` per record 086 B8); the admission tag range is empty, so any admission row is refused (`:489-490`).
- Construction: package role word `<= 2`; unit rows of 3 or 5 words (`application/construction/main.omg:235,427-430`).
- Driver statuses: ten `CompilationStatus` cases (`compiler/driver/compiler.omg:30`).

Libraries:
- Integer helpers over exactly u8..u64, i8..i64 (`core/integer/integer.omg:8-29`); float helpers over f32/f64 only (`core/float/float.omg:50`); `matrix_layout` element `i32` only (`core/matrix_layout.omg:25,138-164`); `CompactBinary` scalars the nine listed kinds (`compact_binary.omg:126-127,459-460`); `alloc` `Vec` element `u32` only (`alloc/vec/vec.omg:30-36`).
- Layout capacities: 32 schema fields and cases, 64 plan entries (`core/layout.omg:52-53,76-77,140`); `CAbiPolicy` refuses numbered fields and `Default` layout sources (`core/c_layout.omg:52,57-67`); access plans 32 fields/readers/writers (`core/access/access.omg:38-105`).
- Calling policies: Windows x64 up to four integer or sized-reference parameters, no callbacks (`win/calling.omg:19-38`); Darwin arm64 up to 32 (`std/darwin_calling.omg:15,23-25`); byte-view policies up to six (`std/posix_byte_view_calling.omg:61-62`).
- Entry shapes: exactly the physical or program-storage forms listed in section 2 (quoted rejection texts at `std/targets/linux_x86_64/entry.omg:229`, `win/entry.omg:167`, `std/uefi_x86_64_entry.omg:225`).
- Units: only meters+meters, m/s, km/h combinations (`core/units.omg:6-8`).
- Windows open options: five-way disposition table (`win/filesystem/handles.omg:59-76`); symlink and junction reparse tags only (`win/filesystem/metadata.omg:15-35`).
- `std/build.omg` per-target rows: `Clock` and the filesystem service are not selected for `macos_x86_64` while console, time and exit are (`:86-120,148,682,831`; record 085). In `application/ocreq/build.omg` a Console provider is selected for `macos_x86_64` but no entry is bound there, and an `alpha_bootstrap` entry is bound with no Console selected for it (`:6-15`; read for this digest).

## 5. Accepts when it does not know

Class words: **stated** (the site or header says so), **traced** (a reader followed it and found it safe or bounded), **open** (no reason found, not traced).

Compiler:
- `check/facts.omg:264-289,333-412`: `push_assumption` does nothing when 128 entries are pending, and `proves` returns `proved()` when the stack empties, so a goal that could not be pushed is never checked. Open (record 088). Section 10 confirms the code path; whether any program reaches 128 pending goals is not traced.
- `check/loans.omg:1138-1155,1186-1194,1270-1275`: locals beyond the 128th are not recorded; `last_use` returns 0, so their loans expire at once and a later conflicting access may be accepted. Stated for the limit, open for the consequence.
- `check/owned_places.omg:196-230`: a place chain deeper than 64 steps returns root 0, which `path_relation` reads as `Apart` (`:518-520`) and loan/move checks read as no conflict. Open (parser nesting bound not traced).
- `check/evaluation.omg:624,637,647` and `check/zone.omg:223-226`: proofs return true on an infeasible zone. Stated ("holds wherever the zone does"); wrong only if infeasibility is set wrongly (sites `check/facts.omg:81`, `check/flow.omg:982,986,1330`, `check/matches.omg:462`).
- `check/flow.omg:1330`: the path after the last transition arm is marked infeasible because typing proved coverage. Stated; depends on `type/coverage.omg`.
- `type/coverage.omg:467-468,640`: `Index` and `Binary` guards count as pure unless they selected a machine; same-spelled names in two guards compare as the same variable. Open.
- `check/holds.omg:61`: `check_holds` runs only for `ExpressionKind::Call`; a `Binary` node that typing resolved to a machine call is not checked for overlapping arguments. Open (confirmed, section 10).
- `check/supply.omg:180-183`: a recorded supply call whose callee is not `Apply` is not refined. Open. `:246-248`: a binder without a contract accepts any argument. Stated (`check/contracts.omg:36-37`).
- `check/frames.omg:206-210`: a non-`Machine` call with only shared operands marks no writes. Open (shared writes to cells not modelled).
- `check/obligations.omg:547`: constructions through bindings other than `Data`/`DataCase` owe no domain proof. Open.
- `check/ownership.omg:1007-1010`: a dropped linear temporary of a kind other than Call, Construct, Match, Array is not rejected. Open.
- `check/arithmetic.omg:376`: `checker_width` answers 64 for any builtin outside six names. Open in record 088; partly answered, section 10.
- `check/arithmetic.omg:211`: measure bodies skip fit checks. Stated (`check/checker.omg:149-152`).
- `check/zone.omg:461-462` (`zone_widen` returns early on an infeasible zone) and `:268,289` (`constrain` closes only rows below `count` though it writes any index below 64). Open.
- `check/recasts.omg:60-61`: any non-Bool to non-Bool recast is accepted without comparing widths. Stated (`:21-22`, typing admitted one width).
- `check/slot_review.omg:65-68`: admitted when there are no origins even if qualifications exist. Stated. `:23,50`: rows past 64 not examined. Open.
- `check/aliases.omg:268-270`: a join with a dead predecessor keeps the current table. Open (record 087 D).
- `rep/source_files/source.omg:468-502`: out-of-range bytes read NUL, so two equal-length out-of-range spans compare equal in `spans_equal`. Open (confirmed, section 10). `package_row`, `unit_package`, `path_length` return defaults for missing rows (`:599-637`). Open.
- `rep/symbol_resolved_trees/symbol_resolved_trees.omg:609,627,646`: `*_binding_at` read any row below capacity, and `name_resolution_clear` (`:381-392`) does not zero the arrays, so an unbound row can read stale data. Open. `ranking_view` and four others return a legal default out of range (`:459-461,682-684,719-721,755-757,791-794`). Open. `operator_home` bounds by `machine_owner_count` (`:517-524`). Open.
- `rep/typed_trees/typed_trees.omg:785-787`: `type_table_intern` returns 0 (the absent type) when full. Open.
- `rep/lowered_program/scalar.omg:62`: `ScalarStatus::Unchecked` consumers. Open in record 089; answered by record 086 (section 9).
- `lower/statements.omg:285`: a `Binder` arm emits no operation that binds its name. Open.
- `lower/expressions.omg:316,359-361` and `compiler/alpha/emit.omg:684-693`: no runtime bounds, overflow or narrowing checks; relies on the checker. Inferred (record 086). Record 088 shows the checker owes index bounds (`check/obligations.omg:169-172`) and conversion ranges (`check/arithmetic.omg` `ConversionOutOfRange`); the end-to-end handoff was not traced.
- `type/joins.omg:135`: shape 0 joins with every value, including type 0. Open. `type/expressions.omg:535-536,1973-1988`: payload field lookup does not check the active case. Stated (delegated to "the Rust producer").
- `type/statements.omg:129`: a `crash` statement is not checked by the typer. Open (parser restricts the cause). `:576`: float subjects accept literal patterns. Open.
- Record 087 A1(b) listed `require_body_completion` (`type/statements.omg:81`) as fail-open for non-Unit bodies. **Wrong, removed** (section 10): the check runs only for non-Unit results and rejects `TerminalResultMismatch` when no terminal statement exists.
- `resolve/facts.omg:316-319`: a `MatchedSubject` name in a data fact whose role is not `WholeSubjectBinder` is skipped. Open. `resolve/type_names.omg:130`: a literal type row binds as a Type unchecked. Open (checked in `type/references.omg:336`).
- `resolve/domains.omg:380-383`, `resolve/machine_signatures.omg:491`: Reference/Array/Slice/Literal rows compare equal by form. Traced (bounded by `same_type_form`); lifetime arguments not compared (inferred).
- `compiler/driver/slot_review.omg:9-14`: ignores its `review` argument. Read (record 086).

Libraries:
- `core/reflect/reflect.omg:128-133,233-236`: a bad handle returns node 0 instead of `Trap`. Open (confirmed, section 10).
- `core/sha256/sha256.omg:116-119,285-288`: a message over 8,192 bytes sets `bad` but still writes a digest from the partial hash. Open. `:85-87`: `sha256_round_k` returns 0 for `i >= 64` (inferred unreachable).
- `core/CompactBinary/compact_binary.omg:516,665-668`: trailing bytes after the last field are not rejected. Open.
- `core/math.omg:418-436`: `sin` above about 6.7e7 returns a wrong finite value; `sin(+inf)` recurses without end (inferred). Stated limit (`:412-415`).
- `core/integer/integer.omg:122-124`: `carrier<T>` body claims "not an integer" if ever run. Stated (never the answer).
- Bodyless compiler-supplied rows accepted by path and shape: `core/float_operations.omg:24-30,55-290`, `core/run.omg:36,79-90,134`, `core/static_interrupt.omg:129,137`, `core/real.omg:6-7,17-21` (two axioms), `core/byte_observation/byte_observation.omg:1-3`. Stated.
- `core/task.omg:63,71`: `ensures true`. Vacuous, nothing accepted by it.
- `alloc/bump/bump.omg:85-95`: `release` does not tie `returned` to the strategy. Stated (checked at `reset`).
- `std/console.omg:364` and `win/console_impl.omg:84-85`: write results ignored, so failed or short writes are not reported. Open.
- `std/filesystem.omg:317`, `std/posix_filesystem_adapter.omg:780-785`, `win/filesystem/metadata.omg:94`: `exists` returns false for every error. Traced (POSIX), open (Windows); `try_exists` keeps errors.
- `std/posix_filesystem_adapter.omg:1215-1221`: `read_dir_count` assumes both dot entries. Open. `:1544-1557`: removes "entry 0" of the parent after draining a child. Inferred correct only for a quiescent directory, open. `:721`: `copy` drops setuid/sticky bits. Read, unstated.
- `std/targets/macos_arm64/posix_filesystem_impl.omg:163-167,198-201`: a negative libc result with `errno == 0` returns 0, read as success. Inferred, open. `:211`: modes narrowed to `u16` by wrapping.
- POSIX target adapters run the raw call with `""` before checking that the path fit, then discard the result (`std/targets/linux_x86_64/posix_filesystem_impl.omg:102-108`). Stated (`std/posix_filesystem_host.omg:205-207`).
- `std/linux_arm64_time_host.omg:86-90`: a persistently failing `nanosleep` recurses without a bound (hang). Read.
- `std/time.omg:913-985,1045-1047`: zero frequency becomes one tick per second, pre-1970 clocks read as the epoch, `elapsed_since` a future start returns zero. Stated ("degrade, never fault").
- `std/macos_gui.omg:238,341-356,406-410`: ignores `AppKit::load()` and `send` results; `msg_translate` always returns 1. Stated for the name case, open otherwise. `win/windows_gui_impl.omg:168-169`: pump ignores translate and dispatch results. Open.
- `std/uefi_x86_64_entry.omg:70,87-97,147,165-187`: any two-parameter UEFI signature yields an accepted plan with whatever sizes it carried; an index under 256 but past `shape_count` reads a default shape. Open.
- `std/targets/uefi_x86_64/tables.omg:42-54`, `std/targets/linux_x86_64/filesystem_impl.omg:29-41`, `std/targets/linux_arm64/filesystem_impl.omg:56-68`, `alloc/vec/vec.omg:44`: hand-written layout plans index `schema.fields[k]` without checking `field_count`. Open (UEFI), stated (Linux, `core/layout.omg:130-137`).
- `std/build.omg`: a requirement with no bind row for a target (the `macos_x86_64` gaps) is not caught here. Open.
- `std/uefi_x86_64_handoff.omg:297-298`: `Ready` depends on leg machines not read by record 086. Open.

## 6. Silent drops

Something is dropped with no row and no diagnostic. Where the effect only loses a fact it is marked "fact loss".
- `check/facts.omg:264-289`: assumption stack full (128). Fact loss for `assume`; an unchecked goal for `proves` (section 5).
- `check/constructions.omg:212-216`: 32 pending construction fields, then the field is dropped. Fact loss, stated (`:185-187`). `:89-90`: payload fields are never written as pending (`_ -> next_field`).
- `check/flow.omg:550,563`: 64 contexts; "An operand without a context simply gains no assumption". Fact loss, stated.
- `check/arrivals.omg:314-317`: 40 arrival paths per state. Fact loss, stated (`:15-16`).
- `check/aliases.omg:165-171`: 16 aliases; a full table keeps none. Fact loss, stated.
- `check/loans.omg:1138-1155`: 128-local use table (section 5).
- `check/slots.omg:209-212`: a full slot table drops an origin; the failure direction is stricter (traced).
- `check/reference_writes.omg:84,217`: queues stop at 256; traced as bounded by the 256-place table.
- `rep/checks/checks.omg:155-158`: beyond 64 diagnostics the record is dropped but `accepted` is set false.
- `rep/checks/checks.omg:251-253,304-306`: `push_slot_origin`/`push_slot_qualification` return false when full; caller handling not traced.
- `rep/tokens/tokens.omg:207-209`: `token_stream_push` full sets `last_retained = false` and records nothing; whether the lexer rejects on it is not traced.
- `rep/source_files/source.omg:280-282`: `source_set_add` returns false when full; caller handling not traced.
- `parse/expression.omg:1609-1621`: `emit_expression` stores `literal: 0` in every node, dropping the literal handle set at `:327`; the typer reads `expression.literal` (`type/expressions.omg:187`). Suspected (record 087); section 10 found no other write to the table.
- `parse/expression.omg:161-164`: `move` is read and dropped. Stated (`:151-153`).
- `parse/expression.omg:2069-2112`: three `ExpressionFrame` literals omit `range_start`/`range_inclusive`; whether omitted fields default is unknown (record 087).
- `resolve/clauses.omg:84`: clause kinds other than Reaches, Satisfies, Terminates are skipped here; traced (bound elsewhere or name nothing).
- `resolve/facts.omg:305`: other expression kinds get no binding; traced (they hold no names).
- `lower/statements.omg:285`: `Binder` arm emits no binding (section 5).
- `application/construction/main.omg:224,260`: `add_packages`/`add_edges` stop at bounds; traced (`read_input` already rejects).

## 7. Unused or unwired

Grep counts are the records' unless marked "checked" (section 10).
- `check/zone.omg:220` `implies`: 0 call sites outside `zone.omg`, only comments (checked). `bound_none`, `bound_negative`: 0 external sites (record 089).
- `type/admission.omg:40` `Admission::Qualified`: never produced; 2 hits, both match arms (`:364,478`) (checked).
- `compiler/request/outcome.omg:329` `reject_code_space`: definition only (checked). `reject_code_number` is called only from `malformed_request` (`:185`); 96 of 97 reject codes are never constructed (record 086).
- `type/expressions.omg:118` `lesser`: definition only (checked).
- `compiler/driver/slot_review.omg`: `OmegaSlotReview` 3 hits, all in that file; 0 in `source-rust/` (record 086).
- `std/windows_filesystem_host.omg:151,154,159,187`: host members `set_file_pointer`, `get_file_time`, `set_file_attributes`, `move_file` have 0 `WindowsFilesystemHost::<name>` call sites (record 086). `win/time_host_impl.omg:36` imports `narrow_i64_to_u64_saturating` without using it.
- `alloc`: `Bump::open`, `try_allocate`, `check_remaining`, `check_extent_length`, `Vec::grow` appear only at their definitions; no `build.omg` names `library/alloc` except the root member list `build.omg:3` (record 084; checked).
- `blocking-executor`: package name appears once (its own `build.omg:6`); no dependent; not a root member (record 084, crate map).
- `core/sha256/sha256.omg`: 63 matches for the type, all inside the file; no `.omg` consumer (record 085).
- Library code used only by `samples/`: `core/matrix_layout.omg` (47 occurrences in 4 files), `core/float_accuracy` and `core/complex` (173 in 8 files), `core/linear_algebra` (28 in 5 files), `std/macos_gui.omg` window machines (36 occurrences, 28 under `samples/gui/*`) and its clock machines (12) (record 085).
- `core/layout.omg:327-333` `IssuedBy`/`IssuedByAny` and `core/extent.omg:297` `ExtentSlot`: not grepped.
- One-caller public machines in the checker (record 088): `binds_machines` (`check/checker.omg:1065`), `callee_parameters` (`check/holds.omg:68`), `shared_parameter` (`:102`), `selected_case` (`check/domains.omg:415`), `invocation_target_same` (`check/invocations.omg:145`), `exclusive_arguments_placed` (`check/obligations.omg:467`), `snapshot_rank` (`check/ranking.omg:275`), `natural_rank_value` (`:458`), `check_recast` (`check/obligations.omg:348`), `publish_reference_backings` (`:436`), `through_write_only` (`check/ownership.omg:935`), `completed_numeric_result` (`check/obligations.omg:505`); the terminal-call roster has one reader (`check/holds.omg:69`).
- Single external callers: `alpha_emit` (`compiler/driver/compiler.omg:182`), `compilation_add_unit` and `compilation_finish` (`application/construction/main.omg:513,574`), `tokenize` (`compiler/driver/compiler.omg:83`); none in `source-rust/` or `tools/` (record 086).
- Parsed but refused by the resolver: `let` definitions (`parse/definition.omg:16` vs `resolve/forms.omg:69`), refinement traits and trait `requires proposition` contracts (`parse/traits.omg` vs `resolve/forms.omg:105-125`), `==`, `!=`, `[]`, `[..]` operator tokens (section 4).
- References that no longer resolve: `alloc/main.omg:8`, `core/fixed_vec.omg:20,34`, `core/math.omg:493` cite deleted `tests/omega/pass/...`; `alloc/build.omg:2` cites `BUMP-ALLOCATOR-CANARY` (only inside `TASKS.md:318`); `blocking-executor/build.omg:1`, `main.omg:1` cite `BLOCKEXEC` (no such item); `std/time.omg:2,56,57,678` cite `TASKS_TIME.md` (no such file).
- `std/uefi_x86_64_handoff.omg:31-33,100-103`: "No package binds these requirements yet"; the termination edges have no catalog entry.

## 8. Stated limits

Compiler scope:
- Lowering: "Recursive owned representations, generic layouts, sums, linear cleanup and unknown qualification remain coverage refusals" (`lower/layout.omg:51-54`); nested reference projection needs a retained dereference path (`:177-180`); recursion refused until tail transfer is lowered (`lower/scalar.omg:338-340`); affine moves unsupported on the copying path (`lower/storage.omg:17-19`); "closed in-process lowering slice" and full publication needs retained identities (`rep/lowered_program/scalar.omg:1,7-9`).
- Emitter: tape at most 2 MiB, frames in [32,64) MiB, 256 MiB initial stack (`compiler/alpha/emit.omg:412-417`).
- Request: semantic phases not implemented (`compiler/request/protocol.omg:359-365`, `outcome.omg:216-218`).
- Construction: Build-source packets must terminate before publication (`application/construction/build_sources.omg:183-184`).
- Lexer: LEXICAL-PROFILE-V1 admits only ASCII syntax (`rep/tokens/tokens.omg:7-9`, `lex/lexer.omg:19-24`).
- Resolver: forms not yet recorded (`resolve/forms.omg:32-53`, `rep/symbol_resolved_trees/symbol_resolved_trees.omg:230-246`); domain predicates and proposition bodies bind no names (`resolve/names.omg:72-73`); multi-domain memberships not bound (`:917-919`); traits through module paths unsupported (`resolve/clauses.omg:37-38`); a sibling's private scope cannot be opened (`resolve/containers.omg:367-368`).
- Typer: generic machine arguments unsupported (`type/binders.omg:34-35`); generic token machines are no candidate (`type/operators.omg:34-35`); requirements with own generics unsupported (`type/clauses.omg:44-47`); named domains and multi-entry casts fail explicitly (`type/expressions.omg:1830-1834`); conformance static arguments and `const` values unsupported (`type/references.omg:37-41,93-95`); joined domains not bound (`type/memberships.omg:23-25`).
- Checker: Exact left shift needs a proof the slice does not make (`check/arithmetic.omg:110-111`); routes of a rejected body are a subset (`check/checker.omg:237-239`); unmatched match events reject as capacity (`check/matches.omg:25-27`); only copyable cast results (`check/ownership.omg:710-713`); predicates beyond 64 fields not proved (`check/domains.omg:400`); conditional and returned references remain unknown (`check/references.omg:30-31`); incomplete graphs are never evidence of acyclicity (`check/invocations.omg:8-9`); "A contradictory set of facts is infeasible ... every query holds there" (`check/zone.omg:13-15`).

Capacities (literal bounds repeated at each use, record 087): 131,072 expressions, 65,536 tokens and type rows, 32,000 members, 16,384 statements, 10,240 machines, 4,096 roots, 2,048 clauses, 1,024 uses, 256 data/traits/domains/generics; source set 512 units, 4 MiB bytes, 64 packages, 256 dependency edges (`rep/source_files/source.omg:164`); checker 256 places, 64 subjects, 128 bodies per root, 64 moved/owed/loans, 8 saved zones, 32,768 edges (`check/checker.omg:44-206`); zone 64 subjects (`check/zone.omg:111-113`); request 65,536 bytes (`compiler/request/request.omg:7`); construction 6,464 row words, 131,072 bytes per unit, 2,097,152 output bytes (`application/construction/main.omg:129-137,502,541`).

Libraries:
- Numeric: `sin` accuracy and large arguments are follow-on (`core/math.omg:412-415`); complex promises no correctly rounded hypot (`core/complex/complex.omg:14-16`); float accuracy is "not a proof of exact real error" (`core/float_accuracy/float_accuracy.omg:8`); wrapping float-to-integer has no declaration (`core/float_operations.omg:698`); truncated Nat subtraction is transitional (`core/nat.omg:443`); `real.omg` is a "temporary axiomatic surface" (`:1`); rationals not structurally normalized (`core/rat.omg:14-17`); heterogeneous Cauchy transitivity not provided (`core/cauchy.omg:242-243`).
- Memory and layout: 32 fields in the pilot slice, proved-fit mutation fail-closed (`core/layout.omg:52-53,98-100`); nested C fields need an explicit child representation (`core/c_layout.omg:1-3`); `Vec` storage mechanics out of scope (`core/vec.omg:11-12`); `FixedVec` bodies blocked on the generic-machine frontier (`core/fixed_vec.omg:61-62`, also `blocking-executor/main.omg:6-8`); hosted programs reach `Granted` only through the device entry (`core/device.omg:97-99`); `alloc` has no capacity ladder or hidden free.
- Platform: x86 control surfaces need x86-64 and CPL0, prose only (`core/code_segment.omg:3-8,21`); Windows "NATIVE EXECUTION IS UNVERIFIED" (`win/time_host_impl.omg:32-34`); UEFI has no Console or ProcessExit provider and is not hosted (`std/time_host.omg:34-36`); Windows calling policy for up to four scalar formals (`win/calling.omg:19-20`); bounded copy, not `CopyFileW` (`win/filesystem/copy.omg:10`); no snapshot during directory enumeration (`win/windows_filesystem_directory.omg:5-6`); removal fails explicitly at 4,096 levels (`win/filesystem_removal/traversal.omg:10-12`); `remove_dir_all` promises no termination (`std/posix_filesystem_adapter.omg:1459-1460`); POSIX path that does not fit becomes `""` (`std/posix_filesystem_host.omg:205-207`); variadic imports unavailable, so Darwin uses `___open` (`std/targets/macos_arm64/posix_filesystem_impl.omg:23-24`); finite console storage does not bound waiting (`std/console.omg:369-370`); interrupted waits are not retried (`std/linux_x86_64_clock.omg:6-7`); Darwin GUI class/style inputs select nothing (`std/macos_gui.omg:8-9`).

## 9. Disagreements between records, and with the crate map

- Crate map §4 says `core/native_execution_authority.omg` is empty (0 lines); record 085 says one line. The file is 44 bytes, one line `public boundary trait NativeCodeExecution {}` with no trailing newline, so `wc -l` prints 0 (checked). Record 085 is right.
- Crate map §2.8 lists `alloc` dependents as the root `build.omg` and sample build files under `samples/cli/facts/resident_backing/*`; record 084 D6 found 0 build files naming `library/alloc`. Only the root `build.omg:3` names it; the resident-backing samples contain `NativeAllocation` and `calloc`, not `alloc` (checked). Record 084 is right.
- Crate map §2.5b cites `application/ocreq/main.omg:14` for `entry_start`; record 084 D4 cites `:16`. It is `:16`; line 14 is a `use` (checked).
- Crate map §2.2 says `SlotReview`, `prepare`, `admits` are re-exported through `compiler/driver/slot_review.omg:1-15`; record 086 B5 says the file defines `OmegaSlotReview` and wrapper machines whose `review` argument is unread. The file defines its own data and two machines that call the pipeline ones (checked); record 086 is right.
- Record 089 A7 says `same_type_form` "does not compare lifetimes or domains (91-92: stated, resolver compares them)"; record 087 B5/B11 infers the resolver does not compare lifetime arguments. The comment at `rep/syntax_trees/syntax_trees.omg:91-92` names "declarations and domains", not lifetimes (checked), so 089's paraphrase overstates it and 087's inference stands unrefuted.
- Record 089 A4 leaves open whether `ScalarStatus::Unchecked` is accepted; record 086 B2 shows `alpha_emit` requires `Complete` (`compiler/alpha/emit.omg:388-391`) and B4 shows the driver maps non-`Complete`, non-`Capacity` lowering to `Unsupported` (`compiler/driver/compiler.omg:175-179`). Together they close the question for the driver path.
- Record 089 A8 leaves open where an `Unknown` integer suffix is rejected; record 086 C21 shows `literal_value` treats it as invalid (`type/literals.omg:27`).
- Record 088 leaves `checker_width`'s `_ -> 64` open for want of `is_carrier`; record 086 C2 lists `is_carrier` as exactly the eight integer builtins (`S2P/foundation/integers.omg:193-201`, checked).
- Record 086 B2/C7 infers that the emitter and lowerer rely on unread checker proofs; record 088 shows the checker owes index bounds (`check/obligations.omg:169-172`) and arithmetic and conversion ranges (`check/arithmetic.omg:207-226`). No record traced one program across the handoff.
- Record 085 says `std/targets/uefi_x86_64/handoff.omg`'s header names `complete_boot_services` and `run` that are not in that file; record 086 A54 finds them in `std/uefi_x86_64_handoff.omg:171,347`. Not a contradiction: they live in the sibling file.
- Record 087 A1(b) says `require_body_completion` runs only for Unit-result bodies; the code does the opposite (section 10). Record 087's "Five things" item 5 and "unsure" list repeat the wrong reading.
- Record 088 says `aliases.omg` and `zone.omg` were outside its unit; records 087 D and 089 A1 read them. Record 087 leaves `alias_table_join`'s dead-predecessor case open and record 088 cites the callers without reading the join.
- Internal code/comment disagreements the records found: `resolve/forms.omg:32-38` says a `requires machine` contract rejects while `:195-219` admits structural ones (checked); `check/qualification.omg:44-46` says byte predicates have no known value while `check/obligations.omg:758-815` evaluates them; `parse/data.omg:22` mentions `[where facts]` that `parse/clause.omg:107-111` rejects; `parse/types.omg:34-39` omits the `write` access accepted at `:168-169`; `check/flow.omg:934-939` has a stale duplicated comment; `S2P/README.md` is titled "Psi product source" while the code produces an Alpha `ScalarProgram` (crate map §4).

## 10. What I checked myself

1. Claim (089 A1.7): `implies` in `check/zone.omg:220` has 0 callers. Code: `git grep -nw implies` over `source/*.omg` outside `zone.omg` finds only comments (`core/wait_wake.omg:19`, `compiler/alpha/emit.omg:415`, `check/checker.omg:752`, `check/flow.omg:36`). **Matched.**
2. Claim (086 C10.5): `Admission::Qualified` is never produced. Code: the only `Admission::Qualified` hits are match arms `type/admission.omg:364,478`; `:419` writes `Adaptation::Qualified`, a different enum. **Matched.**
3. Claim (086 B7.5): `reject_code_space` has only its definition. Code: one hit, `compiler/request/outcome.omg:329`. **Matched.**
4. Claim (086 C16.6): `lesser` has 0 callers. Code: definition at `type/expressions.omg:118`; the other hits are a parameter named `lesser` in `check/facts.omg:116-119`. **Matched.**
5. Claim (086 A26.4, same job): `windows_wide_path(` appears 17 times. Code: 11 in `win/windows_filesystem_control.omg`, one each in five other files and the definition file `win/windows_filesystem_paths.omg`; 17 total. **Matched.**
6. Claim (088 facts.omg.9, open fail-open): `proves` drops goals when the stack is full and reports proved. Code: `push_assumption` goes to an empty `full()` at 128 (`check/facts.omg:268-288`); `both` pushes two goals (`:370-374`); `next` returns `proved()` on an empty stack (`:341-343`). **Matched.** Not traced: parser frames are 32 deep (`parse/parser.omg:30-45`), which may keep right-nested conjunctions below 128 pending goals; inferred only.
7. Claim (089 A5.9a, open): two equal-length out-of-range spans compare equal. Code: `source_set_byte_or_nul` returns 0 for every out-of-range coordinate (`rep/source_files/source.omg:432-470`), and `spans_equal` compares those bytes and returns `same()` (`:476-502`). **Matched.**
8. Claim (087 C17.9b, suspected): `emit_expression` stores `literal: 0`. Code: `literal_node` sets `literal: literal_handle` (`parse/expression.omg:327`); `stored` writes `literal: 0` (`:1620`); this is the only write to `parser.output.expressions[...]` in `S2P/` (`git grep`); the typer reads `literal_value(syntax, expression.literal)` (`type/expressions.omg:187`). **Matched.** Whether this changes a program's result was not run.
9. Claim (087 A1.9b, open fail-open): `require_body_completion` skips non-Unit bodies. Code: `transition item.has_body && typer.output.accepted && result != typer.output.unit { true -> next(...), false -> done() }` (`type/statements.omg:81-84`): the scan runs only for non-Unit results and `missing` rejects `TerminalResultMismatch` (`:112-114`). Unit-result bodies are skipped, which the comment at `:75-77` intends. **Wrong.** Removed from section 5 and noted in section 9.
10. Claim (088 loans.omg.9a): locals past 128 get `last_use` 0 and their loans expire at once. Code: `add` stops at `count < 128` (`check/loans.omg:1140-1145`), `last_use` answers 0 when absent (`:1187-1196`), `loan_live` is `last >= ownership.statement` (`:1270-1274`). **Matched.**
11. Claim (crate map §2.8): `alloc` has dependents under `samples/cli/facts/resident_backing/*`. Code: `git grep -l "library/alloc" -- '*build.omg'` finds only `build.omg`; the resident-backing build files mention `calloc` and `NativeAllocation`. **Wrong.** Corrected in sections 7 and 9.
12. Claim (crate map §4): `core/native_execution_authority.omg` is empty (0 lines). Code: 44 bytes, one line without a trailing newline. **Partly right** (`wc -l` does print 0). Corrected in section 9.
13. Claim (087 B7.6): `resolve/forms.omg`'s header contradicts its code on `requires machine` contracts. Code: header `:32-38` says such a machine "rejects at that form"; `shaped` (`:212-218`) accepts a `Machine` contract with no generics, no nested contracts and admitted clauses. **Matched.**
14. Claim (086 C4.4): the lowerer applies its filter to every machine and refuses `requires` clauses. Code: `next` loops `index < syntax.machine_count` (`lower/scalar.omg:264-268`), `body` sends any non-matching machine to `unsupported` (`:270-276`), `scalar_machine_clauses` returns `no()` for any clause other than `CrashesNone`, `Reaches`, `Crashes Trap` (`:163-175`). **Matched.**
15. Claim (084 D2): every version-2 packet exits before compiling. Code: `version_two` sets `has_build_sources = true` unconditionally (`application/construction/main.omg:87-89`); `check_build_input` exits 246 or reaches `malformed()` (`build_sources.omg:184-196`). **Matched.**
16. Claim (085 reflect.omg, open): a bad handle yields node 0. Code: `select` sends `handle >= count` to `read(graph, 0u64)` (`core/reflect/reflect.omg:128-133`); `members_get.scan` returns `graph_node(graph, 0u64)` when `handles.length < 8` (`:233-236`). **Matched.**
17. Claim (crate map §2.7): 449 build files depend on `std`, 436 in `samples/`. Code: `git grep -l 'source/library/std"' -- '*build.omg'` gives 475, of which 26 are under `_recycling/`; 449 outside it, 436 in `samples/`. **Matched** (the count excludes `_recycling/`).
18. Claim (088 arithmetic.omg.9b, open): whether a carrier other than i64/u64 reaches `checker_width`'s `_ -> 64`. Code: `checker_width` lists six builtins (`check/arithmetic.omg:368-377`); `is_carrier` admits exactly I8..I64, U8..U64 (`S2P/foundation/integers.omg:191-202`). **Partly right**: no other carrier exists in `is_carrier`, but whether every caller gates on it was not traced.
19. Claim (088 holds.omg.9a, open): `check_holds` ignores operator calls. Code: the entry test is `expression.kind == ExpressionKind::Call && expression.count > 1 && machine_accepted` (`check/holds.omg:61`). **Matched.**
20. Claim (crate map §2.5b): `entry_start` at `application/ocreq/main.omg:14`. Code: line 14 is `use compiler_request\request_stage;`; `entry_start` is at `:16`. **Wrong** (citation). Corrected in section 9.
21. Claim (crate map §2.2): `driver/slot_review.omg` re-exports `SlotReview`, `prepare`, `admits`. Code: it defines `public data OmegaSlotReview {}` and its own `prepare`/`admits`, which build a fresh `SlotReview` and call the pipeline machines (`compiler/driver/slot_review.omg:1-14`). **Partly right**: the pipeline machines are reached through it, but not re-exported.

Totals: 15 matched, 3 partly right, 3 wrong.
