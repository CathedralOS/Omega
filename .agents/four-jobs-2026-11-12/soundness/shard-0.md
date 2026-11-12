# Soundness mutations shard 0

PIN: `0835b3aaf803ed97f905be42d94727ff58459263`

## Commands

```bash
git checkout 0835b3aaf803ed97f905be42d94727ff58459263
export CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target
export OMEGA_LIBRARY_ROOT=$PWD/source/library OMEGA_CORPUS_ROOT=$PWD
timeout 120 nice -n 19 target/debug/examples/route_one_comparison <mutant>/main.omg < /dev/null
mbx build -p omega -p compiler --example route_one_comparison
python3 .agents/four-jobs-2026-11-12/soundness/run_shard_mutations.py --shard 0 --resume
```

## Counts

- Fixtures visited this run: 200
- Fixtures completed (progress file): 196
- Originals skipped (not accepted): 9
- Mutants generated (this run): 294
- Mutants run (cumulative): 301
- Kept holes (confirmed-invalid): 169
- Disagreements among kept: 169
- Discarded valid: 132
- Discarded no rule quote: 0

## Confirmed holes and disagreements

### tests/omega/pass/arithmetic/addr_algebra/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/arithmetic/addr_algebra/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_arithmetic_addr_algebra_ab76c8acf2/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/arithmetic/indexed_divisor_requirement/main.omg — widen_requires

- Original: `tests/omega/pass/arithmetic/indexed_divisor_requirement/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_arithmetic_indexed_divi_965d75bbf1/widen_requires/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/state_contracts.md):

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### tests/omega/pass/arithmetic/indexed_divisor_requirement/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/arithmetic/indexed_divisor_requirement/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_arithmetic_indexed_divi_965d75bbf1/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/arithmetic/nested_clamp_resolution_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/arithmetic/nested_clamp_resolution_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_arithmetic_nested_clamp_e4cf3a7699/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/arithmetic/runtime_unit_terminal_guarded_cast_exit/main.omg — remove_mut

- Original: `tests/omega/pass/arithmetic/runtime_unit_terminal_guarded_cast_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_arithmetic_runtime_unit_dc1f1a502f/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/arithmetic/runtime_unit_terminal_guarded_cast_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/arithmetic/runtime_unit_terminal_guarded_cast_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_arithmetic_runtime_unit_dc1f1a502f/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/arithmetic/total_contract_guarded_division/main.omg — widen_requires

- Original: `tests/omega/pass/arithmetic/total_contract_guarded_division/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_arithmetic_total_contra_891b1b28ba/widen_requires/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/state_contracts.md):

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### tests/omega/pass/arithmetic/total_contract_guarded_division/main.omg — drop_divisor_guard

- Original: `tests/omega/pass/arithmetic/total_contract_guarded_division/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_arithmetic_total_contra_891b1b28ba/drop_divisor_guard/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/expressions.md):

> proof error, not a runtime panic. Thus `..=len-1` needs nonzero length, and an

### tests/omega/pass/arrays/guard_compound_index/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/arrays/guard_compound_index/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_arrays_guard_compound_i_9426c1d30d/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/arrays/runtime_index_recast/main.omg — remove_mut

- Original: `tests/omega/pass/arrays/runtime_index_recast/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_arrays_runtime_index_re_895472a97d/remove_mut/main.omg`
- Current checker: `accepted`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/arrays/runtime_index_recast/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/arrays/runtime_index_recast/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_arrays_runtime_index_re_895472a97d/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/asm/atomic_release_disposition_replaced_contents_exit/main.omg — remove_mut

- Original: `tests/omega/pass/asm/atomic_release_disposition_replaced_contents_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_asm_atomic_release_disp_b91482c80d/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/asm/atomic_release_disposition_replaced_contents_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/asm/atomic_release_disposition_replaced_contents_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_asm_atomic_release_disp_b91482c80d/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/asm/checked_label_edges_exit/main.omg — remove_mut

- Original: `tests/omega/pass/asm/checked_label_edges_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_asm_checked_label_edges_35e4663f16/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/asm/checked_label_edges_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/asm/checked_label_edges_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_asm_checked_label_edges_35e4663f16/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/asm/port_write_effect_contract/main.omg — remove_mut

