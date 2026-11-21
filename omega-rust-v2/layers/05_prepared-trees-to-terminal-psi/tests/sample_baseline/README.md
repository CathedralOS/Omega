# Sample compatibility through L5

Run the ten named configurations from the V2 workspace root:

```text
mbx test --locked -p omega-lowering --test sample_baseline -- --include-ignored --nocapture
```

The default test runs one local pair. The opt-in comparison reads eight historical configurations from Git revision `36d45157895e1674fcdc01222fdee0faa1b68082` and compares them alongside the two local configurations. It reads only the files named in `cases.rs`. A checkout used for the comparison needs the pinned Git objects. A missing object fails the run.

| Configurations | What the input exercises |
| --- | --- |
| standalone | Linux x86-64 entry binding to an empty machine. Inspect its parameter list and unit return. |
| module-library | A dependency package tested independently. Inspect the constants returned by both public machines from its two source files. |
| cross-width, rational-landing | Linux x86-64 entry binding to `entry_start`. One program compares differently sized integers. The other passes three arithmetic expressions to `identity`. |
| direct-imports | Freestanding three-file package using the historical file imports and a machine attached to a data declaration. A [filename names no namespace component](../../../../../wiki/spec/language/modules.md#directory-namespaces), so L2 must refuse `use m\a` in `m/b.omg`. L2's `an_import_names_a_declaration_not_the_file_that_declares_it` owns that rule. The `-v2` pair below checks the program's intended behavior. |
| direct-imports-v2, missing-import-v2 | The same nested field read with directory-qualified declaration imports and a free machine. The negative removes exactly the import of `A`. L2 owns its refusal. L5 owns the positive's nested field read in Psi. |
| abstract-premise, false-abstract-premise | Linux x86-64 entry binding to `main`. The second program makes the premise return false. Its intended proof refusal belongs to L6. An earlier parse error leaves that question open. |
| import-from-dependency | The freestanding root needs Build to register `kit` as an alias for its dependency and expose the selected module. The source-only runner records this configuration as missing setup. Wire the actual dependency handoff here when available. |

The hosted rows select one declared entry binding. The source stages run target-neutral with the normal toolchain exports and empty external evidence. The harness never executes a Build recipe or runs the program. A dynamic target demand records missing setup. The test groups source files under their package-relative names and follows the production typing and preparation rounds.

Each row in `observations.tsv` records the first diagnostic with its source location, a missing setup input, or a Psi product with an inspected property. An L5 refusal means preparation finished and lowering received its input. Review the before/after row when a result changes. A new Psi result must also satisfy the case's output assertion. The counts describe these ten configurations only.

## Choosing cases from history

Once a configuration produces its required Psi, mark it as required in the case definition. A later refusal then fails even if someone updates the observations file. For a negative case with a known language rule, require the owning layer's diagnostic at its source location. Cases still under investigation record where compilation first stops.

For returned values, follow the return edge to the operation that supplied it. The nested field test checks that the returned scalar came from the constructed B and that the read selects B's field before A's. A field read elsewhere in the machine cannot satisfy that assertion.

Use the retired corpus to investigate a particular rule. For example, the archived shift tests cover both checking a different value and overwriting a value after checking its range. L5 can test which value reaches the shift. L6 can test whether the range proof still applies to that value. Those observations exercise different responsibilities even when the source programs look similar.

Prefer the smallest pair that exposes the distinction through the owning layer's actual input and output. Keep a full historical sample in this comparison when it adds a package interaction or observable behavior the existing cases miss. Adding another placement of the same rule needs a concrete difference in the handoff it tests.
