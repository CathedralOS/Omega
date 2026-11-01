#!/usr/bin/env python3
"""Prepare the matching-host C boundary; Darwin uses the installed SDK itself."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--cc', default=os.environ.get('CC') or ('clang' if os.name=='nt' else 'cc'))
    args=parser.parse_args()
    fixture=Path(__file__).resolve().parent
    out=args.output.resolve();out.mkdir(parents=True,exist_ok=True)
    host=platform.system()
    if host not in ('Darwin','Linux','Windows'):
        parser.error('matching-host fixture requires a hosted platform')
    if host=='Darwin' and platform.machine().lower() not in ('arm64','aarch64'):
        parser.error('Darwin SDK observation requires ARM64')
    library=out/({'Darwin':'libomega_darwin_stat_fixture.dylib','Linux':'libomega_darwin_stat_fixture.so','Windows':'omega_darwin_stat_fixture.dll'}[host])
    test=out/('provider-test.exe' if host=='Windows' else 'provider-test')
    common=[args.cc,'-std=c11','-Wall','-Wextra','-Werror','-O2']
    shared=(['-dynamiclib','-Wl,-install_name,'+library.name] if host=='Darwin' else
            ['-shared'] if host=='Windows' else ['-shared','-fPIC','-Wl,--version-script='+str(fixture/'provider.map')])
    commands=[common+['-DDARWIN_STAT_FIXTURE_TEST',str(fixture/'provider.c'),'-o',str(test)],
              common+shared+[str(fixture/'provider.c'),'-o',str(library)],[str(test)]]
    observations=[]
    for command in commands:
        result=subprocess.run(command,capture_output=True,text=True,timeout=60)
        observations.append(dict(argv=command,exit=result.returncode,stdout=result.stdout,stderr=result.stderr))
        if result.returncode:
            raise SystemExit(result.stdout+result.stderr)
    sdk=None
    if host=='Darwin':
        sdk={}
        for name,command in [('path',['xcrun','--show-sdk-path']),('version',['xcrun','--show-sdk-version']),('compiler',[args.cc,'--version'])]:
            sdk[name]=subprocess.check_output(command,text=True).strip()
    print(json.dumps(dict(host=platform.platform(),architecture=platform.machine(),sdk=sdk,
        source_model='actual Darwin SDK struct stat' if host=='Darwin' else 'explicit Darwin format replica; no Darwin SDK qualification',
        library=str(library),library_sha256=digest(library),selftest_sha256=digest(test),
        provider_sha256=digest(fixture/'provider.c'),map_sha256=digest(fixture/'provider.map'),
        selftest_exit=0,commands=commands,observations=observations),indent=2))


if __name__=='__main__':
    main()
