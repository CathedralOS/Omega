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

- Fixtures visited this run: 775
- Fixtures completed (progress file): 755
- Originals skipped (not accepted): 25
- Mutants generated (this run): 832
- Mutants run (cumulative): 839
- Kept holes (confirmed-invalid): 627
- Disagreements among kept: 627
- Discarded valid: 212
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

### tests/omega/pass/control_flow/invariant_member_subject_compile/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/control_flow/invariant_member_subject_compile/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_control_flow_invariant__008394ae76/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/control_flow/narrow_negative_dispatch_compile/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/control_flow/narrow_negative_dispatch_compile/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_control_flow_narrow_neg_1bf5d23c20/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/control_flow/renamed_payload_guard_dispatch_compile/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/control_flow/renamed_payload_guard_dispatch_compile/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_control_flow_renamed_pa_d28e480f05/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/control_flow/runtime_branching_helper_string/main.omg — remove_mut

- Original: `tests/omega/pass/control_flow/runtime_branching_helper_string/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_control_flow_runtime_br_367dc6d22c/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/control_flow/runtime_branching_helper_string/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/control_flow/runtime_branching_helper_string/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_control_flow_runtime_br_367dc6d22c/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/control_flow/runtime_dispatch_boundary_total_exit/main.omg — remove_mut

- Original: `tests/omega/pass/control_flow/runtime_dispatch_boundary_total_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_control_flow_runtime_di_4ae6a0c7a8/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/control_flow/runtime_dispatch_boundary_total_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/control_flow/runtime_dispatch_boundary_total_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_control_flow_runtime_di_4ae6a0c7a8/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/control_flow/runtime_guarded_edge_preparation_exit/main.omg — remove_mut

- Original: `tests/omega/pass/control_flow/runtime_guarded_edge_preparation_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_control_flow_runtime_gu_b533a5091a/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/control_flow/runtime_guarded_edge_preparation_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/control_flow/runtime_guarded_edge_preparation_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_control_flow_runtime_gu_b533a5091a/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/control_flow/three_param_state_forward_compile/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/control_flow/three_param_state_forward_compile/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_control_flow_three_para_e1eae48cd0/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/core/content_projection_signed_integer_cancellation/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/core/content_projection_signed_integer_cancellation/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_core_content_projection_fcc3f5aef7/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/core/owned_content_partition/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/core/owned_content_partition/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_core_owned_content_part_c5934b0462/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/data/payload_less_case_equality/main.omg — remove_mut

- Original: `tests/omega/pass/data/payload_less_case_equality/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_data_payload_less_case__dc80de9621/remove_mut/main.omg`
- Current checker: `accepted`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/data/payload_less_case_equality/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/data/payload_less_case_equality/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_data_payload_less_case__dc80de9621/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/dependent/case_where_common_field_window/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/dependent/case_where_common_field_window/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_dependent_case_where_co_e7ef881ddc/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/dependent/collection_predicate_helper_theorem_bounds_sum/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/dependent/collection_predicate_helper_theorem_bounds_sum/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_dependent_collection_pr_a2949e10c8/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/dependent/data_where_arithmetic_bound_compile/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/dependent/data_where_arithmetic_bound_compile/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_dependent_data_where_ar_2068ad5c45/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/dependent/data_where_disequality_compile/main.omg — widen_requires

- Original: `tests/omega/pass/dependent/data_where_disequality_compile/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_dependent_data_where_di_42d0db8130/widen_requires/main.omg`
- Current checker: `accepted`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/state_contracts.md):

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### tests/omega/pass/dependent/data_where_disequality_compile/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/dependent/data_where_disequality_compile/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_dependent_data_where_di_42d0db8130/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/dependent/data_where_single_equality_compile/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/dependent/data_where_single_equality_compile/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_dependent_data_where_si_cec242458f/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/dependent/data_where_u16_bound_compile/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/dependent/data_where_u16_bound_compile/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_dependent_data_where_u1_b3b3199a42/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/dependent/invariant_field_store_compile/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/dependent/invariant_field_store_compile/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_dependent_invariant_fie_c679ccd255/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/dependent/runtime_symbolic_slice_cell_theorem_exit/main.omg — remove_mut

- Original: `tests/omega/pass/dependent/runtime_symbolic_slice_cell_theorem_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_dependent_runtime_symbo_c2d12c9b3d/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/dependent/runtime_symbolic_slice_cell_theorem_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/dependent/runtime_symbolic_slice_cell_theorem_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_dependent_runtime_symbo_c2d12c9b3d/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/domains/borrow_of_cast_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/domains/borrow_of_cast_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_domains_borrow_of_cast__94d7e00116/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/domains/chained_tagged_reference_moves_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/domains/chained_tagged_reference_moves_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_domains_chained_tagged__3552d7c397/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/domains/domain_operator_add/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/domains/domain_operator_add/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_domains_domain_operator_6aee589868/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/domains/domain_pattern_match_bound_generic_open_same_carrier_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/domains/domain_pattern_match_bound_generic_open_same_carrier_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_domains_domain_pattern__ffb8193af6/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/domains/domain_pattern_match_bound_routed_boolean_retained_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/domains/domain_pattern_match_bound_routed_boolean_retained_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_domains_domain_pattern__50940bbcdb/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/domains/indexed_domain_known_disjoint_call_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/domains/indexed_domain_known_disjoint_call_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_domains_indexed_domain__03e508dbed/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/domains/local_alias_domain_transfer/main.omg — remove_mut

- Original: `tests/omega/pass/domains/local_alias_domain_transfer/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_domains_local_alias_dom_42aa3bbcb2/remove_mut/main.omg`
- Current checker: `accepted`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/domains/local_alias_domain_transfer/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/domains/local_alias_domain_transfer/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_domains_local_alias_dom_42aa3bbcb2/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/domains/qualified_fixed_byte_array_ensured_mutation_exit/main.omg — remove_mut

- Original: `tests/omega/pass/domains/qualified_fixed_byte_array_ensured_mutation_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_domains_qualified_fixed_9db6b45d66/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/domains/qualified_fixed_byte_array_ensured_mutation_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/domains/qualified_fixed_byte_array_ensured_mutation_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_domains_qualified_fixed_9db6b45d66/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/domains/routed_selected_replacement_exit/main.omg — remove_mut

- Original: `tests/omega/pass/domains/routed_selected_replacement_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_domains_routed_selected_d7314712cd/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/domains/routed_selected_replacement_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/domains/routed_selected_replacement_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_domains_routed_selected_d7314712cd/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/domains/runtime_scalar_domain_return_cleanup_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/domains/runtime_scalar_domain_return_cleanup_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_domains_runtime_scalar__01960a6818/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/drops/contextual_core_drop_exit/main.omg — remove_mut

- Original: `tests/omega/pass/drops/contextual_core_drop_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_contextual_core_d_22df35ecce/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/drops/contextual_core_drop_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/drops/contextual_core_drop_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_contextual_core_d_22df35ecce/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/drops/drop_erased_member/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/drops/drop_erased_member/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_drop_erased_membe_6502ff1c93/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/drops/result_cleanup_replacement_exit/main.omg — remove_mut

- Original: `tests/omega/pass/drops/result_cleanup_replacement_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_result_cleanup_re_ef5ccd18f3/remove_mut/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/drops/result_cleanup_replacement_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/drops/result_cleanup_replacement_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_result_cleanup_re_ef5ccd18f3/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/drops/runtime_borrowed_temporary_returned_child_cleanup_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/drops/runtime_borrowed_temporary_returned_child_cleanup_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_runtime_borrowed__678e08752d/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/drops/runtime_composed_nominal_output_hook_exit/main.omg — remove_mut

