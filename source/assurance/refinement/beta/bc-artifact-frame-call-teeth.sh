#!/usr/bin/env sh
# Checker-A frame, parameter, and ordinary-call custody negatives.

artifact_frame_call_build_teeth() { :; }

artifact_frame_call_reject_teeth() {
  case_run "frame allocation size" 1 "$T/frame-size.bundle"
  case_run "saved frame-pointer register" 1 "$T/saved-fp.bundle"
  case_run "frame-base register" 1 "$T/frame-base.bundle"
  case_run "parameter slot offset" 1 "$T/param-offset.bundle"
  case_run "parameter source register" 1 "$T/param-register.bundle"
  case_run "two-argument pop order" 1 "$T/call-pop-order.bundle"
  case_run "argument pop stack step" 1 "$T/call-pop-step.bundle"
}
