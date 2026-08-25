#!/usr/bin/env sh
# Phase-isolated call-bound witness teeth; bundles come from common setup.

call_bounds_build_teeth() {
  :
}

call_bounds_reject_teeth() {
  case_run "underreported rejected recursive probe" 1 "$T/call-bounds-probe.bundle"
  case_run "underreported root stack bound" 1 "$T/call-bounds-root.bundle"
}