- Original: `tests/omega/pass/drops/runtime_composed_nominal_output_hook_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_runtime_composed__c4a9492611/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/drops/runtime_composed_nominal_output_hook_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/drops/runtime_composed_nominal_output_hook_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_runtime_composed__c4a9492611/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/drops/runtime_generic_nominal_compound_prerequisite_exit/main.omg — remove_mut

- Original: `tests/omega/pass/drops/runtime_generic_nominal_compound_prerequisite_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_runtime_generic_n_dd856a537c/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/drops/runtime_generic_nominal_compound_prerequisite_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/drops/runtime_generic_nominal_compound_prerequisite_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_runtime_generic_n_dd856a537c/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/drops/runtime_mixed_temporary_scalar_continuation_exit/main.omg — remove_mut

- Original: `tests/omega/pass/drops/runtime_mixed_temporary_scalar_continuation_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_runtime_mixed_tem_cfa3626fd2/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/drops/runtime_mixed_temporary_scalar_continuation_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/drops/runtime_mixed_temporary_scalar_continuation_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_runtime_mixed_tem_cfa3626fd2/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/drops/runtime_nested_temporary_capture_cleanup_exit/main.omg — remove_mut

- Original: `tests/omega/pass/drops/runtime_nested_temporary_capture_cleanup_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_runtime_nested_te_5b1efb427b/remove_mut/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/drops/runtime_nested_temporary_capture_cleanup_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/drops/runtime_nested_temporary_capture_cleanup_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_runtime_nested_te_5b1efb427b/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/drops/runtime_nominal_successor_argument_order_exit/main.omg — remove_mut

- Original: `tests/omega/pass/drops/runtime_nominal_successor_argument_order_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_runtime_nominal_s_dcb037c3fc/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/drops/runtime_nominal_successor_argument_order_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/drops/runtime_nominal_successor_argument_order_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_runtime_nominal_s_dcb037c3fc/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/drops/runtime_projected_nominal_cleanup_exit/main.omg — remove_mut

- Original: `tests/omega/pass/drops/runtime_projected_nominal_cleanup_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_runtime_projected_5c6d00870c/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/drops/runtime_projected_nominal_cleanup_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/drops/runtime_projected_nominal_cleanup_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_runtime_projected_5c6d00870c/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/drops/runtime_scalar_exit_nominal_cleanup_exit/main.omg — remove_mut

- Original: `tests/omega/pass/drops/runtime_scalar_exit_nominal_cleanup_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_runtime_scalar_ex_6cf69870fe/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/drops/runtime_scalar_exit_nominal_cleanup_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/drops/runtime_scalar_exit_nominal_cleanup_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_runtime_scalar_ex_6cf69870fe/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/drops/runtime_selected_projected_sibling_cleanup_exit/main.omg — remove_mut

- Original: `tests/omega/pass/drops/runtime_selected_projected_sibling_cleanup_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_runtime_selected__a90cb6a2ed/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/drops/runtime_selected_projected_sibling_cleanup_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/drops/runtime_selected_projected_sibling_cleanup_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_runtime_selected__a90cb6a2ed/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/drops/runtime_temporary_integer_capture_cleanup_exit/main.omg — remove_mut

- Original: `tests/omega/pass/drops/runtime_temporary_integer_capture_cleanup_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_runtime_temporary_404b2bc071/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/drops/runtime_temporary_integer_capture_cleanup_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/drops/runtime_temporary_integer_capture_cleanup_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_runtime_temporary_404b2bc071/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/drops/snapshot_drop_reassign/main.omg — remove_mut

- Original: `tests/omega/pass/drops/snapshot_drop_reassign/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_snapshot_drop_rea_8950023ea2/remove_mut/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/drops/snapshot_drop_reassign/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/drops/snapshot_drop_reassign/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_snapshot_drop_rea_8950023ea2/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/drops/wildcard_owned_structural_return_cleanup_exit/main.omg — remove_mut

- Original: `tests/omega/pass/drops/wildcard_owned_structural_return_cleanup_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_wildcard_owned_st_0b341be49b/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/drops/wildcard_owned_structural_return_cleanup_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/drops/wildcard_owned_structural_return_cleanup_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_drops_wildcard_owned_st_0b341be49b/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/effects/explicit_none_rows_exit/main.omg — remove_mut

- Original: `tests/omega/pass/effects/explicit_none_rows_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_effects_explicit_none_r_7565b7b295/remove_mut/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/expressions/entry_mut_scalar_param/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/expressions/entry_mut_scalar_param/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_expressions_entry_mut_s_f9e89c8333/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `no-entry`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/expressions/runtime_match_borrowed_call_subject_once_exit/main.omg — remove_mut

- Original: `tests/omega/pass/expressions/runtime_match_borrowed_call_subject_once_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_expressions_runtime_mat_bcaa124482/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/expressions/runtime_match_borrowed_call_subject_once_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/expressions/runtime_match_borrowed_call_subject_once_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_expressions_runtime_mat_bcaa124482/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/expressions/runtime_nested_record_field_call_exit/main.omg — remove_mut

- Original: `tests/omega/pass/expressions/runtime_nested_record_field_call_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_expressions_runtime_nes_bf47dfaf19/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/expressions/runtime_nested_record_field_call_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/expressions/runtime_nested_record_field_call_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_expressions_runtime_nes_bf47dfaf19/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/expressions/runtime_owned_match_pattern_claim_replacement_exit/main.omg — widen_requires

- Original: `tests/omega/pass/expressions/runtime_owned_match_pattern_claim_replacement_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_expressions_runtime_own_2fe8e27d33/widen_requires/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/state_contracts.md):

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### tests/omega/pass/expressions/runtime_owned_match_pattern_claim_replacement_exit/main.omg — remove_mut

- Original: `tests/omega/pass/expressions/runtime_owned_match_pattern_claim_replacement_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_expressions_runtime_own_2fe8e27d33/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/expressions/runtime_owned_match_pattern_claim_replacement_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/expressions/runtime_owned_match_pattern_claim_replacement_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_expressions_runtime_own_2fe8e27d33/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/additional_input_domain_mutation_through_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/additional_input_domain_mutation_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_additional_input__07fd2a2fd2/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/additional_input_domain_mutation_through_target_body_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/additional_input_domain_mutation_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_additional_input__93cb2ee216/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/additional_input_domain_mutation_through_target_body_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/additional_input_domain_mutation_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_additional_input__93cb2ee216/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/address_count_partition_through_parameter_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/address_count_partition_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_address_count_par_66402b10e3/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/affine_discard_root_cleanup_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/affine_discard_root_cleanup_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_affine_discard_ro_20ce259c10/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/affine_discard_root_cleanup_through_other_state_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/affine_discard_root_cleanup_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_affine_discard_ro_20ce259c10/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/aggregate_storage_address_identity_through_other_machine_exit/main.omg — widen_requires

