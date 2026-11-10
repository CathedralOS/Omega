# slice_0141

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A1081 | type_identity.rs:550-555 | REACHABLE | `toolchain-source-owner` parses with a 64-byte leaf but never calls `package_owner`; flat unknown names go to `embedded_name`. |
| A1082 | layout_reports.rs:17-24 | REACHABLE | `valid_alignment` allows non–power-of-two alignments; one `At` on a `Repeated` field skips per-element geometry. |
| A1083 | bignum.rs:298 | REACHABLE | Huge decimal exponent literals build unbounded `pow10` magnitudes; `from_str_radix` does not panic on base>36 (entry panic claim does not match code). |
| A1084 | float_semantics.rs:301-313 | REACHABLE | Directed add of opposite unit magnitudes yields +0 not IEEE −0; non-`BINARY32`/`BINARY64` `FloatFormat` records hit `panic!` in rounding/sqrt. |
| A1085 | lib.rs:419 | REACHABLE | `integer_policy_bridge` wildcard arms omit obligations (e.g. shift-right wrapping); `combine` keeps the left domain; failed float parse becomes +0.0. |
| A1086 | lib.rs:863-868 | REACHABLE | Unresolved proof-call targets are skipped in reachability; `Atom` and bare `StructuralCaseMembership` pass `Proposition::validate` without id checks. |
| A1087 | lib.rs:1247-1252 | REACHABLE | Missing source files make cross-stratum visibility succeed; source-backed names read as empty without a map. |
| A1088 | builtin.rs:663 | REACHABLE | `_ => None` drops service identity for asm intrinsics not listed in the privileged arms, so reach inference stays silent. |

## A1081

**Kind / digest.** ACCEPT, area_15 (preamble read: `digests/area_15.md` lines 1–9). Entry: `toolchain-source-owner` accepted without visitor report; unknown names use `embedded_name` (`type_identity.rs:463-471`).

**Cited site (read).** `omega-rust/psi/foundation/language-semantics/src/type_identity.rs:550-555`: `OwnerForm::ToolchainSource` checks `leaf_byte_atom` length `== 64` then `Ok(())` with no `visitor.package_owner` (`read`). Contrast `OwnerForm::Package` (`528-548`): hex decode, nonzero digest, `visitor.package_owner(digest)` (`read`).

**Enclosing function (read).** `semantic_name` (`463-494`): when `peek_compound::<NameForm>()` is `None` and text is not an opening vocabulary compound, returns `visitor.embedded_name(name)` (`466-470`, `read`). `owner` (`524-560`) called from `nominal` (`500`, `read`).

**Caller search.** Pattern `\bvisit_type_identity_package_owners\b` over `omega-rust`: **28** matches in **4** files (`read`, ripgrep count): `language-semantics/src/tests.rs` (22), `type_identity.rs` (1 def), `typed-trees/src/type_identity.rs` (3), `review-evidence/.../visitor.rs` (2). Production consumers include typed-tree identity emission and review-evidence membership encoding (`read`, not every call body opened).

**Upward trace (read).** `visit_type_identity_package_owners` (`1273-1279`) runs `grammar::type_identity` then `reader.finish()`. Test `generic_identity_reads_const_index_arguments` (`tests.rs:524-532`) embeds valid hex under `toolchain-source-owner` and expects `visit` success while a package-owner counter need not increment (`read`, `513-515` pattern for package owners only).

**Smallest accepting scenario (plain words).** A stored canonical type-identity string whose nominal owner clause is toolchain-source-owner with any 64-character leaf (including non-hexadecimal) completes parsing and does not surface an owner digest to package-owner visitors, so downstream bookkeeping that only learns owners via `package_owner` treats the identity as having no recorded package owner even though the string is syntactically admitted. Separately, a flat name that is not a vocabulary compound is handed to `embedded_name` instead of `MalformedIdentity`.

**Description check.** Digest matches `550-555` and `463-471` at current checkout (`read`); lines match digest commit shape (`git show 457adf4a11f:.../type_identity.rs`, `owner` block `read` via show sample — ToolchainSource arm present).

