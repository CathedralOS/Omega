# Migrating assumed binding contracts

`Build::assume_binding_contract` was removed. Select an implementation that satisfies the requirement the caller invokes, or write a checked adapter that does. An adapter with a stronger internal input requirement must establish that requirement before it calls the implementation. An adapter that promises a narrower result must prove that result on every normal return.

The checked input example accepts any index, tests its domain, and performs the array lookup only on the admitted branch. The checked result example supplies a provider that proves the bound used by its caller. [Input domain](../../../samples/cli/facts/checked_provider_input_domain/main.omg) · [Result bound](../../../samples/cli/facts/checked_provider_result_bound/main.omg)

Remove historical version 4 assumed-binding rows when migrating a lock. The current lock reader rejects that row kind. Native address formation requires its own Product and dependency permissions. An issuer-controlled qualification still requires its declared issuance route. [Current provider rules](../resources/native_borrows.md#provider-contracts-and-migration) · [Lock migration](../packages/locks.md)

[Historical version 4 design](https://github.com/CathedralOS/Omega/blob/3f94bb48575/wiki/spec/build/assumed_binding_contracts.md)
