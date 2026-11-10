# Ambiguities resolved while building the lists

Base commit of this branch: `8febfc42316bdcede60caa33ddd10a641f0dee8b`. Digest citations remain at `457adf4a11f`.
The branch `work/grok-mega-2026-11-10` did not exist on origin. It was created from `origin/main`.

Resolved by the coordinator (read of the digests and the directory trees, not of every cited line):

1. ACCEPT rows are markdown bullets or numbered items under a `###` heading whose text contains "open" (any case). Headings containing "stated", "traced", or "inferred" are skipped. Area 14's heading is `### Open`. Area 09's open heading says one entry was given no class; that entry stays in the row because the digest placed it in the open subsection.
2. One bullet or numbered item is one row, even when it cites several sites. The cited column is the first site, chosen as: a leading bare line number plus the single `.rs` file named by the group header (area 10); else the first `name:line` token; else a bare `:line` prefixed with the only `.rs` name in the entry or group.
3. After the eight priority areas, the remaining areas are in numerical order: 01, 02, 03, 06, 07, 12, 15.
4. A stream A slice has at most 8 rows, one kind, and one digest. Slice numbers follow that area order, ACCEPT first, then DROP, UNUSED, SAMEJOB.
5. Stream B2 includes every `.rs` file under stage 01 `src/`, under `optimization-unit/src/`, and under `optimization-unit/semantics/src/`, plus `literal_fold_rewrites.rs`. The brief names "the semantics crate it calls under optimization-unit"; both the parent crate and `semantics/` are included so a fold in either is on the list.
6. Files over 3,000 lines in stream B are cut on exact line ranges with no overlap.
7. A specification file of 600 lines or fewer is one chapter piece. A longer file is split on headings. Consecutive heading spans are packed up to 600 lines. A span that is still longer is split on the next heading level, then into 600-line parts. The rule-id prefix is the filename stem, with the parent directory prepended when two files share a stem.
8. A stream D family is the first directory under `tests/omega/pass`, `fail`, or `run`. Batches hold 60 programs from that one family. "The same name" for a twin is the final directory component. "Differ only in a numeric literal" means the only textual differences are numeric literals.
9. Stream E units are every git-tracked file under `source/psi`, `source/omega`, `source/library`, and `bootstrap`, in path order, packed to at most 8,000 lines. Files with a NUL byte, extension `exe`/`bin`/`hex`, and the two extensionless alpha binaries are excluded and listed below. Markdown is included.
10. Stream F is every regex match of `requires_unsatisfiable` or `hypotheses_contradictory` under `omega-rust/`, three matches per file. The search command and count are in `grok/F/search.txt`.
11. Stream D2 (lane coverage) is not queued until `grok/C/lanes/lane_NN.md` exists. Stream B's two merged answers are not queued until the per-file notes exist.
12. No worker was running when the cost update arrived. New workers use model `composer-2.5` (not the fast variant).

ACCEPT rows by area: {'08': 84, '09': 91, '10': 93, '11': 56, '13': 84, '14': 185, '04': 70, '05': 120, '01': 47, '02': 39, '03': 35, '06': 47, '07': 61, '12': 52, '15': 51}
DROP/UNUSED/SAMEJOB by area:

DROP: 08=14, 09=15, 10=6, 11=22, 13=43, 14=18, 04=8, 05=0, 01=13, 02=9, 03=8, 06=6, 07=5, 12=6, 15=16

UNUSED: 08=17, 09=15, 10=17, 11=18, 13=14, 14=11, 04=18, 05=39, 01=23, 02=18, 03=27, 06=10, 07=15, 12=8, 15=12

SAMEJOB: 08=28, 09=23, 10=31, 11=56, 13=39, 14=24, 04=34, 05=77, 01=39, 02=55, 03=32, 06=38, 07=61, 12=45, 15=43

Excluded stream E files:
- `bootstrap/0_alpha/alpha_arm64_macos` (binary-extension-or-named-blob)
- `bootstrap/0_alpha/alpha_x64_linux` (binary-extension-or-named-blob)
- `bootstrap/0_alpha/alpha_x64_windows.exe` (binary-extension-or-named-blob)
- `bootstrap/0_alpha/alpha_x64_windows.hex` (binary-extension-or-named-blob)
- `bootstrap/1_beta/beta_compiler_bytecode.tape` (nul-byte)
- `bootstrap/2_gamma/gamma_evaluator_bytecode.tape` (nul-byte)
- `bootstrap/proofs/beta_encoding/definition_package.bin` (binary-extension-or-named-blob)

Bullet and numbered-item counts are the rows. Where a digest's own "N entries" is higher, one bullet joins several sites and stays one row. Area 09's open heading says 124 entries; the open subsection contains 91 bullets, and all 91 are rows. Area 01's open entries are table rows (46) plus the paragraph that says the test harness is classed open. The following paragraph, which the digest marks "Not classed", is not a row.

Missing citations (cited column `-`): 64