## A1082

**Kind / digest.** ACCEPT, area_15. Entry: unchecked `stored_bounds` shifts; non–power-of-two alignment; single-placement `Repeated` materialization.

**`stored_bounds` (read).** `layout_reports.rs:17-24`: signed/unsigned bounds via `1i128 << (stored_width - 1)` / `stored_width` with no `stored_width > 0` guard (`read`).

**Caller search `stored_bounds`.** Pattern `\bstored_bounds\b` in `omega-rust`: **5** matches in **4** files (`read`): definitional line plus `materialization.rs:1136`, `value_custody_core.rs:7651`, `layouts.rs:4785`, `6067`.

**Closing checks on width (read).** `materialization.rs:1164-1168` `byte_sized_width` rejects `stored_width_bits == 0` before `stored_value_fits` (`read`). `value_custody_core.rs:7648-7651` requires `stored_width` in `1..=64` (`read`). `layouts.rs:4519` rejects `stored_width == 0` (`read`). Production calls traced to `stored_bounds` go through these guards (`read`).

**Alignment (read).** `layout-plans/src/lib.rs:153-158` `valid_alignment`: errors only on `alignment == 0`; `align_up` (`161-170`) uses `% alignment` without power-of-two test (`read`). `aggregate_geometry` (`136-143`) calls `valid_alignment` per field (`read`).

**Materialization (read).** `materialization.rs:1752-1753`: when `entries.len() == 1` for aggregate `At` placements, sets `(source_chunk_size, required_align) = (expected_size, None)` without entering the `Repeated` branch that checks `element_count` / per-element size (`1755-1774`, `read`).

**Smallest accepting scenario (plain words).** (1) Layout geometry aggregation is asked to pack fields whose byte alignment is an odd integer greater than one; validation passes and offsets are computed with modulo alignment. (2) A layout report describes a fixed repeated aggregate field with outer `Repeated` shape but only one `At` placement entry; materialization treats it as one whole chunk and does not enforce the declared element count or per-element alignment path.

**Verdict note.** Width-zero `stored_bounds` is **closed** on traced production callers; row still **REACHABLE** via alignment and single-placement repeated acceptance.

## A1083

**Kind / digest.** ACCEPT, area_15. Entry: `from_str_radix` panics above base 36; unbounded `pow10` for huge exponents.

**Cited site (read).** `bignum.rs:298-322` `from_str_radix`: non-decimal path uses `character.to_digit(base)?` — invalid digit or base returns `None`, no `panic!` (`read`). Same at digest commit `457adf4a11f` (`read`, `git show` lines 298-322).

**Decimal exponent (read).** `from_decimal_str` (`418-450`): splits `e`/`E` exponent with `exponent.parse::<i64>().ok()?`; positive scale uses `numerator.mul(&pow10(usize::try_from(scale).ok()?))` (`438-440`); `pow10` (`885-897`) loops without magnitude cap (`read`).

**Caller search `from_str_radix`.** Pattern `\bfrom_str_radix\b` in `omega-rust`: **7** matches in `bignum.rs` (definition + tests) plus single uses in `literals.rs`, `const_value.rs`, `published_facts_crash_calls.rs`, hex helpers, etc. (`read`, ripgrep listing — not every caller body opened).

**Smallest accepting scenario (plain words).** A decimal literal text whose mantissa is modest but whose exponent field parses as a large positive integer within `i64` is accepted as a finite rational and forces construction of an enormous power-of-ten big integer during parsing, with no explicit exponent or bit-width cap at this layer.

**Description check.** Entry panic-at-298 is **not** supported by code at current checkout or `457adf4a11f` (`read`); acceptance hole for huge exponents **is** supported.

## A1084

**Kind / digest.** ACCEPT, area_15. Entry: `add_toward_negative(1,-1)` → +0 vs IEEE −0; sqrt paths reach `unreachable!` / panic on bad format.