- Original: `tests/omega/pass/facts/aggregate_storage_address_identity_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_aggregate_storage_b11599fb98/widen_requires/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/state_contracts.md):

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### tests/omega/pass/facts/aggregate_storage_address_identity_through_other_machine_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/aggregate_storage_address_identity_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_aggregate_storage_b11599fb98/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/alias_fact_transfer_through_local_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/alias_fact_transfer_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_alias_fact_transf_597fedb617/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/alias_fact_transfer_through_local_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/alias_fact_transfer_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_alias_fact_transf_597fedb617/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/applied_zero_establishment_through_const_name_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/applied_zero_establishment_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_applied_zero_esta_543a35d680/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/array_element_default_domain/call_result/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/array_element_default_domain/call_result/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_array_element_def_bb5813f871/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/array_element_default_domain/target_body/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/array_element_default_domain/target_body/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_array_element_def_ad32f33e43/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/as_slice_views_through_field_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/as_slice_views_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_as_slice_views_th_cbf3e39435/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/atomic_boolean_store/parameter/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/atomic_boolean_store/parameter/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_atomic_boolean_st_3e88987585/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/atomic_bounded_fetch/other_state/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/atomic_bounded_fetch/other_state/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_atomic_bounded_fe_4c181473eb/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/atomic_bounded_resident/other_machine/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/atomic_bounded_resident/other_machine/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_atomic_bounded_re_a5e28b4209/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/atomic_compare_exchange_once_target_body_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/atomic_compare_exchange_once_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_atomic_compare_ex_cdce360eb6/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/atomic_compare_exchange_once_through_parameter_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/atomic_compare_exchange_once_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_atomic_compare_ex_8b5c297dcd/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/atomic_try_exchange_once/local/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/atomic_try_exchange_once/local/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_atomic_try_exchan_7ca3d8ccd4/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/body_bearing_comparison_ordered_field_calls_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/body_bearing_comparison_ordered_field_calls_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_body_bearing_comp_65d43bb294/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/body_bearing_comparison_ordered_field_calls_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/body_bearing_comparison_ordered_field_calls_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_body_bearing_comp_65d43bb294/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/borrowed_case_result_through_match_join_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/borrowed_case_result_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_borrowed_case_res_84de95eb39/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/borrowed_projection_cleanup_through_field_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/borrowed_projection_cleanup_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_borrowed_projecti_a776592e2f/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/borrowed_projection_cleanup_through_field_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/borrowed_projection_cleanup_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_borrowed_projecti_a776592e2f/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/borrowed_restoration_window_through_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/borrowed_restoration_window_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_borrowed_restorat_2b15a32145/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/borrowed_restoration_window_through_const_name_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/borrowed_restoration_window_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_borrowed_restorat_2b15a32145/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/borrowed_store_call_through_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/borrowed_store_call_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_borrowed_store_ca_42c6c46741/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/borrowed_store_call_through_call_result_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/borrowed_store_call_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_borrowed_store_ca_42c6c46741/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/borrowed_store_call_through_target_body_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/borrowed_store_call_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_borrowed_store_ca_a5ccb2229f/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/borrowed_store_call_through_target_body_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/borrowed_store_call_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_borrowed_store_ca_a5ccb2229f/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/boundary_zero_origin_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/boundary_zero_origin_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_boundary_zero_ori_884b7a854b/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/boundary_zero_origin_through_other_state_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/boundary_zero_origin_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_boundary_zero_ori_884b7a854b/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/bounded_activation_claim_split_through_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/bounded_activation_claim_split_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_bounded_activatio_8606d0cf40/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/bounded_activation_claim_split_through_other_machine_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/bounded_activation_claim_split_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_bounded_activatio_8606d0cf40/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/bounded_activation_field_transport_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/bounded_activation_field_transport_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_bounded_activatio_c2f33aa611/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/bounded_activation_field_transport_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/bounded_activation_field_transport_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_bounded_activatio_c2f33aa611/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/bounded_activation_through_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/bounded_activation_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_bounded_activatio_b17a2b1f8c/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/bounded_activation_through_other_machine_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/bounded_activation_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_bounded_activatio_b17a2b1f8c/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/build_activation_borrow/call_result/main.omg — remove_mut

- Original: `tests/omega/pass/facts/build_activation_borrow/call_result/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_build_activation__8a1b07674c/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/build_activation_borrow/call_result/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/build_activation_borrow/call_result/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_build_activation__8a1b07674c/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/build_activation_borrow/target_body/main.omg — remove_mut

- Original: `tests/omega/pass/facts/build_activation_borrow/target_body/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_build_activation__628c87c68f/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/build_activation_borrow/target_body/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/build_activation_borrow/target_body/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_build_activation__628c87c68f/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/c_layout_nested_representation_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/c_layout_nested_representation_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_c_layout_nested_r_ab7bc981ed/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/c_layout_nested_representation_through_other_state_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/c_layout_nested_representation_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_c_layout_nested_r_ab7bc981ed/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/c_layout_scalar_record_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/c_layout_scalar_record_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_c_layout_scalar_r_4a4c3c3155/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/c_layout_scalar_record_through_other_state_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/c_layout_scalar_record_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_c_layout_scalar_r_4a4c3c3155/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/call_result_bound_discharge_through_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/call_result_bound_discharge_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_call_result_bound_03596d51f2/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/call_result_bound_discharge_through_other_machine_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/call_result_bound_discharge_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_call_result_bound_03596d51f2/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/captured_storage_bounds_through_local_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/captured_storage_bounds_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_captured_storage__9aec5b07c3/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/captured_storage_bounds_through_local_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/captured_storage_bounds_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_captured_storage__9aec5b07c3/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/captured_window_slot_retirement_through_field_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/captured_window_slot_retirement_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_captured_window_s_7e97362406/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/captured_window_slot_retirement_through_field_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/captured_window_slot_retirement_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_captured_window_s_7e97362406/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/case_dependent_default_domain_through_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/case_dependent_default_domain_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_case_dependent_de_ea48f47ac9/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/case_dependent_default_domain_through_const_name_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/case_dependent_default_domain_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_case_dependent_de_ea48f47ac9/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/case_match_payload_loan_through_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/case_match_payload_loan_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_case_match_payloa_8955a60679/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/case_match_payload_loan_through_call_result_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/case_match_payload_loan_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_case_match_payloa_8955a60679/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/case_payload_exclusive_loan_through_parameter_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/case_payload_exclusive_loan_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_case_payload_excl_713b35b2ed/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/case_payload_exclusive_loan_through_parameter_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/case_payload_exclusive_loan_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_case_payload_excl_713b35b2ed/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/case_payload_stored_loan_through_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/case_payload_stored_loan_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_case_payload_stor_1e2db0fb70/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/case_payload_stored_loan_through_other_machine_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/case_payload_stored_loan_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_case_payload_stor_1e2db0fb70/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/case_where_equality_through_field_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/case_where_equality_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_case_where_equali_62b73a8913/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/case_where_equality_through_field_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/case_where_equality_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_case_where_equali_62b73a8913/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/checked_asm_add_carry_through_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/checked_asm_add_carry_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_checked_asm_add_c_6f93b9e0da/remove_mut/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/checked_asm_arm_load_through_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/checked_asm_arm_load_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_checked_asm_arm_l_407f0e8f4d/remove_mut/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/checked_asm_arm_load_through_target_body_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/checked_asm_arm_load_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_checked_asm_arm_l_f3ad0f205a/remove_mut/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/checked_asm_compare_preserves_through_parameter_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/checked_asm_compare_preserves_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_checked_asm_compa_d0702578df/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/checked_asm_countdown_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/checked_asm_countdown_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_checked_asm_count_7897145fc9/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/checked_asm_flag_restore_through_local_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/checked_asm_flag_restore_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_checked_asm_flag__9f80f6e1c9/remove_mut/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/checked_asm_flag_snapshot_through_field_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/checked_asm_flag_snapshot_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_checked_asm_flag__e023be858d/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/checked_asm_flag_snapshot_through_field_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/checked_asm_flag_snapshot_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_checked_asm_flag__e023be858d/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/checked_asm_nominal_predicate_through_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/checked_asm_nominal_predicate_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_checked_asm_nomin_4ba90d1b0c/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/checked_asm_nominal_predicate_through_const_name_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/checked_asm_nominal_predicate_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_checked_asm_nomin_4ba90d1b0c/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/checked_asm_rank_decrease_through_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/checked_asm_rank_decrease_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_checked_asm_rank__e27d7cbf8f/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/checked_asm_rank_decrease_through_target_body_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/checked_asm_rank_decrease_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_checked_asm_rank__4968b77fea/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/checked_asm_register_contract_bound_through_parameter_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/checked_asm_register_contract_bound_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_checked_asm_regis_043aa95a8d/remove_mut/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/checked_asm_view_move_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/checked_asm_view_move_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_checked_asm_view__b3e4809af6/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/checked_predicate_disjoint_loans_through_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_checked_predicate_6200799d72/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/checked_predicate_disjoint_loans_through_other_machine_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/checked_predicate_disjoint_loans_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_checked_predicate_6200799d72/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/closed_integer_bound_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/closed_integer_bound_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_closed_integer_bo_02dd42fec1/remove_mut/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/component_scalar_lending_through_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/component_scalar_lending_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_component_scalar__6c78287f21/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/component_scalar_lending_through_const_name_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/component_scalar_lending_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_component_scalar__6c78287f21/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/component_scalar_lending_through_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/component_scalar_lending_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_component_scalar__2b9c1f7d3a/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/component_scalar_lending_through_other_machine_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/component_scalar_lending_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_component_scalar__2b9c1f7d3a/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/compound_index_guard_through_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/compound_index_guard_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_compound_index_gu_4fd30e9354/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/compound_index_guard_through_call_result_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/compound_index_guard_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_compound_index_gu_4fd30e9354/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/compound_index_guard_through_target_body_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/compound_index_guard_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_compound_index_gu_fdf7190daa/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/compound_index_guard_through_target_body_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/compound_index_guard_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_compound_index_gu_fdf7190daa/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/computed_nested_state_loan_through_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/computed_nested_state_loan_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_computed_nested_s_e9b201f0e3/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/computed_nested_state_loan_through_other_machine_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/computed_nested_state_loan_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_computed_nested_s_e9b201f0e3/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/computed_record_window_through_match_join_exit/main.omg — widen_requires

- Original: `tests/omega/pass/facts/computed_record_window_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_computed_record_w_e89b872c1e/widen_requires/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/state_contracts.md):

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### tests/omega/pass/facts/computed_record_window_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/computed_record_window_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_computed_record_w_e89b872c1e/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/computed_record_window_through_match_join_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/computed_record_window_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_computed_record_w_e89b872c1e/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/computed_subslice_extent_through_field_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/computed_subslice_extent_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_computed_subslice_effc0423ed/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/content_interval_adjacency_through_const_name_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/content_interval_adjacency_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_content_interval__c03217aec1/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/content_reunion_through_call_result_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/content_reunion_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_content_reunion_t_6bf53c49bc/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/content_reunion_through_target_body_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/content_reunion_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_content_reunion_t_d8ec02839e/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/copied_call_argument_snapshot_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/copied_call_argument_snapshot_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_copied_call_argum_363055d885/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/copied_call_argument_snapshot_through_other_state_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/copied_call_argument_snapshot_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_copied_call_argum_363055d885/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/copied_case_before_later_argument_through_other_machine_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/copied_case_before_later_argument_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_copied_case_befor_fe906c34c0/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/correlated_invariant_store_through_field_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/correlated_invariant_store_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_correlated_invari_f1bcb739eb/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/correlated_invariant_store_through_field_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/correlated_invariant_store_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_correlated_invari_f1bcb739eb/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/crash_entry_ieee_view_through_const_name_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/crash_entry_ieee_view_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_crash_entry_ieee__8d5f1e6617/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/crash_entry_nonempty_path_through_parameter_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/crash_entry_nonempty_path_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_crash_entry_nonem_e32c33a13c/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/crash_entry_representation_view_through_other_state_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/crash_entry_representation_view_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_crash_entry_repre_6016b063f9/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/crash_entry_signed_zero_through_other_machine_exit/main.omg — widen_requires

