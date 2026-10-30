from pathlib import Path
import hashlib,importlib.util,json,subprocess
spec=importlib.util.spec_from_file_location('full_runner',Path(__file__).with_name('full_profile_run.py'))
h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
if __name__=='__main__':
    d=h.OUT/'producers/full-profile-detail';profile=json.loads(subprocess.check_output([str(d/'omega.exe'),'--build-profile']));assert int(profile['opt_level'])>0
    identity=dict(producer='full-profile-detail',compiler_sha256=hashlib.sha256((d/'omega.exe').read_bytes()).hexdigest(),accepted_lock_sha256=hashlib.sha256((d/'omega.lock').read_bytes()).hexdigest(),build_profile=profile)
    (h.WORK/'refined-identity.json').write_text(json.dumps(identity,indent=2)+'\n',encoding='utf-8',newline='\n')
    h.run('full-profile-detail','full-refined-off',False)
    for letter in 'abc':h.run('full-profile-detail','full-refined-'+letter,True)
    h.run('baseline','full-refined-original',False)
    h.run('full-profile-detail','full-refined-warm-a',True,warm_from='full-refined-a')
    h.run('full-profile-detail','full-refined-warm-b',True,warm_from='full-refined-a')
    h.run('full-profile-detail','full-refined-verified',comparison=True)
