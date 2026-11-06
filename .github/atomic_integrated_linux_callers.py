"""One bounded no-build Source prefix under an owned GDB process."""
from pathlib import Path, PurePosixPath
import base64, hashlib, json, os, re, selectors, shutil, signal, subprocess, sys, time, traceback, urllib.error, urllib.request, zipfile
R=Path.cwd();RECIPE=json.loads(Path(sys.argv[1]).read_text());OUT=R/'build/atomic-integrated-linux-callers';OUT.mkdir(parents=True,exist_ok=False)
sha=lambda b:hashlib.sha256(b).hexdigest();read=lambda p:json.loads(p.read_text());git=lambda *a:subprocess.check_output(['git',*a],text=True).strip()
record={'sampling_only':True,'qualification':False,'source':RECIPE['source'],'driver':os.environ['GITHUB_SHA'],'fixture':RECIPE['fixture'],'snapshots':[],'cleanup':{},'passed':False};process=None;pidfd=None;runner_pid=None;began=None;selector=None;rawfiles={};buffers={};results={};stops=[];started_groups=[];streams=bytearray();token=0;allow_stop=False
try:
 assert os.geteuid()!=0 and git('rev-parse','HEAD')==RECIPE['source'] and git('rev-parse','HEAD^{tree}')==RECIPE['tree'] and not git('status','--porcelain')
 for p,obj in RECIPE['objects'].items():assert git('rev-parse','HEAD:'+p)==obj
 for p,h in RECIPE['inputs'].items():assert sha((R/p).read_bytes())==h
 setup=read(R/'build/atomic-integrated-linux-gdb-setup/summary.json');assert setup['passed'] and setup['uid']==os.geteuid()!=0
 gdb=shutil.which('gdb');assert gdb==setup['gdb']['path'] and sha(Path(gdb).read_bytes())==setup['gdb']['sha256']
 version=subprocess.run([gdb,'--version'],capture_output=True,timeout=10,check=True);(OUT/'gdb-version.stdout').write_bytes(version.stdout)
 record['debugger_setup']=setup
 assert hasattr(os,'pidfd_open') and hasattr(signal,'pidfd_send_signal')
 probe=os.pidfd_open(os.getpid());os.close(probe)
 run=json.loads(subprocess.check_output(['gh','api','repos/CathedralOS/Omega/actions/runs/'+str(RECIPE['origin_run'])],text=True));assert run['head_sha']==RECIPE['source'] and run['status']=='completed' and run['conclusion']=='failure'
 artifact=json.loads(subprocess.check_output(['gh','api','repos/CathedralOS/Omega/actions/artifacts/'+str(RECIPE['artifact']['id'])],text=True))
 for k in ['id','name','size_in_bytes','digest']:assert artifact[k]==RECIPE['artifact'][k]
 assert not artifact['expired'] and artifact['workflow_run']['id']==RECIPE['origin_run'] and artifact['workflow_run']['head_sha']==RECIPE['source']
 auth=subprocess.check_output(['gh','auth','token'],text=True).strip()
 class NoRedirect(urllib.request.HTTPRedirectHandler):
  def redirect_request(self,*args,**kwargs):return None
 request=urllib.request.Request(artifact['archive_download_url'],headers={'Authorization':'Bearer '+auth,'Accept':'application/vnd.github+json'})
 try:urllib.request.build_opener(NoRedirect()).open(request,timeout=30);raise AssertionError('expected signed redirect')
 except urllib.error.HTTPError as e:
  assert e.code==302;signed=e.headers['Location']
 archive=OUT/'producer.zip';digest=hashlib.sha256();size=0;download_started=time.monotonic()
 with urllib.request.urlopen(signed,timeout=60) as response,archive.open('wb') as saved:
  while True:
   chunk=response.read(1024*1024)
   if not chunk:break
   size+=len(chunk);assert size<=artifact['size_in_bytes'] and time.monotonic()-download_started<600;digest.update(chunk);saved.write(chunk)
 assert size==artifact['size_in_bytes'] and artifact['digest']=='sha256:'+digest.hexdigest()
 needed=RECIPE['members'];extracted=OUT/'retained';extracted.mkdir();extraction=[]
 with zipfile.ZipFile(archive) as z:
  names=[i.filename for i in z.infolist()];assert len(names)==len(set(names))
  for name in needed:
   i=z.getinfo(name);p=PurePosixPath(name);assert not p.is_absolute() and '..' not in p.parts and not i.is_dir() and (i.external_attr>>16)&0o170000 not in (0o120000,0o060000,0o020000) and i.file_size<300*1024*1024
   raw=z.read(i);dest=extracted/p;dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(raw);extraction.append({'name':name,'bytes':len(raw),'sha256':sha(raw)})
 original=read(extracted/RECIPE['producer_record']);assert original==RECIPE['producer']
 prep=read(extracted/'preparation.json');assert prep['passed'] and prep['candidate']==original['revision']==RECIPE['source'] and prep['tree']==original['tree']==RECIPE['tree'] and prep['producer']==original
 sys.path[:0]=[str(R/'tools/ci'),str(R/'tools')];import compiler_pilot
 assert compiler_pilot.corpus_gate.host_target()=='linux_x86_64'
 binaries={k:(extracted/RECIPE['producer_parent']/Path(original[k]).name).resolve() for k in ['cli','runner']}
 observed_profiles={}
 for kind,binary in binaries.items():
  binary.chmod(0o755);assert sha(binary.read_bytes())==original[kind+'_profile']['sha256']
  saved=read(binary.with_name(binary.name+'.verified-profile.json'));assert saved['source_revision']==RECIPE['source'] and saved['source_tree']==RECIPE['tree'] and saved['observation']==original[kind+'_profile']
  actual=compiler_pilot.corpus_gate.require_optimized(binary);assert actual['sha256']==original[kind+'_profile']['sha256'] and actual['profile']==original[kind+'_profile']['profile'];observed_profiles[kind]=actual
 def verify():
  assert sha(Path(gdb).read_bytes())==setup['gdb']['sha256']
  assert git('rev-parse','HEAD')==RECIPE['source'] and not git('status','--porcelain')
  for p,h in RECIPE['inputs'].items():assert sha((R/p).read_bytes())==h
  for kind,binary in binaries.items():assert compiler_pilot.corpus_gate.require_optimized(binary)==observed_profiles[kind]
 verify()
 cache=OUT/'source-cache';cache.mkdir();env={k:os.environ[k] for k in ['PATH','HOME','LANG','LC_ALL','TMPDIR','LD_LIBRARY_PATH'] if k in os.environ}
 env.update(RUST_MIN_STACK='67108864',OMEGA_LIBRARY_ROOT=str(R/'source/library'),XDG_CACHE_HOME=str(cache),OMEGA_CORPUS_ROOT=str(R),OMEGA_CORPUS_FIXTURE_FILTER=RECIPE['fixture'],OMEGA_CORPUS_FIXTURE_SECS='120',OMEGA_CORPUS_JOBS='1',OMEGA_CORPUS_RECORDS=str(OUT/'source.records.txt'))
 assert not any(k in env for k in ['GH_TOKEN','GITHUB_TOKEN','OMEGA_PHASE_PROFILE','OMEGA_CORPUS_NATIVE','OMEGA_CORPUS_INTERPRETED','OMEGA_CORPUS_NATIVE_TARGET','OMEGA_CORPUS_CHECKED_INTERPRETER'])
 command=[gdb,'--quiet','-nx','-nh','-iex','set auto-load off','--interpreter=mi3']
 record.update(origin_run=run,artifact=artifact,archive_bytes=size,archive_sha256=digest.hexdigest(),extracted_members=extraction,actual_profiles=observed_profiles,debugger_command=command,runner_command=[str(binaries['runner'])],source_environment=env,source_objects=RECIPE['objects'],input_hashes=RECIPE['inputs'],timeout_seconds=120,snapshot_seconds=[60,90],limits='Diagnostic prefix only. No engine/qualification verdict or CPU-share/sole-bottleneck inference. No build, attach, elevation, network symbols or argument/local values.')
 (OUT/'observation.json').write_text(json.dumps(record,indent=2)+'\n')
 process=subprocess.Popen(command,cwd=R,env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
 selector=selectors.DefaultSelector()
 for channel,pipe in [('stdout',process.stdout),('stderr',process.stderr)]:
  os.set_blocking(pipe.fileno(),False);selector.register(pipe,selectors.EVENT_READ,channel);rawfiles[channel]=(OUT/('gdb.'+channel)).open('wb');buffers[channel]=b''
 def pump(timeout):
  global buffers, pidfd, runner_pid
  for key,_ in selector.select(timeout):
   data=os.read(key.fileobj.fileno(),65536)
   if not data:selector.unregister(key.fileobj);continue
   ch=key.data;rawfiles[ch].write(data);rawfiles[ch].flush();assert rawfiles[ch].tell()<=16*1024*1024
   streams.extend(data);assert len(streams)<32*1024*1024;buffers[ch]+=data
   while b'\n' in buffers[ch]:
    line,buffers[ch]=buffers[ch].split(b'\n',1);text=line.decode(errors='replace')
    if ch!='stdout':continue
    m=re.match(r'^(\d+)\^(done|running|error|exit)(?:,|$)',text)
    if m:results[int(m[1])]=(m[2],text)
    if re.match(r'^\d*\*stopped(?:,|$)',text):
     stops.append({'seconds':None if began is None else round(time.monotonic()-began,6),'raw':text});assert allow_stop,'uncommanded stop'
    m=re.match(r'^=thread-group-started,id="i1",pid="(\d+)"$',text)
    if m:
     assert not started_groups
     runner_pid=int(m[1]);held=os.pidfd_open(runner_pid)
     try:
      state=Path(f'/proc/{runner_pid}/stat').read_text().rsplit(')',1)[1].split();assert int(state[1])==process.pid
     except Exception:
      os.close(held);raise
     pidfd=held;started_groups.append(runner_pid)
    assert not re.match(r'^=thread-group-started,id="(?!i1")',text),'unexpected inferior group'
 def send(command,expected='done',seconds=5):
  global token
  token+=1;current=token;process.stdin.write((str(current)+command+'\n').encode());process.stdin.flush();until=time.monotonic()+seconds
  while current not in results:
   assert process.poll() is None and time.monotonic()<until,'debugger command timed out'
   if began is not None:assert time.monotonic()-began<110,'diagnostic stop budget'
   pump(min(.1,max(0,until-time.monotonic())))
  result=results.pop(current);assert result[0]==expected,result;return result[1]
 for setting in ['mi-async on','non-stop off','startup-with-shell off','disable-randomization off','debuginfod enabled off','pagination off','confirm off','print frame-arguments none','print entry-values no','print thread-events off']:send('-gdb-set '+setting)
 send('-file-exec-and-symbols '+json.dumps(str(binaries['runner'])));send('-exec-arguments')
 began=time.monotonic();send('-exec-run','running')
 deadline=began+10
 while not started_groups or b'corpus_runner: fixture-root-v1 ' not in streams:
  assert process.poll() is None and time.monotonic()<deadline; pump(.1)
 assert len(started_groups)==1 and pidfd is not None
 stat=Path(f'/proc/{runner_pid}/stat').read_text().rsplit(')',1)[1].split();assert int(stat[1])==process.pid
 assert Path(f'/proc/{runner_pid}/exe').resolve()==binaries['runner'] and sha(Path(f'/proc/{runner_pid}/exe').read_bytes())==original['runner_profile']['sha256']
 roots=re.findall(rb'corpus_runner: fixture-root-v1 ([0-9a-f]+)',streams);assert len(roots)==1 and Path(bytes.fromhex(roots[0].decode()).decode()).resolve()==R
 record['runner_identity']={'pid':runner_pid,'parent_pid':process.pid,'start_ticks':stat[19],'image':str(binaries['runner']),'sha256':original['runner_profile']['sha256'],'fixture_root':str(R),'held_pidfd':True}
 for delay in [60,90]:
  while time.monotonic()-began<delay:
   assert process.poll() is None and not stops; pump(min(.1,delay-(time.monotonic()-began)))
  allow_stop=True;before=len(stops);send('-exec-interrupt --all')
  until=time.monotonic()+5
  while len(stops)==before:assert process.poll() is None and time.monotonic()<until;pump(.05)
  assert len(stops)==before+1 and 'reason="signal-received"' in stops[-1]['raw'] and 'signal-name="SIGINT"' in stops[-1]['raw'],stops
  start=rawfiles['stdout'].tell();send('-interpreter-exec console '+json.dumps('thread apply all backtrace -frame-arguments none -entry-values no -no-filters 32'),seconds=10);end=rawfiles['stdout'].tell()
  record['snapshots'].append({'requested_seconds':delay,'observed_seconds':round(time.monotonic()-began,6),'stop':stops[-1],'stdout_start':start,'stdout_end':end})
  (OUT/'observation.json').write_text(json.dumps(record,indent=2)+'\n')
  if delay==60:stops.clear();allow_stop=False;send('-exec-continue','running')
 record['passed']=len(record['snapshots'])==2
except Exception as e:
 record['error']=type(e).__name__+': '+str(e);record['traceback']=traceback.format_exc()
finally:
 if process is not None:
  # Bind any descendants to this private debugger session before termination.
  descendants=[]
  try:
   listing=subprocess.run(['ps','-s',str(process.pid),'-o','pid='],capture_output=True,text=True,timeout=2)
   if listing.returncode not in (0,1):raise AssertionError(listing.returncode)
   pids=[int(x) for x in listing.stdout.split()];assert len(pids)<=128
   for child in pids:
    if child==process.pid or child==runner_pid:continue
    held=None
    try:
     held=os.pidfd_open(child)
     if os.getsid(child)==process.pid:descendants.append((child,held))
     else:os.close(held)
    except ProcessLookupError:
     if held is not None:os.close(held)
   for child,held in descendants:
    try:signal.pidfd_send_signal(held,signal.SIGKILL)
    except ProcessLookupError:pass
   record['cleanup']['owned_session_descendants']=[p for p,_ in descendants]
  except Exception as e:record['cleanup']['descendant_cleanup_error']=str(e);record['passed']=False
  # The pidfd holds the exact authenticated inferior even if its PID is reused.
  if pidfd is not None:
   try:signal.pidfd_send_signal(pidfd,signal.SIGKILL)
   except ProcessLookupError:pass
   except Exception as e:record['cleanup']['runner_kill_error']=str(e)
  try:process.kill() if process.poll() is None else None;process.wait(timeout=3)
  except Exception as e:record['cleanup']['debugger_reap_error']=str(e)
  if pidfd is not None:
   try:
    import select
    record['cleanup']['held_runner_exited']=bool(select.select([pidfd],[],[],2)[0])
   except Exception as e:record['cleanup']['runner_wait_error']=str(e)
   os.close(pidfd)
  else:record['cleanup']['held_runner_exited']=False
  if descendants:
   try:
    import select
    pending={held for _,held in descendants};until=time.monotonic()+2
    while pending and time.monotonic()<until:
     ready=select.select(list(pending),[],[],max(0,until-time.monotonic()))[0];pending.difference_update(ready)
    if pending:record['cleanup']['descendant_wait_error']=len(pending);record['passed']=False
   finally:
    for _,held in descendants:os.close(held)
  record['cleanup']['debugger_exited']=process.poll() is not None
  if began is not None:record['cleanup']['source_seconds_including_cleanup']=round(time.monotonic()-began,6)
  for f in rawfiles.values():f.close()
  if selector is not None:selector.close()
 try:
  if 'verify' in globals():verify();record['post_source_binary_checks']=True
 except Exception as e:record['postcheck_error']=str(e);record['passed']=False
 if process is not None:record['passed']=record['passed'] and record['cleanup'].get('held_runner_exited') is True and record['cleanup'].get('debugger_exited') is True and record['cleanup'].get('source_seconds_including_cleanup',121)<120 and not any('error' in key for key in record['cleanup'])
 (OUT/'observation.json').write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps({'passed':record['passed'],'sampling_only':True,'qualification':False,'error':record.get('error'),'snapshots':record['snapshots'],'cleanup':record['cleanup']},indent=2))
raise SystemExit(0 if record['passed'] else 1)