- Original: `tests/omega/pass/facts/crash_entry_signed_zero_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_crash_entry_signe_710e1f63d2/widen_requires/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/state_contracts.md):

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### tests/omega/pass/facts/data_where_standing_borrowed_field_array_local/main.omg — remove_mut

- Original: `tests/omega/pass/facts/data_where_standing_borrowed_field_array_local/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_data_where_standi_ba5e1b3bba/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/data_where_standing_borrowed_field_array_local/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/data_where_standing_borrowed_field_array_local/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_data_where_standi_ba5e1b3bba/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/data_where_standing_borrowed_field_through_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/data_where_standing_borrowed_field_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_data_where_standi_0fd418b741/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/data_where_standing_borrowed_field_through_call_result_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/data_where_standing_borrowed_field_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_data_where_standi_0fd418b741/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/data_where_standing_borrowed_field_through_target_body_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/data_where_standing_borrowed_field_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_data_where_standi_a3fb49c52d/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/data_where_standing_borrowed_field_through_target_body_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/data_where_standing_borrowed_field_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_data_where_standi_a3fb49c52d/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/data_where_standing_mutable_recast_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/data_where_standing_mutable_recast_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_data_where_standi_1ecda18efb/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/data_where_standing_mutable_recast_through_match_join_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/data_where_standing_mutable_recast_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_data_where_standi_1ecda18efb/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/data_where_standing_predicate_helper_through_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/data_where_standing_predicate_helper_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_data_where_standi_76e5fd1487/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/data_where_standing_recast_owner_restoration/main.omg — remove_mut

- Original: `tests/omega/pass/facts/data_where_standing_recast_owner_restoration/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_data_where_standi_d462a51454/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/data_where_standing_recast_owner_restoration/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/data_where_standing_recast_owner_restoration/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_data_where_standi_d462a51454/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/data_where_standing_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/data_where_standing_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_data_where_standi_c9fe467b5f/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/data_where_standing_through_match_join_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/data_where_standing_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_data_where_standi_c9fe467b5f/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/data_where_standing_write_only_direct_field/main.omg — remove_mut

- Original: `tests/omega/pass/facts/data_where_standing_write_only_direct_field/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_data_where_standi_db03d434ef/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/data_where_standing_write_only_relational_window/main.omg — remove_mut

- Original: `tests/omega/pass/facts/data_where_standing_write_only_relational_window/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_data_where_standi_0bc547bf64/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/default_domain_field_write_through_local_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/default_domain_field_write_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_default_domain_fi_e065fc6dad/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/dependency_type_qualified_call_through_field_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/dependency_type_qualified_call_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_dependency_type_q_74a838c993/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/dependent_subtract_through_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/dependent_subtract_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_dependent_subtrac_88c2cba30b/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/dependent_subtract_through_call_result_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/dependent_subtract_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_dependent_subtrac_88c2cba30b/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/dependent_subtract_through_target_body_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/dependent_subtract_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_dependent_subtrac_f3ecfb9ec1/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/dependent_subtract_through_target_body_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/dependent_subtract_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_dependent_subtrac_f3ecfb9ec1/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/descriptor_payload_exclusive_loan_through_parameter_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/descriptor_payload_exclusive_loan_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_descriptor_payloa_0ce946e244/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/descriptor_payload_exclusive_loan_through_parameter_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/descriptor_payload_exclusive_loan_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_descriptor_payloa_0ce946e244/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/descriptor_range_exclusive_loan_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/descriptor_range_exclusive_loan_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_descriptor_range__6768f5ee8f/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/descriptor_range_exclusive_loan_through_other_state_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/descriptor_range_exclusive_loan_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_descriptor_range__6768f5ee8f/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/descriptor_replacement_order_through_other_machine_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/descriptor_replacement_order_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_descriptor_replac_e143bd51fe/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/disjoint_field_borrow_windows_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/disjoint_field_borrow_windows_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_disjoint_field_bo_21dc8cb974/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/disjoint_field_borrow_windows_through_match_join_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/disjoint_field_borrow_windows_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_disjoint_field_bo_21dc8cb974/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/disjoint_windows_through_local_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/disjoint_windows_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_disjoint_windows__f842790519/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/disjoint_windows_through_local_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/disjoint_windows_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_disjoint_windows__f842790519/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/domain_pattern_match_bound_through_call_result_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/domain_pattern_match_bound_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_domain_pattern_ma_101f5c9acd/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/domain_pattern_match_bound_through_target_body_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/domain_pattern_match_bound_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_domain_pattern_ma_bd38943d81/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/domain_selected_cleanup_computed_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/domain_selected_cleanup_computed_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_domain_selected_c_62970e46d3/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/domain_selected_cleanup_computed_other_state_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/domain_selected_cleanup_computed_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_domain_selected_c_62970e46d3/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/domain_selected_cleanup_indexed_inherited_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/domain_selected_cleanup_indexed_inherited_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_domain_selected_c_239a198f5d/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/domain_selected_cleanup_indexed_inherited_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/domain_selected_cleanup_indexed_inherited_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_domain_selected_c_239a198f5d/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/domain_selected_cleanup_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/domain_selected_cleanup_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_domain_selected_c_aaf570e880/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/domain_selected_cleanup_through_other_state_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/domain_selected_cleanup_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_domain_selected_c_aaf570e880/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/double_indexed_rmw_through_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/double_indexed_rmw_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_double_indexed_rm_2b3ceec031/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/double_indexed_rmw_through_other_machine_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/double_indexed_rmw_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_double_indexed_rm_2b3ceec031/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/dynamic_trait_dispatch_through_parameter_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/dynamic_trait_dispatch_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_dynamic_trait_dis_b66acc04f2/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/dynamic_trait_dispatch_through_parameter_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/dynamic_trait_dispatch_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_dynamic_trait_dis_b66acc04f2/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/erased_reference_contract_through_other_state_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/erased_reference_contract_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_erased_reference__98176cf86d/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/exact_sub_guard_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/exact_sub_guard_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_exact_sub_guard_t_0bcb8b6d57/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/exact_sub_guard_through_match_join_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/exact_sub_guard_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_exact_sub_guard_t_0bcb8b6d57/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/exclusive_match_arm_through_local_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/exclusive_match_arm_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_exclusive_match_a_a188d228bf/remove_mut/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/exclusive_match_arm_through_local_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/exclusive_match_arm_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_exclusive_match_a_a188d228bf/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/exclusive_observation_exposure_through_field_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/exclusive_observation_exposure_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_exclusive_observa_a204c6a65a/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/exclusive_observation_exposure_through_field_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/exclusive_observation_exposure_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_exclusive_observa_a204c6a65a/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/field_where_index_through_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/field_where_index_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_field_where_index_26fc5847d7/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/field_where_index_through_const_name_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/field_where_index_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_field_where_index_26fc5847d7/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/finite_family_exclusive_result/parameter/main.omg — remove_mut

- Original: `tests/omega/pass/facts/finite_family_exclusive_result/parameter/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_finite_family_exc_2840302c53/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/finite_family_exclusive_result/parameter/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/finite_family_exclusive_result/parameter/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_finite_family_exc_2840302c53/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/finite_family_input_index/other_state/main.omg — remove_mut

