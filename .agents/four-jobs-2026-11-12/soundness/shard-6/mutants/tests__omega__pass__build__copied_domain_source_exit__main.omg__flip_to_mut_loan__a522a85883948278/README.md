Compiling an application and its build script together must keep the constraints on fields copied from a generic record. Both files import `Parcel` from `contracts.omg`; the copied `value` field must still select that module’s domain, even though the application declares another domain with the same name.

The program passes the field to the consumer requiring its original domain and checks the result, field value, payload and neighboring canaries. It must exit 0 in native execution and the Terminal interpreter. The build selects an entry for all four hosted targets; the recorded execution uses the host target.

The [rejection twin](../../../fail/build/copied_domain_foreign/main.omg) changes only the consuming machine. That machine requires the application’s same-spelled foreign domain, which the field does not satisfy. Rejection must name that call obligation.

Before the repair, the positive refused the generated `Parcel<u64>` field because its source location had been discarded while lowering copied constraints. The repair retains the supplying source separately from whether a new authored selection must be recorded. It uses the existing import-aware resolver and discards the temporary source location after selecting the exact declaration.

This covers the copied-field repair. The separate failure proving `Optional::None` when Build and Product retain duplicate declarations remains outside this witness.
