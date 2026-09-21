'use strict';
let audioKeyIndex = 0, audioInspectMs = 0, audioPointerEditing = false;
const spatialKeys = ['azimuth','elevation','distanceM'];

function editingPose(d = audioDesign) {
  return d.motion === 'path' ? d.keyframes[Math.min(audioKeyIndex, d.keyframes.length - 1)] : d;
}

function spatialEditorMarkup(d) {
  audioKeyIndex = Math.max(0, Math.min(audioKeyIndex, (d.keyframes?.length || 1) - 1));
  audioInspectMs = Math.min(audioInspectMs, d.durationMs);
  const p = editingPose(d);
  return `<section class="au-position"><div class="aw-row aw-between"><strong>音源位置</strong><label class="au-enable"><input id="audio-spatial" type="checkbox" ${d.spatial?'checked':''}> 立体音響</label></div>
    <div class="au-presetbar"><label>定位プリセット <select id="spatial-preset" aria-label="定位プリセット"><option value="">選択してください</option>${Object.entries(Spatial.presets).map(([id,p])=>`<option value="${id}">${p.name}</option>`).join('')}</select></label></div>
    <div ${d.spatial?'':'inert'} class="au-spatialbody ${d.spatial?'':'au-disabled'}"><div class="au-mapwrap">
      <div id="audio-map-control" tabindex="0" role="group" aria-label="音源位置。ドラッグで移動、左右キーで方位、上下キーで距離を調整"><svg id="audio-map" viewBox="0 0 400 400" role="img" aria-label="音源と経路"></svg></div>
      <div class="aw-sub au-center">前 0° · 右 ＋90°</div>
      <svg id="audio-elevation-map" viewBox="0 0 400 100" role="img" aria-label="経路の高さ"></svg>
      <div class="au-scrub"><label for="audio-inspect">経路の確認 <output id="audio-inspect-value"></output></label><input id="audio-inspect" type="range" min="0" max="${d.durationMs}" step="any" value="${audioInspectMs}" aria-label="経路の確認時刻"><div id="audio-position-readout" class="aw-sub"></div></div>
    </div><div class="au-spatialcontrols"><label class="au-motion-label">移動<select id="audio-motion" aria-label="音源の移動">${[['fixed','固定'],['orbit','周回'],['sweep','指定位置まで移動'],['path','経路を編集']].map(([id,name])=>`<option value="${id}" ${d.motion===id?'selected':''}>${name}</option>`).join('')}</select></label>
      ${d.motion==='path'?`<div class="au-path-editor"><div class="aw-row aw-between"><strong>通過点</strong><button id="audio-add-key" ${d.keyframes.length>=32?'disabled':''}>＋ 追加</button></div><div id="audio-key-list">${d.keyframes.map((k,i)=>`<button data-spatial-key="${i}" aria-pressed="${i===audioKeyIndex}" title="${(k.at*d.durationMs).toFixed(1)} ms">${i===0?'開始':i===d.keyframes.length-1?'終了':i}</button>`).join('')}</div><div class="aw-row"><label>時刻 <input id="audio-key-time" aria-label="通過点の時刻" type="number" step="any" value="${+(p.at*d.durationMs).toFixed(3)}" ${audioKeyIndex===0||audioKeyIndex===d.keyframes.length-1?'disabled':''}> ms</label><button id="audio-remove-key" ${audioKeyIndex===0||audioKeyIndex===d.keyframes.length-1?'disabled':''}>削除</button></div><span class="aw-sub">選択した点の位置を調整</span></div>`:''}
      ${audioRange('方位','azimuth',p.azimuth,-180,180,'any','°')}${audioRange('仰角','elevation',p.elevation,-40,90,'any','°')}${audioRange('距離','distanceM',p.distanceM,.3,5,'any','m')}
      ${d.motion==='orbit'?audioRange('回転速度','speed',d.speed,-360,360,1,'°/s'):d.motion==='sweep'?audioRange('移動先','endAzimuth',d.endAzimuth,-180,180,1,'°'):''}
    </div></div><p id="audio-spatial-note" class="aw-sub">${d.spatial?'ステレオ素材は左右を合成し、1つの音源として定位させます。':'立体音響オフ · 元の左右を保持して再生します。'}</p><p class="aw-sub">距離は1 mより遠い範囲で音量に反映 · HRTF: MIT KEMAR / Bill Gardner, Keith Martin</p></section>`;
}

