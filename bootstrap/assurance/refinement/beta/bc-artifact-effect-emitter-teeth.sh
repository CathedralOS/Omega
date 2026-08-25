#!/usr/bin/env sh
# Checker-A source-effect and fixed-emitter custody negatives.

artifact_effect_emitter_build_teeth() { :; }

artifact_effect_emitter_reject_teeth() {
  case_run "valid-entry source call retarget" 1 "$T/call-retarget.bundle"
  case_run "source read register" 1 "$T/read-register.bundle"
  case_run "source write register" 1 "$T/write-register.bundle"
  case_run "helper write register" 1 "$T/helper-write.bundle"
  case_run "jump-skipped emit byte" 1 "$T/emit-byte.bundle"
  case_run "emit length" 1 "$T/emit-length.bundle"
  case_run "emit pointer" 1 "$T/emit-pointer.bundle"
  case_run "emit helper target" 1 "$T/emit-helper.bundle"
  case_run "same-width read/write opcode" 1 "$T/orphan-io.bundle"
  case_run "duplicate source effect location" 1 "$T/duplicate-event.bundle"
  case_run "noncanonical source effect order" 1 "$T/noncanonical-event.bundle"
}
