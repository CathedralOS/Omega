# Generated source cannot select an undeclared name

The same package as `pass/build/generated_source_selects_authored`, except the
generated machine's later state calls `generated_missing()`, which neither the
authored nor the generated stratum declares. The later stratum resolves
against admitted authored declarations and the other generated units
(`wiki/spec/build/execution.md`, "Generated-source boundary"); a name in
neither still rejects, at its generated `.omega/generated/generated.omg`
location. No successful product is published.