- Original: `tests/omega/pass/asm/port_write_effect_contract/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_asm_port_write_effect_c_972b932205/remove_mut/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/asm/port_write_effect_contract/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/asm/port_write_effect_contract/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_asm_port_write_effect_c_972b932205/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/asm/scratch_target_sibling_capture_identity_exit/main.omg — remove_mut

- Original: `tests/omega/pass/asm/scratch_target_sibling_capture_identity_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_asm_scratch_target_sibl_c9f58c9dee/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/atomics/runtime_atomic_fences_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/atomics/runtime_atomic_fences_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_atomics_runtime_atomic__6a5556e723/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/atomics/runtime_projected_state_operands_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/atomics/runtime_projected_state_operands_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_atomics_runtime_project_3e47f4fed8/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrow/copyable_array_view_survives_owner_pass_compile/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrow/copyable_array_view_survives_owner_pass_compile/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrow_copyable_array_v_f252760088/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrow/runtime_write_only_aggregate_selector_snapshot_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrow/runtime_write_only_aggregate_selector_snapshot_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrow_runtime_write_on_97b8432341/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrow/runtime_write_only_aggregate_selector_snapshot_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrow/runtime_write_only_aggregate_selector_snapshot_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrow_runtime_write_on_97b8432341/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrow/runtime_write_only_exclusive_reference_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrow/runtime_write_only_exclusive_reference_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrow_runtime_write_on_ffa91e3d99/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrow/runtime_write_only_exclusive_reference_call_result_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrow/runtime_write_only_exclusive_reference_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrow_runtime_write_on_ffa91e3d99/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrow/runtime_write_only_local_affine_aggregate_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrow/runtime_write_only_local_affine_aggregate_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrow_runtime_write_on_9232230b4b/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrow/runtime_write_only_local_affine_aggregate_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrow/runtime_write_only_local_affine_aggregate_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrow_runtime_write_on_9232230b4b/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrow/storage_address_window_identity_exit/main.omg — widen_requires

