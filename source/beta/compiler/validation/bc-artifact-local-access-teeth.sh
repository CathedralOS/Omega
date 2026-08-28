#!/usr/bin/env sh
# Checker-A procedure-local load/store custody negatives.

artifact_local_access_build_teeth() { :; }

artifact_local_access_reject_teeth() {
  case_run "valid-slot local load retarget" 1 "$T/local-load-slot.bundle"
  case_run "valid-slot local store retarget" 1 "$T/local-store-slot.bundle"
  case_run "local frame-base register" 1 "$T/local-base.bundle"
  case_run "same-width local load/store opcode" 1 "$T/local-load-opcode.bundle"
  case_run "same-width local store/load opcode" 1 "$T/local-store-opcode.bundle"
  case_run "duplicate local access location" 1 "$T/duplicate-local.bundle"
  case_run "noncanonical local access order" 1 "$T/noncanonical-local.bundle"
}