- Original: `tests/omega/pass/facts/finite_family_input_index/other_state/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_finite_family_inp_985a0d68c0/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/finite_family_input_index/other_state/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/finite_family_input_index/other_state/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_finite_family_inp_985a0d68c0/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/finite_family_owned_field/other_machine/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/finite_family_owned_field/other_machine/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_finite_family_own_a24ebf3c05/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/finite_family_returned_descriptor/match_join/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/finite_family_returned_descriptor/match_join/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_finite_family_ret_681ac77889/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/finite_family_static_inline_result/local/main.omg — remove_mut

- Original: `tests/omega/pass/facts/finite_family_static_inline_result/local/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_finite_family_sta_a5a9393d9e/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/finite_family_static_inline_result/local/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/finite_family_static_inline_result/local/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_finite_family_sta_a5a9393d9e/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/fixed_layout_bit_observation_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/fixed_layout_bit_observation_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_fixed_layout_bit__c0dedd418d/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/fixed_layout_bit_observation_through_match_join_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/fixed_layout_bit_observation_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_fixed_layout_bit__c0dedd418d/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/fixed_layout_byte_roundtrip_through_local_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/fixed_layout_byte_roundtrip_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_fixed_layout_byte_e770b9ce71/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/fixed_layout_byte_roundtrip_through_local_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/fixed_layout_byte_roundtrip_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_fixed_layout_byte_e770b9ce71/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/fixed_layout_reference_readback_through_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/fixed_layout_reference_readback_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_fixed_layout_refe_f71bb5ebe0/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/fixed_layout_storage_generic_policy_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/fixed_layout_storage_generic_policy_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_fixed_layout_stor_2994c6e431/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/fixed_layout_storage_generic_policy_const_name_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/fixed_layout_storage_generic_policy_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_fixed_layout_stor_2994c6e431/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/fixed_layout_storage_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/fixed_layout_storage_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_fixed_layout_stor_9f4129df70/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/fixed_layout_storage_through_other_state_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/fixed_layout_storage_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_fixed_layout_stor_9f4129df70/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/fixed_region_byte_roundtrip_through_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/fixed_region_byte_roundtrip_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_fixed_region_byte_172becc274/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/fixed_region_byte_roundtrip_through_other_machine_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/fixed_region_byte_roundtrip_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_fixed_region_byte_172becc274/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/float_range_delivery_through_field_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/float_range_delivery_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_float_range_deliv_0c31d04de3/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/float_result_equality_through_const_name_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/float_result_equality_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_float_result_equa_3462fc2317/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/foreign_byte_array_element_backing_through_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/foreign_byte_array_element_backing_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_foreign_byte_arra_90e3cbbe66/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/foreign_byte_array_element_backing_through_call_result_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/foreign_byte_array_element_backing_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_foreign_byte_arra_90e3cbbe66/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/foreign_byte_array_element_backing_through_target_body_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/foreign_byte_array_element_backing_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_foreign_byte_arra_006ea89868/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/foreign_byte_array_element_backing_through_target_body_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/foreign_byte_array_element_backing_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_foreign_byte_arra_006ea89868/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/foreign_nested_record_backing_other_state/main.omg — remove_mut

- Original: `tests/omega/pass/facts/foreign_nested_record_backing_other_state/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_foreign_nested_re_eb078b095b/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/foreign_nested_record_backing_other_state/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/foreign_nested_record_backing_other_state/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_foreign_nested_re_eb078b095b/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/foreign_reference_result_through_local_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/foreign_reference_result_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_foreign_reference_a575aab8a8/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/foreign_reference_result_through_local_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/foreign_reference_result_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_foreign_reference_a575aab8a8/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/foreign_retained_mutable_borrow_field/main.omg — remove_mut

- Original: `tests/omega/pass/facts/foreign_retained_mutable_borrow_field/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_foreign_retained__4a2951f787/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/foreign_retained_mutable_borrow_field/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/foreign_retained_mutable_borrow_field/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_foreign_retained__4a2951f787/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/foreign_retained_mutable_borrow_target_body/main.omg — remove_mut

- Original: `tests/omega/pass/facts/foreign_retained_mutable_borrow_target_body/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_foreign_retained__2f8d756126/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/foreign_retained_mutable_borrow_target_body/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/foreign_retained_mutable_borrow_target_body/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_foreign_retained__2f8d756126/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/foreign_retained_shared_borrow_other_machine/main.omg — remove_mut

- Original: `tests/omega/pass/facts/foreign_retained_shared_borrow_other_machine/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_foreign_retained__359c206d3b/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/foreign_retained_shared_borrow_other_machine/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/foreign_retained_shared_borrow_other_machine/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_foreign_retained__359c206d3b/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/foreign_retained_shared_borrow_shared_aliases/main.omg — remove_mut