- Original: `tests/omega/pass/borrow/storage_address_window_identity_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrow_storage_address__1a63b37784/widen_requires/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/state_contracts.md):

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### tests/omega/pass/borrow/storage_address_window_identity_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrow/storage_address_window_identity_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrow_storage_address__1a63b37784/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/borrowed_record_reference_field_state_writeback_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/borrowed_record_reference_field_state_writeback_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_borrowed_record_99db62a2e7/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/borrowed_record_reference_field_state_writeback_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/borrowed_record_reference_field_state_writeback_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_borrowed_record_99db62a2e7/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/call_replaced_local_backing_releases_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/call_replaced_local_backing_releases_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_call_replaced_l_b5c60db84c/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/call_replaced_local_backing_releases_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/call_replaced_local_backing_releases_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_call_replaced_l_b5c60db84c/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/collection_element_loans_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/collection_element_loans_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_collection_elem_c0417b82f0/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/collection_element_loans_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/collection_element_loans_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_collection_elem_c0417b82f0/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/constructor_reference_access/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/constructor_reference_access/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_constructor_ref_ebc4e2750f/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/copied_input_descriptor_return_copy_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/copied_input_descriptor_return_copy_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_copied_input_de_91ca8bd3f3/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/copied_input_descriptor_return_copy_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/copied_input_descriptor_return_copy_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_copied_input_de_91ca8bd3f3/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/erased_input_before_returned_reference_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/erased_input_before_returned_reference_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_erased_input_be_1bf3efc7c7/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/erased_input_before_returned_reference_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/erased_input_before_returned_reference_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_erased_input_be_1bf3efc7c7/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/foreign_output_nested_shared_input_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/foreign_output_nested_shared_input_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_foreign_output__6a4211a4b3/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/foreign_output_nested_shared_input_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/foreign_output_nested_shared_input_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_foreign_output__6a4211a4b3/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/nested_reference_join_state_backing_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/nested_reference_join_state_backing_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_nested_referenc_6ac3210cce/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/nested_reference_join_state_backing_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/nested_reference_join_state_backing_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_nested_referenc_6ac3210cce/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/opaque_incoming_nested_stored_byte_view_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/opaque_incoming_nested_stored_byte_view_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_opaque_incoming_f057633817/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/opaque_incoming_nested_stored_byte_view_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/opaque_incoming_nested_stored_byte_view_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_opaque_incoming_f057633817/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/private_arrival_default_domain_bound/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/private_arrival_default_domain_bound/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_private_arrival_adb93fb3b4/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/recast_suspension_branch_cleanup_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/recast_suspension_branch_cleanup_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_recast_suspensi_3642ba4231/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/returned_case_descriptor_formation/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/returned_case_descriptor_formation/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_returned_case_d_c28bfa43ac/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/runtime_aliased_record_byte_field_window_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/runtime_aliased_record_byte_field_window_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_aliased_acb842d2c6/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_aliased_record_byte_field_window_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/runtime_aliased_record_byte_field_window_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_aliased_acb842d2c6/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/runtime_byte_view_element_reference_calls_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/runtime_byte_view_element_reference_calls_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_byte_vi_f87f448851/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_byte_view_element_reference_calls_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/runtime_byte_view_element_reference_calls_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_byte_vi_f87f448851/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/runtime_case_descriptor_retirement_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/runtime_case_descriptor_retirement_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_case_de_7113438058/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_case_descriptor_retirement_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/runtime_case_descriptor_retirement_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_case_de_7113438058/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/runtime_computed_shared_byte_field_window_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/runtime_computed_shared_byte_field_window_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_compute_1a51da560d/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_computed_shared_byte_field_window_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/runtime_computed_shared_byte_field_window_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_compute_1a51da560d/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/runtime_copied_range_record_domains_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/runtime_copied_range_record_domains_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_copied__2c00cd1182/remove_mut/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_copied_range_record_domains_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/runtime_copied_range_record_domains_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_copied__2c00cd1182/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/runtime_dependent_window_image_sibling_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/runtime_dependent_window_image_sibling_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_depende_2e9acbd49b/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_dependent_window_image_sibling_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/runtime_dependent_window_image_sibling_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_depende_2e9acbd49b/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/runtime_dyn_multi_input_window_snapshot_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/runtime_dyn_multi_input_window_snapshot_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_dyn_mul_5749c73d82/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_dyn_multi_input_window_snapshot_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/runtime_dyn_multi_input_window_snapshot_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_dyn_mul_5749c73d82/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/runtime_indexed_scalar_descriptor_call/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/runtime_indexed_scalar_descriptor_call/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_indexed_aa711e4395/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_indexed_scalar_descriptor_call/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/runtime_indexed_scalar_descriptor_call/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_indexed_aa711e4395/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/runtime_mixed_byte_view_reference_result_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/runtime_mixed_byte_view_reference_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_mixed_b_e911c78306/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/runtime_nested_reference_partition_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/runtime_nested_reference_partition_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_nested__aa8380265e/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_nested_reference_partition_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/runtime_nested_reference_partition_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_nested__aa8380265e/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/runtime_owned_subject_descriptor_replacement_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/runtime_owned_subject_descriptor_replacement_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_owned_s_fa1c685752/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_owned_subject_descriptor_replacement_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/runtime_owned_subject_descriptor_replacement_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_owned_s_fa1c685752/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/runtime_projected_reference_partition_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/runtime_projected_reference_partition_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_project_f740484ddc/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_projected_reference_partition_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/runtime_projected_reference_partition_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_project_f740484ddc/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/runtime_reassigned_view_return_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/runtime_reassigned_view_return_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_reassig_f07d443358/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_reassigned_view_return_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/runtime_reassigned_view_return_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_reassig_f07d443358/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/runtime_scalar_field_local_loan_handback_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/runtime_scalar_field_local_loan_handback_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_scalar__8c2dfc7a56/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_scalar_field_local_loan_handback_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/runtime_scalar_field_local_loan_handback_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_scalar__8c2dfc7a56/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/runtime_selected_reference_nested_restoration_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/runtime_selected_reference_nested_restoration_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_selecte_7bddbfcf86/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_selected_reference_nested_restoration_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/runtime_selected_reference_nested_restoration_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_selecte_7bddbfcf86/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/runtime_shared_view_record_presentation_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/runtime_shared_view_record_presentation_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_shared__6809b457d2/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/runtime_tail_recursive_nested_field_frame_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/runtime_tail_recursive_nested_field_frame_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_tail_re_8f07a6450e/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_tail_recursive_nested_field_frame_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/runtime_tail_recursive_nested_field_frame_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_tail_re_8f07a6450e/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/runtime_writing_static_callback_snapshot_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/runtime_writing_static_callback_snapshot_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_writing_2b535c4ae9/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/runtime_writing_static_callback_snapshot_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/runtime_writing_static_callback_snapshot_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_runtime_writing_2b535c4ae9/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/supplied_output_ordinary_helper_saved_reference_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/supplied_output_ordinary_helper_saved_reference_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_supplied_output_3ee210c685/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/supplied_output_ordinary_helper_saved_reference_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/supplied_output_ordinary_helper_saved_reference_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_supplied_output_3ee210c685/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/suspension_projected_call_view_returned_alternatives_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/suspension_projected_call_view_returned_alternatives_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_suspension_proj_1eef1843e3/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/suspension_saved_reference_view_inherited_backing_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/suspension_saved_reference_view_inherited_backing_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_suspension_save_ef4a6a9b52/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/borrows/template_prefix_forms_owned_borrow_exit/main.omg — remove_mut

- Original: `tests/omega/pass/borrows/template_prefix_forms_owned_borrow_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_template_prefix_993018898c/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/borrows/template_prefix_forms_owned_borrow_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/borrows/template_prefix_forms_owned_borrow_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_borrows_template_prefix_993018898c/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/boundary/interleaved_structural_crash_formal_exit/main.omg — remove_mut

