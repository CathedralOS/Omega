// Exercise the actual chart renderer; no DOM package or copied layout algorithm.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const html = fs.readFileSync(path.join(__dirname, '../dashboard/index.html'), 'utf8');
const renderer = html.slice(html.indexOf('function renderHistory()'), html.indexOf('function renderOutcomes()'));
assert.ok(renderer.length > 100);
const point = (executed, day) => ({executed, fixtures:100, date:`2026-10-${day}T12:00:00Z`, revision:'abc'});
const native = [
  {target:'windows_x86_64', history:[point(10,'01'),point(70,'29')]},
  {target:'linux_x86_64', history:[point(15,'01'),point(71,'29')]},
  {target:'macos_arm64', history:[point(20,'01'),point(50,'29')]},
  {target:'linux_arm64', history:[]}
];
function render(target='all', narrow=false) {
  const nodes = Object.fromEntries(['history-target','history-readout','history-plot'].map(id=>[id,{value:target,innerHTML:'',textContent:'',querySelectorAll:()=>[]} ]));
  const context = vm.createContext({
    data:{native,source_base:''}, historySelection:null,
    hosts:{windows_x86_64:['Windows','x86_64'],linux_x86_64:['Linux','x86_64'],macos_arm64:['macOS','arm64'],linux_arm64:['Linux','arm64']},
    $:id=>nodes[id], matchMedia:()=>({matches:narrow}),
    escapeHTML:value=>String(value), number:value=>String(value), date:value=>value, short:value=>value
  });
  vm.runInContext(renderer+';renderHistory();',context);
  return nodes;
}
const comparison = render()['history-plot'].innerHTML;
assert.equal((comparison.match(/data-host-series=/g)||[]).length,3);
assert.ok(comparison.includes('Windows x86-64 70'));
assert.ok(comparison.includes('Linux x86-64 71'));
assert.ok(comparison.includes('macOS arm64 50'));
const labelPositions = [...comparison.matchAll(/class="series-end"[^>]* y="([\d.]+)"/g)].map(match=>Number(match[1])).sort((a,b)=>a-b);
assert.equal(labelPositions.length,3);
assert.ok(labelPositions.every((position,index)=>!index||position-labelPositions[index-1]>=26.99));
assert.equal((render('linux_x86_64')['history-plot'].innerHTML.match(/data-host-series=/g)||[]).length,1);
assert.ok(render('linux_arm64')['history-plot'].innerHTML.includes('No native golden'));
const mobile = render('all',true)['history-plot'].innerHTML;
assert.equal((mobile.match(/class="series-end"/g)||[]).length,0);
assert.equal((mobile.match(/class="mobile-series-row"/g)||[]).length,3);
console.log('Chart checks passed: shared hosts, values, end-label separation, target/empty states, mobile labels.');