**Directed add (read).** `float_semantics.rs:325` macro ties `add_toward_negative` to `add` then toward-negative rounding (`read`). `ExactFloat::add` on two finite values: zero result uses `signed_zero` only when **both** operands negative (`bignum.rs:708-712`, `read`); `1 + (-1)` yields ordinary zero (`read`, `inferred` sign). `round_exact_directed` (`301-313`) only handles `format == BINARY32` / `BINARY64`; else `panic!` (`311`, `read`).

**Square root (read).** `square_root_directed` (`619-667`): `BINARY32`/`BINARY64` arms use `correctly_rounded_square_root_bits` with decode closures containing `unreachable!(...)` when positive finite bits do not decode to rational (`648`, `659`, `read`); non-matching `FloatFormat` record hits `panic!` (`665`, `read`). `FloatFormat` is a `compiler_storage::Record` struct (`25-33`, `read`), not an enum — records unequal to `BINARY32`/`BINARY64` are representable in stored artifacts (`read`).

**Caller search.** `add_toward_negative` defined only via macros/catalog in `float_semantics.rs` (`read`); catalog consumed by validation/installation paths (`inferred`, not full enumerate).

**Smallest accepting scenario (plain words).** (1) Host-independent float semantics are asked to add one and negative one with rounding toward negative; exact rational sum is positive zero, so rounded binary32 meaning is +0, not −0 as IEEE negated-add toward negative would prescribe (`inferred` from `709-712` + rounding path, no pinned test for this pair). (2) A persisted float operation record carries a custom `FloatFormat` struct field tuple that is not equal to the canonical BINARY32/BINARY64 constants; square-root or directed-rounding entry panics instead of returning a diagnostic refusal.

## A1085

**Kind / digest.** ACCEPT, area_15. Entry: wildcard policy bridge; `combine` keeps left; `unwrap_or(0.0)` on failed float conversion.

**`integer_policy_bridge` (read).** `numerics/src/lib.rs:401-437`: `formation_conditions` and `trap_predicates` use `_ => &[]` (`419`, `436`, `read`). Example: `(ShiftRight, Wrapping)` falls through to empty formation and empty traps (`read`). `ArithmeticDomain::combine` (`70-75`) returns the left operand when neither side is `Exact` (`read`).

**Literals (read).** `literals.rs:190-201` `value_f64`: when `presented` absent, `FloatSemantics::from_decimal(...).map(...).unwrap_or(0.0)` (`196-201`, `read`); same pattern `value_f32` (`211-216`, `read`).

**Caller search `integer_policy_bridge`.** Pattern `\binteger_policy_bridge\b` in `omega-rust`: **32** matches in **4** files (`read`): `numerics/lib.rs` (17), `arithmetic_domains.rs` (11), `terminal-psi/semantics/scalars.rs` (2), `operation_emission.rs` (2).

**Smallest accepting scenario (plain words).** (1) Validation asks the policy bridge for integer shift-right under a wrapping domain and receives no formation conditions or trap predicates, so checking does not require shift-count or overflow obligations from this table. (2) Two non-exact arithmetic domains are merged and the left one wins even when they disagree. (3) A float literal carrier holds unparsable decimal text; any consumer calling `value_f64`/`value_f32` observes positive zero without an error.

## A1086

**Kind / digest.** ACCEPT, area_15. Entry: default declaration scope; unresolved proof callee; `Atom` / `StructuralCaseMembership` validation gaps.

**Cited `lib.rs:863-868` (read).** Default `DeclarationSelectionAuthority::declaration_selection_scope` returns `None` (`863-868`). `declarations_core.rs:1088-1090` turns missing scope into `Err` when capturing authority (`read`) — not a silent accept path for recorded authority.

**Proof reachability (read).** `proof_applications.rs:451-456`: in `walk_applications`, `called_machine(application.machine)` `None` ⇒ `continue` with no recorded call edge (`read`). Enclosing `reachability` / `invocation_reachability` (`386-430`) feed pending roots then `walk_applications` (`read`).

