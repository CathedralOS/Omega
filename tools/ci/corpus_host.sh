#!/bin/bash
# Prepare a CI container to run the corpus the way a cloud session does.
#
# The corpus jobs (.github/workflows/corpus-gate.yml, corpus-sweep.yml) run in
# ubuntu:24.04 as root with the checkout at /home/user/Omega, the container and
# path the warm-build job builds main in. Cargo keys its builds by absolute
# path and checksum, so restoring that build here leaves only the crates the
# candidate changed to compile: about 35 seconds for the corpus runner at
# main, against about 11 minutes cold on 4 CPUs.
#
#   cd /home/user/Omega && bash tools/ci/corpus_host.sh
#
# A failed restore (a toolchain change, a missing release asset) is not an
# error: the build is then cold and the job is slower, never wrong.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."
[ "$(pwd -P)" = /home/user/Omega ] || { echo "corpus host: run from /home/user/Omega" >&2; exit 2; }
bash tools/agent_setup.sh
bash tools/warm_build.sh restore primary ||
  echo "corpus host: warm build not restored; the first build compiles everything"