- Original: `tests/omega/pass/facts/foreign_retained_shared_borrow_shared_aliases/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_foreign_retained__c31590c33e/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/foreign_retained_shared_borrow_shared_aliases/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/foreign_retained_shared_borrow_shared_aliases/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_foreign_retained__c31590c33e/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/foreign_scalar_backing_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/foreign_scalar_backing_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_foreign_scalar_ba_3e7a6882cd/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/foreign_scalar_backing_through_other_state_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/foreign_scalar_backing_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_foreign_scalar_ba_3e7a6882cd/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/free_entry_local_state_through_local_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/free_entry_local_state_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_free_entry_local__cc33d3d1f4/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/free_entry_local_state_through_local_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/free_entry_local_state_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_free_entry_local__cc33d3d1f4/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/generic_content_transfer_through_field_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/generic_content_transfer_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_generic_content_t_ac360cfd6b/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/generic_requirement_statement_call_through_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/generic_requirement_statement_call_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_generic_requireme_92ee591776/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/generic_requirement_statement_call_through_const_name_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/generic_requirement_statement_call_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_generic_requireme_92ee591776/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/generic_terminated_array_through_call_result_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/generic_terminated_array_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_generic_terminate_cff38e5ffd/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/generic_terminated_array_through_target_body_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/generic_terminated_array_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_generic_terminate_44c0225ac2/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/guarded_call_entry_through_parameter_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/guarded_call_entry_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_guarded_call_entr_f54f32260b/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/guarded_call_entry_through_parameter_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/guarded_call_entry_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_guarded_call_entr_f54f32260b/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/guarded_line_loan_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/guarded_line_loan_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_guarded_line_loan_d4fa33548a/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/guarded_line_loan_through_other_state_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/guarded_line_loan_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_guarded_line_loan_d4fa33548a/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/guarded_record_line_loan_through_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/guarded_record_line_loan_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_guarded_record_li_5376f47177/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/guarded_record_line_loan_through_other_machine_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/guarded_record_line_loan_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_guarded_record_li_5376f47177/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/implementation_region_forwarding_through_field_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/implementation_region_forwarding_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_implementation_re_cede2c1302/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/in_place_view_argument_through_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/in_place_view_argument_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_in_place_view_arg_a3036b322e/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/in_place_view_argument_through_const_name_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/in_place_view_argument_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_in_place_view_arg_a3036b322e/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/independent_content_partition_through_other_state_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/independent_content_partition_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_independent_conte_c385c5f12b/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/indexed_crash_guard_through_match_join_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/indexed_crash_guard_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_indexed_crash_gua_8ff9c0a2ec/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/indexed_element_mut_loan_through_local_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/indexed_element_mut_loan_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_indexed_element_m_7c3be310c8/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/indexed_recoverable_replacement_window_through_field_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/indexed_recoverable_replacement_window_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_indexed_recoverab_7fe62eb409/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/indexed_recoverable_replacement_window_through_field_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/indexed_recoverable_replacement_window_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_indexed_recoverab_7fe62eb409/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/indexed_storage_address_identity_through_const_name_exit/main.omg — widen_requires

- Original: `tests/omega/pass/facts/indexed_storage_address_identity_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_indexed_storage_a_6bf08fcbf6/widen_requires/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/state_contracts.md):

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### tests/omega/pass/facts/indexed_storage_address_identity_through_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/indexed_storage_address_identity_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_indexed_storage_a_6bf08fcbf6/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/indexed_storage_address_identity_through_const_name_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/indexed_storage_address_identity_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_indexed_storage_a_6bf08fcbf6/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/inferred_array_equation_call_mixed_binders_through_parameter_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/inferred_array_equation_call_mixed_binders_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_inferred_array_eq_5bb3f9a98d/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/invariant_window_snapshot_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/invariant_window_snapshot_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_invariant_window__a517ffd939/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/invariant_window_snapshot_through_other_state_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/invariant_window_snapshot_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_invariant_window__a517ffd939/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/length_through_other_machine_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/length_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_length_through_ot_96c368df20/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/linear_claim_retirement_through_local_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/linear_claim_retirement_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_linear_claim_reti_0896b22ba2/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/linear_owner_borrowed_projection_field/main.omg — remove_mut

