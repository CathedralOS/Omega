#!/usr/bin/env python3
"""SDK transport substitute: real Git objects, no external network fixture.

The operator selects this executable through PATH. Production code sees an
ordinary HTTPS request and performs its normal object/custody checks. Only this
test-owned I/O adapter maps the authored locator to a temporary repository.
"""

import os
from pathlib import Path
import sys

arguments = sys.argv[1:]
if "fetch" in arguments or "ls-remote" in arguments:
    repositories = {
        "https://omega-source.invalid/executed-source.git": "OMEGA_FIXTURE_GIT_REPOSITORY",
        "https://omega-source.invalid/catalog-helper.git": "OMEGA_FIXTURE_GIT_HELPER",
    }
    selected = next((locator for locator in repositories if locator in arguments), None)
    if selected is not None and repositories[selected] in os.environ:
        arguments = [
            Path(os.environ[repositories[selected]]).as_uri()
            if argument == selected
            else "protocol.file.allow=always"
            if argument == "protocol.file.allow=never"
            else argument
            for argument in arguments
        ]
        os.environ["GIT_ALLOW_PROTOCOL"] = "file"
        with open(os.environ["OMEGA_FIXTURE_GIT_FETCH_LOG"], "a") as log:
            log.write("fetch\n")
    else:
        raise SystemExit("fixture Git refuses external transport")
os.execv(os.environ["OMEGA_FIXTURE_REAL_GIT"], ["git", *arguments])
