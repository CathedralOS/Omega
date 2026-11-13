#!/bin/bash
# Prepare a CI container to build the way a cloud session does.
#
# The post-merge jobs that build the compiler (.github/workflows/post-merge-check.yml)
# run in ubuntu:24.04 as root with the checkout at /home/user/Omega, the container
# and path the warm-build job builds main in. Cargo keys its builds by absolute
# path and checksum, so restoring that build here leaves only the crates the
# commit changed to compile.
#
#   cd /home/user/Omega && bash tools/ci/session_host.sh
#
# A failed restore (a toolchain change, a missing release asset) is not an
# error: the build is then cold and the job is slower, never wrong.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."
[ "$(pwd -P)" = /home/user/Omega ] || { echo "session host: run from /home/user/Omega" >&2; exit 2; }
bash tools/agent_setup.sh
bash tools/warm_build.sh restore primary ||
  echo "session host: warm build not restored; the first build compiles everything"