- Original: `tests/omega/pass/facts/linear_owner_borrowed_projection_field/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_linear_owner_borr_2f54468b2d/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/linear_owner_borrowed_projection_field/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/linear_owner_borrowed_projection_field/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_linear_owner_borr_2f54468b2d/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/linear_routed_label_through_const_name_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/linear_routed_label_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_linear_routed_lab_41047926d3/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/match_computed_root_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/match_computed_root_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_match_computed_ro_d213116412/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/match_computed_root_through_match_join_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/match_computed_root_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_match_computed_ro_d213116412/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/match_effectful_subject_custody_through_local_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/match_effectful_subject_custody_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_match_effectful_s_0997df5678/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/match_effectful_subject_custody_through_local_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/match_effectful_subject_custody_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_match_effectful_s_0997df5678/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/match_join_place_read_through_const_name_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/match_join_place_read_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_match_join_place__97b79f7b8b/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/match_loan_last_use_through_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/match_loan_last_use_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_match_loan_last_u_ce83fb1d63/remove_mut/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/match_loan_last_use_through_call_result_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/match_loan_last_use_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_match_loan_last_u_ce83fb1d63/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/match_loan_last_use_through_target_body_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/match_loan_last_use_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_match_loan_last_u_e67c5a9ffd/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/match_loan_last_use_through_target_body_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/match_loan_last_use_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_match_loan_last_u_e67c5a9ffd/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/match_runtime_view_extent_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/match_runtime_view_extent_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_match_runtime_vie_573c6be3c9/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/match_runtime_view_extent_through_match_join_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/match_runtime_view_extent_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_match_runtime_vie_573c6be3c9/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/matched_field_windows_through_local_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/matched_field_windows_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_matched_field_win_dace8967c8/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/matched_field_windows_through_local_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/matched_field_windows_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_matched_field_win_dace8967c8/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/may_write_choice_through_field_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/may_write_choice_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_may_write_choice__20b7074239/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/may_write_choice_through_field_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/may_write_choice_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_may_write_choice__20b7074239/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/may_write_codec_argument_through_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/may_write_codec_argument_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_may_write_codec_a_5bc20dc91f/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/may_write_codec_argument_through_const_name_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/may_write_codec_argument_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_may_write_codec_a_5bc20dc91f/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/may_write_codec_through_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/may_write_codec_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_may_write_codec_t_65e02a488e/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/may_write_codec_through_call_result_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/may_write_codec_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_may_write_codec_t_65e02a488e/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/may_write_codec_through_target_body_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/may_write_codec_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_may_write_codec_t_f97f9e8553/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/may_write_codec_through_target_body_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/may_write_codec_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_may_write_codec_t_f97f9e8553/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/may_write_computed_receiver_through_parameter_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/may_write_computed_receiver_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_may_write_compute_7746b6aa09/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/may_write_computed_receiver_through_parameter_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/may_write_computed_receiver_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_may_write_compute_7746b6aa09/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/may_write_named_state_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/may_write_named_state_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_may_write_named_s_0096ce0e69/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/may_write_named_state_through_other_state_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/may_write_named_state_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_may_write_named_s_0096ce0e69/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/may_write_nested_helper_through_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/may_write_nested_helper_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_may_write_nested__d5b7aa8c89/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/may_write_nested_helper_through_other_machine_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/may_write_nested_helper_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_may_write_nested__d5b7aa8c89/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/may_write_stored_rebind_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/may_write_stored_rebind_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_may_write_stored__3ff715a295/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/may_write_stored_rebind_through_match_join_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/may_write_stored_rebind_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_may_write_stored__3ff715a295/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/method_array_operand_inference_through_local_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/method_array_operand_inference_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_method_array_oper_5bd8a21182/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/mutable_borrow_field_writeback_through_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/mutable_borrow_field_writeback_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_mutable_borrow_fi_81b291ac6a/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/mutable_borrow_field_writeback_through_call_result_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/mutable_borrow_field_writeback_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_mutable_borrow_fi_81b291ac6a/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/mutable_borrow_field_writeback_through_target_body_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/mutable_borrow_field_writeback_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_mutable_borrow_fi_eed6193d44/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/mutable_borrow_field_writeback_through_target_body_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/mutable_borrow_field_writeback_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_mutable_borrow_fi_eed6193d44/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/mutable_slice_element_write_bound_through_parameter_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/mutable_slice_element_write_bound_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_mutable_slice_ele_195cc7b76d/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/mutable_slice_element_write_bound_through_parameter_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/mutable_slice_element_write_bound_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_mutable_slice_ele_195cc7b76d/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/named_const_callable_family_through_other_machine_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/named_const_callable_family_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_named_const_calla_7a5b955638/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/narrowed_integer_write_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/narrowed_integer_write_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_narrowed_integer__2b96879f59/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/narrowed_integer_write_through_match_join_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/narrowed_integer_write_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_narrowed_integer__2b96879f59/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/negated_mathematical_order_through_local_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/negated_mathematical_order_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_negated_mathemati_a1f2de8d35/remove_mut/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/nested_borrow_qualification_through_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/nested_borrow_qualification_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_nested_borrow_qua_638eadb35a/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/nested_borrow_qualification_through_const_name_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/nested_borrow_qualification_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_nested_borrow_qua_638eadb35a/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/nested_machine_joint_rank_bounded_computed_length_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/nested_machine_joint_rank_bounded_computed_length_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_nested_machine_jo_8e1b099096/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/nested_machine_joint_rank_bounded_computed_length_other_state_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/nested_machine_joint_rank_bounded_computed_length_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_nested_machine_jo_8e1b099096/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/nested_record_field_borrow_bound_through_match_join_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/nested_record_field_borrow_bound_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_nested_record_fie_095561d0fc/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/nested_result_extent_through_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/nested_result_extent_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_nested_result_ext_fd456485b1/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/nested_result_extent_through_const_name_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/nested_result_extent_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_nested_result_ext_fd456485b1/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/nested_zero_leaf_array_storage_through_parameter_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/nested_zero_leaf_array_storage_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_nested_zero_leaf__0c703202ce/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/nominal_loan_backing_cleanup_through_field_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/nominal_loan_backing_cleanup_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_nominal_loan_back_afe909b1dc/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/nominal_loan_backing_cleanup_through_field_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/nominal_loan_backing_cleanup_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_nominal_loan_back_afe909b1dc/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/nominal_record_representation_through_const_name_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/nominal_record_representation_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_nominal_record_re_5379fa1076/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/nominal_reference_cleanup_through_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/nominal_reference_cleanup_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_nominal_reference_04e5ecf716/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/nominal_reference_cleanup_through_call_result_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/nominal_reference_cleanup_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_nominal_reference_04e5ecf716/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/nominal_reference_cleanup_through_target_body_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/nominal_reference_cleanup_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_nominal_reference_9eb81b6fe5/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/nominal_reference_cleanup_through_target_body_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/nominal_reference_cleanup_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_nominal_reference_9eb81b6fe5/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/placed_stable_write_through_field_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/placed_stable_write_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_placed_stable_wri_fc6fc7e60b/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/placed_stable_write_through_field_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/placed_stable_write_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_placed_stable_wri_fc6fc7e60b/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/predicate_array_copy_through_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/predicate_array_copy_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_predicate_array_c_d5b2121817/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/predicate_array_copy_through_call_result_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/predicate_array_copy_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_predicate_array_c_d5b2121817/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/predicate_array_copy_through_target_body_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/predicate_array_copy_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_predicate_array_c_ffb8e7affd/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/predicate_array_copy_through_target_body_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/predicate_array_copy_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_predicate_array_c_ffb8e7affd/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/progress_selector_nested_arrival_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/progress_selector_nested_arrival_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_progress_selector_923093c622/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/progress_selector_nested_arrival_through_other_state_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/progress_selector_nested_arrival_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_progress_selector_923093c622/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/progress_selector_owned_arrival_through_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/progress_selector_owned_arrival_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_progress_selector_d0b18c045d/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/progress_selector_owned_arrival_through_other_machine_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/progress_selector_owned_arrival_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_progress_selector_d0b18c045d/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/progress_selector_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/progress_selector_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_progress_selector_7ec44511e5/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/progress_selector_through_match_join_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/progress_selector_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_progress_selector_7ec44511e5/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/qualified_field_capacity_through_local_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/qualified_field_capacity_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_qualified_field_c_6d15fe934d/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/recast_parent_restoration_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/recast_parent_restoration_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_recast_parent_res_1e4e1bba7d/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/recast_parent_restoration_through_other_state_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/recast_parent_restoration_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_recast_parent_res_1e4e1bba7d/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/reference_arrival_handoff_through_field_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/reference_arrival_handoff_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_reference_arrival_0598aff530/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/reference_arrival_handoff_through_field_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/reference_arrival_handoff_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_reference_arrival_0598aff530/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/reference_recast_backing_footprint_through_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/reference_recast_backing_footprint_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_reference_recast__e0f6b7b130/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/reference_recast_backing_footprint_through_const_name_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/reference_recast_backing_footprint_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_reference_recast__e0f6b7b130/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/reflection_declaration_inspection_through_parameter_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/reflection_declaration_inspection_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_reflection_declar_4472bb05a6/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/reflection_declaration_inspection_through_parameter_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/reflection_declaration_inspection_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_reflection_declar_4472bb05a6/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/remainder_width_index_through_other_machine_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/remainder_width_index_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_remainder_width_i_01bcf22aa9/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/replaced_field_loan_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/replaced_field_loan_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_replaced_field_lo_bb9cd4ba47/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/replaced_field_loan_through_match_join_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/replaced_field_loan_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_replaced_field_lo_bb9cd4ba47/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/representation_agreement_through_local_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/representation_agreement_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_representation_ag_22959e4f7c/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/resident_backing_call_result_exit/main.omg — widen_requires

- Original: `tests/omega/pass/facts/resident_backing_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_resident_backing__5d257d2dca/widen_requires/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/state_contracts.md):

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### tests/omega/pass/facts/resident_backing_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/resident_backing_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_resident_backing__5d257d2dca/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/resident_backing_call_result_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/resident_backing_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_resident_backing__5d257d2dca/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/resident_backing_other_machine_exit/main.omg — widen_requires