- Original: `tests/omega/pass/boundary/interleaved_structural_crash_formal_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_boundary_interleaved_st_12d0438fc3/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/boundary/interleaved_structural_crash_formal_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/boundary/interleaved_structural_crash_formal_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_boundary_interleaved_st_12d0438fc3/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/build/artifact_only_output/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/build/artifact_only_output/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_build_artifact_only_out_2e42432d8f/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/build/explicit_lifetime_entry_context_exit/main.omg — remove_mut

- Original: `tests/omega/pass/build/explicit_lifetime_entry_context_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_build_explicit_lifetime_adc511b61f/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/build/explicit_lifetime_entry_context_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/build/explicit_lifetime_entry_context_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_build_explicit_lifetime_adc511b61f/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/build/static_assumed_binding_spelling_exit/main.omg — remove_mut

- Original: `tests/omega/pass/build/static_assumed_binding_spelling_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_build_static_assumed_bi_bd7ffe305a/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/build/static_assumed_binding_spelling_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/build/static_assumed_binding_spelling_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_build_static_assumed_bi_bd7ffe305a/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/build/terminal_result_custody/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/build/terminal_result_custody/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_build_terminal_result_c_dc653cad55/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/calls/native_code_layout_nested_type_const_exit/main.omg — widen_requires

- Original: `tests/omega/pass/calls/native_code_layout_nested_type_const_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_native_code_layou_876f0b3c29/widen_requires/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/state_contracts.md):

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### tests/omega/pass/calls/native_code_layout_nested_type_const_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/calls/native_code_layout_nested_type_const_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_native_code_layou_876f0b3c29/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/calls/nested_subject_call_operand/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/calls/nested_subject_call_operand/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_nested_subject_ca_faf4f37a6a/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/calls/outcome_specific_primary_expression/main.omg — remove_mut

