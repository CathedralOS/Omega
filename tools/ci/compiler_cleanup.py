#!/usr/bin/env python3
"""Matching-host CML4 evidence through the ordinary corpus execution adapter.

The existing programs own their exit, output and diagnostic oracles. This bounded
roster measures ordered cleanup, partial establishment, replacement and the
returned-linear-child fact on the selected host; it is not release qualification.
"""

import sys

import compiler_pilot

PLACES = ("local", "field", "parameter", "call_result", "match_join", "const_name",
          "other_state", "other_machine", "target_body")

POSITIVES = tuple("pass/drops/" + name for name in (
    "runtime_projected_nominal_cleanup_exit",
    "runtime_indexed_nominal_cleanup_exit",
    "runtime_composed_nominal_output_hook_exit",
    "runtime_selected_transfer_fieldwise_construction_exit",
    "runtime_selected_transfer_incomplete_construction_exit",
    "runtime_selected_partial_owner_cleanup_exit",
    "runtime_selected_nominal_owner_cleanup_exit",
    "runtime_record_construction_cleanup_order_exit",
    "runtime_abandoned_record_construction_prefix_exit",
    "runtime_borrowed_nominal_replacement_exit",
    "runtime_nominal_array_replacement_exit",
    "runtime_replaced_record_custody_transfer_exit",
    "runtime_owned_replacement_reestablishment_exit",
    "runtime_owned_record_replacement_exit",
    "runtime_owned_nominal_replacement_exit",
    "runtime_owned_replacement_composition_exit",
    "runtime_empty_array_owner_cleanup_exit",
    "runtime_nested_empty_array_owner_cleanup_exit",
    "runtime_nested_cleanup_postcondition_exit",
    "runtime_generic_reserved_construction_exit",
    "runtime_cleanup_helper_normal_guarantee_exit",
)) + tuple(f"pass/facts/returned_linear_residual_through_{place}_exit" for place in PLACES)

NEGATIVES = tuple("fail/drops/" + name for name in (
    "selected_partial_cleanup_prerequisite",
    "selected_partial_replacement_old_cleanup_prerequisite",
    "selected_partial_requires_disposition",
    "selected_replacement_old_cleanup_prerequisite",
    "borrowed_replacement_old_hook_prerequisite",
    "owned_replacement_old_hook_prerequisite",
    "owned_replacement_transferred_source_read",
)) + tuple(f"fail/facts/returned_linear_residual_through_{place}_rejected" for place in PLACES)


def positives_for_target(target):
    # These existing descriptor controls bind the 64-bit C imports on macOS
    # and Linux. Windows cleanup uses its separate CRT declaration and ABI.
    if target in ("macos_arm64", "linux_x86_64"):
        return POSITIVES + (
            "pass/host/runtime_descriptor_output_streams_exit",
        )
    return POSITIVES


if __name__ == "__main__":
    sys.exit(compiler_pilot.run_suite("compiler-cleanup",
                                    positives_for_target(compiler_pilot.corpus_gate.host_target()),
                                    NEGATIVES))