- Original: `tests/omega/pass/facts/resident_backing_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_resident_backing__79c27ed7a9/widen_requires/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/state_contracts.md):

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### tests/omega/pass/facts/resident_backing_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/resident_backing_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_resident_backing__79c27ed7a9/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/resident_backing_other_machine_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/resident_backing_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_resident_backing__79c27ed7a9/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/resident_backing_runtime_reconstruction_loop_exit/main.omg — widen_requires

- Original: `tests/omega/pass/facts/resident_backing_runtime_reconstruction_loop_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_resident_backing__68be774286/widen_requires/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/state_contracts.md):

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### tests/omega/pass/facts/resident_backing_runtime_reconstruction_loop_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/resident_backing_runtime_reconstruction_loop_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_resident_backing__68be774286/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/resident_backing_runtime_reconstruction_loop_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/resident_backing_runtime_reconstruction_loop_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_resident_backing__68be774286/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/result_cleanup_guarantees_through_other_state_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/result_cleanup_guarantees_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_result_cleanup_gu_14249ddd3f/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/returned_indexed_linear_residual_through_other_machine_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/returned_indexed_linear_residual_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_returned_indexed__3032fc6025/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/returned_linear_residual_through_match_join_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/returned_linear_residual_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_returned_linear_r_46f0b9e0ec/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/returned_write_destination_through_local_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/returned_write_destination_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_returned_write_de_8a2853b2eb/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/runtime_count_index_through_field_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/runtime_count_index_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_runtime_count_ind_e5efcc6a14/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/runtime_index_application_computed_through_parameter_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/runtime_index_application_computed_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_runtime_index_app_044182ae7d/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/runtime_index_application_computed_through_parameter_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/runtime_index_application_computed_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_runtime_index_app_044182ae7d/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/runtime_index_application_equivalent_subject_through_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/runtime_index_application_equivalent_subject_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_runtime_index_app_25f8563d61/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/runtime_index_application_equivalent_subject_through_other_machine_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/runtime_index_application_equivalent_subject_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_runtime_index_app_25f8563d61/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/runtime_index_application_forwarded_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/runtime_index_application_forwarded_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_runtime_index_app_3d541ed68b/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/runtime_index_application_forwarded_through_match_join_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/runtime_index_application_forwarded_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_runtime_index_app_3d541ed68b/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/runtime_index_application_mutating_operator_through_local_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/runtime_index_application_mutating_operator_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_runtime_index_app_0871120062/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/runtime_index_application_mutating_operator_through_local_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/runtime_index_application_mutating_operator_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_runtime_index_app_0871120062/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/runtime_index_application_nested_forwarded_through_field_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/runtime_index_application_nested_forwarded_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_runtime_index_app_f330775b74/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/runtime_index_application_nested_forwarded_through_field_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/runtime_index_application_nested_forwarded_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_runtime_index_app_f330775b74/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/runtime_index_application_ordinary_call_through_const_name_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/runtime_index_application_ordinary_call_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_runtime_index_app_ea53703bda/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/runtime_index_application_ordinary_call_through_const_name_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/runtime_index_application_ordinary_call_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_runtime_index_app_ea53703bda/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/runtime_index_application_selected_operator_through_parameter_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/runtime_index_application_selected_operator_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_runtime_index_app_61438fb02f/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/runtime_index_application_selected_operator_through_parameter_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/runtime_index_application_selected_operator_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_runtime_index_app_61438fb02f/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/saved_case_predicate_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/saved_case_predicate_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_saved_case_predic_7d637b25fe/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/saved_case_predicate_through_match_join_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/saved_case_predicate_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_saved_case_predic_7d637b25fe/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/saved_observation_snapshot_after_call_through_call_result_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/saved_observation_snapshot_after_call_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_saved_observation_c3694cff95/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/saved_observation_snapshot_after_call_through_call_result_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/saved_observation_snapshot_after_call_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_saved_observation_c3694cff95/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/saved_observation_snapshot_after_call_through_target_body_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/saved_observation_snapshot_after_call_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_saved_observation_7ecef72bbf/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/saved_observation_snapshot_after_call_through_target_body_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/saved_observation_snapshot_after_call_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_saved_observation_7ecef72bbf/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/saved_observation_snapshot_indexed_handback_parameter_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/saved_observation_snapshot_indexed_handback_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_saved_observation_ba7aefa2f6/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/saved_observation_snapshot_indexed_handback_parameter_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/saved_observation_snapshot_indexed_handback_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_saved_observation_ba7aefa2f6/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/saved_observation_snapshot_predicate_handback_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/saved_observation_snapshot_predicate_handback_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_saved_observation_01110f92b3/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/saved_observation_snapshot_predicate_handback_match_join_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/saved_observation_snapshot_predicate_handback_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_saved_observation_01110f92b3/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/saved_observation_snapshot_stored_reference_handback_local_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/saved_observation_snapshot_stored_reference_handback_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_saved_observation_da6710ccc2/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/saved_observation_snapshot_stored_reference_handback_local_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/saved_observation_snapshot_stored_reference_handback_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_saved_observation_da6710ccc2/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/saved_observation_snapshot_through_field_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/saved_observation_snapshot_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_saved_observation_3901b17bd7/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/saved_observation_snapshot_through_field_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/saved_observation_snapshot_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_saved_observation_3901b17bd7/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/scalar_tag_field_loan_through_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/scalar_tag_field_loan_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_scalar_tag_field__88602cde3d/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/scalar_tag_field_loan_through_other_machine_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/scalar_tag_field_loan_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_scalar_tag_field__88602cde3d/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/selected_native_code_layout_through_call_result_exit/main.omg — widen_requires

- Original: `tests/omega/pass/facts/selected_native_code_layout_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_selected_native_c_b3c838eb10/widen_requires/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/state_contracts.md):

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### tests/omega/pass/facts/selected_native_code_layout_through_call_result_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/selected_native_code_layout_through_call_result_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_selected_native_c_b3c838eb10/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/selected_native_code_layout_through_target_body_exit/main.omg — widen_requires

- Original: `tests/omega/pass/facts/selected_native_code_layout_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_selected_native_c_8f04e91d18/widen_requires/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/state_contracts.md):

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### tests/omega/pass/facts/selected_native_code_layout_through_target_body_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/selected_native_code_layout_through_target_body_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_selected_native_c_8f04e91d18/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/selected_provider_issuance_through_parameter_exit/main.omg — widen_requires

- Original: `tests/omega/pass/facts/selected_provider_issuance_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_selected_provider_256ac0e37f/widen_requires/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/state_contracts.md):

> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.

### tests/omega/pass/facts/selected_provider_issuance_through_parameter_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/selected_provider_issuance_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_selected_provider_256ac0e37f/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/selected_provider_issuance_through_parameter_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/selected_provider_issuance_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_selected_provider_256ac0e37f/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/shared_borrow_field_projection_through_match_join_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/shared_borrow_field_projection_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_shared_borrow_fie_782c670c78/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/shared_byte_view_rebinding_through_local_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/shared_byte_view_rebinding_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_shared_byte_view__4b3d9411b0/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/shared_qualification_erasure_through_field_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/shared_qualification_erasure_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_shared_qualificat_f09ceed57c/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/skipped_short_circuit_call_through_parameter_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/skipped_short_circuit_call_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_skipped_short_cir_9827983b15/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/skipped_short_circuit_call_through_parameter_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/skipped_short_circuit_call_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_skipped_short_cir_9827983b15/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/slice_view_element_read_bound_through_other_state_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/slice_view_element_read_bound_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_slice_view_elemen_f90c94e394/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/stack_frame_slot_isolation_through_other_machine_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/stack_frame_slot_isolation_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_stack_frame_slot__c0385332c3/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/stack_frame_slot_isolation_through_other_machine_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/stack_frame_slot_isolation_through_other_machine_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_stack_frame_slot__c0385332c3/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/static_callable_type_requirements_through_local_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/static_callable_type_requirements_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_static_callable_t_b36160d8ea/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/static_callable_type_requirements_through_local_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/static_callable_type_requirements_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_static_callable_t_b36160d8ea/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/stored_exclusive_byte_view_through_local_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/stored_exclusive_byte_view_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_stored_exclusive__5c82c6a2cc/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/stored_exclusive_byte_view_through_local_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/stored_exclusive_byte_view_through_local_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_stored_exclusive__5c82c6a2cc/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/stored_shared_byte_view_element_lend_through_field_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/stored_shared_byte_view_element_lend_through_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_stored_shared_byt_294e225f03/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/suspension_transported_loan_through_const_name_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/suspension_transported_loan_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_suspension_transp_9fcdc2dfbb/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/target_family_contract_operand/parameter/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/target_family_contract_operand/parameter/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_target_family_con_9e831363c4/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/target_family_invocation_lifetime/other_state/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/target_family_invocation_lifetime/other_state/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_target_family_inv_83229a3ff4/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/target_layout_derived_constant_through_match_join_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/target_layout_derived_constant_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_target_layout_der_a47db4ab79/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/target_layout_derived_constant_through_match_join_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/target_layout_derived_constant_through_match_join_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_target_layout_der_a47db4ab79/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/temporary_case_cleanup_through_const_name_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/temporary_case_cleanup_through_const_name_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_temporary_case_cl_8d869243c3/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/typed_reflection_mutation_through_parameter_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/typed_reflection_mutation_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_typed_reflection__d2ea29438f/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/typed_reflection_mutation_through_parameter_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/typed_reflection_mutation_through_parameter_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_typed_reflection__d2ea29438f/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/typed_reflection_through_other_state_exit/main.omg — remove_mut

- Original: `tests/omega/pass/facts/typed_reflection_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_typed_reflection__a4403c0e32/remove_mut/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.

### tests/omega/pass/facts/typed_reflection_through_other_state_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/typed_reflection_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_typed_reflection__a4403c0e32/flip_borrow_mutability/main.omg`
- Current checker: `rejected`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/value_location_alignment_contract_field_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/value_location_alignment_contract_field_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_value_location_al_843a07033d/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |

### tests/omega/pass/facts/value_location_alignment_through_other_state_exit/main.omg — flip_borrow_mutability

- Original: `tests/omega/pass/facts/value_location_alignment_through_other_state_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-0/mutants/tests_omega_pass_facts_value_location_al_97f7a69914/flip_borrow_mutability/main.omg`
- Current checker: `accepted`; route: `outside`
- Hand check: **confirmed-invalid**
- Spec (wiki/spec/language/ownership.md):

> | `&mut T` | Exclusive observation and mutation. |
