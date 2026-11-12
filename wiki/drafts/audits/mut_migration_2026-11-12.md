# Q7 migration list (`mut` for writable loans)

Inventory of Omega sources that lend or mutate through locals not declared `mut`, per [Owned local mutability](wiki/spec/language/ownership.md). This is a migration checklist only: no sources were edited.

## Summary

- Measured commit: `0835b3aaf803ed97f905be42d94727ff58459263`
- Total hits: **1315**
- Flagged for review: **568**

| Directory | Hits | Files with ≥1 hit |
| --- | ---: | ---: |
| `tests/omega` | 853 | 625 |
| `samples` | 449 | 366 |
| `source/library` | 1 | 1 |
| `source/psi` | 12 | 7 |

Hits group one line per violation kind at a site (writable borrow, assignment, field store, index store). The scan is lexical (brace-depth scope, comment stripping): it may miss rare spellings and can list `tests/omega/fail/` sites that document intentional rejections alongside migration work. Reference carriers (`let x: &mut T` / `&write T`) skip borrow and store-through-reference sites; carrier reassignment is listed under review when `mut` on the binding may change meaning.

## Flagged for review

- `samples/cli/facts/case_payload_exclusive_loan/call_result/invalid/main.omg`:26 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/case_payload_exclusive_loan/call_result/main.omg`:25 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/case_payload_exclusive_loan/const_name/invalid/main.omg`:25 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/case_payload_exclusive_loan/const_name/main.omg`:24 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/case_payload_exclusive_loan/field/invalid/main.omg`:25 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/case_payload_exclusive_loan/field/main.omg`:24 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/case_payload_exclusive_loan/local/invalid/main.omg`:22 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/case_payload_exclusive_loan/local/main.omg`:21 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/case_payload_exclusive_loan/main.omg`:21 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/case_payload_exclusive_loan/match_join/invalid/main.omg`:28 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/case_payload_exclusive_loan/match_join/main.omg`:27 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/case_payload_exclusive_loan/other_machine/invalid/main.omg`:23 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/case_payload_exclusive_loan/other_machine/main.omg`:22 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/case_payload_exclusive_loan/other_state/invalid/main.omg`:24 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/case_payload_exclusive_loan/other_state/main.omg`:23 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/case_payload_exclusive_loan/parameter/invalid/main.omg`:23 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/case_payload_exclusive_loan/parameter/main.omg`:22 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/case_payload_exclusive_loan/target_body/invalid/main.omg`:48 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/case_payload_exclusive_loan/target_body/invalid/main.omg`:64 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/case_payload_exclusive_loan/target_body/main.omg`:47 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/case_payload_exclusive_loan/target_body/main.omg`:62 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/checked_predicate_disjoint_loans/call_result/invalid/main.omg`:13 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/call_result/invalid/main.omg`:14 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/call_result/main.omg`:13 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/call_result/main.omg`:14 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/const_name/invalid/main.omg`:11 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/const_name/invalid/main.omg`:12 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/const_name/main.omg`:11 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/const_name/main.omg`:12 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/field/invalid/main.omg`:12 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/field/invalid/main.omg`:13 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/field/main.omg`:12 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/field/main.omg`:13 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/local/invalid/main.omg`:12 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/local/invalid/main.omg`:13 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/local/main.omg`:12 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/local/main.omg`:13 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/main.omg`:11 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/main.omg`:12 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/match_join/invalid/main.omg`:13 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/match_join/invalid/main.omg`:14 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/match_join/main.omg`:13 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/match_join/main.omg`:14 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/other_machine/invalid/main.omg`:11 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/other_machine/invalid/main.omg`:12 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/other_machine/main.omg`:11 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/other_machine/main.omg`:12 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/other_state/invalid/main.omg`:16 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/other_state/invalid/main.omg`:17 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/other_state/main.omg`:16 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/other_state/main.omg`:17 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/parameter/invalid/main.omg`:11 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/parameter/invalid/main.omg`:12 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/parameter/main.omg`:11 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/parameter/main.omg`:12 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/target_body/invalid/main.omg`:37 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/target_body/invalid/main.omg`:38 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/target_body/invalid/main.omg`:49 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/target_body/invalid/main.omg`:50 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/target_body/main.omg`:37 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/target_body/main.omg`:38 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/target_body/main.omg`:49 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `samples/cli/facts/checked_predicate_disjoint_loans/target_body/main.omg`:50 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `samples/cli/facts/exclusive_match_arm/call_result/invalid/main.omg`:23 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/exclusive_match_arm/call_result/main.omg`:23 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/exclusive_match_arm/const_name/invalid/main.omg`:21 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/exclusive_match_arm/const_name/main.omg`:21 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/exclusive_match_arm/field/invalid/main.omg`:20 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/exclusive_match_arm/field/main.omg`:20 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/exclusive_match_arm/local/invalid/main.omg`:19 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/exclusive_match_arm/local/main.omg`:19 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/exclusive_match_arm/main.omg`:18 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/exclusive_match_arm/match_join/invalid/main.omg`:24 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/exclusive_match_arm/match_join/main.omg`:24 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/exclusive_match_arm/other_machine/invalid/main.omg`:16 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/exclusive_match_arm/other_machine/main.omg`:16 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/exclusive_match_arm/other_state/invalid/main.omg`:18 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/exclusive_match_arm/other_state/main.omg`:18 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/exclusive_match_arm/parameter/invalid/main.omg`:16 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/exclusive_match_arm/parameter/main.omg`:16 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/exclusive_match_arm/target_body/invalid/main.omg`:45 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/exclusive_match_arm/target_body/invalid/main.omg`:60 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/exclusive_match_arm/target_body/main.omg`:45 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/exclusive_match_arm/target_body/main.omg`:59 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/indexed_recoverable_replacement_window/call_result/invalid/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `samples/cli/facts/indexed_recoverable_replacement_window/call_result/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `samples/cli/facts/indexed_recoverable_replacement_window/const_name/invalid/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `samples/cli/facts/indexed_recoverable_replacement_window/const_name/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `samples/cli/facts/indexed_recoverable_replacement_window/field/invalid/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `samples/cli/facts/indexed_recoverable_replacement_window/field/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `samples/cli/facts/indexed_recoverable_replacement_window/local/invalid/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `samples/cli/facts/indexed_recoverable_replacement_window/local/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `samples/cli/facts/indexed_recoverable_replacement_window/match_join/invalid/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `samples/cli/facts/indexed_recoverable_replacement_window/match_join/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `samples/cli/facts/indexed_recoverable_replacement_window/other_machine/invalid/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `samples/cli/facts/indexed_recoverable_replacement_window/other_machine/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `samples/cli/facts/indexed_recoverable_replacement_window/other_state/invalid/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `samples/cli/facts/indexed_recoverable_replacement_window/other_state/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `samples/cli/facts/indexed_recoverable_replacement_window/parameter/invalid/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `samples/cli/facts/indexed_recoverable_replacement_window/parameter/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `samples/cli/facts/indexed_recoverable_replacement_window/target_body/invalid/main.omg`:59 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `samples/cli/facts/indexed_recoverable_replacement_window/target_body/invalid/main.omg`:73 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `samples/cli/facts/indexed_recoverable_replacement_window/target_body/main.omg`:59 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `samples/cli/facts/indexed_recoverable_replacement_window/target_body/main.omg`:73 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `samples/cli/facts/indexed_storage_address_identity/call_result/invalid/main.omg`:9 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 13;`
- `samples/cli/facts/indexed_storage_address_identity/const_name/invalid/main.omg`:9 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 13;`
- `samples/cli/facts/indexed_storage_address_identity/field/invalid/main.omg`:9 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 13;`
- `samples/cli/facts/indexed_storage_address_identity/local/invalid/main.omg`:8 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 13;`
- `samples/cli/facts/indexed_storage_address_identity/match_join/invalid/main.omg`:8 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 13;`
- `samples/cli/facts/indexed_storage_address_identity/other_machine/invalid/main.omg`:7 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 13;`
- `samples/cli/facts/indexed_storage_address_identity/other_state/invalid/main.omg`:9 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 13;`
- `samples/cli/facts/indexed_storage_address_identity/parameter/invalid/main.omg`:6 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 13;`
- `samples/cli/facts/indexed_storage_address_identity/target_body/invalid/main.omg`:36 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 13;`
- `samples/cli/facts/indexed_storage_address_identity/target_body/invalid/main.omg`:55 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 13;`
- `samples/cli/facts/match_computed_root/call_result/invalid/main.omg`:26 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/match_computed_root/call_result/main.omg`:25 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/match_computed_root/const_name/invalid/main.omg`:23 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/match_computed_root/const_name/main.omg`:22 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/match_computed_root/field/invalid/main.omg`:25 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/match_computed_root/field/main.omg`:24 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/match_computed_root/local/invalid/main.omg`:22 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/match_computed_root/local/main.omg`:21 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/match_computed_root/match_join/invalid/main.omg`:27 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/match_computed_root/match_join/main.omg`:26 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/match_computed_root/other_machine/invalid/main.omg`:19 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/match_computed_root/other_machine/main.omg`:18 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/match_computed_root/other_state/invalid/main.omg`:21 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/match_computed_root/other_state/main.omg`:20 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/match_computed_root/parameter/invalid/main.omg`:19 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/match_computed_root/parameter/main.omg`:18 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/match_computed_root/target_body/invalid/main.omg`:19 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/match_computed_root/target_body/main.omg`:18 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/recoverable_replacement_window/call_result/invalid/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `samples/cli/facts/recoverable_replacement_window/call_result/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `samples/cli/facts/recoverable_replacement_window/const_name/invalid/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `samples/cli/facts/recoverable_replacement_window/const_name/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `samples/cli/facts/recoverable_replacement_window/field/invalid/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `samples/cli/facts/recoverable_replacement_window/field/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `samples/cli/facts/recoverable_replacement_window/local/invalid/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `samples/cli/facts/recoverable_replacement_window/local/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `samples/cli/facts/recoverable_replacement_window/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `samples/cli/facts/recoverable_replacement_window/match_join/invalid/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `samples/cli/facts/recoverable_replacement_window/match_join/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `samples/cli/facts/recoverable_replacement_window/other_machine/invalid/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `samples/cli/facts/recoverable_replacement_window/other_machine/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `samples/cli/facts/recoverable_replacement_window/other_state/invalid/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `samples/cli/facts/recoverable_replacement_window/other_state/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `samples/cli/facts/recoverable_replacement_window/parameter/invalid/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `samples/cli/facts/recoverable_replacement_window/parameter/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `samples/cli/facts/recoverable_replacement_window/target_body/invalid/main.omg`:58 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `samples/cli/facts/recoverable_replacement_window/target_body/invalid/main.omg`:75 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `samples/cli/facts/recoverable_replacement_window/target_body/main.omg`:58 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `samples/cli/facts/recoverable_replacement_window/target_body/main.omg`:75 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `samples/cli/facts/reference_arrival_handoff/call_result/invalid/main.omg`:9 — `original` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `original = 7;`
- `samples/cli/facts/reference_arrival_handoff/call_result/invalid/main.omg`:10 — `child` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `child = 41;`
- `samples/cli/facts/reference_arrival_handoff/call_result/main.omg`:9 — `child` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `child = 41;`
- `samples/cli/facts/reference_arrival_handoff/const_name/invalid/main.omg`:9 — `original` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `original = 7;`
- `samples/cli/facts/reference_arrival_handoff/const_name/invalid/main.omg`:10 — `child` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `child = WRITTEN;`
- `samples/cli/facts/reference_arrival_handoff/const_name/main.omg`:9 — `child` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `child = WRITTEN;`
- `samples/cli/facts/reference_arrival_handoff/field/invalid/main.omg`:9 — `original` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `original = 7;`
- `samples/cli/facts/reference_arrival_handoff/local/invalid/main.omg`:8 — `original` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `original = 7;`
- `samples/cli/facts/reference_arrival_handoff/local/invalid/main.omg`:9 — `child` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `child = 41;`
- `samples/cli/facts/reference_arrival_handoff/local/main.omg`:8 — `child` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `child = 41;`
- `samples/cli/facts/reference_arrival_handoff/main.omg`:7 — `child` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `child = 41;`
- `samples/cli/facts/reference_arrival_handoff/match_join/invalid/main.omg`:15 — `child` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `child = 41;`
- `samples/cli/facts/reference_arrival_handoff/match_join/main.omg`:14 — `child` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `child = 41;`
- `samples/cli/facts/reference_arrival_handoff/other_machine/invalid/main.omg`:11 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 7;`
- `samples/cli/facts/reference_arrival_handoff/parameter/invalid/main.omg`:10 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 7;`
- `samples/cli/facts/reference_arrival_handoff/target_body/invalid/main.omg`:53 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 7;`
- `samples/cli/facts/returned_write_destination/call_result/invalid/main.omg`:9 — `destination` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `destination = destination;`
- `samples/cli/facts/returned_write_destination/call_result/main.omg`:9 — `destination` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `destination = 42;`
- `samples/cli/facts/returned_write_destination/const_name/invalid/main.omg`:8 — `destination` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `destination = destination;`
- `samples/cli/facts/returned_write_destination/const_name/main.omg`:8 — `destination` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `destination = 42;`
- `samples/cli/facts/returned_write_destination/local/invalid/main.omg`:8 — `destination` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `destination = destination;`
- `samples/cli/facts/returned_write_destination/local/main.omg`:8 — `destination` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `destination = 42;`
- `samples/cli/facts/returned_write_destination/main.omg`:7 — `destination` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `destination = 42;`
- `samples/cli/facts/returned_write_destination/match_join/invalid/main.omg`:6 — `destination` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `destination = destination;`
- `samples/cli/facts/returned_write_destination/match_join/main.omg`:6 — `destination` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `destination = 42;`
- `samples/cli/facts/returned_write_destination/target_body/invalid/main.omg`:44 — `destination` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `destination = destination;`
- `samples/cli/facts/returned_write_destination/target_body/main.omg`:44 — `destination` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `destination = 42;`
- `samples/cli/facts/write_only_match_join/call_result/invalid/main.omg`:27 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/write_only_match_join/call_result/main.omg`:26 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/write_only_match_join/const_name/invalid/main.omg`:26 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/write_only_match_join/const_name/main.omg`:25 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/write_only_match_join/field/invalid/main.omg`:26 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/write_only_match_join/field/main.omg`:25 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/write_only_match_join/local/invalid/main.omg`:23 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/write_only_match_join/local/main.omg`:22 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/write_only_match_join/match_join/invalid/main.omg`:29 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/write_only_match_join/match_join/main.omg`:28 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/write_only_match_join/other_machine/invalid/main.omg`:22 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/write_only_match_join/other_machine/main.omg`:21 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/write_only_match_join/other_state/invalid/main.omg`:26 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/write_only_match_join/other_state/main.omg`:25 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/write_only_match_join/parameter/invalid/main.omg`:22 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/write_only_match_join/parameter/main.omg`:21 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/write_only_match_join/target_body/invalid/main.omg`:51 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/write_only_match_join/target_body/invalid/main.omg`:66 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/write_only_match_join/target_body/main.omg`:50 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/facts/write_only_match_join/target_body/main.omg`:64 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `samples/cli/probes/bool_representation_views/main.omg`:41 — `byte_alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `byte_alias = false;`
- `samples/cli/probes/projected_borrow_forwarding/main.omg`:16 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 29;`
- `samples/cli/probes/projected_borrow_forwarding/main.omg`:28 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 29;`
- `tests/omega/fail/borrow/recast_selector_retires_entry_bound/main.omg`:5 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = false;`
- `tests/omega/fail/borrow/returned_write_destination_argument_read/main.omg`:4 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 42;`
- `tests/omega/fail/borrow/returned_write_destination_backing_conflict/main.omg`:6 — `destination` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `destination = 42;`
- `tests/omega/fail/borrow/returned_write_destination_carrier_conflict/main.omg`:5 — `moved` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `moved = 42;`
- `tests/omega/fail/borrow/selector_calls_do_not_share_write_frame/main.omg`:11 — `slot` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `slot = 9;`
- `tests/omega/fail/borrow/write_only_stored_reference_constraint_loss/main.omg`:7 — `value` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `value = 23;`
- `tests/omega/fail/borrows/borrow_local_alias_active/main.omg`:8 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = 3;`
- `tests/omega/fail/borrows/borrow_unknown_index_pair_mut/main.omg`:13 — `left` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `left = Entry { value: 1 };`
- `tests/omega/fail/borrows/borrow_unknown_index_pair_mut/main.omg`:14 — `right` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `right = Entry { value: 2 };`
- `tests/omega/fail/borrows/conditional_copied_slot_retains_parent_loan/main.omg`:24 — `parent` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `parent = 9;`
- `tests/omega/fail/borrows/copied_borrowed_slot_retains_parent_loan/main.omg`:12 — `parent` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `parent = 9;`
- `tests/omega/fail/borrows/erased_input_before_returned_reference_live_source_write/main.omg`:8 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = 10;`
- `tests/omega/fail/borrows/erased_input_before_second_returned_reference_live_source_write/main.omg`:8 — `chosen` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `chosen = 10;`
- `tests/omega/fail/borrows/float_comparison_read_during_live_exclusive_loan/main.omg`:11 — `raised` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `raised = 3.0;`
- `tests/omega/fail/borrows/forwarded_call_view_retains_operand_extent/main.omg`:11 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 70;`
- `tests/omega/fail/borrows/fresh_selector_borrow_has_no_parent_capture/main.omg`:15 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 7;`
- `tests/omega/fail/borrows/fresh_selector_borrow_has_no_parent_capture/main.omg`:16 — `fresh` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `fresh = 11;`
- `tests/omega/fail/borrows/indexed_exclusive_primitive_loan_preserves_field_invariant/main.omg`:8 — `selected_value` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected_value = 2;`
- `tests/omega/fail/borrows/multi_origin_returned_reference_live_source_write/main.omg`:15 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 1;`
- `tests/omega/fail/borrows/mutable_primitive_reference_live_owner_read/main.omg`:9 — `borrowed` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `borrowed = 9;`
- `tests/omega/fail/borrows/nested_machine_carried_recurrent_selector_false_guarantee_rejected/main.omg`:24 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 99;`
- `tests/omega/fail/borrows/parent_use_before_disjoint_sibling_ends/main.omg`:10 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `tests/omega/fail/borrows/parent_use_before_disjoint_sibling_ends/main.omg`:14 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `tests/omega/fail/borrows/plain_read_during_live_exclusive_loan/main.omg`:36 — `borrowed` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `borrowed = 9;`
- `tests/omega/fail/borrows/read_positions_during_live_exclusive_loan/main.omg`:23 — `borrowed` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `borrowed = 9;`
- `tests/omega/fail/borrows/read_positions_during_live_exclusive_loan/main.omg`:31 — `lent` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `lent = 9;`
- `tests/omega/fail/borrows/read_positions_during_live_exclusive_loan/main.omg`:39 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 9;`
- `tests/omega/fail/borrows/region_store_live_sibling_reference/main.omg`:14 — `parent` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `parent = 9;`
- `tests/omega/fail/borrows/selected_reference_live_nested_parent_write/main.omg`:12 — `second_parent` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second_parent = 0;`
- `tests/omega/fail/borrows/selected_reference_live_nested_parent_write/main.omg`:13 — `nested` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `nested = 1;`
- `tests/omega/fail/borrows/selected_reference_live_reborrow_parent_write/main.omg`:9 — `second_view` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second_view = 0;`
- `tests/omega/fail/borrows/selected_reference_live_reborrow_parent_write/main.omg`:10 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 1;`
- `tests/omega/fail/borrows/whole_mut_receiver_read_under_live_local_loan/main.omg`:21 — `other_view` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `other_view = 4;`
- `tests/omega/fail/borrows/whole_mut_receiver_read_under_live_local_loan/main.omg`:28 — `view` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `view = 3;`
- `tests/omega/fail/contracts/borrowed_guarantee_overlapping_field_write_rejected/main.omg`:20 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 73;`
- `tests/omega/fail/contracts/borrowed_guarantee_overlapping_field_write_rejected/main.omg`:34 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 73;`
- `tests/omega/fail/contracts/referent_replacement_false_guarantee/main.omg`:9 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = Pair { first: Child { value: 8 }, second: 9 };`
- `tests/omega/fail/facts/case_payload_exclusive_loan_through_call_result_rejected/main.omg`:26 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/case_payload_exclusive_loan_through_const_name_rejected/main.omg`:25 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/case_payload_exclusive_loan_through_field_rejected/main.omg`:25 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/case_payload_exclusive_loan_through_local_rejected/main.omg`:22 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/case_payload_exclusive_loan_through_match_join_rejected/main.omg`:28 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/case_payload_exclusive_loan_through_other_machine_rejected/main.omg`:19 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/case_payload_exclusive_loan_through_other_state_rejected/main.omg`:24 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/case_payload_exclusive_loan_through_parameter_rejected/main.omg`:19 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/case_payload_exclusive_loan_through_target_body_rejected/main.omg`:48 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/case_payload_exclusive_loan_through_target_body_rejected/main.omg`:64 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/case_predicate_alias_sibling_restore_rejected/main.omg`:15 — `low` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `low = 31;`
- `tests/omega/fail/facts/case_predicate_recast_sibling_restore_rejected/main.omg`:15 — `view` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `view = 31;`
- `tests/omega/fail/facts/checked_predicate_disjoint_loans_through_call_result_rejected/main.omg`:13 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `tests/omega/fail/facts/checked_predicate_disjoint_loans_through_call_result_rejected/main.omg`:14 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `tests/omega/fail/facts/checked_predicate_disjoint_loans_through_const_name_rejected/main.omg`:11 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `tests/omega/fail/facts/checked_predicate_disjoint_loans_through_const_name_rejected/main.omg`:12 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `tests/omega/fail/facts/checked_predicate_disjoint_loans_through_field_rejected/main.omg`:12 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `tests/omega/fail/facts/checked_predicate_disjoint_loans_through_field_rejected/main.omg`:13 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `tests/omega/fail/facts/checked_predicate_disjoint_loans_through_local_rejected/main.omg`:12 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `tests/omega/fail/facts/checked_predicate_disjoint_loans_through_local_rejected/main.omg`:13 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `tests/omega/fail/facts/checked_predicate_disjoint_loans_through_match_join_rejected/main.omg`:13 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `tests/omega/fail/facts/checked_predicate_disjoint_loans_through_match_join_rejected/main.omg`:14 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `tests/omega/fail/facts/checked_predicate_disjoint_loans_through_other_machine_rejected/main.omg`:11 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `tests/omega/fail/facts/checked_predicate_disjoint_loans_through_other_machine_rejected/main.omg`:12 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `tests/omega/fail/facts/checked_predicate_disjoint_loans_through_other_state_rejected/main.omg`:16 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `tests/omega/fail/facts/checked_predicate_disjoint_loans_through_other_state_rejected/main.omg`:17 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `tests/omega/fail/facts/checked_predicate_disjoint_loans_through_parameter_rejected/main.omg`:11 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `tests/omega/fail/facts/checked_predicate_disjoint_loans_through_parameter_rejected/main.omg`:12 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `tests/omega/fail/facts/checked_predicate_disjoint_loans_through_target_body_rejected/main.omg`:37 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `tests/omega/fail/facts/checked_predicate_disjoint_loans_through_target_body_rejected/main.omg`:38 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `tests/omega/fail/facts/checked_predicate_disjoint_loans_through_target_body_rejected/main.omg`:49 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `tests/omega/fail/facts/checked_predicate_disjoint_loans_through_target_body_rejected/main.omg`:50 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `tests/omega/fail/facts/exclusive_match_arm_through_call_result_rejected/main.omg`:25 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/exclusive_match_arm_through_const_name_rejected/main.omg`:21 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/exclusive_match_arm_through_field_rejected/main.omg`:20 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/exclusive_match_arm_through_local_rejected/main.omg`:19 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/exclusive_match_arm_through_match_join_rejected/main.omg`:24 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/exclusive_match_arm_through_other_machine_rejected/main.omg`:18 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/exclusive_match_arm_through_other_state_rejected/main.omg`:20 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/exclusive_match_arm_through_parameter_rejected/main.omg`:18 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/exclusive_match_arm_through_target_body_rejected/main.omg`:18 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/exclusive_match_arm_through_target_body_rejected/main.omg`:30 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/exclusive_match_arm_through_target_body_rejected/main.omg`:42 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/exclusive_match_arm_through_target_body_rejected/main.omg`:54 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/indexed_recoverable_replacement_window_through_call_result_rejected/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `tests/omega/fail/facts/indexed_recoverable_replacement_window_through_const_name_rejected/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `tests/omega/fail/facts/indexed_recoverable_replacement_window_through_field_rejected/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `tests/omega/fail/facts/indexed_recoverable_replacement_window_through_local_rejected/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `tests/omega/fail/facts/indexed_recoverable_replacement_window_through_match_join_rejected/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `tests/omega/fail/facts/indexed_recoverable_replacement_window_through_other_machine_rejected/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `tests/omega/fail/facts/indexed_recoverable_replacement_window_through_other_state_rejected/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `tests/omega/fail/facts/indexed_recoverable_replacement_window_through_parameter_rejected/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `tests/omega/fail/facts/indexed_recoverable_replacement_window_through_target_body_rejected/main.omg`:59 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `tests/omega/fail/facts/indexed_recoverable_replacement_window_through_target_body_rejected/main.omg`:73 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `tests/omega/fail/facts/indexed_storage_address_identity_through_call_result_rejected/main.omg`:9 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 13;`
- `tests/omega/fail/facts/indexed_storage_address_identity_through_const_name_rejected/main.omg`:9 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 13;`
- `tests/omega/fail/facts/indexed_storage_address_identity_through_field_rejected/main.omg`:9 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 13;`
- `tests/omega/fail/facts/indexed_storage_address_identity_through_local_rejected/main.omg`:8 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 13;`
- `tests/omega/fail/facts/indexed_storage_address_identity_through_match_join_rejected/main.omg`:8 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 13;`
- `tests/omega/fail/facts/indexed_storage_address_identity_through_other_machine_rejected/main.omg`:7 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 13;`
- `tests/omega/fail/facts/indexed_storage_address_identity_through_other_state_rejected/main.omg`:9 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 13;`
- `tests/omega/fail/facts/indexed_storage_address_identity_through_parameter_rejected/main.omg`:6 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 13;`
- `tests/omega/fail/facts/indexed_storage_address_identity_through_target_body_rejected/main.omg`:36 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 13;`
- `tests/omega/fail/facts/indexed_storage_address_identity_through_target_body_rejected/main.omg`:55 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 13;`
- `tests/omega/fail/facts/match_computed_root_through_call_result_rejected/main.omg`:26 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/match_computed_root_through_const_name_rejected/main.omg`:23 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/match_computed_root_through_field_rejected/main.omg`:25 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/match_computed_root_through_local_rejected/main.omg`:22 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/match_computed_root_through_match_join_rejected/main.omg`:27 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/match_computed_root_through_other_machine_rejected/main.omg`:19 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/match_computed_root_through_other_state_rejected/main.omg`:21 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/match_computed_root_through_parameter_rejected/main.omg`:19 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/match_computed_root_through_target_body_rejected/main.omg`:19 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/recoverable_replacement_window_through_call_result_rejected/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `tests/omega/fail/facts/recoverable_replacement_window_through_const_name_rejected/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `tests/omega/fail/facts/recoverable_replacement_window_through_field_rejected/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `tests/omega/fail/facts/recoverable_replacement_window_through_local_rejected/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `tests/omega/fail/facts/recoverable_replacement_window_through_match_join_rejected/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `tests/omega/fail/facts/recoverable_replacement_window_through_other_machine_rejected/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `tests/omega/fail/facts/recoverable_replacement_window_through_other_state_rejected/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `tests/omega/fail/facts/recoverable_replacement_window_through_parameter_rejected/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `tests/omega/fail/facts/recoverable_replacement_window_through_target_body_rejected/main.omg`:58 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `tests/omega/fail/facts/recoverable_replacement_window_through_target_body_rejected/main.omg`:75 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `tests/omega/fail/facts/reference_arrival_handoff_through_call_result_rejected/main.omg`:9 — `original` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `original = 7;`
- `tests/omega/fail/facts/reference_arrival_handoff_through_call_result_rejected/main.omg`:10 — `child` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `child = 41;`
- `tests/omega/fail/facts/reference_arrival_handoff_through_const_name_rejected/main.omg`:9 — `original` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `original = 7;`
- `tests/omega/fail/facts/reference_arrival_handoff_through_const_name_rejected/main.omg`:10 — `child` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `child = WRITTEN;`
- `tests/omega/fail/facts/reference_arrival_handoff_through_field_rejected/main.omg`:9 — `original` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `original = 7;`
- `tests/omega/fail/facts/reference_arrival_handoff_through_local_rejected/main.omg`:8 — `original` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `original = 7;`
- `tests/omega/fail/facts/reference_arrival_handoff_through_local_rejected/main.omg`:9 — `child` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `child = 41;`
- `tests/omega/fail/facts/reference_arrival_handoff_through_match_join_rejected/main.omg`:15 — `child` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `child = 41;`
- `tests/omega/fail/facts/reference_arrival_handoff_through_other_machine_rejected/main.omg`:11 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 7;`
- `tests/omega/fail/facts/reference_arrival_handoff_through_parameter_rejected/main.omg`:10 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 7;`
- `tests/omega/fail/facts/reference_arrival_handoff_through_target_body_rejected/main.omg`:13 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 7;`
- `tests/omega/fail/facts/returned_write_destination_through_call_result_rejected/main.omg`:9 — `destination` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `destination = destination;`
- `tests/omega/fail/facts/returned_write_destination_through_const_name_rejected/main.omg`:8 — `destination` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `destination = destination;`
- `tests/omega/fail/facts/returned_write_destination_through_local_rejected/main.omg`:8 — `destination` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `destination = destination;`
- `tests/omega/fail/facts/returned_write_destination_through_match_join_rejected/main.omg`:6 — `destination` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `destination = destination;`
- `tests/omega/fail/facts/returned_write_destination_through_target_body_rejected/main.omg`:44 — `destination` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `destination = destination;`
- `tests/omega/fail/facts/write_only_match_join_through_call_result_rejected/main.omg`:27 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/write_only_match_join_through_const_name_rejected/main.omg`:26 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/write_only_match_join_through_field_rejected/main.omg`:26 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/write_only_match_join_through_local_rejected/main.omg`:23 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/write_only_match_join_through_match_join_rejected/main.omg`:29 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/write_only_match_join_through_other_machine_rejected/main.omg`:22 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/write_only_match_join_through_other_state_rejected/main.omg`:26 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/write_only_match_join_through_parameter_rejected/main.omg`:22 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/write_only_match_join_through_target_body_rejected/main.omg`:51 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/facts/write_only_match_join_through_target_body_rejected/main.omg`:66 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/fail/layouts/callee_integer_at_multi_origin_reference_requires_fit/main.omg`:43 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = value;`
- `tests/omega/fail/layouts/captured_encoded_reference_replacement_requires_fit/main.omg`:43 — `captured` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `captured = 128;`
- `tests/omega/fail/layouts/captured_encoded_reference_replacement_requires_fit/main.omg`:45 — `replacement` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `replacement = 42;`
- `tests/omega/fail/layouts/direct_encoded_assignment_requires_fit/main.omg`:42 — `leaf` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `leaf = 200;`
- `tests/omega/fail/layouts/direct_encoded_assignment_requires_fit/main.omg`:44 — `ordinary` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `ordinary = 127;`
- `tests/omega/fail/ownership/indexed_borrowed_window_wrong_place_repair/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `tests/omega/fail/ownership/whole_borrowed_forwarded_live_descendant/main.omg`:10 — `forwarded` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `forwarded = removed;`
- `tests/omega/fail/proofs/scalar_field_snapshot_after_alias_store/main.omg`:7 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = 7;`
- `tests/omega/fail/recast/borrowed_byte_collection_predicate/main.omg`:5 — `value` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `value = 18446744073709551615;`
- `tests/omega/fail/recast/borrowed_byte_mutable_source_facts/main.omg`:5 — `value` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `value = 18446744073709551615;`
- `tests/omega/fail/recast/borrowed_byte_shared_to_mutable/main.omg`:4 — `value` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `value = 42;`
- `tests/omega/fail/recast/byte_record_field_predicate/main.omg`:6 — `value` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `value = 18446744073709551615;`
- `tests/omega/fail/recast/non_owning_erasure_false_reestablishment/main.omg`:7 — `view` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `view = Storage { value: 73 };`
- `tests/omega/fail/recast/non_owning_erasure_mixed_write_presentations/main.omg`:8 — `view` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `view = Storage { value: 73 };`
- `tests/omega/fail/recast/non_owning_erasure_preserves_array_element/main.omg`:4 — `view` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `view = 73;`
- `tests/omega/fail/recast/non_owning_erasure_preserves_helper_field/main.omg`:7 — `view` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `view = Storage { value: 73 };`
- `tests/omega/fail/recast/non_owning_erasure_preserves_owned_field/main.omg`:8 — `view` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `view = Storage { value: 73 };`
- `tests/omega/fail/recast/non_owning_erasure_preserves_projected_view/main.omg`:6 — `view` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `view = 73;`
- `tests/omega/fail/recast/non_owning_erasure_preserves_reborrow_alias/main.omg`:6 — `view` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `view = 73;`
- `tests/omega/fail/recast/non_owning_erasure_preserves_record_field/main.omg`:7 — `view` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `view = Storage { value: 73 };`
- `tests/omega/fail/recast/non_owning_erasure_preserves_scalar_field/main.omg`:5 — `view` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `view = 73;`
- `tests/omega/fail/recast/non_owning_erasure_preserves_stored_alias/main.omg`:9 — `view` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `view = Storage { value: 73 };`
- `tests/omega/fail/recast/partial_ascii_recast_does_not_restore_field/main.omg`:8 — `all` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `all = 18446744073709551615;`
- `tests/omega/fail/recast/partial_ascii_recast_does_not_restore_field/main.omg`:10 — `prefix` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `prefix = "safe";`
- `tests/omega/fail/recast/recast_mut_bool_as_unconstrained_byte/main.omg`:12 — `flag_bits` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `flag_bits = 0;`
- `tests/omega/fail/recast/recast_mut_where_bounded_field_widened/main.omg`:15 — `bits` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `bits = 2;`
- `tests/omega/fail/recast/write_only_stored_pointer_read/main.omg`:5 — `view` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `view = 73;`
- `tests/omega/fail/text/qualified_byte_alias_replacement_invalid_encoding/main.omg`:5 — `line` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `line = "\xff\0\0\0\0\0\0\0";`
- `tests/omega/fail/text/qualified_local_byte_alias_replacement_invalid_encoding/main.omg`:5 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = "\xff\0\0\0\0\0\0\0";`
- `tests/omega/pass/borrow/mutable_selector_unchanged_exit/main.omg`:6 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = false;`
- `tests/omega/pass/borrow/reference_parameter_referent_copy_exit/main.omg`:38 — `returned_integer` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `returned_integer = -29;`
- `tests/omega/pass/borrow/reference_parameter_referent_copy_exit/main.omg`:42 — `returned_boolean` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `returned_boolean = true;`
- `tests/omega/pass/borrow/reference_parameter_referent_copy_exit/main.omg`:46 — `returned_parameter_field` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `returned_parameter_field = true;`
- `tests/omega/pass/borrow/runtime_exclusive_loan_local_write_exit/main.omg`:19 — `seen` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `seen = 0;`
- `tests/omega/pass/borrow/runtime_write_only_aggregate_selector_snapshot_exit/main.omg`:19 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = incoming;`
- `tests/omega/pass/borrow/runtime_write_only_captured_record_copy_exit/main.omg`:11 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = incoming;`
- `tests/omega/pass/borrow/runtime_write_only_exclusive_reference_call_result_exit/main.omg`:13 — `slot` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `slot = identity(incoming);`
- `tests/omega/pass/borrow/runtime_write_only_reference_aggregate_alias_exit/main.omg`:13 — `slot` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `slot = incoming;`
- `tests/omega/pass/borrow/runtime_write_only_reference_aggregate_call_result_exit/main.omg`:13 — `slot` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `slot = forward(incoming);`
- `tests/omega/pass/borrows/borrow_domain_window_write_and_call/main.omg`:16 — `outside` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `outside = 5;`
- `tests/omega/pass/borrows/borrow_returned_window_write_and_call/main.omg`:25 — `outside` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `outside = 5;`
- `tests/omega/pass/borrows/borrow_stated_index_disequality_mut/main.omg`:22 — `left` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `left = Entry { value: 1 };`
- `tests/omega/pass/borrows/borrow_stated_index_disequality_mut/main.omg`:23 — `right` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `right = Entry { value: 2 };`
- `tests/omega/pass/borrows/borrowed_window_parameter_provenance_exit/main.omg`:15 — `slot` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `slot = incoming;`
- `tests/omega/pass/borrows/borrowed_window_parameter_provenance_exit/main.omg`:63 — `slot` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `slot = incoming;`
- `tests/omega/pass/borrows/conditional_copied_slot_restores_parent_exit/main.omg`:27 — `parent` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `parent = 9;`
- `tests/omega/pass/borrows/copied_borrowed_slot_restores_parent_exit/main.omg`:16 — `parent` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `parent = 9;`
- `tests/omega/pass/borrows/disjoint_branch_closure_with_grandchild/main.omg`:12 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 13 };`
- `tests/omega/pass/borrows/disjoint_branch_closure_with_grandchild/main.omg`:13 — `nested` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `nested = 17;`
- `tests/omega/pass/borrows/disjoint_branch_closure_with_grandchild/main.omg`:14 — `third` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `third = Cell { value: 40 };`
- `tests/omega/pass/borrows/erased_input_before_returned_reference_exit/main.omg`:26 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = 10;`
- `tests/omega/pass/borrows/erased_input_before_returned_reference_exit/main.omg`:32 — `third` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `third = 30;`
- `tests/omega/pass/borrows/erased_input_before_returned_reference_exit/main.omg`:38 — `fifth` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `fifth = 50;`
- `tests/omega/pass/borrows/mut_loan_local_write_through_exit/main.omg`:13 — `borrowed` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `borrowed = 9;`
- `tests/omega/pass/borrows/plain_read_after_exclusive_loan_last_use_exit/main.omg`:37 — `borrowed` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `borrowed = 9;`
- `tests/omega/pass/borrows/plain_read_after_exclusive_loan_last_use_exit/main.omg`:42 — `left` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `left = 5;`
- `tests/omega/pass/borrows/region_stored_reference_occurrences_exit/main.omg`:17 — `parent` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `parent = 9;`
- `tests/omega/pass/borrows/returned_primitive_reference_assignment_capture_exit/main.omg`:9 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 300;`
- `tests/omega/pass/borrows/runtime_disjoint_child_loans_restore_parent_exit/main.omg`:16 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `tests/omega/pass/borrows/runtime_disjoint_child_loans_restore_parent_exit/main.omg`:17 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `tests/omega/pass/borrows/runtime_disjoint_child_loans_restore_parent_exit/main.omg`:26 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 19 };`
- `tests/omega/pass/borrows/runtime_disjoint_child_loans_restore_parent_exit/main.omg`:27 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 51 };`
- `tests/omega/pass/borrows/runtime_index_mut_loan_write_exit/main.omg`:16 — `borrowed` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `borrowed = 9;`
- `tests/omega/pass/borrows/runtime_mutable_primitive_reference_exit/main.omg`:8 — `borrowed` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `borrowed = 9;`
- `tests/omega/pass/borrows/runtime_mutable_primitive_reference_exit/main.omg`:12 — `signed_reference` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `signed_reference = -17;`
- `tests/omega/pass/borrows/runtime_mutable_primitive_reference_exit/main.omg`:15 — `flag_reference` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `flag_reference = true;`
- `tests/omega/pass/borrows/runtime_mutable_reference_captures_selector_exit/main.omg`:8 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = Cell { value: 70 };`
- `tests/omega/pass/borrows/runtime_projected_call_loan_custody_exit/main.omg`:15 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 29;`
- `tests/omega/pass/borrows/runtime_projected_call_loan_custody_exit/main.omg`:27 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 29;`
- `tests/omega/pass/borrows/runtime_reference_aggregate_alias_replacement_exit/main.omg`:13 — `slot` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `slot = incoming;`
- `tests/omega/pass/borrows/runtime_reference_aggregate_call_result_replacement_exit/main.omg`:13 — `slot` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `slot = forward(incoming);`
- `tests/omega/pass/borrows/runtime_selected_reference_nested_restoration_exit/main.omg`:13 — `nested` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `nested = 127;`
- `tests/omega/pass/borrows/runtime_selected_reference_nested_restoration_exit/main.omg`:23 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 13;`
- `tests/omega/pass/contracts/borrowed_guarantee_disjoint_field_write_exit/main.omg`:20 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 73;`
- `tests/omega/pass/contracts/borrowed_guarantee_disjoint_field_write_exit/main.omg`:34 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 73;`
- `tests/omega/pass/contracts/disequality_disjoin/main.omg`:21 — `left` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `left = Entry { value: 1 };`
- `tests/omega/pass/contracts/disequality_disjoin/main.omg`:22 — `right` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `right = Entry { value: 2 };`
- `tests/omega/pass/expressions/anonymous_structural_match_results_exit/main.omg`:30 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 19;`
- `tests/omega/pass/expressions/borrowed_join_match_whole_store/main.omg`:12 — `r` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `r = Pair { left: 35, right: 35 };`
- `tests/omega/pass/facts/case_payload_exclusive_loan_through_call_result_exit/main.omg`:25 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/case_payload_exclusive_loan_through_const_name_exit/main.omg`:24 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/case_payload_exclusive_loan_through_field_exit/main.omg`:24 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/case_payload_exclusive_loan_through_local_exit/main.omg`:21 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/case_payload_exclusive_loan_through_match_join_exit/main.omg`:27 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/case_payload_exclusive_loan_through_other_machine_exit/main.omg`:18 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/case_payload_exclusive_loan_through_other_state_exit/main.omg`:23 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/case_payload_exclusive_loan_through_parameter_exit/main.omg`:22 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/case_payload_exclusive_loan_through_target_body_exit/main.omg`:47 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/case_payload_exclusive_loan_through_target_body_exit/main.omg`:62 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_call_result_exit/main.omg`:13 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_call_result_exit/main.omg`:14 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_const_name_exit/main.omg`:11 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_const_name_exit/main.omg`:12 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_field_exit/main.omg`:12 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_field_exit/main.omg`:13 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_local_exit/main.omg`:12 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_local_exit/main.omg`:13 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_match_join_exit/main.omg`:13 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_match_join_exit/main.omg`:14 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_other_machine_exit/main.omg`:11 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_other_machine_exit/main.omg`:12 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_other_state_exit/main.omg`:16 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_other_state_exit/main.omg`:17 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_parameter_exit/main.omg`:11 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_parameter_exit/main.omg`:12 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_target_body_exit/main.omg`:37 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_target_body_exit/main.omg`:38 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_target_body_exit/main.omg`:49 — `first` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `first = Cell { value: 17 };`
- `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_target_body_exit/main.omg`:50 — `second` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `second = Cell { value: 53 };`
- `tests/omega/pass/facts/exclusive_match_arm_through_call_result_exit/main.omg`:25 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/exclusive_match_arm_through_const_name_exit/main.omg`:21 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/exclusive_match_arm_through_field_exit/main.omg`:20 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/exclusive_match_arm_through_local_exit/main.omg`:19 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/exclusive_match_arm_through_match_join_exit/main.omg`:24 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/exclusive_match_arm_through_other_machine_exit/main.omg`:18 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/exclusive_match_arm_through_other_state_exit/main.omg`:20 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/exclusive_match_arm_through_parameter_exit/main.omg`:18 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/exclusive_match_arm_through_target_body_exit/main.omg`:47 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/exclusive_match_arm_through_target_body_exit/main.omg`:62 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/indexed_recoverable_replacement_window_through_call_result_exit/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `tests/omega/pass/facts/indexed_recoverable_replacement_window_through_const_name_exit/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `tests/omega/pass/facts/indexed_recoverable_replacement_window_through_field_exit/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `tests/omega/pass/facts/indexed_recoverable_replacement_window_through_local_exit/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `tests/omega/pass/facts/indexed_recoverable_replacement_window_through_match_join_exit/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `tests/omega/pass/facts/indexed_recoverable_replacement_window_through_other_machine_exit/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `tests/omega/pass/facts/indexed_recoverable_replacement_window_through_other_state_exit/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `tests/omega/pass/facts/indexed_recoverable_replacement_window_through_parameter_exit/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `tests/omega/pass/facts/indexed_recoverable_replacement_window_through_target_body_exit/main.omg`:59 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `tests/omega/pass/facts/indexed_recoverable_replacement_window_through_target_body_exit/main.omg`:73 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `tests/omega/pass/facts/match_computed_root_through_call_result_exit/main.omg`:27 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/match_computed_root_through_const_name_exit/main.omg`:22 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/match_computed_root_through_field_exit/main.omg`:24 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/match_computed_root_through_local_exit/main.omg`:21 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/match_computed_root_through_match_join_exit/main.omg`:26 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/match_computed_root_through_other_machine_exit/main.omg`:18 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/match_computed_root_through_other_state_exit/main.omg`:20 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/match_computed_root_through_parameter_exit/main.omg`:20 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/match_computed_root_through_target_body_exit/main.omg`:18 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/recoverable_replacement_window_through_call_result_exit/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `tests/omega/pass/facts/recoverable_replacement_window_through_const_name_exit/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `tests/omega/pass/facts/recoverable_replacement_window_through_field_exit/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `tests/omega/pass/facts/recoverable_replacement_window_through_local_exit/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `tests/omega/pass/facts/recoverable_replacement_window_through_match_join_exit/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `tests/omega/pass/facts/recoverable_replacement_window_through_other_machine_exit/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `tests/omega/pass/facts/recoverable_replacement_window_through_other_state_exit/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `tests/omega/pass/facts/recoverable_replacement_window_through_parameter_exit/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `tests/omega/pass/facts/recoverable_replacement_window_through_target_body_exit/main.omg`:58 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `tests/omega/pass/facts/recoverable_replacement_window_through_target_body_exit/main.omg`:75 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first {`
- `tests/omega/pass/facts/reference_arrival_handoff_through_call_result_exit/main.omg`:9 — `child` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `child = 41;`
- `tests/omega/pass/facts/reference_arrival_handoff_through_const_name_exit/main.omg`:9 — `child` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `child = WRITTEN;`
- `tests/omega/pass/facts/reference_arrival_handoff_through_local_exit/main.omg`:8 — `child` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `child = 41;`
- `tests/omega/pass/facts/reference_arrival_handoff_through_match_join_exit/main.omg`:14 — `child` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `child = 41;`
- `tests/omega/pass/facts/returned_write_destination_through_call_result_exit/main.omg`:9 — `destination` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `destination = 42;`
- `tests/omega/pass/facts/returned_write_destination_through_const_name_exit/main.omg`:8 — `destination` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `destination = 42;`
- `tests/omega/pass/facts/returned_write_destination_through_local_exit/main.omg`:8 — `destination` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `destination = 42;`
- `tests/omega/pass/facts/returned_write_destination_through_match_join_exit/main.omg`:6 — `destination` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `destination = 42;`
- `tests/omega/pass/facts/returned_write_destination_through_target_body_exit/main.omg`:44 — `destination` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `destination = 42;`
- `tests/omega/pass/facts/write_only_match_join_through_call_result_exit/main.omg`:26 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/write_only_match_join_through_const_name_exit/main.omg`:25 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/write_only_match_join_through_field_exit/main.omg`:25 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/write_only_match_join_through_local_exit/main.omg`:22 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/write_only_match_join_through_match_join_exit/main.omg`:28 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/write_only_match_join_through_other_machine_exit/main.omg`:21 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/write_only_match_join_through_other_state_exit/main.omg`:25 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/write_only_match_join_through_parameter_exit/main.omg`:21 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/write_only_match_join_through_target_body_exit/main.omg`:50 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/facts/write_only_match_join_through_target_body_exit/main.omg`:64 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = 41;`
- `tests/omega/pass/layouts/direct_encoded_assignment_preserves_descriptor_exit/main.omg`:42 — `leaf` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `leaf = 70;`
- `tests/omega/pass/layouts/direct_encoded_assignment_preserves_descriptor_exit/main.omg`:44 — `ordinary` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `ordinary = 127;`
- `tests/omega/pass/layouts/runtime_captured_encoded_reference_replacement_exit/main.omg`:43 — `captured` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `captured = 127;`
- `tests/omega/pass/layouts/runtime_captured_encoded_reference_replacement_exit/main.omg`:45 — `replacement` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `replacement = 42;`
- `tests/omega/pass/layouts/runtime_multi_origin_encoded_reference_exit/main.omg`:43 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = value;`
- `tests/omega/pass/layouts/runtime_recursive_encoded_reference_exit/main.omg`:81 — `encoded` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `encoded = -128;`
- `tests/omega/pass/layouts/runtime_recursive_encoded_reference_exit/main.omg`:85 — `ordinary` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `ordinary = 127;`
- `tests/omega/pass/layouts/runtime_selected_reference_parent_restoration_exit/main.omg`:39 — `selected` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `selected = value;`
- `tests/omega/pass/ownership/disjoint_field_loans_compile/main.omg`:14 — `left` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `left = 10;`
- `tests/omega/pass/ownership/disjoint_field_loans_compile/main.omg`:15 — `right` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `right = 20;`
- `tests/omega/pass/ownership/disjoint_member_write_during_loan_compile/main.omg`:19 — `loan` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `loan = 4;`
- `tests/omega/pass/ownership/runtime_borrowed_match_repeated_repair_exit/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = match first { Outcome::Success { token } -> token, Outcome::Failure { token } -> token };`
- `tests/omega/pass/ownership/runtime_indexed_recoverable_replacement_exit/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `tests/omega/pass/ownership/runtime_indexed_replacement_effectful_singleton_exit/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `tests/omega/pass/ownership/runtime_indexed_replacement_index_one_exit/main.omg`:30 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `tests/omega/pass/ownership/runtime_recoverable_alias_operand_order_exit/main.omg`:29 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = recover(first);`
- `tests/omega/pass/ownership/runtime_referent_replacement_fact_transport_exit/main.omg`:9 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = Pair { first: Child { value: source + 2 }, second: 5 };`
- `tests/omega/pass/ownership/runtime_replacement_call_child_snapshot_exit/main.omg`:23 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = Pair {`
- `tests/omega/pass/ownership/runtime_retained_alias_referent_transform_exit/main.omg`:26 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = transform(removed);`
- `tests/omega/pass/ownership/runtime_retained_alias_referent_transform_exit/main.omg`:34 — `forwarded` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `forwarded = transform(removed);`
- `tests/omega/pass/ownership/selected_storage_join_held_address_exit/main.omg`:21 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 73;`
- `tests/omega/pass/ownership/selected_storage_join_nested_exit/main.omg`:23 — `held` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `held = 73;`
- `tests/omega/pass/proofs/alias_store_keeps_sibling_snapshot_exit/main.omg`:15 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = 7;`
- `tests/omega/pass/providers/inherited_applied_binding_borrowed_result_exit/main.omg`:42 — `mutable` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `mutable = 43;`
- `tests/omega/pass/providers/inherited_provider_borrowed_result_exit/main.omg`:42 — `mutable` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `mutable = 43;`
- `tests/omega/pass/recast/borrowed_byte_scalar_views_exit/main.omg`:32 — `decoded` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `decoded = 4607182418800017408;`
- `tests/omega/pass/recast/mutable_byte_signed_write_compile/main.omg`:12 — `view` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `view = 0 - 1;`
- `tests/omega/pass/recast/mutable_float32_int_write_compile/main.omg`:12 — `view` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `view = 1077936128;`
- `tests/omega/pass/recast/mutable_float32_write_compile/main.omg`:12 — `view` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `view = 3.25;`
- `tests/omega/pass/recast/mutable_float64_signed_write_compile/main.omg`:12 — `view` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `view = 4613937818241073152;`
- `tests/omega/pass/recast/mutable_float_bits_write_compile/main.omg`:12 — `bits` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `bits = 13830554455654793216;`
- `tests/omega/pass/recast/mutable_i16_unsigned_write_compile/main.omg`:12 — `view` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `view = 65535;`
- `tests/omega/pass/recast/mutable_i32_float_write_compile/main.omg`:12 — `view` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `view = 0.5;`
- `tests/omega/pass/recast/mutable_i32_unsigned_write_compile/main.omg`:12 — `view` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `view = 0 - 3;`
- `tests/omega/pass/recast/mutable_int_float_write_compile/main.omg`:12 — `view` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `view = 1.5;`
- `tests/omega/pass/recast/mutable_local_recast_write_compile/main.omg`:13 — `view` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `view = 0 - 11;`
- `tests/omega/pass/recast/mutable_signed_bits_write_compile/main.omg`:12 — `view` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `view = 0 - 1;`
- `tests/omega/pass/recast/mutable_u16_signed_write_compile/main.omg`:12 — `view` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `view = 0 - 5;`
- `tests/omega/pass/recast/mutable_unsigned_signed_write_compile/main.omg`:12 — `view` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `view = 0 - 7;`
- `tests/omega/pass/recast/non_owning_erasure_retained_field_predicate_exit/main.omg`:6 — `view` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `view = 7;`
- `tests/omega/pass/recast/runtime_bool_representation_recast_exit/main.omg`:35 — `byte_flag` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `byte_flag = false;`
- `tests/omega/pass/recast/runtime_offset_byte_recast_mutable_write_exit/main.omg`:23 — `view` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `view = 67305985;`
- `tests/omega/pass/recast/runtime_restricted_field_loans_exit/main.omg`:20 — `flag` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `flag = true;`
- `tests/omega/pass/text/encoded_source_select/main.omg`:66 — `encoded` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `encoded = -128;`
- `tests/omega/pass/text/encoded_source_select/main.omg`:70 — `ordinary` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `ordinary = 127;`
- `tests/omega/pass/text/runtime_alias_string_write/main.omg`:18 — `label_alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `label_alias = TextBuffer { storage: [104u8, 101u8, 108u8, 108u8, 111u8, 0u8, 0u8, 0u8], used: 5 };`
- `tests/omega/pass/text/runtime_alias_string_write/main.omg`:28 — `another` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `another = TextBuffer { storage: [195u8, 169u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8], used: 2 };`
- `tests/omega/pass/text/runtime_alias_string_write/main.omg`:39 — `alias` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `alias = TextBuffer { storage: [195u8, 169u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8], used: 2 };`
- `tests/omega/pass/write_only/returned_destination_transport_exit/main.omg`:31 — `destination` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `destination = 42;`
- `tests/omega/pass/write_only/returned_destination_transport_exit/main.omg`:50 — `moved` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `moved = 47;`
- `tests/omega/run/borrows/indexed_loan_element_write/main.omg`:15 — `borrowed` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `borrowed = 9;`
- `tests/omega/run/borrows/local_mut_borrow/main.omg`:12 — `borrowed` — reassigning a reference carrier may need `mut` even when referent writes are legal
  - `borrowed = 9;`

## `tests/omega` (853 hits)

### `tests/omega/fail/arithmetic/collection_result_distinct_windows/main.omg`

- L8: `small` — index store — fix: `let mut small`
  - `small[count]`
### `tests/omega/fail/arithmetic/collection_result_rebound_descriptor/main.omg`

- L11: `destination` — index store — fix: `let mut destination`
  - `destination[count]`
### `tests/omega/fail/arithmetic/collection_result_rebound_descriptor_in_later_state/main.omg`

- L16: `destination` — index store — fix: `let mut destination`
  - `destination[count]`
### `tests/omega/fail/atomics/fence_returned_view_case_invalidated/main.omg`

- L11: `options` — writable borrow — fix: `let mut options`
  - `let view: View = wrap(&mut options);`
### `tests/omega/fail/borrow/captured_selector_result_retires_written_bound/main.omg`

- L14: `result` — assignment — fix: `let mut result`
  - `result = 2;`
### `tests/omega/fail/borrow/multi_source_view_source_write/main.omg`

- L14: `selected` — assignment — fix: `let mut selected`
  - `selected = 2;`
### `tests/omega/fail/borrow/named_operator_effectful_argument_retires_bound/main.omg`

- L6: `items` — index store — fix: `let mut items`
  - `items[index]`
### `tests/omega/fail/borrow/named_operator_mutable_argument_retires_bound/main.omg`

- L10: `items` — index store — fix: `let mut items`
  - `items[index]`
### `tests/omega/fail/borrow/recast_selector_retires_entry_bound/main.omg`

- L5: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = false;`
### `tests/omega/fail/borrow/returned_write_destination_argument_read/main.omg`

- L4: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 42;`
### `tests/omega/fail/borrow/returned_write_destination_backing_conflict/main.omg`

- L6: `destination` — assignment **review** — fix: `let mut destination`
  - `destination = 42;`
### `tests/omega/fail/borrow/returned_write_destination_carrier_conflict/main.omg`

- L5: `moved` — assignment **review** — fix: `let mut moved`
  - `moved = 42;`
### `tests/omega/fail/borrow/selector_calls_do_not_share_write_frame/main.omg`

- L11: `slot` — assignment **review** — fix: `let mut slot`
  - `slot = 9;`
### `tests/omega/fail/borrow/write_only_stored_reference_constraint_loss/main.omg`

- L7: `value` — assignment **review** — fix: `let mut value`
  - `value = 23;`
### `tests/omega/fail/borrows/borrow_local_alias_active/main.omg`

- L8: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = 3;`
### `tests/omega/fail/borrows/borrow_unknown_index_pair_mut/main.omg`

- L13: `left` — assignment **review** — fix: `let mut left`
  - `left = Entry { value: 1 };`
- L14: `right` — assignment **review** — fix: `let mut right`
  - `right = Entry { value: 2 };`
### `tests/omega/fail/borrows/conditional_copied_slot_retains_parent_loan/main.omg`

- L24: `parent` — assignment **review** — fix: `let mut parent`
  - `parent = 9;`
### `tests/omega/fail/borrows/conditional_reference_replacement_false_first_stale_index/main.omg`

- L25: `values` — index store — fix: `let mut values`
  - `values[holder.value]`
### `tests/omega/fail/borrows/conditional_reference_replacement_true_first_stale_index/main.omg`

- L25: `values` — index store — fix: `let mut values`
  - `values[holder.value]`
### `tests/omega/fail/borrows/copied_borrowed_slot_retains_parent_loan/main.omg`

- L12: `parent` — assignment **review** — fix: `let mut parent`
  - `parent = 9;`
### `tests/omega/fail/borrows/dropped_reference_qualification_carrier_write/main.omg`

- L12: `held` — field store — fix: `let mut held`
  - `held.target = Ticket { number: 7 };`
### `tests/omega/fail/borrows/erased_input_before_returned_reference_live_source_write/main.omg`

- L8: `first` — assignment **review** — fix: `let mut first`
  - `first = 10;`
### `tests/omega/fail/borrows/erased_input_before_second_returned_reference_live_source_write/main.omg`

- L8: `chosen` — assignment **review** — fix: `let mut chosen`
  - `chosen = 10;`
### `tests/omega/fail/borrows/float_comparison_read_during_live_exclusive_loan/main.omg`

- L11: `raised` — assignment **review** — fix: `let mut raised`
  - `raised = 3.0;`
### `tests/omega/fail/borrows/forwarded_call_view_retains_operand_extent/main.omg`

- L11: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 70;`
### `tests/omega/fail/borrows/fresh_selector_borrow_has_no_parent_capture/main.omg`

- L15: `held` — assignment **review** — fix: `let mut held`
  - `held = 7;`
- L16: `fresh` — assignment **review** — fix: `let mut fresh`
  - `fresh = 11;`
### `tests/omega/fail/borrows/guard_observation_not_retired_during_guard/main.omg`

- L7: `saved` — field store — fix: `let mut saved`
  - `saved.value == 17 && mutate(&mut cell) -> true`
- L8: `saved` — field store — fix: `let mut saved`
  - `saved.value == 19 -> false`
### `tests/omega/fail/borrows/indexed_exclusive_primitive_loan_preserves_field_invariant/main.omg`

- L8: `selected_value` — assignment **review** — fix: `let mut selected_value`
  - `selected_value = 2;`
### `tests/omega/fail/borrows/indexed_shared_primitive_loan_cannot_mutate/main.omg`

- L10: `selected_value` — writable borrow — fix: `let mut selected_value`
  - `change(&mut selected_value);`
### `tests/omega/fail/borrows/local_shared_view_mutable_reference_result/main.omg`

- L8: `records` — writable borrow — fix: `let mut records`
  - `true -> &mut records[1]`
- L9: `records` — writable borrow — fix: `let mut records`
  - `false -> &mut records[0]`
### `tests/omega/fail/borrows/multi_origin_returned_reference_live_source_write/main.omg`

- L15: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 1;`
### `tests/omega/fail/borrows/mutable_primitive_reference_live_owner_read/main.omg`

- L9: `borrowed` — assignment **review** — fix: `let mut borrowed`
  - `borrowed = 9;`
### `tests/omega/fail/borrows/nested_array_reference_call_stale_index/main.omg`

- L15: `values` — index store — fix: `let mut values`
  - `values[external]`
### `tests/omega/fail/borrows/nested_machine_carried_recurrent_selector_false_guarantee_rejected/main.omg`

- L24: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 99;`
### `tests/omega/fail/borrows/nested_reference_call_stale_index/main.omg`

- L14: `values` — index store — fix: `let mut values`
  - `values[external]`
### `tests/omega/fail/borrows/parent_use_before_disjoint_sibling_ends/main.omg`

- L10: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L14: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `tests/omega/fail/borrows/plain_read_during_live_exclusive_loan/main.omg`

- L36: `borrowed` — assignment **review** — fix: `let mut borrowed`
  - `borrowed = 9;`
### `tests/omega/fail/borrows/read_positions_during_live_exclusive_loan/main.omg`

- L23: `borrowed` — assignment **review** — fix: `let mut borrowed`
  - `borrowed = 9;`
- L31: `lent` — assignment **review** — fix: `let mut lent`
  - `lent = 9;`
- L39: `held` — assignment **review** — fix: `let mut held`
  - `held = 9;`
### `tests/omega/fail/borrows/record_view_field_backing_write_while_stored/main.omg`

- L14: `room` — field store — fix: `let mut room`
  - `room.label = main.buffer[0..2];`
- L16: `room` — field store — fix: `let mut room`
  - `room.label == "AB"`
### `tests/omega/fail/borrows/reference_array_shared_cohort_live_source_write/main.omg`

- L13: `messages` — index store — fix: `let mut messages`
  - `messages[1].value`
### `tests/omega/fail/borrows/region_store_live_sibling_reference/main.omg`

- L14: `parent` — assignment **review** — fix: `let mut parent`
  - `parent = 9;`
### `tests/omega/fail/borrows/returned_descriptor_does_not_select_pointee/main.omg`

- L16: `values` — index store — fix: `let mut values`
  - `values[0].value`
### `tests/omega/fail/borrows/returned_shared_reference_case_write/main.omg`

- L11: `selected` — field store — fix: `let mut selected`
  - `selected.choice = Choice::Branch;`
### `tests/omega/fail/borrows/selected_reference_live_nested_parent_write/main.omg`

- L12: `second_parent` — assignment **review** — fix: `let mut second_parent`
  - `second_parent = 0;`
- L13: `nested` — assignment **review** — fix: `let mut nested`
  - `nested = 1;`
### `tests/omega/fail/borrows/selected_reference_live_reborrow_parent_write/main.omg`

- L9: `second_view` — assignment **review** — fix: `let mut second_view`
  - `second_view = 0;`
- L10: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 1;`
### `tests/omega/fail/borrows/stored_fixed_array_shared_write/main.omg`

- L5: `buffer` — writable borrow — fix: `let mut buffer`
  - `overwrite(&mut buffer.values);`
### `tests/omega/fail/borrows/whole_mut_receiver_read_under_live_local_loan/main.omg`

- L21: `other_view` — assignment **review** — fix: `let mut other_view`
  - `other_view = 4;`
- L28: `view` — assignment **review** — fix: `let mut view`
  - `view = 3;`
### `tests/omega/fail/build/boundary_acceptance_fabricated_activation/build.omg`

- L4: `counterfeit` — writable borrow — fix: `let mut counterfeit`
  - `Build::accept_boundary<Boundary>(&mut counterfeit);`
### `tests/omega/fail/build/build_log_counterfeit_facet_refused/build.omg`

- L12: `counterfeit` — writable borrow — fix: `let mut counterfeit`
  - `BuildLog::write_line(&mut counterfeit, "counterfeit line");`
### `tests/omega/fail/build/representation_selection_fabricated_activation/build.omg`

- L4: `counterfeit` — writable borrow — fix: `let mut counterfeit`
  - `Build::select_representation<Token, TokenRepresentation>(&mut counterfeit);`
### `tests/omega/fail/calls/plain_let_reassign_rejected/main.omg`

- L11: `x` — assignment — fix: `let mut x`
  - `x = 2;`
### `tests/omega/fail/constraints/computed_local_value_invalidated/main.omg`

- L7: `saved` — writable borrow — fix: `let mut saved`
  - `replace(&mut saved);`
### `tests/omega/fail/contracts/borrowed_guarantee_overlapping_field_write_rejected/main.omg`

- L20: `held` — assignment **review** — fix: `let mut held`
  - `held = 73;`
- L34: `held` — assignment **review** — fix: `let mut held`
  - `held = 73;`
### `tests/omega/fail/contracts/referent_replacement_false_guarantee/main.omg`

- L9: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = Pair { first: Child { value: 8 }, second: 9 };`
### `tests/omega/fail/control_flow/anonymous_transition_undefined_divisor/main.omg`

- L20: `held` — writable borrow — fix: `let mut held`
  - `CapturedLocal::fill(&mut held);`
### `tests/omega/fail/dependent/data_where_field_free_conjunct_unestablished/main.omg`

- L6: `pending` — field store — fix: `let mut pending`
  - `pending.count = 1;`
### `tests/omega/fail/domains/qualified_fixed_byte_array_shared_local_write/main.omg`

- L6: `view` — index store — fix: `let mut view`
  - `view[0]`
### `tests/omega/fail/domains/returned_fixed_array_ascii_write_invalidates/main.omg`

- L7: `owner` — writable borrow — fix: `let mut owner`
  - `let selected: &mut [u8; 8] in Ascii = relay(&mut owner);`
### `tests/omega/fail/domains/returned_fixed_array_utf8_split_invalidates/main.omg`

- L7: `owner` — writable borrow — fix: `let mut owner`
  - `let selected: &mut [u8; 8] in Utf8 = relay(&mut owner);`
### `tests/omega/fail/drops/state_loan_alias_invalidates_nominal_cleanup_prerequisite/main.omg`

- L5: `room` — writable borrow — fix: `let mut room`
  - `let loan: &mut Room = &mut room;`
### `tests/omega/fail/drops/state_loan_invalidates_nominal_cleanup_prerequisite/main.omg`

- L5: `room` — writable borrow — fix: `let mut room`
  - `transition { _ -> observe(&mut room) }`
### `tests/omega/fail/drops/transitive_state_loan_cleanup_prerequisite/main.omg`

- L5: `room` — writable borrow — fix: `let mut room`
  - `transition { _ -> first(&mut room) }`
### `tests/omega/fail/facts/array_element_default_domain/call_result/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `tests/omega/fail/facts/array_element_default_domain/const_name/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `tests/omega/fail/facts/array_element_default_domain/field/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `tests/omega/fail/facts/array_element_default_domain/local/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `tests/omega/fail/facts/array_element_default_domain/match_join/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `tests/omega/fail/facts/array_element_default_domain/other_machine/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `tests/omega/fail/facts/array_element_default_domain/other_state/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `tests/omega/fail/facts/array_element_default_domain/parameter/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `tests/omega/fail/facts/array_element_default_domain/target_body/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `tests/omega/fail/facts/assumed_binding_input_authority_through_call_result_rejected/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `tests/omega/fail/facts/assumed_binding_input_authority_through_const_name_rejected/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `tests/omega/fail/facts/assumed_binding_input_authority_through_field_rejected/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `tests/omega/fail/facts/assumed_binding_input_authority_through_local_rejected/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `tests/omega/fail/facts/assumed_binding_input_authority_through_match_join_rejected/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `tests/omega/fail/facts/assumed_binding_input_authority_through_other_machine_rejected/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `tests/omega/fail/facts/assumed_binding_input_authority_through_other_state_rejected/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `tests/omega/fail/facts/assumed_binding_input_authority_through_parameter_rejected/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `tests/omega/fail/facts/assumed_binding_input_authority_through_target_body_rejected/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `tests/omega/fail/facts/assumed_binding_result_bound_through_match_join_rejected/main.omg`

- L9: `values` — index store — fix: `let mut values`
  - `values[index]`
### `tests/omega/fail/facts/assumed_binding_result_bound_through_other_machine_rejected/main.omg`

- L9: `values` — index store — fix: `let mut values`
  - `values[index]`
### `tests/omega/fail/facts/assumed_binding_result_bound_through_target_body_rejected/main.omg`

- L31: `values` — index store — fix: `let mut values`
  - `values[Required::get()]`
- L39: `values` — index store — fix: `let mut values`
  - `values[Required::get()]`
### `tests/omega/fail/facts/case_payload_exclusive_loan_through_call_result_rejected/main.omg`

- L26: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/case_payload_exclusive_loan_through_const_name_rejected/main.omg`

- L25: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/case_payload_exclusive_loan_through_field_rejected/main.omg`

- L25: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/case_payload_exclusive_loan_through_local_rejected/main.omg`

- L22: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/case_payload_exclusive_loan_through_match_join_rejected/main.omg`

- L28: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/case_payload_exclusive_loan_through_other_machine_rejected/main.omg`

- L19: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/case_payload_exclusive_loan_through_other_state_rejected/main.omg`

- L24: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/case_payload_exclusive_loan_through_parameter_rejected/main.omg`

- L19: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/case_payload_exclusive_loan_through_target_body_rejected/main.omg`

- L48: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
- L64: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/case_predicate_alias_sibling_restore_rejected/main.omg`

- L15: `low` — assignment **review** — fix: `let mut low`
  - `low = 31;`
### `tests/omega/fail/facts/case_predicate_recast_sibling_restore_rejected/main.omg`

- L15: `view` — assignment **review** — fix: `let mut view`
  - `view = 31;`
### `tests/omega/fail/facts/checked_predicate_disjoint_loans_through_call_result_rejected/main.omg`

- L13: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L14: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `tests/omega/fail/facts/checked_predicate_disjoint_loans_through_const_name_rejected/main.omg`

- L11: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L12: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `tests/omega/fail/facts/checked_predicate_disjoint_loans_through_field_rejected/main.omg`

- L12: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L13: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `tests/omega/fail/facts/checked_predicate_disjoint_loans_through_local_rejected/main.omg`

- L12: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L13: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `tests/omega/fail/facts/checked_predicate_disjoint_loans_through_match_join_rejected/main.omg`

- L13: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L14: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `tests/omega/fail/facts/checked_predicate_disjoint_loans_through_other_machine_rejected/main.omg`

- L11: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L12: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `tests/omega/fail/facts/checked_predicate_disjoint_loans_through_other_state_rejected/main.omg`

- L16: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L17: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `tests/omega/fail/facts/checked_predicate_disjoint_loans_through_parameter_rejected/main.omg`

- L11: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L12: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `tests/omega/fail/facts/checked_predicate_disjoint_loans_through_target_body_rejected/main.omg`

- L37: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L38: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
- L49: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L50: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `tests/omega/fail/facts/computed_subslice_extent_stale_field/main.omg`

- L9: `view` — index store — fix: `let mut view`
  - `view[1]`
### `tests/omega/fail/facts/copied_case_before_later_argument_through_other_state_rejected/main.omg`

- L21: `chosen` — writable borrow — fix: `let mut chosen`
  - `let observed: u64 = read_payload(chosen, set_to_absent(&mut chosen));`
### `tests/omega/fail/facts/exclusive_match_arm_through_call_result_rejected/main.omg`

- L25: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/exclusive_match_arm_through_const_name_rejected/main.omg`

- L21: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/exclusive_match_arm_through_field_rejected/main.omg`

- L20: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/exclusive_match_arm_through_local_rejected/main.omg`

- L19: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/exclusive_match_arm_through_match_join_rejected/main.omg`

- L24: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/exclusive_match_arm_through_other_machine_rejected/main.omg`

- L18: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/exclusive_match_arm_through_other_state_rejected/main.omg`

- L20: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/exclusive_match_arm_through_parameter_rejected/main.omg`

- L18: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/exclusive_match_arm_through_target_body_rejected/main.omg`

- L18: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
- L30: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
- L42: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
- L54: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/fixed_array_eight_elem_index_bound_through_local_rejected/main.omg`

- L9: `local_values` — index store — fix: `let mut local_values`
  - `local_values[local_index]`
### `tests/omega/fail/facts/fixed_array_onethousandtwentyfour_elem_index_bound_through_local_rejected/main.omg`

- L10: `local_arr` — index store — fix: `let mut local_arr`
  - `local_arr[local_idx]`
### `tests/omega/fail/facts/fixed_array_twofiftysix_elem_index_bound_through_local_rejected/main.omg`

- L10: `local_arr` — index store — fix: `let mut local_arr`
  - `local_arr[local_idx]`
### `tests/omega/fail/facts/fixed_array_value_through_local_rejected/main.omg`

- L9: `local_values` — index store — fix: `let mut local_values`
  - `local_values[local_index]`
### `tests/omega/fail/facts/fixed_layout_reference_readback_through_const_name_wrong_region_rejected/main.omg`

- L36: `result` — field store — fix: `let mut result`
  - `result.value == 7u64 && result.pointer.value == 43u64`
### `tests/omega/fail/facts/fixed_layout_reference_readback_through_local_wrong_region_rejected/main.omg`

- L33: `result` — field store — fix: `let mut result`
  - `result.value == 7u64 && result.pointer.value == 43u64`
### `tests/omega/fail/facts/fixed_layout_reference_readback_through_match_join_wrong_region_rejected/main.omg`

- L37: `result` — field store — fix: `let mut result`
  - `result.value == 7u64 && result.pointer.value == 43u64 && result.selected_first == choose_first`
### `tests/omega/fail/facts/fixed_layout_reference_readback_through_other_machine_wrong_region_rejected/main.omg`

- L35: `forwarded` — field store — fix: `let mut forwarded`
  - `forwarded.value == 7u64 && forwarded.pointer.value == 43u64`
### `tests/omega/fail/facts/fixed_layout_reference_readback_through_other_state_wrong_region_rejected/main.omg`

- L35: `result` — field store — fix: `let mut result`
  - `result.value == 7u64 && result.pointer.value == 43u64`
### `tests/omega/fail/facts/fixed_layout_reference_readback_wrong_policy_rejected/main.omg`

- L32: `result` — field store — fix: `let mut result`
  - `result.value == 7u64 && result.pointer.value == 43u64`
### `tests/omega/fail/facts/fixed_layout_reference_readback_wrong_region_rejected/main.omg`

- L32: `result` — field store — fix: `let mut result`
  - `result.value == 7u64 && result.pointer.value == 43u64`
### `tests/omega/fail/facts/foreign_indirect_float_record_result_through_match_join_rejected/main.omg`

- L46: `selected` — field store — fix: `let mut selected`
  - `selected.branch == choice && receive(selected.value)`
### `tests/omega/fail/facts/indexed_element_mut_loan_through_call_result_rejected/main.omg`

- L16: `rooms` — writable borrow — fix: `let mut rooms`
  - `let held: &mut Room = &mut rooms[selected];`
- L17: `rooms` — writable borrow — fix: `let mut rooms`
  - `bump(&mut rooms[selected]);`
### `tests/omega/fail/facts/indexed_element_mut_loan_through_const_name_rejected/main.omg`

- L14: `rooms` — writable borrow — fix: `let mut rooms`
  - `let held: &mut Room = &mut rooms[selected];`
- L15: `rooms` — writable borrow — fix: `let mut rooms`
  - `bump(&mut rooms[selected]);`
### `tests/omega/fail/facts/indexed_element_mut_loan_through_field_rejected/main.omg`

- L14: `rooms` — writable borrow — fix: `let mut rooms`
  - `let held: &mut Room = &mut rooms[selector.index];`
- L15: `rooms` — writable borrow — fix: `let mut rooms`
  - `bump(&mut rooms[selector.index]);`
### `tests/omega/fail/facts/indexed_element_mut_loan_through_local_rejected/main.omg`

- L13: `rooms` — writable borrow — fix: `let mut rooms`
  - `let held: &mut Room = &mut rooms[selected];`
- L14: `rooms` — writable borrow — fix: `let mut rooms`
  - `bump(&mut rooms[selected]);`
### `tests/omega/fail/facts/indexed_element_mut_loan_through_match_join_rejected/main.omg`

- L13: `rooms` — writable borrow — fix: `let mut rooms`
  - `let held: &mut Room = &mut rooms[selected];`
- L14: `rooms` — writable borrow — fix: `let mut rooms`
  - `bump(&mut rooms[selected]);`
### `tests/omega/fail/facts/indexed_element_mut_loan_through_other_machine_rejected/main.omg`

- L22: `rooms` — writable borrow — fix: `let mut rooms`
  - `forward(&mut rooms, from_index);`
### `tests/omega/fail/facts/indexed_element_mut_loan_through_other_state_rejected/main.omg`

- L12: `rooms` — writable borrow — fix: `let mut rooms`
  - `transition { _ -> apply(&mut rooms, from_index) }`
- L16: `rooms` — writable borrow — fix: `let mut rooms`
  - `let held: &mut Room = &mut rooms[selected];`
- L17: `rooms` — writable borrow — fix: `let mut rooms`
  - `bump(&mut rooms[selected]);`
### `tests/omega/fail/facts/indexed_element_mut_loan_through_parameter_rejected/main.omg`

- L19: `rooms` — writable borrow — fix: `let mut rooms`
  - `apply(&mut rooms, from_index);`
### `tests/omega/fail/facts/indexed_element_mut_loan_through_target_body_rejected/main.omg`

- L40: `rooms` — writable borrow — fix: `let mut rooms`
  - `apply(&mut rooms, from_index);`
### `tests/omega/fail/facts/indexed_recoverable_replacement_window_through_call_result_rejected/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `tests/omega/fail/facts/indexed_recoverable_replacement_window_through_const_name_rejected/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `tests/omega/fail/facts/indexed_recoverable_replacement_window_through_field_rejected/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `tests/omega/fail/facts/indexed_recoverable_replacement_window_through_local_rejected/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `tests/omega/fail/facts/indexed_recoverable_replacement_window_through_match_join_rejected/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `tests/omega/fail/facts/indexed_recoverable_replacement_window_through_other_machine_rejected/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `tests/omega/fail/facts/indexed_recoverable_replacement_window_through_other_state_rejected/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `tests/omega/fail/facts/indexed_recoverable_replacement_window_through_parameter_rejected/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `tests/omega/fail/facts/indexed_recoverable_replacement_window_through_target_body_rejected/main.omg`

- L59: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
- L73: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `tests/omega/fail/facts/indexed_storage_address_identity_through_call_result_rejected/main.omg`

- L9: `held` — assignment **review** — fix: `let mut held`
  - `held = 13;`
### `tests/omega/fail/facts/indexed_storage_address_identity_through_const_name_rejected/main.omg`

- L9: `held` — assignment **review** — fix: `let mut held`
  - `held = 13;`
### `tests/omega/fail/facts/indexed_storage_address_identity_through_field_rejected/main.omg`

- L9: `held` — assignment **review** — fix: `let mut held`
  - `held = 13;`
### `tests/omega/fail/facts/indexed_storage_address_identity_through_local_rejected/main.omg`

- L8: `held` — assignment **review** — fix: `let mut held`
  - `held = 13;`
### `tests/omega/fail/facts/indexed_storage_address_identity_through_match_join_rejected/main.omg`

- L8: `held` — assignment **review** — fix: `let mut held`
  - `held = 13;`
### `tests/omega/fail/facts/indexed_storage_address_identity_through_other_machine_rejected/main.omg`

- L7: `held` — assignment **review** — fix: `let mut held`
  - `held = 13;`
### `tests/omega/fail/facts/indexed_storage_address_identity_through_other_state_rejected/main.omg`

- L9: `held` — assignment **review** — fix: `let mut held`
  - `held = 13;`
### `tests/omega/fail/facts/indexed_storage_address_identity_through_parameter_rejected/main.omg`

- L6: `held` — assignment **review** — fix: `let mut held`
  - `held = 13;`
### `tests/omega/fail/facts/indexed_storage_address_identity_through_target_body_rejected/main.omg`

- L36: `held` — assignment **review** — fix: `let mut held`
  - `held = 13;`
- L55: `held` — assignment **review** — fix: `let mut held`
  - `held = 13;`
### `tests/omega/fail/facts/length_through_call_result_rejected/main.omg`

- L13: `window` — index store — fix: `let mut window`
  - `window[length - 1]`
### `tests/omega/fail/facts/length_through_const_name_rejected/main.omg`

- L9: `window` — index store — fix: `let mut window`
  - `window[LENGTH - 1]`
### `tests/omega/fail/facts/length_through_field_rejected/main.omg`

- L10: `window` — index store — fix: `let mut window`
  - `window[meter.length - 1]`
### `tests/omega/fail/facts/length_through_local_rejected/main.omg`

- L9: `window` — index store — fix: `let mut window`
  - `window[length - 1]`
### `tests/omega/fail/facts/length_through_match_join_rejected/main.omg`

- L9: `window` — index store — fix: `let mut window`
  - `window[length - 1]`
### `tests/omega/fail/facts/match_computed_root_through_call_result_rejected/main.omg`

- L26: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/match_computed_root_through_const_name_rejected/main.omg`

- L23: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/match_computed_root_through_field_rejected/main.omg`

- L25: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/match_computed_root_through_local_rejected/main.omg`

- L22: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/match_computed_root_through_match_join_rejected/main.omg`

- L27: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/match_computed_root_through_other_machine_rejected/main.omg`

- L19: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/match_computed_root_through_other_state_rejected/main.omg`

- L21: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/match_computed_root_through_parameter_rejected/main.omg`

- L19: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/match_computed_root_through_target_body_rejected/main.omg`

- L19: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/match_effectful_subject_custody_through_other_state_rejected/main.omg`

- L28: `slot` — writable borrow — fix: `let mut slot`
  - `let held: Token = match refill(&mut slot, c, flag) {`
### `tests/omega/fail/facts/nested_zero_leaf_array_storage_through_call_result_rejected/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `tests/omega/fail/facts/nested_zero_leaf_array_storage_through_const_name_rejected/main.omg`

- L12: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `tests/omega/fail/facts/nested_zero_leaf_array_storage_through_field_rejected/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `tests/omega/fail/facts/nested_zero_leaf_array_storage_through_local_rejected/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `tests/omega/fail/facts/nested_zero_leaf_array_storage_through_match_join_rejected/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `tests/omega/fail/facts/nested_zero_leaf_array_storage_through_other_machine_rejected/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `tests/omega/fail/facts/nested_zero_leaf_array_storage_through_other_state_rejected/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `tests/omega/fail/facts/nested_zero_leaf_array_storage_through_parameter_rejected/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `tests/omega/fail/facts/nested_zero_leaf_array_storage_through_target_body_rejected/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `tests/omega/fail/facts/nested_zero_leaf_array_storage_zeroed_source_rejected/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `tests/omega/fail/facts/recoverable_replacement_window_through_call_result_rejected/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `tests/omega/fail/facts/recoverable_replacement_window_through_const_name_rejected/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `tests/omega/fail/facts/recoverable_replacement_window_through_field_rejected/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `tests/omega/fail/facts/recoverable_replacement_window_through_local_rejected/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `tests/omega/fail/facts/recoverable_replacement_window_through_match_join_rejected/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `tests/omega/fail/facts/recoverable_replacement_window_through_other_machine_rejected/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `tests/omega/fail/facts/recoverable_replacement_window_through_other_state_rejected/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `tests/omega/fail/facts/recoverable_replacement_window_through_parameter_rejected/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `tests/omega/fail/facts/recoverable_replacement_window_through_target_body_rejected/main.omg`

- L58: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
- L75: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `tests/omega/fail/facts/reference_arrival_handoff_through_call_result_rejected/main.omg`

- L9: `original` — assignment **review** — fix: `let mut original`
  - `original = 7;`
- L10: `child` — assignment **review** — fix: `let mut child`
  - `child = 41;`
### `tests/omega/fail/facts/reference_arrival_handoff_through_const_name_rejected/main.omg`

- L9: `original` — assignment **review** — fix: `let mut original`
  - `original = 7;`
- L10: `child` — assignment **review** — fix: `let mut child`
  - `child = WRITTEN;`
### `tests/omega/fail/facts/reference_arrival_handoff_through_field_rejected/main.omg`

- L9: `original` — assignment **review** — fix: `let mut original`
  - `original = 7;`
- L10: `holder` — field store — fix: `let mut holder`
  - `holder.slot = 41;`
### `tests/omega/fail/facts/reference_arrival_handoff_through_local_rejected/main.omg`

- L8: `original` — assignment **review** — fix: `let mut original`
  - `original = 7;`
- L9: `child` — assignment **review** — fix: `let mut child`
  - `child = 41;`
### `tests/omega/fail/facts/reference_arrival_handoff_through_match_join_rejected/main.omg`

- L15: `child` — assignment **review** — fix: `let mut child`
  - `child = 41;`
### `tests/omega/fail/facts/reference_arrival_handoff_through_other_machine_rejected/main.omg`

- L11: `held` — assignment **review** — fix: `let mut held`
  - `held = 7;`
### `tests/omega/fail/facts/reference_arrival_handoff_through_parameter_rejected/main.omg`

- L10: `held` — assignment **review** — fix: `let mut held`
  - `held = 7;`
### `tests/omega/fail/facts/reference_arrival_handoff_through_target_body_rejected/main.omg`

- L13: `held` — assignment **review** — fix: `let mut held`
  - `held = 7;`
### `tests/omega/fail/facts/replaced_field_loan_through_call_result_rejected/main.omg`

- L22: `selected` — writable borrow — fix: `let mut selected`
  - `let written: bool = store(&mut selected.body, 41);`
### `tests/omega/fail/facts/replaced_field_loan_through_field_rejected/main.omg`

- L22: `wrapper` — writable borrow — fix: `let mut wrapper`
  - `let written: bool = store(&mut wrapper.message.body, 41);`
### `tests/omega/fail/facts/replaced_field_loan_through_match_join_rejected/main.omg`

- L21: `selected` — writable borrow — fix: `let mut selected`
  - `let written: bool = store(&mut selected.body, 41);`
### `tests/omega/fail/facts/returned_write_destination_through_call_result_rejected/main.omg`

- L9: `destination` — assignment **review** — fix: `let mut destination`
  - `destination = destination;`
### `tests/omega/fail/facts/returned_write_destination_through_const_name_rejected/main.omg`

- L8: `destination` — assignment **review** — fix: `let mut destination`
  - `destination = destination;`
### `tests/omega/fail/facts/returned_write_destination_through_field_rejected/main.omg`

- L9: `destination` — field store — fix: `let mut destination`
  - `destination.loan = destination.loan;`
### `tests/omega/fail/facts/returned_write_destination_through_local_rejected/main.omg`

- L8: `destination` — assignment **review** — fix: `let mut destination`
  - `destination = destination;`
### `tests/omega/fail/facts/returned_write_destination_through_match_join_rejected/main.omg`

- L6: `destination` — assignment **review** — fix: `let mut destination`
  - `destination = destination;`
### `tests/omega/fail/facts/returned_write_destination_through_target_body_rejected/main.omg`

- L44: `destination` — assignment **review** — fix: `let mut destination`
  - `destination = destination;`
### `tests/omega/fail/facts/signed_division_remainder_through_call_result_rejected/main.omg`

- L18: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `tests/omega/fail/facts/signed_division_remainder_through_const_name_rejected/main.omg`

- L13: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `tests/omega/fail/facts/signed_division_remainder_through_field_rejected/main.omg`

- L16: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `tests/omega/fail/facts/signed_division_remainder_through_local_rejected/main.omg`

- L13: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `tests/omega/fail/facts/signed_division_remainder_through_match_join_rejected/main.omg`

- L21: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `tests/omega/fail/facts/signed_division_remainder_through_other_machine_rejected/main.omg`

- L11: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `tests/omega/fail/facts/signed_division_remainder_through_other_state_rejected/main.omg`

- L17: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `tests/omega/fail/facts/signed_division_remainder_through_parameter_rejected/main.omg`

- L11: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `tests/omega/fail/facts/signed_division_remainder_through_target_body_rejected/main.omg`

- L16: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
- L26: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `tests/omega/fail/facts/write_only_indexed_reborrow_through_field_rejected/main.omg`

- L28: `held` — writable borrow — fix: `let mut held`
  - `replace(&write held.cells[storage.position], 7);`
- L29: `held` — writable borrow — fix: `let mut held`
  - `replace(&write held.cells[storage.position], 9);`
- L31: `held` — writable borrow — fix: `let mut held`
  - `replace(&write held.cells[storage.position], -43);`
### `tests/omega/fail/facts/write_only_match_join_through_call_result_rejected/main.omg`

- L27: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/write_only_match_join_through_const_name_rejected/main.omg`

- L26: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/write_only_match_join_through_field_rejected/main.omg`

- L26: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/write_only_match_join_through_local_rejected/main.omg`

- L23: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/write_only_match_join_through_match_join_rejected/main.omg`

- L29: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/write_only_match_join_through_other_machine_rejected/main.omg`

- L22: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/write_only_match_join_through_other_state_rejected/main.omg`

- L26: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/write_only_match_join_through_parameter_rejected/main.omg`

- L22: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/facts/write_only_match_join_through_target_body_rejected/main.omg`

- L51: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
- L66: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/fail/generics/returned_applied_field_does_not_widen_access/main.omg`

- L7: `returned` — writable borrow — fix: `let mut returned`
  - `mutate(&mut returned.values);`
### `tests/omega/fail/generics/runtime_index_carrier_array_mismatch/main.omg`

- L5: `wrong` — index store — fix: `let mut wrong`
  - `wrong[0].value`
### `tests/omega/fail/host/descriptor_read_guarded_prefix_without_admitted_abi/main.omg`

- L28: `packet` — writable borrow — fix: `let mut packet`
  - `let moved: i64 = block DescriptorIo::read(0, &mut packet, 1);`
### `tests/omega/fail/layouts/callee_integer_at_multi_origin_reference_requires_fit/main.omg`

- L43: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = value;`
### `tests/omega/fail/layouts/captured_encoded_reference_replacement_requires_fit/main.omg`

- L43: `captured` — assignment **review** — fix: `let mut captured`
  - `captured = 128;`
- L45: `replacement` — assignment **review** — fix: `let mut replacement`
  - `replacement = 42;`
### `tests/omega/fail/layouts/demanded_partial_target_layout/main.omg`

- L13: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "macos_arm64" -> on_macos_arm64(schema)`
### `tests/omega/fail/layouts/direct_encoded_assignment_requires_fit/main.omg`

- L42: `leaf` — assignment **review** — fix: `let mut leaf`
  - `leaf = 200;`
- L44: `ordinary` — assignment **review** — fix: `let mut ordinary`
  - `ordinary = 127;`
### `tests/omega/fail/layouts/direct_unselected_ordinary_layout_contract/main.omg`

- L12: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "macos_arm64" -> on_macos_arm64(schema)`
- L13: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "linux_x86_64" -> on_linux_x86_64(schema)`
- L14: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "linux_arm64" -> on_linux_arm64(schema)`
- L15: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "windows_x86_64" -> on_windows_x86_64(schema)`
### `tests/omega/fail/layouts/placed_external_array_denied_read/main.omg`

- L51: `updated` — index store — fix: `let mut updated`
  - `updated[0] == 17 && updated[256] == 97 && updated[511] == 73 {`
### `tests/omega/fail/layouts/placed_primitive_shared_source/main.omg`

- L30: `view` — writable borrow — fix: `let mut view`
  - `write(&mut view.number, 29u64);`
### `tests/omega/fail/layouts/target_layout_call_checks_all_widths/main.omg`

- L10: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "macos_arm64" -> on_macos_arm64(schema)`
- L11: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "linux_x86_64" -> on_linux_x86_64(schema)`
### `tests/omega/fail/layouts/target_layout_invalid_unselected_member/main.omg`

- L10: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "macos_arm64" -> on_macos_arm64(schema)`
- L11: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "linux_x86_64" -> on_linux_x86_64(schema)`
### `tests/omega/fail/layouts/target_layout_recast_checks_all_extents/main.omg`

- L10: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "macos_arm64" -> on_macos_arm64(schema)`
- L11: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "linux_x86_64" -> on_linux_x86_64(schema)`
### `tests/omega/fail/layouts/unselected_ordinary_provider_body_type/main.omg`

- L12: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "macos_arm64" -> on_macos_arm64(schema)`
- L13: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "linux_x86_64" -> on_linux_x86_64(schema)`
- L14: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "linux_arm64" -> on_linux_arm64(schema)`
- L15: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "windows_x86_64" -> on_windows_x86_64(schema)`
### `tests/omega/fail/layouts/unselected_ordinary_provider_crash_ceiling/main.omg`

- L12: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "macos_arm64" -> on_macos_arm64(schema)`
- L13: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "linux_x86_64" -> on_linux_x86_64(schema)`
- L14: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "linux_arm64" -> on_linux_arm64(schema)`
- L15: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "windows_x86_64" -> on_windows_x86_64(schema)`
### `tests/omega/fail/layouts/unselected_target_callback_layout_underflow/main.omg`

- L9: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "macos_arm64" -> on_macos_arm64(schema)`
- L10: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "linux_x86_64" -> on_linux_x86_64(schema)`
- L11: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "linux_arm64" -> on_linux_arm64(schema)`
- L12: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "windows_x86_64" -> on_windows_x86_64(schema)`
### `tests/omega/fail/layouts/unselected_target_helper_layout_underflow/main.omg`

- L9: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "macos_arm64" -> on_macos_arm64(schema)`
- L10: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "linux_x86_64" -> on_linux_x86_64(schema)`
- L11: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "linux_arm64" -> on_linux_arm64(schema)`
- L12: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "windows_x86_64" -> on_windows_x86_64(schema)`
### `tests/omega/fail/ownership/indexed_borrowed_window_wrong_place_repair/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `tests/omega/fail/ownership/whole_borrowed_forwarded_live_descendant/main.omg`

- L10: `forwarded` — assignment **review** — fix: `let mut forwarded`
  - `forwarded = removed;`
### `tests/omega/fail/proofs/completed_poststate_sibling_overwrite/main.omg`

- L13: `value` — writable borrow — fix: `let mut value`
  - `let changed: u64 = initialize(&mut value);`
- L14: `value` — field store — fix: `let mut value`
  - `value.second = 0;`
### `tests/omega/fail/proofs/declaration_byte_predicate_loop_write/main.omg`

- L7: `text` — writable borrow — fix: `let mut text`
  - `transition { _ -> repeat(&mut text) }`
- L10: `text` — field store — fix: `let mut text`
  - `text.bytes = [103, 255, 33];`
### `tests/omega/fail/proofs/declaration_predicate_captured_field/main.omg`

- L10: `counter` — writable borrow — fix: `let mut counter`
  - `let invalid: Pair in Small = Pair { first: counter.value, second: replace(&mut counter) };`
### `tests/omega/fail/proofs/declaration_predicate_cast_after_write/main.omg`

- L9: `value` — writable borrow — fix: `let mut value`
  - `let changed: u64 = widen(&mut value);`
### `tests/omega/fail/proofs/declaration_predicate_effectful_cast/main.omg`

- L12: `value` — writable borrow — fix: `let mut value`
  - `let converted: u8 = widen(&mut value) as u8;`
### `tests/omega/fail/proofs/reviewed_mathematical_data_contract_rejected/main.omg`

- L20: `step` — field store — fix: `let mut step`
  - `step.table_address == 0x12345000u64 && step.index == 7u64 &&`
- L21: `step` — field store — fix: `let mut step`
  - `step.entry_address == 0x12345038u64 && step.entry.physical_address == (word & 0x000FFFFFFFFFF000u64) &&`
### `tests/omega/fail/proofs/scalar_field_snapshot_after_alias_store/main.omg`

- L7: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = 7;`
### `tests/omega/fail/providers/applied_routed_parameter_mint/main.omg`

- L14: `output` — writable borrow — fix: `let mut output`
  - `refresh<Word>(&mut output);`
### `tests/omega/fail/providers/assumed_result_bound_match_join_rejected/main.omg`

- L9: `values` — index store — fix: `let mut values`
  - `values[index]`
### `tests/omega/fail/providers/assumed_result_bound_other_machine_rejected/main.omg`

- L9: `values` — index store — fix: `let mut values`
  - `values[index]`
### `tests/omega/fail/providers/assumed_result_bound_target_body_rejected/main.omg`

- L31: `values` — index store — fix: `let mut values`
  - `values[Required::get()]`
- L39: `values` — index store — fix: `let mut values`
  - `values[Required::get()]`
### `tests/omega/fail/providers/provider_selection_fabricated_activation/build.omg`

- L4: `counterfeit` — writable borrow — fix: `let mut counterfeit`
  - `Build::select_provider<Echo, EchoProvider>(&mut counterfeit, linux_arm64);`
- L5: `counterfeit` — writable borrow — fix: `let mut counterfeit`
  - `Build::select_provider<Echo, EchoProvider>(&mut counterfeit, linux_x86_64);`
- L6: `counterfeit` — writable borrow — fix: `let mut counterfeit`
  - `Build::select_provider<Echo, EchoProvider>(&mut counterfeit, macos_arm64);`
- L7: `counterfeit` — writable borrow — fix: `let mut counterfeit`
  - `Build::select_provider<Echo, EchoProvider>(&mut counterfeit, macos_x86_64);`
- L8: `counterfeit` — writable borrow — fix: `let mut counterfeit`
  - `Build::select_provider<Echo, EchoProvider>(&mut counterfeit, windows_x86_64);`
- L9: `counterfeit` — writable borrow — fix: `let mut counterfeit`
  - `Build::select_provider<Echo, EchoProvider>(&mut counterfeit, uefi_x86_64);`
- L10: `counterfeit` — writable borrow — fix: `let mut counterfeit`
  - `Build::select_provider<Echo, EchoProvider>(&mut counterfeit, cross_platform_cli);`
- L11: `counterfeit` — writable borrow — fix: `let mut counterfeit`
  - `Build::select_provider<Echo, EchoProvider>(&mut counterfeit, local_unchecked);`
- L12: `counterfeit` — writable borrow — fix: `let mut counterfeit`
  - `Build::select_provider<Echo, EchoProvider>(&mut counterfeit, alpha_bootstrap);`
### `tests/omega/fail/recast/borrowed_byte_collection_predicate/main.omg`

- L5: `value` — assignment **review** — fix: `let mut value`
  - `value = 18446744073709551615;`
### `tests/omega/fail/recast/borrowed_byte_mutable_source_facts/main.omg`

- L5: `value` — assignment **review** — fix: `let mut value`
  - `value = 18446744073709551615;`
### `tests/omega/fail/recast/borrowed_byte_shared_to_mutable/main.omg`

- L4: `value` — assignment **review** — fix: `let mut value`
  - `value = 42;`
### `tests/omega/fail/recast/byte_record_field_predicate/main.omg`

- L6: `value` — assignment **review** — fix: `let mut value`
  - `value = 18446744073709551615;`
### `tests/omega/fail/recast/non_owning_erasure_false_reestablishment/main.omg`

- L7: `view` — assignment **review** — fix: `let mut view`
  - `view = Storage { value: 73 };`
### `tests/omega/fail/recast/non_owning_erasure_joined_shared_store/main.omg`

- L4: `view` — field store — fix: `let mut view`
  - `view.value = 73;`
### `tests/omega/fail/recast/non_owning_erasure_mixed_write_presentations/main.omg`

- L8: `view` — assignment **review** — fix: `let mut view`
  - `view = Storage { value: 73 };`
### `tests/omega/fail/recast/non_owning_erasure_preserves_array_element/main.omg`

- L4: `view` — assignment **review** — fix: `let mut view`
  - `view = 73;`
### `tests/omega/fail/recast/non_owning_erasure_preserves_helper_field/main.omg`

- L7: `view` — assignment **review** — fix: `let mut view`
  - `view = Storage { value: 73 };`
### `tests/omega/fail/recast/non_owning_erasure_preserves_owned_field/main.omg`

- L8: `view` — assignment **review** — fix: `let mut view`
  - `view = Storage { value: 73 };`
### `tests/omega/fail/recast/non_owning_erasure_preserves_projected_view/main.omg`

- L6: `view` — assignment **review** — fix: `let mut view`
  - `view = 73;`
### `tests/omega/fail/recast/non_owning_erasure_preserves_reborrow_alias/main.omg`

- L6: `view` — assignment **review** — fix: `let mut view`
  - `view = 73;`
### `tests/omega/fail/recast/non_owning_erasure_preserves_record_field/main.omg`

- L7: `view` — assignment **review** — fix: `let mut view`
  - `view = Storage { value: 73 };`
### `tests/omega/fail/recast/non_owning_erasure_preserves_scalar_field/main.omg`

- L5: `view` — assignment **review** — fix: `let mut view`
  - `view = 73;`
### `tests/omega/fail/recast/non_owning_erasure_preserves_stored_alias/main.omg`

- L9: `view` — assignment **review** — fix: `let mut view`
  - `view = Storage { value: 73 };`
### `tests/omega/fail/recast/partial_ascii_recast_does_not_restore_field/main.omg`

- L8: `all` — assignment **review** — fix: `let mut all`
  - `all = 18446744073709551615;`
- L10: `prefix` — assignment **review** — fix: `let mut prefix`
  - `prefix = "safe";`
### `tests/omega/fail/recast/recast_mut_bool_as_unconstrained_byte/main.omg`

- L12: `flag_bits` — assignment **review** — fix: `let mut flag_bits`
  - `flag_bits = 0;`
### `tests/omega/fail/recast/recast_mut_where_bounded_field_widened/main.omg`

- L15: `bits` — assignment **review** — fix: `let mut bits`
  - `bits = 2;`
### `tests/omega/fail/recast/returned_shared_view_cannot_lend_mutable/main.omg`

- L14: `view` — writable borrow — fix: `let mut view`
  - `replace(&mut view);`
### `tests/omega/fail/recast/write_only_stored_pointer_read/main.omg`

- L5: `view` — assignment **review** — fix: `let mut view`
  - `view = 73;`
### `tests/omega/fail/slices/shared_view_arrival_cannot_mutate/main.omg`

- L3: `intermediate` — writable borrow — fix: `let mut intermediate`
  - `let output: &mut [i32] = &mut intermediate[..];`
### `tests/omega/fail/text/concatenation_into_borrowed_view_rejected/main.omg`

- L33: `room` — field store — fix: `let mut room`
  - `room.label = main.source.label;`
### `tests/omega/fail/text/qualified_byte_alias_replacement_invalid_encoding/main.omg`

- L5: `line` — assignment **review** — fix: `let mut line`
  - `line = "\xff\0\0\0\0\0\0\0";`
### `tests/omega/fail/text/qualified_local_byte_alias_replacement_invalid_encoding/main.omg`

- L5: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = "\xff\0\0\0\0\0\0\0";`
### `tests/omega/fail/time/instant_mutation_leaves_invalid_return/main.omg`

- L10: `instant` — writable borrow — fix: `let mut instant`
  - `let copied: Instant = corrupt(&mut instant);`
### `tests/omega/pass/arithmetic/nested_clamp_resolution_exit/main.omg`

- L30: `low` — field store — fix: `let mut low`
  - `low.neighbour == 73 && middle.neighbour == 79 && high.neighbour == 83 {`
### `tests/omega/pass/atomics/runtime_atomic_fence_captured_ordering_exit/main.omg`

- L15: `trace` — writable borrow — fix: `let mut trace`
  - `Atomic::fence(choose(&mut trace));`
### `tests/omega/pass/atomics/runtime_authored_fence_name_exit/main.omg`

- L10: `trace` — writable borrow — fix: `let mut trace`
  - `Atomic::fence(&mut trace, FenceOrdering::Publish);`
### `tests/omega/pass/borrow/explicit_aggregate_lifetime_selects_source/main.omg`

- L27: `selected` — field store — fix: `let mut selected`
  - `selected.body = 1;`
### `tests/omega/pass/borrow/mutable_selector_rhs_snapshot_exit/main.omg`

- L14: `result` — assignment — fix: `let mut result`
  - `result = 2;`
### `tests/omega/pass/borrow/mutable_selector_unchanged_exit/main.omg`

- L6: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = false;`
### `tests/omega/pass/borrow/named_operator_disjoint_argument_preserves_bound/main.omg`

- L11: `items` — index store — fix: `let mut items`
  - `items[index]`
### `tests/omega/pass/borrow/reference_parameter_referent_copy_exit/main.omg`

- L38: `returned_integer` — assignment **review** — fix: `let mut returned_integer`
  - `returned_integer = -29;`
- L42: `returned_boolean` — assignment **review** — fix: `let mut returned_boolean`
  - `returned_boolean = true;`
- L46: `returned_parameter_field` — assignment **review** — fix: `let mut returned_parameter_field`
  - `returned_parameter_field = true;`
### `tests/omega/pass/borrow/runtime_exclusive_loan_local_write_exit/main.omg`

- L19: `seen` — assignment **review** — fix: `let mut seen`
  - `seen = 0;`
### `tests/omega/pass/borrow/runtime_write_only_aggregate_selector_snapshot_exit/main.omg`

- L19: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = incoming;`
### `tests/omega/pass/borrow/runtime_write_only_captured_record_copy_exit/main.omg`

- L11: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = incoming;`
### `tests/omega/pass/borrow/runtime_write_only_exclusive_reference_call_result_exit/main.omg`

- L13: `slot` — assignment **review** — fix: `let mut slot`
  - `slot = identity(incoming);`
### `tests/omega/pass/borrow/runtime_write_only_reference_aggregate_alias_exit/main.omg`

- L13: `slot` — assignment **review** — fix: `let mut slot`
  - `slot = incoming;`
### `tests/omega/pass/borrow/runtime_write_only_reference_aggregate_call_result_exit/main.omg`

- L13: `slot` — assignment **review** — fix: `let mut slot`
  - `slot = forward(incoming);`
### `tests/omega/pass/borrows/borrow_domain_window_write_and_call/main.omg`

- L16: `outside` — assignment **review** — fix: `let mut outside`
  - `outside = 5;`
### `tests/omega/pass/borrows/borrow_returned_window_write_and_call/main.omg`

- L25: `outside` — assignment **review** — fix: `let mut outside`
  - `outside = 5;`
### `tests/omega/pass/borrows/borrow_stated_index_disequality_mut/main.omg`

- L22: `left` — assignment **review** — fix: `let mut left`
  - `left = Entry { value: 1 };`
- L23: `right` — assignment **review** — fix: `let mut right`
  - `right = Entry { value: 2 };`
### `tests/omega/pass/borrows/borrowed_window_parameter_provenance_exit/main.omg`

- L15: `slot` — assignment **review** — fix: `let mut slot`
  - `slot = incoming;`
- L63: `slot` — assignment **review** — fix: `let mut slot`
  - `slot = incoming;`
### `tests/omega/pass/borrows/conditional_copied_slot_restores_parent_exit/main.omg`

- L27: `parent` — assignment **review** — fix: `let mut parent`
  - `parent = 9;`
### `tests/omega/pass/borrows/copied_borrowed_slot_restores_parent_exit/main.omg`

- L16: `parent` — assignment **review** — fix: `let mut parent`
  - `parent = 9;`
### `tests/omega/pass/borrows/disjoint_branch_closure_with_grandchild/main.omg`

- L12: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 13 };`
- L13: `nested` — assignment **review** — fix: `let mut nested`
  - `nested = 17;`
- L14: `third` — assignment **review** — fix: `let mut third`
  - `third = Cell { value: 40 };`
### `tests/omega/pass/borrows/erased_input_before_returned_reference_exit/main.omg`

- L26: `first` — assignment **review** — fix: `let mut first`
  - `first = 10;`
- L29: `second` — field store — fix: `let mut second`
  - `second.target = 20;`
- L32: `third` — assignment **review** — fix: `let mut third`
  - `third = 30;`
- L35: `fourth` — field store — fix: `let mut fourth`
  - `fourth.first = 40;`
- L38: `fifth` — assignment **review** — fix: `let mut fifth`
  - `fifth = 50;`
### `tests/omega/pass/borrows/explicit_lifetime_select/main.omg`

- L25: `selected` — field store — fix: `let mut selected`
  - `selected.body = 1;`
### `tests/omega/pass/borrows/mut_loan_local_write_through_exit/main.omg`

- L13: `borrowed` — assignment **review** — fix: `let mut borrowed`
  - `borrowed = 9;`
### `tests/omega/pass/borrows/nested_byte_row_forwarded_window_exit/main.omg`

- L6: `selected` — index store — fix: `let mut selected`
  - `selected[0] == 21 && selected[1] == 22 && selected[2] == 23`
### `tests/omega/pass/borrows/nested_reference_call_checked_index_exit/main.omg`

- L65: `values` — index store — fix: `let mut values`
  - `values[external]`
### `tests/omega/pass/borrows/nested_reference_write_disjoint_facts_exit/main.omg`

- L18: `values` — index store — fix: `let mut values`
  - `values[external.retained]`
### `tests/omega/pass/borrows/plain_read_after_exclusive_loan_last_use_exit/main.omg`

- L37: `borrowed` — assignment **review** — fix: `let mut borrowed`
  - `borrowed = 9;`
- L42: `left` — assignment **review** — fix: `let mut left`
  - `left = 5;`
### `tests/omega/pass/borrows/qualified_owner_state_loan_exit/main.omg`

- L58: `view` — field store — fix: `let mut view`
  - `view.identity == 89`
- L102: `resources` — index store — fix: `let mut resources`
  - `resources[0].identity == 41 && resources[1].identity == 19`
- L112: `resources` — index store — fix: `let mut resources`
  - `resources[0].identity == 19 && returned.identity == 0`
- L126: `resources` — index store — fix: `let mut resources`
  - `resources[0].identity == 73 && resources[1].identity == 19`
- L135: `resources` — index store — fix: `let mut resources`
  - `resources[0].identity == 19 && identity == 0`
- L146: `resources` — index store — fix: `let mut resources`
  - `resources[0].identity == 19`
### `tests/omega/pass/borrows/region_stored_reference_occurrences_exit/main.omg`

- L17: `parent` — assignment **review** — fix: `let mut parent`
  - `parent = 9;`
### `tests/omega/pass/borrows/returned_primitive_reference_assignment_capture_exit/main.omg`

- L9: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 300;`
### `tests/omega/pass/borrows/runtime_affine_local_scalar_exit/main.omg`

- L15: `token` — field store — fix: `let mut token`
  - `token.value == comparison.value`
### `tests/omega/pass/borrows/runtime_disjoint_child_loans_restore_parent_exit/main.omg`

- L16: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L17: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
- L26: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 19 };`
- L27: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 51 };`
### `tests/omega/pass/borrows/runtime_index_mut_loan_write_exit/main.omg`

- L16: `borrowed` — assignment **review** — fix: `let mut borrowed`
  - `borrowed = 9;`
### `tests/omega/pass/borrows/runtime_mutable_primitive_reference_exit/main.omg`

- L8: `borrowed` — assignment **review** — fix: `let mut borrowed`
  - `borrowed = 9;`
- L12: `signed_reference` — assignment **review** — fix: `let mut signed_reference`
  - `signed_reference = -17;`
- L15: `flag_reference` — assignment **review** — fix: `let mut flag_reference`
  - `flag_reference = true;`
### `tests/omega/pass/borrows/runtime_mutable_reference_captures_selector_exit/main.omg`

- L8: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = Cell { value: 70 };`
### `tests/omega/pass/borrows/runtime_projected_call_loan_custody_exit/main.omg`

- L15: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 29;`
- L27: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 29;`
- L81: `first` — index store — fix: `let mut first`
  - `first[0].value == 19 && second[0].value == 19`
### `tests/omega/pass/borrows/runtime_qualified_owner_reference_storage_exit/main.omg`

- L18: `bytes` — writable borrow — fix: `let mut bytes`
  - `let held: Held = Held { bytes: &mut bytes };`
- L37: `view` — writable borrow — fix: `let mut view`
  - `let held: Held = Held { bytes: &mut view.owners[selector].second };`
- L48: `held` — writable borrow — fix: `let mut held`
  - `replace(&mut held.owner.second)`
- L52: `observed` — field store — fix: `let mut observed`
  - `observed.bytes == "plain..." && observed.bytes.len == 8`
- L66: `bytes` — writable borrow — fix: `let mut bytes`
  - `let changed: bool = indirect(&mut bytes);`
- L69: `container` — writable borrow — fix: `let mut container`
  - `let field_changed: bool = field_owner(&mut container);`
- L71: `entries` — writable borrow — fix: `let mut entries`
  - `let array_changed: bool = array_owner(&mut entries);`
- L78: `containers` — writable borrow — fix: `let mut containers`
  - `let selected_changed: bool = selected_field(&mut containers, 1);`
- L80: `containers` — index store — fix: `let mut containers`
  - `containers[1].second = "again...";`
- L81: `containers` — writable borrow — fix: `let mut containers`
  - `let descendant_changed: bool = selected_descendant(&mut containers, 1);`
### `tests/omega/pass/borrows/runtime_reference_aggregate_alias_replacement_exit/main.omg`

- L13: `slot` — assignment **review** — fix: `let mut slot`
  - `slot = incoming;`
### `tests/omega/pass/borrows/runtime_reference_aggregate_call_result_replacement_exit/main.omg`

- L13: `slot` — assignment **review** — fix: `let mut slot`
  - `slot = forward(incoming);`
### `tests/omega/pass/borrows/runtime_result_element_call_selector_capture_exit/main.omg`

- L9: `rooms` — writable borrow — fix: `let mut rooms`
  - `bump(&mut rooms[index], switch(&mut index));`
### `tests/omega/pass/borrows/runtime_selected_reference_nested_restoration_exit/main.omg`

- L13: `nested` — assignment **review** — fix: `let mut nested`
  - `nested = 127;`
- L23: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 13;`
### `tests/omega/pass/borrows/runtime_selected_reference_record_helper_exit/main.omg`

- L93: `envelope` — field store — fix: `let mut envelope`
  - `envelope.seal == 17 && first.value == 51 && second.value == 83`
- L122: `envelope` — field store — fix: `let mut envelope`
  - `envelope.seal == 17 && first.value == 51 && second.value == 83`
- L141: `envelope` — field store — fix: `let mut envelope`
  - `envelope.seal == 17 && first.value == 51 && second.value == 83`
### `tests/omega/pass/borrows/runtime_stored_boolean_reference_exit/main.omg`

- L9: `flags` — field store — fix: `let mut flags`
  - `flags.current == initial && flags.opposite != initial`
### `tests/omega/pass/calls/caller_result_binding_mutable_loan_exit/main.omg`

- L26: `found` — writable borrow — fix: `let mut found`
  - `find(2, &mut found);`
### `tests/omega/pass/calls/caller_structural_result_indexed_borrow_exit/main.omg`

- L14: `rooms` — writable borrow — fix: `let mut rooms`
  - `bump(&mut rooms[from_index]);`
### `tests/omega/pass/calls/native_code_layout_frame_exit/main.omg`

- L8: `values` — index store — fix: `let mut values`
  - `values[value]`
### `tests/omega/pass/calls/qualified_record_call_result_field_exit/main.omg`

- L12: `leaf` — field store — fix: `let mut leaf`
  - `leaf.name = "bat";`
- L13: `leaf` — field store — fix: `let mut leaf`
  - `leaf.value = builder.generated;`
### `tests/omega/pass/calls/runtime_alias_write_through_guarded_transition_exit/main.omg`

- L41: `found` — writable borrow — fix: `let mut found`
  - `Store::find(&mut main.store, 2, &mut found);`
### `tests/omega/pass/calls/runtime_local_scalar_array_element_loan_exit/main.omg`

- L14: `values` — writable borrow — fix: `let mut values`
  - `write_marker(&mut values[position]);`
### `tests/omega/pass/calls/runtime_mutable_local_parameter_write_exit/main.omg`

- L14: `rooms` — writable borrow — fix: `let mut rooms`
  - `Main::bump(main, &mut rooms[from_index]);`
### `tests/omega/pass/calls/runtime_nested_case_argument_exit/main.omg`

- L87: `format` — field store — fix: `let mut format`
  - `format.value == 10 && comparison.value == 30 && forwarded.value == 40 && array.value == 29`
### `tests/omega/pass/calls/runtime_owned_mutable_state_parameter_exit/main.omg`

- L15: `amount` — writable borrow — fix: `let mut amount`
  - `reset(&write amount);`
- L16: `amount` — writable borrow — fix: `let mut amount`
  - `replace(&mut amount);`
### `tests/omega/pass/calls/runtime_referenced_local_outlives_sibling_guard_call_exit/main.omg`

- L83: `random` — writable borrow — fix: `let mut random`
  - `_ -> maybe_side(maze_builder, level, &mut random)`
### `tests/omega/pass/collections/runtime_fixed_vec_contract_chain_exit/main.omg`

- L85: `slots` — index store — fix: `let mut slots`
  - `slots[index]`
### `tests/omega/pass/collections/runtime_fixed_vec_round_trip_exit/main.omg`

- L84: `slots` — index store — fix: `let mut slots`
  - `slots[index]`
### `tests/omega/pass/collections/runtime_primitive_array_literal_call_elements_exit/main.omg`

- L16: `values` — index store — fix: `let mut values`
  - `values[0] == 2 && values[1] == 4 && values[2] == 6 -> good(main)`
### `tests/omega/pass/collections/shared_slice_view_dropped_receiver/main.omg`

- L23: `bag` — writable borrow — fix: `let mut bag`
  - `Probe::scan(&main.probe, &mut bag);`
### `tests/omega/pass/contracts/borrowed_guarantee_disjoint_field_write_exit/main.omg`

- L20: `held` — assignment **review** — fix: `let mut held`
  - `held = 73;`
- L34: `held` — assignment **review** — fix: `let mut held`
  - `held = 73;`
### `tests/omega/pass/contracts/disequality_disjoin/main.omg`

- L21: `left` — assignment **review** — fix: `let mut left`
  - `left = Entry { value: 1 };`
- L22: `right` — assignment **review** — fix: `let mut right`
  - `right = Entry { value: 2 };`
### `tests/omega/pass/control_flow/projected_case_subject_root_forwarding_exit/main.omg`

- L103: `tally` — writable borrow — fix: `let mut tally`
  - `Tally::walk(&mut tally, &table);`
### `tests/omega/pass/control_flow/runtime_anonymous_transition_subject_exit/main.omg`

- L20: `held` — writable borrow — fix: `let mut held`
  - `CapturedLocal::fill(&mut held);`
### `tests/omega/pass/control_flow/runtime_later_state_shared_array_loan/main.omg`

- L10: `tally` — writable borrow — fix: `let mut tally`
  - `transition { _ -> fill(&mut tally, &cells) }`
### `tests/omega/pass/dependent/data_where_relational_fields_compile/main.omg`

- L20: `window` — field store — fix: `let mut window`
  - `window.low = start;`
- L21: `window` — field store — fix: `let mut window`
  - `window.high = limit;`
### `tests/omega/pass/domains/indexed_domain_known_disjoint_call_exit/main.omg`

- L45: `main` — writable borrow — fix: `let mut main`
  - `Main::main(&mut main);`
### `tests/omega/pass/drops/runtime_borrowed_ingress_no_nominal_disposal_exit/main.omg`

- L42: `room` — writable borrow — fix: `let mut room`
  - `borrow(&mut room);`
### `tests/omega/pass/drops/runtime_empty_array_owner_cleanup_sites_exit/main.omg`

- L16: `holder` — field store — fix: `let mut holder`
  - `holder.value = 5;`
- L25: `holder` — field store — fix: `let mut holder`
  - `holder.value = 5;`
### `tests/omega/pass/drops/runtime_named_linear_container_cleanup_exit/main.omg`

- L37: `package` — field store — fix: `let mut package`
  - `package.contents = make_receipt(17);`
### `tests/omega/pass/drops/runtime_named_parent_linear_cleanup_exit/main.omg`

- L33: `owner` — field store — fix: `let mut owner`
  - `owner.receipt = Receipt { value: 7 };`
### `tests/omega/pass/drops/runtime_nominal_state_loan_alias_exit/main.omg`

- L36: `room` — writable borrow — fix: `let mut room`
  - `let loan: &mut Room = &mut room;`
### `tests/omega/pass/drops/runtime_nominal_state_loan_cleanup_order_exit/main.omg`

- L52: `room` — writable borrow — fix: `let mut room`
  - `let loan: &mut Room = &mut room;`
### `tests/omega/pass/drops/runtime_nominal_state_loan_crash_exit/main.omg`

- L50: `room` — writable borrow — fix: `let mut room`
  - `transition { _ -> observe(&mut room) }`
### `tests/omega/pass/drops/runtime_nominal_state_loan_nonlending_branch_exit/main.omg`

- L36: `room` — writable borrow — fix: `let mut room`
  - `let loan: &mut Room = &mut room;`
### `tests/omega/pass/drops/state_loan_restores_nominal_cleanup_prerequisite/main.omg`

- L10: `room` — writable borrow — fix: `let mut room`
  - `restore(&mut room);`
### `tests/omega/pass/expressions/anonymous_structural_match_results_exit/main.omg`

- L30: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 19;`
### `tests/omega/pass/expressions/borrowed_join_match_local_origin/main.omg`

- L9: `p` — writable borrow — fix: `let mut p`
  - `true -> &mut p,`
- L10: `q` — writable borrow — fix: `let mut q`
  - `_ -> &mut q`
### `tests/omega/pass/expressions/borrowed_join_match_whole_store/main.omg`

- L12: `r` — assignment **review** — fix: `let mut r`
  - `r = Pair { left: 35, right: 35 };`
### `tests/omega/pass/expressions/match_owned_interleaved_call_results/main.omg`

- L17: `marker` — field store — fix: `let mut marker`
  - `marker.tag == tag && observed`
### `tests/omega/pass/expressions/owned_match_interleaved_values/main.omg`

- L17: `marker` — field store — fix: `let mut marker`
  - `marker.tag == tag && observed`
### `tests/omega/pass/expressions/runtime_match_structural_result_subject_once_exit/main.omg`

- L16: `result` — field store — fix: `let mut result`
  - `result.value == 41 && (counter.value as u32) == 1`
### `tests/omega/pass/facts/array_element_default_domain/call_result/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `tests/omega/pass/facts/array_element_default_domain/const_name/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `tests/omega/pass/facts/array_element_default_domain/field/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `tests/omega/pass/facts/array_element_default_domain/local/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `tests/omega/pass/facts/array_element_default_domain/match_join/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `tests/omega/pass/facts/array_element_default_domain/other_machine/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `tests/omega/pass/facts/array_element_default_domain/other_state/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `tests/omega/pass/facts/array_element_default_domain/parameter/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `tests/omega/pass/facts/array_element_default_domain/target_body/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `tests/omega/pass/facts/assumed_binding_input_authority_through_call_result_exit/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `tests/omega/pass/facts/assumed_binding_input_authority_through_const_name_exit/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `tests/omega/pass/facts/assumed_binding_input_authority_through_field_exit/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `tests/omega/pass/facts/assumed_binding_input_authority_through_local_exit/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `tests/omega/pass/facts/assumed_binding_input_authority_through_match_join_exit/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `tests/omega/pass/facts/assumed_binding_input_authority_through_other_machine_exit/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `tests/omega/pass/facts/assumed_binding_input_authority_through_other_state_exit/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `tests/omega/pass/facts/assumed_binding_input_authority_through_parameter_exit/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `tests/omega/pass/facts/assumed_binding_input_authority_through_target_body_exit/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `tests/omega/pass/facts/assumed_binding_result_bound_through_match_join_exit/main.omg`

- L9: `values` — index store — fix: `let mut values`
  - `values[index]`
### `tests/omega/pass/facts/assumed_binding_result_bound_through_other_machine_exit/main.omg`

- L9: `values` — index store — fix: `let mut values`
  - `values[index]`
### `tests/omega/pass/facts/assumed_binding_result_bound_through_target_body_exit/main.omg`

- L31: `values` — index store — fix: `let mut values`
  - `values[Required::get()]`
- L39: `values` — index store — fix: `let mut values`
  - `values[Required::get()]`
### `tests/omega/pass/facts/c_layout_nested_representation_through_match_join_exit/main.omg`

- L30: `value` — field store — fix: `let mut value`
  - `value.first == 17u8 && value.child.first == 23u8 && value.child.second == 42u64 && value.child.last == 9u16 && value.last == 11u16 && mapped.first == 17u8 && mapped.child.first == 23u8 && mapped.child.second == 42u64 && mapped.child.last == 9u16 && mapped.last == 11u16 && size_of<CLayout<Record>>() == 56u64 && align_of<CLayout<Record>>() == 8u64`
### `tests/omega/pass/facts/c_layout_nested_representation_through_other_machine_exit/main.omg`

- L39: `value` — field store — fix: `let mut value`
  - `value.first == 17u8 && value.child.first == 23u8 && value.child.second == 42u64 && value.child.last == 9u16 && value.last == 11u16 && mapped.first == 17u8 && mapped.child.first == 23u8 && mapped.child.second == 42u64 && mapped.child.last == 9u16 && mapped.last == 11u16 && size_of<CLayout<Record>>() == 56u64 && align_of<CLayout<Record>>() == 8u64`
### `tests/omega/pass/facts/c_layout_scalar_record_through_match_join_exit/main.omg`

- L22: `value` — field store — fix: `let mut value`
  - `value.first == 17u8 && value.second == 42u64 && value.last == 9u16 && mapped.first == 17u8 && mapped.second == 42u64 && mapped.last == 9u16 && size_of<CLayout<Record>>() == 24u64 && align_of<CLayout<Record>>() == 8u64`
### `tests/omega/pass/facts/c_layout_scalar_record_through_other_machine_exit/main.omg`

- L26: `value` — field store — fix: `let mut value`
  - `value.first == 17u8 && value.second == 42u64 && value.last == 9u16 && mapped.first == 17u8 && mapped.second == 42u64 && mapped.last == 9u16 && size_of<CLayout<Record>>() == 24u64 && align_of<CLayout<Record>>() == 8u64`
### `tests/omega/pass/facts/case_payload_exclusive_loan_through_call_result_exit/main.omg`

- L25: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/case_payload_exclusive_loan_through_const_name_exit/main.omg`

- L24: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/case_payload_exclusive_loan_through_field_exit/main.omg`

- L24: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/case_payload_exclusive_loan_through_local_exit/main.omg`

- L21: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/case_payload_exclusive_loan_through_match_join_exit/main.omg`

- L27: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/case_payload_exclusive_loan_through_other_machine_exit/main.omg`

- L18: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/case_payload_exclusive_loan_through_other_state_exit/main.omg`

- L23: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/case_payload_exclusive_loan_through_parameter_exit/main.omg`

- L22: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/case_payload_exclusive_loan_through_target_body_exit/main.omg`

- L47: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
- L62: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_call_result_exit/main.omg`

- L13: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L14: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_const_name_exit/main.omg`

- L11: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L12: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_field_exit/main.omg`

- L12: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L13: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_local_exit/main.omg`

- L12: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L13: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_match_join_exit/main.omg`

- L13: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L14: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_other_machine_exit/main.omg`

- L11: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L12: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_other_state_exit/main.omg`

- L16: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L17: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_parameter_exit/main.omg`

- L11: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L12: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_target_body_exit/main.omg`

- L37: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L38: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
- L49: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L50: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `tests/omega/pass/facts/copied_case_before_later_argument_through_other_state_exit/main.omg`

- L21: `chosen` — writable borrow — fix: `let mut chosen`
  - `let observed: u64 = read_payload(chosen, set_to_absent(&mut chosen));`
### `tests/omega/pass/facts/darwin_stat_abi_through_match_join_exit/main.omg`

- L49: `transported` — field store — fix: `let mut transported`
  - `transported.dev == -2147483647i32 && transported.mode == 33188u16 && transported.nlink == 3u16 && transported.ino == 4294967298u64 && transported.uid == 123u32 && transported.gid == 456u32 && transported.rdev == -123456789i32 && transported.atime.seconds == -17i64 && transported.atime.nanoseconds == 123456789i64 && transported.mtime.seconds == 42i64 && transported.mtime.nanoseconds == 987654321i64 && transported.ctime.seconds == -9i64 && transported.ctime.nanoseconds == 111111111i64 && transported.btime.seconds == 77i64 && transported.btime.nanoseconds == 222222222i64 && transported.size == 8589934593i64 && transported.blocks == 123456i64 && transported.blksize == 4096i32 && transported.flags == 2147483649u32 && transported.generation == 2882400001u32 && transported.reserved == -314159i32 && transported.spare[0] == -4294967297i64 && transported.spare[1] == 68719476737i64 && StatExchange::check(&transported) == 0u64 && StatExchange::extent() == size_of<DarwinStat>() && StatExchange::alignment() == align_of<DarwinStat>() && size_of<DarwinStat>() == 144u64 && align_of<DarwinStat>() == 8u64 && widen_u32_to_u64(narrow_i32_to_u32_wrapping(transported.dev)) == 2147483649u64 && widen_u32_to_u64(narrow_i32_to_u32_wrapping(transported.rdev)) == 4171510507u64`
### `tests/omega/pass/facts/exclusive_match_arm_through_call_result_exit/main.omg`

- L25: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/exclusive_match_arm_through_const_name_exit/main.omg`

- L21: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/exclusive_match_arm_through_field_exit/main.omg`

- L20: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/exclusive_match_arm_through_local_exit/main.omg`

- L19: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/exclusive_match_arm_through_match_join_exit/main.omg`

- L24: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/exclusive_match_arm_through_other_machine_exit/main.omg`

- L18: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/exclusive_match_arm_through_other_state_exit/main.omg`

- L20: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/exclusive_match_arm_through_parameter_exit/main.omg`

- L18: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/exclusive_match_arm_through_target_body_exit/main.omg`

- L47: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
- L62: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/fixed_array_eight_elem_index_bound_through_local_exit/main.omg`

- L9: `local_values` — index store — fix: `let mut local_values`
  - `local_values[local_index]`
### `tests/omega/pass/facts/fixed_array_onethousandtwentyfour_elem_index_bound_through_local_exit/main.omg`

- L10: `local_arr` — index store — fix: `let mut local_arr`
  - `local_arr[local_idx]`
### `tests/omega/pass/facts/fixed_array_twofiftysix_elem_index_bound_through_local_exit/main.omg`

- L10: `local_arr` — index store — fix: `let mut local_arr`
  - `local_arr[local_idx]`
### `tests/omega/pass/facts/fixed_array_value_through_local_exit/main.omg`

- L9: `local_values` — index store — fix: `let mut local_values`
  - `local_values[local_index]`
### `tests/omega/pass/facts/fixed_layout_reference_readback_exit/main.omg`

- L32: `result` — field store — fix: `let mut result`
  - `result.value == 7u64 && result.pointer.value == 43u64`
### `tests/omega/pass/facts/fixed_layout_reference_readback_through_const_name_exit/main.omg`

- L36: `result` — field store — fix: `let mut result`
  - `result.value == 7u64 && result.pointer.value == 43u64`
### `tests/omega/pass/facts/fixed_layout_reference_readback_through_local_exit/main.omg`

- L33: `result` — field store — fix: `let mut result`
  - `result.value == 7u64 && result.pointer.value == 43u64`
### `tests/omega/pass/facts/fixed_layout_reference_readback_through_match_join_exit/main.omg`

- L37: `result` — field store — fix: `let mut result`
  - `result.value == 7u64 && result.pointer.value == 43u64 && result.selected_first == choose_first`
### `tests/omega/pass/facts/fixed_layout_reference_readback_through_other_machine_exit/main.omg`

- L35: `forwarded` — field store — fix: `let mut forwarded`
  - `forwarded.value == 7u64 && forwarded.pointer.value == 43u64`
### `tests/omega/pass/facts/fixed_layout_reference_readback_through_other_state_exit/main.omg`

- L35: `result` — field store — fix: `let mut result`
  - `result.value == 7u64 && result.pointer.value == 43u64`
### `tests/omega/pass/facts/foreign_indirect_float_record_result_through_match_join_exit/main.omg`

- L46: `selected` — field store — fix: `let mut selected`
  - `selected.branch == choice && receive(selected.value)`
### `tests/omega/pass/facts/indexed_element_mut_loan_through_call_result_exit/main.omg`

- L16: `rooms` — writable borrow — fix: `let mut rooms`
  - `bump(&mut rooms[selected]);`
### `tests/omega/pass/facts/indexed_element_mut_loan_through_const_name_exit/main.omg`

- L14: `rooms` — writable borrow — fix: `let mut rooms`
  - `bump(&mut rooms[selected]);`
### `tests/omega/pass/facts/indexed_element_mut_loan_through_field_exit/main.omg`

- L14: `rooms` — writable borrow — fix: `let mut rooms`
  - `bump(&mut rooms[selector.index]);`
### `tests/omega/pass/facts/indexed_element_mut_loan_through_local_exit/main.omg`

- L13: `rooms` — writable borrow — fix: `let mut rooms`
  - `bump(&mut rooms[selected]);`
### `tests/omega/pass/facts/indexed_element_mut_loan_through_match_join_exit/main.omg`

- L13: `rooms` — writable borrow — fix: `let mut rooms`
  - `bump(&mut rooms[selected]);`
### `tests/omega/pass/facts/indexed_element_mut_loan_through_other_machine_exit/main.omg`

- L20: `rooms` — writable borrow — fix: `let mut rooms`
  - `forward(&mut rooms, from_index);`
### `tests/omega/pass/facts/indexed_element_mut_loan_through_other_state_exit/main.omg`

- L12: `rooms` — writable borrow — fix: `let mut rooms`
  - `transition { _ -> apply(&mut rooms, from_index) }`
- L16: `rooms` — writable borrow — fix: `let mut rooms`
  - `bump(&mut rooms[selected]);`
### `tests/omega/pass/facts/indexed_element_mut_loan_through_parameter_exit/main.omg`

- L17: `rooms` — writable borrow — fix: `let mut rooms`
  - `apply(&mut rooms, from_index);`
### `tests/omega/pass/facts/indexed_element_mut_loan_through_target_body_exit/main.omg`

- L56: `rooms` — writable borrow — fix: `let mut rooms`
  - `SelectedApply::apply(&mut rooms, from_index);`
### `tests/omega/pass/facts/indexed_recoverable_replacement_window_through_call_result_exit/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `tests/omega/pass/facts/indexed_recoverable_replacement_window_through_const_name_exit/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `tests/omega/pass/facts/indexed_recoverable_replacement_window_through_field_exit/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `tests/omega/pass/facts/indexed_recoverable_replacement_window_through_local_exit/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `tests/omega/pass/facts/indexed_recoverable_replacement_window_through_match_join_exit/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `tests/omega/pass/facts/indexed_recoverable_replacement_window_through_other_machine_exit/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `tests/omega/pass/facts/indexed_recoverable_replacement_window_through_other_state_exit/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `tests/omega/pass/facts/indexed_recoverable_replacement_window_through_parameter_exit/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `tests/omega/pass/facts/indexed_recoverable_replacement_window_through_target_body_exit/main.omg`

- L59: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
- L73: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `tests/omega/pass/facts/length_through_call_result_exit/main.omg`

- L11: `window` — index store — fix: `let mut window`
  - `window[length - 1]`
### `tests/omega/pass/facts/length_through_const_name_exit/main.omg`

- L8: `window` — index store — fix: `let mut window`
  - `window[LENGTH - 1]`
### `tests/omega/pass/facts/length_through_field_exit/main.omg`

- L9: `window` — index store — fix: `let mut window`
  - `window[meter.length - 1]`
### `tests/omega/pass/facts/length_through_local_exit/main.omg`

- L8: `window` — index store — fix: `let mut window`
  - `window[length - 1]`
### `tests/omega/pass/facts/length_through_match_join_exit/main.omg`

- L8: `window` — index store — fix: `let mut window`
  - `window[length - 1]`
### `tests/omega/pass/facts/match_computed_root_through_call_result_exit/main.omg`

- L27: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/match_computed_root_through_const_name_exit/main.omg`

- L22: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/match_computed_root_through_field_exit/main.omg`

- L24: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/match_computed_root_through_local_exit/main.omg`

- L21: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/match_computed_root_through_match_join_exit/main.omg`

- L26: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/match_computed_root_through_other_machine_exit/main.omg`

- L18: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/match_computed_root_through_other_state_exit/main.omg`

- L20: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/match_computed_root_through_parameter_exit/main.omg`

- L20: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/match_computed_root_through_target_body_exit/main.omg`

- L18: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/match_effectful_subject_custody_through_other_state_exit/main.omg`

- L28: `slot` — writable borrow — fix: `let mut slot`
  - `let held: Token = match refill(&mut slot, c, flag) {`
### `tests/omega/pass/facts/nested_zero_leaf_array_storage_through_call_result_exit/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `tests/omega/pass/facts/nested_zero_leaf_array_storage_through_const_name_exit/main.omg`

- L12: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `tests/omega/pass/facts/nested_zero_leaf_array_storage_through_field_exit/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `tests/omega/pass/facts/nested_zero_leaf_array_storage_through_local_exit/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `tests/omega/pass/facts/nested_zero_leaf_array_storage_through_match_join_exit/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `tests/omega/pass/facts/nested_zero_leaf_array_storage_through_other_machine_exit/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `tests/omega/pass/facts/nested_zero_leaf_array_storage_through_other_state_exit/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `tests/omega/pass/facts/nested_zero_leaf_array_storage_through_parameter_exit/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `tests/omega/pass/facts/nested_zero_leaf_array_storage_through_target_body_exit/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `tests/omega/pass/facts/nested_zero_leaf_array_storage_zeroed_source_exit/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `tests/omega/pass/facts/nominal_loan_backing_cleanup_through_call_result_exit/main.omg`

- L39: `room` — writable borrow — fix: `let mut room`
  - `transition { _ -> observe(&mut room) }`
### `tests/omega/pass/facts/nominal_loan_backing_cleanup_through_const_name_exit/main.omg`

- L37: `room` — writable borrow — fix: `let mut room`
  - `transition { _ -> observe(&mut room) }`
### `tests/omega/pass/facts/nominal_loan_backing_cleanup_through_field_exit/main.omg`

- L37: `holder` — writable borrow — fix: `let mut holder`
  - `transition { _ -> observe(&mut holder.room) }`
### `tests/omega/pass/facts/nominal_loan_backing_cleanup_through_local_exit/main.omg`

- L36: `room` — writable borrow — fix: `let mut room`
  - `transition { _ -> observe(&mut room) }`
### `tests/omega/pass/facts/nominal_loan_backing_cleanup_through_match_join_exit/main.omg`

- L36: `room` — writable borrow — fix: `let mut room`
  - `transition { _ -> observe(&mut room) }`
### `tests/omega/pass/facts/nominal_loan_backing_cleanup_through_other_machine_exit/main.omg`

- L41: `room` — writable borrow — fix: `let mut room`
  - `transition { _ -> observe(&mut room) }`
### `tests/omega/pass/facts/nominal_loan_backing_cleanup_through_target_body_exit/main.omg`

- L41: `room` — writable borrow — fix: `let mut room`
  - `transition { _ -> observe(&mut room) }`
### `tests/omega/pass/facts/recoverable_replacement_window_through_call_result_exit/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `tests/omega/pass/facts/recoverable_replacement_window_through_const_name_exit/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `tests/omega/pass/facts/recoverable_replacement_window_through_field_exit/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `tests/omega/pass/facts/recoverable_replacement_window_through_local_exit/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `tests/omega/pass/facts/recoverable_replacement_window_through_match_join_exit/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `tests/omega/pass/facts/recoverable_replacement_window_through_other_machine_exit/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `tests/omega/pass/facts/recoverable_replacement_window_through_other_state_exit/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `tests/omega/pass/facts/recoverable_replacement_window_through_parameter_exit/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `tests/omega/pass/facts/recoverable_replacement_window_through_target_body_exit/main.omg`

- L58: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
- L75: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `tests/omega/pass/facts/reference_arrival_handoff_through_call_result_exit/main.omg`

- L9: `child` — assignment **review** — fix: `let mut child`
  - `child = 41;`
### `tests/omega/pass/facts/reference_arrival_handoff_through_const_name_exit/main.omg`

- L9: `child` — assignment **review** — fix: `let mut child`
  - `child = WRITTEN;`
### `tests/omega/pass/facts/reference_arrival_handoff_through_field_exit/main.omg`

- L9: `holder` — field store — fix: `let mut holder`
  - `holder.slot = 41;`
### `tests/omega/pass/facts/reference_arrival_handoff_through_local_exit/main.omg`

- L8: `child` — assignment **review** — fix: `let mut child`
  - `child = 41;`
### `tests/omega/pass/facts/reference_arrival_handoff_through_match_join_exit/main.omg`

- L14: `child` — assignment **review** — fix: `let mut child`
  - `child = 41;`
### `tests/omega/pass/facts/replaced_field_loan_through_call_result_exit/main.omg`

- L22: `selected` — writable borrow — fix: `let mut selected`
  - `let written: bool = store(&mut selected.body, 41);`
### `tests/omega/pass/facts/replaced_field_loan_through_field_exit/main.omg`

- L22: `wrapper` — writable borrow — fix: `let mut wrapper`
  - `let written: bool = store(&mut wrapper.message.body, 41);`
### `tests/omega/pass/facts/replaced_field_loan_through_match_join_exit/main.omg`

- L21: `selected` — writable borrow — fix: `let mut selected`
  - `let written: bool = store(&mut selected.body, 41);`
### `tests/omega/pass/facts/returned_write_destination_through_call_result_exit/main.omg`

- L9: `destination` — assignment **review** — fix: `let mut destination`
  - `destination = 42;`
### `tests/omega/pass/facts/returned_write_destination_through_const_name_exit/main.omg`

- L8: `destination` — assignment **review** — fix: `let mut destination`
  - `destination = 42;`
### `tests/omega/pass/facts/returned_write_destination_through_field_exit/main.omg`

- L9: `destination` — field store — fix: `let mut destination`
  - `destination.loan = 42;`
### `tests/omega/pass/facts/returned_write_destination_through_local_exit/main.omg`

- L8: `destination` — assignment **review** — fix: `let mut destination`
  - `destination = 42;`
### `tests/omega/pass/facts/returned_write_destination_through_match_join_exit/main.omg`

- L6: `destination` — assignment **review** — fix: `let mut destination`
  - `destination = 42;`
### `tests/omega/pass/facts/returned_write_destination_through_target_body_exit/main.omg`

- L44: `destination` — assignment **review** — fix: `let mut destination`
  - `destination = 42;`
### `tests/omega/pass/facts/signed_division_remainder_through_call_result_exit/main.omg`

- L18: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `tests/omega/pass/facts/signed_division_remainder_through_const_name_exit/main.omg`

- L13: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `tests/omega/pass/facts/signed_division_remainder_through_field_exit/main.omg`

- L16: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `tests/omega/pass/facts/signed_division_remainder_through_local_exit/main.omg`

- L13: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `tests/omega/pass/facts/signed_division_remainder_through_match_join_exit/main.omg`

- L21: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `tests/omega/pass/facts/signed_division_remainder_through_other_machine_exit/main.omg`

- L11: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `tests/omega/pass/facts/signed_division_remainder_through_other_state_exit/main.omg`

- L17: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `tests/omega/pass/facts/signed_division_remainder_through_parameter_exit/main.omg`

- L11: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `tests/omega/pass/facts/signed_division_remainder_through_target_body_exit/main.omg`

- L16: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
- L26: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `tests/omega/pass/facts/view_extent_after_write_through_field_exit/main.omg`

- L10: `holder` — writable borrow — fix: `let mut holder`
  - `write_first(&mut holder.view[0]);`
### `tests/omega/pass/facts/write_only_indexed_reborrow_through_field_exit/main.omg`

- L28: `held` — writable borrow — fix: `let mut held`
  - `replace(&write held.cells[storage.position], 7);`
- L29: `held` — writable borrow — fix: `let mut held`
  - `replace(&write held.cells[storage.position], 9);`
- L31: `held` — writable borrow — fix: `let mut held`
  - `replace(&write held.cells[storage.position], -43);`
### `tests/omega/pass/facts/write_only_match_join_through_call_result_exit/main.omg`

- L26: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/write_only_match_join_through_const_name_exit/main.omg`

- L25: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/write_only_match_join_through_field_exit/main.omg`

- L25: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/write_only_match_join_through_local_exit/main.omg`

- L22: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/write_only_match_join_through_match_join_exit/main.omg`

- L28: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/write_only_match_join_through_other_machine_exit/main.omg`

- L21: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/write_only_match_join_through_other_state_exit/main.omg`

- L25: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/write_only_match_join_through_parameter_exit/main.omg`

- L21: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/facts/write_only_match_join_through_target_body_exit/main.omg`

- L50: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
- L64: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `tests/omega/pass/float/guarded_float_transition_range_exit/main.omg`

- L54: `gap` — field store — fix: `let mut gap`
  - `gap.difference = start_distance - end_distance;`
### `tests/omega/pass/generics/runtime_index_carrier_array_valid/main.omg`

- L5: `wrong` — index store — fix: `let mut wrong`
  - `wrong[0].value`
### `tests/omega/pass/generics/selected_open_capture_carriers_exit/main.omg`

- L24: `one` — field store — fix: `let mut one`
  - `one.value == 17 && two.value == 29 && array[0] == 31 && array[1] == 41`
### `tests/omega/pass/generics/static_machine_borrowed_parameter_refines_after_substitution/main.omg`

- L74: `current` — writable borrow — fix: `let mut current`
  - `let keep: bool = Touch(&mut current);`
### `tests/omega/pass/host/runtime_descriptor_immutable_primitive_loans_exit/main.omg`

- L18: `coordinate` — writable borrow — fix: `let mut coordinate`
  - `select(&mut coordinate);`
- L30: `flag` — writable borrow — fix: `let mut flag`
  - `let held: &mut bool = &mut flag;`
- L33: `flag` — writable borrow — fix: `let mut flag`
  - `clear(&mut flag);`
- L35: `flag` — writable borrow — fix: `let mut flag`
  - `set(&mut flag);`
- L39: `value` — writable borrow — fix: `let mut value`
  - `let empty: i64 = block ScalarIo::read(0, &mut value, 0);`
- L40: `value` — writable borrow — fix: `let mut value`
  - `let moved: i64 = block receive(&mut value);`
- L42: `value` — writable borrow — fix: `let mut value`
  - `let at_end: i64 = block ScalarIo::read(0, &mut value, 4);`
### `tests/omega/pass/host/runtime_descriptor_reference_record_prefix_exit/main.omg`

- L44: `packet` — writable borrow — fix: `let mut packet`
  - `let zero: i64 = block receive(&mut packet.header, 0);`
- L45: `packet` — writable borrow — fix: `let mut packet`
  - `let moved: i64 = block receive(&mut packet.header, 1);`
- L48: `packet` — writable borrow — fix: `let mut packet`
  - `let line: i64 = block receive(&mut packet.header, 1);`
- L49: `packet` — writable borrow — fix: `let mut packet`
  - `let end: i64 = block receive(&mut packet.header, 1);`
### `tests/omega/pass/host/runtime_semantic_gui_reach_exit/main.omg`

- L29: `window` — writable borrow — fix: `let mut window`
  - `let available: u32 = macos_window_msg_peek(&mut window, [0, 0, 0, 0, 0, 0]);`
### `tests/omega/pass/layouts/direct_encoded_assignment_preserves_descriptor_exit/main.omg`

- L42: `leaf` — assignment **review** — fix: `let mut leaf`
  - `leaf = 70;`
- L44: `ordinary` — assignment **review** — fix: `let mut ordinary`
  - `ordinary = 127;`
### `tests/omega/pass/layouts/placed_primitive_returned_backing/main.omg`

- L34: `holder` — writable borrow — fix: `let mut holder`
  - `true -> &mut holder.value,`
- L35: `holder` — writable borrow — fix: `let mut holder`
  - `false -> &mut holder.value,`
### `tests/omega/pass/layouts/runtime_captured_encoded_reference_replacement_exit/main.omg`

- L43: `captured` — assignment **review** — fix: `let mut captured`
  - `captured = 127;`
- L45: `replacement` — assignment **review** — fix: `let mut replacement`
  - `replacement = 42;`
### `tests/omega/pass/layouts/runtime_multi_origin_encoded_reference_exit/main.omg`

- L43: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = value;`
### `tests/omega/pass/layouts/runtime_placed_external_array_snapshot_exit/main.omg`

- L51: `updated` — index store — fix: `let mut updated`
  - `updated[0] == 17 && updated[256] == 97 && updated[511] == 73 {`
### `tests/omega/pass/layouts/runtime_plan_laid_tiled_record_nested_array_view_exit/main.omg`

- L40: `cells` — index store — fix: `let mut cells`
  - `cells[0] = Cell {`
- L44: `cells` — index store — fix: `let mut cells`
  - `cells[1] = Cell {`
### `tests/omega/pass/layouts/runtime_recursive_encoded_reference_exit/main.omg`

- L81: `encoded` — assignment **review** — fix: `let mut encoded`
  - `encoded = -128;`
- L85: `ordinary` — assignment **review** — fix: `let mut ordinary`
  - `ordinary = 127;`
### `tests/omega/pass/layouts/runtime_selected_reference_parent_restoration_exit/main.omg`

- L39: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = value;`
### `tests/omega/pass/layouts/runtime_target_layout_byte_offset_exit/main.omg`

- L10: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "macos_arm64" -> on_macos_arm64(schema)`
- L11: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "linux_x86_64" -> on_linux_x86_64(schema)`
### `tests/omega/pass/layouts/runtime_target_layout_forwarded_integer_exit/main.omg`

- L10: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "macos_arm64" -> on_macos_arm64(schema)`
- L11: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "linux_x86_64" -> on_linux_x86_64(schema)`
### `tests/omega/pass/layouts/runtime_target_layout_queries_exit/main.omg`

- L12: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "macos_arm64" -> on_macos_arm64(schema)`
- L13: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "linux_x86_64" -> on_linux_x86_64(schema)`
- L14: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "linux_arm64" -> on_linux_arm64(schema)`
- L15: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "windows_x86_64" -> on_windows_x86_64(schema)`
### `tests/omega/pass/layouts/runtime_unselected_ordinary_crash_guard_exit/main.omg`

- L12: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "macos_arm64" -> on_macos_arm64(schema)`
- L13: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "linux_x86_64" -> on_linux_x86_64(schema)`
- L14: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "linux_arm64" -> on_linux_arm64(schema)`
- L15: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "windows_x86_64" -> on_windows_x86_64(schema)`
### `tests/omega/pass/layouts/runtime_unselected_target_callback_layout_safe_exit/main.omg`

- L9: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "macos_arm64" -> on_macos_arm64(schema)`
- L10: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "linux_x86_64" -> on_linux_x86_64(schema)`
- L11: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "linux_arm64" -> on_linux_arm64(schema)`
- L12: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "windows_x86_64" -> on_windows_x86_64(schema)`
### `tests/omega/pass/layouts/runtime_unselected_target_helper_layout_safe_exit/main.omg`

- L9: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "macos_arm64" -> on_macos_arm64(schema)`
- L10: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "linux_x86_64" -> on_linux_x86_64(schema)`
- L11: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "linux_arm64" -> on_linux_arm64(schema)`
- L12: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "windows_x86_64" -> on_windows_x86_64(schema)`
### `tests/omega/pass/layouts/unused_partial_target_layout_exit/main.omg`

- L13: `facts` — field store — fix: `let mut facts`
  - `facts.profile == "macos_arm64" -> on_macos_arm64(schema)`
### `tests/omega/pass/locals/runtime_local_array_member_stores_without_mut_exit/main.omg`

- L14: `padded` — index store — fix: `let mut padded`
  - `padded[0] = title[0];`
- L15: `padded` — index store — fix: `let mut padded`
  - `padded[1] = title[1];`
- L16: `padded` — index store — fix: `let mut padded`
  - `padded[2] = title[2];`
- L17: `padded` — index store — fix: `let mut padded`
  - `padded[3] = 0;`
- L23: `bytes` — index store — fix: `let mut bytes`
  - `bytes[1] = seed;`
- L29: `text` — index store — fix: `let mut text`
  - `text[1] = seed;`
### `tests/omega/pass/locals/runtime_local_qualified_view_member_read_exit/main.omg`

- L48: `sample` — field store — fix: `let mut sample`
  - `sample.count = 7;`
- L49: `sample` — field store — fix: `let mut sample`
  - `sample.label = "héllo";`
### `tests/omega/pass/locals/runtime_local_view_member_element_read_exit/main.omg`

- L20: `sample` — field store — fix: `let mut sample`
  - `sample.readings = "Hi";`
### `tests/omega/pass/ownership/disjoint_field_loans_compile/main.omg`

- L14: `left` — assignment **review** — fix: `let mut left`
  - `left = 10;`
- L15: `right` — assignment **review** — fix: `let mut right`
  - `right = 20;`
### `tests/omega/pass/ownership/disjoint_member_write_during_loan_compile/main.omg`

- L19: `loan` — assignment **review** — fix: `let mut loan`
  - `loan = 4;`
### `tests/omega/pass/ownership/local_element_loan_compile/main.omg`

- L12: `cells` — writable borrow — fix: `let mut cells`
  - `Main::bump(&mut cells[1]);`
### `tests/omega/pass/ownership/partial_carrier_refill_exit/main.omg`

- L17: `detached` — field store — fix: `let mut detached`
  - `detached.value == 17 && sibling == 23 && outer == 31 && selected == 29`
### `tests/omega/pass/ownership/runtime_borrowed_match_repeated_repair_exit/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first { Outcome::Success { token } -> token, Outcome::Failure { token } -> token };`
### `tests/omega/pass/ownership/runtime_indexed_recoverable_replacement_exit/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `tests/omega/pass/ownership/runtime_indexed_replacement_effectful_singleton_exit/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `tests/omega/pass/ownership/runtime_indexed_replacement_index_one_exit/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `tests/omega/pass/ownership/runtime_match_observation_cleanup_exit/main.omg`

- L47: `selected` — field store — fix: `let mut selected`
  - `selected.value == 41 && (counter.value as u32) == 34`
### `tests/omega/pass/ownership/runtime_recoverable_alias_operand_order_exit/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `tests/omega/pass/ownership/runtime_referent_replacement_fact_transport_exit/main.omg`

- L9: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = Pair { first: Child { value: source + 2 }, second: 5 };`
### `tests/omega/pass/ownership/runtime_replacement_call_child_snapshot_exit/main.omg`

- L23: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = Pair {`
### `tests/omega/pass/ownership/runtime_retained_alias_referent_transform_exit/main.omg`

- L26: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = transform(removed);`
- L34: `forwarded` — assignment **review** — fix: `let mut forwarded`
  - `forwarded = transform(removed);`
### `tests/omega/pass/ownership/selected_storage_join_held_address_exit/main.omg`

- L21: `held` — assignment **review** — fix: `let mut held`
  - `held = 73;`
### `tests/omega/pass/ownership/selected_storage_join_nested_exit/main.omg`

- L23: `held` — assignment **review** — fix: `let mut held`
  - `held = 73;`
### `tests/omega/pass/ownership/state_param_loan_compile/main.omg`

- L18: `held` — writable borrow — fix: `let mut held`
  - `Main::fill(&mut held);`
### `tests/omega/pass/proofs/alias_store_keeps_sibling_snapshot_exit/main.omg`

- L15: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = 7;`
### `tests/omega/pass/proofs/declaration_predicate_calls_exit/main.omg`

- L54: `value` — writable borrow — fix: `let mut value`
  - `let changed: u64 = initialize_counter(&mut value);`
- L62: `text` — writable borrow — fix: `let mut text`
  - `transition { _ -> observe(&mut text, 0) }`
- L71: `text` — field store — fix: `let mut text`
  - `text.bytes = [103, 255, 33];`
- L99: `mixed` — field store — fix: `let mut mixed`
  - `mixed.tag == 1 && ready.tag == 1 && generic_byte.payload == 19u8 && generic_wide.payload == 21u64 &&`
### `tests/omega/pass/proofs/reviewed_mathematical_data_contract_exit/main.omg`

- L20: `step` — field store — fix: `let mut step`
  - `step.table_address == 0x12345000u64 && step.index == 7u64 &&`
- L21: `step` — field store — fix: `let mut step`
  - `step.entry_address == 0x12345038u64 && step.entry.physical_address == (word & 0x000FFFFFFFFFF000u64) &&`
### `tests/omega/pass/providers/assumed_result_bound_match_join_exit/main.omg`

- L9: `values` — index store — fix: `let mut values`
  - `values[index]`
### `tests/omega/pass/providers/assumed_result_bound_other_machine_exit/main.omg`

- L9: `values` — index store — fix: `let mut values`
  - `values[index]`
### `tests/omega/pass/providers/assumed_result_bound_target_body_exit/main.omg`

- L31: `values` — index store — fix: `let mut values`
  - `values[Required::get()]`
- L39: `values` — index store — fix: `let mut values`
  - `values[Required::get()]`
### `tests/omega/pass/providers/inherited_applied_binding_borrowed_result_exit/main.omg`

- L42: `mutable` — assignment **review** — fix: `let mut mutable`
  - `mutable = 43;`
### `tests/omega/pass/providers/inherited_provider_borrowed_result_exit/main.omg`

- L42: `mutable` — assignment **review** — fix: `let mut mutable`
  - `mutable = 43;`
### `tests/omega/pass/providers/resource_profile_roles/main.omg`

- L71: `storage` — field store — fix: `let mut storage`
  - `storage.length == 16 {`
### `tests/omega/pass/providers/resource_profile_supply_observation/main.omg`

- L58: `third` — field store — fix: `let mut third`
  - `third.region_count == 1 && fourth.region_count == 1 &&`
### `tests/omega/pass/ranges/runtime_bounded_array_element_order_exit/main.omg`

- L15: `values` — index store — fix: `let mut values`
  - `values[0].value`
### `tests/omega/pass/recast/borrowed_byte_scalar_views_exit/main.omg`

- L32: `decoded` — assignment **review** — fix: `let mut decoded`
  - `decoded = 4607182418800017408;`
### `tests/omega/pass/recast/mutable_byte_signed_write_compile/main.omg`

- L12: `view` — assignment **review** — fix: `let mut view`
  - `view = 0 - 1;`
### `tests/omega/pass/recast/mutable_float32_int_write_compile/main.omg`

- L12: `view` — assignment **review** — fix: `let mut view`
  - `view = 1077936128;`
### `tests/omega/pass/recast/mutable_float32_write_compile/main.omg`

- L12: `view` — assignment **review** — fix: `let mut view`
  - `view = 3.25;`
### `tests/omega/pass/recast/mutable_float64_signed_write_compile/main.omg`

- L12: `view` — assignment **review** — fix: `let mut view`
  - `view = 4613937818241073152;`
### `tests/omega/pass/recast/mutable_float_bits_write_compile/main.omg`

- L12: `bits` — assignment **review** — fix: `let mut bits`
  - `bits = 13830554455654793216;`
### `tests/omega/pass/recast/mutable_i16_unsigned_write_compile/main.omg`

- L12: `view` — assignment **review** — fix: `let mut view`
  - `view = 65535;`
### `tests/omega/pass/recast/mutable_i32_float_write_compile/main.omg`

- L12: `view` — assignment **review** — fix: `let mut view`
  - `view = 0.5;`
### `tests/omega/pass/recast/mutable_i32_unsigned_write_compile/main.omg`

- L12: `view` — assignment **review** — fix: `let mut view`
  - `view = 0 - 3;`
### `tests/omega/pass/recast/mutable_int_float_write_compile/main.omg`

- L12: `view` — assignment **review** — fix: `let mut view`
  - `view = 1.5;`
### `tests/omega/pass/recast/mutable_local_recast_write_compile/main.omg`

- L12: `local` — writable borrow — fix: `let mut local`
  - `let view: &mut i64 = &mut local as &mut i64;`
- L13: `view` — assignment **review** — fix: `let mut view`
  - `view = 0 - 11;`
### `tests/omega/pass/recast/mutable_signed_bits_write_compile/main.omg`

- L12: `view` — assignment **review** — fix: `let mut view`
  - `view = 0 - 1;`
### `tests/omega/pass/recast/mutable_u16_signed_write_compile/main.omg`

- L12: `view` — assignment **review** — fix: `let mut view`
  - `view = 0 - 5;`
### `tests/omega/pass/recast/mutable_unsigned_signed_write_compile/main.omg`

- L12: `view` — assignment **review** — fix: `let mut view`
  - `view = 0 - 7;`
### `tests/omega/pass/recast/non_owning_erasure_retained_field_predicate_exit/main.omg`

- L6: `view` — assignment **review** — fix: `let mut view`
  - `view = 7;`
### `tests/omega/pass/recast/runtime_bool_representation_recast_exit/main.omg`

- L35: `byte_flag` — assignment **review** — fix: `let mut byte_flag`
  - `byte_flag = false;`
### `tests/omega/pass/recast/runtime_offset_byte_recast_mutable_write_exit/main.omg`

- L23: `view` — assignment **review** — fix: `let mut view`
  - `view = 67305985;`
### `tests/omega/pass/recast/runtime_restricted_field_loans_exit/main.omg`

- L20: `flag` — assignment **review** — fix: `let mut flag`
  - `flag = true;`
### `tests/omega/pass/recast/runtime_shared_descriptor_recast_exit/main.omg`

- L13: `bytes` — index store — fix: `let mut bytes`
  - `bytes[0] == 0x33 && bytes[12] == 0x66 && bytes[22] == 0x77`
- L22: `bytes` — field store — fix: `let mut bytes`
  - `bytes.len == 0`
- L49: `bytes` — index store — fix: `let mut bytes`
  - `bytes[0] == 0x41 && bytes[1] == 0x42 && bytes[2] == 0x43`
### `tests/omega/pass/slices/declared_view_fill/main.omg`

- L15: `room` — field store — fix: `let mut room`
  - `room.label = main.source.label;`
### `tests/omega/pass/slices/window_shrink_min_length_tail_index_compile/main.omg`

- L14: `tail` — index store — fix: `let mut tail`
  - `tail[2]`
### `tests/omega/pass/storage/runtime_local_slice_forward_exit/main.omg`

- L12: `room` — writable borrow — fix: `let mut room`
  - `let exits: &mut [Counter] = &mut room.exits[..];`
### `tests/omega/pass/storage/runtime_parameter_slice_forward_exit/main.omg`

- L27: `room` — writable borrow — fix: `let mut room`
  - `mutate(&mut room);`
### `tests/omega/pass/structs/runtime_declared_record_member_fill_exit/main.omg`

- L10: `point` — field store — fix: `let mut point`
  - `point.x = 7;`
- L11: `point` — field store — fix: `let mut point`
  - `point.y = 12;`
### `tests/omega/pass/structs/runtime_state_record_array_element_read_exit/main.omg`

- L36: `packet` — writable borrow — fix: `let mut packet`
  - `fill(&mut packet);`
### `tests/omega/pass/termination/runtime_ranked_assignment_call_prefix_exit/main.omg`

- L41: `scratch` — writable borrow — fix: `let mut scratch`
  - `let result: u32 = countdown(selected, &mut scratch, 4);`
### `tests/omega/pass/text/declared_record_view_field_fill_exit/main.omg`

- L20: `room` — field store — fix: `let mut room`
  - `room.label = main.source.label;`
### `tests/omega/pass/text/encoded_source_select/main.omg`

- L66: `encoded` — assignment **review** — fix: `let mut encoded`
  - `encoded = -128;`
- L70: `ordinary` — assignment **review** — fix: `let mut ordinary`
  - `ordinary = 127;`
### `tests/omega/pass/text/local_record_byte_field_copy_exit/main.omg`

- L22: `room` — writable borrow — fix: `let mut room`
  - `fill(&mut room);`
- L24: `room` — writable borrow — fix: `let mut room`
  - `change(&mut room);`
### `tests/omega/pass/text/runtime_alias_string_write/main.omg`

- L18: `label_alias` — assignment **review** — fix: `let mut label_alias`
  - `label_alias = TextBuffer { storage: [104u8, 101u8, 108u8, 108u8, 111u8, 0u8, 0u8, 0u8], used: 5 };`
- L28: `another` — assignment **review** — fix: `let mut another`
  - `another = TextBuffer { storage: [195u8, 169u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8], used: 2 };`
- L39: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = TextBuffer { storage: [195u8, 169u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8], used: 2 };`
### `tests/omega/pass/text/runtime_byte_field_dynamic_view_exit/main.omg`

- L42: `room` — writable borrow — fix: `let mut room`
  - `fill(&mut room);`
- L44: `room` — writable borrow — fix: `let mut room`
  - `change(&mut room);`
### `tests/omega/pass/text/runtime_inline_byte_case_values_exit/main.omg`

- L66: `changed` — writable borrow — fix: `let mut changed`
  - `let edited: bool = edit(&mut changed);`
### `tests/omega/pass/text/runtime_qualified_byte_array_storage_exit/main.omg`

- L23: `entries` — writable borrow — fix: `let mut entries`
  - `let changed: bool = edit(&mut entries);`
- L27: `empty` — writable borrow — fix: `let mut empty`
  - `write_owner(&mut empty[1]);`
### `tests/omega/pass/text/runtime_view_local_record_local_field_store_exit/main.omg`

- L23: `room` — field store — fix: `let mut room`
  - `room.label = main.source.label;`
### `tests/omega/pass/text/utf8_member_verdict/main.omg`

- L35: `decoded` — field store — fix: `let mut decoded`
  - `decoded.count = 0;`
- L36: `decoded` — field store — fix: `let mut decoded`
  - `decoded.label = "";`
- L37: `decoded` — writable borrow — fix: `let mut decoded`
  - `LabelMessage::decode(&mut decoded, &main.buffer, &mut main.read, &mut main.verdict);`
- L50: `second` — field store — fix: `let mut second`
  - `second.count = 0;`
- L51: `second` — field store — fix: `let mut second`
  - `second.label = "";`
- L52: `second` — writable borrow — fix: `let mut second`
  - `LabelMessage::decode(&mut second, &main.buffer, &mut main.read, &mut main.verdict);`
### `tests/omega/pass/time/runtime_duration_units_and_scaling_exit/main.omg`

- L8: `milliseconds` — field store — fix: `let mut milliseconds`
  - `milliseconds.seconds == 2 && milliseconds.subsecond_nanoseconds == 1000000`
### `tests/omega/pass/traits/runtime_dyn_byte_array_result/main.omg`

- L32: `bytes` — index store — fix: `let mut bytes`
  - `bytes[0]`
### `tests/omega/pass/traits/runtime_dyn_byte_record_result/main.omg`

- L33: `bytes` — index store — fix: `let mut bytes`
  - `bytes[0]`
### `tests/omega/pass/traits/runtime_dyn_byte_subview_join/main.omg`

- L31: `bytes` — index store — fix: `let mut bytes`
  - `bytes[0]`
### `tests/omega/pass/traits/runtime_dyn_stored_byte_input/main.omg`

- L32: `bytes` — index store — fix: `let mut bytes`
  - `bytes[0]`
### `tests/omega/pass/versioning/runtime_version_migration_exit/main.omg`

- L32: `current` — writable borrow — fix: `let mut current`
  - `Counter::from_v1(old, &mut current);`
### `tests/omega/pass/wire/runtime_wire_decode_byte_slice_exit/main.omg`

- L51: `decoded` — writable borrow — fix: `let mut decoded`
  - `BlobCodec::decode(&mut decoded, &main.buffer, &mut main.read, &mut main.verdict);`
### `tests/omega/pass/wire/runtime_wire_decode_predicate_capture_exit/main.omg`

- L48: `bad` — writable borrow — fix: `let mut bad`
  - `&mut bad,`
- L60: `good` — writable borrow — fix: `let mut good`
  - `&mut good,`
### `tests/omega/pass/wire/runtime_wire_decode_ranged_field_exit/main.omg`

- L37: `bad` — writable borrow — fix: `let mut bad`
  - `&mut bad,`
- L49: `good` — writable borrow — fix: `let mut good`
  - `&mut good,`
### `tests/omega/pass/wire/runtime_wire_decode_rejects_noncanonical_varint_exit/main.omg`

- L36: `overlong` — writable borrow — fix: `let mut overlong`
  - `&mut overlong,`
- L50: `overflow` — writable borrow — fix: `let mut overflow`
  - `&mut overflow,`
- L63: `good` — writable borrow — fix: `let mut good`
  - `&mut good,`
### `tests/omega/pass/wire/runtime_wire_decode_rejects_wrong_era_exit/main.omg`

- L55: `decoded` — writable borrow — fix: `let mut decoded`
  - `CounterMessageCodec::decode(&mut decoded, &buffer, &mut main.read, &mut main.verdict);`
### `tests/omega/pass/wire/runtime_wire_policy_authored_plan_exit/main.omg`

- L71: `decoded` — writable borrow — fix: `let mut decoded`
  - `PolicyCodec::decode(&mut decoded, &main.buffer, &mut main.read, &mut main.verdict);`
### `tests/omega/pass/wire/runtime_wire_signed_extremes_roundtrip_exit/main.omg`

- L37: `buffer` — writable borrow — fix: `let mut buffer`
  - `ExtremesCodec::encode(&value, &mut buffer, &mut written);`
- L37: `written` — writable borrow — fix: `let mut written`
  - `ExtremesCodec::encode(&value, &mut buffer, &mut written);`
- L38: `decoded` — writable borrow — fix: `let mut decoded`
  - `ExtremesCodec::decode(&mut decoded, &buffer, &mut read, &mut verdict);`
- L38: `read` — writable borrow — fix: `let mut read`
  - `ExtremesCodec::decode(&mut decoded, &buffer, &mut read, &mut verdict);`
- L38: `verdict` — writable borrow — fix: `let mut verdict`
  - `ExtremesCodec::decode(&mut decoded, &buffer, &mut read, &mut verdict);`
### `tests/omega/pass/wire/runtime_wire_utf8_edge_verdicts_exit/main.omg`

- L43: `decoded` — field store — fix: `let mut decoded`
  - `decoded.count = 0;`
- L44: `decoded` — field store — fix: `let mut decoded`
  - `decoded.label = "";`
- L45: `decoded` — writable borrow — fix: `let mut decoded`
  - `LabelCodec::decode(&mut decoded, &main.buffer, &mut main.read, &mut main.verdict);`
- L55: `d2` — field store — fix: `let mut d2`
  - `d2.count = 0;`
- L56: `d2` — field store — fix: `let mut d2`
  - `d2.label = "";`
- L57: `d2` — writable borrow — fix: `let mut d2`
  - `LabelCodec::decode(&mut d2, &main.buffer, &mut main.read, &mut main.verdict);`
- L68: `d3` — field store — fix: `let mut d3`
  - `d3.count = 0;`
- L69: `d3` — field store — fix: `let mut d3`
  - `d3.label = "";`
- L70: `d3` — writable borrow — fix: `let mut d3`
  - `LabelCodec::decode(&mut d3, &main.buffer, &mut main.read, &mut main.verdict);`
- L81: `d4` — field store — fix: `let mut d4`
  - `d4.count = 0;`
- L82: `d4` — field store — fix: `let mut d4`
  - `d4.label = "";`
- L83: `d4` — writable borrow — fix: `let mut d4`
  - `LabelCodec::decode(&mut d4, &main.buffer, &mut main.read, &mut main.verdict);`
- L94: `d5` — field store — fix: `let mut d5`
  - `d5.count = 0;`
- L95: `d5` — field store — fix: `let mut d5`
  - `d5.label = "";`
- L96: `d5` — writable borrow — fix: `let mut d5`
  - `LabelCodec::decode(&mut d5, &main.buffer, &mut main.read, &mut main.verdict);`
### `tests/omega/pass/wire/systems_wire_protocol_exit/main.omg`

- L82: `decoded` — writable borrow — fix: `let mut decoded`
  - `SensorPacketCodec::decode(&mut decoded, &main.buffer, &mut main.read, &mut main.verdict);`
### `tests/omega/pass/write_only/returned_destination_transport_exit/main.omg`

- L31: `destination` — assignment **review** — fix: `let mut destination`
  - `destination = 42;`
- L50: `moved` — assignment **review** — fix: `let mut moved`
  - `moved = 47;`
### `tests/omega/run/borrows/indexed_loan_element_write/main.omg`

- L15: `borrowed` — assignment **review** — fix: `let mut borrowed`
  - `borrowed = 9;`
### `tests/omega/run/borrows/local_mut_borrow/main.omg`

- L12: `borrowed` — assignment **review** — fix: `let mut borrowed`
  - `borrowed = 9;`
### `tests/omega/run/core/content_reunion/main.omg`

- L133: `field` — field store — fix: `let mut field`
  - `field.base == 10 && field.length == 48 &&`
- L134: `state_result` — field store — fix: `let mut state_result`
  - `state_result.base == 10 && state_result.length == 48 &&`
- L135: `nested` — field store — fix: `let mut nested`
  - `nested.base == 10 && nested.length == 48 &&`
- L136: `composed` — field store — fix: `let mut composed`
  - `composed.base == 10 && composed.length == 48 &&`
- L137: `returned_local` — field store — fix: `let mut returned_local`
  - `returned_local.base == 10 && returned_local.length == 48 &&`
- L138: `anonymous` — field store — fix: `let mut anonymous`
  - `anonymous.base == 10 && anonymous.length == 48;`
### `tests/omega/run/domains/qualified_fixed_byte_array_forwarding_exit/main.omg`

- L63: `local` — writable borrow — fix: `let mut local`
  - `let local_loan: &mut [u8; 8] in Ascii = returned(&mut local);`
- L67: `plain_rows` — writable borrow — fix: `let mut plain_rows`
  - `let plain_row: &mut [u8; 8] = select_plain(&mut plain_rows);`
- L71: `qualified_rows` — writable borrow — fix: `let mut qualified_rows`
  - `let qualified_row: &mut [u8; 8] in Ascii = select_qualified(&mut qualified_rows);`
### `tests/omega/run/domains/qualified_record_view_reference_erased_storage_exit/main.omg`

- L14: `rooms` — writable borrow — fix: `let mut rooms`
  - `update(&mut rooms);`

## `samples` (449 hits)

### `samples/cli/arithmetic/led_mixer/main.omg`

- L26: `channel` — assignment — fix: `let mut channel`
  - `channel = channel - 90;`
- L32: `channel` — assignment — fix: `let mut channel`
  - `channel = channel * 3;`
### `samples/cli/facts/array_element_default_domain/call_result/invalid/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `samples/cli/facts/array_element_default_domain/call_result/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `samples/cli/facts/array_element_default_domain/const_name/invalid/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `samples/cli/facts/array_element_default_domain/const_name/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `samples/cli/facts/array_element_default_domain/field/invalid/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `samples/cli/facts/array_element_default_domain/field/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `samples/cli/facts/array_element_default_domain/local/invalid/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `samples/cli/facts/array_element_default_domain/local/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `samples/cli/facts/array_element_default_domain/match_join/invalid/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `samples/cli/facts/array_element_default_domain/match_join/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `samples/cli/facts/array_element_default_domain/other_machine/invalid/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `samples/cli/facts/array_element_default_domain/other_machine/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `samples/cli/facts/array_element_default_domain/other_state/invalid/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `samples/cli/facts/array_element_default_domain/other_state/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `samples/cli/facts/array_element_default_domain/parameter/invalid/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `samples/cli/facts/array_element_default_domain/parameter/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `samples/cli/facts/array_element_default_domain/target_body/invalid/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `samples/cli/facts/array_element_default_domain/target_body/main.omg`

- L20: `mapping` — index store — fix: `let mut mapping`
  - `mapping[value as u64]`
### `samples/cli/facts/assumed_binding_input_authority/call_result/invalid/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `samples/cli/facts/assumed_binding_input_authority/call_result/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `samples/cli/facts/assumed_binding_input_authority/const_name/invalid/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `samples/cli/facts/assumed_binding_input_authority/const_name/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `samples/cli/facts/assumed_binding_input_authority/field/invalid/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `samples/cli/facts/assumed_binding_input_authority/field/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `samples/cli/facts/assumed_binding_input_authority/local/invalid/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `samples/cli/facts/assumed_binding_input_authority/local/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `samples/cli/facts/assumed_binding_input_authority/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `samples/cli/facts/assumed_binding_input_authority/match_join/invalid/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `samples/cli/facts/assumed_binding_input_authority/match_join/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `samples/cli/facts/assumed_binding_input_authority/other_machine/invalid/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `samples/cli/facts/assumed_binding_input_authority/other_machine/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `samples/cli/facts/assumed_binding_input_authority/other_state/invalid/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `samples/cli/facts/assumed_binding_input_authority/other_state/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `samples/cli/facts/assumed_binding_input_authority/parameter/invalid/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `samples/cli/facts/assumed_binding_input_authority/parameter/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `samples/cli/facts/assumed_binding_input_authority/target_body/invalid/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `samples/cli/facts/assumed_binding_input_authority/target_body/main.omg`

- L10: `values` — index store — fix: `let mut values`
  - `values[index.value]`
### `samples/cli/facts/assumed_binding_result_bound/match_join/invalid/main.omg`

- L9: `values` — index store — fix: `let mut values`
  - `values[index]`
### `samples/cli/facts/assumed_binding_result_bound/match_join/main.omg`

- L9: `values` — index store — fix: `let mut values`
  - `values[index]`
### `samples/cli/facts/assumed_binding_result_bound/other_machine/invalid/main.omg`

- L9: `values` — index store — fix: `let mut values`
  - `values[index]`
### `samples/cli/facts/assumed_binding_result_bound/other_machine/main.omg`

- L9: `values` — index store — fix: `let mut values`
  - `values[index]`
### `samples/cli/facts/assumed_binding_result_bound/target_body/invalid/main.omg`

- L31: `values` — index store — fix: `let mut values`
  - `values[Required::get()]`
- L39: `values` — index store — fix: `let mut values`
  - `values[Required::get()]`
### `samples/cli/facts/assumed_binding_result_bound/target_body/main.omg`

- L31: `values` — index store — fix: `let mut values`
  - `values[Required::get()]`
- L39: `values` — index store — fix: `let mut values`
  - `values[Required::get()]`
### `samples/cli/facts/c_layout_nested_representation/match_join/invalid/main.omg`

- L30: `value` — field store — fix: `let mut value`
  - `value.first == 17u8 && value.child.first == 23u8 && value.child.second == 42u64 && value.child.last == 9u16 && value.last == 11u16 && mapped.first == 17u8 && mapped.child.first == 23u8 && mapped.child.second == 42u64 && mapped.child.last == 9u16 && mapped.last == 11u16 && size_of<CLayout<Record>>() == 56u64 && align_of<CLayout<Record>>() == 8u64`
### `samples/cli/facts/c_layout_nested_representation/match_join/main.omg`

- L30: `value` — field store — fix: `let mut value`
  - `value.first == 17u8 && value.child.first == 23u8 && value.child.second == 42u64 && value.child.last == 9u16 && value.last == 11u16 && mapped.first == 17u8 && mapped.child.first == 23u8 && mapped.child.second == 42u64 && mapped.child.last == 9u16 && mapped.last == 11u16 && size_of<CLayout<Record>>() == 56u64 && align_of<CLayout<Record>>() == 8u64`
### `samples/cli/facts/c_layout_nested_representation/other_machine/invalid/main.omg`

- L39: `value` — field store — fix: `let mut value`
  - `value.first == 17u8 && value.child.first == 23u8 && value.child.second == 42u64 && value.child.last == 9u16 && value.last == 11u16 && mapped.first == 17u8 && mapped.child.first == 23u8 && mapped.child.second == 42u64 && mapped.child.last == 9u16 && mapped.last == 11u16 && size_of<CLayout<Record>>() == 56u64 && align_of<CLayout<Record>>() == 8u64`
### `samples/cli/facts/c_layout_nested_representation/other_machine/main.omg`

- L39: `value` — field store — fix: `let mut value`
  - `value.first == 17u8 && value.child.first == 23u8 && value.child.second == 42u64 && value.child.last == 9u16 && value.last == 11u16 && mapped.first == 17u8 && mapped.child.first == 23u8 && mapped.child.second == 42u64 && mapped.child.last == 9u16 && mapped.last == 11u16 && size_of<CLayout<Record>>() == 56u64 && align_of<CLayout<Record>>() == 8u64`
### `samples/cli/facts/c_layout_scalar_record/match_join/invalid/main.omg`

- L22: `value` — field store — fix: `let mut value`
  - `value.first == 17u8 && value.second == 42u64 && value.last == 9u16 && mapped.first == 17u8 && mapped.second == 42u64 && mapped.last == 9u16 && size_of<CLayout<Record>>() == 24u64 && align_of<CLayout<Record>>() == 8u64`
### `samples/cli/facts/c_layout_scalar_record/match_join/main.omg`

- L22: `value` — field store — fix: `let mut value`
  - `value.first == 17u8 && value.second == 42u64 && value.last == 9u16 && mapped.first == 17u8 && mapped.second == 42u64 && mapped.last == 9u16 && size_of<CLayout<Record>>() == 24u64 && align_of<CLayout<Record>>() == 8u64`
### `samples/cli/facts/c_layout_scalar_record/other_machine/invalid/main.omg`

- L26: `value` — field store — fix: `let mut value`
  - `value.first == 17u8 && value.second == 42u64 && value.last == 9u16 && mapped.first == 17u8 && mapped.second == 42u64 && mapped.last == 9u16 && size_of<CLayout<Record>>() == 24u64 && align_of<CLayout<Record>>() == 8u64`
### `samples/cli/facts/c_layout_scalar_record/other_machine/main.omg`

- L26: `value` — field store — fix: `let mut value`
  - `value.first == 17u8 && value.second == 42u64 && value.last == 9u16 && mapped.first == 17u8 && mapped.second == 42u64 && mapped.last == 9u16 && size_of<CLayout<Record>>() == 24u64 && align_of<CLayout<Record>>() == 8u64`
### `samples/cli/facts/case_payload_exclusive_loan/call_result/invalid/main.omg`

- L26: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/case_payload_exclusive_loan/call_result/main.omg`

- L25: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/case_payload_exclusive_loan/const_name/invalid/main.omg`

- L25: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/case_payload_exclusive_loan/const_name/main.omg`

- L24: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/case_payload_exclusive_loan/field/invalid/main.omg`

- L25: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/case_payload_exclusive_loan/field/main.omg`

- L24: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/case_payload_exclusive_loan/local/invalid/main.omg`

- L22: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/case_payload_exclusive_loan/local/main.omg`

- L21: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/case_payload_exclusive_loan/main.omg`

- L21: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/case_payload_exclusive_loan/match_join/invalid/main.omg`

- L28: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/case_payload_exclusive_loan/match_join/main.omg`

- L27: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/case_payload_exclusive_loan/other_machine/invalid/main.omg`

- L23: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/case_payload_exclusive_loan/other_machine/main.omg`

- L22: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/case_payload_exclusive_loan/other_state/invalid/main.omg`

- L24: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/case_payload_exclusive_loan/other_state/main.omg`

- L23: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/case_payload_exclusive_loan/parameter/invalid/main.omg`

- L23: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/case_payload_exclusive_loan/parameter/main.omg`

- L22: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/case_payload_exclusive_loan/target_body/invalid/main.omg`

- L48: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
- L64: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/case_payload_exclusive_loan/target_body/main.omg`

- L47: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
- L62: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/checked_predicate_disjoint_loans/call_result/invalid/main.omg`

- L13: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L14: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `samples/cli/facts/checked_predicate_disjoint_loans/call_result/main.omg`

- L13: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L14: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `samples/cli/facts/checked_predicate_disjoint_loans/const_name/invalid/main.omg`

- L11: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L12: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `samples/cli/facts/checked_predicate_disjoint_loans/const_name/main.omg`

- L11: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L12: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `samples/cli/facts/checked_predicate_disjoint_loans/field/invalid/main.omg`

- L12: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L13: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `samples/cli/facts/checked_predicate_disjoint_loans/field/main.omg`

- L12: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L13: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `samples/cli/facts/checked_predicate_disjoint_loans/local/invalid/main.omg`

- L12: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L13: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `samples/cli/facts/checked_predicate_disjoint_loans/local/main.omg`

- L12: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L13: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `samples/cli/facts/checked_predicate_disjoint_loans/main.omg`

- L11: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L12: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `samples/cli/facts/checked_predicate_disjoint_loans/match_join/invalid/main.omg`

- L13: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L14: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `samples/cli/facts/checked_predicate_disjoint_loans/match_join/main.omg`

- L13: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L14: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `samples/cli/facts/checked_predicate_disjoint_loans/other_machine/invalid/main.omg`

- L11: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L12: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `samples/cli/facts/checked_predicate_disjoint_loans/other_machine/main.omg`

- L11: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L12: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `samples/cli/facts/checked_predicate_disjoint_loans/other_state/invalid/main.omg`

- L16: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L17: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `samples/cli/facts/checked_predicate_disjoint_loans/other_state/main.omg`

- L16: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L17: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `samples/cli/facts/checked_predicate_disjoint_loans/parameter/invalid/main.omg`

- L11: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L12: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `samples/cli/facts/checked_predicate_disjoint_loans/parameter/main.omg`

- L11: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L12: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `samples/cli/facts/checked_predicate_disjoint_loans/target_body/invalid/main.omg`

- L37: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L38: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
- L49: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L50: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `samples/cli/facts/checked_predicate_disjoint_loans/target_body/main.omg`

- L37: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L38: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
- L49: `first` — assignment **review** — fix: `let mut first`
  - `first = Cell { value: 17 };`
- L50: `second` — assignment **review** — fix: `let mut second`
  - `second = Cell { value: 53 };`
### `samples/cli/facts/content_reunion/main.omg`

- L194: `field` — field store — fix: `let mut field`
  - `field.base == 10 && field.length == 48 &&`
- L195: `state_result` — field store — fix: `let mut state_result`
  - `state_result.base == 10 && state_result.length == 48 &&`
- L196: `nested` — field store — fix: `let mut nested`
  - `nested.base == 10 && nested.length == 48 &&`
- L197: `composed` — field store — fix: `let mut composed`
  - `composed.base == 10 && composed.length == 48 &&`
- L198: `returned_local` — field store — fix: `let mut returned_local`
  - `returned_local.base == 10 && returned_local.length == 48 &&`
- L199: `anonymous` — field store — fix: `let mut anonymous`
  - `anonymous.base == 10 && anonymous.length == 48;`
### `samples/cli/facts/copied_case_before_later_argument/other_state/invalid/main.omg`

- L21: `chosen` — writable borrow — fix: `let mut chosen`
  - `let observed: u64 = read_payload(chosen, set_to_absent(&mut chosen));`
### `samples/cli/facts/copied_case_before_later_argument/other_state/main.omg`

- L21: `chosen` — writable borrow — fix: `let mut chosen`
  - `let observed: u64 = read_payload(chosen, set_to_absent(&mut chosen));`
### `samples/cli/facts/darwin_stat_abi/match_join/invalid/main.omg`

- L49: `transported` — field store — fix: `let mut transported`
  - `transported.dev == -2147483647i32 && transported.mode == 33188u16 && transported.nlink == 3u16 && transported.ino == 4294967298u64 && transported.uid == 123u32 && transported.gid == 456u32 && transported.rdev == -123456789i32 && transported.atime.seconds == -17i64 && transported.atime.nanoseconds == 123456789i64 && transported.mtime.seconds == 42i64 && transported.mtime.nanoseconds == 987654321i64 && transported.ctime.seconds == -9i64 && transported.ctime.nanoseconds == 111111111i64 && transported.btime.seconds == 77i64 && transported.btime.nanoseconds == 222222222i64 && transported.size == 8589934593i64 && transported.blocks == 123456i64 && transported.blksize == 4096i32 && transported.flags == 2147483649u32 && transported.generation == 2882400001u32 && transported.reserved == -314159i32 && transported.spare[0] == -4294967297i64 && transported.spare[1] == 68719476737i64 && StatExchange::check(&transported) == 0u64 && StatExchange::extent() == size_of<DarwinStat>() && StatExchange::alignment() == align_of<DarwinStat>() && size_of<DarwinStat>() == 144u64 && align_of<DarwinStat>() == 8u64 && widen_u32_to_u64(narrow_i32_to_u32_wrapping(transported.dev)) == 2147483649u64 && widen_u32_to_u64(narrow_i32_to_u32_wrapping(transported.rdev)) == 4171510507u64`
### `samples/cli/facts/darwin_stat_abi/match_join/main.omg`

- L49: `transported` — field store — fix: `let mut transported`
  - `transported.dev == -2147483647i32 && transported.mode == 33188u16 && transported.nlink == 3u16 && transported.ino == 4294967298u64 && transported.uid == 123u32 && transported.gid == 456u32 && transported.rdev == -123456789i32 && transported.atime.seconds == -17i64 && transported.atime.nanoseconds == 123456789i64 && transported.mtime.seconds == 42i64 && transported.mtime.nanoseconds == 987654321i64 && transported.ctime.seconds == -9i64 && transported.ctime.nanoseconds == 111111111i64 && transported.btime.seconds == 77i64 && transported.btime.nanoseconds == 222222222i64 && transported.size == 8589934593i64 && transported.blocks == 123456i64 && transported.blksize == 4096i32 && transported.flags == 2147483649u32 && transported.generation == 2882400001u32 && transported.reserved == -314159i32 && transported.spare[0] == -4294967297i64 && transported.spare[1] == 68719476737i64 && StatExchange::check(&transported) == 0u64 && StatExchange::extent() == size_of<DarwinStat>() && StatExchange::alignment() == align_of<DarwinStat>() && size_of<DarwinStat>() == 144u64 && align_of<DarwinStat>() == 8u64 && widen_u32_to_u64(narrow_i32_to_u32_wrapping(transported.dev)) == 2147483649u64 && widen_u32_to_u64(narrow_i32_to_u32_wrapping(transported.rdev)) == 4171510507u64`
### `samples/cli/facts/exclusive_match_arm/call_result/invalid/main.omg`

- L23: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/exclusive_match_arm/call_result/main.omg`

- L23: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/exclusive_match_arm/const_name/invalid/main.omg`

- L21: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/exclusive_match_arm/const_name/main.omg`

- L21: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/exclusive_match_arm/field/invalid/main.omg`

- L20: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/exclusive_match_arm/field/main.omg`

- L20: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/exclusive_match_arm/local/invalid/main.omg`

- L19: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/exclusive_match_arm/local/main.omg`

- L19: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/exclusive_match_arm/main.omg`

- L18: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/exclusive_match_arm/match_join/invalid/main.omg`

- L24: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/exclusive_match_arm/match_join/main.omg`

- L24: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/exclusive_match_arm/other_machine/invalid/main.omg`

- L16: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/exclusive_match_arm/other_machine/main.omg`

- L16: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/exclusive_match_arm/other_state/invalid/main.omg`

- L18: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/exclusive_match_arm/other_state/main.omg`

- L18: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/exclusive_match_arm/parameter/invalid/main.omg`

- L16: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/exclusive_match_arm/parameter/main.omg`

- L16: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/exclusive_match_arm/target_body/invalid/main.omg`

- L45: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
- L60: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/exclusive_match_arm/target_body/main.omg`

- L45: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
- L59: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/fixed_array_eight_elem_index_bound/local/invalid/main.omg`

- L9: `local_values` — index store — fix: `let mut local_values`
  - `local_values[local_index]`
### `samples/cli/facts/fixed_array_eight_elem_index_bound/local/main.omg`

- L9: `local_values` — index store — fix: `let mut local_values`
  - `local_values[local_index]`
### `samples/cli/facts/fixed_array_onethousandtwentyfour_elem_index_bound/local/invalid/main.omg`

- L10: `local_arr` — index store — fix: `let mut local_arr`
  - `local_arr[local_idx]`
### `samples/cli/facts/fixed_array_onethousandtwentyfour_elem_index_bound/local/main.omg`

- L10: `local_arr` — index store — fix: `let mut local_arr`
  - `local_arr[local_idx]`
### `samples/cli/facts/fixed_array_twofiftysix_elem_index_bound/local/invalid/main.omg`

- L10: `local_arr` — index store — fix: `let mut local_arr`
  - `local_arr[local_idx]`
### `samples/cli/facts/fixed_array_twofiftysix_elem_index_bound/local/main.omg`

- L10: `local_arr` — index store — fix: `let mut local_arr`
  - `local_arr[local_idx]`
### `samples/cli/facts/fixed_array_value/local/invalid/main.omg`

- L9: `local_values` — index store — fix: `let mut local_values`
  - `local_values[local_index]`
### `samples/cli/facts/fixed_array_value/local/main.omg`

- L9: `local_values` — index store — fix: `let mut local_values`
  - `local_values[local_index]`
### `samples/cli/facts/fixed_layout_reference_readback/const_name/invalid/main.omg`

- L36: `result` — field store — fix: `let mut result`
  - `result.value == 7u64 && result.pointer.value == 43u64`
### `samples/cli/facts/fixed_layout_reference_readback/const_name/main.omg`

- L36: `result` — field store — fix: `let mut result`
  - `result.value == 7u64 && result.pointer.value == 43u64`
### `samples/cli/facts/fixed_layout_reference_readback/local/invalid/main.omg`

- L33: `result` — field store — fix: `let mut result`
  - `result.value == 7u64 && result.pointer.value == 43u64`
### `samples/cli/facts/fixed_layout_reference_readback/local/main.omg`

- L33: `result` — field store — fix: `let mut result`
  - `result.value == 7u64 && result.pointer.value == 43u64`
### `samples/cli/facts/fixed_layout_reference_readback/main.omg`

- L32: `result` — field store — fix: `let mut result`
  - `result.value == 7u64 && result.pointer.value == 43u64`
### `samples/cli/facts/fixed_layout_reference_readback/match_join/invalid/main.omg`

- L37: `result` — field store — fix: `let mut result`
  - `result.value == 7u64 && result.pointer.value == 43u64 && result.selected_first == choose_first`
### `samples/cli/facts/fixed_layout_reference_readback/match_join/main.omg`

- L37: `result` — field store — fix: `let mut result`
  - `result.value == 7u64 && result.pointer.value == 43u64 && result.selected_first == choose_first`
### `samples/cli/facts/fixed_layout_reference_readback/other_machine/invalid/main.omg`

- L35: `forwarded` — field store — fix: `let mut forwarded`
  - `forwarded.value == 7u64 && forwarded.pointer.value == 43u64`
### `samples/cli/facts/fixed_layout_reference_readback/other_machine/main.omg`

- L35: `forwarded` — field store — fix: `let mut forwarded`
  - `forwarded.value == 7u64 && forwarded.pointer.value == 43u64`
### `samples/cli/facts/fixed_layout_reference_readback/other_state/invalid/main.omg`

- L35: `result` — field store — fix: `let mut result`
  - `result.value == 7u64 && result.pointer.value == 43u64`
### `samples/cli/facts/fixed_layout_reference_readback/other_state/main.omg`

- L35: `result` — field store — fix: `let mut result`
  - `result.value == 7u64 && result.pointer.value == 43u64`
### `samples/cli/facts/foreign_indirect_float_record_result/match_join/invalid/main.omg`

- L46: `selected` — field store — fix: `let mut selected`
  - `selected.branch == choice && receive(selected.value)`
### `samples/cli/facts/foreign_indirect_float_record_result/match_join/main.omg`

- L46: `selected` — field store — fix: `let mut selected`
  - `selected.branch == choice && receive(selected.value)`
### `samples/cli/facts/indexed_element_mut_loan/call_result/invalid/main.omg`

- L16: `rooms` — writable borrow — fix: `let mut rooms`
  - `let held: &mut Room = &mut rooms[selected];`
- L17: `rooms` — writable borrow — fix: `let mut rooms`
  - `bump(&mut rooms[selected]);`
### `samples/cli/facts/indexed_element_mut_loan/call_result/main.omg`

- L16: `rooms` — writable borrow — fix: `let mut rooms`
  - `bump(&mut rooms[selected]);`
### `samples/cli/facts/indexed_element_mut_loan/const_name/invalid/main.omg`

- L14: `rooms` — writable borrow — fix: `let mut rooms`
  - `let held: &mut Room = &mut rooms[selected];`
- L15: `rooms` — writable borrow — fix: `let mut rooms`
  - `bump(&mut rooms[selected]);`
### `samples/cli/facts/indexed_element_mut_loan/const_name/main.omg`

- L14: `rooms` — writable borrow — fix: `let mut rooms`
  - `bump(&mut rooms[selected]);`
### `samples/cli/facts/indexed_element_mut_loan/field/invalid/main.omg`

- L14: `rooms` — writable borrow — fix: `let mut rooms`
  - `let held: &mut Room = &mut rooms[selector.index];`
- L15: `rooms` — writable borrow — fix: `let mut rooms`
  - `bump(&mut rooms[selector.index]);`
### `samples/cli/facts/indexed_element_mut_loan/field/main.omg`

- L14: `rooms` — writable borrow — fix: `let mut rooms`
  - `bump(&mut rooms[selector.index]);`
### `samples/cli/facts/indexed_element_mut_loan/local/invalid/main.omg`

- L13: `rooms` — writable borrow — fix: `let mut rooms`
  - `let held: &mut Room = &mut rooms[selected];`
- L14: `rooms` — writable borrow — fix: `let mut rooms`
  - `bump(&mut rooms[selected]);`
### `samples/cli/facts/indexed_element_mut_loan/local/main.omg`

- L13: `rooms` — writable borrow — fix: `let mut rooms`
  - `bump(&mut rooms[selected]);`
### `samples/cli/facts/indexed_element_mut_loan/main.omg`

- L23: `rooms` — writable borrow — fix: `let mut rooms`
  - `bump(&mut rooms[from_index]);`
### `samples/cli/facts/indexed_element_mut_loan/match_join/invalid/main.omg`

- L13: `rooms` — writable borrow — fix: `let mut rooms`
  - `let held: &mut Room = &mut rooms[selected];`
- L14: `rooms` — writable borrow — fix: `let mut rooms`
  - `bump(&mut rooms[selected]);`
### `samples/cli/facts/indexed_element_mut_loan/match_join/main.omg`

- L13: `rooms` — writable borrow — fix: `let mut rooms`
  - `bump(&mut rooms[selected]);`
### `samples/cli/facts/indexed_element_mut_loan/other_machine/invalid/main.omg`

- L22: `rooms` — writable borrow — fix: `let mut rooms`
  - `forward(&mut rooms, from_index);`
### `samples/cli/facts/indexed_element_mut_loan/other_machine/main.omg`

- L20: `rooms` — writable borrow — fix: `let mut rooms`
  - `forward(&mut rooms, from_index);`
### `samples/cli/facts/indexed_element_mut_loan/other_state/invalid/main.omg`

- L12: `rooms` — writable borrow — fix: `let mut rooms`
  - `transition { _ -> apply(&mut rooms, from_index) }`
- L16: `rooms` — writable borrow — fix: `let mut rooms`
  - `let held: &mut Room = &mut rooms[selected];`
- L17: `rooms` — writable borrow — fix: `let mut rooms`
  - `bump(&mut rooms[selected]);`
### `samples/cli/facts/indexed_element_mut_loan/other_state/main.omg`

- L12: `rooms` — writable borrow — fix: `let mut rooms`
  - `transition { _ -> apply(&mut rooms, from_index) }`
- L16: `rooms` — writable borrow — fix: `let mut rooms`
  - `bump(&mut rooms[selected]);`
### `samples/cli/facts/indexed_element_mut_loan/parameter/invalid/main.omg`

- L19: `rooms` — writable borrow — fix: `let mut rooms`
  - `apply(&mut rooms, from_index);`
### `samples/cli/facts/indexed_element_mut_loan/parameter/main.omg`

- L17: `rooms` — writable borrow — fix: `let mut rooms`
  - `apply(&mut rooms, from_index);`
### `samples/cli/facts/indexed_element_mut_loan/target_body/invalid/main.omg`

- L60: `rooms` — writable borrow — fix: `let mut rooms`
  - `SelectedApply::apply(&mut rooms, from_index);`
### `samples/cli/facts/indexed_element_mut_loan/target_body/main.omg`

- L56: `rooms` — writable borrow — fix: `let mut rooms`
  - `SelectedApply::apply(&mut rooms, from_index);`
### `samples/cli/facts/indexed_recoverable_replacement_window/call_result/invalid/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `samples/cli/facts/indexed_recoverable_replacement_window/call_result/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `samples/cli/facts/indexed_recoverable_replacement_window/const_name/invalid/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `samples/cli/facts/indexed_recoverable_replacement_window/const_name/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `samples/cli/facts/indexed_recoverable_replacement_window/field/invalid/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `samples/cli/facts/indexed_recoverable_replacement_window/field/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `samples/cli/facts/indexed_recoverable_replacement_window/local/invalid/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `samples/cli/facts/indexed_recoverable_replacement_window/local/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `samples/cli/facts/indexed_recoverable_replacement_window/match_join/invalid/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `samples/cli/facts/indexed_recoverable_replacement_window/match_join/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `samples/cli/facts/indexed_recoverable_replacement_window/other_machine/invalid/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `samples/cli/facts/indexed_recoverable_replacement_window/other_machine/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `samples/cli/facts/indexed_recoverable_replacement_window/other_state/invalid/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `samples/cli/facts/indexed_recoverable_replacement_window/other_state/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `samples/cli/facts/indexed_recoverable_replacement_window/parameter/invalid/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `samples/cli/facts/indexed_recoverable_replacement_window/parameter/main.omg`

- L30: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `samples/cli/facts/indexed_recoverable_replacement_window/target_body/invalid/main.omg`

- L59: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
- L73: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `samples/cli/facts/indexed_recoverable_replacement_window/target_body/main.omg`

- L59: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
- L73: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = recover(first);`
### `samples/cli/facts/indexed_storage_address_identity/call_result/invalid/main.omg`

- L9: `held` — assignment **review** — fix: `let mut held`
  - `held = 13;`
### `samples/cli/facts/indexed_storage_address_identity/const_name/invalid/main.omg`

- L9: `held` — assignment **review** — fix: `let mut held`
  - `held = 13;`
### `samples/cli/facts/indexed_storage_address_identity/field/invalid/main.omg`

- L9: `held` — assignment **review** — fix: `let mut held`
  - `held = 13;`
### `samples/cli/facts/indexed_storage_address_identity/local/invalid/main.omg`

- L8: `held` — assignment **review** — fix: `let mut held`
  - `held = 13;`
### `samples/cli/facts/indexed_storage_address_identity/match_join/invalid/main.omg`

- L8: `held` — assignment **review** — fix: `let mut held`
  - `held = 13;`
### `samples/cli/facts/indexed_storage_address_identity/other_machine/invalid/main.omg`

- L7: `held` — assignment **review** — fix: `let mut held`
  - `held = 13;`
### `samples/cli/facts/indexed_storage_address_identity/other_state/invalid/main.omg`

- L9: `held` — assignment **review** — fix: `let mut held`
  - `held = 13;`
### `samples/cli/facts/indexed_storage_address_identity/parameter/invalid/main.omg`

- L6: `held` — assignment **review** — fix: `let mut held`
  - `held = 13;`
### `samples/cli/facts/indexed_storage_address_identity/target_body/invalid/main.omg`

- L36: `held` — assignment **review** — fix: `let mut held`
  - `held = 13;`
- L55: `held` — assignment **review** — fix: `let mut held`
  - `held = 13;`
### `samples/cli/facts/length/call_result/invalid/main.omg`

- L13: `window` — index store — fix: `let mut window`
  - `window[length - 1]`
### `samples/cli/facts/length/call_result/main.omg`

- L11: `window` — index store — fix: `let mut window`
  - `window[length - 1]`
### `samples/cli/facts/length/const_name/invalid/main.omg`

- L9: `window` — index store — fix: `let mut window`
  - `window[LENGTH - 1]`
### `samples/cli/facts/length/const_name/main.omg`

- L8: `window` — index store — fix: `let mut window`
  - `window[LENGTH - 1]`
### `samples/cli/facts/length/field/invalid/main.omg`

- L10: `window` — index store — fix: `let mut window`
  - `window[meter.length - 1]`
### `samples/cli/facts/length/field/main.omg`

- L9: `window` — index store — fix: `let mut window`
  - `window[meter.length - 1]`
### `samples/cli/facts/length/local/invalid/main.omg`

- L9: `window` — index store — fix: `let mut window`
  - `window[length - 1]`
### `samples/cli/facts/length/local/main.omg`

- L8: `window` — index store — fix: `let mut window`
  - `window[length - 1]`
### `samples/cli/facts/length/main.omg`

- L8: `window` — index store — fix: `let mut window`
  - `window[length - 1]`
### `samples/cli/facts/length/match_join/invalid/main.omg`

- L9: `window` — index store — fix: `let mut window`
  - `window[length - 1]`
### `samples/cli/facts/length/match_join/main.omg`

- L8: `window` — index store — fix: `let mut window`
  - `window[length - 1]`
### `samples/cli/facts/match_computed_root/call_result/invalid/main.omg`

- L26: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/match_computed_root/call_result/main.omg`

- L25: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/match_computed_root/const_name/invalid/main.omg`

- L23: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/match_computed_root/const_name/main.omg`

- L22: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/match_computed_root/field/invalid/main.omg`

- L25: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/match_computed_root/field/main.omg`

- L24: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/match_computed_root/local/invalid/main.omg`

- L22: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/match_computed_root/local/main.omg`

- L21: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/match_computed_root/match_join/invalid/main.omg`

- L27: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/match_computed_root/match_join/main.omg`

- L26: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/match_computed_root/other_machine/invalid/main.omg`

- L19: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/match_computed_root/other_machine/main.omg`

- L18: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/match_computed_root/other_state/invalid/main.omg`

- L21: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/match_computed_root/other_state/main.omg`

- L20: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/match_computed_root/parameter/invalid/main.omg`

- L19: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/match_computed_root/parameter/main.omg`

- L18: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/match_computed_root/target_body/invalid/main.omg`

- L19: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/match_computed_root/target_body/main.omg`

- L18: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/match_effectful_subject_custody/other_state/invalid/main.omg`

- L28: `slot` — writable borrow — fix: `let mut slot`
  - `let held: Token = match refill(&mut slot, c, flag) {`
### `samples/cli/facts/match_effectful_subject_custody/other_state/main.omg`

- L28: `slot` — writable borrow — fix: `let mut slot`
  - `let held: Token = match refill(&mut slot, c, flag) {`
### `samples/cli/facts/nested_zero_leaf_array_storage/call_result/invalid/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `samples/cli/facts/nested_zero_leaf_array_storage/call_result/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `samples/cli/facts/nested_zero_leaf_array_storage/const_name/invalid/main.omg`

- L12: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `samples/cli/facts/nested_zero_leaf_array_storage/const_name/main.omg`

- L12: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `samples/cli/facts/nested_zero_leaf_array_storage/field/invalid/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `samples/cli/facts/nested_zero_leaf_array_storage/field/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `samples/cli/facts/nested_zero_leaf_array_storage/local/invalid/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `samples/cli/facts/nested_zero_leaf_array_storage/local/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `samples/cli/facts/nested_zero_leaf_array_storage/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `samples/cli/facts/nested_zero_leaf_array_storage/match_join/invalid/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `samples/cli/facts/nested_zero_leaf_array_storage/match_join/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `samples/cli/facts/nested_zero_leaf_array_storage/other_machine/invalid/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `samples/cli/facts/nested_zero_leaf_array_storage/other_machine/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `samples/cli/facts/nested_zero_leaf_array_storage/other_state/invalid/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `samples/cli/facts/nested_zero_leaf_array_storage/other_state/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `samples/cli/facts/nested_zero_leaf_array_storage/parameter/invalid/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `samples/cli/facts/nested_zero_leaf_array_storage/parameter/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `samples/cli/facts/nested_zero_leaf_array_storage/target_body/invalid/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `samples/cli/facts/nested_zero_leaf_array_storage/target_body/main.omg`

- L11: `view` — field store — fix: `let mut view`
  - `view.len == 0`
### `samples/cli/facts/nominal_loan_backing_cleanup/call_result/invalid/main.omg`

- L38: `room` — writable borrow — fix: `let mut room`
  - `transition { _ -> observe(&mut room) }`
### `samples/cli/facts/nominal_loan_backing_cleanup/call_result/main.omg`

- L39: `room` — writable borrow — fix: `let mut room`
  - `transition { _ -> observe(&mut room) }`
### `samples/cli/facts/nominal_loan_backing_cleanup/const_name/invalid/main.omg`

- L36: `room` — writable borrow — fix: `let mut room`
  - `transition { _ -> observe(&mut room) }`
### `samples/cli/facts/nominal_loan_backing_cleanup/const_name/main.omg`

- L37: `room` — writable borrow — fix: `let mut room`
  - `transition { _ -> observe(&mut room) }`
### `samples/cli/facts/nominal_loan_backing_cleanup/field/invalid/main.omg`

- L36: `holder` — writable borrow — fix: `let mut holder`
  - `transition { _ -> observe(&mut holder.room) }`
### `samples/cli/facts/nominal_loan_backing_cleanup/field/main.omg`

- L37: `holder` — writable borrow — fix: `let mut holder`
  - `transition { _ -> observe(&mut holder.room) }`
### `samples/cli/facts/nominal_loan_backing_cleanup/local/invalid/main.omg`

- L35: `room` — writable borrow — fix: `let mut room`
  - `transition { _ -> observe(&mut room) }`
### `samples/cli/facts/nominal_loan_backing_cleanup/local/main.omg`

- L36: `room` — writable borrow — fix: `let mut room`
  - `transition { _ -> observe(&mut room) }`
### `samples/cli/facts/nominal_loan_backing_cleanup/main.omg`

- L6: `room` — writable borrow — fix: `let mut room`
  - `transition { _ -> observe(&mut room) }`
### `samples/cli/facts/nominal_loan_backing_cleanup/match_join/invalid/main.omg`

- L35: `room` — writable borrow — fix: `let mut room`
  - `transition { _ -> observe(&mut room) }`
### `samples/cli/facts/nominal_loan_backing_cleanup/match_join/main.omg`

- L36: `room` — writable borrow — fix: `let mut room`
  - `transition { _ -> observe(&mut room) }`
### `samples/cli/facts/nominal_loan_backing_cleanup/other_machine/invalid/main.omg`

- L40: `room` — writable borrow — fix: `let mut room`
  - `transition { _ -> observe(&mut room) }`
### `samples/cli/facts/nominal_loan_backing_cleanup/other_machine/main.omg`

- L41: `room` — writable borrow — fix: `let mut room`
  - `transition { _ -> observe(&mut room) }`
### `samples/cli/facts/nominal_loan_backing_cleanup/target_body/invalid/main.omg`

- L40: `room` — writable borrow — fix: `let mut room`
  - `transition { _ -> observe(&mut room) }`
### `samples/cli/facts/nominal_loan_backing_cleanup/target_body/main.omg`

- L41: `room` — writable borrow — fix: `let mut room`
  - `transition { _ -> observe(&mut room) }`
### `samples/cli/facts/recoverable_replacement_window/call_result/invalid/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `samples/cli/facts/recoverable_replacement_window/call_result/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `samples/cli/facts/recoverable_replacement_window/const_name/invalid/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `samples/cli/facts/recoverable_replacement_window/const_name/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `samples/cli/facts/recoverable_replacement_window/field/invalid/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `samples/cli/facts/recoverable_replacement_window/field/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `samples/cli/facts/recoverable_replacement_window/local/invalid/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `samples/cli/facts/recoverable_replacement_window/local/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `samples/cli/facts/recoverable_replacement_window/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `samples/cli/facts/recoverable_replacement_window/match_join/invalid/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `samples/cli/facts/recoverable_replacement_window/match_join/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `samples/cli/facts/recoverable_replacement_window/other_machine/invalid/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `samples/cli/facts/recoverable_replacement_window/other_machine/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `samples/cli/facts/recoverable_replacement_window/other_state/invalid/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `samples/cli/facts/recoverable_replacement_window/other_state/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `samples/cli/facts/recoverable_replacement_window/parameter/invalid/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `samples/cli/facts/recoverable_replacement_window/parameter/main.omg`

- L29: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `samples/cli/facts/recoverable_replacement_window/target_body/invalid/main.omg`

- L58: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
- L75: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `samples/cli/facts/recoverable_replacement_window/target_body/main.omg`

- L58: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
- L75: `alias` — assignment **review** — fix: `let mut alias`
  - `alias = match first {`
### `samples/cli/facts/reference_arrival_handoff/call_result/invalid/main.omg`

- L9: `original` — assignment **review** — fix: `let mut original`
  - `original = 7;`
- L10: `child` — assignment **review** — fix: `let mut child`
  - `child = 41;`
### `samples/cli/facts/reference_arrival_handoff/call_result/main.omg`

- L9: `child` — assignment **review** — fix: `let mut child`
  - `child = 41;`
### `samples/cli/facts/reference_arrival_handoff/const_name/invalid/main.omg`

- L9: `original` — assignment **review** — fix: `let mut original`
  - `original = 7;`
- L10: `child` — assignment **review** — fix: `let mut child`
  - `child = WRITTEN;`
### `samples/cli/facts/reference_arrival_handoff/const_name/main.omg`

- L9: `child` — assignment **review** — fix: `let mut child`
  - `child = WRITTEN;`
### `samples/cli/facts/reference_arrival_handoff/field/invalid/main.omg`

- L9: `original` — assignment **review** — fix: `let mut original`
  - `original = 7;`
- L10: `holder` — field store — fix: `let mut holder`
  - `holder.slot = 41;`
### `samples/cli/facts/reference_arrival_handoff/field/main.omg`

- L9: `holder` — field store — fix: `let mut holder`
  - `holder.slot = 41;`
### `samples/cli/facts/reference_arrival_handoff/local/invalid/main.omg`

- L8: `original` — assignment **review** — fix: `let mut original`
  - `original = 7;`
- L9: `child` — assignment **review** — fix: `let mut child`
  - `child = 41;`
### `samples/cli/facts/reference_arrival_handoff/local/main.omg`

- L8: `child` — assignment **review** — fix: `let mut child`
  - `child = 41;`
### `samples/cli/facts/reference_arrival_handoff/main.omg`

- L7: `child` — assignment **review** — fix: `let mut child`
  - `child = 41;`
### `samples/cli/facts/reference_arrival_handoff/match_join/invalid/main.omg`

- L15: `child` — assignment **review** — fix: `let mut child`
  - `child = 41;`
### `samples/cli/facts/reference_arrival_handoff/match_join/main.omg`

- L14: `child` — assignment **review** — fix: `let mut child`
  - `child = 41;`
### `samples/cli/facts/reference_arrival_handoff/other_machine/invalid/main.omg`

- L11: `held` — assignment **review** — fix: `let mut held`
  - `held = 7;`
### `samples/cli/facts/reference_arrival_handoff/parameter/invalid/main.omg`

- L10: `held` — assignment **review** — fix: `let mut held`
  - `held = 7;`
### `samples/cli/facts/reference_arrival_handoff/target_body/invalid/main.omg`

- L53: `held` — assignment **review** — fix: `let mut held`
  - `held = 7;`
### `samples/cli/facts/replaced_field_loan/call_result/invalid/main.omg`

- L22: `selected` — writable borrow — fix: `let mut selected`
  - `let written: bool = store(&mut selected.body, 41);`
### `samples/cli/facts/replaced_field_loan/call_result/main.omg`

- L22: `selected` — writable borrow — fix: `let mut selected`
  - `let written: bool = store(&mut selected.body, 41);`
### `samples/cli/facts/replaced_field_loan/field/invalid/main.omg`

- L22: `wrapper` — writable borrow — fix: `let mut wrapper`
  - `let written: bool = store(&mut wrapper.message.body, 41);`
### `samples/cli/facts/replaced_field_loan/field/main.omg`

- L22: `wrapper` — writable borrow — fix: `let mut wrapper`
  - `let written: bool = store(&mut wrapper.message.body, 41);`
### `samples/cli/facts/replaced_field_loan/match_join/invalid/main.omg`

- L21: `selected` — writable borrow — fix: `let mut selected`
  - `let written: bool = store(&mut selected.body, 41);`
### `samples/cli/facts/replaced_field_loan/match_join/main.omg`

- L21: `selected` — writable borrow — fix: `let mut selected`
  - `let written: bool = store(&mut selected.body, 41);`
### `samples/cli/facts/returned_write_destination/call_result/invalid/main.omg`

- L9: `destination` — assignment **review** — fix: `let mut destination`
  - `destination = destination;`
### `samples/cli/facts/returned_write_destination/call_result/main.omg`

- L9: `destination` — assignment **review** — fix: `let mut destination`
  - `destination = 42;`
### `samples/cli/facts/returned_write_destination/const_name/invalid/main.omg`

- L8: `destination` — assignment **review** — fix: `let mut destination`
  - `destination = destination;`
### `samples/cli/facts/returned_write_destination/const_name/main.omg`

- L8: `destination` — assignment **review** — fix: `let mut destination`
  - `destination = 42;`
### `samples/cli/facts/returned_write_destination/field/invalid/main.omg`

- L9: `destination` — field store — fix: `let mut destination`
  - `destination.loan = destination.loan;`
### `samples/cli/facts/returned_write_destination/field/main.omg`

- L9: `destination` — field store — fix: `let mut destination`
  - `destination.loan = 42;`
### `samples/cli/facts/returned_write_destination/local/invalid/main.omg`

- L8: `destination` — assignment **review** — fix: `let mut destination`
  - `destination = destination;`
### `samples/cli/facts/returned_write_destination/local/main.omg`

- L8: `destination` — assignment **review** — fix: `let mut destination`
  - `destination = 42;`
### `samples/cli/facts/returned_write_destination/main.omg`

- L7: `destination` — assignment **review** — fix: `let mut destination`
  - `destination = 42;`
### `samples/cli/facts/returned_write_destination/match_join/invalid/main.omg`

- L6: `destination` — assignment **review** — fix: `let mut destination`
  - `destination = destination;`
### `samples/cli/facts/returned_write_destination/match_join/main.omg`

- L6: `destination` — assignment **review** — fix: `let mut destination`
  - `destination = 42;`
### `samples/cli/facts/returned_write_destination/target_body/invalid/main.omg`

- L44: `destination` — assignment **review** — fix: `let mut destination`
  - `destination = destination;`
### `samples/cli/facts/returned_write_destination/target_body/main.omg`

- L44: `destination` — assignment **review** — fix: `let mut destination`
  - `destination = 42;`
### `samples/cli/facts/signed_division_remainder/call_result/invalid/main.omg`

- L18: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `samples/cli/facts/signed_division_remainder/call_result/main.omg`

- L18: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `samples/cli/facts/signed_division_remainder/const_name/invalid/main.omg`

- L13: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `samples/cli/facts/signed_division_remainder/const_name/main.omg`

- L13: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `samples/cli/facts/signed_division_remainder/field/invalid/main.omg`

- L16: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `samples/cli/facts/signed_division_remainder/field/main.omg`

- L16: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `samples/cli/facts/signed_division_remainder/local/invalid/main.omg`

- L13: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `samples/cli/facts/signed_division_remainder/local/main.omg`

- L13: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `samples/cli/facts/signed_division_remainder/main.omg`

- L10: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `samples/cli/facts/signed_division_remainder/match_join/invalid/main.omg`

- L21: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `samples/cli/facts/signed_division_remainder/match_join/main.omg`

- L21: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `samples/cli/facts/signed_division_remainder/other_machine/invalid/main.omg`

- L11: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `samples/cli/facts/signed_division_remainder/other_machine/main.omg`

- L11: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `samples/cli/facts/signed_division_remainder/other_state/invalid/main.omg`

- L17: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `samples/cli/facts/signed_division_remainder/other_state/main.omg`

- L17: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `samples/cli/facts/signed_division_remainder/parameter/invalid/main.omg`

- L11: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `samples/cli/facts/signed_division_remainder/parameter/main.omg`

- L11: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `samples/cli/facts/signed_division_remainder/target_body/invalid/main.omg`

- L16: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
- L26: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `samples/cli/facts/signed_division_remainder/target_body/main.omg`

- L16: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
- L26: `arr` — index store — fix: `let mut arr`
  - `arr[idx]`
### `samples/cli/facts/view_extent_after_write/field/invalid/main.omg`

- L10: `holder` — writable borrow — fix: `let mut holder`
  - `write_first(&mut holder.view[0]);`
### `samples/cli/facts/view_extent_after_write/field/main.omg`

- L10: `holder` — writable borrow — fix: `let mut holder`
  - `write_first(&mut holder.view[0]);`
### `samples/cli/facts/write_only_indexed_reborrow/field/invalid/main.omg`

- L28: `held` — writable borrow — fix: `let mut held`
  - `replace(&write held.cells[storage.position], 7);`
- L29: `held` — writable borrow — fix: `let mut held`
  - `replace(&write held.cells[storage.position], 9);`
- L31: `held` — writable borrow — fix: `let mut held`
  - `replace(&write held.cells[storage.position], -43);`
### `samples/cli/facts/write_only_indexed_reborrow/field/main.omg`

- L28: `held` — writable borrow — fix: `let mut held`
  - `replace(&write held.cells[storage.position], 7);`
- L29: `held` — writable borrow — fix: `let mut held`
  - `replace(&write held.cells[storage.position], 9);`
- L31: `held` — writable borrow — fix: `let mut held`
  - `replace(&write held.cells[storage.position], -43);`
### `samples/cli/facts/write_only_match_join/call_result/invalid/main.omg`

- L27: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/write_only_match_join/call_result/main.omg`

- L26: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/write_only_match_join/const_name/invalid/main.omg`

- L26: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/write_only_match_join/const_name/main.omg`

- L25: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/write_only_match_join/field/invalid/main.omg`

- L26: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/write_only_match_join/field/main.omg`

- L25: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/write_only_match_join/local/invalid/main.omg`

- L23: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/write_only_match_join/local/main.omg`

- L22: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/write_only_match_join/match_join/invalid/main.omg`

- L29: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/write_only_match_join/match_join/main.omg`

- L28: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/write_only_match_join/other_machine/invalid/main.omg`

- L22: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/write_only_match_join/other_machine/main.omg`

- L21: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/write_only_match_join/other_state/invalid/main.omg`

- L26: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/write_only_match_join/other_state/main.omg`

- L25: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/write_only_match_join/parameter/invalid/main.omg`

- L22: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/write_only_match_join/parameter/main.omg`

- L21: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/write_only_match_join/target_body/invalid/main.omg`

- L51: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
- L66: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/facts/write_only_match_join/target_body/main.omg`

- L50: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
- L64: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 41;`
### `samples/cli/games/dungeon_crawler_cli/combat/combat.omg`

- L43: `reward` — writable borrow — fix: `let mut reward`
  - `RewardTables::roll_enemy_reward(&mut combat_system.rewards, event.enemy, random, &mut reward);`
### `samples/cli/games/dungeon_crawler_cli/generation/maze_builder.omg`

- L25: `random` — writable borrow — fix: `let mut random`
  - `Random::seed(seed, &mut random);`
- L34: `random` — writable borrow — fix: `let mut random`
  - `MazeBuilder::carve_room(maze_builder, level, &mut random, CellId::R00, 9, 0);`
- L37: `random` — writable borrow — fix: `let mut random`
  - `_ -> build_main_hall_1(maze_builder, level, &mut random)`
- L250: `enemy` — field store — fix: `let mut enemy`
  - `enemy.kind = EnemyKind::Bat;`
- L251: `enemy` — writable borrow — fix: `let mut enemy`
  - `EnemyName::set(&mut enemy.name, "");`
- L252: `enemy` — field store — fix: `let mut enemy`
  - `enemy.max_health = 1;`
- L253: `enemy` — field store — fix: `let mut enemy`
  - `enemy.health = 1;`
- L254: `enemy` — field store — fix: `let mut enemy`
  - `enemy.attack = 1;`
- L255: `enemy` — field store — fix: `let mut enemy`
  - `enemy.defense = 0;`
- L256: `enemy` — field store — fix: `let mut enemy`
  - `enemy.xp_reward = 0;`
- L257: `enemy` — field store — fix: `let mut enemy`
  - `enemy.depth = 0;`
### `samples/cli/games/dungeon_crawler_cli/rooms/room_formatter.omg`

- L41: `room` — writable borrow — fix: `let mut room`
  - `RoomLookup::find_room(level, cell, &mut room);`
- L53: `room` — writable borrow — fix: `let mut room`
  - `RoomLookup::find_room(level, cell, &mut room);`
### `samples/cli/probes/bool_representation_views/main.omg`

- L41: `byte_alias` — assignment **review** — fix: `let mut byte_alias`
  - `byte_alias = false;`
### `samples/cli/probes/bounded_clip_fraction/main.omg`

- L46: `gap` — field store — fix: `let mut gap`
  - `gap.difference = start_distance - end_distance;`
### `samples/cli/probes/bucket_contract_chain/main.omg`

- L55: `slots` — index store — fix: `let mut slots`
  - `slots[index]`
### `samples/cli/probes/build_facets/build.omg`

- L10: `bytes` — writable borrow — fix: `let mut bytes`
  - `let n: i64 = BuildSource::read(&builder.source, d, &mut bytes, 8);`
### `samples/cli/probes/byte_window_dispatch/main.omg`

- L31: `bytes` — index store — fix: `let mut bytes`
  - `bytes[0]`
### `samples/cli/probes/declaration_predicate_calls/main.omg`

- L54: `value` — writable borrow — fix: `let mut value`
  - `let changed: u64 = initialize_counter(&mut value);`
- L62: `text` — writable borrow — fix: `let mut text`
  - `transition { _ -> observe(&mut text, 0) }`
- L71: `text` — field store — fix: `let mut text`
  - `text.bytes = [103, 255, 33];`
- L99: `mixed` — field store — fix: `let mut mixed`
  - `mixed.tag == 1 && ready.tag == 1 && generic_byte.payload == 19u8 && generic_wide.payload == 21u64 &&`
### `samples/cli/probes/interleaved_match_locals/main.omg`

- L28: `marker` — field store — fix: `let mut marker`
  - `marker.tag == tag && observed`
### `samples/cli/probes/let_mut_loan_out_param/main.omg`

- L31: `found` — writable borrow — fix: `let mut found`
  - `find(2, &mut found);`
### `samples/cli/probes/lifetime_source_select/main.omg`

- L37: `selected` — field store — fix: `let mut selected`
  - `selected.body = 7;`
### `samples/cli/probes/projected_borrow_forwarding/main.omg`

- L16: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 29;`
- L28: `selected` — assignment **review** — fix: `let mut selected`
  - `selected = 29;`
### `samples/cli/probes/uninit_record_fill/main.omg`

- L31: `point` — field store — fix: `let mut point`
  - `point.x = 7;`
- L32: `point` — field store — fix: `let mut point`
  - `point.y = 12;`
### `samples/cli/probes/wire_canonical_varint/main.omg`

- L45: `overlong` — writable borrow — fix: `let mut overlong`
  - `&mut overlong,`
- L54: `overflow` — writable borrow — fix: `let mut overflow`
  - `&mut overflow,`
- L63: `good` — writable borrow — fix: `let mut good`
  - `&mut good,`
### `samples/cli/probes/wire_scalar_roundtrip/main.omg`

- L47: `decoded` — writable borrow — fix: `let mut decoded`
  - `Frame::decode(&mut decoded, &main.buffer, &mut main.read, &mut main.verdict);`
### `samples/cli/probes/wire_slice_decode/main.omg`

- L49: `decoded` — writable borrow — fix: `let mut decoded`
  - `Blob::decode(&mut decoded, &main.buffer, &mut main.read, &mut main.verdict);`
### `samples/cli/probes/wire_zigzag_extremes/main.omg`

- L54: `decoded` — writable borrow — fix: `let mut decoded`
  - `Extremes::decode(&mut decoded, &main.buffer, &mut main.read, &mut main.verdict);`
### `samples/cli/probes/wrong_era_decode/main.omg`

- L26: `decoded` — writable borrow — fix: `let mut decoded`
  - `CounterMessage::decode(&mut decoded, &buffer, &mut main.read, &mut main.verdict);`
### `samples/cli/scientific/dose_response_fit/controls/intended_invalid/invalid.omg`

- L5: `observations` — index store — fix: `let mut observations`
  - `observations[32]`
### `samples/cli/scientific/exact_contingency_inference/controls/inspection/main.omg`

- L12: `counter` — field store — fix: `let mut counter`
  - `counter.value == 7 && counter.untouched == 11`
### `samples/cli/scientific/exact_contingency_inference/controls/runtime_oracle.omg`

- L9: `actual` — field store — fix: `let mut actual`
  - `actual.status == expected.status && actual.count == expected.count &&`
- L10: `actual` — field store — fix: `let mut actual`
  - `actual.partition == expected.partition && actual.observed_weight == expected.observed_weight &&`
- L11: `actual` — field store — fix: `let mut actual`
  - `actual.score == expected.score && actual.denominator == expected.denominator &&`
- L12: `actual` — field store — fix: `let mut actual`
  - `actual.probability_tail == expected.probability_tail && actual.association_tail == expected.association_tail &&`
- L13: `actual` — field store — fix: `let mut actual`
  - `actual.ties == expected.ties`
### `samples/cli/systems/wire_protocol/main.omg`

- L82: `decoded` — writable borrow — fix: `let mut decoded`
  - `SensorPacketCodec::decode(&mut decoded, &main.buffer, &mut main.read, &mut main.verdict);`
### `samples/gui/image_viewer/main.omg`

- L418: `window` — writable borrow — fix: `let mut window`
  - `main.have = Gui::msg_peek(&mut window, main.message);`
### `samples/gui/window_app/main.omg`

- L275: `window` — writable borrow — fix: `let mut window`
  - `main.have = Gui::msg_peek(&mut window, main.message);`
### `samples/gui/window_demo/main.omg`

- L187: `window` — writable borrow — fix: `let mut window`
  - `main.have = Gui::pump_message(&mut window, &mut main.message, &mut main.close_requested);`
### `samples/gui/windowed_calculator/main.omg`

- L336: `window` — writable borrow — fix: `let mut window`
  - `main.have = Gui::msg_peek(&mut window, main.message);`
- L961: `window` — writable borrow — fix: `let mut window`
  - `main.have = Gui::msg_peek(&mut window, main.message);`

## `source/library` (1 hits)

### `source/library/core/sha256.omg`

- L84: `table` — index store — fix: `let mut table`
  - `table[i]`

## `source/psi` (12 hits)

### `source/psi/checking/arrivals.omg`

- L243: `none` — field store — fix: `let mut none`
  - `none.depth = 0;`
### `source/psi/checking/frames.omg`

- L467: `row` — field store — fix: `let mut row`
  - `row.kind == TypeKind::Reference && row.access != ReferenceAccess::Shared && adaptation != Adaptation::Read`
- L477: `row` — field store — fix: `let mut row`
  - `row.kind == TypeKind::Reference && row.access == ReferenceAccess::Shared && adaptation != Adaptation::Read`
### `source/psi/typing/expressions.omg`

- L748: `row` — field store — fix: `let mut row`
  - `row.kind == TypeKind::Builtin`
- L1370: `row` — field store — fix: `let mut row`
  - `row.kind == TypeKind::Builtin`
### `source/psi/typing/landing.omg`

- L25: `row` — field store — fix: `let mut row`
  - `row.kind == TypeKind::AnonymousInteger || row.kind == TypeKind::AnonymousFloat`
- L34: `row` — field store — fix: `let mut row`
  - `row.kind == TypeKind::AnonymousChoice || row.kind == TypeKind::AnonymousArray`
### `source/psi/typing/operators.omg`

- L226: `row` — field store — fix: `let mut row`
  - `row.kind == TypeKind::Data`
### `source/psi/typing/references.omg`

- L424: `generic` — field store — fix: `let mut generic`
  - `generic.kind == GenericParameterKind::Const`
- L595: `declared` — field store — fix: `let mut declared`
  - `declared.kind == GenericParameterKind::Value || declared.kind == GenericParameterKind::Const`
- L600: `declared` — field store — fix: `let mut declared`
  - `declared.kind == expected`
### `source/psi/typing/statements.omg`

- L643: `row` — field store — fix: `let mut row`
  - `row.kind == TypeKind::Builtin && row.builtin == BuiltinType::Never`

## Measurement

- Host: `cursor`
- Commit: `0835b3aaf803ed97f905be42d94727ff58459263`
- Command:
```bash
python3 /opt/cursor/artifacts/mut_migration_scan.py
```
- Roots scanned: `tests/omega`, `samples`, `source/library`, `source/psi`
- Scanner: `/opt/cursor/artifacts/mut_migration_scan.py` (not landed)
