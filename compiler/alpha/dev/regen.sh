#!/usr/bin/env sh
# DEV ONLY — rebuild the seed binary + its flat hex after editing the listing
# (../alpha_x64_windows.hex). Python appears only here; using and reproducing the seed
# need no Python. After running, run ./reproduce.sh to verify.
set -e
cd "$(dirname "$0")"
python materialize.py ../alpha_x64_windows.hex ../alpha_x64_windows.exe
xxd -p ../alpha_x64_windows.exe > alpha_x64_windows.flat.hex
echo "rebuilt ../alpha_x64_windows.exe + alpha_x64_windows.flat.hex"