function setAudioParameter(key, value) {
  if (spatialKeys.includes(key)) editingPose()[key] = value;
  else audioDesign[key] = value;
  if (audioDesign.source === 'file' && ['offsetMs','durationMs'].includes(key)) {
    const asset = project.audioAssets.find(a => a.id === audioDesign.assetId);
    const full = asset.frames / 48;
    audioDesign.offsetMs = Math.min(audioDesign.offsetMs, Math.max(0,full - 20));
    audioDesign.durationMs = Math.min(audioDesign.durationMs,60000,full - audioDesign.offsetMs);
    $$('[data-audio-param="offsetMs"]').forEach(el => {el.max=Math.max(0,full-20);el.value=audioDesign.offsetMs});
    $$('[data-audio-param="durationMs"]').forEach(el => {el.max=Math.min(60000,full-audioDesign.offsetMs);el.value=audioDesign.durationMs});
  }
  audioInspectMs = Math.min(audioInspectMs, audioDesign.durationMs);
  if ($('#audio-inspect')) $('#audio-inspect').max = audioDesign.durationMs;
  $$('[data-spatial-key]').forEach(b=>b.title=(audioDesign.keyframes[+b.dataset.spatialKey].at*audioDesign.durationMs).toFixed(1)+' ms');
  if ($('#audio-key-time')) $('#audio-key-time').value = +(editingPose().at*audioDesign.durationMs).toFixed(3);
}

async function selectSpatialKey(index) {
  await flushAudioDraft(); audioKeyIndex=index;
  audioInspectMs=audioDesign.keyframes[index].at*audioDesign.durationMs;
  audioEditor();
}