- Original: `tests/omega/pass/calls/outcome_specific_primary_expression/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_outcome_specific__8b4e92e192/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/calls/prefix_effectful_call/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/calls/prefix_effectful_call/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_prefix_effectful__cc3170806e/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/calls/reference_loan_field_requires/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/calls/reference_loan_field_requires/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_reference_loan_fi_f361f90865/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/calls/returned_nested_record_disjoint_field_move_exit/main.omg — remove_mut

- Original: `tests/omega/pass/calls/returned_nested_record_disjoint_field_move_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_returned_nested_r_20d2fc1902/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/calls/returned_nested_record_disjoint_field_move_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/calls/returned_nested_record_disjoint_field_move_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_returned_nested_r_20d2fc1902/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/calls/runtime_authored_numeric_guard_result_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/calls/runtime_authored_numeric_guard_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_runtime_authored__6990b67d3b/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/calls/runtime_constructed_copy_payload_arguments_exit/main.omg — remove_mut

- Original: `tests/omega/pass/calls/runtime_constructed_copy_payload_arguments_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_runtime_construct_cd2a640e3b/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/calls/runtime_constructed_copy_payload_arguments_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/calls/runtime_constructed_copy_payload_arguments_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_runtime_construct_cd2a640e3b/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/calls/runtime_constructor_child_pure_calls_exit/main.omg — remove_mut

- Original: `tests/omega/pass/calls/runtime_constructor_child_pure_calls_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_runtime_construct_0116bfd000/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/calls/runtime_constructor_child_pure_calls_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/calls/runtime_constructor_child_pure_calls_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_runtime_construct_0116bfd000/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/calls/runtime_fused_child_computed_borrow_exit/main.omg — remove_mut

- Original: `tests/omega/pass/calls/runtime_fused_child_computed_borrow_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_runtime_fused_chi_648f5881e2/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/calls/runtime_fused_child_computed_borrow_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/calls/runtime_fused_child_computed_borrow_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_runtime_fused_chi_648f5881e2/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/calls/runtime_guarded_terminal_guard_order_exit/main.omg — remove_mut

- Original: `tests/omega/pass/calls/runtime_guarded_terminal_guard_order_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_runtime_guarded_t_3ce4e9aa53/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/calls/runtime_guarded_terminal_guard_order_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/calls/runtime_guarded_terminal_guard_order_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_runtime_guarded_t_3ce4e9aa53/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/calls/runtime_indexed_case_pool_transition_exit/main.omg — remove_mut

- Original: `tests/omega/pass/calls/runtime_indexed_case_pool_transition_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_runtime_indexed_c_07c28cc901/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/calls/runtime_indexed_case_pool_transition_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/calls/runtime_indexed_case_pool_transition_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_runtime_indexed_c_07c28cc901/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/calls/runtime_selected_reference_field_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/calls/runtime_selected_reference_field_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_runtime_selected__7214fdfaca/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/calls/runtime_selected_reference_field_result_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/calls/runtime_selected_reference_field_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_runtime_selected__7214fdfaca/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/calls/runtime_static_tail_reordered_local_custody_exit/main.omg — remove_mut

- Original: `tests/omega/pass/calls/runtime_static_tail_reordered_local_custody_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_runtime_static_ta_8c8cea1f5d/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/calls/runtime_static_tail_reordered_local_custody_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/calls/runtime_static_tail_reordered_local_custody_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_runtime_static_ta_8c8cea1f5d/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/calls/static_boundary_helpers_and_cleanup_exit/main.omg — remove_mut

- Original: `tests/omega/pass/calls/static_boundary_helpers_and_cleanup_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_static_boundary_h_da452228fa/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/calls/static_boundary_helpers_and_cleanup_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/calls/static_boundary_helpers_and_cleanup_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_calls_static_boundary_h_da452228fa/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/capabilities/sysv_small_aggregate_import_compile/main.omg — remove_mut

- Original: `tests/omega/pass/capabilities/sysv_small_aggregate_import_compile/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_capabilities_sysv_small_991977704a/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/capabilities/sysv_small_aggregate_import_compile/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/capabilities/sysv_small_aggregate_import_compile/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_capabilities_sysv_small_991977704a/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/cases/arm_returns_mut_ref/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/cases/arm_returns_mut_ref/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_cases_arm_returns_mut_r_c55faf76ab/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/cases/dup_match_order/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/cases/dup_match_order/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_cases_dup_match_order_m_cb81e467da/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/cases/mixed_record_cases/main.omg — remove_mut

