import sys,json,subprocess
from pathlib import Path
sys.path.insert(0,'build/proof-profile')
import terminal_breakdown_run as h
p=h.OUT/'producers/terminal-demand';profile=json.loads(subprocess.check_output([str(p/'omega.exe'),'--build-profile']));assert int(profile['opt_level'])>0
identities=json.loads((h.WORK/'identities.json').read_text());assert len(identities)==2
identities.append(dict(producer='terminal-demand',compiler_sha256=h.runner.digest(p/'omega.exe'),accepted_lock_sha256=h.runner.digest(p/'omega.lock'),build_profile=profile))
(h.WORK/'identities.json').write_text(json.dumps(identities,indent=2)+'\n',encoding='utf-8',newline='\n')
h.run('terminal-demand','term-demand-cold','counts')
h.run('terminal-demand','term-demand-warm','counts',warm_from='term-demand-cold')
h.run('terminal-demand','term-demand-verified',comparison=True)
