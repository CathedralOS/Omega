#!/bin/bash
set -euo pipefail
sha="${1:?}"
git show --stat --format= --no-renames "$sha" | head -20 | tr '\n' ' ' | cut -c1-200
