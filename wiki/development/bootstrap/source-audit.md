# Reading the Gamma and Epsilon bootstrap sources

Three changes made a reviewer’s comparisons more direct. The Epsilon wire entry and Gamma compiler got smaller. Spelling out the Epsilon keywords cost 894 bytes of evaluator source and made all 17 spellings and token codes visible together.

| Change | Before → after | What was checked |
| --- | --- | --- |
| [Epsilon evaluator boundary, #4542](https://github.com/CathedralOS/Omega/pull/4542) | A byte selector begins `(if (eq index 0) 69 …)`. The entry now says `"EEREQ"`, spells the refusal marker `"EEOUT"` and embeds one hexadecimal SHA-256. Entry source fell from 10,563 to 6,359 bytes. | Both entries passed 49 envelope cases, including the first differing byte at every marker and digest position. Seventeen Console cases checked the output prefix, line ending and distinction between a guest byte-range trap and an impossible view element. |
| [Gamma scanner and compiler, #4546](https://github.com/CathedralOS/Omega/pull/4546) | `(eq byte 40)` became `(eq byte '(')`. Syntax words are grouped by length, e.g. `(c_chars2 at 'i' 'f')`. The two source files fell from 33,737 to 31,662 bytes. | Both targets rebuilt themselves byte for byte. The Alpha compiler fell from 230,666 to 212,403 bytes and the Linux x86-64 compiler from 226,861 to 226,310. Both classifiers passed 137 spelling and near-miss inputs. |
| [Epsilon keyword table, #4548](https://github.com/CathedralOS/Omega/pull/4548) | `epsilon_span_is_8 … 98 111 117 110 100 97 114 121` became `(EpsilonKeyword "boundary" 10 …)`. Packed evaluator source grew from 617,122 to 618,016 bytes. Its generated Gamma source grew from 726,582 to 729,482. | The chosen table takes 421 Gamma pairs per ordinary source view and allocates none per lookup. Both Epsilon compilers emitted the same 1,106,536-byte public lexer module. Its 23 cases returned the expected 465 bytes. |

## Which routes agreed

- **Four Gamma host and target pairs.** The #4546 Gamma compilers ran on Alpha and Linux and generated both targets. All four produced the same 726,582-byte Gamma source for the **#4542 Epsilon evaluator**. On either host the Alpha evaluator was 3,798,810 bytes and the Linux evaluator 1,985,896. Each passed five portable observations and two syntax refusals. [Host/target gate](../../../tests/bootstrap/gamma_host_matrix.py)
- **Nine small-program routes.** With the earlier 226,861-byte Linux Gamma compiler fixed, all three Delta choices crossed with all three Epsilon choices produced `00 5a 00 00 00 5a` on the same 339-byte stdin program and one input byte `Z`. The six bytes encode exit 90 and console output `Z`. The reported rows reused the cold Linux host with SHA-256 `249ffad4eba5d86a28427a259f58dd8aecb2b9bb10e2091157c0f81cc0cbdd13`. Those nine reports cover this one program. [Route selector](../../../tests/bootstrap/route.py)
- **One frozen Omega constructor.** On the 873,205-byte #4540 Omega compiler and construction entry, the Epsilon compilers before and after #4548 published the same 18,816,197-byte Delta module observation, SHA-256 `454b89f25d59fe14224d2cf3019acbdb2b390c6780e39dffeff5b5f6e41ec267`. The #4548 measurement ends at module publication. [Construction procedure](../../../tests/bootstrap/independent/WHOLE_REQUEST.md)
- **One linked product and two refusals.** The #4542 evaluator and a separately compiled 58,603,133-byte Linux executable of the #4540 constructor received the same 828-byte, two-package wire decoder input. Their full 1,612-byte observations matched. The 1,607-byte Alpha tape exited 42 on the seed (SHA-256 `f5ac1bf6ddc9158a4164a644c9342f73cc6c6698c93773e32dfe035176e3a371`). Removing the dependency gave guest exit 1 at coordinate 11. Appending one input byte gave guest exit 3 with empty payload. [Product and refusal controls](../../../tests/bootstrap/independent/product_wire.py)

These runs use fixed revisions. The four host/target tests predate the 729,482-byte keyword-table evaluator. The wire tape exercises one Omega product; the complete Omega-in-Omega rebuild is still open.

<details>
<summary>Reproduce the measurements from pinned sources</summary>

Run on Linux x86-64 with Python, Git and a built repository `xtask`. Use fresh output directories. From the repository root, prepare the snapshot with the earlier Gamma compiler, the snapshot with the revised Gamma classifier, and the later Epsilon lexer.

```sh
export OMEGA_XTASK_BIN="$PWD/target/debug/xtask"
audit=$(mktemp -d)
git worktree add --detach "$audit/early" 9063f0819b5
git worktree add --detach "$audit/gamma" 43e334bc2b0
git worktree add --detach "$audit/keywords" 46d91bbc777
```

Run the small stdin control and the four host/target constructions.

```sh
cd "$audit/early"
python3 tests/bootstrap/route.py "$audit/nine" \
  --gamma native --delta all --epsilon all --case stdin --reuse-stages --seconds 360

cd "$audit/gamma"
tests/bootstrap/linux_x86_64/bootstrap --alpha . "$audit/alpha"
tests/bootstrap/linux_x86_64/bootstrap . "$audit/linux"
python3 tests/bootstrap/gamma_host_matrix.py --epsilon \
  "$audit/alpha" "$audit/linux" "$audit/four"
```

For the focused envelope and keyword checks, run the portable evaluator gate against a retained four-host receipt, then the Epsilon compilation controls at the keyword-table revision.

```sh
cd "$audit/gamma"
python3 tests/bootstrap/interpreted/epsilon/portable.py "$audit/envelopes" \
  --profile envelope --receipt "$audit/four/linux-to-linux-epsilon.gamma"
cd "$audit/keywords"
python3 tests/bootstrap/independent/successor_test.py --compile-epsilon
```

For the module comparison, the early snapshot packs the same Omega compiler and entry as #4540. Feed those exact bytes to both Epsilon compilers. The SHA-256 of `early-input/whole.epsilon` is `8bdbb90999c6b62a04754a3d452966b4903af74179fbc113fdc910a628d27401`. The request builder selects the Delta construction profile.

```sh
for rev in early keywords; do
  cd "$audit/$rev"
  python3 tests/bootstrap/independent/whole_request.py "$audit/$rev-input" --entry-control
  tests/bootstrap/linux_x86_64/bootstrap --delta . "$audit/$rev-build" \
    "$audit/$rev-input/compiler.dcreq"
  "$audit/$rev-build/successor" < "$audit/early-input/whole.epsilon" > "$audit/$rev.module"
done
cmp "$audit/early.module" "$audit/keywords.module"
sha256sum "$audit/early.module"
```

For the linked product, build the early module’s executable and replay the same customer on the evaluator from the four-host check. The historical Linux constructor had SHA-256 `ccc6eb867203f3250e9025e8f616d07e8b2253582b34499201ffe9e4ce4cb21b`. Both commands also execute the emitted Alpha tape and check both refusals.

```sh
cd "$audit/early"
python3 tests/bootstrap/independent/whole_request.py "$audit/early-input" --frame-result "$audit/early.module"
"$audit/early-build/program" < "$audit/early-input/omega.dcreq" > "$audit/omega.gamma"
"$audit/early-build/gamma-compiler" < "$audit/omega.gamma" > "$audit/omega"
chmod +x "$audit/omega"
python3 tests/bootstrap/independent/product_wire.py "$audit/compiled-wire" --constructor "$audit/omega"
python3 tests/bootstrap/independent/product_wire.py "$audit/evaluated-wire" \
  --evaluator "$audit/four/linux-to-linux-epsilon.bin"
cmp "$audit/compiled-wire/wire.tape" "$audit/evaluated-wire/wire.tape"
```

</details>
