#!/usr/bin/env sh
# Checker-A raw-memory width, register, and witness-order custody negatives.

artifact_raw_memory_build_teeth() { :; }

artifact_raw_memory_reject_teeth() {
  case_run "raw memory load width" 1 "$T/memory-load-width.bundle"
  case_run "raw memory store width" 1 "$T/memory-store-width.bundle"
  case_run "raw memory load register" 1 "$T/memory-load-register.bundle"
  case_run "raw memory store register" 1 "$T/memory-store-register.bundle"
  case_run "raw memory store pop step" 1 "$T/memory-pop-step.bundle"
  case_run "duplicate raw memory location" 1 "$T/duplicate-memory.bundle"
  case_run "noncanonical raw memory order" 1 "$T/noncanonical-memory.bundle"
}