function bindSpatialEditor() {
  $('#audio-spatial').onchange=()=>attempt(async()=>{audioDesign.spatial=$('#audio-spatial').checked;audioRevision++;await flushAudioDraft();audioEditor()});
  $('#audio-motion').onchange=()=>attempt(async()=>{const mode=$('#audio-motion').value;if(mode==='path')Spatial.ensurePath(audioDesign);else audioDesign.motion=mode;audioKeyIndex=0;audioInspectMs=0;audioRevision++;await flushAudioDraft();audioEditor()});
  $('#spatial-preset').onchange=()=>attempt(async()=>{const id=$('#spatial-preset').value;if(!id)return;Spatial.applyPreset(audioDesign,id);audioKeyIndex=0;audioInspectMs=0;audioRevision++;await flushAudioDraft();audioEditor()});
  $('#audio-inspect').oninput=()=>{audioInspectMs=+$('#audio-inspect').value;drawAudioPosition()};
  $$('[data-spatial-key]').forEach(b=>b.onclick=()=>attempt(()=>selectSpatialKey(+b.dataset.spatialKey)));
  if ($('#audio-add-key')) {
    $('#audio-add-key').onclick=()=>attempt(async()=>{audioKeyIndex=Spatial.insert(audioDesign,audioInspectMs/audioDesign.durationMs);audioInspectMs=editingPose().at*audioDesign.durationMs;audioRevision++;await flushAudioDraft();audioEditor()});
    $('#audio-remove-key').onclick=()=>attempt(async()=>{if(audioKeyIndex===0||audioKeyIndex===audioDesign.keyframes.length-1)return;audioDesign.keyframes.splice(audioKeyIndex,1);audioKeyIndex--;audioInspectMs=editingPose().at*audioDesign.durationMs;audioRevision++;await flushAudioDraft();audioEditor()});
    $('#audio-key-time').onchange=()=>attempt(async()=>{const keys=audioDesign.keyframes,i=audioKeyIndex;if(i===0||i===keys.length-1)return;const at=Number($('#audio-key-time').value)/audioDesign.durationMs;if(!Number.isFinite(at)||at-keys[i-1].at<.001||keys[i+1].at-at<.001){$('#audio-key-time').value=keys[i].at*audioDesign.durationMs;throw Error('前後の通過点から再生時間の0.1%以上離してください')}keys[i].at=at;audioInspectMs=at*audioDesign.durationMs;audioRevision++;await flushAudioDraft();audioEditor()});
  }
  const map=$('#audio-map-control');
  const selectInMap=index=>{
    audioKeyIndex=index;audioInspectMs=editingPose().at*audioDesign.durationMs;
    $$('[data-spatial-key]').forEach(b=>b.setAttribute('aria-pressed',+b.dataset.spatialKey===index));
    for(const key of spatialKeys)$$(`[data-audio-param="${key}"]`).forEach(el=>el.value=editingPose()[key]);
    $('#audio-key-time').value=+(editingPose().at*audioDesign.durationMs).toFixed(3);
    const endpoint=index===0||index===audioDesign.keyframes.length-1;
    $('#audio-key-time').disabled=endpoint;$('#audio-remove-key').disabled=endpoint;
  };
  const update=e=>{
    if(!audioPointerEditing||!audioDesign.spatial)return;
    const r=map.getBoundingClientRect(),x=(e.clientX-r.left)/r.width*400-200,y=200-(e.clientY-r.top)/r.height*400;
    const p=editingPose();p.azimuth=Math.round(Math.atan2(x,y)*180/Math.PI);p.distanceM=Math.round(Math.max(.3,Math.min(5,(Math.hypot(x,y)-40)/24))*10)/10;
    for(const key of spatialKeys)$$(`[data-audio-param="${key}"]`).forEach(el=>el.value=p[key]);
    if(audioDesign.motion==='path')audioInspectMs=p.at*audioDesign.durationMs;
    queueAudioEdit();drawAudioPosition();
  };
  map.onpointerdown=e=>{
    if(!audioDesign.spatial)return;
    const r=map.getBoundingClientRect(),x=(e.clientX-r.left)/r.width*400,y=(e.clientY-r.top)/r.height*400;
    let nearest=-1,best=16;
    if(audioDesign.motion==='path')audioDesign.keyframes.forEach((k,i)=>{const angle=k.azimuth*Math.PI/180,radius=40+k.distanceM*24,delta=Math.hypot(200+Math.sin(angle)*radius-x,200-Math.cos(angle)*radius-y);if(delta<best){best=delta;nearest=i}});
    if(nearest>=0)selectInMap(nearest);
    audioPointerEditing=true;map.setPointerCapture(e.pointerId);map.focus();
    if(nearest<0)update(e);else drawAudioPosition();
  };
  map.onpointermove=update;
  const end=()=>{audioPointerEditing=false;attempt(flushAudioDraft)};
  map.onpointerup=end;map.onpointercancel=end;
  map.onkeydown=e=>{
    if(!audioDesign.spatial||!['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(e.key))return;
    e.preventDefault();const p=editingPose();
    if(e.key==='ArrowLeft'||e.key==='ArrowRight')p.azimuth=Spatial.wrap(p.azimuth+(e.key==='ArrowLeft'?-1:1));
    else p.distanceM=Math.round(Math.max(.3,Math.min(5,p.distanceM+(e.key==='ArrowUp'?-.1:.1)))*10)/10;
    if(audioDesign.motion==='path')audioInspectMs=p.at*audioDesign.durationMs;
    for(const key of spatialKeys)$$(`[data-audio-param="${key}"]`).forEach(el=>el.value=p[key]);
    queueAudioEdit();drawAudioPosition();
  };
}

function drawAudioPosition(timeMs=null) {
  if(!$('#audio-map')||!audioDesign)return;
  const d=audioDesign, playing=timeMs!==null;
  if(playing&&!audioPointerEditing)audioInspectMs=Math.max(0,Math.min(d.durationMs,timeMs));
  const p=audioPointerEditing?editingPose():Spatial.pose(d,audioInspectMs/d.durationMs);
  const xy=p=>{const rad=p.azimuth*Math.PI/180,r=40+p.distanceM*24;return [200+Math.sin(rad)*r,200-Math.cos(rad)*r]};
  const [x,y]=xy(p);
  const trace=Array.from({length:81},(_,i)=>Spatial.pose(d,i/80));
  const topPath=trace.map((p,i)=>(i?'L':'M')+xy(p).map(v=>v.toFixed(2)).join(' ')).join('');
  const points=d.motion==='path'?d.keyframes:[];
  $('#audio-map').innerHTML=`<rect width="400" height="400" fill="var(--aw-inset)" rx="8"/><path d="M200 24V376M24 200H376" stroke="var(--aw-line)"/>${[1,3,5].map(m=>`<circle cx="200" cy="200" r="${40+m*24}" fill="none" stroke="var(--aw-line)"/><text x="${205+40+m*24}" y="196" fill="var(--aw-sub)" font-size="10">${m} m</text>`).join('')}<g fill="var(--aw-sub)" font-size="13" text-anchor="middle"><text x="200" y="18">前</text><text x="200" y="395">後</text><text x="13" y="205">左</text><text x="388" y="205">右</text></g><circle cx="200" cy="200" r="22" fill="var(--aw-panel)" stroke="var(--aw-ink)"/><path d="M194 178L200 170L206 178M175 192V208M225 192V208" fill="none" stroke="var(--aw-ink)" stroke-width="2"/><path d="${topPath}" fill="none" stroke="var(--aw-sub)" stroke-width="2" stroke-dasharray="4 3"/>${points.map((k,i)=>{const [kx,ky]=xy(k);return `<g data-map-key="${i}"><circle cx="${kx}" cy="${ky}" r="${i===audioKeyIndex?10:7}" fill="var(--aw-panel)" stroke="var(--aw-ink)" stroke-width="${i===audioKeyIndex?3:1}"/><text x="${kx}" y="${ky-16}" text-anchor="middle" fill="var(--aw-ink)" font-size="12">${i===0?'開始':i===points.length-1?'終了':i}</text></g>`}).join('')}<circle cx="${x}" cy="${y}" r="6" fill="var(--aw-ink)" style="pointer-events:none"/>`;
  const sidePath=trace.map((p,i)=>(i?'L':'M')+(20+i/80*360).toFixed(2)+' '+(72-p.elevation*.6).toFixed(2)).join('');
  $('#audio-elevation-map').innerHTML=`<path d="M20 72H380" stroke="var(--aw-line)"/><path d="${sidePath}" fill="none" stroke="var(--aw-sub)"/><circle cx="${20+audioInspectMs/d.durationMs*360}" cy="${72-p.elevation*.6}" r="5" fill="var(--aw-ink)"/><text x="20" y="15" font-size="11" fill="var(--aw-sub)">高さ</text><text x="380" y="96" text-anchor="end" font-size="11" fill="var(--aw-sub)">${(d.durationMs/1000).toFixed(2)} s</text>`;
  $('#audio-inspect').value=audioInspectMs;$('#audio-inspect').disabled=playing;
  $('#audio-inspect-value').textContent=(audioInspectMs/1000).toFixed(2)+' / '+(d.durationMs/1000).toFixed(2)+' s';
  $('#audio-position-readout').textContent=`方位 ${p.azimuth.toFixed(0)}° · 仰角 ${p.elevation.toFixed(0)}° · ${p.distanceM.toFixed(1)} m`;
}
