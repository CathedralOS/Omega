#!/usr/bin/env python3
"""Link the private stream observer against one verified compiler build.
No Cargo workspace or shared exports are changed; supports mbx output placement
and ordinary Cargo deps. Run only after that compiler build has completed.
"""
from pathlib import Path
import json, subprocess, sys
root=Path(__file__).resolve().parents[1]
target=Path(sys.argv[1]).resolve()
output=Path(sys.argv[2]).resolve()
output.parent.mkdir(parents=True,exist_ok=True)
command=['rustc','--edition=2024',str(root/'validation/terminal.rs'),'-o',str(output)]
for directory in sorted({p.parent for pattern in ['lib*.rlib','lib*.rmeta','lib*.so'] for p in target.rglob(pattern)}):
    command += ['-L',f'dependency={directory}']
omega_libraries=list(target.rglob('fingerprint/lib-omega.json'))
if len(omega_libraries)>1:
    binary=target/'omega'
    products=[p for p in target.glob('build/omega/*/out/omega') if p.samefile(binary)]
    assert len(products)==1, products
    binary_metadata=json.loads((products[0].parent.parent/'fingerprint/bin-omega.json').read_text())
    required={row[1]:row[3] for row in binary_metadata['deps']}['omega']
    omega_libraries=[p for p in omega_libraries if int.from_bytes(bytes.fromhex((p.parent/'lib-omega').read_text()),'little')==required]
assert len(omega_libraries)==1,omega_libraries
omega_dependencies={row[1]:row[3] for row in json.loads(omega_libraries[0].read_text())['deps']}
for crate in ['compiler', 'terminal_runtime','package_manager','target','terminal_interpreter','omega_build']:
    matches=list(target.rglob(f'lib{crate}-*.rlib'))
    if not matches: matches=list(target.glob(f'lib{crate}.rlib'))
    if crate=='omega':
        matches=[p for p in matches if p.parent.parent==omega_libraries[0].parent.parent]
    if len(matches)>1:
        # Select the exact dependency fingerprint of the verified CLI library,
        # never a newer file by timestamp or a different corpus feature graph.
        required=omega_dependencies[crate]
        matches=[p for p in matches if int.from_bytes(bytes.fromhex(
            (p.parent.parent/'fingerprint'/f'lib-{crate}').read_text()),'little')==required]
    assert len(matches)==1,(crate,matches)
    metadata=matches[0].with_suffix('.rmeta')
    command+=['--extern',f'{crate}={matches[0]}']
    if metadata.exists(): command+=['--extern',f'{crate}={metadata}']
(output.parent/'driver-link-command.json').write_text(json.dumps(command,indent=2)+'\n')
result=subprocess.run(command)
raise SystemExit(result.returncode)