const cohortRenderer=html.slice(html.indexOf('function renderCohort()'),html.indexOf('function selectView('));
function renderCohort(cohort,narrow=false){
  const nodes=Object.fromEntries(['cohort-plot','growth-plot','cohort-details','cohort-readout'].map(id=>[id,{innerHTML:'',textContent:'',querySelectorAll:()=>[]}]));
  const context=vm.createContext({data:{native:[{target:'windows_x86_64',cohort}]},hosts:{windows_x86_64:['Windows','x86_64']},$:id=>nodes[id],icon:()=>'',escapeHTML:String,number:String,matchMedia:()=>({matches:narrow})});
  vm.runInContext(cohortRenderer+';renderCohort();',context);
  return nodes;
}
const cohort={available:true,baseline_total:4,baseline_runs:2,same_now_runs:3,current_total:100,current_runs:95,recovered:1,lost:0,missing:0,missing_previously_running:0,new_running:92,baseline_revision:'abc'};
const cohortNodes=renderCohort(cohort);
assert.match(cohortNodes['cohort-plot'].innerHTML,/Same set now: 3 recorded runs of 4 fixture IDs, 75 percent/);
assert.match(cohortNodes['cohort-plot'].innerHTML,/All programs now: 95 recorded runs of 100 fixture IDs, 95 percent/);
assert.match(cohortNodes['cohort-details'].innerHTML,/All programs now: 95 \/ 100 \(95%\)/);
assert.match(cohortNodes['growth-plot'].innerHTML,/>1<\/strong> recovered/);
assert.match(cohortNodes['growth-plot'].innerHTML,/>92<\/strong> new/);
assert.match(renderCohort(cohort,true)['cohort-plot'].innerHTML,/viewBox="0 0 350 /);
assert.match(renderCohort({available:false})['cohort-plot'].innerHTML,/No baseline/);
const emptyCurrent=renderCohort({...cohort,current_total:0,current_runs:0});
assert.ok(!emptyCurrent['cohort-plot'].innerHTML.includes('null%'));
assert.ok(emptyCurrent['cohort-details'].innerHTML.includes('unavailable'));
console.log('Cohort checks passed: fixed denominator, new-program separation, mobile geometry and unavailable states.');

const viewRenderer=html.slice(html.indexOf('function selectView('),html.indexOf('function renderFacts()'));
const panels=Object.fromEntries(['overview','facts','work'].map(name=>[name,{hidden:false,dataset:{viewPanel:name}}]));
const tabs=['overview','facts','work'].map(name=>({dataset:{view:name},attrs:{},setAttribute(key,value){this.attrs[key]=String(value);},focus(){this.focused=true;}}));
const viewContext=vm.createContext({$:id=>panels[id.replace('view-','')],document:{querySelectorAll:selector=>selector==='[data-view-panel]'?Object.values(panels):tabs},renderCohort(){},renderHistory(){},renderFacts(){},renderOutcomes(){},renderClaims(){}});
vm.runInContext(viewRenderer,viewContext);
vm.runInContext("selectView('facts',true)",viewContext);
assert.equal(panels.facts.hidden,false);
assert.equal(panels.overview.hidden,true);
assert.equal(panels.work.hidden,true);
assert.equal(tabs[1].attrs['aria-selected'],'true');
assert.equal(tabs[1].focused,true);
vm.runInContext("selectView('unknown')",viewContext);
assert.equal(panels.overview.hidden,false);
console.log('View checks passed: one panel, selected tab, focus and unknown-view fallback.');

const overview=html.slice(html.indexOf('id="view-overview"'),html.indexOf('id="view-facts"'));
for(const chart of ['history','cohort','growth']){
  const section=overview.match(new RegExp('<section[^>]*id="chart-'+chart+'"[^>]*>'));
  assert.ok(section,chart+' remains in Overview');
  assert.ok(!section[0].includes('hidden'),chart+' is visible by default');
}
assert.ok(!html.includes('id="overview-chart"'));
assert.ok(!html.includes('function selectChart('));
console.log('Overview checks passed: all three charts visible, no chart-switching dropdown.');

const plainNodes=ids=>Object.fromEntries(ids.map(id=>[id,{value:'',innerHTML:'',textContent:'',querySelectorAll:()=>[]}]));
const resultNodes=plainNodes(['target','outcome-plot','outcome-metric','outcome-readout',...['good','retained','failed','unmeasured'].flatMap(key=>['bar-'+key,'bar-label-'+key])]);
for(const node of Object.values(resultNodes))node.classList={toggle(){}};
resultNodes.target.value='windows_x86_64';
const resultContext=vm.createContext({data:{facts:[{places:['good','retained','failed',...Array(6).fill('unmeasured')]}]},cellState:state=>[state],outcomeSelection:'retained',$:id=>resultNodes[id],hosts:{windows_x86_64:['Windows','x86_64']},escapeHTML:String,number:String});
vm.runInContext(html.slice(html.indexOf('function renderOutcomes()'),html.indexOf('function showDetail('))+';renderOutcomes();',resultContext);
assert.match(resultNodes['outcome-plot'].innerHTML,/Historical or qualified pass: 1 of 9 fact places/);
assert.match(resultNodes['outcome-plot'].innerHTML,/Unmeasured \/ source absent: 6 of 9 fact places/);
assert.equal((resultNodes['outcome-plot'].innerHTML.match(/class="result-row"/g)||[]).length,4);

const factNodes=plainNodes(['fact-search','fact-head','fact-body','fact-shown']);
const places=Array.from({length:9},(_,i)=>({name:'place'+i,program:true,twins:i===0?[]:['invalid']}));
const factContext=vm.createContext({data:{facts:[{name:'long_fact-name',places}],places:places.map(p=>p.name)},filter:'all',selection:null,$:id=>factNodes[id],names:{},escapeHTML:String,number:String,cellState:()=>['unmeasured','·','No record']});
vm.runInContext(html.match(/const readableName = .*;/)[0],factContext);
factNodes['fact-search'].value='long fact name';
vm.runInContext(html.slice(html.indexOf('function renderFacts()'),html.indexOf('function renderNative()'))+';renderFacts();',factContext);
assert.match(factNodes['fact-head'].innerHTML,/>Source<\/th>/);
assert.match(factNodes['fact-body'].innerHTML,/>8<small>\/9<\/small>/);
assert.match(factNodes['fact-body'].innerHTML,/>long fact name<\/button>/);
assert.ok(!factNodes['fact-body'].innerHTML.includes('source places'));
factNodes['fact-search'].value='absent';vm.runInContext('renderFacts()',factContext);
assert.match(factNodes['fact-body'].innerHTML,/colspan="11"/);

const workNodes=plainNodes(['claim-search','claim-list','claim-shown','registry','work-stats']);
const claim={item:'EXACT-OUTCOME',owner:'Agent one',note:'The current progress note.',paths:['source/psi'],ticket:'ticket1',expires_utc:'2099-01-01T00:00:00Z'};
const workData={claims:{available:true,active:[claim],expired:[{...claim,ticket:'ticket2',expires_utc:'2000-01-01T00:00:00Z'}],version:'abc'}};
const workContext=vm.createContext({data:workData,claimFilter:'active',$:id=>workNodes[id],escapeHTML:String,number:String,date:String,short:String});
vm.runInContext(html.match(/const readableName = .*;/)[0],workContext);
workNodes['claim-search'].value='exact outcome';
vm.runInContext(html.slice(html.indexOf('function claimRows()'),html.indexOf('function render(){'))+';renderClaims();',workContext);
const workHTML=workNodes['claim-list'].innerHTML;
assert.ok(workHTML.indexOf(claim.note)<workHTML.indexOf('<details>'));
assert.ok(workHTML.indexOf('source/psi')<workHTML.indexOf('<details>'));
assert.ok(workHTML.includes(claim.owner)&&workHTML.includes(claim.expires_utc));
assert.ok(workHTML.includes('EXACT-OUTCOME')&&workHTML.includes('ticket1'));
vm.runInContext("claimFilter='expired';renderClaims()",workContext);
assert.match(workNodes['claim-list'].innerHTML,/Unresolved/);
workData.claims.available=false;workData.claims.error='Registry unavailable';vm.runInContext('renderClaims()',workContext);
assert.equal((workNodes['work-stats'].innerHTML.match(/>—</g)||[]).length,2);
assert.match(workNodes['claim-list'].innerHTML,/Registry unavailable/);
console.log('Facts/Work checks passed: visible result counts, source coverage, notes/scope/owner/expiry, exact metadata, lease filters and unknown registry.');

const performanceRenderer=html.slice(html.indexOf('function renderPerformance()'),html.indexOf('function renderCohort()'));
function performanceView(records){
  const nodes=plainNodes(['performance-host','performance-age','performance-quality','performance-stats','performance-cost','performance-scaling','performance-runtime','performance-pairs','performance-footprint','performance-evidence','performance-readout']);
  const context=vm.createContext({data:{performance:{records,errors:[]},source_base:'https://github.com/test/repo/blob/head/'},$:id=>nodes[id],hosts:{windows_x86_64:['Windows','x86_64']},readableName:value=>value.replaceAll('_',' '),escapeHTML:String,number:String,matchMedia:()=>({matches:true})});
  vm.runInContext(performanceRenderer+';renderPerformance();',context);
  return nodes;
}
const performanceRecord={id:'record1',path:'tools/benchmark/records/probe.json',record_revision:'commit',recorded:'2026-01-01T00:00:00Z',context_id:'host1',context:{target:'windows_x86_64',cpu:'CPU',omega_profile:'release'},subject:{name:'probe',root:'probe/main.omg',source_revision:'source'},selection:{enabled:[],disabled:[]},host:{},project:{sloc:100,files:[{}],sha256:'digest'},compile:{median:25,min:20,max:30,samples:[20,30]},compile_ms_kloc:250,runtime:{median:10,min:9,max:11,samples:[9,11]},runtime_oracle:true,runtime_status:'measured',compile_memory:null,run_memory:1024,code_bytes:1024,code_stable:true,notes:[],measurement:null,baseline_ratio:null,baseline_id:null};
const historical=performanceView([performanceRecord]);
const setup=performanceView([{...performanceRecord,preparation:{median:5000}}]);
assert.match(setup['performance-footprint'].innerHTML,/Preparation s/);
assert.match(setup['performance-footprint'].innerHTML,/>5<\/td>/);
const latestCompile=performanceView([performanceRecord,{...performanceRecord,id:'fresh',recorded:'2026-10-29T00:00:00Z',context_id:'host2',runtime:null,runtime_oracle:false,observation_warnings:['Prime preparation timed out (20 min)']}]);
assert.equal(latestCompile['performance-host'].value,'host2');
assert.match(latestCompile['performance-age'].textContent,/Measured/);
assert.match(latestCompile['performance-quality'].textContent,/Prime preparation timed out/);
const refreshed=performanceView([{...performanceRecord,observation_warnings:['Old timeout']},{...performanceRecord,id:'new',recorded:'2026-10-30T00:00:00Z'}]);
assert.ok(!refreshed['performance-quality'].textContent.includes('Old timeout'));
assert.match(historical['performance-runtime'].innerHTML,/10 <small>ms/);
assert.match(historical['performance-runtime'].innerHTML,/Unpaired/);
assert.match(historical['performance-pairs'].textContent,/no matched baseline/);
assert.match(historical['performance-cost'].innerHTML,/250 ms per project KLOC/);
assert.match(historical['performance-footprint'].innerHTML,/>—<\/td>/);
assert.ok(!historical['performance-footprint'].innerHTML.includes('NaN'));
const paired=performanceView([{...performanceRecord,baseline_ratio:2,baseline_id:'baseline'}]);
assert.match(paired['performance-runtime'].innerHTML,/2× observed/);
const noOracle=performanceView([{...performanceRecord,runtime_oracle:false}]);
assert.match(noOracle['performance-runtime'].innerHTML,/No exit-verified/);
assert.match(noOracle['performance-footprint'].innerHTML,/Unverified exit/);
const missingSize=performanceView([{...performanceRecord,project:{sloc:null,error:'Missing source'},compile_ms_kloc:null}]);
assert.match(missingSize['performance-cost'].innerHTML,/No source-bound/);
assert.match(missingSize['performance-footprint'].innerHTML,/>0.025<\/td>/);
assert.equal(performanceView([])['performance-host'].disabled,true);
assert.ok(!performanceView([{...performanceRecord,compile_ms_kloc:.00001}])['performance-cost'].innerHTML.includes('NaN'));
console.log('Performance checks passed: real timings, project scope, pairing limits, unknown size/memory, missing oracle and empty states.');
