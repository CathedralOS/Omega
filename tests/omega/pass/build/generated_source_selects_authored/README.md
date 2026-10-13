# Generated source selects authored declarations

The build seals `generated.txt` as `generated.omg` and includes it as the
package's later resolution stratum. The generated unit declares a data type
and two machines: `generated_check` constructs the generated `GeneratedPair`,
calls the authored `seed()`, and calls the generated `generated_offset()` both
in its body and from a later state. Generated source may select admitted
authored declarations and other generated declarations
(`wiki/spec/build/execution.md`, "Generated-source boundary"); the authored
`main.omg` names none of the generated declarations, which that rule forbids
(pinned by `fail/build/generated_source_base_reference_refused`).
`fail/build/generated_source_undeclared_call_refused` pins that the later
stratum still rejects a call no stratum declares.

Run `omega --check tests/omega/pass/build/generated_source_selects_authored/main.omg`.
Successful publication contains `files/generated.omg`.
