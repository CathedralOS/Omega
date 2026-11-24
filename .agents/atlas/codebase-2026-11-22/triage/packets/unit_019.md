# Atlas packet unit_019: native stages 11 to 14 (encoding, layout, function placement, objects)

Receipt in progress. Claims are being settled against `main` at `24ad2700685e6cc6062026c87ed5fd24ed415265`.

| # | Original claim | Current source | Contract, test or trace | Outcome |
| --- | --- | --- | --- | --- |
| 1 | `R` S11 `src/adapters.rs` fail-open 1 and 2. `X86RequireEqual` and `X86IncrementChecked` encode any register's codec index without an x86 check; `X86DescriptorStore` shifts a raw `operation` and `X86Require` pushes a raw condition byte. | `P11/src/adapters.rs:271-321` | `gpr_code(register).ok_or(AdapterEncodingError)?` now refuses non-GPR registers. The descriptor table and guard predicate are typed enums (`X86DescriptorTable`, `X86Condition`). Fixtures `integer_adapter_checks_require_an_x86_general_purpose_register` and `installed_adapter_preserves_the_descriptor_table_and_guard_predicate`. | **Fixed** in [#5206](https://github.com/CathedralOS/Omega/pull/5206) and [#5208](https://github.com/CathedralOS/Omega/pull/5208). |
