# closed template methods

Compile-scope probe. No standard-library dependency.

- Sibling `module first;`/`module second;` files each declare
  generic `Envelope<T [copy]>` + `Envelope::read`; main imports
  both and instantiates through the module path:
  `first::Envelope<u64>`, `second::Envelope<u64>`,
  `first::Envelope::read<u64>` — same-name generics stay
  module-scoped.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/closed_template_methods/main.omg
mbx run -p omega -- update --project samples/cli/probes/closed_template_methods
```
