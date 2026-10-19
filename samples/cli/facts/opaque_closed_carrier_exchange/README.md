# Closed opaque carrier exchange

Parent: `OPAQUE-BY-VALUE-BOUNDARY-ABI`.

Two actual libc `inet_makeaddr` results cross an opaque boundary by value, move through a helper parameter/result and an enclosing record field, then reach `inet_lnaof`. The guest checks distinct host numbers 66051 and 513, then replaces one consumed field with a third value, 1027, while preserving its sibling. It exits 70; a mismatch exits 71. The selected storage is the exact closed `AddressCarrier<AddressCarrier<AddressWord>>` application. The helper cannot inspect its representation.

The `invalid` twin changes only `AddressWord` by adding nominal cleanup. It must reject the selected carrier's complete Inert graph, even though its byte layout is unchanged. Additional corpus controls cover a growing recursive family and cleanup in an inactive generic sum payload.

At frozen base `53cd2938437a29356a48d6eaf29f14de07a4bb99`, the ordinary nongeneric carrier ran natively on Linux x86_64 and exited 70. Closed `AddressCarrier<u32>` and `AddressCarrier<AddressWord>` applications rejected with `opaque representation carrier AddressCarrier is not target-closed`. The repair retains the exact closed carrier application through Inert admission, Terminal shape, ABI graph and native layout. Terminal record transactions also retain admitted host payloads through moves and projections; writes invalidate enclosing payload tokens while preserving disjoint children. A finite `AddressCarrier<AddressWrapper<AddressWord>>` composition also checks and has a separate native/Terminal witness: the wrapper contains another carrier of its supplied argument. Carrier admission and field preparation distinguish entering that supplied argument from a declaration that manufactures a growing family. Its cleanup twin adds only nominal cleanup; its growing-family twin changes only the carrier field.

```sh
omega run samples/cli/facts/opaque_closed_carrier_exchange/main.omg
omega update --project samples/cli/facts/opaque_closed_carrier_exchange/invalid --target linux_x86_64
# Rejects during package candidate checking: nominal cleanup is not Inert.
```

The native fixture uses libc's actual independently compiled functions. The corpus’s authored Terminal inspection observations execute the same guest comparison through canonical Terminal decode and typed host adapters that call those same libc functions, retaining each actual value by its opaque forwarding identity. This checks Terminal occurrence preservation; native execution checks physical ABI transport. The ordinary CLI Terminal runtime has no trusted `inet_*` adapters and declines those imports. Additional corpus observations protect materialized owned entry records: mutation followed by whole return and unchanged packing followed by whole return retain their guest backing.

This witness does not establish independently compiled Omega-component exchange, mismatched-component admission, or other hosts' runtime results. That broader requirement remains open; equal layouts do not establish representation agreement.