- Original: `tests/omega/pass/cases/mixed_record_cases/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_cases_mixed_record_case_0c16f17222/remove_mut/main.omg`
- Current checker: `accepted`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/cases/mixed_record_cases/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/cases/mixed_record_cases/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_cases_mixed_record_case_0c16f17222/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/cases/runtime_nested_payload_scalar_projection_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/cases/runtime_nested_payload_scalar_projection_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_cases_runtime_nested_pa_db4af3f596/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/collections/array_local_control/main.omg — remove_mut

- Original: `tests/omega/pass/collections/array_local_control/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_collections_array_local_17310fb910/remove_mut/main.omg`
- Current checker: `accepted`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/collections/array_local_control/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/collections/array_local_control/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_collections_array_local_17310fb910/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/collections/contract_length_qualified_result_cast_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/collections/contract_length_qualified_result_cast_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_collections_contract_le_a721508e80/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/collections/runtime_array_child_membership_transport_exit/main.omg — remove_mut

- Original: `tests/omega/pass/collections/runtime_array_child_membership_transport_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_collections_runtime_arr_022fc549f9/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/collections/runtime_array_child_membership_transport_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/collections/runtime_array_child_membership_transport_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_collections_runtime_arr_022fc549f9/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/collections/runtime_composed_element_view_reads_exit/main.omg — remove_mut

- Original: `tests/omega/pass/collections/runtime_composed_element_view_reads_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_collections_runtime_com_a6b082eb86/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/collections/runtime_composed_element_view_reads_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/collections/runtime_composed_element_view_reads_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_collections_runtime_com_a6b082eb86/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/collections/runtime_computed_scalar_selector_argument_exit/main.omg — remove_mut

- Original: `tests/omega/pass/collections/runtime_computed_scalar_selector_argument_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_collections_runtime_com_aa21562af2/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/collections/runtime_computed_scalar_selector_argument_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/collections/runtime_computed_scalar_selector_argument_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_collections_runtime_com_aa21562af2/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/collections/runtime_dynamic_borrow_selector_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/collections/runtime_dynamic_borrow_selector_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_collections_runtime_dyn_4c98030ea3/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/collections/runtime_dynamic_borrow_selector_join_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/collections/runtime_dynamic_borrow_selector_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_collections_runtime_dyn_4c98030ea3/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/collections/runtime_generic_record_slice_reads_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/collections/runtime_generic_record_slice_reads_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_collections_runtime_gen_29870ec186/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/collections/runtime_multi_field_element_view_domains_exit/main.omg — remove_mut

- Original: `tests/omega/pass/collections/runtime_multi_field_element_view_domains_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_collections_runtime_mul_e871a920ca/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/collections/runtime_multi_field_element_view_domains_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/collections/runtime_multi_field_element_view_domains_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_collections_runtime_mul_e871a920ca/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/collections/runtime_ordered_computed_selector_reads_exit/main.omg — remove_mut

- Original: `tests/omega/pass/collections/runtime_ordered_computed_selector_reads_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_collections_runtime_ord_bf2e9012d1/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/collections/runtime_ordered_computed_selector_reads_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/collections/runtime_ordered_computed_selector_reads_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_collections_runtime_ord_bf2e9012d1/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/collections/runtime_projected_range_call_selector_order_exit/main.omg — remove_mut

- Original: `tests/omega/pass/collections/runtime_projected_range_call_selector_order_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_collections_runtime_pro_7eb503c673/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/collections/runtime_projected_range_call_selector_order_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/collections/runtime_projected_range_call_selector_order_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_collections_runtime_pro_7eb503c673/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/comptime/const_joint_ranked_call_component/main.omg — remove_mut

