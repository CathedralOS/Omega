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
assert.match(cohortNodes['growth-plot'].innerHTML,/>1<\/strong> recovered/);
assert.match(cohortNodes['growth-plot'].innerHTML,/>92<\/strong> new/);
assert.match(renderCohort(cohort,true)['cohort-plot'].innerHTML,/viewBox="0 0 350 /);
assert.match(renderCohort({available:false})['cohort-plot'].innerHTML,/No baseline/);
const emptyCurrent=renderCohort({...cohort,current_total:0,current_runs:0});
assert.ok(!emptyCurrent['cohort-plot'].innerHTML.includes('null%'));
assert.ok(emptyCurrent['cohort-plot'].innerHTML.includes('unavailable'));
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