**Propositions (read).** `proposition.rs:1885` `Self::Atom(_) => {}` in `Proposition::validate` (`1880-1973`, `read`). `1941` `StructuralCaseMembership { .. } => {}` in same function (`read`). Context pass `validate_value_terms` (`2028`) also no-ops on `Atom` (`2028`, `read`); `StructuralCaseMembership` checks `subject.root` in `structural_places` (`2064-2067`, `read`) but not case id membership.

**Caller search `walk_applications`.** Pattern `\bwalk_applications\b` in `omega-rust`: **3** matches, all in `proof_applications.rs` (`read`).

**Smallest accepting scenario (plain words).** (1) Proof-application reachability expansion encounters an application whose machine handle has no resolved callee declaration; the walk skips it, so reachability results omit that edge without marking failure. (2) A proposition serialized as `Atom` with an arbitrary unused id, or `StructuralCaseMembership` checked only via `Proposition::validate` without context, passes structural validation even when the id or case should be rejected.

**Description check.** Lines `863-868` are scope-default, not the reachability site; reachability claim matches `451-456` (`read`).

## A1087

**Kind / digest.** ACCEPT, area_15. Entry: missing files read as visible; span cache keyed on table address; empty name without source map.

**Visibility (read).** `source/src/lib.rs:1247-1252`: if `file_at(reference)` or `file_at(declaration)` is `None`, returns `true` (`1248-1251`, `1265-1269`, `read`) — “missing file” does not hide.

**Cache key (read).** `retained_declaration.rs:37-43`: `SourceSpanCandidates` rebuild key uses `std::ptr::from_ref(symbols) as usize` plus arena revisions/lengths (`read`).

**Names (read).** `symbol.rs:209-211` `SymbolNameStorage::Source`: `sources.text_at(...).unwrap_or("")` (`read`). `SymbolNameRef::Source` `as_str` returns `""` (`250`, `read`).

**Caller search `reference_can_see`.** Pattern `\breference_can_see\b` in `omega-rust`: **39** matches across symbol table, resolution pipeline, validation (`read`, ripgrep count).

**Smallest accepting scenario (plain words).** (1) A reference span points at source identity not present in the map while checking whether it may see a declaration; the predicate answers yes. (2) A symbol name stored as a source span is displayed or compared through `as_str` without a live `SourceMap`, yielding empty text rather than failure.

## A1088

**Kind / digest.** ACCEPT, area_15. Entry: `_ => None` in `asm_intrinsic_service_name` for unlisted privileged asm builtins.

**Cited site (read).** `builtin.rs:606-664` `asm_intrinsic_service_name`: explicit arms for `MachineControl`, `PortIo`, listed fences, `is_float_family`, then `_ => None` (`663`, `read`).

**Consumer (read).** `effect_inference.rs:2971-2982`: for asm intrinsics, service reach extended only when `asm_intrinsic_service_name()` is `Some` (`2972-2981`, `read`). `machine_calls_core.rs:5705` similar lookup (`read`, one line not full function).

**Caller search `asm_intrinsic_service_name`.** Pattern `\basm_intrinsic_service_name\b` in `omega-rust`: **10** matches in **4** files (`read`): `builtin.rs` (7, incl. tests), `machine_calls_core.rs` (1), `effect_inference.rs` (1), `checks_multiplicity_borrowed_windows.rs` (1).

**Smallest accepting scenario (plain words).** A newly added `BuiltinFunction` asm-intrinsic variant is registered in the closed inventory but omitted from the `MachineControl`/`PortIo`/fence match arms; compilation accepts calls to it, yet service-reach aggregation records no boundary service because the helper returns `None`.

## What I did not read

- Full body of every `visit_type_identity_package_owners` call site beyond named files.
- Every `integer_policy_bridge` consumer in `arithmetic_domains.rs` (11 call sites).
- Every `reference_can_see` call site among 39 matches.
- Runtime execution proving `add_toward_negative(1,-1)` bit pattern on a hosted target (reasoned from `bignum.rs:708-712` and rounding entry `301-313`, `inferred`).
- Whether layout producers ever emit non–power-of-two alignments or single-placement repeated fields in shipped artifacts (only materialization/geometry code paths read).

SLICE COMPLETE
