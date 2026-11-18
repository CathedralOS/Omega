// The graph has one identity per component. D3 only changes the camera;
// selection highlights the same nodes and connections in the static SVG.
(() => {
  const root=document.querySelector('#compiler-overview');
  if(!root || !window.AtlasZoom)return;
  const {select,zoom,zoomIdentity}=window.AtlasZoom;
  const models={current:JSON.parse(root.querySelector('#overview-model').textContent),proposed:JSON.parse(root.querySelector('#overview-proposed-model').textContent)};
  let view='current',model=models.current;
  let nodes=new Map(model.nodes.map(n=>[n.id,n]));
  let edges=new Map(model.edges.map(e=>[e.id,e]));
  const viewport=root.querySelector('.connected-viewport');
  const svg=root.querySelector('.arch-overview-svg');
  let scene=root.querySelector('.connected-scene');
  const scenes={current:scene.cloneNode(true),proposed:root.querySelector('#overview-proposed-scene').content.querySelector('.connected-scene').cloneNode(true)};
  const cameras=new Map();
  const detail=root.querySelector('.connected-detail');
  const scale=root.querySelector('.connected-scale');
  const traceSelect=root.querySelector('.trace-picker select');
  const traceNote=root.querySelector('.trace-explanation');
  const jump=root.querySelector('.connected-jump');
  const mini=root.querySelector('.connected-minimap svg');
  const NS='http://www.w3.org/2000/svg';
  let current=zoomIdentity,selected=null,expanded=false,selectionTrigger=null;
  const el=(tag,attributes,parent=mini)=>{const node=document.createElementNS(NS,tag);for(const [key,value] of Object.entries(attributes))node.setAttribute(key,value);parent.append(node);return node;};
  const stagePicker=root.querySelector('.stage-picker');
  const stageSelect=stagePicker.querySelector('select');
  let flight=0;
  function trackStage(transform){
    if(view!=='proposed')return;
    const top=(Math.min(80,viewport.clientHeight/5)-transform.y)/transform.k;
    const center=(viewport.clientWidth/2-transform.x)/transform.k;
    const groups=model.groups;
    const closest=groups.reduce((a,b)=>Math.abs(center-a.x-a.width/2)<=Math.abs(center-b.x-b.width/2)?a:b);
    const column=groups.filter(group=>group.x===closest.x);
    const stage=column.find(group=>top<=group.y+group.height)||column[column.length-1];
    stageSelect.selectedIndex=groups.indexOf(stage);
  }
  let miniWindow;
  function drawMinimap(){
    mini.replaceChildren();mini.setAttribute('viewBox',`0 0 ${model.width} ${model.height}`);
    for(const e of model.edges)for(const s of e.sections||[])el('path',{d:'M'+s.points.map(p=>`${p.x} ${p.y}`).join(' L')});
    for(const n of model.nodes)if(n.role!=='transform')el('rect',{x:n.x,y:n.y,width:n.width,height:n.height,class:n.kind==='native'?'mini-native':'mini-node'});
    miniWindow=el('rect',{class:'mini-window'});
  }
  drawMinimap();
  root.classList.add('connected-enhanced');
  svg.setAttribute('role','group');
  root.querySelector('.connected-tools').hidden=false;
  root.querySelector('.camera-tools').hidden=false;
  root.querySelector('.connected-views').hidden=false;
  root.querySelector('.connected-minimap').hidden=false;
  svg.removeAttribute('viewBox');
  svg.setAttribute('width','100%');svg.setAttribute('height','100%');
  const camera=zoom().scaleExtent([0.02,2.5]).extent(()=>[[0,0],[viewport.clientWidth,viewport.clientHeight]])
    .clickDistance(3)
    .on('start',event=>{if(event.sourceEvent)stopFlight();})
    .on('zoom',event=>{
      current=event.transform;trackStage(current);scene.setAttribute('transform',current.toString());
      scale.value=Math.round(current.k*100)+'%';root.dataset.scale=current.k;
      root.dataset.camera=`${current.x},${current.y},${current.k}`;
      root.dataset.whole=String(current.x>=-1 && current.y>=-1 &&
        current.x+model.width*current.k<=viewport.clientWidth+1 && current.y+model.height*current.k<=viewport.clientHeight+1);
      miniWindow.setAttribute('x',-current.x/current.k);miniWindow.setAttribute('y',-current.y/current.k);
      miniWindow.setAttribute('width',viewport.clientWidth/current.k);miniWindow.setAttribute('height',viewport.clientHeight/current.k);
    });
  select(svg).call(camera).on('dblclick.zoom',null);
  function move(x,y,k){select(svg).call(camera.transform,zoomIdentity.translate(x,y).scale(k));}
  function stopFlight(){cancelAnimationFrame(flight);}
  function stagePosition(node){
    const target=node||model.nodes[0];
    return {x:Math.max(12,(viewport.clientWidth-target.width)/2)-target.x,
            y:node?22-target.y:0,k:1};
  }
  function visitStage(id){
    stopFlight();
    const from=current,to=stagePosition(nodes.get(id));
    const draw=t=>move(from.x+(to.x-from.x)*t,from.y+(to.y-from.y)*t,from.k+(to.k-from.k)*t);
    if(matchMedia('(prefers-reduced-motion: reduce)').matches){draw(1);return;}
    const start=performance.now();
    function step(now){
      const t=Math.min(1,(now-start)/300);
      draw(1-(1-t)**3);
      if(t<1)flight=requestAnimationFrame(step);
    }
    flight=requestAnimationFrame(step);
  }
  stageSelect.addEventListener('change',()=>{clear();visitStage(stageSelect.value);});
  function fit(bounds={x:0,y:0,width:model.width,height:model.height}){
    const k=Math.min(1.2,(viewport.clientWidth-20)/bounds.width,(viewport.clientHeight-20)/bounds.height);
    move((viewport.clientWidth-bounds.width*k)/2-bounds.x*k,(viewport.clientHeight-bounds.height*k)/2-bounds.y*k,k);
  }
  function focusNode(n){const k=Math.max(0.75,Math.min(1,current.k));move(viewport.clientWidth/2-(n.x+n.width/2)*k,viewport.clientHeight/2-(n.y+n.height/2)*k,k);}
  function frameConnections(n){
    const related=new Set([n.id]);
    for(const e of model.edges)if(e.start===n.id||e.end===n.id){related.add(e.start);related.add(e.end);}
    const items=[...related].map(id=>nodes.get(id));
    const x=Math.min(...items.map(n=>n.x)),y=Math.min(...items.map(n=>n.y));
    const bounds={x,y,width:Math.max(...items.map(n=>n.x+n.width))-x,height:Math.max(...items.map(n=>n.y+n.height))-y};
    const readable=Math.min((viewport.clientWidth-64)/bounds.width,(viewport.clientHeight-64)/bounds.height)>=0.6;
    if(readable)fit(bounds);else focusNode(n);
  }
  function link(text,href,parent){const a=document.createElement('a');a.textContent=text;a.href=href;parent.append(a);return a;}
  function button(text,action,parent=detail){const b=document.createElement('button');b.type='button';b.textContent=text;b.addEventListener('click',action);parent.append(b);return b;}
  function clear(){traceNote.hidden=true;traceSelect.value='';selected=null;root.classList.remove('has-trace');root.querySelectorAll('.is-traced,.is-selected').forEach(n=>n.classList.remove('is-traced','is-selected'));detail.hidden=true;root.dataset.selection='';jump.value='';}
  function traceDetour(id,frame=true){
    stopFlight();clear();
    const trace=models.current.traces.find(t=>t.id===id);
    if(!trace)return;
    traceSelect.value=id;root.classList.add('has-trace');root.dataset.selection=id;
    if(view==='proposed'){
      const repair=model.repairs[id];
      traceNote.replaceChildren(repair.description+' ');
      link('Repair plan',repair.source,traceNote);
      if(repair.stops.length){
        const stops=document.createElement('span');
        stops.className='repair-stops';traceNote.append(stops);
        for(const key of repair.stops){
          scene.querySelector(`[data-endpoint="${key}"]`).classList.add('is-traced');
          button(nodes.get(key).title,()=>visitStage(key),stops);
        }
        const first=model.spine.indexOf(repair.stops[0]),last=model.spine.indexOf(repair.stops.at(-1));
        for(const edge of model.edges.slice(first,last)){
          scene.querySelector(`[data-edge="${edge.id}"]`).classList.add('is-traced');
        }
        if(frame){
          const t=stagePosition(nodes.get(repair.stops[0]));move(t.x,t.y,t.k);
        }
      }else root.classList.remove('has-trace');
      traceNote.hidden=false;
      return;
    }
    const points=[];
    for(const edgeId of trace.edges){
      const e=edges.get(edgeId);
      scene.querySelector(`[data-edge="${e.id}"]`).classList.add('is-traced');
      for(const key of [e.start,e.end]){
        scene.querySelector(`[data-endpoint="${key}"]`).classList.add('is-traced');
        const n=nodes.get(key);points.push(n,{x:n.x+n.width,y:n.y+n.height});
      }
      for(const section of e.sections)points.push(...section.points);
    }
    traceNote.replaceChildren(trace.description+' ');
    link('Repair plan','foundation-backend-plan.html#start-with-the-extra-arrows',traceNote);
    for(const [i,edgeId] of trace.edges.entries()){
      traceNote.append(' · ');link(trace.edges.length===1?'Source':'Source '+(i+1),edges.get(edgeId).source,traceNote);
    }
    traceNote.hidden=false;
    const x=Math.min(...points.map(p=>p.x)),y=Math.min(...points.map(p=>p.y));
    if(frame)fit({x:x-24,y:y-32,width:Math.max(...points.map(p=>p.x))-x+48,height:Math.max(...points.map(p=>p.y))-y+64});
  }
  traceSelect.addEventListener('change',()=>traceDetour(traceSelect.value));
  function inspect(id,isEdge=false,trigger=null){
    const comparing=traceSelect.value;
    stopFlight();clear();
    if(comparing)traceDetour(comparing,false);
    selected={id,isEdge};selectionTrigger=trigger||document.activeElement;
    jump.value=isEdge?'':id;
    root.dataset.selection=id;root.classList.add('has-trace');
    const chosen=isEdge?edges.get(id):nodes.get(id);
    const adjacent=isEdge?[chosen]:model.edges.filter(e=>e.start===id||e.end===id);
    const activeNodes=new Set(isEdge?[chosen.start,chosen.end]:[id]);
    for(const edge of adjacent){activeNodes.add(edge.start);activeNodes.add(edge.end);root.querySelector(`[data-edge="${edge.id}"]`)?.classList.add('is-traced');}
    for(const nodeId of activeNodes)root.querySelector(`[data-endpoint="${nodeId}"]`)?.classList.add('is-traced');
    root.querySelector(isEdge?`[data-edge="${id}"]`:`[data-endpoint="${id}"]`)?.classList.add('is-selected');
    detail.replaceChildren();detail.hidden=false;
    button('Close ×',()=>{traceDetour(comparing,false);selectionTrigger?.focus({preventScroll:true});}).className='connected-detail-close';
    const heading=document.createElement('h3');heading.textContent=isEdge?chosen.label||`${nodes.get(chosen.start).title} → ${nodes.get(chosen.end).title}`:chosen.title;heading.tabIndex=-1;detail.append(heading);
    if(!isEdge){
      const p=document.createElement('p');p.textContent=chosen.description;detail.append(p);
      if(chosen.gate){const gate=document.createElement('p');gate.className='connected-open-contract';gate.textContent='Open contract: '+chosen.gate;detail.append(gate);}
      if(chosen.evidence){
        const sources=document.createElement('p');sources.textContent='Pinned compiler source';
        for(const [label,url] of chosen.evidence){sources.append(' · ');link(label,url,sources);}
        detail.append(sources);
      }
      link(view==='proposed'?'Read proposal':'Read subsystem',`${chosen.page}.html${chosen.anchor?'#'+chosen.anchor:''}`,detail);detail.append(' · ');
    }
    link(view==='proposed'?'Recorded plan':'Pinned source',chosen.source,detail);
    const list=document.createElement('ul');list.className='connected-relations';detail.append(list);
    for(const edge of adjacent){
      const row=document.createElement('li');list.append(row);
      const start=nodes.get(edge.start),end=nodes.get(edge.end);
      button(start.title,()=>{focusNode(start);inspect(start.id,false,trigger);},row);
      row.append(' → ');
      button(end.title,()=>{focusNode(end);inspect(end.id,false,trigger);},row);
      const caption=document.createElement('p');caption.textContent=edge.label;row.append(caption);link(view==='proposed'?'Plan basis':'Relationship source',edge.source,row);
    }
    heading.focus({preventScroll:true});
  }
  function bindScene(){
  scene.querySelectorAll('[data-endpoint]').forEach(node=>node.addEventListener('click',event=>{event.preventDefault();inspect(node.dataset.endpoint,false,node);}));
  scene.querySelectorAll('[data-edge]').forEach(edge=>{
    edge.addEventListener('click',()=>inspect(edge.dataset.edge,true,edge));
    edge.addEventListener('keydown',event=>{if(event.key==='Enter'||event.key===' '){event.preventDefault();inspect(edge.dataset.edge,true,edge);}});
  });
  }
  bindScene();
  function showViewStatus(){
    root.dataset.view=view;
    stagePicker.hidden=view!=='proposed';
    const help='Drag to pan · scroll or pinch to zoom · select an arrow to inspect its handoff.';
    root.querySelector('.connected-help').textContent=help;
    viewport.setAttribute('aria-label',help+' Arrow keys pan. F fits all.');
    root.querySelectorAll('.connected-views [data-view]').forEach(button=>button.setAttribute('aria-pressed',String(button.dataset.view===view)));
    root.querySelector('.connected-intro').textContent=model.intro;
    root.closest('.architecture').querySelector('.arch-badge').textContent=model.badge;
    root.querySelectorAll('[data-legend]').forEach(legend=>{legend.hidden=legend.dataset.legend!==view;});
    root.querySelector('.arch-arrow-evidence').hidden=view!=='current';
    root.querySelector('.arch-component-list').hidden=view!=='current';
    root.querySelector('.proposed-contracts').hidden=view!=='proposed';
    root.querySelector('.connected-minimap span').textContent=view==='proposed'?'Pipeline':'Whole compiler';
    svg.setAttribute('aria-label',view==='proposed'?'Proposed architecture: unverified planning contracts':'Current compiler architecture');
  }
  function comparisonTrace(){
    if(traceSelect.value || view!=='current' || !selected)return traceSelect.value;
    if(selected.isEdge)return edges.get(selected.id).detour||'';
    const related=model.traces.filter(trace=>trace.edges.some(id=>{
      const edge=edges.get(id);
      return edge.start===selected.id || edge.end===selected.id;
    }));
    return related.length===1?related[0].id:'';
  }
  function switchView(next){
    if(next===view)return;
    stopFlight();
    const trace=comparisonTrace();
    cameras.set(view+':'+trace,{transform:current,width:viewport.clientWidth});
    clear();view=next;model=models[view];
    nodes=new Map(model.nodes.map(n=>[n.id,n]));edges=new Map(model.edges.map(e=>[e.id,e]));
    const replacement=scenes[view].cloneNode(true);scene.replaceWith(replacement);scene=replacement;
    jump.replaceChildren();const placeholder=document.createElement('option');placeholder.value='';placeholder.textContent='Choose a component…';jump.append(placeholder);
    for(const n of model.nodes)if(n.role!=='transform'){const option=document.createElement('option');option.value=n.id;option.textContent=n.title;jump.append(option);}
    bindScene();drawMinimap();showViewStatus();
    const saved=cameras.get(view+':'+trace);
    if(saved){
      const t=responsiveCamera(saved.transform,saved.width);move(t.x,t.y,t.k);
    }else start();
    if(trace)traceDetour(trace,!saved);
  }
  root.querySelectorAll('.connected-views [data-view]').forEach(button=>button.addEventListener('click',()=>switchView(button.dataset.view)));
  showViewStatus();
  svg.addEventListener('click',event=>{if(!event.target.closest('.overview-node,.overview-edge'))clear();});
  jump.addEventListener('change',()=>{if(jump.value){const n=nodes.get(jump.value);frameConnections(n);inspect(n.id,false,jump);}});
  root.querySelectorAll('[data-camera]').forEach(control=>control.addEventListener('click',()=>{
    stopFlight();const action=control.dataset.camera;
    if(action==='start'){clear();start();}
    if(action==='fit'){clear();fit();}
    if(action==='in'||action==='out')select(svg).call(camera.scaleBy,action==='in'?1.3:1/1.3);
    if(action==='natural'){const cx=(viewport.clientWidth/2-current.x)/current.k,cy=(viewport.clientHeight/2-current.y)/current.k;move(viewport.clientWidth/2-cx,viewport.clientHeight/2-cy,1);}
    if(action==='clear')clear();
    if(action==='expand'){
      const fitted=root.dataset.whole==='true';
      expanded=!expanded;root.closest('.architecture').classList.toggle('is-expanded',expanded);
      control.textContent=expanded?'Collapse':'Expand';
      if(fitted)fit();
    }
  }));
  viewport.addEventListener('keydown',event=>{
    if(event.target!==viewport)return;
    if(event.ctrlKey||event.metaKey||event.altKey)return;
    if(['ArrowLeft','ArrowRight','ArrowUp','ArrowDown','+','-','=','f','F','Escape'].includes(event.key))event.preventDefault();
    if(event.key==='f'||event.key==='F')fit();
    if(event.key==='Escape')clear();
    if(event.key==='+'||event.key==='=')select(svg).call(camera.scaleBy,1.3);
    if(event.key==='-')select(svg).call(camera.scaleBy,1/1.3);
    const delta={ArrowLeft:[80,0],ArrowRight:[-80,0],ArrowUp:[0,80],ArrowDown:[0,-80]}[event.key];
    if(delta)move(current.x+delta[0],current.y+delta[1],current.k);
  });
  document.addEventListener('keydown',event=>{if(event.key==='Escape'&&expanded){expanded=false;root.closest('.architecture').classList.remove('is-expanded');root.querySelector('[data-camera="expand"]').textContent='Expand';}});
  mini.addEventListener('click',event=>{const point=mini.createSVGPoint();point.x=event.clientX;point.y=event.clientY;const p=point.matrixTransform(mini.getScreenCTM().inverse());move(viewport.clientWidth/2-p.x*current.k,viewport.clientHeight/2-p.y*current.k,current.k);});
  function responsiveCamera(transform,previousWidth){
    const k=viewport.clientWidth<previousWidth&&transform.k<=0.9?Math.min(transform.k,(viewport.clientWidth-24)/520):transform.k;
    const ratio=k/transform.k;return{x:transform.x*ratio,y:transform.y*ratio,k};
  }
  let cameraWidth=viewport.clientWidth;
  new ResizeObserver(()=>{
    svg.setAttribute('width',viewport.clientWidth);svg.setAttribute('height',viewport.clientHeight);
    const t=responsiveCamera(current,cameraWidth);cameraWidth=viewport.clientWidth;move(t.x,t.y,t.k);
  }).observe(viewport);
  function start(){
    if(view==='proposed'){
      const t=stagePosition(null);move(t.x,t.y,t.k);
      return;
    }
    const first=nodes.get(model.spine[0]);
    move(Math.min(0,(viewport.clientWidth-first.width)/2-first.x),0,1);
  }
  start();
  if(location.hash==='#detours')root.querySelector('[data-camera="expand"]').click();
  function openHandlerHandoff(){
    const binding=location.hash==='#proposed-handler-binding';
    if(!binding&&location.hash!=='#proposed-handlers')return;
    if(!expanded)root.querySelector('[data-camera="expand"]').click();
    switchView('proposed');traceDetour('handlers',!binding);
    if(binding)visitStage('binding-summarize');
  }
  window.addEventListener('hashchange',openHandlerHandoff);
  openHandlerHandoff();
  if(location.hash==='#proposed')switchView('proposed');
})();
