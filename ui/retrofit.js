// Existing world geometry remains source-pinned and locked. Changes share the
// native compiler with preview and export; this UI never manufactures transforms.
export function installRetrofit(ctx) {
  const $=id=>document.getElementById(id);
  const el=(tag,text,cls)=>{const n=document.createElement(tag);if(text)n.textContent=text;if(cls)n.className=cls;return n;};
  const button=(text,run)=>{const n=el('button',text);n.type='button';n.onclick=run;return n;};
  const layers=['structure','furnishings','devices','collision','occlusion','navigation'];
  const pretty=s=>s[0].toUpperCase()+s.slice(1);
  let comparison='proposed',hidden=new Set(),chosen=new Set(),activeShell=null;
  let survey=null,draft=null,floors=[],site=null,sites=[],busy=false;
  const panel=el('section');panel.id='retrofit-panel';panel.hidden=true;
  $('project-panel').after(panel);
  const nav=button('Retrofit',()=>{document.querySelectorAll('aside>section').forEach(n=>n.hidden=n!==panel);document.querySelectorAll('aside nav button').forEach(n=>n.classList.toggle('active',n===nav));draw();});
  nav.dataset.panel='retrofit';document.querySelector('aside nav').append(nav);
  const start=button('Retrofit existing building',()=>openWizard(true));
  $('new').after(start);
  document.querySelector('header .actions').prepend(button('Start project',()=>{
    const dialog=modal('Start a building project');
    dialog.body.append(el('p','Create a new shell or adapt a building already in Night City.'),button('New building',()=>{dialog.close();$('new').click();}),button('Retrofit existing building',()=>{dialog.close();openWizard(true);}),button('Open saved project',()=>{dialog.close();document.querySelector('[data-panel="project"]').click();$('saved').focus();}));
  }));
  const compare=el('div',null,'retrofit-compare');compare.setAttribute('aria-label','Compare building states');
  const select=el('select');select.setAttribute('aria-label','Building comparison');
  for(const mode of ['existing','proposed','changes'])select.add(new Option(pretty(mode),mode));select.value='proposed';
  select.onchange=()=>{comparison=select.value;ctx.redraw();ctx.preview();};compare.append(select);
  document.querySelector('.workspace-toolbar').append(compare);
  const wholeLabel=el('label','Whole building'),whole=el('input');whole.type='checkbox';whole.id='whole-building-preview';whole.checked=true;wholeLabel.prepend(whole);compare.append(wholeLabel);whole.onchange=()=>{ctx.resetCamera();ctx.preview();};
  function modal(title) {
    const d=el('dialog',null,'retrofit-dialog'),head=el('div',null,'row'),body=el('div');head.append(el('h2',title),button('Close',()=>d.close()));d.append(head,body);document.body.append(d);d.addEventListener('close',()=>d.remove());d.showModal();return{node:d,body,close:()=>d.close()};
  }
  function input(parent,label,value,onchange,{type='number',step='0.05'}={}) {
    const l=el('label',label),i=el('input');i.type=type;i.value=value;if(type==='number')i.step=step;i.onchange=()=>onchange(type==='number'?Number(i.value):i.value);l.append(i);parent.append(l);return i;
  }
  function vector(parent,label,value) {
    const group=el('fieldset');group.append(el('legend',label));const row=el('div',null,'grid-fields');['X','Y','Z'].forEach((a,i)=>input(row,a,value[i],v=>value[i]=v));group.append(row);parent.append(group);
  }
  function errorBox(parent){const n=el('p',null,'form-error');n.setAttribute('role','alert');parent.append(n);return n;}
  async function run(control,message,fn,error) {
    if(busy)return;busy=true;control.disabled=true;error.textContent='';ctx.status(message);
    try {await fn();}catch(e){error.textContent=e.message;ctx.status(e.message,true);}finally{busy=false;control.disabled=false;}
  }
  async function openWizard(fresh=false) {
    const w=modal('Retrofit an existing building');
    const state=ctx.state();draft=structuredClone(state.project);survey=null;chosen.clear();site=null;
    if(fresh){draft=ctx.starter();draft.id='retrofit_'+Date.now().toString(36);draft.rooms=[];draft.objects=[];draft.openings={};draft.shell_rooms=[];draft.partitions=[];draft.shells=[];draft.retrofit=[];draft.stairs=[];draft.door_devices={};draft.door_states={};draft.workstations=[];draft.lights=[];draft.audio_areas=[];}
    const origin=[...draft.origin];let yaw=draft.yaw,radius=15,name='Existing building',sectorPaths=[];
    const location=el('section'),step=el('p','1 · Locate → 2 · Select shell → 3 · Define floors', 'eyebrow');location.append(step,el('p','Use world coordinates from a saved viewpoint or a surveyed location. Shell pieces keep their exact native pivots and materials.'));
    const siteSelect=el('select');siteSelect.setAttribute('aria-label','Surveyed location');siteSelect.add(new Option('Custom location / current anchor',''));
    try{sites=await(await ctx.api('/api/shell/sites')).json();sites.forEach((s,i)=>siteSelect.add(new Option(s.name,i)));}catch(e){ctx.status(e.message,true);}
    location.append(siteSelect);
    const fields=el('div');vector(fields,'World position · metres',origin);const yawField=input(fields,'World yaw · degrees',yaw,v=>yaw=v),radiusField=input(fields,'Survey radius · metres',radius,v=>radius=v);const nameField=input(fields,'Building name',name,v=>name=v,{type:'text'});
    const note=el('p',null,'muted');location.append(fields,note);
    const paste=input(location,'Paste viewpoint XYZ (comma or space separated)','',v=>{const xyz=v.match(/-?\d+(?:\.\d+)?/g)?.map(Number);if(xyz?.length!==3){paste.setCustomValidity('Enter three world coordinates');paste.reportValidity();return;}paste.setCustomValidity('');origin.splice(0,3,...xyz);fields.querySelectorAll('fieldset input').forEach((i,n)=>i.value=origin[n]);},{type:'text'});
    siteSelect.onchange=()=>{site=sites[Number(siteSelect.value)];if(siteSelect.value==='')site=null;if(!site)return;origin.splice(0,3,...site.position);yaw=site.yaw;radius=site.radius;name=site.name;sectorPaths=site.sectors;fields.querySelectorAll('fieldset input').forEach((i,n)=>i.value=origin[n]);yawField.value=yaw;radiusField.value=radius;nameField.value=name;note.textContent=site.note;};
    const err=errorBox(location);const scan=button('Find building pieces',()=>run(scan,'Reading native building pieces, collision and occlusion…',async()=>{
      draft.origin=[...origin];draft.yaw=yaw;
      if(site){const nearby=await(await ctx.api('/api/world-sectors',{position:origin,radius})).json();sectorPaths=[...new Set([...site.sectors,...nearby.sectors.filter(s=>s.category==='Navigation').map(s=>s.path)])];}
      survey=await(await ctx.api('/api/shell/survey',{position:origin,radius,sectors:sectorPaths})).json();
      floors=structuredClone(site?.floors||[{name:'Ground',level:0,position:[0,0,0],size:[6,6,4.2],yaw:0}]);
      chosen=new Set(survey.members.filter(m=>['collision','occlusion','navigation'].includes(m.layer)||(site?site.selection.some(s=>s.index===m.index&&(s.instance??null)===(m.instance??null)&&(s.actor??null)===(m.actor??null)&&(s.shape??null)===(m.shape??null)):m.layer==='structure')).map(m=>m.id));
      draft.name=name;showSelection(w,name);
    },err));location.append(scan);w.body.append(location);
  }
  function local(m) {
    const p=draft||ctx.state().project,a=-p.yaw*Math.PI/180,dx=m.position[0]-p.origin[0],dy=m.position[1]-p.origin[1];
    return[dx*Math.cos(a)-dy*Math.sin(a),dx*Math.sin(a)+dy*Math.cos(a),m.position[2]-p.origin[2]];
  }
  function corners(m) {
    const b=m.bounds;if(!b){const c=local(m);return[c[0]-.2,c[1]-.2,c[0]+.2,c[1]+.2];}
    const [x,y,z,w]=m.rotation,scale=m.scale,qv=v=>{const t=[2*(y*v[2]-z*v[1]),2*(z*v[0]-x*v[2]),2*(x*v[1]-y*v[0])];return[v[0]+w*t[0]+y*t[2]-z*t[1],v[1]+w*t[1]+z*t[0]-x*t[2],v[2]+w*t[2]+x*t[1]-y*t[0]];};
    let min=[Infinity,Infinity],max=[-Infinity,-Infinity];const p=draft||ctx.state().project,a=-p.yaw*Math.PI/180;
    for(const bx of[b.min[0],b.max[0]])for(const by of[b.min[1],b.max[1]])for(const bz of[b.min[2],b.max[2]]){const v=qv([bx*scale[0],by*scale[1],bz*scale[2]]),dx=m.position[0]+v[0]-p.origin[0],dy=m.position[1]+v[1]-p.origin[1],xy=[dx*Math.cos(a)-dy*Math.sin(a),dx*Math.sin(a)+dy*Math.cos(a)];xy.forEach((v,i)=>{min[i]=Math.min(min[i],v);max[i]=Math.max(max[i],v);});}
    return[...min,...max];
  }
  function map(members,onchange) {
    const svg=document.createElementNS('http://www.w3.org/2000/svg','svg');svg.classList.add('shell-map');svg.setAttribute('role','group');svg.setAttribute('aria-label','Building pieces, top view. Select pieces in the list or on the plan.');
    const ext=members.map(corners),low=[Math.min(...ext.map(b=>b[0]),-1),Math.min(...ext.map(b=>b[1]),-1)],high=[Math.max(...ext.map(b=>b[2]),6),Math.max(...ext.map(b=>b[3]),6)];
    svg.setAttribute('viewBox',`${low[0]-1} ${-high[1]-1} ${high[0]-low[0]+2} ${high[1]-low[1]+2}`);
    members.forEach((m,i)=>{const b=ext[i],r=document.createElementNS(svg.namespaceURI,'rect');r.setAttribute('x',b[0]);r.setAttribute('y',-b[3]);r.setAttribute('width',Math.max(.08,b[2]-b[0]));r.setAttribute('height',Math.max(.08,b[3]-b[1]));r.setAttribute('class',`shell-piece ${m.layer} ${chosen.has(m.id)?'selected':''}`);const t=document.createElementNS(svg.namespaceURI,'title');t.textContent=m.label;r.append(t);r.onclick=e=>{e.stopPropagation();chosen.has(m.id)?chosen.delete(m.id):chosen.add(m.id);onchange();};svg.append(r);});
    for(const f of floors){const r=document.createElementNS(svg.namespaceURI,'rect');r.setAttribute('x',f.position[0]);r.setAttribute('y',-f.position[1]-f.size[1]);r.setAttribute('width',f.size[0]);r.setAttribute('height',f.size[1]);r.setAttribute('transform',`rotate(${-f.yaw} ${f.position[0]} ${-f.position[1]})`);r.setAttribute('class','shell-floor');r.style.pointerEvents='none';svg.append(r);}
    return svg;
  }
  function showSelection(w,name) {
    w.body.replaceChildren();const heading=el('p','Select the building, not the whole neighbourhood. Orange = selected. Dashed outlines = editable floor boundaries.','muted');w.body.append(heading);
    let filter='structure',query='';const toolbar=el('div',null,'row'),filterSelect=el('select');filterSelect.setAttribute('aria-label','Shell layer');for(const l of layers)filterSelect.add(new Option(pretty(l),l));filterSelect.add(new Option('All layers',''));toolbar.append(filterSelect);
    const search=el('input');search.type='search';search.placeholder='Filter piece names';search.setAttribute('aria-label','Filter shell pieces');toolbar.append(search);w.body.append(toolbar);
    const columns=el('div',null,'shell-columns'),drawing=el('div'),list=el('div',null,'shell-list'),summary=el('p');columns.append(drawing,list);w.body.append(summary,columns);
    const render=()=>{const members=survey.members.filter(m=>(!filter||m.layer===filter)&&m.label.toLowerCase().includes(query));drawing.replaceChildren(map(members,render));list.replaceChildren();summary.textContent=`${chosen.size} selected (including retained technical layers) · ${survey.members.length} native pieces · ${survey.sectors_scanned} sectors · ${survey.failures.length} source errors`;
      for(const m of members){const label=el('label',null,'shell-member'),check=el('input');check.type='checkbox';check.checked=chosen.has(m.id);check.onchange=()=>{check.checked?chosen.add(m.id):chosen.delete(m.id);render();};label.append(check,el('span',`${m.label} · ${m.layer}${m.instance!==null?' · instance '+m.instance:''}${m.diagnostics.length?' · '+m.diagnostics.join('; '):''}`));list.append(label);}
    };filterSelect.onchange=()=>{filter=filterSelect.value;render();};search.oninput=()=>{query=search.value.toLowerCase();render();};
    toolbar.append(button('Select visible',()=>{survey.members.filter(m=>(!filter||m.layer===filter)&&m.label.toLowerCase().includes(query)).forEach(m=>chosen.add(m.id));render();}),button('Clear visible',()=>{survey.members.filter(m=>(!filter||m.layer===filter)&&m.label.toLowerCase().includes(query)).forEach(m=>chosen.delete(m.id));render();}));
    if(survey.failures.length){const details=el('details');details.append(el('summary','Source errors'));survey.failures.forEach(f=>details.append(el('p',`${f.sector}: ${f.error}`)));w.body.append(details);}
    const floorEditor=el('section');w.body.append(floorEditor);
    function drawFloors(){floorEditor.replaceChildren(el('h3','Interior floors'),el('p','Define usable interior space inside the retained shell. These boundaries do not add duplicate walls or floors.','muted'));floors.forEach((f,i)=>{const details=el('details');details.open=true;details.append(el('summary',f.name));input(details,'Floor name',f.name,v=>f.name=v,{type:'text'});input(details,'Floor index',f.level,v=>f.level=v,{step:'1'});vector(details,'Lower-left floor corner',f.position);vector(details,'Width / depth / clear height',f.size);input(details,'Floor yaw',f.yaw,v=>f.yaw=v);details.append(button('Remove floor',()=>{floors.splice(i,1);drawFloors();render();}));floorEditor.append(details);});floorEditor.append(button('Add floor',()=>{const previous=floors.at(-1);floors.push({name:`Floor ${floors.length+1}`,level:floors.length,position:[previous?.position[0]||0,previous?.position[1]||0,(previous?.position[2]||0)+4.2],size:[...(previous?.size||[6,6,4.2])],yaw:previous?.yaw||0});drawFloors();render();}),button('Update floor outlines',render));}
    drawFloors();render();const error=errorBox(w.body),previewImage=el('img');previewImage.className='shell-preview';previewImage.alt='Native preview of the selected shell';previewImage.hidden=true;w.body.append(previewImage);
    const payload=()=>({project:draft,name,sources:survey.sources,members:survey.members.filter(m=>chosen.has(m.id)),floors});
    const preview=button('Preview selected shell',()=>run(preview,'Preparing the selected native shell on the GPU…',async()=>{const response=await ctx.api('/api/shell/preview',{import:payload(),options:{size:768,cutaway:false,level:null}});if(previewImage.dataset.url)URL.revokeObjectURL(previewImage.dataset.url);previewImage.dataset.url=URL.createObjectURL(await response.blob());previewImage.src=previewImage.dataset.url;previewImage.hidden=false;},error));
    const commit=button('Import shell and floors',()=>run(commit,'Importing source-pinned building shell…',async()=>{if(!chosen.size)throw Error('Select the shell pieces first');if(!floors.length)throw Error('Define at least one interior floor');const result=await(await ctx.api('/api/shell/import',payload())).json();ctx.accept(result);activeShell=result.project.shells.at(-1).id;draft=null;chosen.clear();w.close();nav.click();ctx.status('Existing shell imported · retained geometry will not be duplicated in export');},error));commit.className='primary';w.body.append(preview,commit);
  }
  function draw() {
    const {project}=ctx.state();if(!project)return;const shells=project.shells||[];compare.hidden=!shells.length;
    panel.replaceChildren(el('h2','Existing building'),button('Import another shell',()=>openWizard(false)));
    if(!shells.length){panel.append(el('p','Import a measured building shell, then furnish it and plan changes with the normal editor.'));return;}
    if(!shells.some(s=>s.id===activeShell))activeShell=shells[0].id;const shell=shells.find(s=>s.id===activeShell);const shellSelect=el('select');shellSelect.setAttribute('aria-label','Imported building');shells.forEach(s=>shellSelect.add(new Option(s.name,s.id)));shellSelect.value=activeShell;shellSelect.onchange=()=>{activeShell=shellSelect.value;chosen.clear();draw();};panel.append(shellSelect);
    const layersBox=el('details');layersBox.append(el('summary','Visible layers'));layersBox.open=true;for(const layer of layers){const label=el('label',pretty(layer)),check=el('input');check.type='checkbox';check.checked=!hidden.has(layer);check.onchange=()=>{check.checked?hidden.delete(layer):hidden.add(layer);ctx.redraw();ctx.preview();};label.prepend(check);layersBox.append(label);}panel.append(layersBox);
    const counts=el('p',`${shell.members.filter(m=>!m.removed).length} retained pieces · ${shell.operations.length} changes · ${shell.floors.length} floors`,'muted');panel.append(counts);
    panel.append(button('Edit interior floor boundaries',()=>editFloors(shell)));
    const controls=el('div',null,'retrofit-actions');for(const [kind,label]of[['entrance','Create entrance'],['replace_wall','Replace wall'],['remove','Remove obstruction'],['floor_connection','Add floor connection']])controls.append(button(label,()=>operationWizard(shell,kind)));panel.append(controls);
    const details=el('details');details.open=true;details.append(el('summary','Select existing pieces'));const list=el('div',null,'shell-list');
    for(const layer of layers){
      const members=shell.members.filter(m=>m.layer===layer);if(!members.length)continue;
      const group=el('details');group.open=layer==='structure';group.append(el('summary',`${pretty(layer)} · ${members.length}`));
      for(const m of members){
        const label=el('label',null,'shell-member'),check=el('input');check.type='checkbox';check.checked=chosen.has(m.id);
        check.disabled=m.removed&&m.layer!=='navigation';
        check.onchange=()=>{check.checked?chosen.add(m.id):chosen.delete(m.id);};label.append(check,el('span',m.label+(m.removed?' · changed':'')));group.append(label);
      }
      list.append(group);
    }
    details.append(list);panel.append(details);
    const changes=el('section');changes.append(el('h3','Planned changes'));if(!shell.operations.length)changes.append(el('p','No changes to the existing shell. New objects and interiors use the normal Build and Library panels.','muted'));for(const op of shell.operations){const row=el('div');row.append(el('strong',op.label),el('p',`${op.members.length} source pieces · ${op.kind.replaceAll('_',' ')}`),button('Restore original',async()=>{const error=errorBox(row);const b=row.querySelector('button');await run(b,'Restoring original geometry…',async()=>ctx.accept(await(await ctx.api('/api/shell/operation',{project:ctx.state().project,shell:shell.id,operation:op,restore:true})).json()),error);}));changes.append(row);}panel.append(changes);
    const sources=el('details');sources.append(el('summary','Source pins and export review'));sources.append(el('p','Source hashes and node/instance counts are validated on compilation and export. Retained references are preview-only. Additions and replacement geometry are packed with exact source removals.'));for(const pin of project.retrofit)sources.append(el('p',`${pin.path} · ${pin.expectedNodes} placements · ${pin.nodeDeletions.length} changed placements`));panel.append(sources);
    const masks=shell.operations.flatMap(op=>op.navigation_masks||[]);if(masks.length){const tiles=new Set(masks.map(m=>`${m.sector}:${m.placement_index}:${m.resource_index}`));panel.append(el('p',`${masks.length} navigation regions across ${tiles.size} native tiles. Restoring a change keeps the other regions.`));}
    const compiled=ctx.state().compiled;
    const review=el('section');review.append(el('h3','Export changes'),el('p',`${compiled.placements.filter(p=>!p.reference_only).length} new or replacement visual pieces · ${project.retrofit.reduce((n,p)=>n+p.nodeDeletions.length,0)} source placement patches.`));
    for(const op of shell.operations){const changed=op.members.map(id=>shell.members.find(m=>m.id===id));const affected=new Set(changed.map(m=>m.layer));if(op.kind!=='remove'&&!affected.has('collision'))review.append(el('p',op.label+': no source collision selected. Confirm there is no collider blocking the opening.','form-error'));if(op.kind==='entrance'&&!affected.has('occlusion'))review.append(el('p',op.label+': no occluder selected. Inspect the source occlusion layer before installation.','muted'));}
    panel.append(review);
    // Make retained references selectable in the same plan as authored additions.
    for(const member of shell.members){if(!member.object)continue;const nodes=[...$('plan').querySelectorAll('[data-object]')].filter(n=>n.dataset.object===member.object);for(const node of nodes){node.style.display=hidden.has(member.layer)||(comparison==='proposed'&&member.removed)||(comparison==='changes'&&!member.removed)?'none':'';node.style.stroke=member.removed?'#ec8282':chosen.has(member.id)?'#ffc079':'#82aeb3';node.onclick=e=>{e.stopPropagation();chosen.has(member.id)?chosen.delete(member.id):chosen.add(member.id);activeShell=shell.id;nav.click();ctx.redraw();};}}
  }
  function editFloors(shell) {
    const w=modal('Interior floor boundaries'),next=structuredClone(shell.floors);
    const rows=el('div');w.body.append(rows);
    function render(){rows.replaceChildren();next.forEach((f,i)=>{const section=el('section');input(section,'Floor name',f.name,v=>f.name=v,{type:'text'});input(section,'Floor index',f.level,v=>f.level=v,{step:'1'});vector(section,'Local lower-left corner',f.position);vector(section,'Width / depth / clear height',f.size);input(section,'Yaw',f.yaw,v=>f.yaw=v);section.append(button('Remove floor',()=>{next.splice(i,1);render();}));rows.append(section);});}
    render();w.body.append(button('Add floor',()=>{const f=structuredClone(next.at(-1)||{position:[0,0,-4.2],size:[6,6,4.2],yaw:0});f.name='Floor '+next.length;f.level=next.length;f.position[2]+=4.2;next.push(f);render();}));
    const error=errorBox(w.body),apply=button('Apply floor boundaries',()=>run(apply,'Updating retained interior boundaries…',async()=>{await ctx.edit(p=>{p.shells.find(s=>s.id===shell.id).floors=next;p.shell_rooms=p.shell_rooms.filter(r=>!r.id.startsWith(shell.id+'_floor_'));for(const f of next)p.shell_rooms.push({id:shell.id+'_floor_'+f.level,position:f.position,size:f.size,yaw:f.yaw,level:f.level,retained_sides:['north','south','east','west','floor','ceiling'],openings:{}});});w.close();},error));w.body.append(apply);
  }
  function operationWizard(shell,kind) {
    const members=shell.members.filter(m=>chosen.has(m.id)&&!m.removed);if(!members.length){ctx.status('Select the existing pieces affected by the change first',true);return;}
    draft=null;const w=modal({entrance:'Create an entrance',replace_wall:'Replace a wall',remove:'Remove an obstruction',floor_connection:'Connect floors'}[kind]);
    const first=members.find(m=>m.layer==='structure')||members[0],b=corners(first),floor=shell.floors.find(f=>f.level===Number($('level').value)),position=[(b[0]+b[2])/2,(b[1]+b[3])/2,floor?.position[2]??local(first)[2]];
    const op={id:'change_'+Date.now().toString(36),label:w.node.querySelector('h2').textContent,kind,members:members.map(m=>m.id),position,size:kind==='floor_connection'?[2.6,3,0.6]:[1.5,1,2.5],yaw:0,preset:'hinged_e',diagnostics:[],stair:null};
    w.body.append(el('p','The selected source pieces are replaced only where the change volume intersects. Review collision and occlusion separately; neighbouring source actors and instances remain.','muted'));
    const affected=el('div');members.forEach(m=>affected.append(el('p',`${m.layer}: ${m.label}`)));w.body.append(affected);
    input(w.body,'Change name',op.label,v=>op.label=v,{type:'text'});vector(w.body,'Opening bottom centre · local metres',op.position);vector(w.body,'Width / depth / height',op.size);const yaw=input(w.body,'Opening yaw',op.yaw,v=>op.yaw=v);
    w.body.append(button('Align opening to selected surface',()=>{const q=first.rotation,[x,y,z,ww]=q;op.yaw=Math.atan2(2*(ww*z+x*y),1-2*(y*y+z*z))*180/Math.PI-ctx.state().project.yaw;const dim=first.bounds?.dimensions;if(dim&&dim[0]<dim[1])op.yaw+=90;op.yaw=(op.yaw+360)%360;yaw.value=op.yaw;}));
    if(kind==='entrance'){const l=el('label','Native door'),s=el('select');for(const [key,name]of[['hinged_e','Hinged door E'],['hinged_h','Hinged door H'],['sliding_entropy_c','Sliding door'],['garage_w300_h250','Garage shutter']])s.add(new Option(name,key));s.onchange=()=>op.preset=s.value;l.append(s);w.body.append(l);}
    if(kind==='floor_connection'){op.stair={id:op.id,x:position[0]-1.3,y:position[1]-1.5,width:2.6,run:3,level:Number($('level').value),landing:1.2,reverse:false,railings:true,axis:'y',type:'return',flight_width:1.2};input(w.body,'Stair X',op.stair.x,v=>op.stair.x=v);input(w.body,'Stair Y',op.stair.y,v=>op.stair.y=v);input(w.body,'Lower floor index',op.stair.level,v=>op.stair.level=v,{step:'1'});w.body.append(el('p','Return flight: 2.6 m wide, 3 m run, 1.2 m landings. Both floor boundaries must contain the shaft and landings.','muted'));}
    const related=shell.members.filter(m=>!m.removed&&!op.members.includes(m.id)&&['collision','occlusion','navigation'].includes(m.layer)).sort((a,b)=>{const distance=m=>{const c=local(m);return Math.hypot(c[0]-op.position[0],c[1]-op.position[1],c[2]-op.position[2]);};return distance(a)-distance(b);});const checks=el('details');checks.open=true;checks.append(el('summary','Related technical layers · nearest first'));for(const m of related){const label=el('label',null,'shell-member'),c=el('input');c.type='checkbox';c.onchange=()=>c.checked?op.members.push(m.id):op.members=op.members.filter(id=>id!==m.id);label.append(c,el('span',`${m.layer}: ${m.label} · local ${local(m).map(v=>v.toFixed(2)).join(', ')} m · source ${m.index}${m.kind.includes('Occluder')?' · entire occluder group':''}`));checks.append(label);}w.body.append(checks);
    const error=errorBox(w.body),apply=button('Apply change',()=>run(apply,'Compiling native replacement geometry and collision…',async()=>{ctx.accept(await(await ctx.api('/api/shell/operation',{project:ctx.state().project,shell:shell.id,operation:op})).json());chosen.clear();w.close();ctx.status('Change applied · compare Existing, Proposed and Changes');},error));apply.className='primary';w.body.append(apply);
  }
  return{draw,allFloors:()=>!!ctx.state().project?.shells?.length&&whole.checked,options:()=>({comparison,hidden_layers:[...hidden]})};
}
