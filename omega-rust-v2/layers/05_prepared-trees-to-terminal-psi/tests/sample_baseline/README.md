# Sample compatibility through L5

Run the fifteen named configurations from the V2 workspace root:

```text
mbx test --locked -p omega-lowering --test sample_baseline -- --include-ignored --nocapture
```

The default test runs one local pair. The opt-in comparison reads eight historical configurations from Git revision `36d45157895e1674fcdc01222fdee0faa1b68082` and compares them alongside seven migrated configurations. A migrated configuration keeps its historical `build.omg` but replaces the root source files with current-language files from `tests/fixtures`. A dependency's files still come from the pinned history. It reads only the files named in `cases.rs`. A checkout used for the comparison needs the pinned Git objects. A missing object fails the run.

| Configurations | What the input exercises |
| --- | --- |
| standalone | Linux x86-64 entry binding to an empty machine. Inspect its parameter list and unit return. |
| module-library | A dependency package tested independently. Inspect the constants returned by both public machines from its two source files. |
| cross-width, rational-landing | Linux x86-64 entry binding to `entry_start`. Both declare `machine Main::main` under the data declaration `Main`. Named decision `no-data-code-attachment` retires that form, so L2 must refuse it at the owner. |
| cross-width-v2, rational-landing-v2 | The same programs with `Main::main` declared as the free machine `main`; an ignored test checks that nothing else changed. One compares differently sized integers. The other passes three arithmetic expressions to `identity`. Both still stop at L2 because the toolchain core does not yet declare the policy domain `Wrapping` used by their `hits` field. |
| direct-imports | Freestanding three-file package using the historical file imports and a machine attached to a data declaration. A [filename names no namespace component](../../../../../wiki/spec/language/modules.md#directory-namespaces), so L2 must refuse `use m\a` in `m/b.omg`. L2's `an_import_names_a_declaration_not_the_file_that_declares_it` owns that rule. The `-v2` pair below checks the program's intended behavior. |
| direct-imports-v2, missing-import-v2 | The same nested field read with directory-qualified declaration imports and a free machine. The negative removes exactly the import of `A`. L2 owns its refusal. L5 owns the positive's nested field read in Psi. |
| abstract-premise, false-abstract-premise | Linux x86-64 entry binding to `main`. The second program makes the premise return false. Its intended proof refusal belongs to L6. Both stop in L1 at `crash Trap;`, a V1 statement the spec never shows; the parser refuses it on purpose (`statements/reject/crash_statement`). Both also use the `<machine Observe>` binder with `requires machine Observe(...)`, which the spec retires for executable code ([generics](../../../../../wiki/spec/language/generics.md#retired-spellings)). Rewritten with `<const Observe: Observer>`, the program stops in L5 at the premise-bearing closed callback. |
| import-from-dependency | The freestanding root's `build.omg` registers `dep` as a product dependency under the alias `kit`, and `dep/build.omg` names that package `mod_kernel`. The harness supplies the package and the `kit` edge to the production resolver profile. The historical `use kit\other;` treats the file `other.omg` as a module. A file name adds no namespace ([directory namespaces](../../../../../wiki/spec/language/modules.md#directory-namespaces)), so L2 must refuse the import with the edge present, as it refuses `direct-imports`. |
| import-from-dependency-v2, missing-alias-v2, retargeted-alias-v2 | The same configuration with the root file replaced by `fixtures/dependency_import/main.omg`, which imports the declaration `kit\other_v`. With the registered edge, L5 must lower `entry` to a call into `mod_kernel`'s `other_v`. The Psi-only consumer `returned_call` checks that call. The two negatives keep every source byte. One omits the edge. The other points `kit` at the toolchain package. L2 must refuse both at the import. The refusal text cannot tell a missing alias from a missing declaration, so only this twin shows that the historical refusal comes from the file name. |

The hosted rows select one declared entry binding. The source stages run target-neutral with the normal toolchain exports and empty external evidence. The harness never executes a Build recipe or runs the program. It confirms each selected configuration, including a dependency registration and the dependency's package name, in the pinned `build.omg` files. The test groups source files under their package-relative names and follows the production typing and preparation rounds.

Each row in `observations.tsv` records the first diagnostic with its source location or a Psi product with an inspected property. An L5 refusal means preparation finished and lowering received its input. Review the before/after row when a result changes. A new Psi result must also satisfy the case's output assertion. The counts describe these fifteen configurations only.

## Choosing cases from history

Once a configuration produces its required Psi, mark it as required in the case definition. A later refusal then fails even if someone updates the observations file. For a negative case with a known language rule, require the owning layer's diagnostic at its source location. Cases still under investigation record where compilation first stops.

For returned values, follow the return edge to the operation that supplied it. The nested field test checks that the returned scalar came from the constructed B and that the read selects B's field before A's. A field read elsewhere in the machine cannot satisfy that assertion.

Use the retired corpus to investigate a particular rule. For example, the archived shift tests cover both checking a different value and overwriting a value after checking its range. L5 can test which value reaches the shift. L6 can test whether the range proof still applies to that value. Those observations exercise different responsibilities even when the source programs look similar.

Prefer the smallest pair that exposes the distinction through the owning layer's actual input and output. Keep a full historical sample in this comparison when it adds a package interaction or observable behavior the existing cases miss. Adding another placement of the same rule needs a concrete difference in the handoff it tests.