- Original: `tests/omega/pass/comptime/const_joint_ranked_call_component/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_comptime_const_joint_ra_fb7f48ef6e/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/comptime/runtime_generic_constructor_field_demand_extent_exit/main.omg — remove_mut

- Original: `tests/omega/pass/comptime/runtime_generic_constructor_field_demand_extent_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_comptime_runtime_generi_9e9492e92e/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/comptime/runtime_generic_constructor_field_demand_extent_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/comptime/runtime_generic_constructor_field_demand_extent_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_comptime_runtime_generi_9e9492e92e/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/concurrency/runtime_array_loan_suspension_exit/main.omg — remove_mut

- Original: `tests/omega/pass/concurrency/runtime_array_loan_suspension_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_concurrency_runtime_arr_af71a849a8/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/concurrency/runtime_array_loan_suspension_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/concurrency/runtime_array_loan_suspension_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_concurrency_runtime_arr_af71a849a8/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/concurrency/suspend_message_carrier/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/concurrency/suspend_message_carrier/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_concurrency_suspend_mes_0f01881ab7/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/constants/runtime_predicate_constant_materialization_exit/main.omg — remove_mut

- Original: `tests/omega/pass/constants/runtime_predicate_constant_materialization_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_constants_runtime_predi_f51969e043/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/constants/runtime_predicate_constant_materialization_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/constants/runtime_predicate_constant_materialization_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_constants_runtime_predi_f51969e043/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/contracts/borrowed_field_ensures/main.omg — remove_mut

- Original: `tests/omega/pass/contracts/borrowed_field_ensures/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_contracts_borrowed_fiel_b60c543e78/remove_mut/main.omg`
- Current checker: `accepted`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/contracts/borrowed_field_ensures/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/contracts/borrowed_field_ensures/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_contracts_borrowed_fiel_b60c543e78/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/contracts/ensures_operand_discharge/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/contracts/ensures_operand_discharge/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_contracts_ensures_opera_c31266a794/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/contracts/nested_call_later_operand_preserves_storage_fact/main.omg — remove_mut

- Original: `tests/omega/pass/contracts/nested_call_later_operand_preserves_storage_fact/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_contracts_nested_call_l_1132177519/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/contracts/nested_call_later_operand_preserves_storage_fact/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/contracts/nested_call_later_operand_preserves_storage_fact/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_contracts_nested_call_l_1132177519/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/contracts/proof_fact_indexed_domain_application/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/contracts/proof_fact_indexed_domain_application/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_contracts_proof_fact_in_b0871d8ee1/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/contracts/returned_data_identity_with_slice/main.omg — remove_mut

- Original: `tests/omega/pass/contracts/returned_data_identity_with_slice/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_contracts_returned_data_393b98e9e9/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/contracts/returned_data_identity_with_slice/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/contracts/returned_data_identity_with_slice/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_contracts_returned_data_393b98e9e9/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/contracts/runtime_float_fused_crash_guard_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/contracts/runtime_float_fused_crash_guard_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_contracts_runtime_float_d6d5a2e45b/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/contracts/saved_call_case_guarantee_requires_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/contracts/saved_call_case_guarantee_requires_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_contracts_saved_call_ca_3560058abb/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/contracts/terminal_contract/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/contracts/terminal_contract/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_contracts_terminal_cont_03bcb70892/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/control_flow/case_guard_wildcard_fallthrough_compile/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/control_flow/case_guard_wildcard_fallthrough_compile/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_control_flow_case_guard_23d3c9ce1d/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/control_flow/dispatch_loop_nonzero_unsigned_exit/main.omg — remove_mut

- Original: `tests/omega/pass/control_flow/dispatch_loop_nonzero_unsigned_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_control_flow_dispatch_l_6ba06ed8c4/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/control_flow/dispatch_loop_nonzero_unsigned_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/control_flow/dispatch_loop_nonzero_unsigned_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_control_flow_dispatch_l_6ba06ed8c4/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |
